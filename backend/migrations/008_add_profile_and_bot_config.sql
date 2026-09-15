-- Profile/preferences (Settings page)
ALTER TABLE users ADD COLUMN IF NOT EXISTS display_name VARCHAR(255);
ALTER TABLE users ADD COLUMN IF NOT EXISTS timezone VARCHAR(100) NOT NULL DEFAULT 'UTC';
ALTER TABLE users ADD COLUMN IF NOT EXISTS default_visibility visibility NOT NULL DEFAULT 'friends';
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_event_invites BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_rsvp_changes BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_announcements BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_weekly_digest BOOLEAN NOT NULL DEFAULT true;

-- DB-backed, user-editable bot channel config (Discord server page),
-- replacing the single env-var DISCORD_ANNOUNCEMENT_CHANNEL_ID as the
-- source of truth for where new events get announced. One row per guild -
-- this app only ever links one guild today (DISCORD_GUILD_ID), so in
-- practice it's one row, but keyed by guild_id rather than being a
-- singleton table in case that ever changes.
CREATE TABLE IF NOT EXISTS discord_bot_config (
    guild_id VARCHAR(255) PRIMARY KEY,
    events_channel_id VARCHAR(255),
    announcements_channel_id VARCHAR(255),
    reminders_channel_id VARCHAR(255),
    updated_at TIMESTAMPTZ NOT NULL
);
