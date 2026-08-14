'use client';

import { useCallback, useEffect } from 'react';
import { useShallow } from 'zustand/react/shallow';
import AppFatalError from './components/AppFatalError';
import AppLoadingSkeleton from './components/AppLoadingSkeleton';
import AppShell from './components/AppShell';
import { apiService } from './services/api';
import { applyUiZoom, loadUiZoom } from './lib/appearance';
import AboutPanel from './components/AboutPanel';
import AppUsagePage from './components/AppUsagePage';
import CalendarPage from './components/CalendarPage';
import HardwarePage from './components/HardwarePage';
import HealthPage from './components/HealthPage';
import NetworkPage from './components/NetworkPage';
import SettingsPage from './components/SettingsPage';
import TakeoverDialog from './components/TakeoverDialog';
import DashboardPage from './components/dashboard/DashboardPage';
import { useAppBootstrap } from './hooks/useAppBootstrap';
import { useAppStore } from './store/app-store';

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
