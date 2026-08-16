'use client';

import type { CSSProperties, KeyboardEvent, ReactNode } from 'react';
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { motion, AnimatePresence, type Variants } from 'framer-motion';
import {
  CalendarDays,
  ChartColumn,
  ChartLine,
  Clock3,
  Cpu,
  LayoutGrid,
  Minus,
  PanelLeftClose,
  PanelLeftOpen,
  Settings2,
  Square,
  Timer,
  Copy,
  TriangleAlert,
  Activity,
  Radio,
  X,
  Info,
  HeartPulse,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getVersion } from '@tauri-apps/api/app';
import clsx from 'clsx';
import { useTranslation } from 'react-i18next';
import { BRAND } from '../lib/brand';
import { applyFontFamily, applyThemeMode } from '../lib/appearance';
import { useAppStore } from '../store/app-store';
import { apiService } from '../services/api';
import TitlebarZoom from './TitlebarZoom';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

const MAIN_TAB_ITEMS = [
  { id: 'dashboard', titleKey: 'appShell.tabs.dashboard', icon: LayoutGrid },
  { id: 'usage', titleKey: 'appShell.tabs.usage', icon: ChartColumn },
  { id: 'calendar', titleKey: 'appShell.tabs.calendar', icon: CalendarDays },
  { id: 'network', titleKey: 'appShell.tabs.network', icon: ChartLine },
  { id: 'health', titleKey: 'appShell.tabs.health', icon: HeartPulse },
  { id: 'hardware', titleKey: 'appShell.tabs.hardware', icon: Cpu },
  { id: 'settings', titleKey: 'appShell.tabs.settings', icon: Settings2 },
] as const;

const ABOUT_TAB = { id: 'about', titleKey: 'appShell.tabs.about', icon: Info } as const;

type ActiveTab = (typeof MAIN_TAB_ITEMS)[number]['id'] | typeof ABOUT_TAB.id;

const SIDEBAR_COLLAPSED_WIDTH = 64; // w-16
const SIDEBAR_EXPANDED_WIDTH = 216;
const SIDEBAR_EXPANDED_STORAGE_KEY = 'digitrace.sidebar.expanded';

function readStoredSidebarExpanded(): boolean {
  try {
    return window.localStorage.getItem(SIDEBAR_EXPANDED_STORAGE_KEY) === '1';
  } catch {
    return false;
  }
}

const TAB_TRANSITION_ORDER: ActiveTab[] = [...MAIN_TAB_ITEMS.map((tab) => tab.id), ABOUT_TAB.id];

function getTabTransitionDirection(fromTab: ActiveTab, toTab: ActiveTab) {
  const fromIndex = TAB_TRANSITION_ORDER.indexOf(fromTab);
  const toIndex = TAB_TRANSITION_ORDER.indexOf(toTab);
  if (fromIndex === -1 || toIndex === -1 || fromIndex === toIndex) {
    return 0;
  }
  return toIndex > fromIndex ? 1 : -1;
}

// 页面切换：纯淡入淡出，避免横向/纵向「飞入」观感。
const TAB_CONTENT_VARIANTS: Variants = {
  enter: { opacity: 0 },
  center: {
    opacity: 1,
    transition: { duration: 0.16, ease: 'easeOut' },
  },
  exit: {
    opacity: 0,
    transition: { duration: 0.12, ease: 'easeIn' },
  },
};

interface AppShellProps {
  activeTab: ActiveTab;
  onTabChange: (tab: ActiveTab) => void;
  isMonitoring: boolean;
  error: string | null;
  bridgeWarning: string | null;
  onDismissBridgeWarning: () => void;
  dashboardContent: ReactNode;
  usageContent: ReactNode;
  calendarContent: ReactNode;
  networkContent: ReactNode;
  healthContent: ReactNode;
  hardwareContent: ReactNode;
  settingsContent: ReactNode;
  aboutContent: ReactNode;
}

// Tauri 拖拽区域是 DOM 属性（不是 CSS），必须作为 data-* 属性写在元素上。
// 值必须用 "deep"：空值/true 只对「直接点击该元素本身」生效，内部子元素点击不会拖拽；
// "deep" 让标题栏内任意子元素点击都能拖动（对应 THRM 的 Wails CSS 继承行为）。
const DRAG_PROPS = { 'data-tauri-drag-region': 'deep' } as const;
const NO_DRAG_PROPS = { 'data-tauri-drag-region': 'false' } as const;

/* ──────────────────────────────────────────────────────────────
 * TitleBar — slim, fixed at the very top of the window.
 * Outside the scroll viewport, so window controls never scroll.
 * ────────────────────────────────────────────────────────────── */

function TitleBarButton({
  icon,
  label,
  onClick,
  danger = false,
}: {
  icon: ReactNode;
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      {...NO_DRAG_PROPS}
      onClick={(event) => {
        event.stopPropagation();
        onClick();
      }}
      className={clsx(
        'flex h-8 w-10 cursor-pointer items-center justify-center rounded-md text-muted-foreground transition-colors',
        danger
          ? 'hover:bg-red-500 hover:text-white'
          : 'hover:bg-foreground/10 hover:text-foreground',
      )}
    >
      {icon}
    </button>
  );
}

function TitleBar({
  minimizeLabel,
  maximizeLabel,
  restoreLabel,
  closeLabel,
  isMaximised,
  leftSlot,
  leftOffset,
  onMinimise,
  onToggleMaximise,
  onClose,
}: {
  minimizeLabel: string;
  maximizeLabel: string;
  restoreLabel: string;
  closeLabel: string;
  isMaximised: boolean;
  leftSlot?: ReactNode;
  leftOffset: number;
  onMinimise: () => void;
  onToggleMaximise: () => void;
  onClose: () => void;
}) {
  return (
    <>
      <div
        className="glacier-titlebar pointer-events-auto absolute right-0 top-0 z-40 flex h-10 items-center bg-background pr-32 transition-[left] duration-300 ease-out"
        {...DRAG_PROPS}
        style={{ left: leftOffset }}
      >
        <div className="flex h-full min-w-0 flex-1 items-center px-3 pt-1">
          {leftSlot}
        </div>
      </div>

      {/* Keep only native window controls interactive above modal overlays. */}
      <div
        className="glacier-titlebar pointer-events-auto absolute right-0 top-0 z-[9999] flex h-10 items-center gap-0.5 bg-transparent pr-1"
        {...NO_DRAG_PROPS}
      >
        <TitlebarZoom />
        <TitleBarButton icon={<Minus className="h-3.5 w-3.5" />} label={minimizeLabel} onClick={onMinimise} />
        <TitleBarButton
          icon={isMaximised ? <Copy className="h-3 w-3" /> : <Square className="h-3 w-3" />}
          label={isMaximised ? restoreLabel : maximizeLabel}
          onClick={onToggleMaximise}
        />
        <TitleBarButton icon={<X className="h-3.5 w-3.5" />} label={closeLabel} onClick={onClose} danger />
      </div>
    </>
  );
}

function StatusBadges({
  isMonitoring,
  items,
  compact = false,
}: {
  isMonitoring: boolean;
  items: string[];
  compact?: boolean;
}) {
  const { t } = useTranslation();
  const [today, setToday] = useState<{ active: number; idle: number } | null>(null);
  const refreshSeconds = useAppStore((state) => state.config?.refresh_interval_seconds ?? 10);
  const baseClass = compact
    ? 'inline-flex h-6 items-center gap-1.5 rounded-full border px-2.5 text-[11px] font-medium'
    : 'inline-flex h-8 items-center gap-1.5 rounded-xl border px-3 text-[13px] font-medium';

  const needsToday = items.includes('active') || items.includes('idle');

  useEffect(() => {
    if (!needsToday) {
      setToday(null);
      return;
    }
    const now = new Date();
    const date = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
    let disposed = false;
    const load = async () => {
      try {
        const data = await apiService.getDashboardData(date, date);
        if (!disposed) {
          setToday({ active: data.active_seconds, idle: data.idle_seconds });
        }
      } catch {
        /* 静默 */
      }
    };
    void load();
    const timer = window.setInterval(() => void load(), Math.max(5, refreshSeconds) * 1000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [needsToday, refreshSeconds]);

  const formatDuration = (seconds: number) => {
    if (!seconds || seconds <= 0) return '0 分';
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    return h > 0 ? `${h}小时${m}分` : `${m}分`;
  };

  return (
    <div
      className={clsx(
        'flex min-w-0 items-center gap-2 text-[13px] tabular-nums',
        compact && 'translate-y-px overflow-hidden whitespace-nowrap',
      )}
    >
      {items.includes('monitoring') && (
        <span
          className={clsx(
            baseClass,
            'glacier-status-chip',
            isMonitoring
              ? 'glacier-status-chip--tint border-primary/20 bg-primary/10 text-primary'
              : 'border-border bg-card text-muted-foreground',
          )}
        >
          <Activity className="h-3.5 w-3.5" />
          {isMonitoring ? t('appShell.status.monitoring') : t('appShell.status.paused')}
        </span>
      )}

      {items.includes('tray') && (
        <span
          className={clsx(
            baseClass,
            'glacier-status-chip border-border bg-card font-semibold text-primary shadow-sm shadow-black/5',
          )}
        >
          <Radio className="h-3.5 w-3.5" />
          {t('appShell.status.trayRunning')}
        </span>
      )}

      {items.includes('active') && today && (
        <span
          className={clsx(baseClass, 'glacier-status-chip border-border bg-card font-semibold shadow-sm shadow-black/5')}
        >
          <Timer className="h-3.5 w-3.5 text-primary" />
          {t('appShell.status.todayActive')} {formatDuration(today.active)}
        </span>
      )}

      {items.includes('idle') && today && (
        <span
          className={clsx(baseClass, 'glacier-status-chip border-border bg-card text-muted-foreground shadow-sm shadow-black/5')}
        >
          <Clock3 className="h-3.5 w-3.5" />
          {t('appShell.status.todayIdle')} {formatDuration(today.idle)}
        </span>
      )}
    </div>
  );
}

/* ──────────────────────────────────────────────────────────────
 * OverlayScrollbar — floating thumb, never reserves width.
 * Native scrollbar is hidden via .app-scroll-root--hide-native.
 * ────────────────────────────────────────────────────────────── */

function OverlayScrollbar({
  scrollRef,
  topOffset = 6,
}: {
  scrollRef: React.RefObject<HTMLDivElement | null>;
  topOffset?: number;
}) {
  const trackRef = useRef<HTMLDivElement | null>(null);
  const thumbRef = useRef<HTMLDivElement | null>(null);
  const hideTimerRef = useRef<number | null>(null);
  const draggingRef = useRef<{ startY: number; startScroll: number } | null>(null);
  const [visible, setVisible] = useState(false);
  const [hasOverflow, setHasOverflow] = useState(false);

  const updateThumb = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;

    const { scrollTop, scrollHeight, clientHeight } = el;
    const overflow = scrollHeight - clientHeight;
    if (overflow <= 1) {
      setHasOverflow(false);
      setVisible(false);
      return;
    }
    setHasOverflow(true);

    const thumb = thumbRef.current;
    const track = trackRef.current;
    if (!thumb || !track) return;

    const trackHeight = track.clientHeight;
    const ratio = clientHeight / scrollHeight;
    const thumbHeight = Math.max(28, trackHeight * ratio);
    const maxThumbTop = trackHeight - thumbHeight;
    const top = (scrollTop / overflow) * maxThumbTop;
    thumb.style.height = `${thumbHeight}px`;
    thumb.style.transform = `translateY(${top}px)`;
  }, [scrollRef]);

  const flashVisible = useCallback(() => {
    setVisible(true);
    if (hideTimerRef.current) {
      window.clearTimeout(hideTimerRef.current);
    }
    hideTimerRef.current = window.setTimeout(() => {
      if (!draggingRef.current) {
        setVisible(false);
      }
    }, 1400);
  }, []);

  useLayoutEffect(() => {
    updateThumb();
  }, [hasOverflow, updateThumb]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;

    const onActivity = () => {
      updateThumb();
      flashVisible();
    };

    el.addEventListener('scroll', onActivity, { passive: true });
    el.addEventListener('mouseenter', onActivity);
    el.addEventListener('wheel', onActivity, { passive: true });
    el.addEventListener('touchstart', onActivity, { passive: true });

    const ro = new ResizeObserver(() => updateThumb());
    ro.observe(el);
    const content = el.firstElementChild;
    if (content instanceof HTMLElement) {
      ro.observe(content);
    }

    updateThumb();
    if (el.scrollHeight - el.clientHeight > 1) {
      flashVisible();
    }

    return () => {
      el.removeEventListener('scroll', onActivity);
      el.removeEventListener('mouseenter', onActivity);
      el.removeEventListener('wheel', onActivity);
      el.removeEventListener('touchstart', onActivity);
      ro.disconnect();
      if (hideTimerRef.current) window.clearTimeout(hideTimerRef.current);
    };
  }, [scrollRef, updateThumb, flashVisible]);

  const handleThumbPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const el = scrollRef.current;
      if (!el) return;
      event.preventDefault();
      (event.target as HTMLElement).setPointerCapture(event.pointerId);
      draggingRef.current = { startY: event.clientY, startScroll: el.scrollTop };
      setVisible(true);
    },
    [scrollRef],
  );

  const handleThumbPointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const drag = draggingRef.current;
      const el = scrollRef.current;
      const track = trackRef.current;
      const thumb = thumbRef.current;
      if (!drag || !el || !track || !thumb) return;
      const dy = event.clientY - drag.startY;
      const trackHeight = track.clientHeight;
      const thumbHeight = thumb.clientHeight;
      const maxThumbTop = trackHeight - thumbHeight;
      if (maxThumbTop <= 0) return;
      const overflow = el.scrollHeight - el.clientHeight;
      const scrollDelta = (dy / maxThumbTop) * overflow;
      el.scrollTop = drag.startScroll + scrollDelta;
    },
    [scrollRef],
  );

  const handleThumbPointerUp = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      draggingRef.current = null;
      try {
        (event.target as HTMLElement).releasePointerCapture(event.pointerId);
      } catch {
        /* noop */
      }
      flashVisible();
    },
    [flashVisible],
  );

  if (!hasOverflow) return null;

  return (
    <div
      ref={trackRef}
      className={clsx('app-overlay-scrollbar', visible && 'is-visible')}
      style={{ top: topOffset }}
      onMouseEnter={() => setVisible(true)}
      onMouseLeave={flashVisible}
    >
      <div
        ref={thumbRef}
        className="app-overlay-scrollbar-thumb"
        onPointerDown={handleThumbPointerDown}
        onPointerMove={handleThumbPointerMove}
        onPointerUp={handleThumbPointerUp}
        onPointerCancel={handleThumbPointerUp}
      />
    </div>
  );
}

function SidebarNavButton({
  icon: Icon,
  label,
  isActive,
  expanded,
  onClick,
  role,
}: {
  icon: LucideIcon;
  label: string;
  isActive: boolean;
  expanded: boolean;
  onClick: () => void;
  role?: 'tab';
}) {
  const button = (
    <button
      type="button"
      role={role}
      aria-label={label}
      aria-selected={isActive}
      onClick={onClick}
      className={clsx(
        'group/nav relative flex h-11 w-full cursor-pointer items-center rounded-xl text-left transition-colors duration-200',
        isActive ? 'text-primary' : 'text-sidebar-foreground/62 hover:text-sidebar-foreground',
      )}
    >
      <span
        className={clsx(
          'pointer-events-none absolute inset-y-0 left-2.5 right-2.5 rounded-xl transition-colors duration-200',
          isActive ? 'border border-primary/15 bg-primary/10' : 'bg-transparent group-hover/nav:bg-sidebar-accent',
        )}
      />
      <span className="relative z-10 flex w-16 shrink-0 items-center justify-center">
        <Icon className="h-4.5 w-4.5" />
      </span>
      <span className="relative z-10 min-w-0 flex-1 truncate pr-3 text-sm font-medium">{label}</span>
    </button>
  );

  if (expanded) {
    return button;
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

/* ──────────────────────────────────────────────────────────────
 * AppShell — layout
 * ────────────────────────────────────────────────────────────── */

export default function AppShell({
  activeTab,
  onTabChange,
  isMonitoring,
  error,
  bridgeWarning,
  onDismissBridgeWarning,
  dashboardContent,
  usageContent,
  calendarContent,
  networkContent,
  healthContent,
  hardwareContent,
  settingsContent,
  aboutContent,
}: AppShellProps) {
  const { t } = useTranslation();
  const [isWindowsChrome, setIsWindowsChrome] = useState(false);
  const [nativeBackdrop, setNativeBackdrop] = useState(false);
  const [isMaximised, setIsMaximised] = useState(false);
  const [appVersion, setAppVersion] = useState('');
  const [sidebarExpanded, setSidebarExpanded] = useState(readStoredSidebarExpanded);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const previousActiveTabRef = useRef<ActiveTab>(activeTab);
  const windowBlur = useAppStore((state) => state.config?.window_blur);
  const titlebarItems = useAppStore((state) => state.config?.titlebar_items) ?? ['monitoring', 'tray'];
  const themeMode = useAppStore((state) => state.config?.theme_mode);
  const fontFamily = useAppStore((state) => state.config?.font_family);
  const startMinimized = useAppStore((state) => state.config?.start_minimized);
  const launchShowWindow = useAppStore((state) => state.config?.launch_show_window);

  // 配置加载后立即应用主题与字体（重启后保持，不依赖进入设置页）。
  useEffect(() => {
    if (themeMode) applyThemeMode(themeMode);
  }, [themeMode]);

  useEffect(() => {
    if (fontFamily) applyFontFamily(fontFamily);
  }, [fontFamily]);

  useEffect(() => {
    const applySystem = () => {
      if (!themeMode || themeMode === 'system') applyThemeMode('system');
    };
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    mq.addEventListener('change', applySystem);
    return () => mq.removeEventListener('change', applySystem);
  }, [themeMode]);

  useEffect(() => {
    getVersion()
      .then(setAppVersion)
      .catch(() => setAppVersion(''));
  }, []);

  // 窗口初始隐藏（visible: false）：前端就绪后显示，避免启动白框/透明框闪烁。
  // 「静默自启」（start_minimized）或关闭「启动显示主界面」时不显示窗口。
  useEffect(() => {
    if (!startMinimized && (launchShowWindow ?? true)) {
      void getCurrentWindow().show();
    }
  }, [startMinimized, launchShowWindow]);

  const syncWindowState = useCallback(async () => {
    try {
      setIsMaximised(await getCurrentWindow().isMaximized());
    } catch {
      setIsMaximised(false);
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let cleanup = () => {};

    const initializeWindowChrome = async () => {
      try {
        const isWindows = /Windows/i.test(navigator.userAgent);
        if (disposed) return;
        setIsWindowsChrome(isWindows);
        if (!isWindows) {
          setNativeBackdrop(false);
          setIsMaximised(false);
          return;
        }
        // nativeBackdrop 由下方独立 effect 依据配置驱动。
        const handleResize = () => void syncWindowState();
        window.addEventListener('resize', handleResize);
        cleanup = () => window.removeEventListener('resize', handleResize);
        await syncWindowState();
      } catch {
        if (!disposed) {
          setIsWindowsChrome(false);
          setIsMaximised(false);
        }
      }
    };

    void initializeWindowChrome();

    return () => {
      disposed = true;
      cleanup();
    };
  }, [syncWindowState]);

  useEffect(() => {
    if (!isWindowsChrome) {
      setNativeBackdrop(false);
      return;
    }
    setNativeBackdrop(windowBlur !== 'off');
  }, [isWindowsChrome, windowBlur]);

  const scheduleWindowStateSync = useCallback(() => {
    window.setTimeout(() => void syncWindowState(), 80);
  }, [syncWindowState]);

  const handleToggleMaximise = useCallback(() => {
    void getCurrentWindow().toggleMaximize();
    scheduleWindowStateSync();
  }, [scheduleWindowStateSync]);

  const handleOpenRepository = useCallback(() => {
    // 数迹仓库地址待定；为空时点击 logo 不打开任何链接。
    if (BRAND.repositoryUrl) {
      window.open(BRAND.repositoryUrl, '_blank', 'noopener,noreferrer');
    }
  }, []);

  const handleLogoKeyDown = useCallback((event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      handleOpenRepository();
    }
  }, [handleOpenRepository]);

  const handleToggleSidebar = useCallback(() => {
    setSidebarExpanded((prev) => {
      const next = !prev;
      try {
        window.localStorage.setItem(SIDEBAR_EXPANDED_STORAGE_KEY, next ? '1' : '0');
      } catch {
        /* 持久化失败不影响交互 */
      }
      return next;
    });
  }, []);

  const sidebarWidth = sidebarExpanded ? SIDEBAR_EXPANDED_WIDTH : SIDEBAR_COLLAPSED_WIDTH;

  const handleTabChange = (tab: ActiveTab) => {
    if (tab === activeTab) return;
    onTabChange(tab);
  };

  const contentMap: Record<ActiveTab, ReactNode> = {
    dashboard: dashboardContent,
    usage: usageContent,
    calendar: calendarContent,
    network: networkContent,
    health: healthContent,
    hardware: hardwareContent,
    settings: settingsContent,
    about: aboutContent,
  };
  const transitionDirection = getTabTransitionDirection(previousActiveTabRef.current, activeTab);

  useEffect(() => {
    if (previousActiveTabRef.current === activeTab) {
      return;
    }
    const scrollElement = scrollRef.current;
    if (scrollElement) {
      scrollElement.scrollTop = 0;
      scrollElement.scrollLeft = 0;
    }
    previousActiveTabRef.current = activeTab;
  }, [activeTab]);

  return (
    <div
      className={clsx(
        'glacier-shell relative flex h-dvh w-full overflow-hidden bg-background text-foreground',
        isWindowsChrome && nativeBackdrop && 'glacier-native-backdrop',
      )}
    >
      {isWindowsChrome && (
        <TitleBar
          minimizeLabel={t('appShell.titleBar.minimize')}
          maximizeLabel={t('appShell.titleBar.maximize')}
          restoreLabel={t('appShell.titleBar.restore')}
          closeLabel={t('appShell.titleBar.close')}
          isMaximised={isMaximised}
          leftOffset={sidebarWidth}
          leftSlot={
            <div className="flex min-w-0 items-center gap-3">
              <StatusBadges isMonitoring={isMonitoring} items={titlebarItems} compact />
            </div>
          }
          onMinimise={() => getCurrentWindow().minimize()}
          onToggleMaximise={handleToggleMaximise}
          onClose={() => getCurrentWindow().close()}
        />
      )}

      <aside
        className="glacier-sidebar flex shrink-0 flex-col overflow-hidden border-r border-sidebar-border bg-sidebar text-sidebar-foreground shadow-[1px_0_0_rgba(15,23,42,0.04)] transition-[width] duration-300 ease-out dark:shadow-[1px_0_0_rgba(255,255,255,0.04)]"
        style={{ width: sidebarWidth }}
      >
        <div className="flex h-[76px] items-center pl-2" {...DRAG_PROPS}>
          <div
            aria-label={BRAND.name}
            role="link"
            tabIndex={0}
            onClick={handleOpenRepository}
            onKeyDown={handleLogoKeyDown}
            className="group flex cursor-pointer items-center gap-1.5 outline-none"
            {...NO_DRAG_PROPS}
          >
            <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary/12 text-primary">
              <Activity className="h-4.5 w-4.5" />
            </span>
            <span
              className={clsx(
                'flex min-w-0 items-center gap-1.5 transition-all duration-300',
                sidebarExpanded ? 'w-auto opacity-100' : 'w-0 opacity-0',
              )}
            >
              <span className="truncate text-base font-semibold tracking-wide text-foreground">
                {BRAND.name}
              </span>
              {appVersion && (
                <span className="mt-px shrink-0 rounded bg-muted/80 px-1 py-[1px] text-[9px] font-medium leading-none text-muted-foreground">
                  v{appVersion}
                </span>
              )}
            </span>
          </div>
        </div>

        <nav className="flex flex-1 flex-col gap-1" role="tablist" {...NO_DRAG_PROPS}>
          {MAIN_TAB_ITEMS.map((tab) => (
            <SidebarNavButton
              key={tab.id}
              icon={tab.icon}
              label={t(tab.titleKey)}
              isActive={activeTab === tab.id}
              expanded={sidebarExpanded}
              onClick={() => handleTabChange(tab.id)}
              role="tab"
            />
          ))}
        </nav>

        <div className="flex flex-col gap-1 pb-5" {...NO_DRAG_PROPS}>
          <SidebarNavButton
            icon={ABOUT_TAB.icon}
            label={t(ABOUT_TAB.titleKey)}
            isActive={activeTab === ABOUT_TAB.id}
            expanded={sidebarExpanded}
            onClick={() => handleTabChange(ABOUT_TAB.id)}
          />

          {(() => {
            const toggleLabel = sidebarExpanded ? t('appShell.sidebar.collapse') : t('appShell.sidebar.expand');
            const ToggleIcon = sidebarExpanded ? PanelLeftClose : PanelLeftOpen;
            const toggleButton = (
              <button
                type="button"
                aria-label={toggleLabel}
                aria-expanded={sidebarExpanded}
                onClick={handleToggleSidebar}
                className="group/nav relative flex h-11 w-full cursor-pointer items-center rounded-xl text-left text-sidebar-foreground/62 transition-colors duration-200 hover:text-sidebar-foreground"
              >
                <span className="pointer-events-none absolute inset-y-0 left-2.5 right-2.5 rounded-xl bg-transparent transition-colors duration-200 group-hover/nav:bg-sidebar-accent" />
                <span className="relative z-10 flex w-16 shrink-0 items-center justify-center">
                  <ToggleIcon className="h-4.5 w-4.5" />
                </span>
                <span className="relative z-10 min-w-0 flex-1 truncate pr-3 text-sm font-medium">{toggleLabel}</span>
              </button>
            );
            return sidebarExpanded ? (
              toggleButton
            ) : (
              <Tooltip>
                <TooltipTrigger asChild>{toggleButton}</TooltipTrigger>
                <TooltipContent side="right">{toggleLabel}</TooltipContent>
              </Tooltip>
            );
          })()}
        </div>
      </aside>

      <section className="glacier-content relative flex min-w-0 flex-1 flex-col overflow-hidden">
        {!isWindowsChrome && (
          <header
            className="shrink-0 border-b border-border/65 bg-background/92 px-4 pb-3 pt-3 backdrop-blur-xl sm:px-5 lg:px-6"
            {...DRAG_PROPS}
          >
            <div
              className="mx-auto flex min-h-9 max-w-[1120px] min-[1536px]:max-w-[1280px] min-[1800px]:max-w-[1440px] min-[2400px]:max-w-[1560px] items-center justify-start gap-3"
              {...NO_DRAG_PROPS}
            >
              <StatusBadges isMonitoring={isMonitoring} items={titlebarItems} />
            </div>
          </header>
        )}

        <div className="glacier-content-panel relative min-h-0 flex-1 overflow-hidden">
          <div
            ref={scrollRef}
            className="app-scroll-root app-scroll-root--hide-native h-full"
            {...NO_DRAG_PROPS}
          >
            <div id="ui-zoom-root" className="min-h-full px-4 pb-6 pt-4 sm:px-5 lg:px-6">
              {/* Alerts */}
              <div className="mx-auto max-w-[1120px] min-[1536px]:max-w-[1280px] min-[1800px]:max-w-[1440px] min-[2400px]:max-w-[1560px]">
                <AnimatePresence>
                  {error && (
                    <motion.div
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: 'auto' }}
                      exit={{ opacity: 0, height: 0 }}
                      className="overflow-hidden"
                    >
                      <div className="mb-3 rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-2.5 text-sm text-destructive">
                        {error}
                      </div>
                    </motion.div>
                  )}

                  {bridgeWarning && (
                    <motion.div
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: 'auto' }}
                      exit={{ opacity: 0, height: 0 }}
                      className="overflow-hidden"
                    >
                      <div className="mb-3 flex items-start gap-3 rounded-lg border border-amber-300/50 bg-amber-50/80 px-4 py-2.5 text-amber-800 dark:border-amber-700/40 dark:bg-amber-900/15 dark:text-amber-200">
                        <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0" />
                        <p className="flex-1 text-sm leading-relaxed">{bridgeWarning}</p>
                        <button
                          type="button"
                          aria-label={t('appShell.bridgeWarning.closeAria')}
                          onClick={onDismissBridgeWarning}
                          className="cursor-pointer rounded p-0.5 transition hover:bg-amber-200/60 dark:hover:bg-amber-800/40"
                        >
                          <X className="h-3.5 w-3.5" />
                        </button>
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>

              {/* Tab content */}
              <main className="mx-auto w-full max-w-[1120px] min-[1536px]:max-w-[1280px] min-[1800px]:max-w-[1440px] min-[2400px]:max-w-[1560px] min-w-0 overflow-hidden">
                <AnimatePresence mode="wait" initial={false} custom={transitionDirection}>
                  <motion.div
                    key={activeTab}
                    custom={transitionDirection}
                    variants={TAB_CONTENT_VARIANTS}
                    initial="enter"
                    animate="center"
                    exit="exit"
                    data-page-reveal="cards"
                    className="w-full min-w-0 px-1 pb-2 will-change-transform"
                  >
                    {contentMap[activeTab]}
                  </motion.div>
                </AnimatePresence>
              </main>
            </div>
          </div>

          {/* Floating overlay scrollbar — never reserves width */}
          <OverlayScrollbar scrollRef={scrollRef} topOffset={6} />
        </div>
      </section>
    </div>
  );
}
