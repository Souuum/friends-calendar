---
name: mockup-announcements-feed
description: Replace the current event-RSVP-based /announcements page with the Friends Calendar Mockups project's real Discord-channel message mirror (pinned post, author/tag/reactions/replies feed, channels list, weekly digest toggle). Not yet executed as of 2026-09-15 - has open design questions flagged below, read those before starting. Use when asked to rebuild announcements as a Discord message feed.
---

# Announcements as a Discord message mirror

Source: `isAnnounce` screen in `Friends Calendar Mockups.dc.html`.
**Not yet executed, and less shovel-ready than the other skills** - this
one has real open design questions, not just adaptations. Read the "Open
questions" section before writing code, don't silently guess through them.

## This supersedes, not extends, last session's /announcements

`desktop/src/routes/announcements/+page.svelte` currently shows *calendar
events that got RSVP-announced to Discord* (filters `GET /api/events` by
`discord_message_id != null`). The mockup's "Announcements" is a
completely different thing: a mirror of general Discord channel messages -
posts with an author, a tag (Event/General/Poll), body text, reaction and
reply counts, one pinned post, a channels sidebar, and a digest toggle.
These aren't reconcilable by tweaking the existing page - implementing this
skill means replacing what `/announcements` shows.

Decide up front whether to keep the RSVP-tracking view too (e.g. fold it
into the friend/event detail views instead of a top-level page) or drop it
entirely now that this richer feed exists - **ask the user this**, don't
decide it silently, since it's a real product-scope call, not an
implementation detail.

## Open questions to resolve before/while implementing

- **What is a "tag" (Event/General/Poll)?** Nothing in Discord's message
  data implies this - it looks manually curated or inferred from message
  content/embeds. Simplest honest option: infer `Event` when a message has
  an associated `calendar_events.discord_message_id` match, default
  everything else to `General` (drop `Poll` unless there's a real signal
  for it, e.g. a Discord poll object in the message payload - check what
  `GET /channels/{id}/messages` actually returns for that server before
  assuming).
- **Reactions/replies counts**: Discord's message object gives reaction
  counts directly (`message.reactions[].count`). Thread reply counts need
  either the channel's associated thread object (`message.thread.message_count`)
  or a separate thread-messages fetch - check the API docs for the exact
  shape before assuming a field name.
  See `backend/src/services/discord_announcement.rs` for the existing
  thread-creation code (`create_thread_from_message`) as a reference for
  how this app already talks to threads.
- **Pinned post**: Discord has a real pinned-messages concept
  (`GET /channels/{id}/pins`) - use it rather than inventing app-side
  pinning, unless the intent is for *this app's users* to pin independently
  of Discord (needs a decision + a DB column either way).
- **Digest toggle ("One summary posted every Monday at 9:00")**: this is a
  scheduled job, which nothing in this backend has today (no cron/task
  runner - `main.rs` is request-driven only). Needs either an external
  scheduler hitting a new endpoint, or an in-process `tokio::time::interval`
  loop spawned in `main.rs` (same pattern as the Discord bot's
  `tokio::spawn` in `main.rs` - see how `bot::DiscordBot::start` is
  conditionally spawned there). Don't build the toggle UI without a real
  mechanism behind it - a setting that visibly does nothing is worse than
  no setting.

## Likely data model (once the above is resolved)

A cache table, synced on-demand or periodically (mirrors `services::friends::sync_friends`'s
pattern - fetch from Discord, upsert locally, so the page doesn't hit the
Discord API on every load):

```sql
CREATE TABLE IF NOT EXISTS announcement_posts (
    id UUID PRIMARY KEY,
    discord_message_id VARCHAR(255) NOT NULL UNIQUE,
    channel_id VARCHAR(255) NOT NULL,
    author_discord_id VARCHAR(255) NOT NULL,
    title TEXT, -- Discord messages don't have titles; derive from first line or an embed title if present
    body TEXT NOT NULL,
    tag VARCHAR(20) NOT NULL DEFAULT 'general',
    reaction_count INT NOT NULL DEFAULT 0,
    reply_count INT NOT NULL DEFAULT 0,
    pinned BOOLEAN NOT NULL DEFAULT false,
    posted_at TIMESTAMPTZ NOT NULL,
    synced_at TIMESTAMPTZ NOT NULL
);
```

## Backend

- `services::discord_feed` (new): fetch + upsert from
  `GET /channels/{id}/messages` (paginated, same `base_url`-as-parameter
  pattern as `services::friends` - see `.claude/skills/add-tests/SKILL.md`
  for why: it's what makes this mockable with `wiremock`), plus pins and
  reaction/thread data per the resolved open questions above.
- `handlers::announcements` (new, or repurpose the existing one after
  confirming with the user per "supersedes" above): `GET /api/announcements`,
  `POST /api/announcements/sync`.

## Frontend

Rework `desktop/src/routes/announcements/+page.svelte` (or a fresh route,
depending on the scope decision above) to match the mockup: pinned card +
feed + channels/digest sidebar.

## Tests

Backend: `wiremock` for the Discord message-fetch/pins calls, `#[sqlx::test]`
for the cache upsert logic, same shape as `services::friends`'s test module.
Frontend: component tests for the feed states. Full guidance in
`.claude/skills/add-tests/SKILL.md`.
