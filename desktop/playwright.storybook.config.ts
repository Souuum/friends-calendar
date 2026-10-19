import { defineConfig, devices } from '@playwright/test';

/**
 * Renders every Storybook story in a real browser and fails on any that
 * throws.
 *
 * `yarn run check` type-checks the stories and `build:storybook` compiles
 * them, but neither notices a story that crashes on mount - a store passed
 * where a value was expected, a stub returning the wrong shape at runtime.
 * Only rendering it does.
 *
 * Runs against the static build rather than `storybook dev`: it's what gets
 * deployed, and it serves in a second instead of a cold Vite start. The
 * build has to exist before Playwright starts, because the spec reads
 * `storybook-static/index.json` to generate one test per story - hence
 * `yarn test:storybook` builds first.
 */

const PORT = 6008;

export default defineConfig({
  testDir: './storybook-smoke',
  outputDir: './storybook-smoke/.results',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  reporter: process.env.CI ? [['github'], ['list']] : 'list',

  use: {
    baseURL: `http://localhost:${PORT}`,
    ...devices['Desktop Chrome'],
    viewport: { width: 1280, height: 900 }
  },

  webServer: {
    command: `python3 -m http.server ${PORT} --directory storybook-static`,
    url: `http://localhost:${PORT}/index.json`,
    reuseExistingServer: !process.env.CI,
    timeout: 30_000
  }
});
