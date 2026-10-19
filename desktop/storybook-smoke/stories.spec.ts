import { readFileSync } from 'node:fs';
import { test, expect } from '@playwright/test';

type IndexEntry = { id: string; title: string; name: string; type: 'story' | 'docs' };

function loadStories(): IndexEntry[] {
  const path = new URL('../storybook-static/index.json', import.meta.url);
  let raw: string;
  try {
    raw = readFileSync(path, 'utf8');
  } catch {
    throw new Error('storybook-static/index.json not found - run `yarn build:storybook` first.');
  }
  const { entries } = JSON.parse(raw) as { entries: Record<string, IndexEntry> };
  return Object.values(entries).filter((entry) => entry.type === 'story');
}

const stories = loadStories();

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
