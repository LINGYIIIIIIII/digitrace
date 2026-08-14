// 仪表盘布局模型（v3）
// - 三档尺寸：小=1/3 列、中=2/3 列、大=整行
// - 布局固定为「紧凑型」：三列小中卡为主，信息密度最高
// - 聚合卡与组成小卡互斥：启用聚合卡会自动隐藏其成员卡

export type CardSize = 'sm' | 'md' | 'lg';
export type TemplateId = 'balanced' | 'compact' | 'showcase' | 'classic';

export type CardId =
  | 'stats'
  | 'appUsage'
  | 'hourly'
  | 'calendar'
  | 'networkStats'
  | 'networkLive'
  | 'netApps'
  | 'attrUsage'
  | 'hardwareGauges'
  | 'diskTemp'
  | 'health'
  | 'durationAgg'
  | 'netAgg'
  | 'hwAgg'
  | 'tempAgg';

export type AggregateId = Extract<CardId, 'durationAgg' | 'netAgg' | 'hwAgg' | 'tempAgg'>;

export interface CardState {
  size: CardSize;
  visible: boolean;
}

export type CardStates = Record<CardId, CardState>;

export interface DashboardLayout {
  version: 3;
  template: TemplateId;
  cards: CardStates;
  order: CardId[];
}

export const LAYOUT_KEY = 'digitrace.dashboard.layout';
export const LAYOUT_VERSION = 3;

export const ALL_CARD_IDS: CardId[] = [
  'stats',
  'appUsage',
  'hourly',
  'calendar',
  'networkStats',
  'networkLive',
  'netApps',
  'attrUsage',
  'hardwareGauges',
  'diskTemp',
  'health',
  'durationAgg',
  'netAgg',
  'hwAgg',
  'tempAgg',
];

export const AGGREGATE_IDS: AggregateId[] = ['durationAgg', 'netAgg', 'hwAgg', 'tempAgg'];

/** 每个聚合卡由哪些单独小卡组成（启用聚合卡时自动隐藏它们）。 */
export const AGGREGATE_MEMBERS: Record<AggregateId, CardId[]> = {
  durationAgg: ['stats', 'hourly'],
  netAgg: ['networkStats', 'networkLive'],
  hwAgg: ['hardwareGauges', 'diskTemp'],
  tempAgg: ['hardwareGauges', 'diskTemp'],
};

/** 内容重叠的聚合卡之间互斥（硬件聚合与温度聚合二选一）。 */
export const AGGREGATE_CONFLICTS: Partial<Record<AggregateId, AggregateId[]>> = {
  hwAgg: ['tempAgg'],
  tempAgg: ['hwAgg'],
};

const DEFAULT_SIZES: Record<CardId, CardSize> = {
  stats: 'md',
  appUsage: 'md',
  hourly: 'md',
  calendar: 'md',
  networkStats: 'sm',
  networkLive: 'md',
  netApps: 'md',
  attrUsage: 'lg',
  hardwareGauges: 'md',
  diskTemp: 'sm',
  health: 'sm',
  durationAgg: 'lg',
  netAgg: 'lg',
  hwAgg: 'lg',
  tempAgg: 'lg',
};

interface TemplatePreset {
  order: CardId[];
  sizes: Partial<Record<CardId, CardSize>>;
  visible: CardId[];
}

export const TEMPLATE_IDS: TemplateId[] = ['balanced', 'compact', 'showcase', 'classic'];

const TEMPLATES: Record<TemplateId, TemplatePreset> = {
  // 均衡型（Bento 风）：大卡带动节奏，小卡凑行
  balanced: {
    order: ['durationAgg', 'calendar', 'health', 'appUsage', 'hwAgg', 'netAgg'],
    sizes: {
      durationAgg: 'lg',
      calendar: 'md',
      health: 'sm',
      appUsage: 'md',
      hwAgg: 'sm',
      netAgg: 'md',
    },
    visible: ['durationAgg', 'calendar', 'health', 'appUsage', 'hwAgg', 'netAgg'],
  },
  // 紧凑型：三列小中卡为主，信息密度最高
  compact: {
    order: ['stats', 'networkStats', 'health', 'appUsage', 'calendar', 'hourly', 'netApps', 'hardwareGauges'],
    sizes: {
      stats: 'sm',
      networkStats: 'sm',
      health: 'sm',
      appUsage: 'md',
      calendar: 'sm',
      hourly: 'sm',
      netApps: 'sm',
      hardwareGauges: 'lg',
    },
    visible: ['stats', 'networkStats', 'health', 'appUsage', 'calendar', 'hourly', 'netApps', 'hardwareGauges'],
  },
  // 展示型：每行一张大卡，聚焦单个数据
  showcase: {
    order: ['durationAgg', 'calendar', 'appUsage', 'netAgg', 'hwAgg'],
    sizes: {
      durationAgg: 'lg',
      calendar: 'lg',
      appUsage: 'lg',
      netAgg: 'lg',
      hwAgg: 'lg',
    },
    visible: ['durationAgg', 'calendar', 'appUsage', 'netAgg', 'hwAgg'],
  },
  // 经典型：左 1/3 日历 + 右 2/3 堆叠（贴近旧版布局）
  classic: {
    order: ['stats', 'calendar', 'appUsage', 'netApps', 'hourly'],
    sizes: {
      stats: 'lg',
      calendar: 'sm',
      appUsage: 'md',
      netApps: 'md',
      hourly: 'md',
    },
    visible: ['stats', 'calendar', 'appUsage', 'netApps', 'hourly'],
  },
};

export function resolveTemplate(template: TemplateId): DashboardLayout {
  const preset = TEMPLATES[template];
  const cards = {} as CardStates;
  for (const id of ALL_CARD_IDS) {
    cards[id] = {
      size: preset.sizes[id] ?? DEFAULT_SIZES[id],
      visible: preset.visible.includes(id),
    };
  }
  return { version: LAYOUT_VERSION, template, cards, order: [...preset.order] };
}

export function spanClass(size: CardSize): string {
  if (size === 'lg') return 'md:col-span-3';
  if (size === 'md') return 'md:col-span-2';
  return 'md:col-span-1';
}

export function isAggregate(id: CardId): id is AggregateId {
  return AGGREGATE_IDS.includes(id as AggregateId);
}

/**
 * 切换某张卡的显示/隐藏，并自动处理聚合冲突：
 * - 启用聚合卡 → 隐藏其成员卡 + 冲突聚合卡
 * - 启用成员卡 → 隐藏包含它的聚合卡
 */
export function toggleCard(prev: DashboardLayout, id: CardId, visible: boolean): DashboardLayout {
  const cards: CardStates = { ...prev.cards, [id]: { ...prev.cards[id], visible } };
  if (visible) {
    if (isAggregate(id)) {
      for (const member of AGGREGATE_MEMBERS[id]) {
        cards[member] = { ...cards[member], visible: false };
      }
      for (const conflict of AGGREGATE_CONFLICTS[id] ?? []) {
        cards[conflict] = { ...cards[conflict], visible: false };
      }
    }
    for (const aggId of AGGREGATE_IDS) {
      if (id !== aggId && AGGREGATE_MEMBERS[aggId].includes(id)) {
        cards[aggId] = { ...cards[aggId], visible: false };
      }
    }
  }
  return { ...prev, cards };
}

export function setCardSize(prev: DashboardLayout, id: CardId, size: CardSize): DashboardLayout {
  return { ...prev, cards: { ...prev.cards, [id]: { ...prev.cards[id], size } } };
}

export function moveCard(prev: DashboardLayout, id: CardId, dir: -1 | 1): DashboardLayout {
  const order = [...prev.order];
  const idx = order.indexOf(id);
  const target = idx + dir;
  if (idx < 0 || target < 0 || target >= order.length) return prev;
  [order[idx], order[target]] = [order[target], order[idx]];
  return { ...prev, order };
}

/** 把 moving 移动到 target 之前（缩略图拖拽排序用）。 */
export function insertCardBefore(
  prev: DashboardLayout,
  moving: CardId,
  target: CardId,
): DashboardLayout {
  if (moving === target) return prev;
  const order = prev.order.filter((id) => id !== moving);
  const idx = order.indexOf(target);
  if (idx < 0) return prev;
  order.splice(idx, 0, moving);
  return { ...prev, order };
}

function normalizeCurrent(raw: unknown): DashboardLayout | null {
  const obj = raw as Partial<DashboardLayout> | null;
  if (!obj || !obj.cards || !Array.isArray(obj.order)) return null;
  const cards = {} as CardStates;
  for (const id of ALL_CARD_IDS) {
    const st = (obj.cards as Partial<CardStates>)[id];
    const size: CardSize = st && ['sm', 'md', 'lg'].includes(st.size) ? st.size : DEFAULT_SIZES[id];
    cards[id] = { size, visible: st ? !!st.visible : false };
  }
  const order = [...(obj.order as CardId[])];
  for (const id of ALL_CARD_IDS) {
    if (!order.includes(id)) order.push(id);
  }
  return { version: LAYOUT_VERSION, template: 'compact', cards, order };
}

/**
 * 读取已保存布局。
 * - 无历史布局 → 紧凑型默认；
 * - 旧版布局 → 迁移到紧凑型，仅保留原有「显示/隐藏」勾选（尺寸与顺序用紧凑型默认）；
 * - 当前版本（v3）→ 原样还原（含尺寸与顺序）。
 */
export function loadLayout(): DashboardLayout {
  try {
    const raw = JSON.parse(window.localStorage.getItem(LAYOUT_KEY) ?? 'null') as unknown;
    const obj = raw as { version?: number; cards?: unknown; show?: unknown; order?: unknown } | null;
    if (obj && typeof obj === 'object') {
      if (obj.version === LAYOUT_VERSION && obj.cards && Array.isArray(obj.order)) {
        const current = normalizeCurrent(raw);
        if (current) return current;
      }
      // 旧版布局（v1 show/order 或 v2 cards）：迁移到紧凑型，保留勾选。
      const visible: Partial<Record<CardId, boolean>> = {};
      const show = obj.show as Record<string, boolean> | undefined;
      if (show) {
        for (const id of ALL_CARD_IDS) {
          if (typeof show[id] === 'boolean') visible[id] = show[id];
        }
      }
      const cards = obj.cards as Partial<CardStates> | undefined;
      if (cards) {
        for (const id of ALL_CARD_IDS) {
          const st = cards[id];
          if (st && typeof st.visible === 'boolean') visible[id] = st.visible;
        }
      }
      const base = resolveTemplate('compact');
      const merged = { ...base.cards };
      for (const id of ALL_CARD_IDS) {
        if (visible[id] !== undefined) merged[id] = { ...merged[id], visible: visible[id] };
      }
      return { version: LAYOUT_VERSION, template: 'compact', cards: merged, order: [...base.order] };
    }
  } catch {
    /* 损坏数据回退默认 */
  }
  return resolveTemplate('compact');
}

export function saveLayout(layout: DashboardLayout): void {
  try {
    window.localStorage.setItem(LAYOUT_KEY, JSON.stringify(layout));
  } catch {
    /* 持久化失败时仅本次会话生效 */
  }
}
