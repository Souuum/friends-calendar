---
name: mockup-availability
description: Implement cross-user free/busy computation from the Friends Calendar Mockups project - the calendar's "Free tonight" bar, the friend-detail weekly availability heatmap, "best overlap this week", and "Propose a time". Not yet executed as of 2026-09-15 - a genuinely new capability other skills reference but don't duplicate. Use when asked to build availability, free/busy, or "who's free" features.
---

# Cross-user availability

Source: the "Free tonight" bar on the Calendar screen (`isCalendar`), the
"Free this week" strip on Friend detail (`isFriend`), and "Propose a time"
in both places, in `Friends Calendar Mockups.dc.html`. **Not yet executed.**
`.claude/skills/mockup-friends-directory/SKILL.md` explicitly deferred its
availability-shaped UI to this skill rather than faking it - implement this
before revisiting those omissions.

## What "free" means here

A user is "free" at time T if they have no `event_participants` row with
`status IN ('accepted', 'maybe')` on an event whose `[start_time, end_time)`
covers T, for events they're actually going to (declined/pending doesn't
block). This is computable from existing tables (`calendar_events` +
`event_participants`) - **no new tables needed**, this is a query/algorithm
skill, not a schema one.

## Backend

- `services::availability` (new): `free_slots_for_users(db, user_ids: &[Uuid], from: DateTime<Utc>, to: DateTime<Utc>) -> Result<...>` -
  fetch all `accepted`/`maybe` events for the given users in the range
  (one query with `user_id = ANY($1)`, joined through `event_participants`),
  then compute per-user busy intervals and their union/intersection over
  the window. Keep the interval math in plain Rust (sort + merge), not SQL -
  much easier to unit-test as pure logic (see the "Unit" tier in
  `.claude/skills/add-tests/SKILL.md` - this is exactly the kind of thing
  to pull out into a testable pure function, interval merging has a lot of
  edge cases worth locking down with cheap `#[test]`s: back-to-back events,
  overlapping events, zero-length windows).
- Two consumption shapes:
  - "Who's free right now / tonight, among my friends" -
    `GET /api/availability/friends?at=<timestamp>` → which friends (from
    `services::friends::get_friends`) are free at that instant.
  - "Best overlap this week" / weekly heatmap - bucket a week into
    hour-or-coarser slots, count how many of a given user set are free in
    each slot, return the top slot(s) plus per-day/per-hour free counts for
    the heatmap. This is the more expensive computation - consider whether
    it needs caching/memoizing per user-set-plus-week, or if it's cheap
    enough to compute per request for a friend-group-sized app (probably
    fine to start simple and only add caching if it's actually slow).
- "Propose a time": once the best-overlap slot is known, this can just
  pre-fill `CreateEventModal.svelte`'s start/end fields and open it -
  doesn't need its own backend endpoint beyond the computation above.

## Frontend

- Calendar's "Free tonight" bar: consumed on `CalendarView.svelte`/`Calendar.svelte`.
- Friend detail's weekly strip: consumed on
  `desktop/src/routes/friends/[id]/+page.svelte` (from
  `mockup-friends-directory`, once that's landed) - pass the current user +
  that one friend's IDs to the friends-availability endpoint.
- `api.ts`: `getFriendsAvailability(at?)`, `getWeeklyOverlap(userIds, weekStart)`
  (naming illustrative - match whatever the backend ends up exposing).

## Tests

Backend: **unit tests are the priority here** - the interval-merging/free-slot
logic is pure and deserving of thorough `#[test]` coverage (adjacent
events, overlapping events, all-day vs. timed, empty input, single user vs.
many). Integration test for the endpoint's DB query shape via `#[sqlx::test]`
on top of that. Frontend: component tests for the free-tonight bar and
heatmap rendering given canned availability data (mock `$lib/api`). Full
guidance in `.claude/skills/add-tests/SKILL.md`.
