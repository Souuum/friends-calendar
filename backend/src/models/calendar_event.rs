use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::Type;
use uuid::Uuid;

// Two separate rename_alls, and they are not redundant: the `sqlx` one
// controls the Postgres enum representation, the `serde` one controls the
// JSON wire format. Without the serde one, JSON used the Rust variant
// names ("Friends"), while the entire frontend sends and compares
// lowercase - so PATCH /api/auth/me and PUT /api/events/:id/participation
// both 422'd on every request the app actually made. Keep them in sync.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "visibility", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    #[default]
    Private,
    Friends,
    Public,
}

// See the note on Visibility above - same split, same reason.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "participation_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ParticipationStatus {
    Pending,
    Accepted,
    Declined,
    Maybe,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CalendarEvent {
    pub id: Uuid,
    pub creator_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub location: Option<String>,
    pub visibility: Visibility,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub discord_message_id: Option<String>,
    pub discord_channel_id: Option<String>,
    pub price: Option<String>,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EventParticipant {
    pub id: Uuid,
    pub event_id: Uuid,
    pub user_id: Uuid,
    pub status: ParticipationStatus,
    pub invited_at: DateTime<Utc>,
    pub responded_at: Option<DateTime<Utc>>,
}

// Response with participant details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventWithParticipants {
    #[serde(flatten)]
    pub event: CalendarEvent,
    pub participants: Vec<ParticipantInfo>,
    pub is_creator: bool,
    pub my_status: Option<ParticipationStatus>,
    /// Whether the caller is actually on the guest list, as opposed to
    /// merely being able to see the event (it's public, or friends-visible
    /// and the creator is a friend). `my_status` can't answer this: a
    /// non-participant has no status, and so does an invitee who hasn't
    /// replied - the UI has to tell "you owe an answer" apart from "this is
    /// someone else's event you can see".
    pub is_participant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    pub user_id: Uuid,
    pub discord_id: String,
    pub username: String,
    pub avatar_url: Option<String>,
    pub status: ParticipationStatus,
    pub responded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEventRequest {
    pub title: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub location: Option<String>,
    pub visibility: Option<Visibility>,
    pub participant_ids: Option<Vec<Uuid>>, // Invite users by ID
    pub price: Option<String>,
    pub link: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEventRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub location: Option<String>,
    pub visibility: Option<Visibility>,
    pub price: Option<String>,
    pub link: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InviteParticipantsRequest {
    pub user_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateParticipationRequest {
    pub status: ParticipationStatus,
}

#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub include_declined: Option<bool>,
}
