---
name: mockup-responsive-calendar
description: Make the Calendar screen responsive per mobile mockup screens 01-03 (month, agenda/list, event bottom sheet) - agenda view toggle, and EventDetailsModal becoming a bottom sheet below md. Depends on mockup-calendar-redesign and mockup-responsive-shell. Use when asked to make the calendar/event-detail screens responsive.
---

# Responsive calendar: agenda view + bottom sheet

Source: `Friends Calendar Mobile.dc.html` screens 01 (Calendar · month), 02
(Calendar · agenda), 03 (Event · sheet). **Requires**
`mockup-responsive-shell` (bottom tab bar + `md:` convention) and
`mockup-calendar-redesign` (`EventPeekPanel.svelte`, Free-tonight bar,
filter chips) to already be done - this skill adapts their output for
narrow viewports rather than building calendar UI from scratch.

## What to build

1. **Month view stays a grid below `md:`** (screen 01) - no structural
   change to `MonthView.svelte`'s grid itself, just narrower cell padding/
   font sizes if they overflow at 402px (check, don't assume). The
   Free-tonight bar from `mockup-calendar-redesign` shortens its copy
   below `md:` to fit (mockup: "5 friends free tonight" short form, not
   the full desktop sentence).

2. **`EventPeekPanel` becomes an inline agenda list below `md:`** instead
   of a side `aside` (no room at 402px) - selecting a day shows that day's
   events as a list directly below the grid (mockup screen 01's "Tuesday
   15 · 2 events" section), not as a separate panel. Reuse
   `EventPeekPanel`'s RSVP-button logic per list item rather than
   duplicating it - extract the inline-RSVP-buttons bit into a small
   sub-component if `EventPeekPanel` doesn't already separate presentation
   from data-fetching cleanly (check its shape from
   `mockup-calendar-redesign` before deciding whether to extract).

3. **Grid/List toggle** (screen 02, next to the view switcher, replaces
   Month/Week tabs below `md:` with Grid/List) - "List" mode groups all
   upcoming events by day (reuse the exact grouping logic
   `/friends/[id]`'s shared-events section or `/announcements`'s date
   handling already uses if applicable, don't reinvent date-grouping).
   Each row keeps the existing pill/status styling from
   `EventDetailsModal`/`EventPeekPanel`'s status-color helpers - extract
   those into a shared util if they're currently private to one component,
   rather than copy-pasting the color-mapping switch statement a third
   time (it already exists in at least `EventRsvpCard.svelte` and
   `AnnouncementPostCard.svelte`'s tag styling - check
   `EventCardParticipant.svelte`/`EventDetailsModal.svelte` for the
   canonical status→color mapping first).

4. **`EventDetailsModal.svelte` becomes a bottom sheet below `md:`** - CSS
   only: `fixed inset-x-0 bottom-0` instead of centered, rounded top
   corners, no change to its internal content/logic. This is the same
   component used both here and reached via the agenda list's rows -
   don't fork it into two components for desktop-modal vs mobile-sheet,
   condition the positioning classes on the `md:` breakpoint.

## Tests

Extend `Calendar.svelte`'s test suite (from `mockup-calendar-redesign`)
with cases for the Grid/List toggle actually changing what's rendered.
`EventDetailsModal.test.ts` (if one doesn't exist yet, check first) should
assert the sheet/modal renders its content regardless of viewport class
(component tests don't evaluate real CSS breakpoints, so the useful
assertion is "the responsive classes are present," not "it visually looks
like a sheet at 402px" - note that limitation in the test file's comments
rather than pretending to verify something `happy-dom` can't check).

## Verification

`yarn test`, `yarn run check`, `yarn build`, plus a manual resize check
(same caveat as `mockup-responsive-shell` - CSS breakpoints aren't testable
via component tests). No backend changes.
