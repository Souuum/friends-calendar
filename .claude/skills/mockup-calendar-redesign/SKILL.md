---
name: mockup-calendar-redesign
description: Rebuild the desktop Calendar screen (month + week) to match the "new calendar design" in the Friends Calendar Mockups project - a Free-tonight bar, functional filter chips, and a persistent side peek-panel with inline RSVP, replacing today's hover-tooltip/modal interaction. Backend-free - uses only endpoints that already exist. Use when asked to implement the redesigned calendar or its Free-tonight/peek-panel UI.
---

# Calendar redesign: Free-tonight bar, filters, peek panel

Source: Claude Design project `Friends Calendar Mockups.dc.html`
(`0b825812-c8f2-4f1a-98d7-38900c6a0133`), the `calendar` screen. Executed
first (alongside `mockup-responsive-shell`) because it needs **no new
backend** - `GET /api/availability/friends-now` and
`GET /api/availability/week` already exist (`services::availability`,
`handlers::availability`, built by the earlier `mockup-availability` skill)
but were never wired into the Calendar screen itself, only into the
Friends directory and friend-detail page. `POST /api/events/:id/participation`
already backs RSVP changes via `api.updateParticipation`.

## What's missing today (confirmed via grep - none of this exists)

`desktop/src/lib/components/templates/Calendar.svelte` renders a
`CalendarHeader` (title/prev/next/today/view-switcher/+New Event) then
`MonthView`/`WeekView`/`DayView`, with month using a hover `EventTooltip`
and week/day opening `EventDetailsModal` (a centered dialog) on click.
There is no Free-tonight bar, no working filter chips, and no side panel.

## What to build

1. **Free-tonight bar** (new section between `CalendarHeader` and the
   grid, styled like the mockup's rounded bordered bar):
   - `api.getFreeFriendsNow()` on mount (same call `/friends` already
     makes) → avatar stack of free friends + "N friends have nothing on"
     text.
   - "Best overlap this week" text: **do not** add a new backend
     aggregate endpoint for this. Derive it client-side, reusing
     `api.getWeekAvailability(friendId, weekStart)` per friend (already
     used by `/friends/[id]`) would mean N calls - instead, compute the
     day with the largest `free_user_ids` overlap from a single call per
     friend already loaded that session is overkill for a first pass.
     Simplest honest option matching this codebase's existing
     "derive from what's already fetched" pattern (see `/friends`'s
     `noteFor`): use `getFreeFriendsNow()`'s count as "N free right now"
     and skip inventing a fabricated "best overlap" figure - the mockup's
     exact copy is cosmetic flavor text, not a real computed guarantee to
     promise falsely. Flag this simplification in the PR/commit, don't
     silently claim a number that isn't real.
   - "Propose a time" button: opens `CreateEventModal` (via a callback
     prop or event dispatch already flowing through `CalendarHeader`) -
     no pre-fill logic needed beyond opening it; don't invent a
     time-suggestion algorithm for this pass.

2. **Functional filter chips** (All events / Going / Awaiting my response /
   Mine): client-side filter over the `events` prop already passed into
   `Calendar.svelte` - `Going` = `my_status === 'accepted'`, `Awaiting` =
   `my_status === 'pending'` or undefined, `Mine` = `is_creator`. Filtering
   changes what `MonthView`/`WeekView`/`DayView` receive, not a new fetch.

3. **`EventPeekPanel.svelte`** (new, `organisms/`) - a persistent
   296px-wide `aside` next to the grid (month **and** week/day, replacing
   `EventDetailsModal` at `md:` and up - see `mockup-responsive-calendar`
   for what happens below `md:`, not built in this pass):
   - Presentational: takes a selected `EventWithParticipants | null` prop,
     renders title/time/place/price/status pill, and (if `!is_creator`)
     inline Going/Maybe/Can't buttons calling `api.updateParticipation`
     then dispatching a `refresh` event upward (mirrors
     `EventDetailsModal.svelte`'s existing `handleStatusChange` pattern -
     read that file for the exact call shape before writing a new one).
   - Empty state when nothing's selected yet (mockup shows this too -
     don't render an empty aside with nothing in it).
   - `MonthView`/`WeekView`/`DayView`'s click handlers now set
     `selectedEvent` in `Calendar.svelte` instead of opening
     `EventDetailsModal` - keep `EventDetailsModal.svelte` itself, it's
     reused as the mobile bottom sheet by `mockup-responsive-calendar`,
     don't delete it.

## Tests

Component test for `EventPeekPanel.svelte` (presentational, props-driven
like `LinkedServerCard.svelte`/`AnnouncementPostCard.svelte` - no `$lib/api`
mocking needed for the render itself, only for the RSVP button clicks).
A `Calendar.svelte` test covering the free-tonight bar rendering and that
filter chips actually change which events are visible - mock `$lib/api`'s
`getFreeFriendsNow` the same way `friends/page.test.ts` does. Full
conventions in `.claude/skills/add-tests/SKILL.md`.

## Verification

`yarn test`, `yarn run check`, `yarn build` from `desktop/`. No backend
changes, no `cargo` verification needed.
