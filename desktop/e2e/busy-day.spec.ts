import { test, expect, type Page } from '@playwright/test';
import { mockApi } from './fixtures';

/**
 * Tapping a month day that holds only imported busy time.
 *
 * Reported as "external events from Google / Teams calendars aren't shown":
 * on a phone the month cell is too narrow to read, so the list under the
 * grid is where a day is actually read - and it used to say "Nothing on this
 * day" for a day full of meetings. The component tests cover the DOM; this
 * covers what a person sees at real widths, in both themes.
 */

/** A day cell that the shared fixtures leave empty of app events. */
function quietDay(page: Page) {
  return page.locator('[role="button"][aria-pressed]').nth(2);
}

/** Two anonymous blocks, 09-10 and 14-15 local, on the given `YYYY-MM-DD`. */
function busyOn(date: string) {
  const [y, m, d] = date.split('-').map(Number);
  const at = (h: number) => new Date(y, m - 1, d, h, 0, 0).toISOString();
  return [
    { starts_at: at(9), ends_at: at(10) },
    { starts_at: at(14), ends_at: at(15) }
  ];
}

async function openQuietDay(page: Page, theme: 'light' | 'dark') {
  await mockApi(page);
  await page.addInitScript((t) => window.localStorage.setItem('theme', t as string), theme);
  await page.goto('/');
  const date = await quietDay(page).getAttribute('data-date');
  expect(date, 'the grid cell carries its date').toBeTruthy();

  // Registered after mockApi, so it wins for this one endpoint.
  await page.route('**/api/calendar/external/busy**', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(busyOn(date!))
    })
  );
  await page.reload();
  await page.waitForLoadState('networkidle');
  await quietDay(page).click();
  // Any entrance animation has to settle before measuring.
  await page.waitForFunction(() =>
    document
      .getAnimations()
      .every((a) => a.effect?.getTiming().iterations === Infinity || a.playState === 'finished')
  );
}

/** WCAG ratios for every text-bearing element inside `root`. */
async function contrastIn(page: Page, selector: string) {
  return page.evaluate((sel) => {
    const ctx = document.createElement('canvas').getContext('2d')!;
    const toRgb = (css: string): [number, number, number, number] => {
      ctx.clearRect(0, 0, 1, 1);
      ctx.fillStyle = '#000';
      ctx.fillStyle = css;
      ctx.fillRect(0, 0, 1, 1);
      const d = ctx.getImageData(0, 0, 1, 1).data;
      return [d[0], d[1], d[2], d[3] / 255];
    };
    const lum = ([r, g, b]: number[]) => {
      const c = (v: number) => {
        const s = v / 255;
        return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
      };
      return 0.2126 * c(r) + 0.7152 * c(g) + 0.0722 * c(b);
    };
    const out: { ratio: number; text: string }[] = [];
    for (const el of Array.from(document.querySelector(sel)!.querySelectorAll('*'))) {
      const text = Array.from(el.childNodes)
        .filter((n) => n.nodeType === Node.TEXT_NODE)
        .map((n) => n.textContent?.trim())
        .join(' ')
        .trim();
      if (!text) continue;
      let node: Element | null = el;
      let acc = [0, 0, 0];
      let alpha = 0;
      while (node && alpha < 1) {
        const [r, g, b, a] = toRgb(getComputedStyle(node).backgroundColor);
        if (a > 0) {
          const w = a * (1 - alpha);
          acc = [acc[0] + r * w, acc[1] + g * w, acc[2] + b * w];
          alpha += w;
        }
        node = node.parentElement;
      }
      if (alpha === 0) continue;
      const fg = lum(toRgb(getComputedStyle(el).color));
      const bg = lum(acc.map((v) => v / alpha));
      out.push({
        ratio: (Math.max(fg, bg) + 0.05) / (Math.min(fg, bg) + 0.05),
        text: text.slice(0, 40)
      });
    }
    return out;
  }, selector);
}

for (const theme of ['light', 'dark'] as const) {
  test(`a tapped day with only imported busy time lists it (${theme})`, async ({
    page
  }, testInfo) => {
    await openQuietDay(page, theme);

    const list = page.getByTestId('selected-day-busy');
    await expect(list).toBeVisible();
    await page.screenshot({
      path: `e2e/screenshots/${testInfo.project.name}/busy-day-${theme}.png`,
      fullPage: true
    });
    // From `md:` up the content column scrolls on its own, so a full-page
    // shot stops at the fold; this one is of the list itself.
    await list.locator('xpath=ancestor::section[1]').screenshot({
      path: `e2e/screenshots/${testInfo.project.name}/busy-day-list-${theme}.png`
    });

    await expect(list.locator(':scope > *')).toHaveCount(2);
    await expect(list).toContainText('Busy');
    await expect(page.getByText(/Nothing on this day/)).toHaveCount(0);

    // Not interactive: no detail exists for an anonymous block.
    await expect(list.locator('button, a, [role="button"], [tabindex]')).toHaveCount(0);

    // No horizontal overflow at this width.
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth
    );
    expect(overflow, 'page is wider than the viewport').toBeLessThanOrEqual(0);

    // Legible: the same 3:1 floor contrast.spec.ts gates on.
    const poor = (await contrastIn(page, '[data-testid="selected-day-busy"]')).filter(
      (f) => f.ratio < 3
    );
    expect(poor, JSON.stringify(poor)).toEqual([]);
  });
}
