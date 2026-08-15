// 仪表盘布局模型（v4 · 九宫格单元网格）
// - 网格：3 列，行高统一（--tile-h ≈ 列宽），grid-auto-flow: row dense 自动回填空洞
// - 卡片尺寸 = 列 × 行 单元数，7 档：
//     1x1 一格 | 1x2 竖条 | 2x1 横条 | 2x2 标准 | 3x1 整行窄 | 3x2 整行高 | 3x3 整屏
// - 聚合卡与组成小卡互斥：启用聚合卡会自动隐藏其成员卡

export type CardSize = '1x1' | '1x2' | '2x1' | '2x2' | '3x1' | '3x2' | '3x3';
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
  version: 4;
  template: TemplateId;
  cards: CardStates;
  order: CardId[];
}

export const LAYOUT_KEY = 'digitrace.dashboard.layout';
export const LAYOUT_VERSION = 4;

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

/** 尺寸 → 列数 / 行数。 */
export const SIZE_COLS: Record<CardSize, number> = {
  '1x1': 1,
  '1x2': 1,
  '2x1': 2,
  '2x2': 2,
  '3x1': 3,
  '3x2': 3,
  '3x3': 3,
};
export const SIZE_ROWS: Record<CardSize, number> = {
  '1x1': 1,
  '1x2': 2,
  '2x1': 1,
  '2x2': 2,
  '3x1': 1,
  '3x2': 2,
  '3x3': 3,
};

/** 内容密度档（供卡片组件复用现有 sm/md/lg 内部布局）。 */
export type Density = 'sm' | 'md' | 'lg';
export function densityOf(size: CardSize): Density {
  if (size === '1x1') return 'sm';
  if (size === '1x2' || size === '2x1' || size === '2x2') return 'md';
  return 'lg';
}

/**
 * 位移吸附：水平/垂直各自独立计步。
 * - 死区 0.25 格（防止轻微抖动误触）；
 * - 之后每拖满 1 格换一档，可连续跨档（1x1 向右拖 2 格直达 3x1）。
 */
export function snapDelta(delta: number, stride: number): number {
  const dead = stride * 0.25;
  if (delta > dead) return Math.floor((delta - dead) / stride) + 1;
  if (delta < -dead) return -(Math.floor((-delta - dead) / stride) + 1);
  return 0;
}

/**
 * 列 × 行 → 合法档位。越界收敛到尺寸表：
 * - 列夹在 1..3；
 * - 列 < 3 时最高 2 行（2x3 / 1x3 不在档位表：竖长只提供 1x2，整行宽才允许 3 行）。
 */
export function sizeFor(cols: number, rows: number): CardSize {
  const c = Math.min(3, Math.max(1, Math.round(cols)));
  const r = Math.min(c === 3 ? 3 : 2, Math.max(1, Math.round(rows)));
  return `${c}x${r}` as CardSize;
}

/**
 * 手柄拖拽吸附：起点档位 + 水平/垂直位移（px）→ 目标档位。
 * 左右拖改变列数、上下拖改变行数，取代旧的固定顺序循环（SIZE_CYCLE）。
 */
export function resizeByDelta(
  start: CardSize,
  dx: number,
  dy: number,
  strideX: number,
  strideY: number,
): CardSize {
  return sizeFor(
    SIZE_COLS[start] + snapDelta(dx, strideX),
    SIZE_ROWS[start] + snapDelta(dy, strideY),
  );
}

const DEFAULT_SIZES: Record<CardId, CardSize> = {
  stats: '2x1',
  appUsage: '2x2',
  hourly: '2x1',
  calendar: '1x2',
  networkStats: '2x2',
  networkLive: '2x1',
  netApps: '2x2',
  attrUsage: '2x2',
  hardwareGauges: '2x2',
  diskTemp: '1x1',
  health: '1x1',
  durationAgg: '3x2',
  netAgg: '3x2',
  hwAgg: '3x2',
  tempAgg: '3x2',
};

interface TemplatePreset {
  order: CardId[];
  sizes: Partial<Record<CardId, CardSize>>;
  visible: CardId[];
}

export const TEMPLATE_IDS: TemplateId[] = ['balanced', 'compact', 'showcase', 'classic'];

/** 每套模板都是「完整矩形」：单元总数 = 3 的倍数 × 整数行，无洞无溢。 */
const TEMPLATES: Record<TemplateId, TemplatePreset> = {
  // 均衡型（Bento 风）：整行大卡 + 2:1/1:2 穿插，行行填满
  balanced: {
    order: ['durationAgg', 'appUsage', 'calendar', 'hwAgg', 'netAgg', 'networkLive', 'health'],
    sizes: {
      durationAgg: '3x2',
      appUsage: '2x2',
      calendar: '1x2',
      hwAgg: '3x2',
      netAgg: '3x2',
      networkLive: '2x1',
      health: '1x1',
    },
    visible: ['durationAgg', 'appUsage', 'calendar', 'hwAgg', 'netAgg', 'networkLive', 'health'],
  },
  // 紧凑型：信息密度最高，小格 + 标准格填满
  compact: {
    order: [
      'stats',
      'health',
      'appUsage',
      'calendar',
      'hourly',
      'diskTemp',
      'networkStats',
      'networkLive',
      'netApps',
    ],
    sizes: {
      stats: '2x1',
      health: '1x1',
      appUsage: '2x2',
      calendar: '1x2',
      hourly: '2x1',
      diskTemp: '1x1',
      networkStats: '2x2',
      networkLive: '1x2',
      netApps: '3x1',
    },
    visible: [
      'stats',
      'health',
      'appUsage',
      'calendar',
      'hourly',
      'diskTemp',
      'networkStats',
      'networkLive',
      'netApps',
    ],
  },
  // 展示型：每行一张大卡，聚焦单个数据
  showcase: {
    order: ['durationAgg', 'calendar', 'appUsage', 'netAgg', 'hwAgg'],
    sizes: {
      durationAgg: '3x2',
      calendar: '3x2',
      appUsage: '3x2',
      netAgg: '3x2',
      hwAgg: '3x2',
    },
    visible: ['durationAgg', 'calendar', 'appUsage', 'netAgg', 'hwAgg'],
  },
  // 经典型：左 1 列竖条 + 右 2 列堆叠（贴近旧版布局）
  classic: {
    order: ['calendar', 'appUsage', 'stats', 'hourly', 'health', 'netApps'],
    sizes: {
      calendar: '1x2',
      appUsage: '2x2',
      stats: '3x1',
      hourly: '2x2',
      health: '1x2',
      netApps: '3x2',
    },
    visible: ['calendar', 'appUsage', 'stats', 'hourly', 'health', 'netApps'],
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

/** 网格 span 类（3 列 × 统一行高）。用字面量映射，避免 Tailwind JIT purge 掉动态类名。 */
const SPAN_CLASSES: Record<CardSize, string> = {
  '1x1': 'col-span-1 row-span-1',
  '1x2': 'col-span-1 row-span-2',
  '2x1': 'col-span-2 row-span-1',
  '2x2': 'col-span-2 row-span-2',
  '3x1': 'col-span-3 row-span-1',
  '3x2': 'col-span-3 row-span-2',
  '3x3': 'col-span-3 row-span-3',
};

export function spanClass(size: CardSize): string {
  return SPAN_CLASSES[size];
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

/** 旧档位（v3 sm/md/lg）→ 新单元尺寸。 */
function migrateSize(old: string | undefined): CardSize {
  if (old === 'sm') return '1x1';
  if (old === 'lg') return '3x2';
  return '2x2';
}

function normalizeCurrent(raw: unknown): DashboardLayout | null {
  const obj = raw as Partial<DashboardLayout> | null;
  if (!obj || !obj.cards || !Array.isArray(obj.order)) return null;
  const cards = {} as CardStates;
  for (const id of ALL_CARD_IDS) {
    const st = (obj.cards as Partial<CardStates>)[id];
    const size: CardSize =
      st && SIZE_COLS[st.size as CardSize] ? (st.size as CardSize) : DEFAULT_SIZES[id];
    cards[id] = { size, visible: st ? !!st.visible : false };
  }
  const order = [...(obj.order as CardId[])];
  for (const id of ALL_CARD_IDS) {
    if (!order.includes(id)) order.push(id);
  }
  return { version: LAYOUT_VERSION, template: 'balanced', cards, order };
}

/**
 * 读取已保存布局。
 * - 无历史布局 → 均衡型默认；
 * - v1/v2/v3 旧布局 → 迁移到均衡型：尺寸按旧档位映射，保留「显示/隐藏」与顺序；
 * - 当前版本（v4）→ 原样还原（含尺寸与顺序）。
 */
export function loadLayout(): DashboardLayout {
  try {
    const raw = JSON.parse(window.localStorage.getItem(LAYOUT_KEY) ?? 'null') as unknown;
    const obj = raw as
      | { version?: number; cards?: unknown; show?: unknown; order?: unknown }
      | null;
    if (obj && typeof obj === 'object') {
      if (obj.version === LAYOUT_VERSION && obj.cards && Array.isArray(obj.order)) {
        const current = normalizeCurrent(raw);
        if (current) return current;
      }
      // 旧版布局（v1 show/order、v2/v3 cards）：迁移到均衡型，保留勾选与顺序。
      const visible: Partial<Record<CardId, boolean>> = {};
      const sizeMap: Partial<Record<CardId, CardSize>> = {};
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
          if (st) {
            if (typeof st.visible === 'boolean') visible[id] = st.visible;
            // v3 的 sm/md/lg 映射到新档位；v4 尺寸直接保留。
            const sz = st.size as string | undefined;
            if (sz && (SIZE_COLS[sz as CardSize] || ['sm', 'md', 'lg'].includes(sz))) {
              sizeMap[id] = SIZE_COLS[sz as CardSize] ? (sz as CardSize) : migrateSize(sz);
            }
          }
        }
      }
      const base = resolveTemplate('balanced');
      const merged = { ...base.cards };
      const order = Array.isArray(obj.order)
        ? [...(obj.order as CardId[])]
        : [...base.order];
      for (const id of ALL_CARD_IDS) {
        if (visible[id] !== undefined) merged[id] = { ...merged[id], visible: visible[id] };
        if (sizeMap[id] !== undefined) merged[id] = { ...merged[id], size: sizeMap[id] };
      }
      for (const id of ALL_CARD_IDS) {
        if (!order.includes(id)) order.push(id);
      }
      return { version: LAYOUT_VERSION, template: 'balanced', cards: merged, order };
    }
  } catch {
    /* 损坏数据回退默认 */
  }
  return resolveTemplate('balanced');
}

export function saveLayout(layout: DashboardLayout): void {
  try {
    window.localStorage.setItem(LAYOUT_KEY, JSON.stringify(layout));
  } catch {
    /* 持久化失败时仅本次会话生效 */
  }
}
