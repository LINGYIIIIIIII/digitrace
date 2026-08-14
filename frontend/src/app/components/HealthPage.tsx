'use client';

import { useCallback, useEffect, useState } from 'react';
import { BellRing, Check, Clock3, Coffee, Hourglass, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { apiService } from '../services/api';
import { useAppStore } from '../store/app-store';
import type { HealthSnapshotDto } from '../types';
import { Button, Card, ToggleSwitch } from './ui/index';

const REMINDER_OPTIONS = [30, 45, 60, 90, 120] as const;
const BREAK_OPTIONS = [3, 5, 10] as const;

function formatDuration(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (h > 0) return `${h} 小时 ${m} 分`;
  if (m > 0) return `${m} 分 ${String(sec).padStart(2, '0')} 秒`;
  return `${sec} 秒`;
}

export default function HealthPage() {
  const { t } = useTranslation();
  const { config, updateConfig } = useAppStore(
    useShallow((s) => ({ config: s.config, updateConfig: s.updateConfig })),
  );
  const [snap, setSnap] = useState<HealthSnapshotDto | null>(null);
  const [testing, setTesting] = useState(false);
  const [testState, setTestState] = useState<'idle' | 'ok' | 'failed'>('idle');

  useEffect(() => {
    let disposed = false;
    const tick = async () => {
      try {
        const next = await apiService.getHealthSnapshot();
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

  const patchConfig = useCallback(
    async (patch: Partial<NonNullable<typeof config>>) => {
      if (!config) return;
      await updateConfig({ ...config, ...patch });
    },
    [config, updateConfig],
  );

  const testNotification = useCallback(async () => {
    setTesting(true);
    setTestState('idle');
    try {
      await apiService.testHealthNotification();
      setTestState('ok');
    } catch {
      setTestState('failed');
    } finally {
      setTesting(false);
    }
  }, []);

  // 控件状态绑定即时生效的配置（点击立即反馈），快照只用于实时数字展示，
  // 避免按钮要等下一次 2 秒轮询才变化。
  const enabled = config?.health_reminder_enabled ?? snap?.enabled ?? true;
  const reminderMinutes = config?.health_reminder_minutes ?? snap?.reminder_minutes ?? 60;
  const breakMinutes = config?.health_break_minutes ?? snap?.break_minutes ?? 5;
  const streakSeconds = snap?.streak_seconds ?? 0;
  const idleSeconds = snap?.idle_seconds ?? 0;

  const statItem = (icon: React.ReactNode, label: string, value: string) => (
    <div className="flex items-center gap-4 rounded-xl border border-border/60 bg-card/60 px-5 py-5">
      <span className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary">
        {icon}
      </span>
      <div className="min-w-0">
        <div className="text-sm text-muted-foreground">{label}</div>
        <div className="truncate text-2xl font-semibold tabular-nums">{value}</div>
      </div>
    </div>
  );

  return (
    <div className="space-y-4">
      {/* 实时状态 */}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {statItem(<Hourglass className="h-6 w-6" />, t('health.streak'), formatDuration(streakSeconds))}
        {statItem(
          <Clock3 className="h-6 w-6" />,
          t('health.nextReminder'),
          enabled ? formatDuration(snap?.next_reminder_seconds ?? 0) : '—',
        )}
        {statItem(<BellRing className="h-6 w-6" />, t('health.remindersToday'), String(snap?.reminders_today ?? 0))}
        {statItem(
          <Coffee className="h-6 w-6" />,
          t('health.idle'),
          idleSeconds >= breakMinutes * 60 ? t('health.away') : t('health.using'),
        )}
      </div>

      {/* 设置 */}
      <Card padding="md">
        <div className="flex items-center justify-between gap-3">
          <div>
            <div className="text-sm font-semibold">{t('health.enable')}</div>
            <p className="mt-0.5 text-xs text-muted-foreground">{t('health.enableDescription')}</p>
          </div>
          <ToggleSwitch
            enabled={enabled}
            onChange={(v) => void patchConfig({ health_reminder_enabled: v })}
          />
        </div>

        <div className="mt-5 space-y-5">
          <div>
            <div className="mb-2 text-sm font-medium">{t('health.reminderInterval')}</div>
            <div className="flex flex-wrap gap-2">
              {REMINDER_OPTIONS.map((m) => (
                <button
                  key={m}
                  type="button"
                  disabled={!enabled}
                  onClick={() => void patchConfig({ health_reminder_minutes: m })}
                  className={
                    'rounded-full border px-3 py-1 text-xs transition-colors disabled:opacity-40 ' +
                    (reminderMinutes === m
                      ? 'border-primary/30 bg-primary/10 text-primary'
                      : 'border-border text-muted-foreground hover:text-foreground')
                  }
                >
                  {t('health.minutes', { minutes: m })}
                </button>
              ))}
            </div>
          </div>

          <div>
            <div className="mb-2 text-sm font-medium">{t('health.breakThreshold')}</div>
            <div className="flex flex-wrap gap-2">
              {BREAK_OPTIONS.map((m) => (
                <button
                  key={m}
                  type="button"
                  disabled={!enabled}
                  onClick={() => void patchConfig({ health_break_minutes: m })}
                  className={
                    'rounded-full border px-3 py-1 text-xs transition-colors disabled:opacity-40 ' +
                    (breakMinutes === m
                      ? 'border-primary/30 bg-primary/10 text-primary'
                      : 'border-border text-muted-foreground hover:text-foreground')
                  }
                >
                  {t('health.minutes', { minutes: m })}
                </button>
              ))}
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-3 border-t border-border/60 pt-4">
            <Button
              variant="outline"
              size="sm"
              onClick={() => void testNotification()}
              disabled={testing || !enabled}
            >
              {testing ? t('health.testing') : t('health.test')}
            </Button>
            {testState === 'ok' && (
              <span className="inline-flex items-center gap-1 text-xs text-emerald-600 dark:text-emerald-400">
                <Check className="h-3.5 w-3.5" />
                {t('health.testOk')}
              </span>
            )}
            {testState === 'failed' && (
              <span className="inline-flex items-center gap-1 text-xs text-destructive">
                <X className="h-3.5 w-3.5" />
                {t('health.testFailed')}
              </span>
            )}
            {snap?.last_reminder_local && (
              <span className="text-xs text-muted-foreground">
                {t('health.lastReminder', { time: snap.last_reminder_local })}
              </span>
            )}
          </div>
        </div>
      </Card>

      <p className="text-xs leading-relaxed text-muted-foreground">{t('health.hint')}</p>
    </div>
  );
}
