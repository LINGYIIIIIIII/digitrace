/*
 * 数迹 Digitrace — 共享内存实时指标 C 头文件（语言无关）
 *
 * 文件：%APPDATA%\TimeTrace\metrics.map（固定 4096 字节）
 * 布局：[ MetricsHeader (16B) ][ MetricsSnapshot ]
 *
 * 读取方式（任意语言）：
 *   1. CreateFileW(path, GENERIC_READ, FILE_SHARE_READ|FILE_SHARE_WRITE, NULL,
 *                   OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL)
 *   2. CreateFileMappingW(file, NULL, PAGE_READONLY, 0, 4096, NULL)
 *   3. MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 4096)
 *   4. 校验 header->magic == 0x43544D44 && header->version == 1
 *   5. 读取 header 后的 MetricsSnapshot；seq 变化即新数据
 */

#ifndef DIGITRACE_METRICS_H
#define DIGITRACE_METRICS_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define METRICS_MAGIC   0x43544D44u  /* "DMTC" */
#define METRICS_VERSION 1u
#define METRICS_FILE_SIZE 4096u
#define METRICS_ACTIVE_APP_LEN 128u

typedef struct {
    uint32_t magic;
    uint32_t version;
    uint32_t snapshot_size;
    uint32_t reserved;
} MetricsHeader;

typedef struct {
    uint64_t seq;               /* 单调递增，检测更新 */
    int64_t  timestamp_ms;      /* Unix 毫秒 */
    double   cpu_total_percent; /* 0-100 */
    double   cpu_temp_c;        /* ℃，无传感器 -1 */
    double   gpu_usage_percent; /* 0-100，无 N 卡 -1 */
    double   gpu_temp_c;        /* ℃，无数据 -1 */
    double   mem_used_mb;       /* MB */
    double   mem_percent;       /* 0-100 */
    double   net_down_bps;      /* B/s */
    double   net_up_bps;        /* B/s */
    double   fps;               /* 预留：-1 表示未实现 */
    char     active_app[METRICS_ACTIVE_APP_LEN]; /* UTF-8，0 填充 */
} MetricsSnapshot;

#ifdef __cplusplus
}
#endif

#endif /* DIGITRACE_METRICS_H */
