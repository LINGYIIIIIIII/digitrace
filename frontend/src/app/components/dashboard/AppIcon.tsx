'use client';

import { useEffect, useState } from 'react';
import { apiService } from '../../services/api';

// 模块级有界缓存（exePath → dataURL）：应用列表反复挂载/重渲染时
// 避免重复 IPC 提取 + canvas 解码 + base64 字符串（内存与 GC 都省）。
const ICON_CACHE = new Map<string, string>();
const ICON_CACHE_CAP = 120;
// 进行中的请求去重：同一 exe 并发挂载只发一次。
const IN_FLIGHT = new Map<string, Promise<string | null>>();

function cacheIcon(exePath: string, url: string) {
  ICON_CACHE.set(exePath, url);
  if (ICON_CACHE.size > ICON_CACHE_CAP) {
    const first = ICON_CACHE.keys().next().value;
    if (first !== undefined) ICON_CACHE.delete(first);
  }
}

function fetchIcon(exePath: string): Promise<string | null> {
  let p = IN_FLIGHT.get(exePath);
  if (!p) {
    p = (async () => {
      try {
        const icon = await apiService.getAppIcon(exePath);
        if (!icon || icon.rgba.length === 0) return null;
        const canvas = document.createElement('canvas');
        canvas.width = icon.width;
        canvas.height = icon.height;
        const ctx = canvas.getContext('2d');
        if (!ctx) return null;
        const imageData = ctx.createImageData(icon.width, icon.height);
        imageData.data.set(icon.rgba);
        ctx.putImageData(imageData, 0, 0);
        const url = canvas.toDataURL();
        cacheIcon(exePath, url);
        return url;
      } catch {
        return null;
      } finally {
        IN_FLIGHT.delete(exePath);
      }
    })();
    IN_FLIGHT.set(exePath, p);
  }
  return p;
}

/** 从 exe 提取的应用图标（RGBA → dataURL，带缓存），失败时显示占位。 */
export default function AppIcon({ exePath, size = 24 }: { exePath: string; size?: number }) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (!exePath) {
      return;
    }
    const cached = ICON_CACHE.get(exePath);
    if (cached) {
      setSrc(cached);
      return;
    }
    void fetchIcon(exePath).then((url) => {
      if (!cancelled && url) setSrc(url);
    });
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
