import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import  tailwindcss  from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [sveltekit(), tailwindcss()],
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  // Svelte ships separate SSR/browser builds; without forcing the browser
  // condition under Vitest, component tests would run against the SSR
  // build (no DOM lifecycle) instead of the client one.
  resolve: {
    conditions: process.env.VITEST ? ["browser"] : undefined,
  },
  test: {
    environment: "happy-dom",
    globals: true,
    setupFiles: ["./vitest-setup.js"],
    // Playwright owns e2e/. Without this, vitest's default include pattern
    // matches e2e/*.spec.ts and fails on the @playwright/test import - and
    // those tests need a real browser, which is the whole reason they're
    // not in this tier.
    exclude: ["**/node_modules/**", "**/build/**", "**/.svelte-kit/**", "e2e/**"],
  },
}));
