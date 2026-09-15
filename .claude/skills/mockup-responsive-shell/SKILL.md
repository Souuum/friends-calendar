---
name: mockup-responsive-shell
description: Establish the responsive app shell - a bottom tab bar replacing the sidebar below the md breakpoint, matching MobileTabBar.dc.html from the mobile mockup - and the breakpoint convention every other responsive skill reuses. Backend-free. Use when asked to make the app shell/navigation responsive, or before starting any other mockup-responsive-* skill.
---

# Responsive app shell: bottom tab bar

Source: Claude Design project `Friends Calendar Mockups` (`0b825812-c8f2-4f1a-98d7-38900c6a0133`),
files `Friends Calendar Mobile.dc.html` (13 screens, 402x874 reference) and
`MobileTabBar.dc.html` (the shared 5-tab bar component every mobile screen
imports). **Run this before any other `mockup-responsive-*` skill** - they
all assume the breakpoint convention and tab-bar visibility this one sets
up; there's nothing to reveal a page *into* on mobile until this exists.

## Breakpoint convention

This app has **zero** responsive styling today (confirmed via grep - no
`@media`, no `sm:`/`md:` Tailwind prefixes anywhere in `desktop/src`).
Pick Tailwind's `md:` (768px) as the single breakpoint: sidebar/desktop
layout at `md:` and up, bottom-tab-bar/mobile layout below it. The
mockup's reference width is 402px, comfortably under `md:`, so there's no
need for a narrower `sm:` tier unless a later skill finds a specific
component breaking between 402px and 768px - don't add breakpoints
speculatively.

## What to build

1. **`BottomTabBar.svelte`** (new, `templates/`, alongside `Frame.svelte`) -
   mirrors `MobileTabBar.dc.html`: 5 tabs, each a pip indicator + label
   (+ badge on Alerts). Map today's `Frame.svelte` `navItems` to the
   mockup's tab labels/routes:
   - Calendar → `/` (mockup label "Calendar")
   - Friends → `/friends` ("Friends")
   - Hub → `/announcements` ("Hub" - the mockup's name for the
     Discord-message-mirror feed; keep the route as-is, just relabel the
     tab)
   - Alerts → `/notifications` ("Alerts"), badge = `$unreadNotificationCount`
     (already a store, reused as-is)
   - Me → `/settings` ("Me" - the mockup folds Settings **and** Discord
     server under one "Me" tab, with Server reached by drilling in from
     Settings, not a 6th tab. Don't add a 6th tab or a separate Server
     entry here - `mockup-responsive-settings-and-server` handles making
     `/server` reachable from within `/settings` on mobile.)
   - Highlight/active state: same `$page.url.pathname` matching
     `Frame.svelte` already uses for the sidebar - reuse that logic
     rather than duplicating it.

2. **`Frame.svelte`** - hide the existing `<aside>` sidebar below `md:`
   (`hidden md:flex` or equivalent), render `<BottomTabBar>` fixed to the
   bottom of the viewport below `md:` only (`md:hidden`), and add bottom
   padding to `<main>` below `md:` so content doesn't sit under the fixed
   bar. `Header.svelte` (top bar with avatar/logout) stays but should be
   checked for overflow at 402px - a quick pass, not a redesign, since the
   mockup's mobile screens show a simpler per-page header (e.g. "‹ Back"),
   which individual `mockup-responsive-*` skills handle per-page, not here.

3. Don't touch route content in this skill - it's shell-only. Every page
   should render exactly as it does today at `md:` and up; only the
   nav chrome changes, and only below `md:`.

## Tests

`BottomTabBar.test.ts` mirrors `Frame.test.ts`'s pattern exactly (settable
`$app/stores` mock via `__setPathname`, `$app/navigation`'s `goto` mocked,
`$lib/api`'s `getUnreadNotificationCount` mocked) - assert clicking a tab
navigates, and the active tab highlights based on `$page.url.pathname`.
Update `Frame.test.ts` if needed to confirm the sidebar/tab-bar swap
doesn't break the existing sidebar-navigation assertions (they should still
pass unchanged at the default/desktop test viewport - `happy-dom` doesn't
evaluate CSS media queries, so `hidden md:flex` classes existing doesn't
require any test changes only if no test currently asserts visibility by
CSS; if one does, adjust it to check for the element's presence, not
computed visibility).

## Verification

`yarn test`, `yarn run check`, `yarn build`. Manually resize a browser
window (or Tauri dev window) below/above 768px to confirm the swap - CSS
breakpoint behavior isn't exercised by `happy-dom` component tests. No
backend changes.
