import { readFileSync } from 'node:fs';
import { test, expect } from '@playwright/test';

type IndexEntry = { id: string; title: string; name: string; type: 'story' | 'docs' };

function loadIndex(): IndexEntry[] {
  const path = new URL('../storybook-static/index.json', import.meta.url);
  let raw: string;
  try {
    raw = readFileSync(path, 'utf8');
  } catch {
    throw new Error('storybook-static/index.json not found - run `yarn build:storybook` first.');
  }
  const { entries } = JSON.parse(raw) as { entries: Record<string, IndexEntry> };
  return Object.values(entries);
}

const index = loadIndex();
const stories = index.filter((entry) => entry.type === 'story');
const docs = index.filter((entry) => entry.type === 'docs');
const storyIds = new Set(stories.map((story) => story.id));

test('the build contains stories', () => {
  // Guards the loop below: an empty index would otherwise pass vacuously.
  expect(stories.length).toBeGreaterThan(0);
});

for (const story of stories) {
  test(`${story.title} › ${story.name}`, async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => {
      if (message.type() === 'error') errors.push(message.text());
    });

    await page.goto(`/iframe.html?id=${story.id}&viewMode=story`);

    // Storybook swaps a class on <body> once it has either rendered the
    // story or given up on it, so wait for whichever comes first.
    await page.waitForFunction(() =>
      ['sb-show-main', 'sb-show-errordisplay', 'sb-show-nopreview'].some((c) =>
        document.body.classList.contains(c)
      )
    );
    // Components that fetch in onMount throw after the first paint, so give
    // those promises a moment to settle before judging.
    await page.waitForTimeout(300);

    const shownError = await page.evaluate(() =>
      document.body.classList.contains('sb-show-errordisplay')
        ? (document.querySelector('#error-message')?.textContent ?? 'unknown error')
        : null
    );

    expect(shownError, 'Storybook showed its error screen').toBeNull();
    expect(errors, 'errors were thrown while rendering').toEqual([]);
  });
}

for (const doc of docs) {
  test(`${doc.title} (docs page)`, async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => {
      if (message.type() === 'error') errors.push(message.text());
    });

    await page.goto(`/iframe.html?id=${doc.id}&viewMode=docs`);
    await expect(page.locator('#storybook-docs h1').first()).toBeVisible();

    // Links between pages are plain `?path=/story/<id>` hrefs, which nothing
    // else checks: renaming a story would leave a dead link on the homepage.
    const linked = await page
      .locator('#storybook-docs a[href^="?path=/story/"]')
      .evaluateAll((links) =>
        links.map((link) => link.getAttribute('href')!.replace('?path=/story/', ''))
      );
    expect(
      linked.filter((id) => !storyIds.has(id)),
      'links to stories that do not exist'
    ).toEqual([]);
    expect(errors, 'errors were thrown while rendering').toEqual([]);
  });
}
