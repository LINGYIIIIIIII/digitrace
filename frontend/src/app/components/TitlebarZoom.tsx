'use client';

import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check, ZoomIn } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { applyUiZoom, loadUiZoom, previewUiZoom } from '../lib/appearance';
import { Slider } from '@/components/ui/slider';
import { Input } from '@/components/ui/input';
import { Select } from './ui/index';

const NO_DRAG = { 'data-tauri-drag-region': 'false' } as const;
const PRESETS = [75, 100, 125, 150];

/**
 * 顶栏缩放：常驻在标题栏（不随内容缩放变化），点击弹出面板。
 * 调整时实时预览，点「确定」才保存生效，点「取消」还原——像 Windows 一样。
 */
export default function TitlebarZoom() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [saved, setSaved] = useState(() => (typeof window !== 'undefined' ? loadUiZoom() : 100));
  const [preview, setPreview] = useState<number | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  const shown = preview ?? saved;

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      const insidePanel =
        (rootRef.current && rootRef.current.contains(t)) ||
        (panelRef.current && panelRef.current.contains(t));
      if (!insidePanel) {
        // Radix Select 的下拉渲染在 body 的 popper 里：点它不应被当成「点面板外」而还原。
        if (t instanceof Element && t.closest('[data-radix-popper-content-wrapper], [role="listbox"]')) {
          return;
        }
        cancel();
      }
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const applyPreview = (v: number) => {
    setPreview(v);
    previewUiZoom(v);
  };

  const confirm = () => {
    if (preview != null) {
      setSaved(preview);
      applyUiZoom(preview);
    }
    setPreview(null);
    setOpen(false);
  };

  const cancel = () => {
    if (preview != null) {
      previewUiZoom(saved);
    }
    setPreview(null);
    setOpen(false);
  };

  return (
    <div ref={rootRef} className="relative flex items-center" {...NO_DRAG}>
      <button
        type="button"
        onClick={() => {
          setPreview(null);
          setOpen((o) => !o);
        }}
        className="flex h-8 items-center gap-1 rounded-md px-2 text-[11px] font-medium tabular-nums text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground"
        title={t('settings.zoom.title')}
      >
        <ZoomIn className="h-3.5 w-3.5" />
        {shown}%
      </button>

      {open && (
        createPortal(
          <div
            ref={panelRef}
            className="fixed right-2 top-12 z-[10000] w-64 rounded-xl border border-border/80 bg-[#161b23]/95 p-3 shadow-2xl ring-1 ring-black/30"
          >
          <div className="mb-2 flex items-center justify-between">
            <span className="text-xs font-semibold">{t('settings.zoom.title')}</span>
            <span className="text-xs font-semibold tabular-nums">{shown}%</span>
          </div>

          <Slider
            value={[shown]}
            min={75}
            max={175}
            step={1}
            onValueChange={(v) => applyPreview(v[0])}
          />

          <div className="mt-2 flex items-center gap-1.5">
            <Input
              type="number"
              min={75}
              max={175}
              value={String(shown)}
              onChange={(e) => {
                const n = Number(e.target.value);
                if (n >= 75 && n <= 175) applyPreview(n);
              }}
              className="h-8 w-16 text-center"
            />
            <span className="text-xs text-muted-foreground">%</span>
            <Select
              value={String(shown)}
              onChange={(v) => applyPreview(Number(v))}
              options={PRESETS.map((n) => ({ value: String(n), label: `${n}%` }))}
              size="sm"
              className="ml-auto w-20"
              triggerClassName="h-8"
              contentClassName="z-[10001]"
            />
          </div>

          <div className="mt-3 flex justify-end gap-1.5">
            <button
              type="button"
              onClick={cancel}
              className="rounded-md px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            >
              {t('common.cancel')}
            </button>
            <button
              type="button"
              onClick={confirm}
              className="flex items-center gap-1 rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
            >
              <Check className="h-3 w-3" />
              {t('common.save')}
            </button>
          </div>
          </div>,
          document.body,
        )
      )}
    </div>
  );
}
