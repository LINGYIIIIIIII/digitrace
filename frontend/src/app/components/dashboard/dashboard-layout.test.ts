import { describe, expect, it } from 'vitest';
import { resizeByDelta, sizeFor, snapDelta } from './dashboard-layout';

describe('dashboard-layout 手柄尺寸吸附', () => {
  it('sizeFor 保留合法档位', () => {
    expect(sizeFor(1, 1)).toBe('1x1');
    expect(sizeFor(1, 2)).toBe('1x2');
    expect(sizeFor(2, 1)).toBe('2x1');
    expect(sizeFor(2, 2)).toBe('2x2');
    expect(sizeFor(3, 1)).toBe('3x1');
    expect(sizeFor(3, 2)).toBe('3x2');
    expect(sizeFor(3, 3)).toBe('3x3');
  });

  it('sizeFor 越界收敛：列夹在 1..3，窄列最高 2 行', () => {
    expect(sizeFor(0, 1)).toBe('1x1');
    expect(sizeFor(4, 2)).toBe('3x2');
    // 竖长 1x3 不在档位表（用户明确不要），收敛到 1x2
    expect(sizeFor(1, 3)).toBe('1x2');
    // 2x3 不在档位表，收敛到 2x2
    expect(sizeFor(2, 3)).toBe('2x2');
    expect(sizeFor(3, 4)).toBe('3x3');
  });

  it('snapDelta 有 0.25 格死区，之后每拖满 1 格换一档', () => {
    expect(snapDelta(10, 100)).toBe(0);
    expect(snapDelta(25, 100)).toBe(0);
    expect(snapDelta(26, 100)).toBe(1);
    expect(snapDelta(100, 100)).toBe(1);
    expect(snapDelta(125, 100)).toBe(2);
    expect(snapDelta(225, 100)).toBe(3);
    expect(snapDelta(-125, 100)).toBe(-2);
  });

  it('resizeByDelta 左右拖变列数、上下拖变行数', () => {
    // 步长参考：缩略图列距 ≈ 151px、行距 ≈ 139px
    expect(resizeByDelta('1x1', 150, 0, 151, 139)).toBe('2x1');
    expect(resizeByDelta('1x1', 0, 150, 151, 139)).toBe('1x2');
    expect(resizeByDelta('1x1', 120, 160, 151, 139)).toBe('2x2');
    // 一次拖够可直接跨档：向右 2 格 → 整行
    expect(resizeByDelta('1x1', 300, 0, 151, 139)).toBe('3x1');
  });

  it('resizeByDelta 越界吸附到最近合法档位', () => {
    // 2 列拖到第 3 行：2x3 不存在 → 保持 2x2
    expect(resizeByDelta('2x2', 0, 160, 151, 139)).toBe('2x2');
    // 2 列同时拖宽拖高 → 3x3 整屏
    expect(resizeByDelta('2x2', 200, 160, 151, 139)).toBe('3x3');
    // 整行逐行加高
    expect(resizeByDelta('3x1', 0, 150, 151, 139)).toBe('3x2');
    expect(resizeByDelta('3x2', 0, 150, 151, 139)).toBe('3x3');
    // 已是最小，继续缩小不动
    expect(resizeByDelta('1x1', -150, 0, 151, 139)).toBe('1x1');
    expect(resizeByDelta('1x1', 0, -150, 151, 139)).toBe('1x1');
    // 2x2 向左缩成窄列（1x2）
    expect(resizeByDelta('2x2', -170, 0, 151, 139)).toBe('1x2');
  });
});
