use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::models::Visibility;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub discord_id: String,
    pub username: String,
    pub discriminator: Option<String>,
    pub avatar: Option<String>,
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub display_name: Option<String>,
    pub timezone: String,
    pub default_visibility: Visibility,
    pub notify_event_invites: bool,
    pub notify_rsvp_changes: bool,
    pub notify_announcements: bool,
    pub notify_weekly_digest: bool,
    pub notify_event_reminders: bool,
}

impl User {
    pub fn build_avatar_url(discord_id: &str, avatar: &Option<String>) -> Option<String> {
        avatar.as_ref().map(|a| {
            format!(
                "https://cdn.discordapp.com/avatars/{}/{}.png",
                discord_id, a
            )
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>,
    pub email: Option<String>,
}

/// All fields optional - only the ones present get changed. Mirrors the
/// pattern already used by UpdateEventRequest in calendar_event.rs.
#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    pub timezone: Option<String>,
    pub default_visibility: Option<Visibility>,
    pub notify_event_invites: Option<bool>,
    pub notify_rsvp_changes: Option<bool>,
    pub notify_announcements: Option<bool>,
    pub notify_weekly_digest: Option<bool>,
    pub notify_event_reminders: Option<bool>,
}

/// Deleting an account is real and immediate - this app has no "soft
/// delete"/trash concept anywhere else, so account deletion doesn't
/// invent one either. `confirm_username` guards against a stray click:
/// the request must echo the account's own username back, not just carry
/// a valid session.
#[derive(Debug, Deserialize)]
pub struct DeleteAccountRequest {
    pub confirm_username: String,
}
