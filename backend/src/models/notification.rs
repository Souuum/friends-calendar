use crate::models::ParticipationStatus;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// A notification as returned to the client. `message` is pre-rendered at
/// write time (services::notifications::create), not composed from `kind`/
/// `actor` at read time - keeps the read path a single simple query, at
/// the cost of old notifications not retroactively reflecting e.g. a
/// later username change. Same tradeoff most apps make here.
#[derive(Debug, Clone, Serialize)]
pub struct NotificationInfo {
    pub id: Uuid,
    pub kind: String,
    pub actor_username: Option<String>,
    pub actor_avatar_url: Option<String>,
    pub event_id: Option<Uuid>,
    pub message: String,
    pub read: bool,
    pub created_at: DateTime<Utc>,
    /// How the *recipient* has answered the event this notification is
    /// about, if it is about one and they are on its participant list.
    ///
    /// ⚠️ Without this the notifications page cannot tell an invite you have
    /// answered from one you haven't: it showed three untouched
    /// Going/Maybe/Can't buttons either way, so answering appeared to do
    /// nothing and the state was gone again on reload. Marking the row read
    /// is not a substitute - "seen" and "answered" are different facts.
    pub my_status: Option<ParticipationStatus>,
}
