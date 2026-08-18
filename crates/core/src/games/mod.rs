//! 游戏识别与统计。
//!
//! 分层策略（保证稳定性，任何一层失效都不影响整体）：
//! 1. **平台扫描**：Steam（libraryfolders.vdf + appmanifest_*.acf）、Epic（Manifests/*.item）、
//!    WeGame（games 目录）、米哈游（注册表尽力而为）——格式确定才解析，其余全容错跳过；
//! 2. **内置知名名单**（`KNOWN_GAMES`）：覆盖启动器格式不确定的平台（米哈游等），
//!    按 exe 文件名 stem 匹配，确定性兜底；
//! 3. **手动条目**：用户自行添加/移除。
//!
//! 匹配以 **exe 路径精确匹配**为主（每个会话都记录了前台进程的完整路径），
//! 文件名简写/目录前缀为辅；全程大小写不敏感，不依赖任何模糊启发式。

pub mod platform;
pub mod stats;

pub use platform::{FoundGame, scan_all_platforms};
pub use stats::{GameStat, game_stats_all, game_stats_in_range, game_stats_today};

use crate::contracts::GameRow;

/// 内置知名游戏名单：exe 文件名 stem（无扩展名，小写）→ 游戏名。
/// 覆盖启动器内部格式不确定或难以解析的平台（米哈游/WeGame 常见游戏等）。
pub const KNOWN_GAMES: &[(&str, &str)] = &[
    // 米哈游（启动器格式多变，按进程名兜底）
    ("genshinimpact", "原神"),
    ("starrail", "崩坏：星穹铁道"),
    ("zenlesszonezero", "绝区零"),
    ("bh3", "崩坏3"),
    // 腾讯 WeGame 常见游戏
    ("leagueclient", "英雄联盟"),
    ("crossfire", "穿越火线"),
    ("dnf", "地下城与勇士"),
    ("valorant", "无畏契约"),
    ("lostark", "命运方舟"),
    ("cf", "穿越火线"),
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
}
