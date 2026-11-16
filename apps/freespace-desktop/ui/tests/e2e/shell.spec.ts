import { expect, test } from '@playwright/test';

test('boots the shell', async ({ page }) => {
  await page.goto('/');

  await expect(
    page.getByRole('heading', { level: 1, name: 'Диски' }),
  ).toBeVisible();
});
