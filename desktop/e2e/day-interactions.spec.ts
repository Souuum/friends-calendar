import { test, expect } from '@playwright/test';
import { mockApi } from './fixtures';

/**
 * ⚠️ These are browser tests because happy-dom has no pointer-movement
 * model and no layout: "dragging to scroll does not fire the long press" is
 * unassertable there, and it is the whole reason the action has a movement
 * threshold at all.
 */
test.beforeEach(async ({ page }) => {
  await mockApi(page);
  await page.goto('/');
});

/** A day cell with no events, so nothing else is under the pointer. */
function emptyDay(page) {
  return page.locator('[role="button"][aria-pressed]').nth(2);
}

test('a long press opens the create form on that day', async ({ page }) => {
  const cell = emptyDay(page);
  const box = (await cell.boundingBox())!;

  // `pointerType: 'touch'` deliberately: the action ignores mouse presses,
  // because a click-and-think would otherwise open the form.
  await cell.dispatchEvent('pointerdown', {
    pointerType: 'touch',
    clientX: box.x + 20,
    clientY: box.y + 20
  });
  await page.waitForTimeout(700);

  await expect(page.getByRole('dialog')).toBeVisible();
});

// Without the movement threshold, every scroll that starts on a day cell
// opens the create form.
test('dragging to scroll does not open it', async ({ page }) => {
  const cell = emptyDay(page);
  const box = (await cell.boundingBox())!;

  await cell.dispatchEvent('pointerdown', {
    pointerType: 'touch',
    clientX: box.x + 20,
    clientY: box.y + 20
  });
  await cell.dispatchEvent('pointermove', {
    pointerType: 'touch',
    clientX: box.x + 20,
    clientY: box.y + 120
  });
  await page.waitForTimeout(700);

  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('a short tap selects the day instead of creating', async ({ page }) => {
  const cell = emptyDay(page);

  await cell.click();

  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByText(/Nothing on this day/)).toBeVisible();
});
