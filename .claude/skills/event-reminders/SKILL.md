---
name: event-reminders
description: Send "your event starts soon" reminders to Discord and to the in-app notification feed, filling in the reminders_channel_id that /server already lets people configure but nothing has ever used. Depends on settings-integrity landing first. Not yet executed as of 2026-09-16. Use when asked about reminders, upcoming-event alerts, or the unused reminders channel setting.
---

# Event reminders

`discord_bot_config.reminders_channel_id` has been settable on `/server`
since `mockup-settings-and-server` (migration 008). It is written, read
back, and used by **nothing** - grep it: every hit is the config CRUD path
itself. This is the last configured-but-unimplemented setting in the app,
and reminders are the most obviously useful feature a friends calendar can
add.

## Do these first

- **`.claude/skills/settings-integrity/SKILL.md`** must land first. It puts
  the preference gate inside `services::notifications::create`. If you
  build reminders before that exists, you create a second notification path
  that ignores user preferences, and someone has to retrofit it later.
- **`.claude/skills/event-visibility-listing/SKILL.md`** ideally lands
  first too, because it settles who can see an event - which is the same
  question as who should be reminded about one. If it hasn't landed,
  restrict reminders to actual participants and say so explicitly.

## Model it on `services::digest`, not on something new

`services::digest` already solves the same shape of problem and is the
pattern to copy:

- `is_due(now, last_sent) -> bool` as a **pure, unit-tested function**,
  separate from anything that touches the DB or the network. This is why
  digest has real tests despite being a scheduled job - do the same.
- `maybe_send_*` does the DB read, the send, and the timestamp stamp.
- `spawn_*_loop` on a `tokio::time::interval`, spawned from `main.rs`
  behind the same "only if bot token and guild id are configured" guard as
  the bot itself.
- Sending goes through `services::discord_feed::send_channel_message`, the
  shared HTTP-POST helper - don't add a third way to post to Discord.

## Decide first

1. **Lead time.** One fixed offset (e.g. 1 hour before start) is the
   simplest thing that works and needs no schema. Per-event or per-user
   lead times need a column and a UI, and nobody has asked for them. Start
   fixed; note the constraint rather than building configurability nobody
   requested.
2. **Where does it go - Discord channel, in-app notifications, or both?**
   The configured `reminders_channel_id` implies Discord. But the app also
   has a working notification feed with an unread badge, and a reminder is
   exactly the kind of thing it exists for. Recommend both, with the in-app
   one gated on the user's preference per `settings-integrity`.
3. **Who gets reminded?** Accepted participants only, or `maybe` too, or
   everyone invited including no-answers? "Accepted + maybe" is the
   defensible default - reminding someone who explicitly declined is spam.
4. **New `notify_event_reminders` preference?** If reminders are delivered
   in-app per user, they need a toggle, and none of the existing four fits
   (a reminder is not an invite or an RSVP change). Adding one means
   migration `010` plus a row in the settings UI. Decide with the user
   rather than reusing `notify_event_invites`.

## Schema (migration `010`, next free number)

You need idempotency: the loop wakes repeatedly and must not re-send. Add a
marker rather than recomputing from timestamps:

```sql
ALTER TABLE calendar_events
  ADD COLUMN IF NOT EXISTS reminder_sent_at TIMESTAMPTZ;
```

Follow the existing convention of `ADD COLUMN IF NOT EXISTS` (see
migrations 004/005 and the renumbering incident in CLAUDE.md) so it's safe
against a dev database that already has the column.

## The part that will actually be wrong: time windows

A reminder loop is a correctness trap. Be explicit about all of this:

- **Poll interval vs lead time.** Digest polls hourly because `is_due` has
  a whole-morning tolerance. A 1-hour-lead reminder polled hourly can fire
  up to an hour early or late. Either poll more often (every 5 minutes) or
  define the window as "starts within the next N minutes and hasn't been
  reminded", and accept the granularity. Write down which you chose.
- **Catch-up after downtime.** If the process is down for three hours, on
  restart every event that started in that window is "due". Don't send
  reminders for events that already started - filter on
  `start_time > now()` as well as the window.
- **Timezones.** `start_time` is `TIMESTAMPTZ` and comparisons happen in
  UTC, which is correct. `users.timezone` is stored but unused everywhere
  in this app; do not start interpreting it here without a decision, and do
  not format reminder text in server-local time.
- **Clock skew / DST.** Because everything is UTC `TIMESTAMPTZ`, DST is a
  formatting concern only. Keep it that way.

## Tests

The pure function carries the weight, same as `digest::is_due`:

- `is_due` style unit tests: event starts in 55 min with no reminder sent
  -> due; already reminded -> not due; already started -> not due; starts
  in 6 hours -> not due. No DB, no network, plain `#[test]`.
- `#[sqlx::test]` for the selection query: picks up accepted/maybe
  participants, skips declined, skips events already stamped.
- `wiremock` for the Discord POST, exactly like `services::friends` and
  `services::discord_feed` - the function under test must take the API base
  URL as a parameter (`&state.discord_api_base`) so tests never hit the
  real API.
- A test that a second pass immediately after the first sends nothing.

## Frontend

Minimal: reminders mostly happen server-side. If in-app delivery is in
scope, the existing `/notifications` page and `unreadNotificationCount`
store already render whatever `services::notifications::create` writes, so
a new `kind` needs only a label and an icon - check how
`NotificationInfo.message` is composed at write time (it's stored, not
derived from `kind` at read time).

## Done when

A configured reminders channel receives exactly one message per upcoming
event at the agreed lead time, declined participants aren't reminded,
restarting the process doesn't double-send, and the preference gate from
`settings-integrity` is respected.
