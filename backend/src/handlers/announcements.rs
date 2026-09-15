use axum::{Json, extract::State};

use crate::{
    config::AppState,
    error::AppError,
    middleware::auth::Claims,
    models::AnnouncementPostInfo,
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
}
