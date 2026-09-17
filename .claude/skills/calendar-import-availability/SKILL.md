---
name: calendar-import-availability
description: Read a user's real calendar (Google, Outlook, or any .ics URL) so availability reflects their whole life, not just this app. Depends on nothing, but makes availability-best-overlap actually correct. Not yet executed as of 2026-09-17. Use when asked about importing calendars, Google Calendar sync, free/busy accuracy, or connecting an external calendar.
---

# Availability that knows about the rest of your life

Today "free" means **"no Friends Calendar event"**. Somebody with a full week
of work meetings shows as free all week, and `best-slot` confidently suggests
a time they are in a standup. That is the single biggest gap between what
availability claims and what it knows.

## Why this is cheaper than it looks

⚠️ **There is exactly one place to change.** `services::availability::fetch_busy_intervals`
is the only function that builds availability data, and four pure consumers
take it as plain `(user_id, start, end)` tuples: `compute_free_users_at`,
`compute_free_users_per_day`, `rank_slots`, and the week strip.

An external calendar is **more rows in that list**. Best-overlap, "free this
week", "free tonight" and the availability strip all start accounting for a
work calendar with no change to any of them. Do not touch the ranking or
free/busy logic; union the intervals at the fetch and stop.

## The decision that makes this acceptable to ship

⚠️ **Store busy intervals. Never store event content.**

Importing somebody's work calendar into a *social* app is a privacy problem
the moment you keep titles - "Interview with…", "Oncology appointment". But
availability only ever needs `(start, end)`. Discard everything else at the
boundary, before it reaches the database, and the sensitive half never
lands. Say so in the connect UI, in those words; it is the difference
between a feature people enable and one they don't.

This also keeps the cached table tiny and makes a leak boring.

## Providers, honestly

| | Route | Notes |
|---|---|---|
| **Google** | OAuth + Calendar API `freeBusy.query` | ⚠️ `freeBusy` returns intervals **only** - no titles - which is exactly what's wanted. Verification review needed past ~100 users. |
| **Microsoft / Teams** | OAuth + Graph `getSchedule` | Same shape, same benefit. |
| **Apple** | ⚠️ **No public API.** | CalDAV with an app-specific password, or the .ics route below. Realistically: .ics only. |
| **Any** | A pasted secret `.ics` URL | Works everywhere, no OAuth. But see the warning below. |

**Start with Google + the .ics escape hatch.** Microsoft is the same shape
once the abstraction exists; Apple users are served by .ics.

⚠️ **A pasted .ics URL is a bearer credential to somebody's entire
calendar**, stored at rest - a secret this app does not otherwise hold, and
one that cannot be scoped or revoked from our side. If you offer it: store
it encrypted or not at all, say plainly what it grants, and make
disconnecting delete it. OAuth is better precisely because the user can
revoke it from Google.

## Schema (migration `018`, assuming export took 017)

```sql
CREATE TABLE external_calendars (
    id              UUID PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider        VARCHAR(20) NOT NULL,   -- 'google' | 'microsoft' | 'ics'
    -- OAuth refresh token, or the .ics URL. See the warning above.
    credential      TEXT NOT NULL,
    last_synced_at  TIMESTAMPTZ,
    last_error      TEXT,                   -- shown in the UI; a silently
                                            -- dead connection is worse than
                                            -- none, because availability
                                            -- looks right and isn't
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, provider)
);

-- Intervals only. No titles, no attendees, no location - see above.
CREATE TABLE external_busy (
    id           UUID PRIMARY KEY,
    calendar_id  UUID NOT NULL REFERENCES external_calendars(id) ON DELETE CASCADE,
    starts_at    TIMESTAMPTZ NOT NULL,
    ends_at      TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_external_busy_window ON external_busy (calendar_id, starts_at, ends_at);
```

## The sync loop

Copy `services::digest` / `services::reminders`: a pure `is_due`, a
`sync_all`, and `spawn_*_loop` on a `tokio::time::interval` from `main.rs`
behind the same "only if configured" guard.

- **Refresh a rolling window** (say now → +30 days) and replace that window
  wholesale per calendar. Incremental sync tokens are a later optimisation
  and a source of drift.
- **A failing connection is logged, stamped into `last_error`, and skipped** -
  one revoked token must not stop everyone else's sync. Same rule
  `reaction_sync` already follows.
- Hourly is plenty. Google's push notifications are a later optimisation.

## Decide first

1. **Does an imported busy block make you unavailable, or just "probably
   busy"?** Recommend genuinely busy - a suggestion you have to double-check
   is worth little. But say it in the UI.
2. **All-day events.** A day-long "Annual leave" should probably not block a
   19:00 slot. Recommend treating all-day blocks as busy for the *day
   strip* but not for slot ranking, and write the test both ways.
3. **Whose calendars count?** Only the caller's own and their friends' - and
   friends only in aggregate (a count), never "Ana is busy 14:00-15:00".
   Leaking a friend's schedule shape is exactly what the intervals-only rule
   is protecting against.

## Tests

- **Unit, pure**: merging app events and external intervals, including
  overlapping and adjacent blocks. The existing half-open convention
  (`start < end && other_start < end`) must hold across both sources -
  there's an existing test for the app side; write its twin.
- **Integration, wiremock**: a Google `freeBusy` response becomes rows; an
  expired token sets `last_error` and does **not** wipe the cached window
  (stale availability beats suddenly-everyone-is-free).
- **Integration (`#[sqlx::test]`)**: disconnecting deletes the credential
  *and* the cached intervals.
- ⚠️ **One test asserting no title, description or attendee ever reaches the
  database.** That is the promise the feature is sold on; assert it rather
  than trusting the mapping code to stay honest.
- **Functional**: availability endpoints reflect an imported block - the
  point of the whole feature, and it passes through `fetch_busy_intervals`
  without the ranking changing.

## Frontend

A "Connected calendars" card on `/settings`: connect, last synced, any
error, disconnect. Plus one line on the availability surfaces saying where
the data came from, so "why does it say I'm busy" has an answer.

## Done when

`best-slot` stops suggesting times people are already in meetings, and
nobody's meeting titles are in this database.
