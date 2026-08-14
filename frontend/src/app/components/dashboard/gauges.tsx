'use client';

import type { ReactNode } from 'react';
import { Card } from '../ui/index';

export function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}

export function levelColor(pct: number): string {
  return pct >= 85 ? '#ef4444' : pct >= 60 ? '#fb8c00' : '#1e88e5';
}

export function tempColor(temp: number | null | undefined): string {
  if (temp == null) return 'var(--muted-foreground)';
  return temp > 85 ? '#ef4444' : temp > 75 ? '#f97316' : 'var(--primary)';
}

export function timeStr(d: Date): string {
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}:${String(d.getSeconds()).padStart(2, '0')}`;
}

/** THRM 同款半圆表盘：轨道 + 状态色进度弧。 */
export function SemiGauge({
  ratio,
  color,
  children,
}: {
  ratio: number;
  color: string;
  children: ReactNode;
}) {
  const r = 84;
  const cx = 100;
  const cy = 100;
  const arc = Math.PI * r;
  const safe = Math.max(0, Math.min(1, Number.isFinite(ratio) ? ratio : 0));
  const dashOffset = arc * (1 - safe);
  return (
    <div className="relative w-full max-w-60">
      <svg viewBox="0 0 200 116" className="block w-full" preserveAspectRatio="xMidYMid meet" aria-hidden="true">
        <path
          d={`M ${cx - r} ${cy} A ${r} ${r} 0 0 1 ${cx + r} ${cy}`}
          fill="none"
          stroke="var(--muted)"
          strokeWidth="10"
          strokeLinecap="round"
        />
        <path
          d={`M ${cx - r} ${cy} A ${r} ${r} 0 0 1 ${cx + r} ${cy}`}
          fill="none"
          stroke={color}
          strokeWidth="10"
          strokeLinecap="round"
          strokeDasharray={arc}
          strokeDashoffset={dashOffset}
          style={{ transition: 'stroke-dashoffset 600ms cubic-bezier(0.22, 1, 0.36, 1)' }}
        />
      </svg>
      <div className="pointer-events-none absolute inset-x-0 top-[68%] -translate-y-1/2 flex flex-col items-center justify-center">
        {children}
      </div>
    </div>
  );
}

/**
 * 双弧表盘：同一条半圆轨道上重叠两条弧。
 * 渲染顺序按数值排序——数值大的画在底层，小的画在顶层，
 * 所以无论温度高还是占用高，长的弧都在底下、短弧都在上面，无需手动切换。
 */
export function DualArcGauge({
  temp,
  usage,
  children,
}: {
  temp: number | null | undefined;
  usage: number | null | undefined;
  children: ReactNode;
}) {
  const r = 84;
  const cx = 100;
  const cy = 100;
  const arc = Math.PI * r;
  // 橙色=温度，蓝色=占用。大的先画（底层），小的后画（顶层）。
  const arcs = [
    { value: usage != null ? usage / 100 : 0, color: '#1E88E5' },
    { value: temp != null ? temp / 100 : 0, color: '#F97316' },
  ].sort((a, b) => b.value - a.value);
  return (
    <div className="relative w-full max-w-60">
      <svg viewBox="0 0 200 116" className="block w-full" preserveAspectRatio="xMidYMid meet" aria-hidden="true">
        <path
          d={`M ${cx - r} ${cy} A ${r} ${r} 0 0 1 ${cx + r} ${cy}`}
          fill="none"
          stroke="var(--muted)"
          strokeWidth="10"
          strokeLinecap="round"
        />
        {arcs.map((a) => {
          const safe = Math.max(0, Math.min(1, a.value));
          return (
            <path
              key={a.color}
              d={`M ${cx - r} ${cy} A ${r} ${r} 0 0 1 ${cx + r} ${cy}`}
              fill="none"
              stroke={a.color}
              strokeWidth="10"
              strokeLinecap="round"
              strokeDasharray={arc}
              strokeDashoffset={arc * (1 - safe)}
              style={{ transition: 'stroke-dashoffset 600ms cubic-bezier(0.22, 1, 0.36, 1)' }}
            />
          );
        })}
      </svg>
      <div className="pointer-events-none absolute inset-x-0 top-[68%] -translate-y-1/2 flex flex-col items-center justify-center">
        {children}
      </div>
    </div>
  );
}

/** THRM 风格统计卡：大数字 + 状态色 + 进度条。 */
export function StatCard({
  icon,
  label,
  value,
  unit,
  sub,
  color,
  percent,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  unit?: string;
  sub?: string;
  color: string;
  percent?: number;
}) {
  return (
    <Card className="gap-2">
      <div className="flex items-center justify-between">
        <span className="flex h-9 w-9 items-center justify-center rounded-xl text-white" style={{ backgroundColor: color }}>
          {icon}
        </span>
        <span className="min-w-0 flex-1 truncate px-2 text-right text-xs text-muted-foreground">{label}</span>
      </div>
      <div className="flex items-baseline gap-1">
        <span className="text-3xl font-bold tabular-nums leading-none" style={{ color }}>
          {value}
        </span>
        {unit && <span className="text-sm font-medium text-muted-foreground">{unit}</span>}
      </div>
      {sub && <div className="truncate text-xs text-muted-foreground">{sub}</div>}
      {percent !== undefined && (
        <span className="h-1 w-full overflow-hidden rounded-full bg-muted/60">
          <span
            className="block h-full rounded-full transition-all duration-500"
            style={{ width: `${Math.min(100, Math.max(0, percent))}%`, backgroundColor: color }}
          />
        </span>
      )}
    </Card>
  );
}
