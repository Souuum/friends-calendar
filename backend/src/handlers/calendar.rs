use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use uuid::Uuid;

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
pub struct InvitableQuery {
    /// The friend you're thinking of inviting.
    pub user_id: Uuid,
}

/// Your upcoming events this friend isn't already on.
///
/// Exists as an endpoint rather than a client-side filter over
/// `GET /api/events` because answering it that way means fetching every
/// event to discard most of them.
///
/// ⚠️ Friends-only, the same guard `handlers::availability::week` has -
/// otherwise this becomes a way to enumerate a stranger's event membership
/// by user id.
pub async fn list_invitable_events(
    claims: Claims,
    State(state): State<AppState>,
    Query(query): Query<InvitableQuery>,
) -> Result<Json<Vec<CalendarEvent>>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let friends = crate::services::friends::get_friends(&state.db, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    if !friends.iter().any(|f| f.user_id == query.user_id) {
        return Err(AppError::ValidationError(
            "Not one of your friends".to_string(),
        ));
    }

    let events = calendar::list_invitable_events(&state.db, user.id, query.user_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(events))
}

/// Chases the people who never answered.
///
/// See `services::nudge` for why this one is rate-limited in the database
/// rather than in the UI: it is the only endpoint here that lets a person
/// send something to other people on demand.
pub async fn nudge_no_answers(
    claims: Claims,
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<crate::services::nudge::NudgeReport>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let outcome = crate::services::nudge::nudge(
        &state.db,
        &state.discord_api_base,
        &state.http_client,
        state.discord_bot_token.as_deref(),
        event_id,
        user.id,
        chrono::Utc::now(),
    )
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    match outcome {
        Ok(report) => {
            tracing::info!(
                "👋 {} nudged {} no-answer(s) on event {}",
                user.username,
                report.nudged,
                event_id
            );
            Ok(Json(report))
        }
        // NotFound for "not yours" too: a 403 would confirm the event exists
        // and belongs to somebody else.
        Err(crate::services::nudge::NudgeError::NotYours) => Err(AppError::NotFound),
        Err(crate::services::nudge::NudgeError::TooSoon(next)) => {
            Err(AppError::ValidationError(format!(
                "Already nudged recently - you can nudge again after {}",
                next.format("%H:%M on %e %b")
            )))
        }
    }
}

/// Records every ✅ already sitting on announcement messages.
///
/// `bot.rs` only sees reactions added while it is connected, so anything
/// ticked before an event was announced through this app - or during any
/// downtime - never became an RSVP. This reconciles from Discord's own
/// state, and is safe to run repeatedly: the underlying upsert is
/// idempotent.
pub async fn sync_reactions(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<crate::services::reaction_sync::SyncReport>, AppError> {
    let bot_token = state.discord_bot_token.as_deref().ok_or_else(|| {
        AppError::ValidationError("DISCORD_BOT_TOKEN is not configured".to_string())
    })?;

    let report = crate::services::reaction_sync::sync_all(
        &state.db,
        &state.discord_api_base,
        &state.http_client,
        bot_token,
    )
    .await
    .map_err(|e| AppError::ExternalApiError(e.to_string()))?;

    Ok(Json(report))
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
    // --- POST /api/events/:id/nudge --------------------------------------

    #[sqlx::test]
    async fn nudging_someone_elses_event_is_not_found(db: PgPool) {
        let creator = seed_user(&db, "creator-discord", "creator").await;
        let stranger = seed_user(&db, "stranger-discord", "stranger").await;
        let event: uuid::Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO calendar_events (id, creator_id, title, start_time, end_time, created_at, updated_at)
            VALUES (gen_random_uuid(), $1, 'Theirs', now() + interval '2 days',
                    now() + interval '2 days 2 hours', now(), now())
            RETURNING id
            "#,
        )
        .bind(creator.id)
        .fetch_one(&db)
        .await
        .unwrap();

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&stranger.discord_id, &state.jwt_secret).unwrap();

        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/events/{event}/nudge"))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // NotFound, not Forbidden: a 403 confirms the event exists and
        // belongs to somebody else.
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // The rate limit is the endpoint's, not the button's - a disabled button
    // is a suggestion, this is the actual surface.
    #[sqlx::test]
    async fn a_second_nudge_over_http_is_refused(db: PgPool) {
        let creator = seed_user(&db, "creator-discord", "creator").await;
        let invitee = seed_user(&db, "invitee-discord", "invitee").await;
        let event: uuid::Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO calendar_events (id, creator_id, title, start_time, end_time, created_at, updated_at)
            VALUES (gen_random_uuid(), $1, 'Mine', now() + interval '2 days',
                    now() + interval '2 days 2 hours', now(), now())
            RETURNING id
            "#,
        )
        .bind(creator.id)
        .fetch_one(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO event_participants (id, event_id, user_id, status, invited_at)
             VALUES (gen_random_uuid(), $1, $2, 'pending', now())",
        )
        .bind(event)
        .bind(invitee.id)
        .execute(&db)
        .await
        .unwrap();

        let state = AppState::for_test(db.clone(), "http://unused.invalid".to_string());
        let token = generate_jwt(&creator.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);
        let request = || {
            Request::builder()
                .method("POST")
                .uri(format!("/api/events/{event}/nudge"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap()
        };

        let first = app.clone().oneshot(request()).await.unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        let body = axum::body::to_bytes(first.into_body(), usize::MAX)
            .await
            .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(report["nudged"], 1);

        let second = app.oneshot(request()).await.unwrap();
        assert_eq!(second.status(), StatusCode::BAD_REQUEST);

        let notifications: i64 =
            sqlx::query_scalar("SELECT count(*) FROM notifications WHERE user_id = $1")
                .bind(invitee.id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(notifications, 1);
    }
    // --- GET /api/events/invitable ---------------------------------------
    //
    // One test per clause on purpose: a single omnibus test passes as soon
    // as the list is non-empty, which it would be for the wrong reasons.

    async fn befriend(db: &PgPool, a: uuid::Uuid, b: uuid::Uuid) {
        for (x, y) in [(a, b), (b, a)] {
            sqlx::query(
                "INSERT INTO friendships (id, user_id, friend_id, source, synced_at, created_at)
                 VALUES (gen_random_uuid(), $1, $2, 'discord_guild', now(), now())",
            )
            .bind(x)
            .bind(y)
            .execute(db)
            .await
            .unwrap();
        }
    }

    async fn an_event(
        db: &PgPool,
        creator: uuid::Uuid,
        title: &str,
        starts_in_hours: i64,
    ) -> uuid::Uuid {
        sqlx::query_scalar::<_, uuid::Uuid>(
            r#"
            INSERT INTO calendar_events (id, creator_id, title, start_time, end_time, created_at, updated_at)
            VALUES (gen_random_uuid(), $1, $2, now() + make_interval(hours => $3),
                    now() + make_interval(hours => $3 + 2), now(), now())
            RETURNING id
            "#,
        )
        .bind(creator)
        .bind(title)
        .bind(starts_in_hours as i32)
        .fetch_one(db)
        .await
        .unwrap()
    }

    async fn invitable_titles(
        db: PgPool,
        me: &crate::models::User,
        friend: uuid::Uuid,
    ) -> Vec<String> {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&me.discord_id, &state.jwt_secret).unwrap();
        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri(format!("/api/events/invitable?user_id={friend}"))
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
        let events: serde_json::Value = serde_json::from_slice(&body).unwrap();
        events
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["title"].as_str().unwrap().to_string())
            .collect()
    }

    #[sqlx::test]
    async fn invitable_includes_your_own_events_nobody_is_on_yet(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let friend = seed_user(&db, "friend-discord", "friend").await;
        befriend(&db, me.id, friend.id).await;
        an_event(&db, me.id, "Board games", 48).await;

        assert_eq!(
            invitable_titles(db, &me, friend.id).await,
            vec!["Board games"]
        );
    }

    #[sqlx::test]
    async fn invitable_excludes_events_they_are_already_on(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let friend = seed_user(&db, "friend-discord", "friend").await;
        befriend(&db, me.id, friend.id).await;
        let already = an_event(&db, me.id, "Already invited", 48).await;
        an_event(&db, me.id, "Not yet", 72).await;
        sqlx::query(
            "INSERT INTO event_participants (id, event_id, user_id, status, invited_at)
             VALUES (gen_random_uuid(), $1, $2, 'pending', now())",
        )
        .bind(already)
        .bind(friend.id)
        .execute(&db)
        .await
        .unwrap();

        assert_eq!(invitable_titles(db, &me, friend.id).await, vec!["Not yet"]);
    }

    #[sqlx::test]
    async fn invitable_excludes_events_that_have_already_started(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let friend = seed_user(&db, "friend-discord", "friend").await;
        befriend(&db, me.id, friend.id).await;
        an_event(&db, me.id, "Last night", -24).await;
        an_event(&db, me.id, "Tomorrow", 24).await;

        assert_eq!(invitable_titles(db, &me, friend.id).await, vec!["Tomorrow"]);
    }

    // invite_participants is creator-only and silently no-ops for anyone
    // else, so offering someone else's event would be a choice that does
    // nothing. The list has to agree with the rule.
    #[sqlx::test]
    async fn invitable_excludes_events_you_did_not_create(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let friend = seed_user(&db, "friend-discord", "friend").await;
        let someone_else = seed_user(&db, "other-discord", "other").await;
        befriend(&db, me.id, friend.id).await;
        an_event(&db, someone_else.id, "Theirs", 48).await;

        assert!(invitable_titles(db, &me, friend.id).await.is_empty());
    }

    // Otherwise this enumerates a stranger's event membership by user id.
    #[sqlx::test]
    async fn invitable_refuses_someone_who_is_not_your_friend(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let stranger = seed_user(&db, "stranger-discord", "stranger").await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&me.discord_id, &state.jwt_secret).unwrap();
        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri(format!("/api/events/invitable?user_id={}", stranger.id))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
