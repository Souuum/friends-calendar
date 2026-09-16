---
name: event-visibility-listing
description: Make the visibility field (private/friends/public) actually affect which events people see. Today GET /api/events only returns events you were explicitly invited to, so friends/public events are invisible to everyone else and the setting is decorative. Not yet executed as of 2026-09-16. Use when asked about event visibility, discovering friends' events, or why a public event isn't showing up.
---

# Make `visibility` mean something

`calendar_events.visibility` has existed since migration
`002_create_events_table.sql`, is settable in `CreateEventModal.svelte`,
and is about to become a user-level default too (see
`.claude/skills/settings-integrity/SKILL.md`). It currently has **no effect
on listing whatsoever.**

`services::calendar::list_user_events` is participant-only:

```sql
SELECT DISTINCT e.* FROM calendar_events e
JOIN event_participants ep ON e.id = ep.event_id
WHERE ep.user_id = $1
```

So an event marked `public` is seen by exactly the people who were already
invited - i.e. the same people who'd see it if it were `private`. Note the
single-event fetch `GET /api/events/:id` *does* check
`OR e.visibility = 'public'`, so the two endpoints already disagree about
what visibility means. That inconsistency is the bug in miniature.

This is the largest remaining semantic hole in the data model, and it
gates anything that surfaces events more widely (reminders, discovery,
"what's on this week").

## Decide first - these are product/privacy calls, not implementation details

Do **not** guess these. They change who can read whose calendar.

1. **What does `friends` resolve to?** This app has two separate notions of
   friendship, both in `friendships` with different `source` values:
   `'discord_guild'` (auto-synced from shared guild membership, see
   `services::friends`) and `'friend_request'` (explicitly accepted, see
   `services::friend_requests`). Does a `friends`-visible event show to
   both, or only to explicitly-accepted friends? Guild-synced "friends" can
   be dozens of people the user never individually approved - defaulting to
   both may be much wider exposure than the user pictured when they clicked
   "Friends".
2. **What does `public` mean in a single-guild app?** Every user belongs to
   the one linked guild, so "public" and "everyone in the guild" are the
   same set today. Confirm that's intended rather than shipping a scope
   that silently widens if multi-guild ever lands.
3. **Does a discovered event appear in the calendar grid alongside your own,
   or in a separate lane?** A friend's party you were never invited to is
   not the same thing as an event you owe an RSVP to, and the calendar
   currently renders one flat list. Mixing them without any visual
   distinction will read as a bug.

Ask the user and record the answers in this file before writing the query.

## Backend

`services::calendar::list_user_events` becomes a union of:

- events you participate in (the current clause - keep the
  `include_declined` behaviour exactly as-is), **plus**
- events where `visibility = 'public'`, **plus**
- events where `visibility = 'friends'` and the creator is a friend of the
  caller per the decision in (1).

Keep `DISTINCT` - an event can match more than one clause (you're invited
*and* it's public), and it must appear once.

Watch out for these, in rough order of how likely they are to bite:

- **`EventWithParticipants.my_status` / `is_creator`.** For a discovered
  event you have no `event_participants` row at all, so `my_status` is
  `None`. The frontend already handles a missing status (`statusOf()` in
  `EventPeekPanel.svelte` falls back to `pending`), but "No answer" is the
  wrong label for an event you were never asked about. Decide whether the
  API should distinguish "invited, hasn't answered" from "not invited,
  just visible" - a nullable `is_participant` flag on the response is the
  cheap way.
- **RSVP on a non-invited event.** `EventPeekPanel`'s Going/Maybe/Can't
  buttons call `api.updateParticipation`, which assumes a participant row.
  Either hide the RSVP row for non-participants, or make
  `update_participation_status` insert a participant row on first RSVP
  (self-join). The latter is nicer but is a real behaviour change - it
  means a public event's guest list can grow without the creator inviting
  anyone.
- **The declined filter.** `AND ep.status != 'declined'` lives in the
  participant clause; make sure declining a public event still hides it
  rather than having it reappear through the visibility clause. This is the
  easiest regression to ship by accident - test it explicitly.
- **Query shape.** The function builds SQL by string concatenation with
  positional `$n` params counted by hand (`param_count`). Adding clauses
  means the numbering has to stay correct across every optional
  `start_date`/`end_date` combination. Consider restructuring rather than
  threading more counters through - and if you keep the manual approach,
  test all four date-parameter combinations.

## Frontend

- `Calendar.svelte`'s filter chips are All / Going / Awaiting my answer /
  Created by me. Discovered events belong to none of those cleanly -
  "Awaiting my answer" currently matches `!my_status || my_status ===
  'pending'`, which would sweep up every public event in the guild. Fix
  that predicate as part of this work, and consider a fifth chip per
  decision (3).
- Remember Svelte's reactive-`$:` dependency tracking is **static**: an
  identifier only counts as a dependency if it appears literally in the
  `$:` line. `filteredEvents` and `eventsForDay` in `Calendar.svelte` are
  both written carefully for this reason - read the comments there before
  editing, and don't move the filter logic back inside a helper that reads
  state from its closure.

## Tests

- `#[sqlx::test]` per visibility level: a non-participant sees `public`,
  sees `friends` iff the friendship rule says so, never sees `private`.
- A test that a declined participant still doesn't see a public event.
- A test that the caller's own events are unaffected (no duplicates from
  `DISTINCT`, ordering still `start_time ASC`).
- Component test that a discovered event renders without RSVP controls (or
  with them, per the decision) and isn't miscounted by the filter chips.

## Done when

`GET /api/events` returns what the visibility field promises, the two
endpoints agree on what `public` means, and declining still hides an event.
