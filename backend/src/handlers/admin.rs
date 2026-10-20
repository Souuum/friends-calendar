use axum::{Json, extract::State};
use serde::Serialize;

use crate::{
    config::AppState,
    error::AppError,
    middleware::auth::Claims,
    services::admin::{self, AdminCheck},
};

/// What a refusal says. Names the Discord permissions, so the person reading
/// it knows who to ask rather than just that they can't.
pub const NOT_AN_ADMIN: &str = "Only server admins (Administrator or Manage Server on Discord) can change the bot's configuration";

/// Whether `user_discord_id` runs `discord_guild_id`, asked of Discord.
///
/// With no bot token there is no way to read anyone's roles, so nobody is an
/// admin - the config pages can't do anything useful without the bot anyway.
pub async fn admin_check(
    state: &AppState,
    discord_guild_id: &str,
    user_discord_id: &str,
) -> Result<AdminCheck, AppError> {
    let Some(bot_token) = state.discord_bot_token.as_deref() else {
        return Ok(AdminCheck::NotAdmin);
    };
    admin::check_guild_admin(
        &state.discord_api_base,
        &state.http_client,
        bot_token,
        discord_guild_id,
        user_discord_id,
    )
    .await
    .map_err(|e| AppError::ExternalApiError(e.to_string()))
}

/// The guard every bot-config endpoint calls before changing anything.
///
/// Server-side on purpose: hiding the pages in the app is only a courtesy,
/// and any signed-in user can call the API directly.
pub async fn require_admin(
    state: &AppState,
    discord_guild_id: &str,
    claims: &Claims,
) -> Result<(), AppError> {
    match admin_check(state, discord_guild_id, &claims.sub).await? {
        AdminCheck::Admin => Ok(()),
        AdminCheck::NotAdmin | AdminCheck::BotNotInGuild => {
            Err(AppError::Forbidden(NOT_AN_ADMIN.to_string()))
        }
    }
}

#[derive(Serialize)]
pub struct AdminStatus {
    pub is_admin: bool,
}

/// `GET /api/discord/admin`: whether the caller administers the linked server
/// (`DISCORD_GUILD_ID`), so the app knows whether to show `/server` and
/// `/servers`. Never 403s - "no" is an answer, not an error.
pub async fn get_admin_status(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<AdminStatus>, AppError> {
    let Some(guild_id) = state.discord_guild_id.as_deref() else {
        return Ok(Json(AdminStatus { is_admin: false }));
    };
    let check = admin_check(&state, guild_id, &claims.sub).await?;
    Ok(Json(AdminStatus {
        is_admin: check == AdminCheck::Admin,
    }))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::Value;
    use sqlx::PgPool;
    use tower::ServiceExt;
    use wiremock::MockServer;

    use crate::config::AppState;
    use crate::handlers::auth::generate_jwt;
    use crate::services::admin::test_support::mount_guild_member;

    async fn is_admin_over_http(db: PgPool, discord: &MockServer, discord_id: &str) -> bool {
        let state = AppState::for_test(db, discord.uri());
        let token = generate_jwt(discord_id, &state.jwt_secret).unwrap();
        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/discord/admin")
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
        json["is_admin"].as_bool().unwrap()
    }

    // `AppState::for_test` links the guild "test-guild-id".
    #[sqlx::test]
    async fn reports_an_admin_of_the_linked_server(db: PgPool) {
        let discord = MockServer::start().await;
        mount_guild_member(&discord, "test-guild-id", "admin-discord", true).await;
        assert!(is_admin_over_http(db, &discord, "admin-discord").await);
    }

    #[sqlx::test]
    async fn reports_an_ordinary_member_as_not_an_admin(db: PgPool) {
        let discord = MockServer::start().await;
        mount_guild_member(&discord, "test-guild-id", "member-discord", false).await;
        assert!(!is_admin_over_http(db, &discord, "member-discord").await);
    }

    #[sqlx::test]
    async fn requires_auth(db: PgPool) {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/discord/admin")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
