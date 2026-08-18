'use client';

// 游戏时长卡片：当前游戏 / 今日游戏时长 / 连续时长 / 下次提醒。
// 数据来自 get_game_snapshot（轻量，轮询刷新间隔），不依赖仪表盘主数据。

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { BellRing, Gamepad2, Hourglass } from 'lucide-react';
import { apiService } from '../../services/api';
import { useAppStore } from '../../store/app-store';
import type { GameSnapshotDto } from '../../types';
import { CardShell, formatDuration } from './card-common';
import type { CardSize } from './dashboard-layout';

function useGameSnapshot(): GameSnapshotDto | null {
  const { config } = useAppStore(useShallow((s) => ({ config: s.config })));
  const refreshSeconds = config?.refresh_interval_seconds ?? 10;
  const [snap, setSnap] = useState<GameSnapshotDto | null>(null);

  useEffect(() => {
    let disposed = false;
    const tick = async () => {
      try {
        const next = await apiService.getGameSnapshot();
        if (!disposed) setSnap(next);
      } catch {
        /* 静默降级 */
      }
    };
    void tick();
    const timer = window.setInterval(() => void tick(), refreshSeconds * 1000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [refreshSeconds]);

  return snap;
}

export function GamesCard({ size }: { size: CardSize }) {
  const { t } = useTranslation();
  const snap = useGameSnapshot();
  const compact = size === '1x1' || size === '1x2';

  return (
    <CardShell title={t('dashboard.cards.games')}>
      <div className="flex h-full flex-col justify-center gap-1.5">
        {/* 当前游戏 */}
        <div className="flex items-center gap-2 rounded-lg border border-border/60 px-2 py-1.5">
          <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-primary/15 bg-primary/10 text-primary">
            <Gamepad2 className="h-4 w-4" />
          </span>
          <div className="min-w-0">
            <div className="truncate text-[11px] leading-tight text-muted-foreground">
              {t('dashboard.games.nowPlaying')}
            </div>
            <div className="truncate text-sm font-semibold leading-tight">
              {snap?.current_game ?? t('dashboard.games.noGame')}
            </div>
          </div>
        </div>
        {/* 今日游戏时长 */}
        <div className="flex items-center gap-2 rounded-lg border border-border/60 px-2 py-1.5">
          <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-primary/15 bg-primary/10 text-primary">
            <Gamepad2 className="h-4 w-4 text-primary" />
          </span>
          <div className="min-w-0">
            <div className="truncate text-[11px] leading-tight text-muted-foreground">
              {t('dashboard.games.today')}
            </div>
            <div className="truncate text-sm font-semibold leading-tight">
              {snap ? formatDuration(snap.today_seconds) : '--'}
            </div>
          </div>
        </div>
        {!compact && (
          <div className="flex items-center gap-2 rounded-lg border border-border/60 px-2 py-1.5">
            <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-primary/15 bg-primary/10 text-primary">
              <Hourglass className="h-4 w-4" />
            </span>
            <div className="min-w-0">
              <div className="truncate text-[11px] leading-tight text-muted-foreground">
                {t('dashboard.games.streak')}
              </div>
              <div className="truncate text-sm font-semibold leading-tight">
                {snap ? formatDuration(snap.streak_seconds) : '--'}
              </div>
            </div>
          </div>
        )}
        {!compact && (
          <div className="flex items-center gap-2 rounded-lg border border-border/60 px-2 py-1.5">
            <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-primary/15 bg-primary/10 text-primary">
              <BellRing className="h-4 w-4" />
            </span>
            <div className="min-w-0">
              <div className="truncate text-[11px] leading-tight text-muted-foreground">
                {t('dashboard.games.nextReminder')}
              </div>
              <div className="truncate text-sm font-semibold leading-tight">
                {snap ? formatDuration(snap.next_reminder_seconds) : '--'}
              </div>
            </div>
          </div>
        )}
      </div>
    </CardShell>
  );
}
