'use client';

import { useEffect, useState } from 'react';
import { ArrowDown, ArrowUp } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { apiService } from '../services/api';
import type { NetAppsSnapshotDto } from '../types';
import { Card } from './ui/index';
import AppIcon from './dashboard/AppIcon';

function formatBytes(bytes: number, perSecond = false): string {
  if (!bytes || bytes <= 0) return perSecond ? '0.0 B/s' : '0.0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}${perSecond ? '/s' : ''}`;
}

/**
 * 按应用网络流量卡片（网络页与仪表盘共用）。
 * 内部每 2 秒拉取一次每进程 TCP 实时速率与会话累计。
 */
export default function NetAppsCard({
  title,
  limit = 10,
  compact = false,
}: {
  title: string;
  limit?: number;
  compact?: boolean;
}) {
  const { t } = useTranslation();
  const [snap, setSnap] = useState<NetAppsSnapshotDto | null>(null);

  useEffect(() => {
    let disposed = false;
    const tick = async () => {
      try {
        const next = await apiService.getNetApps();
        if (!disposed) setSnap(next);
      } catch {
        /* 静默降级 */
      }
    };
    void tick();
    const timer = window.setInterval(() => void tick(), 2000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, []);

  const apps = snap?.apps ?? [];
  const visible = apps.slice(0, limit);
  const bytesMode = snap?.bytes_available ?? false;
  const etwMode = snap?.etw_mode ?? false;

  return (
    <Card padding="none" className="h-full overflow-hidden">
      <div className="border-b border-border/60 px-4 py-3 text-sm font-semibold">{title}</div>
      <div className="p-3">
        {!bytesMode && !compact && (
          <p className="mb-2 rounded-lg border border-amber-500/20 bg-amber-500/5 px-2.5 py-1.5 text-[11px] leading-relaxed text-muted-foreground">
            {t('network.bytesHint')}
          </p>
        )}
        {visible.length === 0 ? (
          <p className="py-6 text-center text-xs text-muted-foreground">
            {t('network.appTrafficEmpty')}
          </p>
        ) : (
          <div className={`space-y-0.5 ${apps.length > limit ? 'max-h-80 overflow-y-auto pr-1' : ''}`}>
            {visible.map((app) => (
              <div
                key={app.exe_path || app.app_name}
                className={`flex items-center rounded-lg transition-colors hover:bg-accent/40 ${compact ? 'gap-2 px-1.5 py-1' : 'gap-2.5 px-2 py-1.5'}`}
              >
                <AppIcon exePath={app.exe_path} size={compact ? 18 : 24} />
                <div className="min-w-0 flex-1">
                  {compact ? (
                    <>
                      <div className="truncate text-sm">{app.app_name}</div>
                      <div className="flex items-center justify-between gap-1 text-[11px] tabular-nums text-muted-foreground">
                        <span className="inline-flex min-w-0 items-center gap-1">
                          {etwMode ? (
                            <span className="truncate">
                              {t('network.appFlow')} {formatBytes(app.download_bps, true)}
                            </span>
                          ) : bytesMode ? (
                            <>
                              <ArrowDown className="h-2.5 w-2.5 shrink-0 text-[#1E88E5]" />
                              <span className="truncate">{formatBytes(app.download_bps, true)}</span>
                              <ArrowUp className="h-2.5 w-2.5 shrink-0 text-[#FB8C00]" />
                              <span className="truncate">{formatBytes(app.upload_bps, true)}</span>
                            </>
                          ) : (
                            <span>
                              {t('network.appActive')} {app.active_connections}
                            </span>
                          )}
                        </span>
                        <span className="shrink-0">
                          {t('network.appTotal')}{' '}
                          {etwMode
                            ? formatBytes(app.session_download)
                            : bytesMode
                            ? formatBytes(app.session_download + app.session_upload)
                            : app.total_connections}
                        </span>
                      </div>
                    </>
                  ) : (
                    <>
                      <div className="flex items-baseline justify-between gap-2">
                        <span className="min-w-0 truncate text-sm">{app.app_name}</span>
                        <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">
                          {etwMode ? (
                            <>
                              {t('network.appFlow')} {formatBytes(app.download_bps, true)}
                            </>
                          ) : bytesMode ? (
                            <>
                              {t('network.appActive')} {app.active_connections} · {t('network.appTotal')}{' '}
                              {formatBytes(app.session_download + app.session_upload)}
                            </>
                          ) : (
                            <>
                              {t('network.appActive')} {app.active_connections} · {t('network.appTotal')}{' '}
                              {app.total_connections}
                            </>
                          )}
                        </span>
                      </div>
                      {etwMode ? (
                        <div className="mt-0.5 text-[11px] tabular-nums text-muted-foreground">
                          {t('network.appTotal')} {formatBytes(app.session_download)}
                        </div>
                      ) : bytesMode ? (
                        <div className="mt-0.5 flex items-center gap-3 text-[11px] tabular-nums">
                          <span className="inline-flex items-center gap-1 text-[#1E88E5]">
                            <ArrowDown className="h-3 w-3" />
                            {formatBytes(app.download_bps, true)}
                          </span>
                          <span className="inline-flex items-center gap-1 text-[#FB8C00]">
                            <ArrowUp className="h-3 w-3" />
                            {formatBytes(app.upload_bps, true)}
                          </span>
                        </div>
                      ) : null}
                    </>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </Card>
  );
}
