'use client';

import type { LucideIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';

interface PlaceholderPageProps {
  icon: LucideIcon;
  titleKey: string;
  descriptionKey: string;
}

/**
 * P1 阶段占位页：展示各 tab 的开发状态。后续逐个替换为真实页面。
 */
export default function PlaceholderPage({
  icon: Icon,
  titleKey,
  descriptionKey,
}: PlaceholderPageProps) {
  const { t } = useTranslation();

  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center gap-4 rounded-2xl border border-dashed border-border bg-card/50 p-10 text-center">
      <span className="flex h-16 w-16 items-center justify-center rounded-2xl border border-primary/15 bg-primary/10 text-primary">
        <Icon className="h-8 w-8" />
      </span>
      <h2 className="text-xl font-semibold">{t(titleKey)}</h2>
      <p className="max-w-sm text-sm text-muted-foreground">{t(descriptionKey)}</p>
      <span className="mt-2 rounded-full border border-border bg-background px-3 py-1 text-xs font-medium text-muted-foreground">
        {t('placeholder.developing')}
      </span>
    </div>
  );
}
