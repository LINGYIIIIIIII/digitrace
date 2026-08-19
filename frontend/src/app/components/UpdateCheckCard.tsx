'use client';

import { CheckCircle2, Download, XCircle } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { apiService, isTauriRuntime } from '../services/api';
import type { UpdateCheckDto, UpdateProgressDto } from '../types';
import { Button } from './ui/index';

/**
 * 手动「检查更新」卡片：检查 → 发现新版 → 下载（带进度）→ 安装。
 * 设置页「更新」与「关于」页共用；同时监听后台自动检查推送的
 * `update-available` / `update-progress` 事件。
 */
export function UpdateCheckCard() {
  const { t } = useTranslation();
  const [checkState, setCheckState] = useState<'idle' | 'checking' | 'latest' | 'found' | 'error'>('idle');
  const [update, setUpdate] = useState<UpdateCheckDto | null>(null);
  const [progress, setProgress] = useState<UpdateProgressDto | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [actionMsg, setActionMsg] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    const setup = async () => {
      unlisteners.push(await listen<UpdateCheckDto>('update-available', (event) => {
        if (disposed) return;
        setUpdate(event.payload);
        setCheckState('found');
      }));
      unlisteners.push(await listen<UpdateProgressDto>('update-progress', (event) => {
        if (disposed) return;
        setProgress(event.payload);
      }));
    };
    void setup();
    return () => {
      disposed = true;
      unlisteners.forEach((f) => f());
    };
  }, []);

  const doCheck = useCallback(async () => {
    setCheckState('checking');
    setActionMsg(null);
    try {
      const dto = await apiService.checkForUpdate();
      setUpdate(dto);
      if (dto.message) {
        setCheckState('error');
        setActionMsg(dto.message);
      } else if (dto.has_update) {
        setCheckState('found');
      } else {
        setCheckState('latest');
      }
    } catch (e) {
      setCheckState('error');
      setActionMsg(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const doUpdate = useCallback(async () => {
    setDownloading(true);
    setProgress(null);
    setActionMsg(null);
    try {
      const res = await apiService.downloadUpdate();
      if (!res.ok) {
        setActionMsg(res.message ?? t('about.update.failed'));
        setDownloading(false);
        return;
      }
      setActionMsg(t('about.update.installing'));
      await apiService.installUpdate();
    } catch (e) {
      setActionMsg(e instanceof Error ? e.message : String(e));
      setDownloading(false);
    }
  }, [t]);

  const percent = progress && progress.percent >= 0 ? Math.round(progress.percent) : null;

  return (
    <div className="flex flex-wrap items-center gap-3">
      <Button
        variant={checkState === 'found' ? 'primary' : 'outline'}
        size="sm"
        onClick={() => void doCheck()}
        disabled={checkState === 'checking' || downloading}
      >
        {checkState === 'checking' ? t('about.update.checking') : t('about.update.check')}
      </Button>
      {checkState === 'found' && update && (
        <Button size="sm" onClick={() => void doUpdate()} disabled={downloading}>
          {downloading ? t('about.update.downloading') : t('about.update.download')}
        </Button>
      )}
      {checkState === 'latest' && (
        <span className="inline-flex items-center gap-1 text-xs text-emerald-600 dark:text-emerald-400">
          <CheckCircle2 className="h-3.5 w-3.5" />
          {t('about.update.latest')}
        </span>
      )}
      {checkState === 'error' && (
        <span className="inline-flex items-center gap-1 text-xs text-destructive">
          <XCircle className="h-3.5 w-3.5" />
          {t('about.update.checkFailed')}
        </span>
      )}
      {update && update.has_update && (
        <span className="text-xs text-muted-foreground">
          {t('about.update.found', { version: update.latest_version })}
        </span>
      )}
      {update?.notes && checkState === 'found' && (
        <p className="w-full text-xs leading-relaxed text-muted-foreground">{update.notes}</p>
      )}
      {downloading && progress && (
        <div className="w-full">
          <div className="h-1.5 w-full overflow-hidden rounded-full bg-border/60">
            <div
              className="h-full rounded-full bg-primary transition-all"
              style={{ width: percent !== null ? `${percent}%` : '30%' }}
            />
          </div>
          <p className="mt-1 text-[11px] tabular-nums text-muted-foreground">
            {percent !== null
              ? `${percent}%`
              : `${Math.round((progress.downloaded_bytes || 0) / 1024 / 1024)} MB`}
          </p>
        </div>
      )}
      {actionMsg && (
        <p className="w-full text-xs leading-relaxed text-muted-foreground">{actionMsg}</p>
      )}
    </div>
  );
}
