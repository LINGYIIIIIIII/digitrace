'use client';

import { useEffect, useState } from 'react';
import { apiService } from '../../services/api';

/** 从 exe 提取的应用图标（RGBA → dataURL），失败时显示占位。 */
export default function AppIcon({ exePath, size = 24 }: { exePath: string; size?: number }) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (!exePath) {
      return;
    }
    apiService
      .getAppIcon(exePath)
      .then((icon) => {
        if (cancelled || !icon || icon.rgba.length === 0) {
          return;
        }
        const canvas = document.createElement('canvas');
        canvas.width = icon.width;
        canvas.height = icon.height;
        const ctx = canvas.getContext('2d');
        if (!ctx) {
          return;
        }
        const imageData = ctx.createImageData(icon.width, icon.height);
        imageData.data.set(icon.rgba);
        ctx.putImageData(imageData, 0, 0);
        setSrc(canvas.toDataURL());
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [exePath]);

  if (!src) {
    return (
      <span
        className="inline-block shrink-0 rounded-md border border-border/60 bg-background/70"
        style={{ width: size, height: size }}
      />
    );
  }
  return (
    <img
      src={src}
      width={size}
      height={size}
      alt=""
      draggable={false}
      className="shrink-0 rounded-md"
    />
  );
}
