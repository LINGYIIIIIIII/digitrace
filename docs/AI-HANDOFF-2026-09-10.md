# AI 交接：2026-09-10 仓库整理与性能优化

> 本文面向**后续接手的 AI / 开发者**，记录本机一次完整「检查 → 整理 → 固化 → 优化 → 门禁」过程。  
> 主仓：`C:\Users\chen\Documents\Codex\2026-08-18\digitrace-c-users-chen-documents-codex\work\timetrace-20260818`  
> 会话结束时：`main` 相对 `origin/main` **ahead 10**，工作树 clean，**未 push、未打 tag**。

---

## 0. 30 秒速览

| 项 | 值 |
|---|---|
| 产品 | 数迹 Digitrace（Tauri 2 + Next.js + Rust） |
| 版本文件 | `2.31.3`（Cargo.toml / tauri.conf.json / CHANGELOG 顶部一致） |
| 最新公开 tag | `v2.31.0` |
| 本机运行 | `outputs\digitrace-v2.31.3-blur12.exe`（托盘，计划任务 `DigitraceElevatedAutostart`） |
| 数据目录 | `%APPDATA%\TimeTrace`（勿改目录名，兼容历史） |
| 旧仓 | `2026-08-07\github-com-wellorbetter-timetrace\work\timetrace` → 根下有 `DEPRECATED.md`，只读 |
| 远程 | origin `LINGYIIIIIIII/digitrace`；upstream `wellorbetter/timetrace` **禁止推送** |

---

## 1. 本机环境结论（检查结果）

### 1.1 双仓

| 仓 | 状态 |
|---|---|
| **主仓 8/18** `timetrace-20260818` | 唯一有效；HEAD 曾停在 `0994529`（v2.31.0），工作区曾有大量未提交 2.31.x |
| 旧仓 8/07 `timetrace` | 停在 `00ff297` v2.26.1；其本地 `origin/main` ref 过期，「ahead 42」是假象 |

### 1.2 运行与数据（检查时）

- 进程：`digitrace-v2.31.3-blur12.exe` PID 103412，`--tray`，约 18MB
- 计划任务 `DigitraceElevatedAutostart`：Highest，路径写死 blur12
- `time.db` ~323MB：sessions ~9744，page_visits ~15308，game_entries 26，watched_games 14
- `monitor.db` ~20MB：metric_samples ~189077
- `config.json` 魔数 `dgc1`（整文件 AES）
- 无 HKCU Run、无标准卸载项、无桌面快捷方式

---

## 2. 本会话完成的工作（按时间序）

### 2.1 改动包 A — 仓库卫生（`01dc3af` 部分 + 旧仓文件）

**`.gitignore` 追加：**

```
/target-blur/
/target-verify/
/outputs/
frontend/.next 111/
```

**旧仓新增（不进 git 主线逻辑）：**

- `work/timetrace/DEPRECATED.md`
- 上级 `github-com-wellorbetter-timetrace/DEPRECATED.md`

内容：指向 8/18 主仓、禁止在旧仓开发/推送、说明 origin ref 过期。

---

### 2.2 改动包 B — 固化 2.31.3 工作线（3 个提交）

在固化前跑通门禁，并修了一处 clippy：

- `crates/core/src/games/platform.rs`：`collapsible_if` 合并嵌套 if

| 提交 | 说明 |
|---|---|
| `01dc3af` | chore: 收口 .gitignore |
| `90b116b` | chore(frontend): ESLint / Prettier / Playwright / editorconfig / e2e smoke |
| `1748bf3` | feat: 固化 v2.31.1–v2.31.3（游戏 shortcuts、硬件 live、窗口/图标/blur、身份字段加密迁移、版本 2.31.3、交接清单抬头） |

**`1748bf3` 覆盖的功能线（原未提交工作）：**

1. **游戏**：Steam `shortcuts.vdf` 扫描、`watched_games` 表、周期统计  
2. **加密**：`usage_sessions` / `page_visits` 的 `app_path`/`app_name` AES 迁移  
3. **硬件**：前端 `hardware-live-store.ts` 单例  
4. **窗口/托盘**：状态 sanitize、图标 BGRA、256px、blur/DWM  
5. **工程**：版本对齐 2.31.3；CHANGELOG 2.31.1–2.31.3  

---

### 2.3 性能与逻辑优化（P1–P6 及后续）

| 提交 | 对应规划 | 内容 |
|---|---|---|
| `6ea1301` | P1–P5 | Steam 库复用、`GameMatchIndex`、Tracker 缓存骨架、硬件 hidden 暂停、窗口 generation debounce |
| `26bc88f` | P3/P6 | GameTracker 锁序与缓存完善、迁移分批 |
| `7475872` | P2/P6 加强 | `get_playable_sessions_by_range` SQL 预过滤、迁移游标分页 |
| `9683ce6` | 文档 | 本 AI-HANDOFF、交接清单抬头、CHANGELOG 性能收口 |
| `69618d2` | 可观测性 | `encrypt_fallback_count()`：加密失败回退明文计数并导出 |

#### P1 — Steam 库复用（`platform.rs`）

- 新增 `steam_libraries(root)`：只读一次 `libraryfolders.vdf`
- `steam_installed(libs)` 接收库列表，不再内嵌解析
- 多库查找主 exe 时 **命中即 `break`**

#### P2 — 游戏匹配索引 + SQL 预过滤

- `games/mod.rs`：`GameMatchIndex`  
  - `path` 精确路径 HashMap  
  - `stem` / `app_name` 多值 HashMap  
  - `with_dir` 仅目录条目前缀扫描  
  - 语义与 `game_row_matches` 一致（有单测）
- `stats.rs`：`game_stats_periods` / `game_stats_in_range` 改用 index
- `contracts/storage.rs` + `sqlite.rs`：`get_playable_sessions_by_range`  
  - SQL 侧排除 idle、非正 duration，减少解密行数

#### P3 — GameTracker 缓存

- `GAMES_CACHE_TTL` / `CONFIG_CACHE_TTL` = 30s
- `SharedCache` + `GAMES_DIRTY` / `invalidate_games_cache()`
- 增删游戏、刷新库后立刻失效
- 统一 api 锁序，避免与提醒线程死锁风险

#### P4 — 硬件 live store

- `250ms setInterval` 闸 → **链式 `setTimeout` 对齐 refresh 间隔**
- `document.hidden` 时暂停轮询与计时；`visibilitychange` 恢复

#### P5 — 窗口状态

- `SAVE_GENERATION: AtomicU64` 真 debounce（期间再动则重睡）
- 写入：`*.json.tmp` → 删除目标 → `rename`；失败回退直接写

#### P6 — 敏感字段迁移

- `BATCH = 500`
- 分批 + `unchecked_transaction`
- 后续提交改为 **游标分页** SELECT，避免一次载入全部明文行

---

## 3. 门禁（会话结束时实测）

```text
cargo fmt --all -- --check               OK
cargo clippy --workspace -- -D warnings  OK
cargo test --workspace                   67 passed; 0 failed
npx tsc --noEmit                         OK
npm run test (vitest)                    14 passed
npm run lint (eslint)                    OK
git status                               clean
```

**新会话开工前请先重跑上述门禁**，不要假设仍然全绿。

---

## 4. 本地提交清单（均未 push）

```text
649be66 docs: 同步 AI-HANDOFF 与交接清单的最终提交清单
8502329 docs: AI-HANDOFF 补记加密回退计数与提交清单
69618d2 feat: 敏感字段加密回退可观测计数
9683ce6 docs: AI 交接文档与 2026-09-10 整理/优化记录
7475872 perf: 游戏统计 SQL 预过滤与加密迁移游标分页
26bc88f fix: 统一 GameTracker 锁序并分批迁移敏感字段
6ea1301 perf: 游戏匹配索引、Steam 库复用、提醒缓存与窗口/硬件调度
1748bf3 feat: 固化 v2.31.1–v2.31.3 未发布工作线
90b116b chore(frontend): 接入 ESLint/Prettier/Playwright 与 editorconfig
01dc3af chore: 收口 .gitignore，忽略 outputs 与多 target 缓存
```

推送到 GitHub 时：

```powershell
git push origin main
# 确认 CI / 真机后：
git tag v2.31.3
git push origin v2.31.3
```

**禁止** `git push upstream ...`。

---

## 5. 代码地图（优化相关）

| 关注点 | 文件 |
|---|---|
| 游戏匹配权威语义 | `crates/core/src/games/mod.rs` → `game_row_matches` |
| 匹配加速 | 同文件 → `GameMatchIndex` |
| 平台扫描 | `crates/core/src/games/platform.rs` → `scan_all_platforms` / `steam_libraries` |
| 周期统计 | `crates/core/src/games/stats.rs` → `game_stats_periods` |
| 可玩会话查询 | `crates/core/src/storage/sqlite.rs` → `get_playable_sessions_by_range` |
| 敏感迁移 | 同文件 → `encrypt_legacy_sensitive_fields`（BATCH/游标） |
| 加密回退计数 | 同文件 → `encrypt_fallback_count()`；`lib.rs` 再导出 |
| 提醒与缓存 | `src-tauri/src/games.rs` → `GameTracker` / `SharedCache` |
| 窗口状态 | `src-tauri/src/window_state.rs` |
| 硬件轮询 | `frontend/src/app/lib/hardware-live-store.ts` |
| 硬件卡接入 | `frontend/src/app/components/dashboard/cards-hardware.tsx` |

---

## 6. 明确未做（后续 AI 勿当已完成）

### 发版

- [ ] push main  
- [ ] tag `v2.31.3` / push tag  
- [ ] 核对 GitHub Release 六资产与 notes  

### 本机运行链路

- [ ] 自启任务仍指向 `blur12`，未改稳定路径  
- [ ] 无标准安装目录 / 卸载清单 / 快捷方式  
- [ ] `%APPDATA%\TimeTrace\updates\数迹-update.exe` 仍为旧包  
- [ ] `target` / `target-blur` / `target-verify` ~30GB 未清  

### 产品 / 隐私

- [ ] 「敏感字段全加密」与库表实际范围的产品口径  
- [ ] ~~`enc_str` 失败静默回退明文，无 UI/指标~~ → 已有 `encrypt_fallback_count()`；**UI 展示仍待做**  
- [ ] `watched_games` 仍按 title 明文，改名会丢关注  
- [ ] 部分历史版本 Release notes 缺口  

### 工程

- [ ] 真机 UI 验收矩阵（深浅主题 × 缩放 × 窄窗 × resize × 托盘）  
- [ ] e2e 仅 1 个 smoke  
- [ ] 大文件未再拆（`sqlite.rs` ~1.7k、`SettingsPage.tsx` ~1.3k）  
- [ ] `metrics.map` 自 8/14 停更，外部共享内存路径未写清  

---

## 7. 操作纪律（给后续 AI）

1. **只在 8/18 主仓开发**；旧仓只读。  
2. **main worktree 写入前**：若宿主要求隔离 worktree，先问用户。  
3. 提交前必须过：fmt / clippy `-D warnings` / workspace test / 前端 tsc+vitest+lint。  
4. 版本纪律：`scripts/bump.ps1` + CHANGELOG 顶条目与 Cargo/tauri.conf 一致，再 tag。  
5. 构建顺序：先 `frontend` build（`generate_context!` 需要 `frontend/dist`）再 `cargo build --release`。  
6. 本地验证避开用户实例：临时改 `identifier` 或停 blur12；验完恢复 `com.digitrace.app`。  
7. 交付习惯：产物在仓外或 `outputs/`（已 ignore）；用户要求「打开新版本」= `Start-Process` 拉起 exe。  
8. 推送仅 origin；schanel/TLS 有问题时用 SSH deploy key（见历史 `work/git-ssh.cmd` 模式）。  

---

## 8. 建议的下一步（按用户目标选一条）

| 目标 | 动作 |
|---|---|
| 尽快发版 | push → 真机点检 → tag v2.31.3 → 核对 Release |
| 本机更稳 | 从当前 HEAD 出正式 exe → 改计划任务路径 → 清 blur/verify |
| 隐私闭环 | 写清加密范围文档；enc 失败可见；watched 稳定键 |

---

## 9. 关联文档

| 文档 | 位置 |
|---|---|
| 项目交接清单（长期） | 仓库根 `交接清单.md` |
| 更新日志 | 仓库根 `CHANGELOG.md` |
| 旧仓停用说明 | 旧仓 `DEPRECATED.md` |
| 会话内分析（可选参考） | MiMo 会话目录 `数迹-缺点分析与改动文档.md`、`数迹-2.31x逻辑分析与优化实施.md` |
