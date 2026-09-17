import { test, expect, type Page } from '@playwright/test';
import { mockApi } from './fixtures';

/**
 * Dismissal and submission of the "add to calendar" form.
 *
 * ⚠️ These live here rather than in vitest because **happy-dom does not
 * implement HTML5 constraint validation**. The bug this file exists for -
 * a `required` field the parser couldn't fill, so Chrome refused the submit
 * before `on:submit` ran, with no request, no error and no visible reason -
 * passed every component test in the suite. Same family as happy-dom
 * computing no layout: the tier can't see the failure at all.
 */

const ADOPTED = {
  event: { id: 'e9' },
  rsvps_recorded: 2,
  backfill_failed: false
};

/** `a1` parses cleanly; `a3` has no timestamp, so its date can't be read. */
async function openModal(page: Page, post: 'parseable' | 'unparseable') {
  await page.goto('/announcements');
  const buttons = page.getByRole('button', { name: /Add to calendar/ });
  await buttons.nth(post === 'parseable' ? 0 : 1).click();
  await expect(page.getByLabel('Title')).toBeVisible();
}

test.beforeEach(async ({ page }) => {
  await mockApi(page);
  await page.route('**/api/announcements/*/adopt', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(ADOPTED)
    })
  );
});

test('cancel dismisses it', async ({ page }) => {
  await openModal(page, 'parseable');
  await page.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByLabel('Title')).toHaveCount(0);
});

test('close dismisses it', async ({ page }) => {
  await openModal(page, 'parseable');
  await page.getByRole('button', { name: 'Close' }).click();
  await expect(page.getByLabel('Title')).toHaveCount(0);
});

test('a successful submit dismisses it and reports the RSVPs', async ({ page }) => {
  await openModal(page, 'parseable');
  await page.getByRole('button', { name: 'Add to calendar' }).last().click();

  await expect(page.getByLabel('Title')).toHaveCount(0);
  await expect(page.getByRole('status')).toContainText('2 people already going');
});

// The reported bug. A post with no parseable date leaves a required field
// empty; the browser used to refuse the submit silently, so the button did
// nothing and there was no way to tell why.
test('an unfillable field is reported, not silently swallowed', async ({ page }) => {
  let adoptCalled = false;
  await page.route('**/api/announcements/*/adopt', (route) => {
    adoptCalled = true;
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(ADOPTED)
    });
  });

  await openModal(page, 'unparseable');
  await expect(page.getByLabel('Starts')).toHaveValue('');

  await page.getByRole('button', { name: 'Add to calendar' }).last().click();

  // Visible, in the page, saying which field it means.
  await expect(page.getByText(/still needs a start time/)).toBeVisible();
  expect(adoptCalled).toBe(false);

  // And filling it in gets you through, rather than leaving you stuck.
  await page.getByLabel('Starts').fill('2027-03-01T20:00');
  await page.getByLabel('Ends').fill('2027-03-01T23:00');
  await page.getByRole('button', { name: 'Add to calendar' }).last().click();

  await expect(page.getByLabel('Title')).toHaveCount(0);
  expect(adoptCalled).toBe(true);
});

test('a failing submit keeps the draft, but never traps you', async ({ page }) => {
  await page.route('**/api/announcements/*/adopt', (route) =>
    route.fulfill({
      status: 400,
      contentType: 'application/json',
      body: JSON.stringify({ error: 'That announcement is already on the calendar as an event' })
    })
  );

  await openModal(page, 'parseable');
  await page.getByRole('button', { name: 'Add to calendar' }).last().click();

  await expect(page.getByText(/already on the calendar/)).toBeVisible();
  await expect(page.getByLabel('Title')).toHaveValue('EsdeeKid');

  await page.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByLabel('Title')).toHaveCount(0);
});
