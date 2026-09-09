// 硬件实时数据共享单例：仪表盘多张卡与硬件页共用同一轮询器。
// 先发布 CPU/内存快照，再异步补温度，避免慢传感器阻塞首屏。
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

function emit() {
  listeners.forEach((listener) => listener());
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
      state = { ...state, sampleId: state.sampleId + 1 };
      emit();
    }
  } catch {
    state = { ...state, error: true };
    emit();
  } finally {
    inFlight = false;
  }
}

function startPolling() {
  if (timer !== null) return;
  const loop = () => {
    const refreshSeconds = useAppStore.getState().config?.live_refresh_interval_seconds ?? 1;
    const now = Date.now();
    if (now - lastFetch >= refreshSeconds * 1000) {
      lastFetch = now;
      void tick();
    }
  };
  void loop();
  timer = window.setInterval(loop, 250);
}

function stopPolling() {
  if (timer !== null && listeners.size === 0) {
    window.clearInterval(timer);
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

/** 共享硬件快照；sampleId 仅在一轮温度补采完成后递增。 */
export function useHardwareLiveShared() {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
