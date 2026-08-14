'use client';

import { useEffect } from 'react';

/**
 * 系统主题自动跟随：仅当用户未手动指定明暗时，跟随系统偏好切换 .dark。
 */
export default function SystemThemeSync() {
  useEffect(() => {
    const STORAGE_KEY = 'digitrace-theme-mode';
    const mq = window.matchMedia('(prefers-color-scheme: dark)');

    const apply = () => {
      const stored = window.localStorage.getItem(STORAGE_KEY);
      if (stored === 'light' || stored === 'dark') {
        return;
      }
      document.documentElement.classList.toggle('dark', mq.matches);
    };

    apply();
    mq.addEventListener('change', apply);
    return () => mq.removeEventListener('change', apply);
  }, []);

  return null;
}
