'use client';

import { Heart, ShieldCheck, Sparkles } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getVersion } from '@tauri-apps/api/app';
import { apiService } from '../services/api';
import { BRAND } from '../lib/brand';
import { UpdateCheckCard } from './UpdateCheckCard';

/**
 * 关于页：品牌、版本、更新检查、致谢（TimeTrace / THRM）、隐私。
 */
export default function AboutPanel() {
  const { t } = useTranslation();
  const [version, setVersion] = useState('');

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

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

      {/* 更新（设置页与关于页共用组件） */}
      <div className="rounded-2xl border border-border bg-card p-5 shadow-sm">
        <div className="mb-3 flex items-center gap-2 text-primary">
          <Sparkles className="h-4 w-4" />
          <h3 className="text-sm font-semibold">{t('about.update.title')}</h3>
        </div>
        <UpdateCheckCard />
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
