// 日历共享工具：格式化与热力配色（月/年视图与日面板共用）。

export function formatDuration(seconds: number): string {  if (!seconds || seconds <= 0) return '0 秒';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) return `${h} 小时 ${m} 分 ${s} 秒`;
  if (m > 0) return `${m} 分 ${s} 秒`;
  return `${s} 秒`;
}


export function formatBytes(bytes: number, perSecond = false): string {  if (!bytes || bytes <= 0) return perSecond ? '0.0 B/s' : '0.0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}${perSecond ? '/s' : ''}`;
}


export function heatColor(seconds: number, max: number): string {  if (seconds <= 0 || max <= 0) return 'bg-card hover:bg-accent';
  const ratio = seconds / max;
  if (ratio < 0.25) return 'bg-primary/15 hover:bg-primary/25';
  if (ratio < 0.5) return 'bg-primary/30 hover:bg-primary/40';
  if (ratio < 0.75) return 'bg-primary/55 hover:bg-primary/65';
  return 'bg-primary/85 hover:bg-primary';
}

/** 分钟序号 → "HH:MM"。 */

export function fmtMin(minute: number): string {  const hh = Math.floor(minute / 60);
  const mm = minute % 60;
  return `${String(hh).padStart(2, '0')}:${String(mm).padStart(2, '0')}`;
}


