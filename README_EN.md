<p align="center">
  <h1 align="center">Digitrace</h1>
</p>

<p align="center">
  A local-first Windows app/game time tracking & system monitoring tool
  <br>
  Tauri 2 + Next.js + Rust · All data stays on your machine, no telemetry
</p>

---

## Features

- **Dashboard** — editable nine-cell grid cards (add/remove/sort, multiple sizes), calendar heatmap, app stats, network stats, hardware gauges
- **App usage** — automatic foreground-app tracking with per-app today/week/month totals, idle/lock/sleep detection, hourly and window-title breakdown
- **Game tracking** — Steam, Epic, WeGame and MiHoYo detection, a dedicated games page and library, daily/history totals, and continuous-play reminders
- **Calendar** — month view with year jump, Chinese weekdays, daily details, full-year heatmap
- **Network monitor** — live download/upload curves, 24h / today / session / 7d / 30d history, per-app usage (Windows official API, no admin)
- **Hardware monitor** — CPU/GPU usage & temperature, GPU power, memory, disk temperature (optional kernel driver for CPU registers, off by default)
- **Health reminders** — continuous-usage and game-play reminders with native Windows notifications, state persists across restarts
- **Standalone monitor + Lite viewer** — `digitrace-monitor` is a headless collector/logger; `digitrace-lite-viewer` is a roughly 240 KB zero-dependency Win32 viewer that works without the full app
- **External integration** — the monitor exposes a read-only Named Pipe `\\.\pipe\DigitraceMetricsV1` with versioned JSON snapshots for local UIs such as DeskBox
- **System tray** — live data rows, configurable content, version shown
- **UI** — frameless glass design, themes / fonts / zh-en-ja i18n, free zoom (75%–175%), dark & light

## Privacy & Data

- All data is stored locally in `%APPDATA%\TimeTrace`; nothing is uploaded
- Sensitive fields (window titles, diary entries) are encrypted with Windows DPAPI
- The optional kernel driver is only installed with your explicit consent, for CPU temperature

## Tech Stack

| Module | Description |
| --- | --- |
| `crates/core` | Rust core: Win32 monitoring, hardware access, SQLite storage, encryption |
| `crates/metrics` | Language-independent shared-memory realtime metrics and `metrics.h` |
| `crates/monitor` | Headless monitor/logger with shared memory and read-only Named Pipe output |
| `crates/metrics-viewer` | Zero-dependency Win32 Lite viewer for realtime metrics |
| `src-tauri` | Tauri 2 shell: window, tray, OS integration |
| `frontend` | Next.js frontend: dashboard and all pages |

## Build

### Requirements

- Windows 10/11
- [Rust toolchain](https://rustup.rs/)
- [Node.js](https://nodejs.org/) (>= 22)
- Visual Studio 2022 Build Tools (C++ desktop workload)

### Commands

```bash
# 1) Rust tests (workspace green)
cargo test --workspace

# 2) Frontend type check + build (output to frontend/dist, embedded into the exe)
cd frontend
npm ci
npx tsc --noEmit
npm run build
cd ..

# 3) Windows Release build (embeds the frontend automatically)
cd src-tauri && cargo build --release
# Output: target/release/timetrace.exe
```

## License

[GNU General Public License v3.0](LICENSE)

Credits & third-party notices (TimeTrace MIT, THRM MIT, bundled fonts) are in [NOTICE.md](NOTICE.md).
