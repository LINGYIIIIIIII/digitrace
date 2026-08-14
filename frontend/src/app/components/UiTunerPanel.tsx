'use client';

import { useEffect, useState } from 'react';
import { RotateCcw, SlidersHorizontal, X } from 'lucide-react';

const TUNER_KEY = 'digitrace.ui-tuner';
const ENABLED_KEY = 'digitrace.ui-tuner.enabled';

interface TunerVals {
  radius: number;
  gap: number;
  pad: number;
  titleSize: number;
}

const DEFAULTS: TunerVals = { radius: 16, gap: 16, pad: 12, titleSize: 14 };

function applyVars(v: TunerVals) {
  const r = document.documentElement.style;
  r.setProperty('--ui-radius', `${v.radius}px`);
  r.setProperty('--ui-gap', `${v.gap}px`);
  r.setProperty('--ui-card-pad', `${v.pad}px`);
  r.setProperty('--ui-title-size', `${v.titleSize}px`);
}

function loadVals(): TunerVals {
  try {
    const raw = JSON.parse(window.localStorage.getItem(TUNER_KEY) ?? 'null');
    return { ...DEFAULTS, ...raw };
  } catch {
    return DEFAULTS;
  }
}

/**
 * UI 调试面板：实时调整卡片圆角 / 间距 / 内边距 / 标题字号。
 * 由「设置 → 外观 → UI 调试面板」开关控制显隐；调整结果存本机，
 * 把满意的数值告诉开发者即可固化进代码。
 */
export default function UiTunerPanel() {
  const [enabled, setEnabled] = useState(false);
  const [open, setOpen] = useState(false);
  const [vals, setVals] = useState<TunerVals>(DEFAULTS);

  useEffect(() => {
    setEnabled(window.localStorage.getItem(ENABLED_KEY) === '1');
    setVals(loadVals());
    applyVars(loadVals());
    const onToggle = () => setEnabled(window.localStorage.getItem(ENABLED_KEY) === '1');
    window.addEventListener('ui-tuner-toggle', onToggle);
    return () => window.removeEventListener('ui-tuner-toggle', onToggle);
  }, []);

  const update = (key: keyof TunerVals, value: number) => {
    setVals((prev) => {
      const next = { ...prev, [key]: value };
      window.localStorage.setItem(TUNER_KEY, JSON.stringify(next));
      applyVars(next);
      return next;
    });
  };

  if (!enabled) return null;

  const sliders = [
    { key: 'radius' as const, label: '卡片圆角', min: 4, max: 28, unit: 'px' },
    { key: 'gap' as const, label: '卡片间距', min: 4, max: 32, unit: 'px' },
    { key: 'pad' as const, label: '卡片内边距', min: 4, max: 28, unit: 'px' },
    { key: 'titleSize' as const, label: '标题字号', min: 11, max: 20, unit: 'px' },
  ];

  return (
    <div className="fixed bottom-4 right-4 z-[999]">
      {open && (
        <div className="mb-2 w-64 rounded-2xl border border-border/70 bg-card/95 p-3 shadow-xl backdrop-blur">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-sm font-semibold">UI 调试</span>
            <button type="button" onClick={() => setOpen(false)} className="text-muted-foreground hover:text-foreground">
              <X className="h-4 w-4" />
            </button>
          </div>
          <div className="space-y-2.5">
            {sliders.map((s) => (
              <div key={s.key}>
                <div className="mb-0.5 flex justify-between text-[11px] text-muted-foreground">
                  <span>{s.label}</span>
                  <span className="tabular-nums">
                    {vals[s.key]}
                    {s.unit}
                  </span>
                </div>
                <input
                  type="range"
                  min={s.min}
                  max={s.max}
                  value={vals[s.key]}
                  onChange={(e) => update(s.key, Number(e.target.value))}
                  className="w-full accent-primary"
                />
              </div>
            ))}
          </div>
          <button
            type="button"
            className="mt-2 flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground"
            onClick={() => {
              window.localStorage.setItem(TUNER_KEY, JSON.stringify(DEFAULTS));
              setVals(DEFAULTS);
              applyVars(DEFAULTS);
            }}
          >
            <RotateCcw className="h-3 w-3" />
            恢复默认
          </button>
        </div>
      )}
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="flex h-9 w-9 items-center justify-center rounded-full border border-border/70 bg-card/95 text-muted-foreground shadow-lg backdrop-blur hover:text-foreground"
        title="UI 调试"
      >
        <SlidersHorizontal className="h-4 w-4" />
      </button>
    </div>
  );
}
