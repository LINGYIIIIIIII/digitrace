# 数迹 Lite · 实时指标查看器（metrics-viewer）

零依赖 Win32/GDI 原生小窗口，只读取共享内存中的实时指标，**无需 WebView**，
独立 exe 仅约 240KB。适合不想开主界面的场景：双击即看 CPU/GPU/内存/温度/网络速率/前台应用。

## 使用

共享内存由 v2.26.0 的数迹完整版或独立监控 `digitrace-monitor` 写入，两种方式任选：

1. **完整版运行中**：直接双击 `digitrace-lite-viewer-<版本>.exe` 即可查看。
2. **精简版（不运行完整版）**：把 `digitrace-lite-viewer.exe` 与 `digitrace-monitor.exe`
   放在同一目录后双击查看器——数据陈旧时它会自动拉起独立监控，无需手动启动。

窗口每秒刷新一次；关闭窗口即退出，不驻留托盘。若 5 秒内无新数据，底部会提示「数据已过期」。

## 数据来源

读取 `%APPDATA%\TimeTrace\metrics.map`（4096 字节内存映射文件）：

| 偏移 | 字段 | 说明 |
|------|------|------|
| 0    | `MetricsHeader`（16B） | magic `0x43544D44` "DMTC"、version=1、snapshot_size |
| 16   | `MetricsSnapshot`（216B） | seq、时间戳、CPU/GPU/内存/温度/网络/帧率、前台应用 |

结构定义见 `crates/metrics/metrics.h`（C 头文件，任何语言可解析）。

## 构建

```bash
cargo build --release -p metrics-viewer
```

依赖仅 `metrics`（共享内存读写）+ `windows-sys`（Win32 原生 API）。
