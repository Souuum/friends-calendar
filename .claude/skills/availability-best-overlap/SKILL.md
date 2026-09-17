---
name: availability-best-overlap
description: Rank when the group is actually free and surface it - the "Best overlap this week: Fri 20:00, 7 free" line the mockup shows on the calendar, and the suggested slot the create form is supposed to open with. Needs hour-granularity availability, which services::availability does not have. Not yet executed as of 2026-09-17. Use when asked about suggesting a time, "propose a time", best overlap, or when everyone is free.
---

# The feature the app is named for

`Friends Calendar Mockups.dc.html` puts this on the calendar's Free-tonight
bar, between the avatar stack and the button:

> Best overlap this week: **Fri 20:00**, 7 free

and `Friends Calendar Mobile.dc.html` screen 12 says step one of creating
an event comes *"with the suggested slot offered up front"*.

Neither exists. The app can tell you who is free **right now** and who is
free **on a given day**, and then stops - so "Propose a time" opens an
empty form and the user does the overlap arithmetic in their head. That
arithmetic is the entire reason to have a shared calendar.

## Why this is bigger than it looks: the granularity is wrong

`services::availability` is **day-granularity on purpose**. Its own doc
comment says so:

> "Free" that day means zero accepted/maybe events overlap it at all - day
> granularity, not hour-by-hour, matching what the mockup's weekly strip
> actually shows (one colored cell per day, not a full calendar grid).

That was the right call for the strip. It cannot answer "Fri **20:00**".
Someone with a 09:00 dentist appointment is "busy Friday" and therefore
excluded from every Friday evening suggestion - which is exactly the
population this feature exists to find.

⚠️ **Do not change `compute_free_users_per_day`.** The weekly strip is
correct as it is and has tests pinning it. Add a slot-level computation
beside it and leave the day one alone; they answer different questions.

## Do these first

Nothing blocks it. But this feeds `.claude/skills/invite-friend-to-event/SKILL.md`,
so land this first if both are planned.

## Shape of the computation

The busy-interval fetch is already done and already reusable:
`busy_intervals_for` (or whatever the private fetch is called) pulls every
`accepted`/`maybe` event overlapping a window for a set of users in **one**
query. Build on that - don't add a second query shape.

Then, purely:

```rust
pub struct Slot { pub start: DateTime<Utc>, pub free_user_ids: Vec<Uuid> }

/// Candidate slots over [from, to), ranked by how many of `user_ids` are
/// free for the whole slot. Pure - no I/O, unit-tested directly.
fn rank_slots(
    busy: &[(Uuid, DateTime<Utc>, DateTime<Utc>)],
    user_ids: &[Uuid],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    slot_minutes: i64,
) -> Vec<Slot>
```

## Decide first - these change the answer, not just the code

1. **Which slots are candidates?** Every 30 minutes across a week is 336
   slots of mostly-nonsense (03:00 Tuesday). Recommend **evening slots on
   each day** (say 18:00-22:00 at 1h steps) plus weekend afternoons, and
   say so in the response. A suggestion nobody would act on is noise.
2. **How long is a slot?** The overlap answer depends on it - 7 people are
   free for an hour, 3 for four hours. Recommend defaulting to the duration
   the user has already typed into the create form when there is one, and
   2h otherwise.
3. **⚠️ Timezone.** "Fri 20:00" is a *local* time, and `users.timezone` is
   stored and read by nothing (a standing gap in CLAUDE.md). Evening slots
   computed in UTC are wrong for every user not on UTC. Either consult
   `users.timezone` here - which makes this the first feature that does -
   or compute in the requester's browser timezone and pass it in. Decide
   explicitly; don't let it default to UTC silently.
4. **Ties.** Several slots will have the same count. Prefer the **soonest**,
   and make that deterministic so the suggestion doesn't shuffle between
   page loads for no reason.
5. **Who counts?** The caller's friends, like every other availability
   endpoint - and `handlers::availability::week` already 400s if you ask
   about a stranger. Keep that guard; this endpoint must not become a way
   to probe someone's calendar by user id.

## Endpoint

`GET /api/availability/best-slot?from=<ISO>&to=<ISO>&duration_minutes=<n>`
returning the top slot plus a handful of runners-up. Return several: the
calendar bar shows one, but the create form wants alternatives, and two
endpoints for one computation will drift.

## Tests

- **Unit, heavily** - this is a pure interval problem and that is where the
  bugs are. Minimum: a slot touching the end of a busy interval is free
  (half-open intervals, same convention the existing code uses - check it
  and match, don't guess); an all-day event blocks every slot that day;
  someone with no events is free in all of them; ties resolve to the
  earliest; an empty user list doesn't panic.
- **Integration (`#[sqlx::test]`)**: `declined` and `pending` don't block,
  matching `week_availability`. There is already a test asserting that for
  the day path - write the slot-level twin, because the two will be read as
  a pair.
- **Functional**: asking about a non-friend 400s.
- **Component**: the calendar bar renders the slot, and "Propose a time"
  opens the create form **with that slot filled in** - assert the input
  value. A suggestion the button doesn't actually apply is the failure mode
  here.

## Done when

The Free-tonight bar answers "when", not just "who", and "Propose a time"
proposes one.
