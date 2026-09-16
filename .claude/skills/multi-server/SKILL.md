---
name: multi-server
description: Extend the app from one Discord server to several - a `guilds`/`user_guilds`/`event_publications` data model, visibility scoped to where an event is published, a bot-invite flow, and a server picker so one event can be announced to any selection of servers. One event row, N announcements - never mirrored copies. Not yet executed as of 2026-09-16. Use when asked about multiple servers, guilds, cross-server events, or publishing an event to more than one place.
---

# Multi-server: one event, many announcements

The user confirmed the shape (2026-09-16): **one `calendar_events` row
published to N servers**, not events mirrored into other servers as separate
rows. Mirroring was considered and rejected - it means two sources of truth
and needs rules for what happens when a copy is edited.

Everything below assumes that. If someone later asks for real mirroring,
that's a different skill, not an extension of this one.

## Why this is bigger than it looks

Two things in the current code make single-server an assumption rather than
a configuration.

### 1. `visibility: friends` silently widens

`friendships` rows are created by `services::friends::sync_friends` from
*shared guild membership*, with `source = 'discord_guild'`. Event listing
(`services::calendar::list_user_events`, rewritten by
`.claude/skills/event-visibility-listing/SKILL.md`) treats **any** row in
that table as "friend".

With one guild that's coherent. With several it becomes "anyone I share *any*
server with" - so a work server and a games server start seeing each other's
events. **A feature about publishing quietly becomes a privacy change.**

This is the single most important thing to get right, and it must land with
the rest of the feature, not after. See "Visibility" below.

### 2. One event = one Discord message is in the schema

`calendar_events.discord_message_id` / `discord_channel_id` are single
columns, and three things reverse-look-up through them:

- `bot.rs` - `record_attendance`/`withdraw_attendance` find the event by
  message id, turning a ✅ into a participant row.
- `services::reminders` - the thread it posts into *is* that message id (a
  thread started from a message shares the message's id).
- `services::discord_feed` - infers the `event` tag by matching a synced
  message against `calendar_events.discord_message_id`.

All three become "find the event for *this* message", which is fine, but
they need the child table below to exist first.

## Data model (one migration, no behaviour change)

Do this **first and alone**. It's mechanical, it has no UI, and everything
else is easy once it exists.

```sql
CREATE TABLE guilds (
    id UUID PRIMARY KEY,
    discord_guild_id VARCHAR(64) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    icon VARCHAR(255),
    -- The channel the bot announces to in THIS server. Replaces
    -- discord_bot_config.announcements_channel_id, which was per-guild in
    -- name only (one row ever existed).
    announcements_channel_id VARCHAR(64),
    added_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- We derive friendships from guild membership today but never store the
-- membership itself, so there is currently no way to answer "which servers
-- can this person publish to?".
CREATE TABLE user_guilds (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    guild_id UUID NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, guild_id)
);

-- One row per place an event was announced. Replaces the two columns on
-- calendar_events.
CREATE TABLE event_publications (
    id UUID PRIMARY KEY,
    event_id UUID NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
    guild_id UUID NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    channel_id VARCHAR(64) NOT NULL,
    -- NULL until the announcement actually posts; the row is the intent,
    -- the id is the result. Lets a failed post be retried without losing
    -- which servers were chosen.
    discord_message_id VARCHAR(64),
    posted_at TIMESTAMPTZ,
    UNIQUE (event_id, guild_id)
);
CREATE INDEX ON event_publications (discord_message_id);
```

**Backfill, then drop** - same discipline as migration 012: promote the
existing `DISCORD_GUILD_ID` to the first `guilds` row, turn every event's
current `discord_message_id`/`discord_channel_id` into one publication row,
then drop those columns. Two sources of truth for "where was this posted" is
exactly the drift this project keeps removing.

⚠️ `#[sqlx::test]` migrates an **empty** database, so it will never exercise
the backfill. Verify it by hand on a scratch database seeded with real-shaped
rows, as was done for 012 - that's the only way to know it works before it
runs against the Proxmox database.

## Visibility

Decide with the user before writing the query. The safe default, and the one
this skill recommends:

> An event is visible to you if you're a participant, **or** it's published
> to a server you're in and its visibility allows it.

That makes `public`/`friends` **relative to the publication**, not global:
- `public` → anyone in a server it was published to
- `friends` → people in a published-to server who are also friends
- `private` → participants only, as today

The alternative (keep global friendship) is simpler but reintroduces the
cross-server leak described above. Don't pick it without saying so out loud.

`list_user_events` and `get_event_with_participants` must stay in agreement -
the listing calls the latter per event, so anything it rejects vanishes from
the list regardless of what the listing query matched. That bit already bit
once; see the visibility skill.

## Bot

- **The gateway watches one channel, captured at process start.**
  `bot.rs`'s `Handler` holds `announcement_channel_id` and filters on it.
  CLAUDE.md already notes the lack of hot-reload as a wart; multi-server
  makes it a blocker, because adding a server has to start being watched
  without a restart. Options: re-query the set of announcement channels on
  each reaction (cheap, one indexed lookup, no restart needed - recommended),
  or hold a shared `RwLock<HashSet<u64>>` refreshed when config changes.
  Prefer the query: it can't go stale.
- The reaction handlers themselves need almost no change - they already take
  a message id and look the event up. Point them at `event_publications`.

## Bot-invite flow (the genuinely new surface)

Adding a server is an OAuth authorize URL with `scope=bot applications.commands`
and a permissions integer, then a callback. `DISCORD_GUILD_ID` as an env var
stops making sense and should become a seed for the first `guilds` row, not
an ongoing source of truth.

This is the part that **cannot be tested without real Discord** - wiremock
covers the REST calls, but not whether the invite actually works. Budget time
for manual verification and say so rather than implying it's covered.

## Frontend

- `/server` (singular) becomes `/servers`: a list, an "Add server" button
  starting the invite flow, and per-server channel config. The existing page
  is a reasonable per-server detail view once it takes an id.
- `CreateEventModal` gains a server multi-select, defaulting to the creator's
  servers - or to a remembered preference if one is added. It sits next to
  the invite picker and the reminder chips; both of those are already
  create-only, and this should be too until editing publications is designed.
- **Editing and deleting fan out.** Changing an event means editing N
  messages; deleting means deleting N. Decide whether an edit re-posts, edits
  in place, or does nothing - and note that reminders address the thread by
  the message id, so re-posting orphans the thread.

## Already fine

Don't re-solve these:
- **Duplicate RSVPs.** Someone in two published-to servers can react in both;
  `record_attendance` is idempotent onto one participant row, and there's a
  test.
- **Reminders** address the thread via the message id, so once publications
  exist they just need to pick *which* publication's thread (or post in each).

## Sequencing

1. Data model + backfill, no behaviour change. ← start here
2. Visibility scoping.
3. Bot: per-guild channel resolution, no restart needed.
4. Bot-invite flow.
5. Server picker + `/servers`.

Each is independently shippable and CI-green. Don't bundle 1 with anything.

## Verification

`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`
(clean as of 2026-09-16 - don't add to it), `yarn test`, `yarn run check`,
`yarn build`, plus the hand-run backfill check described above.
