---
name: mockup-responsive-announcements
description: Make the Announcements/Hub feed responsive per mobile mockup screen 07 - pinned-post treatment, card feed styling, compose affordance restyled as a pill. Depends on mockup-responsive-shell; pairs with mockup-announcement-thread's "Open" link. Use when asked to make the announcements/hub feed responsive.
---

# Responsive announcements ("Hub")

Source: `Friends Calendar Mobile.dc.html` screen 07 (Hub · announcements).
**Requires** `mockup-responsive-shell` first. Independent of
`mockup-announcement-thread` (that skill adds the thread page this one
links to via "Open" - if it hasn't run yet, keep the "Open" affordance
pointing nowhere or hide it rather than 404ing, and note that in the PR).

## What to build

- `desktop/src/routes/announcements/+page.svelte`: below `md:`, the first
  **pinned** post (if any - `AnnouncementPostInfo.pinned`) gets the
  mockup's distinct dark/inverted card treatment instead of the same
  styling as every other post; the rest render as the existing card feed
  (already single-column, check padding at 402px).
- "Sync now" button: the mockup replaces this with a floating compose pill
  above the tab bar, but this app's sync is a pull (fetch from Discord),
  not a compose action - don't relabel "Sync now" as "compose," they're
  different operations. Keep an explicit "Sync now" control (pull-to-
  refresh gestures aren't reliably web-native), just restyle its
  position/appearance to fit the mobile layout rather than copying the
  mockup's compose-pill semantics verbatim.
- `AnnouncementPostCard.svelte`: verify avatar/tag-badge/reaction-count row
  doesn't wrap awkwardly at 402px.

## Tests

Extend `announcements/page.test.ts` only if the pinned-post treatment adds
new conditional rendering worth asserting on (e.g. a test that the first
pinned post gets a distinct class/element). Pure spacing/padding changes
don't need new tests.

## Verification

`yarn test`, `yarn run check`, `yarn build`, manual resize check. No
backend changes (this skill doesn't touch the reply/thread feature).
