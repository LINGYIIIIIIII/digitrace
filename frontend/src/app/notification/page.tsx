'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { BellRing } from 'lucide-react';

/**
 * 应用内通知页：由 health.rs 以独立无边框小窗口加载（notification.html?title=..&body=..）。
 * 样式与主界面一致（半透明玻璃拟态 + MiSans 字体），点击后关闭并打开主界面。
 */
export default function NotificationPage() {
  const [title, setTitle] = useState('数迹');
  const [body, setBody] = useState('');

  useEffect(() => {
    // 通知窗口是透明无边框的，确保 html/body 不遮挡透明背景。
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';
    const q = new URLSearchParams(window.location.search);
    setTitle(q.get('title') ?? '数迹');
    setBody(q.get('body') ?? '');
  }, []);

  const handleClick = () => {
    void invoke('dismiss_notification').catch(() => undefined);
  };

  return (
    <div
      className="flex h-screen w-screen items-start justify-end"
      style={{ background: 'transparent' }}
    >
      <button
        type="button"
        onClick={handleClick}
        className="m-3 flex w-[356px] cursor-pointer items-start gap-3 rounded-2xl border border-border/60 bg-card/85 p-4 text-left shadow-xl backdrop-blur-xl transition-transform hover:scale-[1.02]"
        style={{ fontFamily: '"MiSans", "Microsoft YaHei", "PingFang SC", sans-serif' }}
      >
        <span className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-primary/15 text-primary">
          <BellRing className="h-5 w-5" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="text-sm font-semibold text-foreground">{title}</div>
          <p className="mt-1 whitespace-pre-line text-xs leading-relaxed text-muted-foreground">
            {body}
          </p>
        </div>
      </button>
    </div>
  );
}
