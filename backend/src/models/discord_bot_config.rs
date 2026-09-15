use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Which channel each bot feature posts to. Three slots to match the
/// mockup's "Bot channels" section, but only `announcements_channel_id`
/// has a real consumer today (services::calendar::create_event's
/// auto-announce) - `events_channel_id`/`reminders_channel_id` are stored
/// and editable, not yet wired to anything, since this app doesn't
/// distinguish "event cards" from announcements and has no reminder
/// system. See CLAUDE.md for the full picture, including what this
/// deliberately does NOT do (the gateway bot's reaction-watching channel
/// is fixed at process startup, not live-reloaded from this config).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BotChannelConfig {
    pub guild_id: String,
    pub events_channel_id: Option<String>,
    pub announcements_channel_id: Option<String>,
    pub reminders_channel_id: Option<String>,
    // Weekly digest: a real background job (services::digest) checks this
    // rather than the per-user notify_weekly_digest preference (which has
    // no send mechanism behind it) - see 009_add_announcement_feed.sql.
    pub digest_enabled: bool,
    pub last_digest_sent_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBotChannelConfigRequest {
    pub events_channel_id: Option<String>,
    pub announcements_channel_id: Option<String>,
    pub reminders_channel_id: Option<String>,
    pub digest_enabled: Option<bool>,
}
