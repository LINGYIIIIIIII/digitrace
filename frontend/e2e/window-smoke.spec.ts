import { expect, test } from '@playwright/test';

test('desktop shell renders without console errors or horizontal overflow', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await expect(page.locator('body')).toBeVisible();
  await expect.poll(() => page.evaluate(() => document.body.scrollWidth <= window.innerWidth)).toBe(true);
  expect(errors).toEqual([]);
});

test('narrow resize keeps the shell usable', async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 620 });
  await page.goto('/');
  await expect(page.locator('body')).toBeVisible();
  await expect.poll(() => page.evaluate(() => document.body.scrollWidth <= window.innerWidth)).toBe(true);
});
