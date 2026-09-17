use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

use crate::models::User;

/// Raw DB row - `author_avatar_url` isn't stored, it's derived the same
/// way `services::friends::get_friends` derives `FriendInfo.avatar_url`.
#[derive(Debug, Clone, FromRow)]
pub struct AnnouncementPostRow {
    pub id: Uuid,
    pub author_discord_id: String,
    pub author_username: String,
    pub author_avatar: Option<String>,
    pub title: Option<String>,
    pub body: String,
    pub tag: String,
    pub reaction_count: i32,
    pub reply_count: i32,
    pub pinned: bool,
    pub posted_at: DateTime<Utc>,
    /// Selected so `list_posts` can build a thread deep link. It is not
    /// carried onto `AnnouncementPostInfo` - see the note there.
    pub discord_message_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnnouncementPostInfo {
    pub id: Uuid,
    pub author_username: String,
    pub author_avatar_url: Option<String>,
    pub title: Option<String>,
    pub body: String,
    pub tag: String,
    pub reaction_count: i32,
    pub reply_count: i32,
    pub pinned: bool,
    pub posted_at: DateTime<Utc>,
    /// Deep link to the message's own Discord thread, when the guild is
    /// known. Built server-side rather than exposing `discord_message_id`
    /// and `channel_id`, which a test asserts never reach the client.
    pub thread_url: Option<String>,
}

impl From<AnnouncementPostRow> for AnnouncementPostInfo {
    fn from(row: AnnouncementPostRow) -> Self {
        Self {
            id: row.id,
            author_avatar_url: User::build_avatar_url(&row.author_discord_id, &row.author_avatar),
            author_username: row.author_username,
            title: row.title,
            body: row.body,
            tag: row.tag,
            reaction_count: row.reaction_count,
            reply_count: row.reply_count,
            pinned: row.pinned,
            posted_at: row.posted_at,
            thread_url: None,
        }
    }
}

/// A single reply in an announcement's Discord thread.
///
/// Serialize-only and fetched live from Discord rather than cached:
/// `announcement_posts.reply_count` is whatever the last sync saw, which is
/// stale the moment anyone replies.
#[derive(Debug, Clone, Serialize)]
pub struct ReplyInfo {
    pub author_username: String,
    pub author_avatar_url: Option<String>,
    pub body: String,
    pub posted_at: DateTime<Utc>,
}
