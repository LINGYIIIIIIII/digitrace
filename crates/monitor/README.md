# 数迹 · 独立监控进程（digitrace-monitor）

无界面常驻后台进程：每秒采集 **CPU / 内存 / 温度（CPU/GPU） / 网络速率**，
写入共享内存 `%APPDATA%\TimeTrace\metrics.map`，供 Lite 查看器、完整版或任何语言程序零拷贝读取。

**核心价值**：即使数迹完整版没有启动，只要有独立监控在跑，Lite 查看器也能看实时指标。

## 用法

```bash
digitrace-monitor              # 启动（无窗口常驻，单实例）
digitrace-monitor --stop       # 通知运行中的实例退出
```

- 单实例：重复启动（不带 `--stop`）静默忽略，不干扰已运行实例。
- 与完整版并发运行安全：`metrics` 模块内部用命名互斥串行化写入，
  `seq` 读现有值 +1 全局单调，读取方据此检测更新。

## 外部 UI 数据接口

监控进程同时提供只读 Windows Named Pipe：

```text
\\.\pipe\DigitraceMetricsV1
```

每次客户端连接后，服务端返回一行 JSON 快照并关闭连接。管道为出站模式，
客户端只能读取，不能向监控进程写入命令。`version` 当前为 `1`；没有传感器的
温度、GPU 或功耗字段返回 `null`。DeskBox 等 C# 程序可以使用
`NamedPipeClientStream(PipeDirection.In)` 每秒或每两秒轮询一次。

## 与精简版搭配

`digitrace-lite-viewer.exe` 启动时若发现共享内存数据陈旧（完整版未运行），
会自动拉起**同目录**的 `digitrace-monitor.exe`，无需手动启动。
把这两个 exe 放同一目录即组成「精简版 = 显示界面 + 独立监控」。

## 构建

```bash
cargo build --release -p digitrace-monitor
```

依赖：`timetrace-core`（采集）+ `metrics`（共享内存）+ `serde_json`（Pipe 数据）+
`windows-sys`（Win32 原生 API）。无 UI、无 WebView，当前 release 版约 1.5MiB。
