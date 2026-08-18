'use client';

import { useCallback, useEffect, useState } from 'react';
import { BellRing, Gamepad2, Hourglass, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { apiService } from '../services/api';
import { useAppStore } from '../store/app-store';
import type { GameEntryDto, GameSnapshotDto } from '../types';
import { Button, Card } from './ui/index';

function formatDuration(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) return `${hours}小时${minutes}分`;
  return `${minutes}分`;
}

export default function GamesPage() {
  const { t } = useTranslation();
  const refreshSeconds = useAppStore(useShallow((s) => s.config?.refresh_interval_seconds ?? 10));
  const [snapshot, setSnapshot] = useState<GameSnapshotDto | null>(null);
  const [games, setGames] = useState<GameEntryDto[] | null>(null);
  const [loading, setLoading] = useState(false);

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
  }, [loadGames]);

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
        {stat(<Gamepad2 className="h-5 w-5" />, t('dashboard.games.today'), snapshot ? formatDuration(snapshot.today_seconds) : '--')}
        {stat(<Hourglass className="h-5 w-5" />, t('dashboard.games.streak'), snapshot ? formatDuration(snapshot.streak_seconds) : '--')}
        {stat(<BellRing className="h-5 w-5" />, t('dashboard.games.nextReminder'), snapshot ? formatDuration(snapshot.next_reminder_seconds) : '--')}
      </div>

      <Card padding="none" className="overflow-hidden">
        <div className="flex items-center justify-between gap-3 border-b border-border/60 px-4 py-3">
          <div>
            <h2 className="text-sm font-semibold">{t('settings.games.library')}</h2>
            <p className="mt-0.5 text-xs text-muted-foreground">{t('settings.games.libraryDescription')}</p>
          </div>
          {games && <span className="text-xs tabular-nums text-muted-foreground">{games.length}</span>}
        </div>
        {games === null ? (
          <div className="px-4 py-8 text-center text-sm text-muted-foreground">
            <p>{t('settings.games.loadFailed')}</p>
            <Button className="mt-3" size="sm" variant="outline" onClick={() => void loadGames()}>
              {t('settings.games.load')}
            </Button>
          </div>
        ) : games.length === 0 ? (
          <p className="px-4 py-8 text-center text-sm text-muted-foreground">{t('settings.games.empty')}</p>
        ) : (
          <div className="divide-y divide-border/50">
            {games.map((game) => (
              <div key={game.id} className="flex flex-wrap items-center gap-x-4 gap-y-1 px-4 py-3">
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium">{game.title}</div>
                  <div className="truncate text-xs text-muted-foreground">{game.source} · {game.exe_path}</div>
                </div>
                <div className="flex shrink-0 items-center gap-4 text-xs tabular-nums text-muted-foreground">
                  <span>{t('settings.games.today')} {formatDuration(game.today_seconds)}</span>
                  <span>{t('settings.games.total')} {formatDuration(game.total_seconds)}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
