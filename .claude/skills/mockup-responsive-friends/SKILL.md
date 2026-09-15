---
name: mockup-responsive-friends
description: Make the Friends directory and friend-detail pages responsive per mobile mockup screens 04-05 - single-column rows, restyled availability rail, invite button pinned to the bottom safe area. Depends on mockup-responsive-shell. Use when asked to make the friends pages responsive.
---

# Responsive friends: directory + detail

Source: `Friends Calendar Mobile.dc.html` screens 04 (Friends) and 05
(Friend profile). **Requires** `mockup-responsive-shell` first.

## What's already close

`desktop/src/routes/friends/+page.svelte` already renders a single grid
that collapses toward one column at narrow widths
(`grid-cols-1 sm:grid-cols-2 lg:grid-cols-3`) and `/friends/[id]` is
already its own route (back-navigation "for free," no shell change
needed). This skill is a smaller polish pass, not a rebuild.

## What to build

1. **`/friends`**: below `md:`, force single column explicitly (don't rely
   on the grid degrading - the mockup's rows are full-width list rows, not
   narrow cards) and restyle the "Free now" section into the mockup's
   rounded `var(--tint)`-style availability card (avatar stack + "N free
   tonight" + "Propose a time →" link, matching the Free-tonight bar's
   copy/behavior from `mockup-calendar-redesign` - reuse that component or
   its data-fetching if it was built as something reusable, don't
   re-implement `getFreeFriendsNow()` handling a second time).
   Search input and "+ Add friend"/"Sync friends" buttons stack instead of
   sitting inline at narrow widths.

2. **`/friends/[id]`**: below `md:`, the "Invite to an event" button
   (currently wherever it's placed today - check the file) pins to the
   bottom of the viewport in the safe-area padding the mockup shows,
   rather than scrolling with the page. Stats row, availability strip, and
   shared-events list already stack reasonably - verify at 402px width and
   fix any overflow (e.g. the availability strip's 7 day-columns need a
   minimum touch target width check), don't restructure what already
   works.

## Tests

Extend `friends/page.test.ts` and `friends/[id]/page.test.ts` only if
behavior changes (e.g. if the availability card is extracted into a new
component, add a test for it) - pure Tailwind class changes for layout
don't need new assertions beyond what already exists, per this repo's
convention of testing behavior, not CSS.

## Verification

`yarn test`, `yarn run check`, `yarn build`, manual resize check. No
backend changes.
