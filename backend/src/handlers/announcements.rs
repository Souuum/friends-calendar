use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::{
    config::AppState,
    error::AppError,
    middleware::auth::Claims,
    models::{AnnouncementPostInfo, ReplyInfo},
    services::{discord_config, discord_feed},
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
