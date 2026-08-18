'use client';

import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { apiService } from '../services/api';
import { useAppStore } from '../store/app-store';
import { Button } from './ui/index';
import { ToggleSwitch } from './ui/index';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';

interface PendingTakeover {
  exe_path: string;
  version: string;
  elevated: boolean;
}

/**
 * 新版接管确认框（应用内风格）：旧版运行中时双击新版 exe，
 * 旧版收到单实例唤醒 → 弹本对话框询问是否切换。
 */
export default function TakeoverDialog() {
  const { t } = useTranslation();
  const { config, updateConfig } = useAppStore(
    useShallow((s) => ({ config: s.config, updateConfig: s.updateConfig })),
  );
  const [pending, setPending] = useState<PendingTakeover | null>(null);
  const [switching, setSwitching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // 切换后是否打开主界面（默认跟随「启动时隐藏」设置的反面）。
  const [showWindow, setShowWindow] = useState(true);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<PendingTakeover>('takeover-pending', (event) => {
      if (!disposed) {
        setError(null);
        setPending(event.payload);
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const doSwitch = async () => {
    if (!pending) return;
    setSwitching(true);
    try {
      // 先把「切换后是否显示主界面」的选择写入配置，新版启动时遵守。
      if (config && config.launch_show_window !== showWindow) {
        await updateConfig({ ...config, launch_show_window: showWindow });
      }
      const result = await apiService.switchToPending(pending.exe_path, pending.elevated);
      if (!result.ok) {
        setError(result.message ?? t('takeover.failed'));
        setSwitching(false);
      }
    } catch {
      setSwitching(false);
      setError(t('takeover.failed'));
    }
  };

  return (
    <Dialog open={!!pending} onOpenChange={(open) => { if (!open) setPending(null); }}>
      <DialogContent hideClose>
        <DialogHeader>
          <DialogTitle>{t('takeover.title', { version: pending?.version ?? '' })}</DialogTitle>
          <DialogDescription>{t('takeover.description')}</DialogDescription>
        </DialogHeader>
        {pending?.exe_path && (
          <p className="break-all rounded-lg border border-border/60 bg-background/60 px-3 py-2 font-mono text-[11px] text-muted-foreground">
            {pending.exe_path}
          </p>
        )}
        {pending?.elevated && (
          <p className="rounded-lg border border-amber-500/25 bg-amber-500/5 px-3 py-2 text-xs leading-relaxed text-muted-foreground">
            {t('takeover.elevatedHint')}
          </p>
        )}
        {error && <p className="text-xs text-destructive">{error}</p>}
        <DialogFooter>
          <div className="flex w-full flex-col gap-3">
            <div className="flex items-center justify-between gap-3">
              <span className="text-xs text-muted-foreground">{t('takeover.showWindow')}</span>
              <ToggleSwitch
                size="sm"
                enabled={showWindow}
                onChange={(v) => setShowWindow(v)}
              />
            </div>
            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => { setError(null); setPending(null); }}>
                {t('takeover.later')}
              </Button>
              <Button loading={switching} onClick={() => void doSwitch()}>
                {t('takeover.switchNow')}
              </Button>
            </div>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
