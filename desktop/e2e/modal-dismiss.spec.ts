import { test, expect, type Locator, type Page } from '@playwright/test';
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

/**
 * Waits until every finite animation on the page has finished.
 *
 * ⚠️ Three ways to get this wrong, all of which were tried here first:
 *
 * 1. `element.getAnimations()` right after the element appears returns an
 *    **empty list** - the browser hasn't created the animation yet - so the
 *    wait resolves instantly and you measure mid-flight.
 * 2. Polling for a *stable* box is fooled by `animation-delay`:
 *    `anim-sheet` waits 100ms at `translateY(100%)`, so two consecutive
 *    reads 50ms apart both report the element parked off screen, and the
 *    poll happily returns that.
 * 3. Waiting for the list to empty hangs forever on `anim-pulse-dot`, which
 *    is `infinite` and never finishes.
 *
 * Polling `document.getAnimations()` for `playState === 'finished'` covers
 * the delay phase (where the state is already `running`) and skips infinite
 * animations explicitly.
 */
async function waitForAnimations(page: Page) {
  await page.waitForFunction(() =>
    document.getAnimations().every((animation) => {
      const timing = (animation.effect as KeyframeEffect | null)?.getTiming();
      return timing?.iterations === Infinity || animation.playState === 'finished';
    })
  );
}

/** The element's box once nothing is animating it. Viewport-relative. */
async function settledBox(locator: Locator) {
  await waitForAnimations(locator.page());
  const box = await locator.boundingBox();
  if (!box) throw new Error('element has no box');
  return box;
}

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
    }

    // The reported bug: no control within reach on a phone.
    test('the close control is on screen and big enough to hit', async ({ page }) => {
      await open(page);
      const close = page.getByRole('button', { name: 'Close' });
      // `anim-pop` scales the dialog from 0.95, so a box read mid-animation
      // reports a 44px control as 43.0 - see settledBox.
      const box = await settledBox(close);
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

      const box = await settledBox(page.getByRole('button', { name: 'Close' }));
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
  // By testid, not by role: the sidebar is an <aside> too, and it's visible
  // from md: up - so `getByRole('complementary')` matches two elements at
  // tablet width. Same trap the sidebar's own testid exists for.
  const sheet = (page: Page) => page.getByTestId('event-peek');

  async function openSheet(page: Page) {
    await page.goto('/');
    await page.getByText('Soirée jeux de société chez Hugo', { exact: false }).first().click();
    await expect(sheet(page).getByRole('button', { name: 'Close' })).toBeVisible();
  }

  test('tapping outside dismisses it on mobile', async ({ page }, testInfo) => {
    test.skip(testInfo.project.name === 'desktop-1280', 'a persistent column from lg: (1024px) up');
    await openSheet(page);

    // Well above the sheet, over the calendar grid.
    await page.mouse.click(200, 300);

    await expect(sheet(page).getByRole('button', { name: 'Close' })).toHaveCount(0);
  });

  test('the close control is reachable and big enough', async ({ page }, testInfo) => {
    test.skip(testInfo.project.name === 'desktop-1280', 'no close control on the column');
    await openSheet(page);

    const close = sheet(page).getByRole('button', { name: 'Close' });
    // `anim-sheet` slides this up from below the fold - measure it parked.
    const box = await settledBox(close);
    expect(box.height).toBeGreaterThanOrEqual(44);
    expect(box.width).toBeGreaterThanOrEqual(44);
    expect(box.y + box.height).toBeLessThanOrEqual(page.viewportSize()!.height);

    await close.click();
    await expect(close).toHaveCount(0);
  });

  // The bug the CI-only failure turned out to be: `Calendar.svelte`'s body
  // carries `anim-fade-up`, whose `forwards` fill left an identity
  // `transform` in effect - which makes it the containing block for every
  // `position: fixed` descendant. The sheet's `bottom: 0` was resolving
  // against the calendar's content box rather than the viewport, so it sat
  // below the fold (20px on macOS, far enough on Linux to push the close
  // button off screen). Its full-screen backdrop missed the viewport for
  // the same reason.
  test('the sheet is pinned to the viewport, not to the page content', async ({
    page
  }, testInfo) => {
    test.skip(testInfo.project.name === 'desktop-1280', 'a static column from lg: up');
    await openSheet(page);

    const viewportHeight = page.viewportSize()!.height;
    const box = await settledBox(sheet(page));
    expect(
      Math.round(box.y + box.height),
      'sheet bottom should sit exactly on the viewport bottom'
    ).toBe(viewportHeight);

    // Same containing-block failure, same fix - assert it directly rather
    // than trusting that one implies the other.
    const offenders = await page.evaluate(() => {
      const found: string[] = [];
      let node = document.querySelector('[data-testid="event-peek"]')
        ?.parentElement as HTMLElement | null;
      while (node) {
        const style = getComputedStyle(node);
        if (
          style.transform !== 'none' ||
          style.filter !== 'none' ||
          style.perspective !== 'none' ||
          style.contain !== 'none'
        ) {
          found.push(`${node.tagName}.${node.className.slice(0, 40)} transform=${style.transform}`);
        }
        node = node.parentElement;
      }
      return found;
    });
    expect(
      offenders,
      'an ancestor with a transform/filter/contain makes `fixed` resolve against it, not the viewport'
    ).toEqual([]);
  });

  test('the back gesture dismisses it without leaving the page', async ({ page }, testInfo) => {
    test.skip(testInfo.project.name === 'desktop-1280', 'a persistent column from lg: (1024px) up');
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
    test.skip(testInfo.project.name !== 'desktop-1280', 'sheet behaviour is tested above');
    await page.goto('/');
    await page.getByText('Soirée jeux de société chez Hugo', { exact: false }).first().click();
    await expect(page.getByTestId('event-peek').getByText('Cost per person')).toBeVisible();

    await page.mouse.click(400, 200);

    await expect(page.getByTestId('event-peek').getByText('Cost per person')).toBeVisible();
  });
});
