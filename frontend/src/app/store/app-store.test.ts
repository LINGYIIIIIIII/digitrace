import { beforeEach, describe, expect, it, vi } from 'vitest';

// 用 mock 替换 apiService，避免引入 @tauri-apps 依赖。
vi.mock('../services/api', () => ({
  apiService: {
    getConfig: vi.fn(),
    setConfig: vi.fn(),
  },
}));

import { useAppStore } from './app-store';

describe('app-store 启动流程', () => {
  beforeEach(() => {
    useAppStore.setState({
      isLoading: true,
      bridgeOk: false,
      error: null,
      config: null,
      activeTab: 'dashboard',
    });
    vi.clearAllMocks();
  });

  it('初始化成功：写入配置并标记桥接可用', async () => {
    const { apiService } = await import('../services/api');
    (apiService.getConfig as ReturnType<typeof vi.fn>).mockResolvedValue({
      poll_interval_ms: 3000,
      update_github_repo: 'LINGYIIIIIIII/digitrace',
    });

    await useAppStore.getState().initializeApp();

    const s = useAppStore.getState();
    expect(s.bridgeOk).toBe(true);
    expect(s.isLoading).toBe(false);
    expect(s.error).toBeNull();
    expect(s.config?.poll_interval_ms).toBe(3000);
  });

  it('初始化失败：记录错误并保持桥接不可用', async () => {
    const { apiService } = await import('../services/api');
    (apiService.getConfig as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error('bridge down'),
    );

    await useAppStore.getState().initializeApp();

    const s = useAppStore.getState();
    expect(s.bridgeOk).toBe(false);
    expect(s.isLoading).toBe(false);
    expect(s.error).toBe('bridge down');
  });

  it('updateConfig 成功后同步本地配置', async () => {
    const { apiService } = await import('../services/api');
    const cfg = { poll_interval_ms: 5000, update_silent: true } as never;
    await useAppStore.getState().updateConfig(cfg as never);

    expect(useAppStore.getState().config).toBe(cfg);
    expect(apiService.setConfig).toHaveBeenCalledWith(cfg);
  });
});
