'use client';

/** 设置页共用：分类、选项表、行布局组件。自 SettingsPage 拆出，行为不变。 */
import type { ComponentType, ReactNode } from 'react';
import {
  Database,
  Gamepad2,
  Palette,
  RefreshCw,
  Rocket,
  SlidersHorizontal,
  Thermometer,
} from 'lucide-react';

export type SettingsCategory =
  | 'general'
  | 'appearance'
  | 'startup'
  | 'hardware'
  | 'update'
  | 'games'
  | 'logs';

export const CATEGORIES: {
  id: SettingsCategory;
  icon: ComponentType<{ className?: string }>;
  labelKey: string;
}[] = [
  { id: 'general', icon: SlidersHorizontal, labelKey: 'settings.tabs.general' },
  { id: 'appearance', icon: Palette, labelKey: 'settings.tabs.appearance' },
  { id: 'startup', icon: Rocket, labelKey: 'settings.tabs.startup' },
  { id: 'hardware', icon: Thermometer, labelKey: 'settings.tabs.hardware' },
  { id: 'update', icon: RefreshCw, labelKey: 'settings.tabs.update' },
  { id: 'games', icon: Gamepad2, labelKey: 'settings.tabs.games' },
  { id: 'logs', icon: Database, labelKey: 'settings.tabs.logs' },
];
export const THEME_MODE_OPTIONS = [
  { value: 'system', labelKey: 'settings.theme.system' },
  { value: 'light', labelKey: 'settings.theme.light' },
  { value: 'dark', labelKey: 'settings.theme.dark' },
];

export const WINDOW_BLUR_OPTIONS = [
  { value: 'auto', labelKey: 'settings.blur.auto' },
  { value: 'mica', labelKey: 'settings.blur.mica' },
  { value: 'acrylic', labelKey: 'settings.blur.acrylic' },
  { value: 'tabbed', labelKey: 'settings.blur.tabbed' },
  { value: 'off', labelKey: 'settings.blur.off' },
];

export const FONT_OPTIONS = [
  { value: 'system', labelKey: 'settings.font.system' },
  { value: 'harmonyos', labelKey: 'settings.font.harmonyos' },
  { value: 'noto', labelKey: 'settings.font.noto' },
  { value: 'misans', labelKey: 'settings.font.misans' },
];

export const TITLEBAR_ITEMS = [
  { value: 'monitoring', labelKey: 'settings.titlebar.monitoring' },
  { value: 'tray', labelKey: 'settings.titlebar.tray' },
  { value: 'active', labelKey: 'settings.titlebar.active' },
  { value: 'idle', labelKey: 'settings.titlebar.idle' },
];

export const TRAY_ITEMS = [
  { value: 'cpu', labelKey: 'settings.tray.cpu' },
  { value: 'memory', labelKey: 'settings.tray.memory' },
  { value: 'network', labelKey: 'settings.tray.network' },
  { value: 'active', labelKey: 'settings.tray.active' },
  { value: 'temp', labelKey: 'settings.tray.temp' },
];

// 语言选项固定显示各自母语名称，不随界面语言翻译（避免 THRM 的坑）。
export const LANGUAGE_OPTIONS = [
  { value: 'zh-CN', label: '中文' },
  { value: 'en-US', label: 'English' },
  { value: 'ja-JP', label: '日本語' },
];

export const POLL_INTERVAL_OPTIONS = [
  { value: '1000', label: '1 秒' },
  { value: '2000', label: '2 秒' },
  { value: '3000', label: '3 秒' },
  { value: '5000', label: '5 秒' },
];

export const IDLE_THRESHOLD_OPTIONS = [
  { value: '1', label: '1 分钟' },
  { value: '3', label: '3 分钟' },
  { value: '5', label: '5 分钟' },
  { value: '10', label: '10 分钟' },
];

export const REFRESH_INTERVAL_OPTIONS = [
  { value: '5', label: '5 秒' },
  { value: '10', label: '10 秒' },
  { value: '30', label: '30 秒' },
  { value: '60', label: '1 分钟' },
];

export const LIVE_REFRESH_INTERVAL_OPTIONS = [
  { value: '1', label: '1 秒' },
  { value: '2', label: '2 秒' },
  { value: '5', label: '5 秒' },
  { value: '10', label: '10 秒' },
];

export const NETWORK_LIVE_WINDOW_OPTIONS = [
  { value: '60', label: '1 分钟' },
  { value: '120', label: '2 分钟' },
  { value: '300', label: '5 分钟' },
  { value: '600', label: '10 分钟' },
];

/** 游戏时长人性化显示：秒/分钟/小时。 */
export function fmtGameHours(seconds: number): string {
  if (seconds < 60) return `${seconds} 秒`;
  const m = Math.round(seconds / 60);
  if (m < 60) return `${m} 分钟`;
  return `${(seconds / 3600).toFixed(1)} 小时`;
}

export function SettingRow({
  icon,
  title,
  description,
  children,
}: {
  icon: ReactNode;
  title: string;
  description: string;
  children: ReactNode;
}) {  return (
    <div className="flex items-center justify-between gap-4 px-5 py-4">
      <div className="flex min-w-0 items-start gap-3">
        <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-primary/15 bg-primary/10 text-primary">
          {icon}
        </span>
        <div className="min-w-0">
          <div className="text-sm font-medium text-foreground">{title}</div>
          <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">{description}</p>
        </div>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

