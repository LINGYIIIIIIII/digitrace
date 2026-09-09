'use client';

// 游戏时长卡片：当前游戏 / 今日游戏时长 / 连续时长 / 下次提醒。
// 数据来自 get_game_snapshot（轻量，轮询刷新间隔），不依赖仪表盘主数据。

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { BellRing, CircleDot, Gamepad2, Hourglass } from 'lucide-react';
import { apiService } from '../../services/api';
import { useAppStore } from '../../store/app-store';
import type { GameSnapshotDto, WatchedGameDto } from '../../types';
import { CardShell, clsx, formatDuration } from './card-common';
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

/** 关注的游戏卡片：今日是否启动 + 游玩时长。数据来自 get_watched_games_today。 */
export function WatchedGamesCard({ size }: { size: CardSize }) {
  const { t } = useTranslation();
  const { config } = useAppStore(useShallow((s) => ({ config: s.config })));
  const refreshSeconds = config?.refresh_interval_seconds ?? 10;
  const [watched, setWatched] = useState<WatchedGameDto[] | null>(null);
  const compact = size === '1x1' || size === '1x2';

  useEffect(() => {
    let disposed = false;
    const tick = async () => {
      try {
        const next = await apiService.getWatchedGamesToday();
        if (!disposed) setWatched(next);
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

  return (
    <CardShell title={t('dashboard.cards.watchedGames')}>
      {watched === null ? (
        <div className="flex flex-1 items-center justify-center text-xs text-muted-foreground">
          {t('settings.games.loading')}
        </div>
      ) : watched.length === 0 ? (
        <div className="flex flex-1 items-center justify-center px-2 text-center text-xs text-muted-foreground">
          {t('settings.games.watchedEmpty')}
        </div>
      ) : (
        <div className="flex h-full flex-col gap-1.5">
          {watched.slice(0, compact ? 3 : 8).map((g) => (
            <div key={g.title} className="flex items-center gap-2 rounded-lg border border-border/60 px-2 py-1.5">
              <span className={clsx('flex h-6 w-6 shrink-0 items-center justify-center rounded-md border text-primary', g.launched_today ? 'border-primary/20 bg-primary/10' : 'border-border bg-muted text-muted-foreground')}>
                <Gamepad2 className="h-4 w-4" />
              </span>
              <div className="min-w-0 flex-1">
                <div className="truncate text-[11px] leading-tight text-muted-foreground">{g.title}</div>
                <div className="truncate text-sm font-semibold leading-tight">
                  {g.launched_today ? formatDuration(g.today_seconds) : t('settings.games.notLaunched')}
                </div>
              </div>
              {g.launched_today && <CircleDot className="h-3 w-3 shrink-0 text-primary" />}
            </div>
          ))}
        </div>
      )}
    </CardShell>
  );
}
