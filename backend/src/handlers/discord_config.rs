use axum::{extract::State, Json};

use crate::{
    config::AppState, error::AppError, middleware::auth::Claims, models::{BotChannelConfig, UpdateBotChannelConfigRequest},
    services::discord_config,
};

fn require_guild_id(state: &AppState) -> Result<&str, AppError> {
    state
        .discord_guild_id
        .as_deref()
        .ok_or_else(|| AppError::ValidationError("No Discord server is linked".to_string()))
}

pub async fn get_config(_claims: Claims, State(state): State<AppState>) -> Result<Json<BotChannelConfig>, AppError> {
    let guild_id = require_guild_id(&state)?;

    let config = discord_config::get_config(&state.db, guild_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .unwrap_or_else(|| BotChannelConfig {
            guild_id: guild_id.to_string(),
            events_channel_id: None,
            announcements_channel_id: None,
            reminders_channel_id: None,
        });

    Ok(Json(config))
}

pub async fn update_config(
    _claims: Claims,
    State(state): State<AppState>,
    Json(req): Json<UpdateBotChannelConfigRequest>,
) -> Result<Json<BotChannelConfig>, AppError> {
    let guild_id = require_guild_id(&state)?;

    let config = discord_config::upsert_config(
        &state.db,
        guild_id,
        req.events_channel_id,
        req.announcements_channel_id,
        req.reminders_channel_id,
    )
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(config))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::Value;
    use sqlx::PgPool;
    use tower::ServiceExt;

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

    #[sqlx::test]
    async fn get_then_update_over_http(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        // Nothing set yet - still 200, with nulls (not 404 or 400).
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/discord/config")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert!(json["announcements_channel_id"].is_null());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/discord/config")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"announcements_channel_id":"12345"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/discord/config")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["announcements_channel_id"], "12345");
    }

    #[sqlx::test]
    async fn requires_auth(db: PgPool) {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let app = crate::build_router(state);

        let response = app
            .oneshot(Request::builder().uri("/api/discord/config").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
