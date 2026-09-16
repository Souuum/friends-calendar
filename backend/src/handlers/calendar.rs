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
    services::{calendar, discord_announcement},
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

    // Taken before `req` is consumed below.
    let guild_ids = req.guild_ids.clone().unwrap_or_default();

    let event = calendar::create_event(&state.db, user.id, req)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    tracing::info!(
        "📅 Event created: {} by user {}",
        event.title,
        user.username
    );

    // Announce to each server the creator chose. Publishing is opt-in: no
    // selection means no announcement, which combined with publication-scoped
    // visibility means the event stays with its guest list. That's the
    // intended default - see .claude/skills/multi-server/SKILL.md.
    announce_to_selected_servers(&state, &event, guild_ids).await;

    Ok(Json(event))
}

/// Renders the Discord announcement for an event that hasn't been created
/// yet, so the create form can show exactly what will be posted.
///
/// Reuses services::discord_announcement::format_event_message verbatim
/// rather than reimplementing the format in the client - the whole point of
/// the endpoint is that the preview can't drift from the real message. The
/// response is the raw message *source*, Discord markdown and `<t:…>`
/// timestamps included; Discord renders those, the preview shows them as
/// they'll be sent.
pub async fn preview_announcement(
    _claims: Claims,
    Json(req): Json<CreateEventRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // A transient, unsaved event purely to feed the formatter. The ids and
    // timestamps are never read by it - only title/start/location/price/
    // link/description are - but the struct needs them.
    let preview = CalendarEvent {
        id: Uuid::nil(),
        creator_id: Uuid::nil(),
        title: req.title,
        description: req.description,
        start_time: req.start_time,
        end_time: req.end_time,
        location: req.location,
        visibility: req.visibility.unwrap_or_default(),
        created_at: req.start_time,
        updated_at: req.start_time,
        price: req.price,
        link: req.link,
    };

    Ok(Json(serde_json::json!({
        "message": crate::services::discord_announcement::format_event_message(&preview)
    })))
}

/// Publishes an event to each selected server: record the intent, post, then
/// stamp the resulting message id.
///
/// Every failure here is logged rather than returned. The event exists and
/// the user's request succeeded; one server's Discord being unreachable
/// shouldn't fail that, and the publication row survives so it's visible
/// which server didn't get its message.
async fn announce_to_selected_servers(
    state: &AppState,
    event: &CalendarEvent,
    guild_ids: Vec<Uuid>,
) {
    let Some(bot_token) = &state.discord_bot_token else {
        return;
    };

    for guild_id in guild_ids {
        let Ok(Some(discord_guild_id)) =
            crate::services::guilds::discord_id_of(&state.db, guild_id).await
        else {
            tracing::warn!("⚠️  Unknown server {} - skipping", guild_id);
            continue;
        };

        // Per server, not per process: each guild has its own configured
        // announcements channel, falling back to the env var for the original
        // single-server deployment.
        let channel_id = crate::services::discord_config::resolve_announcement_channel_id(
            &state.db,
            &discord_guild_id,
            state.discord_announcement_channel_id,
        )
        .await
        .unwrap_or(state.discord_announcement_channel_id);

        let Some(channel_id) = channel_id else {
            tracing::warn!(
                "⚠️  No announcements channel configured for server {} - skipping",
                discord_guild_id
            );
            continue;
        };
        let channel_id = channel_id.to_string();

        // Recorded before posting, so a Discord failure loses the message but
        // not the fact that this event was meant to reach this server.
        let publication_id = match crate::services::guilds::add_publication(
            &state.db,
            event.id,
            guild_id,
            &channel_id,
        )
        .await
        {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!("⚠️  Failed to record publication: {:?}", e);
                continue;
            }
        };

        match discord_announcement::announce_event(
            &state.discord_api_base,
            &state.http_client,
            bot_token,
            &channel_id,
            event,
        )
        .await
        {
            Ok(message_id) => {
                if let Err(e) =
                    crate::services::guilds::mark_published(&state.db, publication_id, &message_id)
                        .await
                {
                    // Posted but unrecorded, so reactions on it won't resolve
                    // back to the event. Worth shouting about.
                    tracing::error!("❌ Announced but failed to record publication: {:?}", e);
                } else {
                    tracing::info!(
                        "✅ Announced event {} in server {} as message {}",
                        event.id,
                        discord_guild_id,
                        message_id
                    );
                }
            }
            Err(e) => tracing::warn!(
                "⚠️  Failed to announce in server {}: {:?}",
                discord_guild_id,
                e
            ),
        }
    }
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
    if let (Some(start), Some(end)) = (req.start_time, req.end_time)
        && end <= start
    {
        return Err(AppError::ValidationError(
            "End time must be after start time".to_string(),
        ));
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

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use chrono::{Duration, Utc};
    use serde_json::Value;
    use sqlx::PgPool;
    use tower::ServiceExt;

    use crate::config::AppState;
    use crate::handlers::auth::generate_jwt;
    use crate::models::CreateEventRequest;
    use crate::services::auth::create_or_update_user;
    use uuid::Uuid;

    async fn seed_user(db: &PgPool, discord_id: &str, username: &str) -> crate::models::User {
        create_or_update_user(
            db,
            crate::models::DiscordUser {
                id: discord_id.to_string(),
                username: username.to_string(),
                discriminator: "0".to_string(),
                avatar: None,
                email: None,
            },
        )
        .await
        .unwrap()
    }

    // Companion to handlers::profile's casing regression test. The RSVP
    // path was the worse half of the same bug: the frontend's
    // api.updateParticipation sends {"status":"accepted"} (types.ts
    // `Status` is lowercase throughout), and ParticipationStatus rejected
    // it, so every Going/Maybe/Can't click failed. The component tests
    // didn't catch it because they build fixtures in TypeScript and never
    // cross the JSON boundary.
    #[sqlx::test]
    async fn update_participation_accepts_the_lowercase_status_the_client_sends(db: PgPool) {
        let creator = seed_user(&db, "creator-discord", "creator").await;
        let invitee = seed_user(&db, "invitee-discord", "invitee").await;

        let event = crate::services::calendar::create_event(
            &db,
            creator.id,
            CreateEventRequest {
                title: "Board games".to_string(),
                description: None,
                start_time: Utc::now() + Duration::days(1),
                end_time: Utc::now() + Duration::days(1) + Duration::hours(2),
                location: None,
                visibility: None,
                participant_ids: Some(vec![invitee.id]),
                price: None,
                link: None,
                guild_ids: None,
                reminder_leads: None,
            },
        )
        .await
        .unwrap();

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&invitee.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/events/{}/participation", event.id))
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"status":"accepted"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "the exact body api.updateParticipation sends must be accepted"
        );

        // And it must read back in the same casing, or Calendar.svelte's
        // `my_status === 'accepted'` filter and EventPeekPanel's STATUS
        // lookup both silently miss.
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/events/{}", event.id))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["my_status"], "accepted");
        // Value, not casing, is the other tests' business - this event was
        // created without an explicit visibility, so it inherits the
        // creator's `default_visibility` (see
        // services::calendar::tests::create_event_falls_back_to_the_creators_default_visibility).
        // Only assert the wire form is lowercase.
        let visibility = json["visibility"].as_str().unwrap();
        assert!(
            matches!(visibility, "private" | "friends" | "public"),
            "visibility {visibility:?} is not one of the lowercase values the client expects"
        );

        // Not indexed by position - participant order isn't guaranteed and
        // isn't what this test is about. Every status must be one of the
        // lowercase values types.ts declares in `Status`.
        let participants = json["participants"].as_array().unwrap();
        assert!(!participants.is_empty());
        for p in participants {
            let status = p["status"].as_str().unwrap();
            assert!(
                matches!(status, "pending" | "accepted" | "declined" | "maybe"),
                "participant status {status:?} is not one of the lowercase values the client expects"
            );
        }
    }

    // The preview's whole reason to exist is that it can't drift from the
    // real announcement, so this asserts equality with the formatter rather
    // than matching on any particular wording.
    #[sqlx::test]
    async fn announcement_preview_returns_exactly_what_would_be_posted(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let start = Utc::now() + Duration::days(2);
        let body = serde_json::json!({
            "title": "Raclette",
            "start_time": start,
            "end_time": start + Duration::hours(2),
            "location": "Chez Lina",
            "price": "15"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/events/announcement-preview")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        let preview = json["message"].as_str().unwrap();

        let expected = crate::services::discord_announcement::format_event_message(
            &crate::models::CalendarEvent {
                id: Uuid::nil(),
                creator_id: Uuid::nil(),
                title: "Raclette".to_string(),
                description: None,
                start_time: start,
                end_time: start + Duration::hours(2),
                location: Some("Chez Lina".to_string()),
                visibility: crate::models::Visibility::default(),
                created_at: start,
                updated_at: start,
                price: Some("15".to_string()),
                link: None,
            },
        );
        assert_eq!(preview, expected);
    }
}
