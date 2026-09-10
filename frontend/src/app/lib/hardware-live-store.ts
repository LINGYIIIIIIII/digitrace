// 硬件实时数据共享单例：仪表盘多张卡与硬件页共用同一轮询器。
// 先发布 CPU/内存快照，再异步补温度，避免慢传感器阻塞首屏。
// 调度：链式 setTimeout（按 live_refresh 对齐），页面 hidden 时暂停。
import { useSyncExternalStore } from 'react';
import { apiService } from '../services/api';
import { useAppStore } from '../store/app-store';
import type { HardwareSnapshotDto, TemperatureSnapshotDto } from '../types';

interface HardwareLiveState {
  snapshot: HardwareSnapshotDto | null;
  temp: TemperatureSnapshotDto | null;
  error: boolean;
  sampleId: number;
}

let state: HardwareLiveState = {
  snapshot: null,
  temp: null,
  error: false,
  sampleId: 0,
};
const listeners = new Set<() => void>();
let timer: number | null = null;
let inFlight = false;
let lastFetch = 0;
let visibilityBound = false;

function emit() {
  listeners.forEach((listener) => listener());
}

function refreshSeconds(): number {
  return useAppStore.getState().config?.live_refresh_interval_seconds ?? 1;
}

async function tick() {
  if (inFlight) return;
  inFlight = true;
  try {
    // 先返回 sysinfo 快照，CPU/内存仪表盘可以立即绘制。
    const snapshot = await apiService.getHardwareSnapshot();
    state = { ...state, snapshot, error: false };
    emit();

    // 温度可能触发 nvidia-smi、PawnIO 或磁盘传感器回退，单独等待，
    // 不让它阻塞上面的首屏数据。
    try {
      const temp = await apiService.getTemperatureSnapshot();
      state = { ...state, temp, sampleId: state.sampleId + 1 };
      emit();
    } catch {
      // 温度失败：保留旧 temp，不 bump sampleId，避免无意义重绘。
    }
  } catch {
    state = { ...state, error: true };
    emit();
  } finally {
    inFlight = false;
    lastFetch = Date.now();
  }
}

function scheduleNext() {
  if (timer !== null) window.clearTimeout(timer);
  if (listeners.size === 0) return;
  if (typeof document !== 'undefined' && document.hidden) return;
  const gap = refreshSeconds() * 1000;
  const wait = Math.max(0, gap - (Date.now() - lastFetch));
  timer = window.setTimeout(async () => {
    timer = null;
    if (listeners.size === 0) return;
    if (typeof document !== 'undefined' && document.hidden) return;
    await tick();
    scheduleNext();
  }, wait);
}

function onVisibility() {
  if (typeof document === 'undefined') return;
  if (document.hidden) {
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
    return;
  }
  if (listeners.size > 0) {
    void tick().finally(scheduleNext);
  }
}

function bindVisibility() {
  if (visibilityBound || typeof document === 'undefined') return;
  document.addEventListener('visibilitychange', onVisibility);
  visibilityBound = true;
}

function startPolling() {
  bindVisibility();
  if (timer !== null || inFlight) return;
  if (typeof document !== 'undefined' && document.hidden) return;
  void tick().finally(scheduleNext);
}

function stopPolling() {
  if (listeners.size > 0) return;
  if (timer !== null) {
    window.clearTimeout(timer);
    timer = null;
  }
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  startPolling();
  return () => {
    listeners.delete(listener);
    stopPolling();
  };
}

function getSnapshot() {
  return state;
}

/** 共享硬件快照；sampleId 仅在温度补采成功后递增。 */
export function useHardwareLiveShared() {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
