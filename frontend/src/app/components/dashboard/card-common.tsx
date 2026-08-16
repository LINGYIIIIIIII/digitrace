// 卡片公共：样式工具、格式化、网格档位、CardShell 外壳。
import type { ReactNode } from 'react';
import { Card } from '../ui/index';
import type { CardSize } from './dashboard-layout';
export function clsx(...parts: (string | false | undefined | null)[]): string {
  return parts.filter(Boolean).join(' ');
}

export function formatDuration(seconds: number): string {
  if (!seconds || seconds <= 0) return '0 秒';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) return `${h} 小时 ${m} 分 ${s} 秒`;
  if (m > 0) return `${m} 分 ${s} 秒`;
  return `${s} 秒`;
}

/** 紧凑时长（小卡片用）：1:02:03 / 4:05 / 0:00。 */
export function formatDurationCompact(seconds: number): string {
  if (!seconds || seconds <= 0) return '0:00';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  return `${m}:${String(s).padStart(2, '0')}`;
}

export function formatBytesPerSecond(bytes: number): string {
  if (!bytes || bytes <= 0) return '0.0 B/s';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}/s`;
}

/** 速率坐标轴取整：向上取到 1/2/5×10ⁿ 档位（避免 3.7MB/s 这种碎刻度）。 */
export function niceCeil(v: number): number {
  if (v <= 0) return 0;
  const exp = Math.floor(Math.log10(v));
  const base = 10 ** exp;
  const m = v / base;
  const nice = m <= 1 ? 1 : m <= 2 ? 2 : m <= 5 ? 5 : 10;
  return nice * base;
}

/** 截断长应用名（图表 X 轴标签用；悬停 Tooltip 与下方列表仍显示全名）。 */
export function truncateLabel(name: string, max: number): string {
  return name.length > max ? `${name.slice(0, max)}…` : name;
}

export const CHART_H: Record<CardSize, string> = {
  '1x1': 'h-20',
  '1x2': 'h-28',
  '2x1': 'h-24',
  '2x2': 'h-32',
  '3x1': 'h-36',
  '3x2': 'h-44',
  '3x3': 'h-56',
};
export const LIST_LIMIT: Record<CardSize, number> = {
  '1x1': 4,
  '1x2': 6,
  '2x1': 6,
  '2x2': 8,
  '3x1': 10,
  '3x2': 12,
  '3x3': 16,
};

/** 窄格（宽 1 列）：内容用紧凑密度。 */
export function isNarrow(size: CardSize): boolean {
  return size === '1x1' || size === '1x2';
}
/** 宽格（宽 3 列）：内容用舒展密度。 */
export function isWide(size: CardSize): boolean {
  return size === '3x1' || size === '3x2' || size === '3x3';
}
export const WEEK_LABELS = ['1', '2', '3', '4', '5', '6', '日'];

export function CardShell({
  title,
  children,
  bodyClassName,
}: {
  title: string;
  children: ReactNode;
  bodyClassName?: string;
}) {
  return (
    <Card padding="none" className="flex h-full flex-col overflow-hidden">
      <div className="border-b border-border/60 px-4 py-3 text-[var(--ui-title-size)] font-semibold">{title}</div>
      <div className={clsx('flex min-h-0 flex-1 flex-col p-[var(--ui-card-pad)]', bodyClassName)}>{children}</div>
    </Card>
  );
}



