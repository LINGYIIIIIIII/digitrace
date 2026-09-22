//! 游戏识别与统计。
//!
//! 分层策略（保证稳定性，任何一层失效都不影响整体）：
//! 1. **平台扫描**：Steam（libraryfolders.vdf + appmanifest_*.acf）、Epic（Manifests/*.item）、
//!    WeGame（games 目录）、米哈游（注册表尽力而为）——格式确定才解析，其余全容错跳过；
//! 2. **内置知名名单**（`KNOWN_GAMES`）：仅作为进程名识别别名，
//!    不会单独生成“已安装游戏”条目，避免把未安装的游戏显示到游戏库；
//! 3. **手动条目**：用户自行添加/移除。
//!
//! 匹配以 **exe 路径精确匹配**为主（每个会话都记录了前台进程的完整路径），
//! 文件名简写/目录前缀为辅；全程大小写不敏感，不依赖任何模糊启发式。

pub mod platform;
pub mod stats;

pub use platform::{FoundGame, scan_all_platforms};
pub use stats::{
    GameStat, GameStatsPeriods, game_stats_all, game_stats_in_range, game_stats_month,
    game_stats_periods, game_stats_today, game_stats_week, game_stats_year,
};

use crate::contracts::GameRow;

/// 内置知名游戏识别别名：exe 文件名 stem（无扩展名，小写）→ 游戏名。
/// 该名单不直接写入游戏库，只有从真实进程/平台记录中命中时才可使用。
/// 注意：`shortcut_to_game` 用 `stem.contains(key)`，短 key（cf/dnf）易误伤，
/// 这里改为足够长且不易撞车的 stem。
pub const KNOWN_GAMES: &[(&str, &str)] = &[
    // 米哈游（启动器格式多变，按进程名兜底）
    ("genshinimpact", "原神"),
    ("starrail", "崩坏：星穹铁道"),
    ("zenlesszonezero", "绝区零"),
    ("bh3", "崩坏3"),
    // 腾讯 WeGame 常见游戏（用完整/较长 stem，避免短 key contains 误匹配）
    ("leagueclient", "英雄联盟"),
    ("crossfire", "穿越火线"),
    ("dnf", "地下城与勇士"), // 保留：需配合全等或长 stem；见 shortcut_to_game
    ("valorant", "无畏契约"),
    ("lostark", "命运方舟"),
    // 其它热门游戏
    ("narakabladepoint", "永劫无间"),
    ("cs2", "CS2"),
    ("tslgame", "绝地求生"),
    ("r5apex", "Apex 英雄"),
    ("fortniteclient-win64-shipping", "堡垒之夜"),
    ("eldenring", "艾尔登法环"),
    ("rdr2", "荒野大镖客2"),
    ("cyberpunk2077", "赛博朋克 2077"),
    ("gta5", "GTA V"),
    ("minecraft", "我的世界"),
    ("minecraft.windows", "我的世界"),
    ("terraria", "泰拉瑞亚"),
    ("stardew valley", "星露谷物语"),
    ("hollow_knight", "空洞骑士"),
    ("ittakestwo", "双人成行"),
];

/// 从路径里取 exe 文件名 stem（不含扩展名、不含目录）。
pub fn exe_stem(path: &str) -> &str {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    name.strip_suffix(".exe").unwrap_or(name)
}

/// 判断 `path` 是否位于目录 `dir` 之下（大小写不敏感，要求完整目录边界）。
pub fn path_under(path: &str, dir: &str) -> bool {
    let p = path.to_lowercase();
    let d = dir.trim_end_matches(['\\', '/']).to_lowercase();
    p.len() > d.len()
        && p.starts_with(&d)
        && matches!(p.as_bytes().get(d.len()), Some(b'\\') | Some(b'/'))
}

/// 判断一条会话是否命中某游戏条目。
///
/// 规则：
/// - 条目 exe_path 不含目录（known 名单 / 手动简写）→ 按 exe 文件名 stem 匹配；
/// - 否则：exe 路径精确相等，或会话路径位于条目目录之下（目录级条目兜底）。
pub fn game_row_matches(row: &GameRow, app_path: &str, app_name: &str) -> bool {
    let has_dir = row.exe_path.contains('\\') || row.exe_path.contains('/');
    if !has_dir {
        let stem = exe_stem(app_path);
        return stem.eq_ignore_ascii_case(row.exe_path.trim_end_matches(".exe"));
    }
    app_path.eq_ignore_ascii_case(&row.exe_path)
        || path_under(app_path, &row.exe_path)
        || (!row.app_name.is_empty() && app_name.eq_ignore_ascii_case(&row.app_name))
}

/// 会话 → 游戏条目的查找索引。
///
/// 语义与 [`game_row_matches`] 完全一致，只是把 O(n) 线性扫描换成
/// 精确路径 / stem / app_name 的哈希命中；目录前缀兜底只扫带目录的条目
/// （通常很少）。用于全历史会话聚合等热路径。
pub struct GameMatchIndex<'a> {
    games: &'a [GameRow],
    /// 规范化 exe_path → 条目下标（精确路径命中）。
    path: std::collections::HashMap<String, usize>,
    /// 小写 stem → 无目录条目下标列表（known / 手动简写）。
    stem: std::collections::HashMap<String, Vec<usize>>,
    /// 小写 app_name → 条目下标列表。
    app_name: std::collections::HashMap<String, Vec<usize>>,
    /// 含目录的条目下标（仅这些需要 path_under 前缀扫描）。
    with_dir: Vec<usize>,
}

impl<'a> GameMatchIndex<'a> {
    pub fn build(games: &'a [GameRow]) -> Self {
        let mut path = std::collections::HashMap::new();
        let mut stem = std::collections::HashMap::new();
        let mut app_name = std::collections::HashMap::new();
        let mut with_dir = Vec::new();
        for (i, g) in games.iter().enumerate() {
            let has_dir = g.exe_path.contains('\\') || g.exe_path.contains('/');
            if has_dir {
                with_dir.push(i);
                path.insert(platform::normalize_path(&g.exe_path), i);
            } else {
                let key = g.exe_path.trim_end_matches(".exe").to_ascii_lowercase();
                stem.entry(key).or_insert_with(Vec::new).push(i);
            }
            if !g.app_name.is_empty() {
                app_name
                    .entry(g.app_name.to_ascii_lowercase())
                    .or_insert_with(Vec::new)
                    .push(i);
            }
        }
        Self {
            games,
            path,
            stem,
            app_name,
            with_dir,
        }
    }

    /// 与 [`game_row_matches`] 同语义；命中则返回条目引用。
    pub fn find(&self, app_path: &str, app_name: &str) -> Option<&'a GameRow> {
        let norm = platform::normalize_path(app_path);
        if let Some(&i) = self.path.get(&norm) {
            return Some(&self.games[i]);
        }
        let stem_key = exe_stem(app_path).to_ascii_lowercase();
        if let Some(list) = self.stem.get(&stem_key) {
            for &i in list {
                let row = &self.games[i];
                if game_row_matches(row, app_path, app_name) {
                    return Some(row);
                }
            }
        }
        if !app_name.is_empty() {
            let name_key = app_name.to_ascii_lowercase();
            if let Some(list) = self.app_name.get(&name_key) {
                for &i in list {
                    let row = &self.games[i];
                    if game_row_matches(row, app_path, app_name) {
                        return Some(row);
                    }
                }
            }
        }
        // 目录前缀兜底：仅扫描带目录的条目。
        for &i in &self.with_dir {
            let row = &self.games[i];
            if path_under(app_path, &row.exe_path) {
                return Some(row);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(exe_path: &str, app_name: &str) -> GameRow {
        GameRow {
            id: 1,
            title: "T".to_string(),
            exe_path: exe_path.to_string(),
            app_name: app_name.to_string(),
            source: "manual".to_string(),
            appid: None,
            watched: false,
        }
    }

    #[test]
    fn exact_path_match() {
        let r = row(
            r"D:\Steam\steamapps\common\ELDEN RING\Game\eldenring.exe",
            "",
        );
        assert!(game_row_matches(
            &r,
            r"D:\Steam\steamapps\common\ELDEN RING\Game\eldenring.exe",
            "eldenring"
        ));
        assert!(!game_row_matches(
            &r,
            r"D:\Steam\steamapps\common\OTHER\other.exe",
            "other"
        ));
    }

    #[test]
    fn case_insensitive_path_match() {
        let r = row(r"d:\steam\common\elden ring\game\eldenring.exe", "");
        assert!(game_row_matches(
            &r,
            r"D:\Steam\Common\ELDEN RING\Game\eldenring.exe",
            "x"
        ));
    }

    #[test]
    fn stem_match_without_dir() {
        let r = row("eldenring", "");
        assert!(game_row_matches(
            &r,
            r"D:\Games\ELDENRING\eldenring.exe",
            "eldenring"
        ));
        assert!(game_row_matches(
            &r,
            r"D:\Games\ELDENRING\Game\eldenring.exe",
            "x"
        ));
        assert!(!game_row_matches(
            &r,
            r"D:\Games\ELDENRING\launcher.exe",
            "x"
        ));
    }

    #[test]
    fn directory_prefix_match() {
        let r = row(r"D:\Games\Genshin", "");
        assert!(game_row_matches(
            &r,
            r"D:\Games\Genshin\GenshinImpact.exe",
            ""
        ));
        assert!(!game_row_matches(
            &r,
            r"D:\Games\GenshinImpact\other.exe",
            ""
        ));
    }

    #[test]
    fn name_fallback_match() {
        let r = row(r"C:\unknown\path\game.exe", "艾尔登法环");
        assert!(game_row_matches(
            &r,
            r"C:\unknown\path\game.exe",
            "艾尔登法环"
        ));
        // 名称相同时命中（按设计）；路径不同且名称不同则不命中
        assert!(game_row_matches(&r, r"C:\other\game.exe", "艾尔登法环"));
        assert!(!game_row_matches(&r, r"C:\other\game.exe", "other"));
    }

    #[test]
    fn exe_stem_extracts_name() {
        assert_eq!(exe_stem(r"D:\a\b\Game.exe"), "Game");
        assert_eq!(exe_stem("Game"), "Game");
        assert_eq!(exe_stem(r"D:\a\b\game"), "game");
    }

    #[test]
    fn path_under_boundary() {
        assert!(path_under(r"D:\Games\Game1\x.exe", r"D:\Games\Game1"));
        assert!(path_under(r"D:\Games\Game1\x.exe", r"D:\Games\Game1\"));
        assert!(!path_under(r"D:\Games\Game10\x.exe", r"D:\Games\Game1"));
        assert!(!path_under(r"D:\Games\x.exe", r"D:\Games\Game1"));
    }

    #[test]
    fn match_index_agrees_with_linear() {
        let games = vec![
            row(r"D:\Steam\common\ELDEN RING\Game\eldenring.exe", ""),
            row("starrail", "starrail"),
            row(r"D:\Games\Genshin", ""),
            row(r"C:\unknown\path\game.exe", "艾尔登法环"),
        ];
        let index = GameMatchIndex::build(&games);
        let cases: &[(&str, &str)] = &[
            (
                r"D:\Steam\common\ELDEN RING\Game\eldenring.exe",
                "eldenring",
            ),
            (r"d:\steam\common\elden ring\game\ELDENRING.EXE", "x"),
            (r"D:\Games\StarRail\StarRail.exe", "starrail"),
            (r"D:\Games\Genshin\GenshinImpact.exe", ""),
            (r"C:\unknown\path\game.exe", "艾尔登法环"),
            (r"C:\other\game.exe", "艾尔登法环"),
            (r"C:\nope\chrome.exe", "chrome"),
        ];
        for &(path, name) in cases {
            let linear = games.iter().find(|g| game_row_matches(g, path, name));
            let hashed = index.find(path, name);
            assert_eq!(
                linear.map(|g| g.exe_path.as_str()),
                hashed.map(|g| g.exe_path.as_str()),
                "mismatch for {path} / {name}"
            );
        }
    }
}
