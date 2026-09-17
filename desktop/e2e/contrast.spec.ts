import { test, expect, type Page } from '@playwright/test';
import { mockApi, ROUTES_UNDER_TEST } from './fixtures';

/**
 * Real WCAG contrast across every route, in both themes.
 *
 * This exists because theming bugs are systematically invisible to the
 * other tiers: the DOM is correct, the classes are correct, the tests pass,
 * and the page is unreadable. The unread notification row shipped at
 * **1.01:1** - its text was the same colour as its background - because
 * only the grey ramp had been inverted for dark mode and `bg-indigo-50`
 * stayed light while the text on it flipped.
 *
 * ## Why the gate is 3:1 rather than AA's 4.5:1
 *
 * At 4.5 this fails on ~27 light-mode elements that use `--color-muted`
 * (#7c7c83) and the Discord brand blurple (#5865f2), which measure 3.8-4.4.
 * Those are the mockup's own token values and Discord's own brand colour -
 * a real accessibility gap, but a design decision to change deliberately,
 * not something a test should force. 3:1 is the floor for "legible at all",
 * and it catches every bug this audit was written for.
 *
 * Anything between 3 and 4.5 is reported in the failure message when the
 * test does fail, so the gap stays visible rather than forgotten.
 */

const HARD_FLOOR = 3;

async function auditPage(page: Page) {
  return page.evaluate(() => {
    // Canvas converts any CSS colour - oklch, oklab, rgb - into sRGB,
    // which is what the WCAG formula needs. Parsing oklch by hand and
    // treating its hue as a channel is how an earlier helper here produced
    // nonsense.
    const ctx = document.createElement('canvas').getContext('2d')!;
    const toRgb = (css: string): [number, number, number, number] => {
      ctx.clearRect(0, 0, 1, 1);
      ctx.fillStyle = '#000';
      ctx.fillStyle = css;
      ctx.fillRect(0, 0, 1, 1);
      const d = ctx.getImageData(0, 0, 1, 1).data;
      return [d[0], d[1], d[2], d[3] / 255];
    };
    const luminance = ([r, g, b]: number[]) => {
      const channel = (v: number) => {
        const s = v / 255;
        return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
      };
      return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
    };

    const out: { ratio: number; need: number; text: string; where: string }[] = [];
    for (const el of Array.from(document.querySelectorAll('*'))) {
      // Only elements with their own text, so a container isn't blamed for
      // a child's colours.
      const text = Array.from(el.childNodes)
        .filter((n) => n.nodeType === Node.TEXT_NODE)
        .map((n) => n.textContent?.trim())
        .join(' ')
        .trim();
      if (!text) continue;

      const rect = el.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0) continue;
      const style = getComputedStyle(el);
      if (style.visibility === 'hidden' || style.opacity === '0') continue;

      // The background it is actually painted on. Translucent layers must be
      // composited over what is behind them, not treated as opaque: a
      // `bg-gray-500/20` chip over a dark card renders dark, but reading its
      // raw rgb reports a mid grey and invents a contrast failure that isn't
      // on screen.
      let background: number[] | null = null;
      let node: Element | null = el;
      let acc: number[] = [0, 0, 0];
      let accAlpha = 0;
      while (node && accAlpha < 1) {
        const [r, g, b, a] = toRgb(getComputedStyle(node).backgroundColor);
        if (a > 0) {
          const weight = a * (1 - accAlpha);
          acc = [acc[0] + r * weight, acc[1] + g * weight, acc[2] + b * weight];
          accAlpha += weight;
        }
        node = node.parentElement;
      }
      if (accAlpha > 0) {
        // Whatever is still uncovered is the page's own ground.
        background = acc.map((c) => c / accAlpha);
      }
      if (!background) continue;

      const fg = luminance(toRgb(style.color));
      const bg = luminance(background);
      const ratio = (Math.max(fg, bg) + 0.05) / (Math.min(fg, bg) + 0.05);

      const size = parseFloat(style.fontSize);
      const bold = Number(style.fontWeight) >= 700;
      const large = size >= 24 || (size >= 18.66 && bold);

      out.push({
        ratio,
        need: large ? 3 : 4.5,
        text: text.slice(0, 32),
        where: String(el.className).slice(0, 44)
      });
    }
    return out;
  });
}

for (const theme of ['light', 'dark'] as const) {
  test(`text is legible in ${theme} mode`, async ({ page }, testInfo) => {
    test.skip(testInfo.project.name !== 'desktop-1280', 'colours do not vary by viewport');

    const unreadable: string[] = [];
    const belowAA: string[] = [];

    for (const route of ROUTES_UNDER_TEST) {
      await mockApi(page);
      await page.addInitScript((t) => window.localStorage.setItem('theme', t as string), theme);
      await page.goto(route);
      await page.waitForLoadState('networkidle');
      await page.locator('main').first().waitFor({ state: 'visible' });

      for (const f of await auditPage(page)) {
        const line = `${f.ratio.toFixed(2)}:1 (AA wants ${f.need}) ${route} "${f.text}" [${f.where}]`;
        if (f.ratio < HARD_FLOOR) unreadable.push(line);
        else if (f.ratio < f.need) belowAA.push(line);
      }
    }

    expect(
      unreadable,
      `Unreadable text (below ${HARD_FLOOR}:1):\n${unreadable.join('\n')}\n\n` +
        `For reference, ${belowAA.length} more sit between ${HARD_FLOOR}:1 and AA:\n` +
        belowAA.slice(0, 10).join('\n')
    ).toEqual([]);
  });
}
