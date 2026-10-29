import { test, expect } from '@playwright/test';
import { mockApi } from './fixtures';

/**
 * An announcement notification is an `<a href="/announcements">` whose click
 * handler marks it read. In a real browser that is a race the component
 * tier cannot see: happy-dom performs no navigation, so it can't tell
 * whether the mark-read POST survives the page changing underneath it.
 *
 * SvelteKit intercepts the click and navigates client-side, so the document
 * is not unloaded and the in-flight fetch is not cancelled. A full-page
 * navigation (`data-sveltekit-reload`, `target`, an external origin) would
 * abort it, and the notification would come back unread on the next visit.
 * Mutation-tested: adding `data-sveltekit-reload` to the row fails the
 * first test, but only because the stubbed mark-read answers after a delay.
 */

test('opening an announcement notification marks it read and lands on the feed', async ({
  page
}) => {
  await mockApi(page);

  const markRead: string[] = [];
  const failed: string[] = [];
  page.on('requestfailed', (req) => {
    if (req.url().includes('/read')) failed.push(`${req.url()} ${req.failure()?.errorText}`);
  });
  await page.route('**/api/notifications/*/read', async (route) => {
    markRead.push(new URL(route.request().url()).pathname);
    // A real backend takes time to answer. Responding instantly would let
    // the request complete even under a full-page navigation that aborts
    // it, so the test would pass against exactly the bug it is for.
    await new Promise((resolve) => setTimeout(resolve, 400));
    await route.fulfill({ status: 200, contentType: 'application/json', body: 'null' });
  });

  await page.goto('/notifications');
  const link = page.getByRole('link', { name: /posted an announcement/ });
  await expect(link).toHaveAttribute('href', '/announcements');

  const finished = page.waitForEvent(
    'requestfinished',
    (req) => req.url().endsWith('/api/notifications/n3/read'),
    { timeout: 5000 }
  );
  await link.click();

  await expect(page).toHaveURL(/\/announcements$/);
  await finished;
  expect(markRead).toEqual(['/api/notifications/n3/read']);
  expect(failed).toEqual([]);
});

test('other notification kinds stay on the page', async ({ page }) => {
  await mockApi(page);
  await page.goto('/notifications');

  await page.getByRole('button', { name: /a répondu/ }).click();
  await expect(page).toHaveURL(/\/notifications$/);
});
