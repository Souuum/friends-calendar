---
name: mockup-settings-and-server
description: Split /settings into a real profile/preferences page and a new /server page, per the Friends Calendar Mockups project - editable display name/timezone/default visibility/notification toggles/account deletion, plus DB-backed multi-channel Discord bot config replacing the single env-var channel. Not yet executed as of 2026-09-15. Use when asked to build account settings, preferences, or Discord server/channel configuration UI.
---

# Settings (profile/preferences) + Discord server (bot config)

Source: `isSettings` and `isServer` screens in `Friends Calendar Mockups.dc.html`.
**Not yet executed.** Today `desktop/src/routes/settings/+page.svelte`
shows linked-server info + friends (built last session, see CLAUDE.md's
"Settings page" section) - the mockup splits that into two real, different
concerns: a profile/preferences page, and a separate Discord-server/bot
config page. This skill covers both since they share the "account
configuration" surface, but they should land as two routes.

## Decide first: rename the existing `/settings`

The current `/settings` (linked server + friends) doesn't match either new
screen's content. Options: move its "linked server card" content into the
new `/server` page (natural fit - `LinkedServerCard.svelte` already exists
and mostly matches `isServer`'s server-summary card) and its "friends"
content into `.claude/skills/mockup-friends-directory/SKILL.md`'s `/friends`
page (if that's landed) or keep as a redirect. Don't leave three pages
showing overlapping friend/server info - consolidate.

## Data model (new migration, next available number)

Profile/preference fields - either new columns on `users` or a separate
`user_preferences` table (1:1 with `users`); given `users` already holds
profile-ish fields (`username`, `avatar`, `email`), extending it is more
consistent with this codebase's existing style than introducing a new 1:1
table:

```sql
ALTER TABLE users ADD COLUMN IF NOT EXISTS display_name VARCHAR(255);
ALTER TABLE users ADD COLUMN IF NOT EXISTS timezone VARCHAR(100) NOT NULL DEFAULT 'UTC';
ALTER TABLE users ADD COLUMN IF NOT EXISTS default_visibility visibility NOT NULL DEFAULT 'friends'; -- reuse the enum from 002_create_events_table.sql
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_event_invites BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_rsvp_changes BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_announcements BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_weekly_digest BOOLEAN NOT NULL DEFAULT true;
```

(`ADD COLUMN IF NOT EXISTS` per the migration-collision precedent already
documented in CLAUDE.md - cheap insurance, no reason not to.)

Multi-channel bot config - this is the bigger architectural change: today
`DISCORD_ANNOUNCEMENT_CHANNEL_ID` is a single env var
(`AppState.discord_announcement_channel_id`, `backend/src/config.rs`). The
mockup's "Bot channels" section (event cards / announcements / reminders,
each independently changeable from the UI) implies **DB-backed, user-editable
config**, not env vars:

```sql
CREATE TABLE IF NOT EXISTS discord_bot_config (
    guild_id VARCHAR(255) PRIMARY KEY,
    events_channel_id VARCHAR(255),
    announcements_channel_id VARCHAR(255),
    reminders_channel_id VARCHAR(255),
    updated_at TIMESTAMPTZ NOT NULL
);
```

This is a real migration path decision: `AppState.discord_announcement_channel_id`
currently seeds itself from env at startup and everything reads it from
there (`main.rs`'s bot spawn, `handlers::calendar::create_event`'s
auto-announce). Moving to DB-backed config means those read sites need to
either re-query the DB per use, or `AppState` needs a way to refresh a
cached value when the config changes. Pick one deliberately, don't half-do
it (e.g. UI that "changes" a channel but the running bot/announcer keeps
using the stale env-var value is worse than not having the UI).

## Backend

- Profile: `PATCH /api/auth/me` (extend `handlers::auth`, or a new
  `handlers::profile`) for display_name/timezone/default_visibility/notification
  toggles. `default_visibility` should actually be *used* by
  `CreateEventModal.svelte` as the default `visibility` value, not just
  stored.
- `DELETE /api/auth/me` (or `/api/account`) - account deletion. Check
  `ON DELETE CASCADE` coverage before shipping this live: `friendships`,
  `event_participants`, `calendar_events.creator_id` all cascade already
  (see migrations 002/003), but verify rather than assume, and add a
  confirmation step server-side too (e.g. requiring the request to echo
  back the username) since this is genuinely destructive and the mockup's
  UI-only "Delete" button isn't enough of a safeguard on its own.
- `handlers::discord_config` (new): `GET`/`PUT /api/discord/config` for
  the three channel IDs, replacing the env-var read sites per the decision
  above.

## Frontend

- `desktop/src/routes/settings/+page.svelte`: rebuild per `isSettings` -
  profile section, default-visibility picker (reuse the pill styling
  pattern already used for event visibility in `CreateEventModal.svelte`),
  notification toggles, delete-account danger zone (real confirm dialog,
  not just the mockup's button).
- `desktop/src/routes/server/+page.svelte` (new): server summary
  (reuse `LinkedServerCard.svelte`), bot channel pickers, static permissions
  list (this one really can be hardcoded - it should just reflect the
  actual `GatewayIntents`/OAuth scopes `bot.rs`/`config.rs` request, not a
  live Discord call).

## Tests

Backend: `#[sqlx::test]` for profile update, account deletion (assert
cascade actually removes friendships/events/participants), discord-config
read/write. Frontend: form component tests, delete-account confirmation
flow test. Full guidance in `.claude/skills/add-tests/SKILL.md`.
