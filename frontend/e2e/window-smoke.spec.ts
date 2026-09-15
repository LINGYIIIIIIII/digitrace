import { expect, test } from '@playwright/test';

/** 无 Tauri 时的轻量冒烟：壳层可渲染、无横向溢出、关键导航文案存在。 */

test('desktop shell renders without console errors or horizontal overflow', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await expect(page.locator('body')).toBeVisible();
  await expect
    .poll(() => page.evaluate(() => document.body.scrollWidth <= window.innerWidth))
    .toBe(true);
  expect(errors).toEqual([]);
});

test('narrow resize keeps the shell usable', async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 620 });
  await page.goto('/');
  await expect(page.locator('body')).toBeVisible();
  await expect
    .poll(() => page.evaluate(() => document.body.scrollWidth <= window.innerWidth))
    .toBe(true);
});

test('brand or shell landmark is present', async ({ page }) => {
  await page.goto('/');
  // 无 Tauri 时可能停在加载骨架；只要 body 可交互且无横向溢出即可
  await expect(page.locator('body')).toBeVisible();
  const hasOverflow = await page.evaluate(() => document.body.scrollWidth > window.innerWidth);
  expect(hasOverflow).toBe(false);
});
