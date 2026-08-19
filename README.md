<p align="center">
  <h1 align="center">数迹 Digitrace</h1>
</p>

<p align="center">
  本地优先的 Windows 应用/游戏时长统计与系统监控工具
  <br>
  Tauri 2 + Next.js + Rust · 数据全部本地存储，无遥测
</p>

---

## 功能特性

- **仪表盘** — 可编辑的九宫格卡片（增删/排序/多种尺寸），日历热力、应用统计、流量统计、硬件表盘一屏聚合
- **应用使用统计** — 自动记录前台应用活跃时长，支持单个应用今日/本周/本月统计，识别空闲/锁屏/睡眠，按小时/窗口标题细分
- **游戏统计** — 识别 Steam、Epic、WeGame、米哈游等游戏，独立游戏导航、游戏库、今日/历史时长和连续游戏提醒
- **日历** — 月视图 + 年份跳转、中文星期、当日使用详情、全年热力视图
- **网络监控** — 实时下载/上传速率曲线、24h/今日/本次启动/近 7 天/近 30 天历史、按应用流量（Windows 官方接口，免管理员）
- **硬件监控** — CPU/GPU 占用与温度、GPU 功耗、内存、磁盘温度（可选内核驱动读取 CPU 寄存器，默认不安装）
- **健康提醒** — 连续使用检测，游戏连续使用提醒，Windows 原生通知，状态跨重启保留
- **独立监控 + Lite 查看器** — `digitrace-monitor` 无界面常驻采集写共享内存；`metrics-viewer`
  轻量查看器（约 240KB，零依赖 Win32）双击即看实时指标，完整版不运行时也可独立使用
- **外部集成** — 独立监控进程提供只读 Named Pipe `\\.\pipe\DigitraceMetricsV1`，以版本化 JSON 暴露部分实时指标，便于 DeskBox 等本机 UI 集成
- **系统托盘** — 实时数据行、显示内容可配置、版本号
- **界面** — 无边框玻璃材质、主题/字体/中英日三语、整体无极缩放（75%–175%）、深色浅色

## 隐私与数据

- 所有数据保存在本机 `%APPDATA%\TimeTrace`，不上传任何内容
- 敏感字段（窗口标题、日记内容）使用 Windows DPAPI 加密存储
- 可选内核驱动仅在你明确同意后安装，用于读取 CPU 温度

## 技术栈

| 模块 | 说明 |
| --- | --- |
| `crates/core` | Rust 核心：Win32 监控、硬件读取、SQLite 存储、安全加密 |
| `crates/metrics` | 共享内存实时指标：语言无关发布/读取（C 头文件 `metrics.h`） |
| `crates/monitor` | 独立监控进程 `digitrace-monitor`：无界面常驻采集、记录分钟历史、写共享内存并提供只读 Named Pipe |
| `crates/metrics-viewer` | Lite 查看器 `digitrace-lite-viewer`：零依赖 Win32，读共享内存显示 |
| `src-tauri` | Tauri 2 应用外壳：窗口、托盘、系统集成 |
| `frontend` | Next.js 前端：仪表盘与全部页面 UI |

## 构建

### 环境要求

- Windows 10/11
- [Rust 工具链](https://rustup.rs/)
- [Node.js](https://nodejs.org/)（≥ 22）
- Visual Studio 2022 生成工具（C++ 桌面开发工作负载）

### 命令

```bash
# 1) Rust 测试（workspace 全绿）
cargo test --workspace

# 2) 前端类型检查 + 构建（产物输出到 frontend/dist，会被嵌入 exe）
cd frontend
npm ci
npx tsc --noEmit
npm run build
cd ..

# 3) Windows Release 构建（自动嵌入前端产物）
cd src-tauri && cargo build --release
# 产物：target/release/timetrace.exe
```

## 自动更新机制

- 更新源二选一（优先 GitHub 仓库）：设置页配置公开仓库（如 `LINGYIIIIIIII/digitrace`，
  直接读取最新 Release，tag 作版本、`.exe` 附件作安装包、`.sha256` 附件作强校验），
  或更新清单 JSON（`{version, url, sha256, notes}`，必须 HTTPS）
- 检查时机：默认启动后约 6 秒检查一次、之后每 6 小时轮询；也可设置每天固定检查小时，按自然日去重
- 校验：下载后强制 SHA-256 校验，缺失或失败一律拒绝安装
- 静默模式（设置 → 更新 → 静默更新）：后台自动下载，退出应用时静默替换并以托盘模式重启，
  全程无弹窗、无通知；关闭时保持手动确认流程

## 许可证

[GNU General Public License v3.0](LICENSE)

致谢与第三方声明（TimeTrace MIT、THRM MIT、内置字体）见 [NOTICE.md](NOTICE.md)。
