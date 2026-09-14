use axum::{extract::State, Json};

use crate::{config::AppState, error::AppError, middleware::auth::Claims, models::LinkedServerInfo, services::friends};

// Which Discord server this app is currently linked to (name/icon/member
// count) — the "other users from that server" half of the same user-page
// feature is GET /api/friends (already existing).
pub async fn get_linked_server(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<LinkedServerInfo>, AppError> {
    let (bot_token, guild_id) = match (&state.discord_bot_token, &state.discord_guild_id) {
        (Some(token), Some(guild)) => (token, guild),
        _ => {
            return Err(AppError::ValidationError(
                "No Discord server is linked (set DISCORD_BOT_TOKEN and DISCORD_GUILD_ID)".to_string(),
            ))
        }
    };

    let info = friends::get_linked_server_info(&state.discord_api_base, &state.http_client, bot_token, guild_id)
        .await
        .map_err(|e| AppError::ExternalApiError(e.to_string()))?;

    Ok(Json(info))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::{json, Value};
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

    // Functional: drives an actual HTTP request through the real router
    // (routing + JWT auth middleware + handler + service), not just the
    // handler function in isolation — see .claude/skills/add-tests/SKILL.md.
    #[sqlx::test]
    async fn get_linked_server_returns_guild_info_over_http(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/guilds/test-guild-id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "test-guild-id",
                "name": "Friends Server",
                "icon": "abc123",
                "approximate_member_count": 6
            })))
            .mount(&server)
            .await;

        let state = AppState::for_test(db, server.uri());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/discord/server")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["id"], "test-guild-id");
        assert_eq!(json["name"], "Friends Server");
        assert_eq!(
            json["icon_url"],
            "https://cdn.discordapp.com/icons/test-guild-id/abc123.png"
        );
        assert_eq!(json["approximate_member_count"], 6);
    }

    #[sqlx::test]
    async fn get_linked_server_requires_auth(db: PgPool) {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let app = crate::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/discord/server")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test]
    async fn get_linked_server_400s_when_not_configured(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;

        let mut state = AppState::for_test(db, "http://unused.invalid".to_string());
        state.discord_bot_token = None;
        state.discord_guild_id = None;
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/discord/server")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
