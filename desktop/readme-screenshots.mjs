// Regenerates the screenshots in docs/screenshots/ for the README.
//
//   cd desktop && yarn build && yarn preview --port 4173 &
//   npx tsx readme-screenshots.mjs
//
// Uses e2e/fixtures.ts, so the content is the same deterministic sample data
// the layout tests run against - no backend, no database, and nobody's real
// events in a public screenshot.
import { chromium } from '@playwright/test';
import { mockApi } from './e2e/fixtures.ts';

const OUT = '/Users/soum/Documents/DevStuff/rust-friends-calendar/docs/screenshots';
const browser = await chromium.launch();

async function shot(name, path, { width, height, theme = 'light', after } = {}) {
  const page = await browser.newPage({
    viewport: { width: width ?? 1440, height: height ?? 960 },
    deviceScaleFactor: 2
  });
  await mockApi(page);
  await page.addInitScript((t) => window.localStorage.setItem('theme', t), theme);
  await page.goto('http://localhost:4173' + path);
  await page.waitForTimeout(900);
  if (after) await after(page);
  await page.waitForFunction(() =>
    document.getAnimations().every((a) => {
      const it = a.effect?.getTiming?.().iterations;
      return it === Infinity || a.playState === 'finished';
    })
  );
  await page.screenshot({ path: `${OUT}/${name}.png` });
  await page.close();
  console.log('shot', name);
}

await shot('calendar', '/');
await shot('calendar-dark', '/', { theme: 'dark' });
await shot('friends', '/friends');
await shot('announcements', '/announcements');
await shot('settings', '/settings');
await shot('event-detail', '/', {
  after: async (p) => {
    await p.getByText('Soirée jeux de société chez Hugo', { exact: false }).first().click();
    // Off the grid: hovering a day cell also raises the tooltip, which
    // would overlap the peek panel in the shot.
    await p.mouse.move(1400, 120);
    await p.waitForTimeout(800);
  }
});
await shot('mobile-calendar', '/', {
  width: 402,
  height: 820,
  // The list view is what makes the calendar usable at 402px - a 7-column
  // grid gets ~52px per day - so it is the honest mobile showcase.
  after: async (p) => {
    await p.getByRole('button', { name: 'List' }).click();
    await p.waitForTimeout(500);
  }
});
await shot('mobile-announcements', '/announcements', { width: 402, height: 820 });
await browser.close();
