import { test, expect, type Page } from '@playwright/test';
import { mockApi, ROUTES_UNDER_TEST } from './fixtures';

/**
 * Assertions a machine can make without judgement. Nothing here compares
 * pixels - see the note in playwright.config.ts for why that would be a
 * permanently-red CI job rather than a safety net.
 */

/** Names the elements sticking out past the viewport, so a failure is actionable. */
async function overflowingElements(page: Page) {
  return page.evaluate(() => {
    const limit = document.documentElement.clientWidth;
    const out: string[] = [];
    for (const el of Array.from(document.querySelectorAll('*'))) {
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) continue;
      if (r.right > limit + 1 || r.left < -1) {
        // `className` is an SVGAnimatedString on SVG nodes, not a string.
        const cls = String((el as HTMLElement).className ?? '').slice(0, 70);
        out.push(
          `<${el.tagName.toLowerCase()} class="${cls}"> left=${Math.round(r.left)} right=${Math.round(r.right)}`
        );
      }
    }
    return out.slice(0, 6);
  });
}

async function visit(page: Page, route: string) {
  await mockApi(page);
  await page.goto(route);
  await page.waitForLoadState('networkidle');
  // Pages render their shell before their fetch resolves; wait for the
  // content region so we measure a settled layout, not a loading state.
  await page.locator('main').first().waitFor({ state: 'visible' });
}

for (const route of ROUTES_UNDER_TEST) {
  const slug = route === '/' ? 'root' : route.replace(/^\//, '').replace(/\//g, '-');

  test(`${route} does not scroll horizontally`, async ({ page }, testInfo) => {
    await visit(page, route);

    // Captured before the assertion on purpose: a failing layout is exactly
    // when the picture is most useful.
    await page.screenshot({
      path: `e2e/screenshots/${testInfo.project.name}/${slug}.png`,
      fullPage: true
    });

    const { scrollWidth, clientWidth } = await page.evaluate(() => ({
      scrollWidth: Math.max(document.documentElement.scrollWidth, document.body.scrollWidth),
      clientWidth: document.documentElement.clientWidth
    }));

    expect(
      scrollWidth,
      `${route} overflows by ${scrollWidth - clientWidth}px. Offenders:\n` +
        (await overflowingElements(page)).join('\n')
    ).toBeLessThanOrEqual(clientWidth + 1);
  });
}

test('the shell switches layout at the md breakpoint', async ({ page }, testInfo) => {
  await visit(page, '/friends');

  const sidebar = page.getByTestId('sidebar');
  const tabBar = page.getByTestId('bottom-tab-bar');

  // Tailwind's `md:` is min-width 768px, so tablet-768 gets the desktop shell.
  if (testInfo.project.name === 'mobile-402') {
    await expect(tabBar).toBeVisible();
    await expect(sidebar).toBeHidden();
  } else {
    await expect(sidebar).toBeVisible();
    await expect(tabBar).toBeHidden();
  }
});

test('content clears the fixed bottom tab bar', async ({ page }, testInfo) => {
  test.skip(
    testInfo.project.name !== 'mobile-402',
    'the tab bar only exists below the md breakpoint'
  );

  await visit(page, '/friends');

  const bar = page.getByTestId('bottom-tab-bar');
  await expect(bar).toBeVisible();
  const barBox = await bar.boundingBox();
  expect(barBox).not.toBeNull();

  // The bar is `fixed`, so it sits outside flow and cannot push content up.
  // `main`'s bottom padding is the only thing keeping the last row reachable.
  const paddingBottom = await page
    .locator('main')
    .first()
    .evaluate((el) => parseFloat(getComputedStyle(el).paddingBottom));

  expect(
    paddingBottom,
    `main has ${paddingBottom}px bottom padding but the tab bar is ` +
      `${barBox!.height}px tall, so the last row sits under it`
  ).toBeGreaterThanOrEqual(barBox!.height);
});

test('every tab target is big enough to hit', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'mobile-402', 'mobile shell only');

  await visit(page, '/friends');

  const buttons = page.getByTestId('bottom-tab-bar').getByRole('button');
  const count = await buttons.count();
  expect(count).toBe(5);

  for (let i = 0; i < count; i++) {
    const box = await buttons.nth(i).boundingBox();
    expect(box).not.toBeNull();
    // 44px is the long-standing iOS guideline and the usual floor.
    expect(box!.height, `tab ${i} is only ${box!.height}px tall`).toBeGreaterThanOrEqual(44);
  }
});

/**
 * Catches the "invisible control" class of bug generally, rather than one
 * instance of it: a button whose text colour matches the background it is
 * actually painted on.
 *
 * This is how `bg-discord-blurple` hid for so long. The colour was never
 * defined, so the class compiled to nothing; the button kept its
 * `text-white` and rendered white-on-white, appearing only on hover where
 * a real `hover:bg-blue-600` took over.
 */
test('no control is invisible against its own background', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop-1280', 'colours do not vary by viewport');

  const offenders: string[] = [];

  for (const route of ROUTES_UNDER_TEST) {
    await visit(page, route);

    const bad = await page.evaluate(() => {
      // Must handle oklch as well as rgb: Chrome returns colours authored
      // as oklch() unchanged, and the page ground is one of them. An
      // rgb-only parser returns null for it, the ancestor walk below finds
      // nothing painted, and every element gets skipped - which is exactly
      // why the first version of this check passed while the bug it was
      // written for was still present. (theme.spec.ts has the same parser
      // for the same reason; keep them in step.)
      const lum = (c: string): number | null => {
        // oklab too: a running transition reports interpolated colours,
        // and Chrome interpolates in oklab.
        const ok = c.match(/^okl(?:ch|ab)\(\s*([\d.]+)(%?)/i);
        if (ok) {
          const l = Number(ok[1]);
          return ok[2] === '%' ? l / 100 : l;
        }
        const m = c.match(/^rgba?\(([^)]+)\)/i);
        if (!m) return null;
        const parts = m[1].split(/[\s,/]+/).map(Number);
        const [r, g, b] = parts;
        const alpha = parts.length > 3 ? parts[3] : 1;
        if (alpha === 0) return null; // transparent: not painted here
        return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
      };

      const out: string[] = [];
      for (const el of Array.from(document.querySelectorAll('button, a'))) {
        const rect = el.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) continue;

        const style = getComputedStyle(el);
        if (style.visibility === 'hidden' || style.opacity === '0') continue;

        const fg = lum(style.color);
        if (fg === null) continue;

        // The background it is actually painted on: its own, or the
        // nearest ancestor that paints one.
        let bg: number | null = null;
        let node: Element | null = el;
        while (node && bg === null) {
          bg = lum(getComputedStyle(node).backgroundColor);
          node = node.parentElement;
        }
        if (bg === null) continue;

        if (Math.abs(fg - bg) < 0.06) {
          out.push(
            `"${(el.textContent ?? '').trim().slice(0, 28)}" fg=${fg.toFixed(2)} bg=${bg.toFixed(2)} class="${String(el.className).slice(0, 60)}"`
          );
        }
      }
      return out;
    });

    offenders.push(...bad.map((b) => `${route}  ${b}`));
  }

  expect(
    offenders,
    `controls indistinguishable from their background:\n${offenders.join('\n')}`
  ).toEqual([]);
});
