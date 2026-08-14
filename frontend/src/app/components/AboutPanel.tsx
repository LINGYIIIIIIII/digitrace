'use client';

import { CheckCircle2, Download, Heart, RefreshCw, ShieldCheck, Sparkles, XCircle } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { getVersion } from '@tauri-apps/api/app';
import { apiService } from '../services/api';
import type { UpdateCheckDto, UpdateProgressDto } from '../types';
import { BRAND } from '../lib/brand';
import { Button } from './ui/index';

/**
 * 关于页：品牌、版本、致谢（TimeTrace / THRM）、隐私。
 */
export default function AboutPanel() {
  const { t } = useTranslation();
  const [version, setVersion] = useState('');
  const [checkState, setCheckState] = useState<'idle' | 'checking' | 'latest' | 'found' | 'error'>('idle');
  const [update, setUpdate] = useState<UpdateCheckDto | null>(null);
  const [progress, setProgress] = useState<UpdateProgressDto | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [actionMsg, setActionMsg] = useState<string | null>(null);

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  useEffect(() => {
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
    <div className="mx-auto max-w-2xl space-y-4">
      <div className="flex flex-col items-center gap-3 rounded-2xl border border-border bg-card p-8 text-center shadow-sm">
        <span className="flex h-20 w-20 items-center justify-center rounded-3xl border border-primary/15 bg-primary/10 text-primary">
          <Sparkles className="h-10 w-10" />
        </span>
        <div>
          <h2 className="text-2xl font-semibold">{BRAND.name}</h2>
          <p className="mt-1 text-sm text-muted-foreground">{BRAND.description}</p>
        </div>
        <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
          <span className="rounded-full border border-border bg-background px-2.5 py-1 font-medium">
            v{version || '—'}
          </span>
          <button
            type="button"
            onClick={() => {
              if (BRAND.repositoryUrl) void apiService.openExternalUrl(BRAND.repositoryUrl);
            }}
            title={BRAND.repositoryUrl}
            className="cursor-pointer rounded-full border border-border bg-background px-2.5 py-1 font-medium transition-colors hover:border-primary/40 hover:text-primary"
          >
            GPL-3.0
          </button>
        </div>
      </div>

      {/* 更新 */}
      <div className="rounded-2xl border border-border bg-card p-5 shadow-sm">
        <div className="mb-3 flex items-center gap-2 text-primary">
          <Download className="h-4 w-4" />
          <h3 className="text-sm font-semibold">{t('about.update.title')}</h3>
        </div>
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
        </div>
        {update && update.has_update && (
          <p className="mt-3 text-sm">
            {t('about.update.found', { version: update.latest_version })}
          </p>
        )}
        {update?.notes && checkState === 'found' && (
          <p className="mt-1 text-xs leading-relaxed text-muted-foreground">{update.notes}</p>
        )}
        {downloading && progress && (
          <div className="mt-3">
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
          <p className="mt-2 text-xs leading-relaxed text-muted-foreground">{actionMsg}</p>
        )}
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        <div className="rounded-2xl border border-border bg-card p-5 shadow-sm">
          <div className="mb-2 flex items-center gap-2 text-primary">
            <Sparkles className="h-4 w-4" />
            <h3 className="text-sm font-semibold">{t('about.ack.title')}</h3>
          </div>
          <div className="space-y-2 text-sm">
            <div>
              <div className="font-medium">TimeTrace</div>
              <p className="text-xs leading-relaxed text-muted-foreground">{t('about.ack.timetrace')}</p>
            </div>
            <div>
              <div className="font-medium">THRM</div>
              <p className="text-xs leading-relaxed text-muted-foreground">{t('about.ack.thrm')}</p>
            </div>
          </div>
        </div>

        <div className="rounded-2xl border border-border bg-card p-5 shadow-sm">
          <div className="mb-2 flex items-center gap-2 text-primary">
            <ShieldCheck className="h-4 w-4" />
            <h3 className="text-sm font-semibold">{t('about.privacy')}</h3>
          </div>
          <p className="text-sm leading-relaxed text-muted-foreground">{t('about.privacyDescription')}</p>
        </div>
      </div>

      <p className="flex items-center justify-center gap-1.5 pb-4 text-center text-xs text-muted-foreground">
        <Heart className="h-3.5 w-3.5 text-red-400" />
        {t('about.credits')}
      </p>
    </div>
  );
}
