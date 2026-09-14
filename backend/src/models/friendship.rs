use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

// friendships rows themselves (id, user_id, friend_id, source, synced_at,
// created_at) are only ever read back joined with `users` — see
// services::friends::get_friends — so there's no standalone row struct
// here, just the shapes actually returned to the client.

// A friend as returned to the client. Mirrors the shape of ParticipantInfo
// in calendar_event.rs (avatar_url is derived, not the raw Discord hash).
#[derive(Debug, Clone, Serialize)]
pub struct FriendInfo {
    pub user_id: Uuid,
    pub username: String,
    pub avatar_url: Option<String>,
    pub synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncFriendsResult {
    pub synced: usize,
    pub removed: usize,
    pub friends: Vec<FriendInfo>,
}
