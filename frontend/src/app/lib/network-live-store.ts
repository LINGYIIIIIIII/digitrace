// 网络实时数据共享单例：仪表盘多张卡 + 网络页共用同一个 1s 级轮询器。
// 之前每个消费组件各自轮询（快照 + 300 点窗口），重复请求造成渲染进程
// 频繁 JSON 解析与 GC——合并后内存/CPU 都显著下降，且各页面显示天然一致。
import { useSyncExternalStore } from 'react';
import { apiService } from '../services/api';
import { useAppStore } from '../store/app-store';
import type { NetworkSnapshotDto } from '../types';

export interface LivePoint {
  t: string;
  down: number;
  up: number;
}

interface LiveState {
  snapshot: NetworkSnapshotDto | null;
  points: LivePoint[];
}

let state: LiveState = { snapshot: null, points: [] };
let listeners = new Set<() => void>();
let timer: number | null = null;
let lastFetch = 0;

const pad = (n: number) => String(n).padStart(2, '0');

async function tick() {
  const config = useAppStore.getState().config;
  const windowSeconds = config?.network_live_window_seconds ?? 300;
  try {
    const [snap, samples] = await Promise.all([
      apiService.getNetworkSnapshot(),
      apiService.getNetworkLiveWindow(windowSeconds),
    ]);
    state = {
      snapshot: snap,
      points: samples.map((s) => {
        const d = new Date(s.ts);
        return {
          t: `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`,
          down: s.down,
          up: s.up,
        };
      }),
    };
    emit();
  } catch {
    /* 静默降级 */
  }
}

function emit() {
  listeners.forEach((l) => l());
}

function startPolling() {
  if (timer != null) return;
  const loop = () => {
    const config = useAppStore.getState().config;
    const refreshSeconds = config?.live_refresh_interval_seconds ?? 1;
    const now = Date.now();
    if (now - lastFetch >= refreshSeconds * 1000) {
      lastFetch = now;
      void tick();
    }
  };
  void loop();
  timer = window.setInterval(loop, 1000);
}

function stopPolling() {
  if (timer != null && listeners.size === 0) {
    window.clearInterval(timer);
    timer = null;
  }
}

function subscribe(cb: () => void) {
  listeners.add(cb);
  startPolling();
  return () => {
    listeners.delete(cb);
    stopPolling();
  };
}

function getSnapshot(): LiveState {
  return state;
}

/** 共享实时数据：快照 + 最近窗口曲线（所有消费者共用同一份状态）。 */
export function useNetworkLiveShared() {
  return useSyncExternalStore(subscribe, getSnapshot);
}
