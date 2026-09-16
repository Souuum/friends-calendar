use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Which channel the bot posts to.
///
/// One slot, not the mockup's three: `events_channel_id` and
/// `reminders_channel_id` were dropped in migration 013 because nothing
/// ever read them - event announcements use this channel, and reminders go
/// into the event's own Discord thread. See CLAUDE.md for what this
/// deliberately does NOT do (the gateway bot's reaction-watching channel is
/// fixed at process startup, not live-reloaded from this config).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BotChannelConfig {
    pub guild_id: String,
    pub announcements_channel_id: Option<String>,
    // Weekly digest: a real background job (services::digest) checks this
    // rather than the per-user notify_weekly_digest preference (which has
    // no send mechanism behind it) - see 009_add_announcement_feed.sql.
    pub digest_enabled: bool,
    pub last_digest_sent_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBotChannelConfigRequest {
    pub announcements_channel_id: Option<String>,
    pub digest_enabled: Option<bool>,
}
