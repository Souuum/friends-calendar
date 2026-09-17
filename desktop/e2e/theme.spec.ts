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

      // oklab as well as oklch: while a transition is running Chrome
      // reports the *interpolated* colour, and it interpolates in oklab.
      // Both put perceptual lightness first, on the same scale.
      const ok = value.match(/^okl(?:ch|ab)\(\s*([\d.]+)(%?)/i);
      if (ok) {
        const l = Number(ok[1]);
        return ok[2] === '%' ? l / 100 : l;
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

// The failure mode that inverting a ramp invites: `bg-surface` cards and the
// `bg-gray-50` page ground both land on near-identical darks, and every
// card boundary disappears.
test('cards stay distinguishable from the page behind them', async ({ page }) => {
  await visit(page, '/servers', 'dark');

  const pageBg = await luminanceOf(page, 'body', 'background-color');
  // Scoped to `main`, because the first `.bg-surface` in the document is the
  // sidebar, which is `hidden` below md - measuring a display:none element
  // told us nothing about whether a card is distinguishable. (Not
  // `:visible`: that's a Playwright locator pseudo-class, and `luminanceOf`
  // resolves its selector with querySelector in page context.)
  const SURFACE = 'main .bg-surface';
  await expect(page.locator(SURFACE).first()).toBeVisible();
  const cardBg = await luminanceOf(page, SURFACE, 'background-color');

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

test('the header toggle flips the theme and persists it', async ({ page }) => {
  // Deliberately NOT visit(): that helper seeds localStorage via
  // addInitScript, which re-runs on every navigation - including the reload
  // below, where it would overwrite exactly the value under test. Starting
  // from an unset preference on a light OS gives the same starting state
  // without pinning storage.
  await mockApi(page);
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/settings');
  await page.waitForLoadState('networkidle');
  await page.locator('main').first().waitFor({ state: 'visible' });

  const toggle = page.getByTestId('theme-toggle');
  await expect(toggle).toBeVisible();
  await expect(toggle).toHaveAttribute('aria-label', 'Switch to dark mode');

  const before = await luminanceOf(page, 'body', 'background-color');
  await toggle.click();

  await expect(page.locator('html')).toHaveClass(/dark/);
  await expect(toggle).toHaveAttribute('aria-label', 'Switch to light mode');

  // Wait for the cross-fade to finish before measuring. Reading straight
  // after the click samples the colour mid-transition, which is still the
  // old one - the class coming off is the signal that it has settled, and
  // is more honest than sleeping for the duration.
  await expect(page.locator('html')).not.toHaveClass(/theme-transition/);

  const after = await luminanceOf(page, 'body', 'background-color');
  expect(before, 'the page should have been light before the click').toBeGreaterThan(0.8);
  expect(after, 'the page should be dark after the click').toBeLessThan(0.3);

  // Survives navigation, which is what writing to localStorage buys.
  await page.reload();
  await page.waitForLoadState('networkidle');
  await expect(page.locator('html')).toHaveClass(/dark/);
});

test('the toggle leaves system by flipping what is rendered', async ({ page }) => {
  await mockApi(page);
  await page.addInitScript(() => window.localStorage.setItem('theme', 'system'));
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/settings');
  await page.waitForLoadState('networkidle');

  // System says dark, so the page is dark and the button offers light.
  await expect(page.locator('html')).toHaveClass(/dark/);
  const toggle = page.getByTestId('theme-toggle');
  await expect(toggle).toHaveAttribute('aria-label', 'Switch to light mode');

  await toggle.click();
  await expect(page.locator('html')).not.toHaveClass(/dark/);
});

test('the cross-fade rule wins over Tailwind transition utilities', async ({ page }) => {
  await visit(page, '/settings', 'light');

  const duration = await page.evaluate(() => {
    document.documentElement.classList.add('theme-transition');
    // A button carrying Tailwind's own `transition-colors`, which would
    // otherwise set transition-property and shadow the theme rule.
    const el = document.querySelector('[aria-label="Appearance"] button');
    return el ? getComputedStyle(el).transitionDuration : null;
  });

  expect(duration, 'the .theme-transition rule should apply here').toBe('0.22s');
});

// The theme rule uses transition-property/duration longhand precisely so
// this override still lands. A `transition:` shorthand would re-set the
// duration after it and animate for users who asked not to see animation.
test('reduced motion still overrides the cross-fade', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await visit(page, '/settings', 'light');

  const duration = await page.evaluate(() => {
    document.documentElement.classList.add('theme-transition');
    const el = document.querySelector('[aria-label="Appearance"] button');
    return el ? getComputedStyle(el).transitionDuration : null;
  });

  // Chrome normalises 0.001ms to "1e-06s", so compare seconds rather than
  // the formatted string.
  const seconds = Number(String(duration).replace(/s$/, ''));
  expect(seconds, `expected effectively zero, got "${duration}"`).toBeLessThan(0.01);
});

/**
 * `text-white` means "ink on a coloured fill" and must stay light in both
 * themes. Dark mode originally redefined `--color-white` to use it as the
 * card surface, which rendered every such label near-black - measured
 * oklch(0.22) on an oklch(0.62) purple button. Surface now has its own
 * token; this guards the split.
 */
test('text-white stays light in dark mode', async ({ page }) => {
  await visit(page, '/settings', 'dark');

  const samples = await page.evaluate(() =>
    Array.from(document.querySelectorAll('[class~="text-white"]'))
      .filter((el) => el.getBoundingClientRect().width > 0)
      .map((el) => ({
        text: (el.textContent ?? '').trim().slice(0, 24),
        color: getComputedStyle(el).color
      }))
  );

  expect(samples.length, 'expected some text-white elements to check').toBeGreaterThan(0);
  for (const s of samples) {
    const l = Number(String(s.color).match(/^okl(?:ch|ab)\(\s*([\d.]+)/i)?.[1] ?? 1);
    expect(l, `"${s.text}" renders at lightness ${l} - it should be near-white`).toBeGreaterThan(
      0.8
    );
  }
});

// The reported bug: the switcher's background was an arbitrary hex, which
// compiles to a literal colour rather than var(--color-*), so the theme
// swap could not reach it and the control stayed light with pale text.
test('the view switcher follows the theme', async ({ page }) => {
  await visit(page, '/', 'dark');

  const bg = await luminanceOf(page, '[data-testid="view-switcher"]', 'background-color');
  expect(bg, `switcher background lightness ${bg} - should be dark`).toBeLessThan(0.4);
});

/**
 * Native controls are painted by the browser, not by CSS.
 *
 * ⚠️ No selector reaches inside a `<select>`'s dropdown - the one thing
 * that steers it is `color-scheme`. Without it the root stayed `normal`
 * (light) however dark the page was, so Tailwind's preflight
 * (`color: inherit; background-color: transparent` on form controls) left
 * each `<option>` inheriting near-white text on the popup's light-scheme
 * white background. The create-event Visibility picker was white on white.
 *
 * This is unassertable in vitest: happy-dom paints nothing and resolves no
 * system colours.
 */
test('native controls follow the theme', async ({ page }) => {
  await visit(page, '/', 'dark');
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).colorScheme)).toBe(
    'dark'
  );

  await visit(page, '/', 'light');
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).colorScheme)).toBe(
    'light'
  );
});

// The control that actually broke. ⚠️ The dropdown itself cannot be
// asserted on: it is an OS-level window that headless Chromium neither
// paints nor screenshots, and the *closed* select measures the same dark
// fill with or without the fix (a pixel-sampling version of this test
// passed with the bug present, which is why it isn't here). What is
// checkable is the pair that made it white-on-white: the option text
// inherits near-white, so the popup behind it has to be dark-scheme too.
test('the visibility dropdown will not render light text on a light popup', async ({ page }) => {
  await visit(page, '/', 'dark');
  await page.getByRole('button', { name: '+ New Event' }).click();

  const { scheme, optionLightness } = await page.evaluate(() => {
    const select = document.querySelector('#visibility') as HTMLSelectElement;
    const option = select.querySelector('option')!;
    const colour = getComputedStyle(option).color;
    return {
      scheme: getComputedStyle(select).colorScheme,
      optionLightness: Number(colour.match(/^okl(?:ch|ab)\(\s*([\d.]+)/i)?.[1] ?? 0)
    };
  });

  expect(optionLightness, 'options inherit near-white text in dark mode').toBeGreaterThan(0.8);
  expect(scheme, 'so the popup behind that text must be dark-scheme').toBe('dark');
});
