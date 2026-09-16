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
