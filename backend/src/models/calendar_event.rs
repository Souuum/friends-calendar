use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "visibility", rename_all = "lowercase")]
pub enum Visibility {
    Private,
    Friends,
    Public,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Private
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "participation_status", rename_all = "lowercase")]
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
}

#[derive(Debug, Deserialize)]
pub struct UpdateEventRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub location: Option<String>,
    pub visibility: Option<Visibility>,
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