import { create } from 'zustand';
import { apiService } from '../services/api';
import type { AppConfig } from '../types';

export type ActiveTab =
  | 'dashboard'
  | 'usage'
  | 'calendar'
  | 'network'
  | 'health'
  | 'hardware'
  | 'settings'
  | 'about';

interface AppStore {
  isLoading: boolean;
  error: string | null;
  config: AppConfig | null;
  bridgeOk: boolean;
  activeTab: ActiveTab;

  setActiveTab: (tab: ActiveTab) => void;
  clearBridgeWarning: () => void;
  initializeApp: () => Promise<void>;
  updateConfig: (config: AppConfig) => Promise<void>;
}

export const useAppStore = create<AppStore>((set, get) => ({
  isLoading: true,
  error: null,
  config: null,
  bridgeOk: false,
  activeTab: 'dashboard',

  setActiveTab: (tab) => set({ activeTab: tab }),

  clearBridgeWarning: () => set({ error: null }),

  initializeApp: async () => {
    try {
      const config = await apiService.getConfig();
      set({
        config,
        bridgeOk: true,
        isLoading: false,
        error: null,
      });
    } catch (error) {
      set({
        isLoading: false,
        error: error instanceof Error ? error.message : String(error),
        bridgeOk: false,
      });
    }
  },

  updateConfig: async (config) => {
    await apiService.setConfig(config);
    set({ config });
  },
}));
