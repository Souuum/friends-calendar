import { test, expect, type Page } from '@playwright/test';
import { mockApi, ROUTES_UNDER_TEST } from './fixtures';

/**
 * Dark mode is implemented by redefining Tailwind's colour variables under
 * `.dark` rather than adding `dark:` utilities to ~272 call sites, so the
 * risk isn't "a component was missed" - it's that an inverted grey ramp
 * collapses somewhere: a card ending up the same colour as the page behind
 * it, or text that stayed dark on a now-dark surface.
 *
 * These assertions are about *luminance relationships*, which a machine can
 * check. Whether it looks good is still a question for the screenshots.
 */

/**
 * Lightness of a computed colour, 0 (black) to 1 (white).
 *
 * Handles both forms Chrome returns, which is the point: colours authored
 * as `oklch()` come back as `oklch()`, not converted to rgb. Naively
 * regexing three numbers out of `oklch(0.21 0.006 285)` treats the *hue*
 * (285) as a blue channel and yields nonsense - the first version of this
 * helper did exactly that and reported light-mode backgrounds as dark.
 */
async function luminanceOf(page: Page, selector: string, prop: string): Promise<number> {
  return page.evaluate(
    ([sel, property]) => {
      const el = document.querySelector(sel as string);
      if (!el) throw new Error(`no element matched ${sel}`);
      const value = getComputedStyle(el)
        .getPropertyValue(property as string)
        .trim();

      const oklch = value.match(/^oklch\(\s*([\d.]+)(%?)/i);
      if (oklch) {
        // L is already perceptual lightness: 0-1, or 0-100 with a percent.
        const l = Number(oklch[1]);
        return oklch[2] === '%' ? l / 100 : l;
      }

      const rgb = value.match(/^rgba?\(([^)]+)\)/i);
      if (rgb) {
        const [r, g, b] = rgb[1].split(/[\s,/]+/).map(Number);
        // Rec. 709 luma - close enough to tell "dark" from "light".
        return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
      }

      throw new Error(`unrecognised colour format: "${value}"`);
    },
    [selector, prop]
  );
}

async function visit(page: Page, route: string, theme: 'light' | 'dark') {
  await mockApi(page);
  await page.addInitScript((value) => {
    window.localStorage.setItem('theme', value as string);
  }, theme);
  await page.goto(route);
  await page.waitForLoadState('networkidle');
  await page.locator('main').first().waitFor({ state: 'visible' });
}

test('the dark class is applied before paint, not after hydration', async ({ page }) => {
  await visit(page, '/', 'dark');
  // Set by the inline script in app.html. If this only appeared after
  // hydration, every load would flash light first.
  await expect(page.locator('html')).toHaveClass(/dark/);
});

test('light mode does not get the dark class', async ({ page }) => {
  await visit(page, '/', 'light');
  await expect(page.locator('html')).not.toHaveClass(/dark/);
});

test('dark mode actually inverts the page ground and text', async ({ page }) => {
  await visit(page, '/settings', 'dark');

  const bg = await luminanceOf(page, 'body', 'background-color');
  const fg = await luminanceOf(page, 'body', 'color');

  expect(bg, `body background luminance ${bg.toFixed(2)} - should be dark`).toBeLessThan(0.3);
  expect(fg, `body text luminance ${fg.toFixed(2)} - should be light`).toBeGreaterThan(0.6);
});

test('light mode is still light', async ({ page }) => {
  await visit(page, '/settings', 'light');

  const bg = await luminanceOf(page, 'body', 'background-color');
  const fg = await luminanceOf(page, 'body', 'color');

  expect(bg).toBeGreaterThan(0.8);
  expect(fg).toBeLessThan(0.5);
});

// The failure mode that inverting a ramp invites: `bg-white` cards and the
// `bg-gray-50` page ground both land on near-identical darks, and every
// card boundary disappears.
test('cards stay distinguishable from the page behind them', async ({ page }) => {
  await visit(page, '/servers', 'dark');

  const pageBg = await luminanceOf(page, 'body', 'background-color');
  const card = page.locator('.bg-white').first();
  await expect(card).toBeVisible();
  const cardBg = await luminanceOf(page, '.bg-white', 'background-color');

  expect(
    Math.abs(cardBg - pageBg),
    `card lightness ${cardBg.toFixed(3)} vs page ${pageBg.toFixed(3)} - too close to tell apart`
  ).toBeGreaterThan(0.03);
});

for (const route of ROUTES_UNDER_TEST) {
  const slug = route === '/' ? 'root' : route.replace(/^\//, '').replace(/\//g, '-');

  test(`${route} in dark mode`, async ({ page }, testInfo) => {
    await visit(page, route, 'dark');

    await page.screenshot({
      path: `e2e/screenshots/${testInfo.project.name}-dark/${slug}.png`,
      fullPage: true
    });

    // Theming shouldn't change layout, but a token swap can alter borders
    // and shadows, so the overflow guard is worth repeating here.
    const { scrollWidth, clientWidth } = await page.evaluate(() => ({
      scrollWidth: Math.max(document.documentElement.scrollWidth, document.body.scrollWidth),
      clientWidth: document.documentElement.clientWidth
    }));
    expect(scrollWidth, `${route} overflows in dark mode`).toBeLessThanOrEqual(clientWidth + 1);
  });
}
