'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { ArrowDown, ArrowUp, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { apiService } from '../services/api';
import type { AttributedUsageResult } from '../types';
import AppIcon from './dashboard/AppIcon';
import { Card } from './ui/index';

function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return '0.0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}

type RangeKey = 1 | 7 | 30;

/**
 * 按应用流量卡片（Windows 官方数据使用量，免管理员、非实时累计字节）。
 * 数据源：ConnectionProfile.GetAttributedNetworkUsageAsync，与系统设置
 * 「数据使用量 → 查看各应用的使用情况」一致，通常滞后约 1 小时。
 */
export default function AttributedUsageCard({
  title,
  limit = 15,
  compact = false,
}: {
  title: string;
  limit?: number;
  compact?: boolean;
}) {
  const { t } = useTranslation();
  const [days, setDays] = useState<RangeKey>(1);
  const [result, setResult] = useState<AttributedUsageResult | null>(null);
  const [loading, setLoading] = useState(false);
  const requestIdRef = useRef(0);

  const load = useCallback(async (d: number) => {
    const id = ++requestIdRef.current;
    setLoading(true);
    try {
      const res = await apiService.getAttributedUsage(d);
      if (requestIdRef.current === id) setResult(res);
    } catch (e) {
      if (requestIdRef.current === id) {
        setResult({
          available: false,
          apps: [],
          message: e instanceof Error ? e.message : String(e),
          since_local: '',
          until_local: '',
        });
      }
    } finally {
      if (requestIdRef.current === id) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(days);
  }, [days, load]);

  const visible = result?.apps.slice(0, limit) ?? [];
  const total = result?.apps.reduce((sum, a) => sum + a.total_bytes, 0) ?? 0;

  return (
    <Card padding="none" className="flex h-full flex-col overflow-hidden">
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border/60 px-4 py-3">
        <span className="text-sm font-semibold">{title}</span>
        <div className="flex items-center gap-2">
          {!compact && (
            <div className="flex gap-1">
              {([1, 7, 30] as RangeKey[]).map((d) => (
                <button
                  key={d}
                  type="button"
                  onClick={() => setDays(d)}
                  className={
                    'rounded-full border px-2.5 py-0.5 text-xs transition-colors ' +
                    (days === d
                      ? 'border-primary/30 bg-primary/10 text-primary'
                      : 'border-border text-muted-foreground hover:text-foreground')
                  }
                >
                  {d === 1 ? t('network.attrToday') : t('network.rangeDays', { days: d })}
                </button>
              ))}
            </div>
          )}
          <button
            type="button"
            onClick={() => void load(days)}
            disabled={loading}
            title={t('network.attrRefresh')}
            className="inline-flex h-6 w-6 items-center justify-center rounded-full border border-border text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
          >
            <RefreshCw className={`h-3 w-3 ${loading ? 'animate-spin' : ''}`} />
          </button>
        </div>
      </div>
      <div className="flex min-h-0 flex-1 flex-col p-3">
        {loading && result === null && (
          <p className="flex flex-1 items-center justify-center py-6 text-center text-xs text-muted-foreground">{t('network.attrLoading')}</p>
        )}
        {result && !result.available && (
          <p className="shrink-0 rounded-lg border border-amber-500/20 bg-amber-500/5 px-2.5 py-2 text-[11px] leading-relaxed text-muted-foreground">
            {result.message || t('network.attrUnavailable')}
          </p>
        )}
        {result && result.available && result.apps.length === 0 && (
          <p className="flex flex-1 items-center justify-center py-6 text-center text-xs text-muted-foreground">{t('network.attrEmpty')}</p>
        )}
        {result && result.available && result.apps.length > 0 && (
          <>
            <div className="flex min-h-0 flex-1 flex-col overflow-y-auto pr-1">
              {visible.map((app, index) => (
                <div
                  key={app.app_id || app.app_name || `row-${index}`}
                  className={`flex min-h-0 flex-1 items-center rounded-lg transition-colors hover:bg-accent/40 ${compact ? 'gap-2 px-1.5 py-0.5' : 'gap-2.5 px-2 py-1'}`}
                >
                  <AppIcon exePath={app.exe_path} size={compact ? 18 : 24} />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-baseline justify-between gap-2">
                      <span
                        className={`min-w-0 truncate ${compact ? 'text-xs' : 'text-sm'}`}
                        title={app.exe_path || app.app_id}
                      >
                        {app.app_name || app.app_id || t('network.attrUnknown')}
                      </span>
                      <span className={`shrink-0 tabular-nums text-muted-foreground ${compact ? 'text-[10px]' : 'text-[11px]'}`}>
                        {formatBytes(app.total_bytes)}
                      </span>
                    </div>
                    {!compact && app.exe_path && (
                      <p className="truncate text-[11px] text-muted-foreground/70" title={app.exe_path}>
                        {app.exe_path}
                      </p>
                    )}
                    {!compact && (
                      <div className="mt-0.5 flex items-center gap-3 text-[11px] tabular-nums">
                      <span className="inline-flex shrink-0 items-center gap-1 text-[#1E88E5]">
                        <ArrowDown className="h-3 w-3" />
                        {formatBytes(app.download_bytes)}
                      </span>
                      <span className="inline-flex shrink-0 items-center gap-1 text-[#FB8C00]">
                        <ArrowUp className="h-3 w-3" />
                        {formatBytes(app.upload_bytes)}
                      </span>
                      <div className="h-1 min-w-0 flex-1 overflow-hidden rounded-full bg-border/60">
                        <div
                          className="h-full rounded-full bg-primary/50"
                          style={{
                            width: total > 0 ? `${Math.max(2, (app.total_bytes / total) * 100)}%` : '0%',
                          }}
                        />
                      </div>
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
            {!compact && (
              <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
                {t('network.attrHint', { since: result.since_local, until: result.until_local })}
              </p>
            )}
          </>
        )}
      </div>
    </Card>
  );
}
