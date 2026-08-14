# 数迹 Lite · 实时指标查看器（metrics-viewer）

零依赖 Win32/GDI 原生小窗口，只读数迹写入的共享内存实时指标，**无需 WebView**，
独立 exe 仅约 230KB。适合不想开主界面的场景：双击即看 CPU/GPU/内存/温度/网络速率/前台应用。

## 使用

1. 启动新版数迹（v2.25.2+，旧版不写共享内存）。
2. 双击 `digitrace-lite-viewer-<版本>.exe`（与主程序一同随 GitHub Release 发布）。
3. 窗口每秒刷新一次；关闭窗口即退出，不驻留托盘。

## 数据来源

读取 `%APPDATA%\TimeTrace\metrics.map`（4096 字节内存映射文件）：

| 偏移 | 字段 | 说明 |
|------|------|------|
| 0    | `MetricsHeader`（16B） | magic `0x43544D44` "DMTC"、version=1、seq |
| 16   | `MetricsSnapshot`（224B） | CPU/GPU/内存/温度/网络/帧率/前台应用 |

结构定义见 `crates/metrics/metrics.h`（C 头文件，任何语言可解析）。

## 构建

```bash
cargo build --release -p metrics-viewer
```

依赖仅 `metrics`（共享内存读写）+ `windows-sys`（Win32 原生 API）。
