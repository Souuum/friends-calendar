use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// An incoming (pending) friend request, as returned to the recipient.
#[derive(Debug, Clone, Serialize)]
pub struct FriendRequestInfo {
    pub id: Uuid,
    pub from_user_id: Uuid,
    pub from_username: String,
    pub from_avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}
