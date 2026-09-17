import { test, expect, type Page } from '@playwright/test';
import { mockApi } from './fixtures';

/**
 * Every way out of a modal, at every width.
 *
 * ⚠️ These belong in a real browser, not vitest: happy-dom has no layout
 * (so it cannot tell that a control is off screen), no `history` the back
 * gesture can act on, and no constraint validation. The bug these cover -
 * a modal with no reachable close control, escaped only by a back gesture
 * that navigated off the page - was invisible to every component test.
 */

const MODALS = [
  {
    name: 'create event',
    path: '/',
    open: async (page: Page) => page.getByRole('button', { name: '+ New Event' }).click()
  },
  {
    name: 'add to calendar',
    path: '/announcements',
    open: async (page: Page) =>
      page
        .getByRole('button', { name: /Add to calendar/ })
        .first()
        .click()
  }
];

test.beforeEach(async ({ page }) => {
  await mockApi(page);
});

for (const modal of MODALS) {
  test.describe(modal.name, () => {
    const dialog = (page: Page) => page.getByRole('dialog');

    async function open(page: Page) {
      await page.goto(modal.path);
      await modal.open(page);
      await expect(dialog(page)).toBeVisible();
      // ⚠️ `anim-pop` scales the dialog from 0.95, and boundingBox() during
      // it reports the *scaled* size - a 44px control measures 43.0 and the
      // tap-target assertion fails for a reason that has nothing to do with
      // the CSS. Same trap as sampling colour mid theme-transition.
      await dialog(page).evaluate((el) =>
        Promise.all(el.getAnimations({ subtree: true }).map((a) => a.finished))
      );
    }

    // The reported bug: no control within reach on a phone.
    test('the close control is on screen and big enough to hit', async ({ page }) => {
      await open(page);
      const close = page.getByRole('button', { name: 'Close' });
      const box = (await close.boundingBox())!;
      const viewport = page.viewportSize()!;

      expect(box.y, 'close button is above the fold').toBeGreaterThanOrEqual(0);
      expect(box.y + box.height, 'close button is below the fold').toBeLessThanOrEqual(
        viewport.height
      );
      // The same 44px floor the bottom tab bar is held to.
      expect(box.height, `only ${box.height}px tall`).toBeGreaterThanOrEqual(44);
      expect(box.width, `only ${box.width}px wide`).toBeGreaterThanOrEqual(44);

      await close.click();
      await expect(dialog(page)).toHaveCount(0);
    });

    test('stays reachable after scrolling to the bottom of the form', async ({ page }) => {
      await open(page);
      await page.mouse.wheel(0, 4000);
      await page.waitForTimeout(150);

      const box = (await page.getByRole('button', { name: 'Close' }).boundingBox())!;
      expect(box.y).toBeGreaterThanOrEqual(0);
      expect(box.y + box.height).toBeLessThanOrEqual(page.viewportSize()!.height);
    });

    test('escape closes it', async ({ page }) => {
      await open(page);
      await page.keyboard.press('Escape');
      await expect(dialog(page)).toHaveCount(0);
    });

    test('tapping the backdrop closes it', async ({ page }) => {
      await open(page);
      // Top-left of the overlay is backdrop at every width.
      await page.mouse.click(5, 5);
      await expect(dialog(page)).toHaveCount(0);
    });

    // What the user actually did: swipe back. It used to leave the page.
    test('the back gesture closes it without leaving the page', async ({ page }) => {
      await page.goto('/settings');
      await page.goto(modal.path);
      await modal.open(page);
      await expect(dialog(page)).toBeVisible();

      await page.goBack();

      await expect(dialog(page)).toHaveCount(0);
      expect(new URL(page.url()).pathname).toBe(modal.path);
    });

    // The other half: a modal opened and closed must not leave a dead entry
    // behind, or leaving the page takes two presses of back.
    test('closing it leaves history clean', async ({ page }) => {
      await page.goto('/settings');
      await page.goto(modal.path);
      await modal.open(page);
      await expect(dialog(page)).toBeVisible();

      await page.getByRole('button', { name: 'Close' }).click();
      await expect(dialog(page)).toHaveCount(0);

      await page.goBack();
      await expect(page).toHaveURL(/\/settings$/);
    });
  });
}

/**
 * The event peek panel is a sheet below `md:` and a persistent column from
 * `md:` up, so its dismissal rules differ by width - which is exactly the
 * kind of thing only a real browser can check.
 */
test.describe('event peek sheet', () => {
  const sheet = (page: Page) => page.getByRole('complementary');

  async function openSheet(page: Page) {
    await page.goto('/');
    await page.getByText('Soirée jeux de société chez Hugo', { exact: false }).first().click();
    await expect(sheet(page).getByRole('button', { name: 'Close' })).toBeVisible();
    // `anim-sheet` slides the panel up from below the fold, so measuring
    // during it puts every control off screen. Same trap as `anim-pop`.
    await sheet(page).evaluate((el) =>
      Promise.all(el.getAnimations({ subtree: true }).map((a) => a.finished))
    );
  }

  test('tapping outside dismisses it on mobile', async ({ page }, testInfo) => {
    test.skip(testInfo.project.name !== 'mobile-402', 'a persistent column from md: (768px) up');
    await openSheet(page);

    // Well above the sheet, over the calendar grid.
    await page.mouse.click(200, 300);

    await expect(sheet(page).getByRole('button', { name: 'Close' })).toHaveCount(0);
  });

  test('the close control is reachable and big enough', async ({ page }, testInfo) => {
    test.skip(testInfo.project.name !== 'mobile-402', 'no close control on the column');
    await openSheet(page);

    const close = sheet(page).getByRole('button', { name: 'Close' });
    const box = (await close.boundingBox())!;
    expect(box.height).toBeGreaterThanOrEqual(44);
    expect(box.width).toBeGreaterThanOrEqual(44);
    expect(box.y + box.height).toBeLessThanOrEqual(page.viewportSize()!.height);

    await close.click();
    await expect(close).toHaveCount(0);
  });

  test('the back gesture dismisses it without leaving the page', async ({ page }, testInfo) => {
    test.skip(testInfo.project.name !== 'mobile-402', 'a persistent column from md: (768px) up');
    await page.goto('/settings');
    await openSheet(page);

    await page.goBack();

    await expect(sheet(page).getByRole('button', { name: 'Close' })).toHaveCount(0);
    expect(new URL(page.url()).pathname).toBe('/');
  });

  // The desktop column has an explicit "select an event" empty state and is
  // meant to stay put; dismissing it on any stray click would be a
  // regression, not a fix.
  test('the desktop column is not dismissed by clicking the calendar', async ({
    page
  }, testInfo) => {
    test.skip(testInfo.project.name === 'mobile-402', 'sheet behaviour is tested above');
    await page.goto('/');
    await page.getByText('Soirée jeux de société chez Hugo', { exact: false }).first().click();
    await expect(page.getByRole('complementary').getByText('Cost per person')).toBeVisible();

    await page.mouse.click(400, 200);

    await expect(page.getByRole('complementary').getByText('Cost per person')).toBeVisible();
  });
});
