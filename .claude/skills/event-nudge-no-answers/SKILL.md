---
name: event-nudge-no-answers
description: Build the endpoint behind the peek panel's permanently-disabled "Nudge no-answers" button - a rate-limited ping to participants who never answered. Not yet executed as of 2026-09-17. Use when asked about chasing RSVPs, the disabled nudge button, or reminding people who haven't replied.
---

# The last placeholder control

`EventPeekPanel` renders a **"Nudge no-answers"** button that has been
`disabled` since `event-edit-flow` shipped, with a comment saying so. It is
the only remaining control in the app that exists but does nothing, and the
comment is right that it needs its own feature rather than being wired up
as a side effect.

Everything it needs already exists: `event_participants` records `pending`,
`services::notifications` delivers in-app with a preference gate, and
`services::discord_feed` owns the HTTP path to Discord.

## Do these first

- Nothing hard. But read **`.claude/skills/event-reminders/SKILL.md`** and
  copy its shape rather than inventing a parallel one - a nudge is a
  reminder with a manual trigger and a different audience.

## The one genuinely new problem: this is user-triggered sending

Every other outbound message in this app is either a consequence of
creating something (`create_event`'s announce) or a scheduled job
(`digest`, `reminders`). This is the first thing a person can fire at other
people **on demand**, which means it is the first that can be used to
annoy them.

⚠️ **Rate-limit it in the database, not in the UI.** A disabled button is a
suggestion; `POST /api/events/:id/nudge` is the actual surface. Recommend a
`nudged_at` column on `calendar_events` (migration `015`, next free number)
and refusing a second nudge within some window - 24h is defensible for an
event, and the response should say when the next one is allowed rather than
failing opaquely.

⚠️ **Only the creator may nudge.** `EventPeekPanel` already only shows the
button for `is_creator`, but the endpoint must enforce it - the UI check is
a convenience, not a guard.

## Decide first

1. **Where does the nudge land?** Three options, and they are not equal:
   - **In-app notification** to each pending participant. Works today, uses
     the existing preference gate, reaches only people who use the app.
   - **A message in the event's Discord thread.** Free - the thread id *is*
     the `discord_message_id` (see `event-reminders`). But it pings the
     thread, not the people, so the no-answers may never see it.
   - **A Discord DM per pending participant.** Actually reaches them, and
     is the only option that is unambiguously a *nudge*. It also needs
     `POST /users/@me/channels` to open a DM channel per user, and DMs from
     a bot are the most annoying thing this app could learn to do.

   Recommend **in-app + the thread message**, and treat DMs as a separate
   decision to take with the user. Say which you implemented.
2. **Does "no answer" include `maybe`?** No. `maybe` *is* an answer.
   Nudging it turns a considerate feature into pestering. Pending only.
3. **What does the creator see?** The count nudged. If it's zero (everyone
   answered) the button should be disabled with that reason, not enabled
   and silently doing nothing.

## Schema

```sql
-- migration 015 (next free)
ALTER TABLE calendar_events ADD COLUMN IF NOT EXISTS nudged_at TIMESTAMPTZ;
```

One column, no child table: unlike `event_reminders` there is nothing
per-lead-time to store, and a nudge history nobody reads is not worth a
table. If "who was nudged when" is ever wanted, that's the migration that
adds it.

## Tests

- **Unit, pure**: `can_nudge(now, nudged_at) -> bool`, separate from any
  I/O, exactly as `digest::is_due` and `reminders::is_due` are. This is the
  bit that will be wrong and the bit that's cheapest to test.
- **Integration (`#[sqlx::test]`)**: pending participants get a
  notification; `accepted`, `maybe` and `declined` do not; the preference
  gate is respected (it will be, for free, if you go through
  `services::notifications::create` - do that rather than inserting rows).
- **Functional**: a non-creator gets 403, not 200-with-no-effect. A second
  nudge inside the window is refused and `nudged_at` is unchanged.
- **Integration, wiremock**: the thread post goes to the right message id,
  and a 404 from Discord (the thread was never created - a documented
  tolerated failure) is logged and does **not** fail the request, because
  the in-app half did go out. Same rule `reminders` already follows.
- **Component**: the button is enabled only when there are pending
  participants, and reports the count afterwards.

## Done when

The button is live, it cannot be used to spam anyone, and nothing in the
app is still a control that looks functional and isn't.
