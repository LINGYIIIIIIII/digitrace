//! 平台游戏扫描：Steam / Epic / WeGame / 米哈游 + 内置名单。
//!
//! 稳定性原则：所有 I/O 全容错（目录缺失、解析失败一律跳过并继续），
//! 绝不 panic、绝不阻塞调用方。返回的是「尽力而为」的发现结果。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
    out.extend(scan_steam_shortcuts());
    out.extend(scan_epic_games());
    out.extend(scan_wegame_games());
    out.extend(scan_mihoyo_games());
    dedupe(out)
}

// ── Steam 非游戏快捷方式（shortcuts.vdf）───────────────────────────
//
// 玩家常把米哈游等非 Steam 游戏通过 Steam 添加为快捷方式，启动 exe 记录在
// userdata\<id>\config\shortcuts.vdf。这是最权威的「启动 exe」来源，且能
// 精确对应到那款游戏（图标用真正的启动 exe，而非目录里随便一个）。

/// 读取 Steam 各 userdata 的 shortcuts.vdf，返回「游戏名 → 启动 exe 路径」。
/// 只保留「已知游戏 exe 名」命中的，避免把无关快捷方式塞进游戏库。
fn scan_steam_shortcuts() -> Vec<FoundGame> {
    let Some(root) = steam_root() else {
        return Vec::new();
    };
    let userdata = root.join("userdata");
    let Ok(entries) = std::fs::read_dir(&userdata) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let vdf = entry.path().join("config").join("shortcuts.vdf");
        let Ok(bytes) = std::fs::read(&vdf) else {
            continue;
        };
        for exe in parse_shortcut_exes(&bytes) {
            let Some(g) = shortcut_to_game(&exe) else {
                continue;
            };
            let key = g.exe_path.to_lowercase();
            if seen.insert(key) {
                out.push(g);
            }
        }
    }
    out
}

/// 从 shortcuts.vdf 的字节里提取所有 `Exe` 值（带引号的路径）。二进制 VDF 里
/// Exe 字段值形如 `Exe\x00"路径"`，这里扫描 `Exe\x00` 后的引号字符串。
fn parse_shortcut_exes(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let needle = b"Exe\0";
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let mut j = i + needle.len();
            // 跳过多余的字段分隔字节直到遇到引号
            while j < bytes.len() && bytes[j] != b'"' {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'"' {
                j += 1;
                let start = j;
                while j < bytes.len() && bytes[j] != b'"' {
                    j += 1;
                }
                if j < bytes.len()
                    && bytes[j] == b'"'
                    && let Ok(s) = std::str::from_utf8(&bytes[start..j])
                {
                    out.push(s.to_string());
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// 把一个启动 exe 路径映射成 FoundGame：exe 文件名 stem 命中已知游戏才返回。
/// 匹配收紧：短 key（<5 字符）只允许全等或「key + 非字母数字后缀」，
/// 避免 `contains("cf")` 误伤 cloudflared 等。
fn game_key_matches(stem: &str, key: &str) -> bool {
    if stem == key {
        return true;
    }
    if key.len() >= 5 {
        return stem.contains(key);
    }
    // 短 key：仅允许 key 作完整前缀 token（后接 _ / - 或结束），避免 contains 误伤。
    match stem.strip_prefix(key) {
        Some(rest) => rest.is_empty() || rest.starts_with('_') || rest.starts_with('-'),
        None => false,
    }
}

fn shortcut_to_game(exe: &str) -> Option<FoundGame> {
    let stem = crate::games::exe_stem(exe).to_lowercase();
    // 优先米哈游（用 MIHOYO_EXE_TITLES 映射可读中文名），再走通用 KNOWN_GAMES。
    let title = MIHOYO_EXE_TITLES
        .iter()
        .find(|(k, _)| game_key_matches(&stem, k))
        .map(|(_, t)| (*t).to_string())
        .or_else(|| {
            crate::games::KNOWN_GAMES
                .iter()
                .find(|(k, _)| game_key_matches(&stem, k))
                .map(|(_, t)| (*t).to_string())
        })?;
    Some(FoundGame {
        title,
        exe_path: exe.to_string(),
        app_name: stem,
        source: "steam-shortcut",
        appid: None,
    })
}

/// 归一化路径用于比较：统一小写 + `/`→`\` + 去尾部斜杠。
/// 避免 Windows 下 `C:\X` 与 `c:/x` 这种同路径不同写法被当成两条。
pub fn normalize_path(p: &str) -> String {
    p.replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

fn dedupe(games: Vec<FoundGame>) -> Vec<FoundGame> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for g in games {
        if seen.insert(normalize_path(&g.exe_path)) {
            out.push(g);
        }
    }
    out
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

/// 解析 Steam 库根目录列表：主库 + libraryfolders.vdf 里声明的其它库。
/// 同一次扫描只读一次 vdf，避免外层对每个游戏重复解析。
fn steam_libraries(root: &Path) -> Vec<PathBuf> {
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
    libs
}

/// 扫描 Steam 已安装游戏。返回 (title, appid, installdir) 列表。
fn steam_installed(libs: &[PathBuf]) -> Vec<(String, String, String)> {
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
    let libs = steam_libraries(&root);
    let mut out = Vec::new();
    for (title, appid, installdir) in steam_installed(&libs) {
        // 跳过 Steam 上的非游戏应用/工具（如 Wallpaper Engine 动态壁纸），
        // 它们会往游戏库塞大量无关 exe（applicationwallpaperinject32 等）。
        let il = installdir.to_lowercase();
        if NON_GAME_STEAM_DIRS.iter().any(|d| il.contains(d)) {
            continue;
        }
        // 每个库根目录下 common/<installdir> 都可能是游戏位置；命中主 exe 即停。
        for lib in &libs {
            let common = lib.join("steamapps").join("common").join(&installdir);
            // 只取一个「主 exe」而非目录下全部 exe：很多游戏目录会混入
            // helper/引擎组件等额外 exe，全收会导致游戏库出现大量同名/无关条目。
            // 优先选文件名与 installdir 匹配的 exe（更接近真正启动的主程序），
            // 否则回退到扫描到的第一个。
            let exes = scan_exes(&common, 0);
            let instal = installdir.to_lowercase();
            let chosen = exes
                .iter()
                .find(|e| {
                    let stem = e
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    stem.contains(&instal) || instal.contains(&stem)
                })
                .or_else(|| exes.first())
                .cloned();
            if let Some(exe) = chosen {
                out.push(FoundGame {
                    title: title.clone(),
                    exe_path: exe.to_string_lossy().into_owned(),
                    app_name: crate::games::exe_stem(&exe.to_string_lossy()).to_string(),
                    source: "steam",
                    appid: Some(appid.clone()),
                });
                break;
            }
            // 找不到 exe 时用安装目录兜底（目录前缀匹配）
            if common.exists() {
                out.push(FoundGame {
                    title: title.clone(),
                    exe_path: common.to_string_lossy().into_owned(),
                    app_name: installdir.clone(),
                    source: "steam",
                    appid: Some(appid.clone()),
                });
                break;
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
//
// 安装形态不唯一：有的在 <root>\games\<Game>\，有的库挂在 <root>\apps\，
// 注册表键/值名也随版本变化。因此改为「收集全部候选根 → 各扫一层 →
// 按 exe 路径去重」，不再第一个命中就返回。

/// WeGame 下明显不是游戏的一级目录名（全小写，contains 匹配）。
const WEGAME_NON_GAME_DIRS: &[&str] = &[
    "crashreport",
    "crash_report",
    "crash",
    "uninstall",
    "unins",
    "download",
    "downloading",
    "cache",
    "temp",
    "tmp",
    "logs",
    "log",
    "update",
    "updater",
    "redist",
    "directx",
    "vcredist",
    "common",
    "launcher",
    "browser",
    "cef",
    "webview",
];

/// 在 WeGame 游戏目录里挑一个主 exe：
/// 1. 优先 exe stem 与目录名互相包含（小写）；
/// 2. 否则跳过 NON_GAME_EXES 后取第一个（scan_exes 已过滤）。
fn pick_wegame_exe(dir: &Path, title: &str) -> Option<PathBuf> {
    let exes = scan_exes(dir, 0);
    if exes.is_empty() {
        return None;
    }
    let title_lc = title.to_lowercase();
    for exe in &exes {
        let stem = exe
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !stem.is_empty() && (stem.contains(&title_lc) || title_lc.contains(&stem)) {
            return Some(exe.clone());
        }
    }
    Some(exes[0].clone())
}

/// 目录名是否像非游戏（crashreport / uninstall / download 等）。
fn is_wegame_non_game_dir(name: &str) -> bool {
    let n = name.to_lowercase();
    WEGAME_NON_GAME_DIRS.iter().any(|d| n.contains(d))
}

/// 收集全部可能的 WeGame 安装根（去重，不短路返回）。
fn wegame_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let push_unique = |p: PathBuf, roots: &mut Vec<PathBuf>| {
        if p.as_os_str().is_empty() {
            return;
        }
        if !roots.iter().any(|r| r == &p) {
            roots.push(p);
        }
    };

    // 1) 注册表：HKLM/HKCU 多键多值，全部收集。
    {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        for (hive, sub) in [
            (HKEY_LOCAL_MACHINE, "SOFTWARE\\WOW6432Node\\Tencent\\WeGame"),
            (HKEY_LOCAL_MACHINE, "SOFTWARE\\Tencent\\WeGame"),
            (
                HKEY_LOCAL_MACHINE,
                "SOFTWARE\\WOW6432Node\\Tencent\\WeGameLauncher",
            ),
            (HKEY_LOCAL_MACHINE, "SOFTWARE\\Tencent\\WeGameLauncher"),
            (HKEY_CURRENT_USER, "Software\\Tencent\\WeGame"),
            (HKEY_CURRENT_USER, "Software\\Tencent\\WeGameLauncher"),
        ] {
            if let Ok(key) = RegKey::predef(hive).open_subkey(sub) {
                for val in [
                    "InstallPath",
                    "InstallDir",
                    "install_path",
                    "Path",
                    "InstallRoot",
                    "RootPath",
                ] {
                    if let Ok(p) = key.get_value::<String, _>(val)
                        && !p.is_empty()
                    {
                        push_unique(PathBuf::from(p), &mut roots);
                    }
                }
            }
        }
    }

    // 2) 默认路径 + 常见变体（Tencent\WeGame、WeGame\apps）。
    for c in [
        "C:\\Program Files\\WeGame",
        "C:\\Program Files (x86)\\WeGame",
        "C:\\Program Files\\Tencent\\WeGame",
        "C:\\Program Files (x86)\\Tencent\\WeGame",
    ] {
        push_unique(PathBuf::from(c), &mut roots);
        push_unique(PathBuf::from(c).join("apps"), &mut roots);
    }

    // 3) 各盘符下 Program Files\WeGame。
    for drive in ["C:\\", "D:\\", "E:\\", "F:\\", "G:\\"] {
        if !Path::new(drive).exists() {
            continue;
        }
        push_unique(
            PathBuf::from(drive).join("Program Files").join("WeGame"),
            &mut roots,
        );
        push_unique(
            PathBuf::from(drive)
                .join("Program Files (x86)")
                .join("WeGame"),
            &mut roots,
        );
        push_unique(
            PathBuf::from(drive).join("Tencent").join("WeGame"),
            &mut roots,
        );
    }

    roots
}

/// 扫描单个 WeGame 根：优先 root\games\*，若根文件名为 apps 再扫根一级。
fn scan_wegame_under(root: &Path) -> Vec<FoundGame> {
    let mut out = Vec::new();
    let games_dir = root.join("games");
    let mut scanned: Vec<PathBuf> = Vec::new();
    if games_dir.is_dir() {
        scanned.push(games_dir);
    }
    let is_apps = root
        .file_name()
        .map(|n| n.to_string_lossy().eq_ignore_ascii_case("apps"))
        .unwrap_or(false);
    if is_apps {
        scanned.push(root.to_path_buf());
    }
    for base in scanned {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let title = entry.file_name().to_string_lossy().into_owned();
            if is_wegame_non_game_dir(&title) {
                continue;
            }
            let dir = entry.path();
            let Some(exe) = pick_wegame_exe(&dir, &title) else {
                continue;
            };
            out.push(FoundGame {
                title,
                exe_path: exe.to_string_lossy().into_owned(),
                app_name: crate::games::exe_stem(&exe.to_string_lossy()).to_string(),
                source: "wegame",
                appid: None,
            });
        }
    }
    out
}

/// 扫描全部 WeGame 候选根，按 exe 路径去重后返回。
pub fn scan_wegame_games() -> Vec<FoundGame> {
    let mut out: Vec<FoundGame> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for root in wegame_roots() {
        for g in scan_wegame_under(&root) {
            let key = normalize_path(&g.exe_path);
            if seen.insert(key) {
                out.push(g);
            }
        }
    }
    out
}

// ── 米哈游（磁盘目录扫描 + 注册表兜底）──────────────────────────────────
//
// 米哈游/崩铁等游戏的安装路径并不总写入可读的注册表值（很多只留 SDK 缓存），
// 且启动器版本各异。因此以「扫描已知位置 + 常见安装目录」为主：
//  1) 各盘符根目录、Program Files(x86) 下的一级目录，匹配米哈游游戏名特征；
//  2) 在这些目录里递归找游戏 exe（GenshinImpact/StarRail/ZenlessZoneZero/BH3 等）；
//  3) 注册表 HKCU\Software\miHoYo\logkey 的安装路径兜底。
// 所有 I/O 全容错，绝不 panic。

/// 米哈游已知游戏：exe 文件名子串 → 显示名。
const MIHOYO_EXE_TITLES: &[(&str, &str)] = &[
    ("genshinimpact", "原神"),
    ("yuanshen", "原神"),
    ("starrail", "崩坏：星穹铁道"),
    ("zenlesszonezero", "绝区零"),
    ("bh3", "崩坏3"),
    ("bh3.exe", "崩坏3"),
];

/// 米哈游相关目录名特征（用于定位安装/启动器目录）。
const MIHOYO_DIR_FEATURES: &[&str] = &[
    "genshin",
    "star rail",
    "starrail",
    "zenless",
    "mihoyo",
    "hoyoplay",
    "hoyo",
    "原神",
    "崩坏",
    "绝区零",
    "honkai",
];

/// 常见安装根：盘符根目录 + Program Files(x86)。
fn mihoyo_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    // 盘符必须带尾部反斜杠（"D:\"），否则 PathBuf::from("D:") 是相对路径，
    // 导致 read_dir 出的路径畸形（如 "D:Star Rail Game"，缺分隔符），
    // 与 shortcuts.vdf 的 "D:\Star Rail Game" 无法归一化合并 → 星穹铁道重复。
    for drive in ["C:\\", "D:\\", "E:\\", "F:\\", "G:\\"] {
        if !Path::new(drive).exists() {
            continue;
        }
        roots.push(PathBuf::from(drive));
        roots.push(PathBuf::from(drive).join("Program Files"));
        roots.push(PathBuf::from(drive).join("Program Files (x86)"));
        // 米哈游启动器（HoYoPlay）把游戏装到 <Program Files>\miHoYo Launcher\games\
        // 下（每个游戏一个子目录，如 ZenlessZoneZero Game / Genshin Impact Game）。
        // 需要把这个父级加入扫描根，才能发现 C 盘的这两款游戏。
        roots.push(
            PathBuf::from(drive)
                .join("Program Files")
                .join("miHoYo Launcher")
                .join("games"),
        );
        roots.push(
            PathBuf::from(drive)
                .join("Program Files (x86)")
                .join("miHoYo Launcher")
                .join("games"),
        );
    }
    roots
}

pub fn scan_mihoyo_games() -> Vec<FoundGame> {
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // 1) 扫描常见安装根下符合特征的一级目录，递归找游戏 exe。
    for root in mihoyo_search_roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !MIHOYO_DIR_FEATURES.iter().any(|f| name.contains(f)) {
                continue;
            }
            collect_mihoyo_exe(entry.path(), &mut seen, &mut out);
        }
    }

    // 2) 注册表兜底：HKCU\Software\miHoYo\<logkey> 的安装路径。
    collect_from_registry(&mut seen, &mut out);

    out
}

/// 在一个可能包含米哈游游戏的目录里递归找游戏 exe（限深，跳过非游戏子目录）。
fn collect_mihoyo_exe(dir: PathBuf, seen: &mut HashSet<String>, out: &mut Vec<FoundGame>) {
    for exe in scan_exes(&dir, 0) {
        let stem = exe
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if let Some((_, title)) = MIHOYO_EXE_TITLES.iter().find(|(k, _)| stem.contains(k)) {
            let exe_str = exe.to_string_lossy().into_owned();
            if seen.insert(exe_str.to_lowercase()) {
                out.push(FoundGame {
                    title: (*title).to_string(),
                    exe_path: exe_str,
                    app_name: stem.clone(),
                    source: "mihoyo",
                    appid: None,
                });
            }
        }
    }
}

/// 注册表兜底：读各游戏子键下的安装路径键。
fn collect_from_registry(seen: &mut HashSet<String>, out: &mut Vec<FoundGame>) {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let attempts: &[(&str, &str)] = &[
        ("Genshin Impact", "原神"),
        ("Star Rail", "崩坏：星穹铁道"),
        ("ZenlessZoneZero", "绝区零"),
        ("BH3", "崩坏3"),
        ("HYP", "崩坏：星穹铁道"),
    ];
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for (sub, _) in attempts {
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
            let mut tmp = Vec::new();
            collect_mihoyo_exe(dir, seen, &mut tmp);
            for g in tmp {
                out.push(g);
            }
            break;
        }
    }
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

/// Steam 安装目录里的非游戏应用（壁纸/工具/运行库），跳过不当作游戏。
const NON_GAME_STEAM_DIRS: &[&str] = &[
    "wallpaper_engine",
    "wallpaper engine",
    "steamworks shared",
    "steamworks common redistributables",
    "steam controller configs",
];

/// 非游戏/工具类 exe 文件名（小写）黑名单：扫描时排除。
/// 这些常出现在游戏目录里，若被当主 exe 会造成图标/启动 exe 错误
/// （如 createdump/crashpad 是崩溃处理器，BEService 是反作弊服务）。
const NON_GAME_EXES: &[&str] = &[
    "crashpad_handler",
    "crashpad_uploader",
    "createdump",
    "crash_uploader",
    "crashreport",
    "unitycrashhandler64",
    "crumble",
    "beservice_x64",
    "winmtr",
    "msedgewebview2",
    "dxsetup",
    "vcredist",
    "steamcmd",
    "steamworks",
    "setup",
    "uninstall",
    "unins000",
    "installermessage",
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
            let stem = entry.file_name().to_string_lossy().to_lowercase();
            // 排除崩溃处理器/反作弊/安装器等非游戏 exe，避免被当成主 exe。
            if NON_GAME_EXES.iter().any(|n| stem.contains(n)) {
                continue;
            }
            out.push(path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_mihoyo_exe_finds_star_rail_dir() {
        // 模拟 D:\Star Rail Game\StarRail.exe 这类自定义安装目录。
        let dir = std::env::temp_dir().join(format!("tt_mihoyo_sr_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("StarRail.exe");
        std::fs::write(&exe, b"MZ fake").unwrap();

        let mut seen = HashSet::new();
        let mut out = Vec::new();
        collect_mihoyo_exe(dir.clone(), &mut seen, &mut out);
        assert_eq!(out.len(), 1, "应发现一个星穹铁道条目");
        assert_eq!(out[0].title, "崩坏：星穹铁道");
        assert!(out[0].exe_path.ends_with("StarRail.exe"));
        assert_eq!(out[0].source, "mihoyo");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_mihoyo_exe_finds_yuanshen_and_zenless() {
        // 模拟 miHoYo Launcher 下的两个游戏目录：exe 名分别是
        // YuanShen.exe（原神）与 ZenlessZoneZero.exe（绝区零）。
        let base = std::env::temp_dir().join(format!("tt_mihoyo_hy_{}", std::process::id()));
        let ys = base.join("Genshin Impact Game");
        std::fs::create_dir_all(&ys).unwrap();
        std::fs::write(ys.join("YuanShen.exe"), b"MZ").unwrap();
        let zz = base.join("ZenlessZoneZero Game");
        std::fs::create_dir_all(&zz).unwrap();
        std::fs::write(zz.join("ZenlessZoneZero.exe"), b"MZ").unwrap();

        let mut seen = HashSet::new();
        let mut out = Vec::new();
        collect_mihoyo_exe(base.clone(), &mut seen, &mut out);
        let titles: Vec<&str> = out.iter().map(|g| g.title.as_str()).collect();
        assert!(titles.contains(&"原神"), "应发现原神，实际: {titles:?}");
        assert!(titles.contains(&"绝区零"), "应发现绝区零，实际: {titles:?}");
        assert_eq!(out.len(), 2, "应有两条米哈游条目，实际: {titles:?}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn collect_mihoyo_exe_skips_non_game_exe() {
        let dir = std::env::temp_dir().join(format!("tt_mihoyo_non_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("launcher.exe"), b"MZ").unwrap();

        let mut seen = HashSet::new();
        let mut out = Vec::new();
        collect_mihoyo_exe(dir.clone(), &mut seen, &mut out);
        assert_eq!(out.len(), 0, "launcher.exe 不应被当作游戏");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_shortcut_exes_extracts_paths() {
        // 模拟 shortcuts.vdf 里的 Exe 字段字节：`Exe\0"路径"\0`。
        let bytes = b"appid\0\x01\x02\x03\x04AppName\0Zenless\0Exe\0\"C:\\Program Files\\miHoYo Launcher\\games\\ZenlessZoneZero Game\\ZenlessZoneZero.exe\"\0StartDir\0x\0";
        let exes = parse_shortcut_exes(bytes);
        assert_eq!(exes.len(), 1);
        assert!(exes[0].ends_with("ZenlessZoneZero.exe"));
    }

    #[test]
    fn shortcut_to_game_maps_mihoyo() {
        let g = shortcut_to_game(
            r"C:\Program Files\miHoYo Launcher\games\Genshin Impact Game\YuanShen.exe",
        )
        .unwrap();
        assert_eq!(g.title, "原神");
        assert_eq!(g.source, "steam-shortcut");
        let g2 = shortcut_to_game(r"D:\Star Rail Game\StarRail.exe").unwrap();
        assert_eq!(g2.title, "崩坏：星穹铁道");
        // 无关 exe 不应命中
        assert!(shortcut_to_game(r"C:\Tools\random_tool.exe").is_none());
    }

    #[test]
    fn game_key_short_requires_boundary() {
        assert!(game_key_matches("dnf", "dnf"));
        assert!(game_key_matches("dnf_win64", "dnf"));
        assert!(!game_key_matches("cloudflared", "cf"));
        assert!(!game_key_matches("office", "cf"));
        assert!(game_key_matches("crossfire", "crossfire"));
        assert!(game_key_matches("eldenring64", "eldenring"));
    }

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

    #[test]
    fn dedupe_normalizes_separator_and_case() {
        // Windows 同路径可能以 `C:\X`、`c:/X`、`C:\x` 等不同写法出现，
        // 归一化（小写 + `/`→`\` + 去尾斜杠）后应合并为一条。
        let games = vec![
            FoundGame {
                title: "G1".into(),
                exe_path:
                    r"C:\Program Files\miHoYo Launcher\games\Genshin Impact Game\YuanShen.exe"
                        .into(),
                app_name: String::new(),
                source: "mihoyo",
                appid: None,
            },
            FoundGame {
                title: "G2".into(),
                exe_path:
                    r"c:/program files/miHoYo Launcher/games/Genshin Impact Game/YuanShen.exe"
                        .into(),
                app_name: String::new(),
                source: "steam-shortcut",
                appid: None,
            },
        ];
        let out = dedupe(games);
        assert_eq!(out.len(), 1, "大小写与分隔符不同应合并为一条");
    }

    #[test]
    fn scan_exes_skips_non_game_exes() {
        let dir = std::env::temp_dir().join(format!("tt_exe_filter_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("createdump.exe"), b"MZ").unwrap();
        std::fs::write(dir.join("crashpad_handler.exe"), b"MZ").unwrap();
        std::fs::write(dir.join("Game.exe"), b"MZ").unwrap();
        let exes = scan_exes(&dir, 0);
        let names: Vec<String> = exes
            .iter()
            .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["Game".to_string()], "应只保留游戏主 exe");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── WeGame 增强 ──────────────────────────────────────────────

    fn make_wegame_tree() -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "tt_wegame_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&base);
        let game = base.join("games").join("MyRPG");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join("MyRPG.exe"), b"mz").unwrap();
        std::fs::write(game.join("helper.exe"), b"mz").unwrap();
        let junk = base.join("games").join("Download");
        std::fs::create_dir_all(&junk).unwrap();
        std::fs::write(junk.join("dl.exe"), b"mz").unwrap();
        let other = base.join("games").join("SomeTitle");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("zzz_launcher.exe"), b"mz").unwrap();
        std::fs::write(other.join("real_game.exe"), b"mz").unwrap();
        base
    }

    #[test]
    fn wegame_pick_exe_prefers_name_match() {
        let base = make_wegame_tree();
        let dir = base.join("games").join("MyRPG");
        let exe = pick_wegame_exe(&dir, "MyRPG").expect("应找到主 exe");
        assert!(
            exe.to_string_lossy().to_lowercase().contains("myrpg"),
            "got {:?}",
            exe
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn wegame_filters_non_game_dirs() {
        let base = make_wegame_tree();
        let found = scan_wegame_under(&base);
        let titles: Vec<_> = found.iter().map(|g| g.title.as_str()).collect();
        assert!(titles.contains(&"MyRPG"), "got {:?}", titles);
        assert!(
            !titles.iter().any(|t| t.eq_ignore_ascii_case("Download")),
            "got {:?}",
            titles
        );
        assert!(found.iter().all(|g| g.source == "wegame"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn wegame_roots_returns_vec_no_panic() {
        let roots = wegame_roots();
        assert!(
            roots
                .iter()
                .any(|p| p.to_string_lossy().to_lowercase().contains("wegame"))
        );
    }

    #[test]
    fn wegame_apps_layout_scanned() {
        let base = std::env::temp_dir().join(format!(
            "tt_wegame_apps_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&base);
        let apps = base.join("apps");
        let g = apps.join("LolGame");
        std::fs::create_dir_all(&g).unwrap();
        std::fs::write(g.join("LolGame.exe"), b"mz").unwrap();
        let found = scan_wegame_under(&apps);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source, "wegame");
        assert_eq!(found[0].title, "LolGame");
        let _ = std::fs::remove_dir_all(&base);
    }
}
