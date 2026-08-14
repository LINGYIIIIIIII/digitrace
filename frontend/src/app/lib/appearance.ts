'use client';

/** 应用主题模式：system / light / dark。 */
export function applyThemeMode(mode: string) {
  try {
    window.localStorage.setItem('digitrace.theme-mode', mode);
  } catch {
    /* 持久化失败不影响应用 */
  }
  const root = document.documentElement;
  if (mode === 'dark') {
    root.classList.add('dark');
    return;
  }
  if (mode === 'light') {
    root.classList.remove('dark');
    return;
  }
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  root.classList.toggle('dark', prefersDark);
}

/** 应用界面字体：system / harmonyos / noto / misans。 */
export function applyFontFamily(family: string) {
  const root = document.documentElement;
  if (family === 'harmonyos') {
    root.style.setProperty(
      '--font-ui-cjk',
      '"HarmonyOS Sans SC", "HarmonyOS Sans", "Microsoft YaHei", "PingFang SC", sans-serif',
    );
  } else if (family === 'noto') {
    root.style.setProperty(
      '--font-ui-cjk',
      '"Noto Sans SC", "Source Han Sans SC", "Microsoft YaHei", "PingFang SC", sans-serif',
    );
  } else if (family === 'misans') {
    root.style.setProperty(
      '--font-ui-cjk',
      '"MiSans", "Microsoft YaHei", "PingFang SC", sans-serif',
    );
  } else {
    root.style.removeProperty('--font-ui-cjk');
  }
}

const UI_ZOOM_KEY = 'digitrace.ui-zoom';

/** 读取已保存的界面缩放百分比（75–175，默认 100）。 */
export function loadUiZoom(): number {
  try {
    const v = Number(window.localStorage.getItem(UI_ZOOM_KEY));
    if (Number.isFinite(v) && v >= 75 && v <= 175) return v;
  } catch {
    /* ignore */
  }
  return 100;
}

function setZoomCss(percent: number) {
  // 缩放只作用于内容区（#ui-zoom-root），顶栏保持在窗口顶部、不随缩放变化。
  const target = document.getElementById('ui-zoom-root');
  if (target) {
    target.style.setProperty('zoom', String(percent / 100));
  }
  // 通知缩放指示（顶栏按钮 / 设置页实时跟随）。
  window.dispatchEvent(new CustomEvent('ui-zoom-change', { detail: percent }));
}

/** 保存并应用界面整体缩放（字体 / 卡片 / 图表全部等比缩放，连续可调）。 */
export function applyUiZoom(percent: number) {
  try {
    window.localStorage.setItem(UI_ZOOM_KEY, String(percent));
  } catch {
    /* ignore */
  }
  setZoomCss(percent);
}

/** 仅预览缩放（顶栏面板拖动时实时生效，不写入已保存值）。 */
export function previewUiZoom(percent: number) {
  setZoomCss(percent);
}
