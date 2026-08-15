'use client';

import dynamic from 'next/dynamic';
import { useCallback, useEffect } from 'react';
import { useShallow } from 'zustand/react/shallow';
import AppFatalError from './components/AppFatalError';
import AppLoadingSkeleton from './components/AppLoadingSkeleton';
import AppShell from './components/AppShell';
import { apiService } from './services/api';
import { applyUiZoom, loadUiZoom } from './lib/appearance';
import TakeoverDialog from './components/TakeoverDialog';
import DashboardPage from './components/dashboard/DashboardPage';
import { useAppBootstrap } from './hooks/useAppBootstrap';
import { useAppStore } from './store/app-store';

// 首屏只加载仪表盘；其余页面按需分包（next/dynamic）：减小首屏 JS 体积与
// 渲染进程内存占用、加快启动——切页时才加载对应页面 chunk（本地加载很快）。
const pageLoading = () => (
  <div className="py-24 text-center text-sm text-muted-foreground">…</div>
);
const AppUsagePage = dynamic(() => import('./components/AppUsagePage'), { ssr: false, loading: pageLoading });
const CalendarPage = dynamic(() => import('./components/CalendarPage'), { ssr: false, loading: pageLoading });
const HardwarePage = dynamic(() => import('./components/HardwarePage'), { ssr: false, loading: pageLoading });
const HealthPage = dynamic(() => import('./components/HealthPage'), { ssr: false, loading: pageLoading });
const NetworkPage = dynamic(() => import('./components/NetworkPage'), { ssr: false, loading: pageLoading });
const SettingsPage = dynamic(() => import('./components/SettingsPage'), { ssr: false, loading: pageLoading });
const AboutPanel = dynamic(() => import('./components/AboutPanel'), { ssr: false, loading: pageLoading });

export default function Home() {
  useAppBootstrap();

  const view = useAppStore(
    useShallow((state) => ({
      bridgeOk: state.bridgeOk,
      isLoading: state.isLoading,
      error: state.error,
      activeTab: state.activeTab,
    })),
  );

  const initializeApp = useAppStore((state) => state.initializeApp);
  const setActiveTab = useAppStore((state) => state.setActiveTab);
  const clearBridgeWarning = useAppStore((state) => state.clearBridgeWarning);

  // 首帧渲染完成后显示窗口（由后端按启动参数决定是否展开），避免纯黑闪烁。
  const markUiReady = useCallback(async () => {
    try {
      await apiService.markUiReady();
    } catch {
      /* 忽略：窗口未显示时后端会自行处理 */
    }
  }, []);

  useEffect(() => {
    if (view.bridgeOk && !view.isLoading) {
      // AppShell 渲染完成后应用保存的整体缩放（字体/页面等比，顶栏不参与缩放）。
      applyUiZoom(loadUiZoom());
      void markUiReady();
    }
  }, [view.bridgeOk, view.isLoading, markUiReady]);

  if (view.isLoading) {
    return <AppLoadingSkeleton />;
  }

  if (view.error && !view.bridgeOk) {
    return <AppFatalError message={view.error} onRetry={initializeApp} />;
  }

  return (
    <>
      <AppShell
        activeTab={view.activeTab}
        onTabChange={setActiveTab}
        isMonitoring={view.bridgeOk}
        error={view.error}
        bridgeWarning={null}
        onDismissBridgeWarning={clearBridgeWarning}
        dashboardContent={<DashboardPage />}
        usageContent={<AppUsagePage />}
        calendarContent={<CalendarPage />}
        networkContent={<NetworkPage />}
        healthContent={<HealthPage />}
        hardwareContent={<HardwarePage />}
        settingsContent={<SettingsPage />}
        aboutContent={<AboutPanel />}
      />
      <TakeoverDialog />
    </>
  );
}
