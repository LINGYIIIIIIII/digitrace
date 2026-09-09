'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { BellRing, CircleDot, Gamepad2, Hourglass, RefreshCw, Star } from 'lucide-react';
import clsx from 'clsx';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { apiService } from '../services/api';
import { useAppStore } from '../store/app-store';
import type { GameEntryDto, GameSnapshotDto, WatchedGameDto } from '../types';
import AppIcon from './dashboard/AppIcon';
import { Badge, Button, Card, Select, Skeleton } from './ui/index';

type GameSort = 'total' | 'today' | 'alpha';

function formatDuration(totalSeconds: number, t: TFunction): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) return t('settings.games.durationHoursMinutes', { hours, minutes });
  return t('settings.games.durationMinutes', { minutes });
}

export default function GamesPage() {
  const { t } = useTranslation();
  const refreshSeconds = useAppStore(useShallow((s) => s.config?.refresh_interval_seconds ?? 10));
  const [snapshot, setSnapshot] = useState<GameSnapshotDto | null>(null);
  const [games, setGames] = useState<GameEntryDto[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [sortBy, setSortBy] = useState<GameSort>('total');
  const [watched, setWatched] = useState<WatchedGameDto[] | null>(null);

  const loadGames = useCallback(async () => {
    setLoading(true);
    try {
      setGames(await apiService.getGamesLibrary());
    } catch {
      setGames(null);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadWatched = useCallback(async () => {
    try {
      setWatched(await apiService.getWatchedGamesToday());
    } catch {
      setWatched(null);
    }
  }, []);

  const toggleWatch = useCallback(async (id: number, watched: boolean) => {
    try {
      await apiService.setGameWatched(id, watched);
      await loadGames();
      await loadWatched();
    } catch {
      /* 静默 */
    }
  }, [loadGames, loadWatched]);

  const refreshGames = useCallback(async () => {
    setLoading(true);
    try {
      await apiService.refreshGameLibrary();
      await loadGames();
    } finally {
      setLoading(false);
    }
  }, [loadGames]);

  useEffect(() => {
    void loadGames();
    void loadWatched();
  }, [loadGames, loadWatched]);

  useEffect(() => {
    let disposed = false;
    const tick = async () => {
      try {
        const next = await apiService.getGameSnapshot();
        if (!disposed) setSnapshot(next);
      } catch {
        /* 页面保持上一次快照 */
      }
    };
    void tick();
    const timer = window.setInterval(() => void tick(), Math.max(2, refreshSeconds) * 1000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [refreshSeconds]);

  const sortedGames = useMemo(() => {
    if (!games) return null;
    return [...games].sort((a, b) => {
      if (sortBy === 'alpha') {
        return a.title.localeCompare(b.title, undefined, { sensitivity: 'base' });
      }
      const delta = sortBy === 'today'
        ? b.today_seconds - a.today_seconds
        : b.total_seconds - a.total_seconds;
      return delta || a.title.localeCompare(b.title);
    });
  }, [games, sortBy]);

  const stat = (icon: React.ReactNode, label: string, value: string) => (
    <Card padding="md" className="flex min-w-0 items-center gap-3">
      <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
        {icon}
      </span>
      <div className="min-w-0">
        <div className="truncate text-xs text-muted-foreground">{label}</div>
        <div className="truncate text-xl font-semibold tabular-nums">{value}</div>
      </div>
    </Card>
  );

  return (
    <div className="mx-auto max-w-5xl space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold">{t('settings.tabs.games')}</h1>
          <p className="mt-1 text-sm text-muted-foreground">{t('settings.games.libraryDescription')}</p>
        </div>
        <Button size="sm" variant="outline" loading={loading} onClick={() => void refreshGames()}>
          <RefreshCw className="mr-1.5 h-4 w-4" />
          {t('settings.games.refresh')}
        </Button>
      </div>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {stat(<Gamepad2 className="h-5 w-5" />, t('dashboard.games.nowPlaying'), snapshot?.current_game ?? t('dashboard.games.noGame'))}
        {stat(<Gamepad2 className="h-5 w-5" />, t('dashboard.games.today'), snapshot ? formatDuration(snapshot.today_seconds, t) : '--')}
        {stat(<Hourglass className="h-5 w-5" />, t('dashboard.games.streak'), snapshot ? formatDuration(snapshot.streak_seconds, t) : '--')}
        {stat(<BellRing className="h-5 w-5" />, t('dashboard.games.nextReminder'), snapshot ? formatDuration(snapshot.next_reminder_seconds, t) : '--')}
      </div>

      {/* 关注的游戏：今日启动 + 时长 */}
      <Card padding="none" className="overflow-hidden">
        <div className="border-b border-border/60 px-4 py-3">
          <h2 className="text-sm font-semibold">{t('settings.games.watchedTitle')}</h2>
          <p className="mt-0.5 text-xs text-muted-foreground">{t('settings.games.watchedDesc')}</p>
        </div>
        {watched === null ? (
          <div className="px-4 py-8 text-center text-sm text-muted-foreground">{t('settings.games.loading')}</div>
        ) : watched.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-muted-foreground">{t('settings.games.watchedEmpty')}</div>
        ) : (
          <div className="divide-y divide-border/50">
            {watched.map((g) => (
              <div key={g.title} className="flex items-center gap-3 px-4 py-3">
                <AppIcon exePath={g.exe_path} size={34} />
                <div className="min-w-0 flex-1">
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="min-w-0 truncate text-sm font-medium">{g.title}</span>
                    {g.launched_today && (
                      <Badge variant="success" size="sm">
                        <CircleDot className="mr-1 h-3 w-3" />
                        {t('settings.games.launched')}
                      </Badge>
                    )}
                  </div>
                </div>
                <div className="shrink-0 text-xs tabular-nums text-muted-foreground">
                  {t('settings.games.today')} {formatDuration(g.today_seconds, t)}
                </div>
              </div>
            ))}
          </div>
        )}
      </Card>

      <Card padding="none" className="overflow-hidden">
        <div className="flex items-center justify-between gap-3 border-b border-border/60 px-4 py-3">
          <div>
            <h2 className="text-sm font-semibold">{t('settings.games.library')}</h2>
            {games && <p className="mt-0.5 text-xs text-muted-foreground">{games.length}</p>}
          </div>
          <div className="flex items-center gap-3">
            <Select
              value={sortBy}
              onChange={setSortBy}
              size="sm"
              label={t('settings.games.sortBy')}
              triggerClassName="h-8 min-w-[10rem] text-xs"
              options={[
                { value: 'total', label: t('settings.games.sortTotal') },
                { value: 'today', label: t('settings.games.sortToday') },
                { value: 'alpha', label: t('settings.games.sortAlpha') },
              ]}
            />
          </div>
        </div>
        {games === null ? (
          loading ? (
            <div className="divide-y divide-border/50">
              {[0, 1, 2].map((item) => (
                <div key={item} className="flex items-center gap-3 px-4 py-3">
                  <Skeleton className="h-[34px] w-[34px] rounded-md" />
                  <div className="min-w-0 flex-1 space-y-2">
                    <Skeleton className="h-3.5 w-2/5" />
                    <Skeleton className="h-3 w-3/5" />
                  </div>
                  <Skeleton className="h-3.5 w-28" />
                </div>
              ))}
            </div>
          ) : (
            <div className="px-4 py-8 text-center text-sm text-muted-foreground">
              <p>{t('settings.games.loadFailed')}</p>
              <Button className="mt-3" size="sm" variant="outline" onClick={() => void loadGames()}>
                {t('settings.games.load')}
              </Button>
            </div>
          )
        ) : games.length === 0 ? (
          <p className="px-4 py-8 text-center text-sm text-muted-foreground">{t('settings.games.empty')}</p>
        ) : (
          <div className="divide-y divide-border/50">
            {sortedGames?.map((game) => (
              <div key={game.id} className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-3 px-4 py-3 transition-colors hover:bg-muted/35 sm:grid-cols-[auto_minmax(0,1fr)_auto]">
                <AppIcon exePath={game.exe_path} size={34} />
                <div className="min-w-0 flex-1">
                  <div className="flex min-w-0 items-center gap-2">
                    <div className="min-w-0 truncate text-sm font-medium">{game.title}</div>
                    {snapshot?.current_game === game.title && (
                      <span className="shrink-0">
                        <Badge variant="success" size="sm">
                          <CircleDot className="mr-1 h-3 w-3" />
                          {t('settings.games.playing')}
                        </Badge>
                      </span>
                    )}
                  </div>
                  <div className="truncate text-xs text-muted-foreground">{game.source} · {game.exe_path}</div>
                </div>
                <div className="col-start-2 flex shrink-0 items-center gap-3 text-xs tabular-nums text-muted-foreground sm:col-start-auto sm:gap-4">
                  <span>{t('settings.games.today')} {formatDuration(game.today_seconds, t)}</span>
                  <span>{t('settings.games.total')} {formatDuration(game.total_seconds, t)}</span>
                  <button
                    type="button"
                    aria-label={game.watched ? t('settings.games.unwatch') : t('settings.games.watch')}
                    title={game.watched ? t('settings.games.unwatch') : t('settings.games.watch')}
                    onClick={() => void toggleWatch(game.id, !game.watched)}
                    className={clsx(
                      'flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border transition-colors',
                      game.watched
                        ? 'border-primary/30 bg-primary/10 text-primary'
                        : 'border-border/70 bg-card text-muted-foreground hover:border-primary/40 hover:text-primary',
                    )}
                  >
                    <Star className={clsx('h-4 w-4', game.watched && 'fill-current')} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
