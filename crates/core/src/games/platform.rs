//! 平台游戏扫描：Steam / Epic / WeGame / 米哈游 + 内置名单。
//!
//! 稳定性原则：所有 I/O 全容错（目录缺失、解析失败一律跳过并继续），
//! 绝不 panic、绝不阻塞调用方。返回的是「尽力而为」的发现结果。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::games::KNOWN_GAMES;

/// 扫描发现的一个游戏（尚未入库）。
#[derive(Debug, Clone)]
pub struct FoundGame {
    /// 游戏显示名。
    pub title: String,
    /// 游戏可执行文件完整路径（known 名单为 exe 文件名 stem）。
    pub exe_path: String,
    /// 规范化应用名（进程 stem；known 名单用）。
    pub app_name: String,
    /// 来源：steam / epic / wegame / mihoyo / known。
    pub source: &'static str,
    /// 平台应用 ID。
    pub appid: Option<String>,
}

/// 扫描全部平台并去重（按 exe_path 大小写不敏感）。
pub fn scan_all_platforms() -> Vec<FoundGame> {
    let mut out = Vec::new();
    out.extend(scan_steam_games());
    out.extend(scan_epic_games());
    out.extend(scan_wegame_games());
    out.extend(scan_mihoyo_games());
    out.extend(known_games());
    dedupe(out)
}

fn dedupe(games: Vec<FoundGame>) -> Vec<FoundGame> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for g in games {
        if seen.insert(g.exe_path.to_lowercase()) {
            out.push(g);
        }
    }
    out
}

fn known_games() -> Vec<FoundGame> {
    KNOWN_GAMES
        .iter()
        .map(|(stem, title)| FoundGame {
            title: (*title).to_string(),
            exe_path: (*stem).to_string(),
            app_name: (*stem).to_string(),
            source: "known",
            appid: None,
        })
        .collect()
}

// ── Steam ────────────────────────────────────────────────────────

fn steam_root() -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    if let Ok(steam) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam")
        && let Ok(p) = steam.get_value::<String, _>("SteamPath")
        && !p.is_empty()
    {
        return Some(PathBuf::from(p));
    }
    for candidate in ["C:\\Program Files (x86)\\Steam", "C:\\Program Files\\Steam"] {
        let p = PathBuf::from(candidate);
        if p.join("steamapps").exists() {
            return Some(p);
        }
    }
    None
}

/// 扫描 Steam 已安装游戏。返回 (title, appid, installdir) 列表。
fn steam_installed(root: &Path) -> Vec<(String, String, String)> {
    let mut libs = vec![root.to_path_buf()];
    let vdf = root.join("steamapps").join("libraryfolders.vdf");
    if let Ok(text) = std::fs::read_to_string(&vdf) {
        for (k, v) in parse_vdf_pairs(&text) {
            if k == "path" && !v.is_empty() {
                let p = PathBuf::from(v.replace("\\\\", "\\"));
                if !libs.iter().any(|l| l == &p) {
                    libs.push(p);
                }
            }
        }
    }
    let mut out = Vec::new();
    for lib in libs {
        let apps_dir = lib.join("steamapps");
        let Ok(entries) = std::fs::read_dir(&apps_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let pairs = parse_vdf_pairs(&text);
            let get = |key: &str| {
                pairs
                    .iter()
                    .rev()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.clone())
            };
            if let (Some(appid), Some(title), Some(installdir)) =
                (get("appid"), get("name"), get("installdir"))
            {
                out.push((title, appid, installdir));
            }
        }
    }
    out
}

pub fn scan_steam_games() -> Vec<FoundGame> {
    let Some(root) = steam_root() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (title, appid, installdir) in steam_installed(&root) {
        // 每个库根目录下 common/<installdir> 都可能是游戏位置
        let mut found_any = false;
        let mut libs = vec![root.to_path_buf()];
        if let Ok(text) = std::fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf"))
        {
            for (k, v) in parse_vdf_pairs(&text) {
                if k == "path" && !v.is_empty() {
                    let p = PathBuf::from(v.replace("\\\\", "\\"));
                    if !libs.iter().any(|l| l == &p) {
                        libs.push(p);
                    }
                }
            }
        }
        for lib in libs {
            let common = lib.join("steamapps").join("common").join(&installdir);
            for exe in scan_exes(&common, 0) {
                out.push(FoundGame {
                    title: title.clone(),
                    exe_path: exe.to_string_lossy().into_owned(),
                    app_name: crate::games::exe_stem(&exe.to_string_lossy()).to_string(),
                    source: "steam",
                    appid: Some(appid.clone()),
                });
                found_any = true;
            }
            // 找不到 exe 时用安装目录兜底（目录前缀匹配）
            if !found_any && common.exists() {
                out.push(FoundGame {
                    title: title.clone(),
                    exe_path: common.to_string_lossy().into_owned(),
                    app_name: installdir.clone(),
                    source: "steam",
                    appid: Some(appid.clone()),
                });
            }
        }
    }
    out
}

/// 极简 VDF 键值对提取：扫描 `"key" "value"` 双引号配对（跨嵌套层级，顺序保留）。
/// 用于 libraryfolders.vdf 与 appmanifest_*.acf；解析失败返回空（容错）。
pub fn parse_vdf_pairs(text: &str) -> Vec<(String, String)> {
    let bytes = text.as_bytes();
    let mut pairs = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let Some(end) = find_quote(bytes, i + 1) else {
            break;
        };
        let key = String::from_utf8_lossy(&bytes[i + 1..end]).into_owned();
        i = end + 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'"' {
            let Some(end2) = find_quote(bytes, i + 1) else {
                break;
            };
            // VDF 字符串里的 \\ 是转义反斜杠（如 "D:\\Steam"），还原为单反斜杠。
            let val = String::from_utf8_lossy(&bytes[i + 1..end2])
                .into_owned()
                .replace("\\\\", "\\");
            pairs.push((key, val));
            i = end2 + 1;
        }
    }
    pairs
}

fn find_quote(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            // VDF 转义 \"：跳过（路径/名字里极少出现）
            if i > 0 && bytes[i - 1] == b'\\' {
                i += 1;
                continue;
            }
            return Some(i);
        }
        i += 1;
    }
    None
}

// ── Epic ─────────────────────────────────────────────────────────

pub fn scan_epic_games() -> Vec<FoundGame> {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".into());
    let dir = PathBuf::from(program_data)
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("item"))
            .unwrap_or(false)
            && let Some(g) = parse_epic_item(&path)
        {
            out.push(g);
        }
    }
    out
}

fn parse_epic_item(path: &Path) -> Option<FoundGame> {
    #[derive(serde::Deserialize)]
    struct Item {
        #[serde(rename = "DisplayName")]
        display_name: Option<String>,
        #[serde(rename = "InstallLocation")]
        install_location: Option<String>,
        #[serde(rename = "LaunchExecutable")]
        launch_executable: Option<String>,
        #[serde(rename = "AppName")]
        app_name: Option<String>,
    }
    let text = std::fs::read_to_string(path).ok()?;
    let item: Item = serde_json::from_str(&text).ok()?;
    let title = item.display_name?;
    let install = item.install_location?;
    let exe_rel = item.launch_executable?;
    if install.is_empty() || exe_rel.is_empty() {
        return None;
    }
    let exe_path = Path::new(&install).join(&exe_rel);
    Some(FoundGame {
        title,
        exe_path: exe_path.to_string_lossy().into_owned(),
        app_name: crate::games::exe_stem(&exe_rel).to_string(),
        source: "epic",
        appid: item.app_name,
    })
}

// ── WeGame ───────────────────────────────────────────────────────

pub fn scan_wegame_games() -> Vec<FoundGame> {
    let Some(root) = wegame_root() else {
        return Vec::new();
    };
    let games_dir = root.join("games");
    let Ok(entries) = std::fs::read_dir(&games_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let dir = entry.path();
        let title = entry.file_name().to_string_lossy().into_owned();
        for exe in scan_exes(&dir, 0) {
            out.push(FoundGame {
                title: title.clone(),
                exe_path: exe.to_string_lossy().into_owned(),
                app_name: crate::games::exe_stem(&exe.to_string_lossy()).to_string(),
                source: "wegame",
                appid: None,
            });
        }
    }
    out
}

fn wegame_root() -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    for (hive, sub) in [
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\WOW6432Node\\Tencent\\WeGame"),
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\Tencent\\WeGame"),
        (HKEY_CURRENT_USER, "Software\\Tencent\\WeGame"),
    ] {
        if let Ok(key) = RegKey::predef(hive).open_subkey(sub) {
            for val in [
                "InstallPath",
                "InstallDir",
                "install_path",
                "Path",
                "InstallRoot",
            ] {
                if let Ok(p) = key.get_value::<String, _>(val)
                    && !p.is_empty()
                {
                    return Some(PathBuf::from(p));
                }
            }
        }
    }
    for c in [
        "C:\\Program Files\\WeGame",
        "C:\\Program Files (x86)\\WeGame",
    ] {
        let p = PathBuf::from(c);
        if p.join("games").exists() {
            return Some(p);
        }
    }
    None
}

// ── 米哈游（注册表尽力而为；真正兜底是 KNOWN_GAMES）─────────────

pub fn scan_mihoyo_games() -> Vec<FoundGame> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let attempts: &[(&str, &str)] = &[
        ("Genshin Impact", "原神"),
        ("Star Rail", "崩坏：星穹铁道"),
        ("ZenlessZoneZero", "绝区零"),
        ("BH3", "崩坏3"),
    ];
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let mut out = Vec::new();
    for (sub, title) in attempts {
        let key_path = format!("Software\\miHoYo\\{sub}");
        let Ok(key) = hkcu.open_subkey(&key_path) else {
            continue;
        };
        for val in ["GamePath", "InstallPath", "Path", "gamePath", "installPath"] {
            let Ok(p) = key.get_value::<String, _>(val) else {
                continue;
            };
            if p.is_empty() {
                continue;
            }
            let dir = PathBuf::from(&p);
            if let Some(exe_path) = find_mihoyo_exe(&dir) {
                out.push(FoundGame {
                    title: (*title).to_string(),
                    exe_path,
                    app_name: String::new(),
                    source: "mihoyo",
                    appid: None,
                });
            }
            break;
        }
    }
    out
}

fn find_mihoyo_exe(dir: &Path) -> Option<String> {
    for exe in scan_exes(dir, 0) {
        let stem = exe
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if ["genshinimpact", "starrail", "zenlesszonezero", "bh3"]
            .iter()
            .any(|k| stem.contains(k))
        {
            return Some(exe.to_string_lossy().into_owned());
        }
    }
    None
}

// ── 通用 exe 扫描 ────────────────────────────────────────────────

const EXE_SCAN_MAX_DEPTH: usize = 4;
/// 常见非游戏目录（引擎/运行库），跳过避免噪声与耗时。
const SKIP_DIRS: &[&str] = &[
    "_commonredist",
    "redist",
    "steamworks shared",
    "vc_redist",
    "directx",
    "drivers",
    "engine",
    "ue_4",
    "ue_5",
    "bins",
    "crashreport",
];

/// 递归收集目录下的 .exe 文件（限深度；跳过常见非游戏目录）。
pub fn scan_exes(dir: &Path, depth: usize) -> Vec<PathBuf> {
    if depth > EXE_SCAN_MAX_DEPTH {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let ft = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if ft.is_dir() {
            let lower = entry.file_name().to_string_lossy().to_lowercase();
            if SKIP_DIRS.iter().any(|s| lower.contains(s)) {
                continue;
            }
            out.extend(scan_exes(&path, depth + 1));
        } else if ft.is_file()
            && path
                .extension()
                .map(|e| e.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
        {
            out.push(path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vdf_pairs_extract_keys() {
        let vdf = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"D:\\Steam"
		"label"		""
	}
	"1"
	{
		"path"		"E:\\Games"
	}
}
"#;
        let pairs = parse_vdf_pairs(vdf);
        let paths: Vec<&str> = pairs
            .iter()
            .filter(|(k, _)| k == "path")
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(paths, vec![r"D:\Steam", r"E:\Games"]);
    }

    #[test]
    fn vdf_appmanifest_parses() {
        let acf = r#"
"AppState"
{
	"appid"		"1245620"
	"name"		"ELDEN RING"
	"installdir"		"ELDEN RING"
}
"#;
        let pairs = parse_vdf_pairs(acf);
        let get = |k: &str| {
            pairs
                .iter()
                .rev()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(get("appid").as_deref(), Some("1245620"));
        assert_eq!(get("name").as_deref(), Some("ELDEN RING"));
        assert_eq!(get("installdir").as_deref(), Some("ELDEN RING"));
    }

    #[test]
    fn epic_item_parses() {
        let dir = std::env::temp_dir().join(format!("tt_epic_{}.item", std::process::id()));
        std::fs::write(
            &dir,
            r#"{
  "DisplayName": "Fortnite",
  "InstallLocation": "D:\\Epic\\Fortnite",
  "LaunchExecutable": "FortniteGame\\Binaries\\Win64\\FortniteClient-Win64-Shipping.exe",
  "AppName": "Fortnite"
}"#,
        )
        .unwrap();
        let g = parse_epic_item(&dir).unwrap();
        assert_eq!(g.title, "Fortnite");
        assert_eq!(
            g.exe_path,
            r"D:\Epic\Fortnite\FortniteGame\Binaries\Win64\FortniteClient-Win64-Shipping.exe"
        );
        assert_eq!(g.source, "epic");
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn epic_item_missing_fields_returns_none() {
        let dir = std::env::temp_dir().join(format!("tt_epic_bad_{}.item", std::process::id()));
        std::fs::write(&dir, "{}").unwrap();
        assert!(parse_epic_item(&dir).is_none());
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn dedupe_by_path_ci() {
        let games = vec![
            FoundGame {
                title: "A".into(),
                exe_path: r"D:\Game\a.exe".into(),
                app_name: String::new(),
                source: "steam",
                appid: None,
            },
            FoundGame {
                title: "B".into(),
                exe_path: r"d:\game\A.EXE".into(),
                app_name: String::new(),
                source: "known",
                appid: None,
            },
        ];
        let out = dedupe(games);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "A");
    }
}
