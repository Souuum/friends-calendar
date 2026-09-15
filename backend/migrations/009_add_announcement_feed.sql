-- Discord channel message mirror (replaces the old event-RSVP /announcements
-- view). One row per Discord message synced from the configured
-- announcements channel; see services::discord_feed.
CREATE TABLE IF NOT EXISTS announcement_posts (
    id UUID PRIMARY KEY,
    discord_message_id VARCHAR(255) NOT NULL UNIQUE,
    channel_id VARCHAR(255) NOT NULL,
    author_discord_id VARCHAR(255) NOT NULL,
    author_username VARCHAR(255) NOT NULL,
    author_avatar VARCHAR(255),
    title TEXT,
    body TEXT NOT NULL,
    tag VARCHAR(20) NOT NULL DEFAULT 'general', -- 'event' | 'general', see services::discord_feed
    reaction_count INT NOT NULL DEFAULT 0,
    reply_count INT NOT NULL DEFAULT 0,
    pinned BOOLEAN NOT NULL DEFAULT false,
    posted_at TIMESTAMPTZ NOT NULL,
    synced_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_announcement_posts_channel_pinned_posted
    ON announcement_posts(channel_id, pinned DESC, posted_at DESC);

-- Weekly digest: a real scheduled job (see services::digest), gated by this
-- guild-level toggle rather than the per-user notify_weekly_digest
-- preference added in 008 (that one has no send mechanism behind it yet).
ALTER TABLE discord_bot_config ADD COLUMN IF NOT EXISTS digest_enabled BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE discord_bot_config ADD COLUMN IF NOT EXISTS last_digest_sent_at TIMESTAMPTZ;
