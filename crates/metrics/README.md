# 数迹共享内存实时指标（metrics）

语言无关、零拷贝的实时指标发布/读取模块。

## 文件

- 映射文件：`%APPDATA%\TimeTrace\metrics.map`（固定 4096 字节）
- 布局：`[ MetricsHeader (16B) ][ MetricsSnapshot ]`
- C 结构体定义：`metrics.h`（任何语言可复制）

## 实时指标（1Hz）

| 字段 | 含义 |
|---|---|
| `cpu_total_percent` | CPU 总占用 0-100 |
| `cpu_temp_c` | CPU 温度 ℃（无传感器 -1） |
| `gpu_usage_percent` / `gpu_temp_c` | GPU 占用/温度（无 N 卡 -1） |
| `mem_used_mb` / `mem_percent` | 内存已用 MB / 百分比 |
| `net_down_bps` / `net_up_bps` | 网络上下行速率 B/s |
| `fps` | 帧率（**预留，-1 表示未实现**） |
| `active_app` | 当前前台应用名（UTF-8） |

`seq` 单调递增用于检测更新，`timestamp_ms` 为采样时刻。

## 读取（任意语言）

以 C 为例（Python/C#/Rust 同理，映射同一文件 + 读结构体）：

```c
HANDLE f = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ|FILE_SHARE_WRITE,
                       NULL, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
HANDLE m = CreateFileMappingW(f, NULL, PAGE_READONLY, 0, 4096, NULL);
MetricsHeader* hdr = (MetricsHeader*)MapViewOfFile(m, FILE_MAP_READ, 0, 0, 4096);
if (hdr->magic == METRICS_MAGIC && hdr->version == METRICS_VERSION) {
    MetricsSnapshot* snap = (MetricsSnapshot*)((char*)hdr + sizeof(MetricsHeader));
    // snap->cpu_total_percent … 实时数据，零拷贝
}
```

## Rust 侧

- 发布方：持有 `metrics::CollectorLease::acquire()` 的进程使用
  `metrics::MetricsPublisher::open()` + `publish(snapshot)` 每秒发布；同一时刻只允许一个采集者。
- 读取方：`metrics::MetricsReader::open()` + `read()`（外部 Rust 工具）

采集租约使用 `Local\\DigitraceCollectorLeaseV1` 命名互斥体。完整版与
`digitrace-monitor.exe` 竞争租约，未获租约的一方自动降级为只读跟随者；持有者异常退出时
Windows 会自动释放互斥体，不会留下需要手动清理的锁文件。

## 历史数据

实时值走共享内存；历史数据读 `%APPDATA%\TimeTrace\` 下的 SQLite：

- `time.db`：使用统计（`usage_sessions` 会话、`page_visits` 窗口标题）
- `monitor.db`：**分钟级聚合历史**（表 `metric_samples`）

`monitor.db` 的 `metric_samples` 结构：`(day, minute, metric, avg, max, samples)`，
主键 `(day, minute, metric)`，`day` 为 `YYYY-MM-DD`，`minute` 为当天的分钟序号（0-1439）。

内置 metric 名（外部工具可按名查询历史）：

| metric | 含义 |
|---|---|
| `net_down_bps` / `net_up_bps` | 网络上下行速率（B/s） |
| `cpu_percent` | CPU 总占用（0-100） |
| `mem_percent` / `mem_used_mb` | 内存百分比 / 已用 MB |
| `cpu_temp_c` | CPU 温度 ℃（无传感器时不写） |
| `gpu_usage_percent` / `gpu_temp_c` | GPU 占用 / 温度（无 N 卡时不写） |

示例（SQL）：

```sql
-- 最近 24 小时 CPU 占用
SELECT day, minute, avg, max FROM metric_samples
WHERE metric = 'cpu_percent' AND day >= date('now', 'localtime', '-1 day')
ORDER BY day, minute;
```
