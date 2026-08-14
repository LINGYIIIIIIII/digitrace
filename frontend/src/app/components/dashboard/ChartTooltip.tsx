'use client';

import type { ReactNode } from 'react';

/**
 * 统一的玻璃质感图表 Tooltip（对应 THRM 样式：半透明 + 高斯模糊 + 圆角 + 阴影）。
 * 用法：<Tooltip content={<ChartTooltip labelFormatter={...} valueFormatter={...} />} />
 */
export default function ChartTooltip({
  active,
  label,
  payload,
  labelFormatter,
  valueFormatter,
}: {
  active?: boolean;
  label?: string | number;
  payload?: Array<{
    name?: string;
    value?: number | string;
    color?: string;
    stroke?: string;
    fill?: string;
  }>;
  labelFormatter?: (label: string | number) => string;
  valueFormatter?: (value: number | string, name: string) => ReactNode;
}) {
  if (!active || !payload || payload.length === 0) {
    return null;
  }

  return (
    <div
      style={{
        backgroundColor: 'var(--chart-tooltip-bg)',
        border: '1px solid var(--chart-tooltip-border)',
        borderRadius: 10,
        boxShadow: 'var(--chart-tooltip-shadow)',
        padding: '8px 12px',
        color: 'var(--chart-tooltip-text)',
        backdropFilter: 'blur(16px) saturate(180%)',
        WebkitBackdropFilter: 'blur(16px) saturate(180%)',
        fontFamily: 'var(--font-sans)',
        minWidth: 120,
      }}
    >
      {label !== undefined && label !== '' && (
        <div style={{ fontWeight: 600, marginBottom: 4, fontSize: 12 }}>
          {labelFormatter ? labelFormatter(label) : String(label)}
        </div>
      )}
      <div className="space-y-0.5">
        {payload.map((row, i) => (
          <div
            key={i}
            style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12, lineHeight: '18px' }}
          >
            <span
              style={{
                width: 8,
                height: 8,
                borderRadius: 9999,
                backgroundColor: row.color || row.stroke || row.fill || 'var(--chart-primary)',
                display: 'inline-block',
                flexShrink: 0,
              }}
            />
            <span style={{ color: 'var(--chart-tooltip-text)' }}>{row.name}</span>
            <span
              style={{
                marginLeft: 'auto',
                fontWeight: 500,
                fontVariantNumeric: 'tabular-nums',
                paddingLeft: 12,
              }}
            >
              {valueFormatter ? valueFormatter(row.value ?? 0, row.name ?? '') : row.value}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

/** 秒数 → 轴标签（如 1.5h / 30分 / 45s）。 */
export function formatAxisSeconds(seconds: number): string {
  if (!seconds || seconds <= 0) return '0';
  if (seconds >= 3600) {
    const h = seconds / 3600;
    return `${h >= 10 ? Math.round(h) : h.toFixed(1)}h`;
  }
  if (seconds >= 60) {
    return `${Math.round(seconds / 60)}分`;
  }
  return `${Math.round(seconds)}s`;
}
