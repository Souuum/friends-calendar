import { defineConfig, devices } from '@playwright/test';

/**
 * Layout tests, run in a real browser.
 *
 * These exist because the vitest/happy-dom suite cannot do layout at all -
 * every geometry property comes back 0 there, even on an element with an
 * explicit width, so `hidden md:block` is just a string of characters to it.
 * That makes the whole responsive pass structurally unverifiable at that
 * tier, not merely untested.
 *
 * What belongs here: assertions a machine can make without judgement -
 * horizontal overflow, occlusion, whether a breakpoint actually switches.
 * What does NOT belong here: pixel-diffed screenshot baselines. Font
 * rendering differs between a macOS dev machine and CI's Linux container,
 * so committed baselines would fail in CI immediately and permanently.
 * Screenshots are written as artifacts to look at, never as a gate.
 */

const PORT = 4173;

export default defineConfig({
  testDir: './e2e',
  outputDir: './e2e/.results',
  fullyParallel: true,
  // A `.only` left in a commit would silently shrink the suite to one test.
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI
    ? [['list'], ['html', { open: 'never', outputFolder: 'e2e/.report' }]]
    : 'list',

  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: 'retain-on-failure'
  },

  // One project per viewport rather than resizing inside tests: a failure
  // then names the width it failed at, and the three run in parallel.
  // 402x874 is the mobile mockup's own reference size.
  projects: [
    {
      name: 'mobile-402',
      use: { ...devices['Desktop Chrome'], viewport: { width: 402, height: 874 } }
    },
    {
      name: 'tablet-768',
      use: { ...devices['Desktop Chrome'], viewport: { width: 768, height: 1024 } }
    },
    {
      name: 'desktop-1280',
      use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 900 } }
    }
  ],

  // adapter-static with `fallback: 'index.html'` means preview serves the
  // built SPA and client-side routing works, so no backend is needed - the
  // API is mocked per-test at the network layer instead.
  webServer: {
    command: `yarn build && yarn preview --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 180_000
  }
});
