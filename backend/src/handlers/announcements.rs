use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::{
    config::AppState,
    error::AppError,
    middleware::auth::Claims,
    models::{AnnouncementPostInfo, CalendarEvent, CreateEventRequest, ReplyInfo},
    services::{
        discord_config, discord_feed,
        event_adoption::{self, AdoptError},
        reaction_sync,
    },
};

/// The resolved announcements channel - same DB-config-first,
/// env-var-fallback resolution `services::calendar::create_event` uses, so
/// this feed always mirrors whatever channel events actually get announced
/// to. Single channel only, matching the rest of this app's
/// single-guild/single-channel scope (see CLAUDE.md).
async fn resolve_channel(state: &AppState) -> Result<String, AppError> {
    let guild_id = state
        .discord_guild_id
        .as_deref()
        .ok_or_else(|| AppError::ValidationError("No Discord server is linked".to_string()))?;

    let channel_id = discord_config::resolve_announcement_channel_id(
        &state.db,
        guild_id,
        state.discord_announcement_channel_id,
    )
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?
    .ok_or_else(|| AppError::ValidationError("No announcements channel configured".to_string()))?;

    Ok(channel_id.to_string())
}

/// Resolves a local announcement id to the Discord message it mirrors, plus
/// the thread that message owns (creating the thread if it has none yet).
///
/// `discord_message_id`/`channel_id` stay server-side: `AnnouncementPostInfo`
/// deliberately exposes neither, so the client addresses a post by its local
/// UUID and Discord's ids never become part of the public API.
async fn resolve_thread(state: &AppState, id: Uuid) -> Result<String, AppError> {
    let bot_token = state
        .discord_bot_token
        .as_deref()
        .ok_or_else(|| AppError::ValidationError("Discord bot is not configured".to_string()))?;

    let row: Option<(String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT discord_message_id, channel_id, title, body FROM announcement_posts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let (message_id, channel_id, title, body) = row.ok_or(AppError::NotFound)?;

    // Discord needs a name when the thread doesn't exist yet; the post's own
    // title (or its first line of body) is the only sensible one.
    let thread_name = title.unwrap_or(body);

    discord_feed::fetch_or_create_thread(
        &state.discord_api_base,
        &state.http_client,
        bot_token,
        &channel_id,
        &message_id,
        &thread_name,
    )
    .await
    .map_err(|e| AppError::ExternalApiError(e.to_string()))
}

pub async fn list_replies(
    _claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ReplyInfo>>, AppError> {
    let thread_id = resolve_thread(&state, id).await?;
    let bot_token = state.discord_bot_token.as_deref().unwrap_or_default();

    let replies = discord_feed::fetch_replies(
        &state.discord_api_base,
        &state.http_client,
        bot_token,
        &thread_id,
    )
    .await
    .map_err(|e| AppError::ExternalApiError(e.to_string()))?;

    Ok(Json(replies))
}

/// What adopting an announcement produced: the event, and how many ✅ that
/// were already sitting on the message became RSVPs.
///
/// The count is reported rather than left implicit because it's the whole
/// reason adoption beats re-creating the event by hand - people had already
/// reacted, and those answers are recovered instead of being asked for
/// again.
#[derive(Debug, serde::Serialize)]
pub struct AdoptionResult {
    pub event: CalendarEvent,
    pub rsvps_recorded: usize,
    /// True when the backfill could not run or failed. The event is still
    /// created; only the recovery of existing reactions was missed, and
    /// `POST /api/events/sync-reactions` retries it.
    pub backfill_failed: bool,
}

/// Turns an announcement the server has already seen into a calendar event,
/// binding it to the existing Discord message instead of posting a new one.
///
/// See `services::event_adoption` for why this is the reverse of the normal
/// create-then-announce path. The request body is an ordinary
/// `CreateEventRequest` so the two paths describe an event identically;
/// `guild_ids` is the one field ignored, since the server is decided by
/// where the message already lives.
pub async fn adopt_announcement(
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateEventRequest>,
) -> Result<Json<AdoptionResult>, AppError> {
    // Same guard as create_event - an adopted event is a real event and gets
    // the same validation, not a looser path in because it came from Discord.
    if req.end_time <= req.start_time {
        return Err(AppError::ValidationError(
            "End time must be after start time".to_string(),
        ));
    }

    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let target = event_adoption::resolve_target(&state.db, id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .map_err(|e| match e {
            AdoptError::UnknownPost => AppError::NotFound,
            other => AppError::ValidationError(other.to_string()),
        })?;

    let event = event_adoption::adopt(&state.db, user.id, &target, req)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    tracing::info!(
        "📅 Adopted Discord message {} as event {} for {}",
        target.message_id,
        event.id,
        user.username
    );

    // Best-effort, and awaited rather than spawned: it is one Discord call,
    // and the people who already reacted are exactly what the user expects to
    // see on the event they just adopted. A failure here doesn't undo the
    // adoption - the event is real either way, and sync-reactions retries.
    let (rsvps_recorded, backfill_failed) = match state.discord_bot_token.as_deref() {
        Some(bot_token) => match reaction_sync::sync_message(
            &state.db,
            &state.discord_api_base,
            &state.http_client,
            bot_token,
            &target.channel_id,
            &target.message_id,
        )
        .await
        {
            Ok((_seen, recorded)) => (recorded, false),
            Err(e) => {
                tracing::warn!(
                    "⚠️  Adopted {}, but the RSVP backfill failed: {e}",
                    event.id
                );
                (0, true)
            }
        },
        None => (0, true),
    };

    Ok(Json(AdoptionResult {
        event,
        rsvps_recorded,
        backfill_failed,
    }))
}

pub async fn list_announcements(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<Vec<AnnouncementPostInfo>>, AppError> {
    let channel_id = resolve_channel(&state).await?;

    let posts = discord_feed::list_posts(&state.db, &channel_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(posts))
}

pub async fn sync_announcements(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<Vec<AnnouncementPostInfo>>, AppError> {
    let channel_id = resolve_channel(&state).await?;

    let bot_token = state.discord_bot_token.as_deref().ok_or_else(|| {
        AppError::ValidationError("DISCORD_BOT_TOKEN is not configured".to_string())
    })?;

    discord_feed::sync_channel(
        &state.discord_api_base,
        &state.db,
        &state.http_client,
        bot_token,
        &channel_id,
    )
    .await
    .map_err(|e| AppError::ExternalApiError(e.to_string()))?;

    let posts = discord_feed::list_posts(&state.db, &channel_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(posts))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::config::AppState;
    use crate::handlers::auth::generate_jwt;
    use crate::services::auth::create_or_update_user;

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

    fn discord_message(id: &str, content: &str) -> serde_json::Value {
        json!({
            "id": id,
            "author": { "id": "author-1", "username": "alice", "avatar": null },
            "content": content,
            "timestamp": "2026-03-01T12:00:00Z",
            "pinned": false,
            "reactions": [],
            "thread": null
        })
    }

    #[sqlx::test]
    async fn sync_then_list_round_trips_over_http(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/123456789/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(vec![discord_message("msg-1", "Hello everyone")]),
            )
            .mount(&server)
            .await;

        let state = AppState::for_test(db, server.uri());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/announcements/sync")
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
        let synced: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(synced.as_array().unwrap().len(), 1);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/announcements")
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
        let posts: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(posts[0]["body"], "Hello everyone");
        assert_eq!(posts[0]["author_username"], "alice");
    }

    /// A post in the feed with nothing behind it - a hand-written
    /// announcement, or one this app posted before its database existed.
    async fn seed_post(db: &PgPool, message_id: &str) -> uuid::Uuid {
        let id = uuid::Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO announcement_posts
                (id, discord_message_id, channel_id, author_discord_id, author_username,
                 body, tag, posted_at, synced_at)
            VALUES ($1, $2, '123456789', 'julioo-discord', 'Julioo', 'Concert', 'general', now(), now())
            "#,
        )
        .bind(id)
        .bind(message_id)
        .execute(db)
        .await
        .unwrap();
        id
    }

    fn adopt_body() -> Body {
        Body::from(
            json!({
                "title": "EsdeeKid",
                "start_time": "2027-03-01T20:00:00Z",
                "end_time": "2027-03-01T23:00:00Z",
                "location": "Le Bikini",
                "visibility": "friends"
            })
            .to_string(),
        )
    }

    /// Discord's reaction list for the ✅ on `message_id`, plus a refusal to
    /// accept any *new* message being posted: adoption must bind to what is
    /// already there, never announce a second time.
    async fn mock_discord(message_id: &str, reactors: serde_json::Value) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/channels/123456789/messages/{message_id}/reactions/%E2%9C%85"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(reactors))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/channels/123456789/messages"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        server
    }

    // The whole point of adopting rather than re-creating: people had
    // already ticked ✅, and those answers come back as RSVPs instead of
    // having to be asked for again.
    #[sqlx::test]
    async fn adopting_a_post_creates_an_event_and_recovers_the_reactions(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;
        let post = seed_post(&db, "msg-1").await;
        crate::services::guilds::ensure_guild(&db, "test-guild-id")
            .await
            .unwrap();

        let server = mock_discord(
            "msg-1",
            json!([
                { "id": "friend-discord", "username": "friend", "bot": false },
                { "id": "bot-discord", "username": "friends-calendar", "bot": true }
            ]),
        )
        .await;

        let state = AppState::for_test(db.clone(), server.uri());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();

        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/announcements/{post}/adopt"))
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(adopt_body())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(result["event"]["title"], "EsdeeKid");
        // The bot's own ✅ is not a participant, so one of the two reactors.
        assert_eq!(result["rsvps_recorded"], 1);
        assert_eq!(result["backfill_failed"], false);

        let status: String = sqlx::query_scalar(
            r#"
            SELECT ep.status::text FROM event_participants ep
            JOIN users u ON u.id = ep.user_id
            WHERE u.discord_id = 'friend-discord'
            "#,
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(status, "accepted");

        // `.expect(0)` on the POST mock is checked when the server drops.
        drop(server);
    }

    // Adopting the same post twice would leave two events fighting over one
    // message's reactions.
    #[sqlx::test]
    async fn adopting_the_same_post_twice_is_refused(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;
        let post = seed_post(&db, "msg-1").await;
        crate::services::guilds::ensure_guild(&db, "test-guild-id")
            .await
            .unwrap();
        let server = mock_discord("msg-1", json!([])).await;

        let state = AppState::for_test(db.clone(), server.uri());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let request = || {
            Request::builder()
                .method("POST")
                .uri(format!("/api/announcements/{post}/adopt"))
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(adopt_body())
                .unwrap()
        };

        assert_eq!(
            app.clone().oneshot(request()).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            app.oneshot(request()).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        let events: i64 = sqlx::query_scalar("SELECT count(*) FROM calendar_events")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(events, 1);
    }

    #[sqlx::test]
    async fn adopting_an_unknown_post_is_a_404(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();

        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/announcements/{}/adopt", uuid::Uuid::new_v4()))
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(adopt_body())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn requires_auth(db: PgPool) {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let app = crate::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/announcements")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test]
    async fn list_400s_when_no_discord_server_is_linked(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;

        let mut state = AppState::for_test(db, "http://unused.invalid".to_string());
        state.discord_guild_id = None;
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/announcements")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test]
    async fn list_400s_when_no_channel_is_configured(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;

        let mut state = AppState::for_test(db, "http://unused.invalid".to_string());
        state.discord_announcement_channel_id = None;
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/announcements")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // The round trip the skill asks for: sync a post, open its thread, post
    // a reply, and see it come back.
    #[sqlx::test]
    async fn replies_are_readable_but_not_writable(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;

        let server = MockServer::start().await;
        // The feed sync.
        Mock::given(method("GET"))
            .and(path("/channels/123456789/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(vec![discord_message("msg-1", "Ski trip deposit")]),
            )
            .mount(&server)
            .await;
        // Resolving the thread: the message has one already.
        Mock::given(method("GET"))
            .and(path("/channels/123456789/messages/msg-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg-1",
                "author": { "id": "1", "username": "alice", "avatar": null },
                "content": "Ski trip deposit",
                "timestamp": "2026-03-01T12:00:00Z",
                "thread": { "id": "thread-1", "message_count": 1 }
            })))
            .mount(&server)
            .await;
        // Posting into it, then reading it back.
        Mock::given(method("POST"))
            .and(path("/channels/thread-1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "r1" })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/channels/thread-1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "id": "r1",
                "author": { "id": "2", "username": "bob", "avatar": null },
                "content": "I'm in",
                "timestamp": "2026-03-01T13:00:00Z"
            }])))
            .mount(&server)
            .await;

        let state = AppState::for_test(db, server.uri());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        // Sync so the post exists locally and has an id to address.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/announcements/sync")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let posts: Value = serde_json::from_slice(&bytes).unwrap();
        let post_id = posts[0]["id"].as_str().unwrap().to_string();

        // Discord's own ids stay server-side - the client only ever has the
        // local UUID.
        assert!(posts[0].get("discord_message_id").is_none());

        // Posting is gone: replies used to be sent by the bot, so the thread
        // showed "friends-calendar" saying whatever a user typed, and with
        // no allowed_mentions guard a user could make the bot ping
        // @everyone with the bot's permissions. The route must be absent,
        // not merely unused by the UI.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/announcements/{post_id}/reply"))
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"body":"I'm in"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Reading the thread still works.
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/announcements/{post_id}/replies"))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
