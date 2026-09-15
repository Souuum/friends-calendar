use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use uuid::Uuid;

use serde::Deserialize;

use crate::{
    config::AppState,
    error::AppError,
    middleware::auth::Claims,
    models::{
        CalendarEvent, CreateEventRequest, EventWithParticipants, InviteParticipantsRequest,
        ListEventsQuery, UpdateEventRequest, UpdateParticipationRequest,
    },
    services::{calendar, discord_announcement::DiscordAnnouncer},
};

// Create a new event
pub async fn create_event(
    claims: Claims,
    State(state): State<AppState>,
    Json(req): Json<CreateEventRequest>,
) -> Result<Json<CalendarEvent>, AppError> {
    // Validate time range
    if req.end_time <= req.start_time {
        return Err(AppError::ValidationError(
            "End time must be after start time".to_string(),
        ));
    }

    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let mut event = calendar::create_event(&state.db, user.id, req)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    tracing::info!(
        "📅 Event created: {} by user {}",
        event.title,
        user.username
    );

    // Auto-announce to Discord if configured. The channel comes from
    // services::discord_config first (live-editable from /server), falling
    // back to AppState.discord_announcement_channel_id (env var) if no DB
    // config has been set yet - see discord_config's own docs for why this
    // fallback exists and what it doesn't cover.
    let resolved_channel_id = match &state.discord_guild_id {
        Some(guild_id) => crate::services::discord_config::resolve_announcement_channel_id(
            &state.db,
            guild_id,
            state.discord_announcement_channel_id,
        )
        .await
        .unwrap_or(state.discord_announcement_channel_id),
        None => state.discord_announcement_channel_id,
    };

    if let (Some(bot_token), Some(channel_id)) = (&state.discord_bot_token, resolved_channel_id) {
        let announcer = DiscordAnnouncer::new(bot_token.clone(), channel_id);

        match announcer.announce_event(&event).await {
            Ok(message_id) => {
                // Update event with Discord message ID
                if let Ok(updated) = sqlx::query_as::<_, CalendarEvent>(
                    r#"
                    UPDATE calendar_events
                    SET discord_message_id = $1, discord_channel_id = $2, updated_at = NOW()
                    WHERE id = $3
                    RETURNING *
                    "#,
                )
                .bind(&message_id)
                .bind(channel_id.to_string())
                .bind(event.id)
                .fetch_one(&state.db)
                .await
                {
                    event = updated;
                    tracing::info!(
                        "✅ Event announced and linked to Discord message {}",
                        message_id
                    );
                }
            }
            Err(e) => {
                tracing::warn!("⚠️  Failed to announce event to Discord: {:?}", e);
            }
        }
    }

    Ok(Json(event))
}

// Get a specific event by ID with participants
pub async fn get_event(
    claims: Claims,
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<EventWithParticipants>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let event = calendar::get_event_with_participants(&state.db, event_id, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::NotFound)?;

    Ok(Json(event))
}

// List user's events (created or invited to)
pub async fn list_events(
    claims: Claims,
    State(state): State<AppState>,
    Query(query): Query<ListEventsQuery>,
) -> Result<Json<Vec<EventWithParticipants>>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let events = calendar::list_user_events(
        &state.db,
        user.id,
        query.start_date,
        query.end_date,
        query.include_declined.unwrap_or(false),
    )
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(events))
}

// Update an event (only creator can update)
pub async fn update_event(
    claims: Claims,
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<UpdateEventRequest>,
) -> Result<Json<CalendarEvent>, AppError> {
    // Validate time range if both are provided
    if let (Some(start), Some(end)) = (req.start_time, req.end_time) {
        if end <= start {
            return Err(AppError::ValidationError(
                "End time must be after start time".to_string(),
            ));
        }
    }

    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let event = calendar::update_event(&state.db, event_id, user.id, req)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::NotFound)?;

    tracing::info!(
        "📝 Event updated: {} by user {}",
        event.title,
        user.username
    );

    Ok(Json(event))
}

// Delete an event (only creator can delete)
pub async fn delete_event(
    claims: Claims,
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let deleted = calendar::delete_event(&state.db, event_id, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    if !deleted {
        return Err(AppError::NotFound);
    }

    tracing::info!("🗑️  Event deleted: {} by user {}", event_id, user.username);

    Ok(StatusCode::NO_CONTENT)
}

// Invite participants to an event (only creator can invite)
pub async fn invite_participants(
    claims: Claims,
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<InviteParticipantsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let participants = calendar::invite_participants(&state.db, event_id, user.id, req.user_ids)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    tracing::info!(
        "👥 {} participants invited to event {} by user {}",
        participants.len(),
        event_id,
        user.username
    );

    Ok(Json(participants))
}

// Update your participation status for an event
pub async fn update_participation(
    claims: Claims,
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<UpdateParticipationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let participant =
        calendar::update_participation_status(&state.db, event_id, user.id, req.status.clone())
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .ok_or(AppError::NotFound)?;

    tracing::info!(
        "✅ User {} updated participation to {:?} for event {}",
        user.username,
        req.status,
        event_id
    );

    Ok(Json(participant))
}

// Remove a participant from an event (only creator can remove)
pub async fn remove_participant(
    claims: Claims,
    State(state): State<AppState>,
    Path((event_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let removed = calendar::remove_participant(&state.db, event_id, user.id, user_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    if !removed {
        return Err(AppError::NotFound);
    }

    tracing::info!(
        "❌ User removed from event {} by creator {}",
        event_id,
        user.username
    );

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct LinkDiscordMessageRequest {
    pub message_id: String,
    pub channel_id: String,
}

pub async fn link_discord_message(
    claims: Claims,
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<LinkDiscordMessageRequest>,
) -> Result<Json<CalendarEvent>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    // Verify user is the creator and update
    let event = sqlx::query_as::<_, CalendarEvent>(
        r#"
        UPDATE calendar_events
        SET discord_message_id = $1, discord_channel_id = $2, updated_at = NOW()
        WHERE id = $3 AND creator_id = $4
        RETURNING *
        "#,
    )
    .bind(&req.message_id)
    .bind(&req.channel_id)
    .bind(event_id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?
    .ok_or(AppError::NotFound)?;

    tracing::info!(
        "🔗 Linked event {} to Discord message {}",
        event_id,
        req.message_id
    );

    Ok(Json(event))
}
