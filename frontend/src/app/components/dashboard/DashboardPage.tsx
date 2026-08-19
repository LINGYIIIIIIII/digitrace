'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check, Edit3, Eye, EyeOff, GripHorizontal, RotateCcw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { apiService } from '../../services/api';
import { useAppStore } from '../../store/app-store';
import type { DashboardDataDto } from '../../types';
import { shiftDayStr, todayStr, weekdayOf } from '../../lib/datetime';
import { Button } from '../ui/index';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import NetAppsCard from '../NetAppsCard';
import AttributedUsageCard from '../AttributedUsageCard';
import {
  AppUsageCard,
  CalendarCard,
  DiskTempCard,
  DurationAggCard,
  GamesCard,
  HardwareGaugesCard,
  HealthCard,
  HourlyCard,
  HwAggCard,
  NetAggCard,
  NetworkLiveCard,
  NetworkStatsCard,
  StatsCard,
  TempAggCard,
} from './cards';
import {
  insertCardBefore,
  isAggregate,
  loadLayout,
  resolveTemplate,
  resizeByDelta,
  saveLayout,
  setCardSize,
  sizeFor,
  SIZE_COLS,
  SIZE_ROWS,
  spanClass,
  TEMPLATE_IDS,
  toggleCard,
} from './dashboard-layout';
import type { CardId, CardSize, DashboardLayout } from './dashboard-layout';

type DateRangeKey = 'today' | 'yesterday' | 'week' | 'month' | 'year';

/** 日期范围边界按配置时区计算（今天/昨天/本周/本月）。 */
function rangeFor(
  key: DateRangeKey,
  config: { timezone?: string } | null,
): { start: string; end: string; focus: string } {
  const focus = todayStr(config);
  if (key === 'today') {
    return { start: focus, end: focus, focus };
  }
  if (key === 'yesterday') {
    const s = shiftDayStr(focus, -1);
    return { start: s, end: s, focus: s };
  }
  if (key === 'week') {
    const wd = weekdayOf(focus) === 0 ? 7 : weekdayOf(focus);
    const monday = shiftDayStr(focus, -(wd - 1));
    return { start: monday, end: focus, focus };
  }
  if (key === 'month') {
    return { start: `${focus.slice(0, 7)}-01`, end: focus, focus };
  }
  return { start: `${focus.slice(0, 4)}-01-01`, end: focus, focus };
}

function clsx(...parts: (string | false | undefined | null)[]): string {
  return parts.filter(Boolean).join(' ');
}

export default function DashboardPage() {
  const { t } = useTranslation();
  const { config } = useAppStore(useShallow((s) => ({ config: s.config })));
  const refreshInterval = config?.refresh_interval_seconds ?? 10;
  const [range, setRange] = useState<DateRangeKey>('today');
  const [layout, setLayout] = useState<DashboardLayout>(() => loadLayout());
  const [editOpen, setEditOpen] = useState(false);
  const [editLayout, setEditLayout] = useState<DashboardLayout | null>(null);
  const [dragId, setDragId] = useState<CardId | null>(null);
  const [dropTargetId, setDropTargetId] = useState<CardId | null>(null);
  const [resizeId, setResizeId] = useState<CardId | null>(null);
  const [resizeSize, setResizeSize] = useState<CardSize | null>(null);
  const [resizePos, setResizePos] = useState<{ x: number; y: number } | null>(null);
  const resizeRef = useRef<{
    id: CardId;
    startX: number;
    startY: number;
    startSize: CardSize;
    strideX: number;
    strideY: number;
  } | null>(null);
  // 网格行高：等于列宽 × 0.92（近正方形单元），由容器宽度实时计算。
  const gridRef = useRef<HTMLDivElement | null>(null);
  // 编辑缩略图网格（测列距步长用）。
  const thumbGridRef = useRef<HTMLDivElement | null>(null);
  const [tileH, setTileH] = useState(120);
  useEffect(() => {
    const el = gridRef.current;
    if (!el) return;
    const update = () => {
      const gap = 16; // --ui-gap
      const w = el.clientWidth;
      const col = (w - gap * 2) / 3;
      setTileH(Math.max(88, Math.round(col * 0.92)));
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // 编辑缩略图行高：弹窗固定 max-w-lg(512px)，列宽 ≈ (512 - p-6*2 - 容器p-2*2 - gap*2)/3 ≈ 145px，
  // 行高 = 列宽 × 0.92（与主网格同比例 1:1）。用固定值避免 ResizeObserver 时序抖动。
  const [thumbH] = useState(() => Math.round(145 * 0.92));
  const [data, setData] = useState<DashboardDataDto | null>(null);
  const [hourly, setHourly] = useState<number[]>([]);
  const [calendarUsage, setCalendarUsage] = useState<Map<string, number>>(new Map());
  const [calendarMax, setCalendarMax] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const { start, end, focus } = useMemo(() => rangeFor(range, config), [range, config]);
  const year = Number(focus.slice(0, 4));

  // 轻量轮询：只刷新仪表盘数据 + 今日小时分布（10s 一次）。
  const load = useCallback(async () => {
    try {
      const [dash, hourlyData] = await Promise.all([
        apiService.getDashboardData(start, end),
        apiService.getDayHourly(focus),
      ]);
      setData(dash);
      setHourly(hourlyData);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [start, end, focus]);

  useEffect(() => {
    void load();
    const timer = window.setInterval(() => void load(), refreshInterval * 1000);
    return () => window.clearInterval(timer);
  }, [load, refreshInterval]);

  // 全年热力图：数据量大，只在年份变化时加载一次，不随 10s 轮询刷新。
  useEffect(() => {
    let cancelled = false;
    void apiService
      .getYearHeatmap(year)
      .then((rows) => {
        if (cancelled) return;
        const usage = new Map<string, number>();
        let max = 0;
        for (const [date, secs] of rows) {
          usage.set(date, secs);
          if (secs > max) max = secs;
        }
        setCalendarUsage(usage);
        setCalendarMax(max);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [year]);

  // 使用时间秒级跳动：由时长卡片内部每 1s 取「进行中的活跃会话已持续秒数」
  // （空闲时后端返回 0，自动停止）。仅当范围包含今天时才允许跳动（历史范围不跳）。
  const liveTickable = end >= todayStr(config);

  const openEditor = useCallback(() => {
    setEditLayout(JSON.parse(JSON.stringify(layout)) as DashboardLayout);
    setEditOpen(true);
  }, [layout]);

  const saveEditor = useCallback(() => {
    if (editLayout) {
      setLayout(editLayout);
      saveLayout(editLayout);
    }
    setEditOpen(false);
  }, [editLayout]);

  // 缩略图右下角手柄：2D 方向吸附拖拽——左右拖改变宽度、上下拖改变高度，
  // 位移超过死区后按格计步、越界吸附到最近合法档位（取代旧固定顺序循环）。
  useEffect(() => {
    if (!resizeId) return;
    const onMove = (e: PointerEvent) => {
      const r = resizeRef.current;
      if (!r) return;
      const next = resizeByDelta(
        r.startSize,
        e.clientX - r.startX,
        e.clientY - r.startY,
        r.strideX,
        r.strideY,
      );
      setResizeSize(next);
      setResizePos({ x: e.clientX, y: e.clientY });
      setEditLayout((prev) => (prev ? setCardSize(prev, r.id, next) : prev));
    };
    const onUp = () => {
      resizeRef.current = null;
      document.body.style.cursor = '';
      setResizeId(null);
      setResizeSize(null);
      setResizePos(null);
    };
    document.body.style.cursor = 'nwse-resize';
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      document.body.style.cursor = '';
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  }, [resizeId]);

  const startResize = useCallback(
    (e: React.PointerEvent, id: CardId) => {
      if (!editLayout) return;
      e.preventDefault();
      e.stopPropagation();
      // 指针捕获：拖出窗口/弹窗外松手也能收到 pointerup，避免拖拽状态卡死。
      try {
        e.currentTarget.setPointerCapture(e.pointerId);
      } catch {
        /* 某些输入设备不支持捕获时忽略 */
      }
      const grid = thumbGridRef.current;
      // 3 列网格的列距步长 = 列宽 + 间距（gap-1.5 = 6px）= (clientWidth + 6) / 3
      const strideX = grid ? (grid.clientWidth + 6) / 3 : 150;
      resizeRef.current = {
        id,
        startX: e.clientX,
        startY: e.clientY,
        startSize: editLayout.cards[id].size,
        strideX,
        strideY: thumbH + 6,
      };
      setResizeSize(editLayout.cards[id].size);
      setResizePos({ x: e.clientX, y: e.clientY });
      setResizeId(id);
    },
    [editLayout, thumbH],
  );

  // 键盘调尺寸（手柄聚焦后按方向键）：与拖拽同一套吸附规则，越界自动收敛。
  const keyResize = useCallback(
    (e: React.KeyboardEvent, id: CardId) => {
      if (!editLayout) return;
      const size = editLayout.cards[id].size;
      const cols = SIZE_COLS[size];
      const rows = SIZE_ROWS[size];
      let next: CardSize | null = null;
      if (e.key === 'ArrowLeft') next = sizeFor(cols - 1, rows);
      else if (e.key === 'ArrowRight') next = sizeFor(cols + 1, rows);
      else if (e.key === 'ArrowUp') next = sizeFor(cols, rows - 1);
      else if (e.key === 'ArrowDown') next = sizeFor(cols, rows + 1);
      if (!next || next === size) return;
      e.preventDefault();
      setEditLayout((prev) => (prev ? setCardSize(prev, id, next) : prev));
    },
    [editLayout],
  );

  const renderCard = useCallback(
    (id: CardId, size: CardSize) => {
      switch (id) {
        case 'stats':
          return <StatsCard data={data} size={size} tickable={liveTickable} />;
        case 'appUsage':
          return <AppUsageCard data={data} focus={focus} size={size} />;
        case 'hourly':
          return <HourlyCard hourly={hourly} size={size} />;
        case 'calendar':
          return <CalendarCard usage={calendarUsage} max={calendarMax} focus={focus} size={size} />;
        case 'networkStats':
          return <NetworkStatsCard size={size} />;
        case 'networkLive':
          return <NetworkLiveCard size={size} />;
        case 'netApps':
          return (
            <NetAppsCard
              title={t('dashboard.netApps.title')}
              limit={size === '1x1' ? 4 : size === '1x2' ? 6 : size === '3x1' ? 12 : 8}
              compact={size === '1x1' || size === '1x2'}
            />
          );
        case 'attrUsage':
          return (
            <AttributedUsageCard
              title={t('network.attrTitle')}
              limit={size === '1x1' ? 5 : size === '3x2' || size === '3x3' ? 15 : 8}
              compact={size === '1x1' || size === '1x2'}
            />
          );
        case 'hardwareGauges':
          return <HardwareGaugesCard size={size} />;
        case 'diskTemp':
          return <DiskTempCard size={size} />;
        case 'health':
          return <HealthCard />;
        case 'games':
          return <GamesCard size={size} />;
        case 'durationAgg':
          return <DurationAggCard data={data} hourly={hourly} size={size} tickable={liveTickable} />;
        case 'netAgg':
          return <NetAggCard size={size} />;
        case 'hwAgg':
          return <HwAggCard size={size} />;
        case 'tempAgg':
          return <TempAggCard size={size} />;
        default:
          return null;
      }
    },
    [data, focus, hourly, calendarUsage, calendarMax, liveTickable, t],
  );

  if (error) {
    return (
      <div className="mx-auto max-w-2xl rounded-2xl border border-destructive/30 bg-destructive/5 p-6 text-center text-sm text-destructive">
        {t('dashboard.loadFailed')}
        <Button variant="outline" size="sm" className="mt-3" onClick={() => void load()}>
          {t('dashboard.retry')}
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* 顶部：日期范围 + 编辑 */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap gap-1.5">
          {(['today', 'yesterday', 'week', 'month', 'year'] as DateRangeKey[]).map((key) => (
            <button
              key={key}
              type="button"
              aria-pressed={range === key}
              onClick={() => setRange(key)}
              className={clsx(
                'rounded-full border px-3 py-1 text-xs font-medium transition-colors',
                range === key
                  ? 'border-primary/30 bg-primary/10 text-primary'
                  : 'border-border bg-card text-muted-foreground hover:text-foreground',
              )}
            >
              {t(`dashboard.range.${key}`)}
            </button>
          ))}
        </div>
        <Button variant="outline" size="sm" onClick={openEditor}>
          <Edit3 className="h-3.5 w-3.5" />
          {t('dashboard.edit')}
        </Button>
      </div>

      {/* 卡片网格：九宫格单元网格（3 列，行高统一 --tile-h，dense 自动回填空洞） */}
      <div
        ref={gridRef}
        className="grid grid-cols-1 gap-[var(--ui-gap)] md:grid-cols-3 [grid-auto-flow:row_dense]"
        style={{ ['--tile-h' as string]: `${tileH}px`, gridAutoRows: 'var(--tile-h)' }}
      >
        {layout.order.map((id) => {
          const card = layout.cards[id];
          if (!card.visible) return null;
          return (
            <div key={id} className={clsx('min-h-0', spanClass(card.size))}>
              {renderCard(id, card.size)}
            </div>
          );
        })}
      </div>

      {/* 编辑弹窗 */}
      <Dialog open={editOpen} onOpenChange={setEditOpen}>
        <DialogContent className="max-h-[85vh] max-w-xl overflow-y-auto">
          <DialogHeader>
            <DialogTitle>{t('dashboard.editTitle')}</DialogTitle>
            <DialogDescription>{t('dashboard.editDescription')}</DialogDescription>
          </DialogHeader>

          {/* 可视化缩略图：同款九宫格网格（行高与主网格同比例 1:1），拖动换位置，拖右下角调尺寸，点眼睛隐藏 */}
          <div className="rounded-xl border border-border/60 bg-muted/25 p-2">
            <div
              ref={thumbGridRef}
              className="grid grid-cols-3 gap-1.5 [grid-auto-flow:row_dense]"
              style={{ gridAutoRows: `${thumbH}px` }}
            >
              {editLayout?.order.map((id) => {
                const card = editLayout.cards[id];
                if (!card.visible) return null;
                return (
                  <div
                    key={id}
                    draggable={resizeId !== id}
                    onDragStart={(e) => {
                      e.dataTransfer.setData('text/plain', id);
                      e.dataTransfer.effectAllowed = 'move';
                      setDragId(id);
                    }}
                    onDragEnd={() => {
                      setDragId(null);
                      setDropTargetId(null);
                    }}
                    onDragOver={(e) => {
                      e.preventDefault();
                      if (dragId && dragId !== id) setDropTargetId(id);
                    }}
                    onDragLeave={() => {
                      setDropTargetId((prev) => (prev === id ? null : prev));
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      const src = (e.dataTransfer.getData('text/plain') || dragId) as CardId;
                      if (src && src !== id) {
                        setEditLayout((prev) => (prev ? insertCardBefore(prev, src, id) : prev));
                      }
                      setDragId(null);
                      setDropTargetId(null);
                    }}
                    className={clsx(
                      'relative flex min-h-0 items-center justify-center gap-1 rounded-lg border text-[10px] font-medium transition-colors',
                      spanClass(card.size),
                      dragId === id
                        ? 'border-primary/50 bg-primary/15 opacity-50'
                        : dropTargetId === id
                          ? 'border-primary bg-primary/20 ring-2 ring-primary/30'
                          : 'border-border/70 bg-card hover:border-primary/40',
                    )}
                  >
                    <span className="truncate px-1.5">{t(`dashboard.cards.${id}`)}</span>
                    {isAggregate(id) && (
                      <span className="shrink-0 rounded bg-primary/15 px-1 py-px text-[8px] font-semibold text-primary">
                        {t('dashboard.aggBadge')}
                      </span>
                    )}
                    <button
                      type="button"
                      onClick={() => setEditLayout((prev) => (prev ? toggleCard(prev, id, false) : prev))}
                      onDragStart={(e) => e.stopPropagation()}
                      className="absolute -right-1.5 -top-1.5 flex h-4 w-4 items-center justify-center rounded-full border border-border bg-background text-muted-foreground shadow-sm hover:text-foreground"
                      title={t('dashboard.hideCard')}
                    >
                      <EyeOff className="h-2.5 w-2.5" />
                    </button>
                    <button
                      type="button"
                      onPointerDown={(e) => startResize(e, id)}
                      onKeyDown={(e) => keyResize(e, id)}
                      onDragStart={(e) => e.stopPropagation()}
                      aria-label={t('dashboard.resizeHandle')}
                      title={t('dashboard.resizeHandle')}
                      className={clsx(
                        'absolute bottom-0.5 right-0.5 flex h-4 w-4 cursor-nwse-resize items-center justify-center rounded border-0 bg-muted p-0 text-muted-foreground shadow-sm transition-colors hover:text-foreground',
                        resizeId === id && 'bg-primary/20 text-primary',
                      )}
                    >
                      <GripHorizontal className="h-3 w-3" />
                    </button>
                  </div>
                );
              })}
            </div>
            {editLayout && editLayout.order.filter((id) => editLayout.cards[id].visible).length === 0 && (
              <p className="py-4 text-center text-xs text-muted-foreground">{t('dashboard.emptyGrid')}</p>
            )}
            <p className="mt-2 text-[10px] leading-relaxed text-muted-foreground">{t('dashboard.resizeHint')}</p>
          </div>

          {/* 已隐藏卡片：点击恢复 */}
          {editLayout &&
            (() => {
              const hidden = editLayout.order.filter((id) => !editLayout.cards[id].visible);
              if (hidden.length === 0) return null;
              return (
                <div className="rounded-xl border border-dashed border-border/60 bg-muted/15 p-2">
                  <div className="mb-1.5 text-[11px] text-muted-foreground">{t('dashboard.hiddenCards')}</div>
                  <div className="flex flex-wrap gap-1.5">
                    {hidden.map((id) => (
                      <button
                        key={id}
                        type="button"
                        onClick={() => setEditLayout((prev) => (prev ? toggleCard(prev, id, true) : prev))}
                        className="flex items-center gap-1 rounded-md border border-border/60 bg-card px-2 py-1 text-[10px] text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
                      >
                        <Eye className="h-3 w-3" />
                        {t(`dashboard.cards.${id}`)}
                      </button>
                    ))}
                  </div>
                </div>
              );
            })()}

          <p className="rounded-lg border border-primary/15 bg-primary/5 px-3 py-2 text-xs leading-relaxed text-muted-foreground">
            {t('dashboard.aggregateHint')}
          </p>

          <DialogFooter>
            <div className="mr-auto flex flex-wrap items-center gap-1.5">
              <span className="text-[11px] text-muted-foreground">{t('dashboard.templateTitle')}</span>
              {(TEMPLATE_IDS).map((tid) => (
                <button
                  key={tid}
                  type="button"
                  onClick={() => setEditLayout(resolveTemplate(tid))}
                  className={clsx(
                    'rounded-full border px-2.5 py-1 text-[11px] font-medium transition-colors',
                    editLayout?.template === tid
                      ? 'border-primary/30 bg-primary/10 text-primary'
                      : 'border-border/70 bg-card text-muted-foreground hover:border-primary/40 hover:text-foreground',
                  )}
                >
                  {t(`dashboard.templates.${tid}`)}
                </button>
              ))}
              <Button variant="ghost" size="sm" onClick={() => setEditLayout(resolveTemplate('balanced'))}>
                <RotateCcw className="h-3.5 w-3.5" />
                {t('dashboard.resetLayout')}
              </Button>
            </div>
            <Button variant="outline" onClick={() => setEditOpen(false)}>
              {t('common.cancel')}
            </Button>
            <Button onClick={saveEditor}>
              <Check className="h-4 w-4" />
              {t('common.save')}
            </Button>
          </DialogFooter>
        </DialogContent>

        {/* 拖拽调尺寸时的悬浮尺寸徽标：portal 到 body，避开弹窗 transform 对 fixed 定位的影响 */}
        {resizePos && resizeSize
          ? createPortal(
              <span
                className="pointer-events-none fixed z-[60] rounded-md border border-border bg-background/95 px-1.5 py-0.5 text-[10px] font-semibold tabular-nums text-foreground shadow-md"
                style={{
                  left: resizePos.x,
                  top: resizePos.y,
                  transform: 'translate(-50%, calc(-100% - 10px))',
                }}
              >
                {resizeSize}
              </span>,
              document.body,
            )
          : null}
      </Dialog>
    </div>
  );
}
