use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::{
    config::AppState,
    error::AppError,
    middleware::auth::Claims,
    services::{
        discord_feed::{self, ChannelInfo},
        guilds::{self, GuildInfo},
    },
};

/// Permissions the bot needs, as a Discord permissions bitfield.
///
/// Spelled out rather than pasted as a magic number, because the only way to
/// check a number like this is to decompose it again:
///   View Channels             1024
///   Send Messages             2048
///   Add Reactions               64   (the ✅ people RSVP with)
///   Read Message History     65536   (syncing the announcements feed)
///   Create Public Threads  34359738368
///   Send Messages in Threads 274877906944  (reminders, replies)
///
/// Server Members Intent is *not* here - it's a toggle in the Discord
/// developer portal, not a per-guild permission, and friend sync needs it.
const BOT_PERMISSIONS: u64 = 1024 + 2048 + 64 + 65536 + 34_359_738_368 + 274_877_906_944;

#[derive(serde::Serialize)]
pub struct ServersResponse {
    pub guilds: Vec<GuildInfo>,
    /// Where to send someone to add the bot to another server.
    pub invite_url: String,
}

/// The servers the bot is in, plus how to add another.
///
/// Servers register themselves: the gateway's `guild_create` fires when the
/// bot joins (and for every server on reconnect), and records the name and
/// icon. There's no "add server" endpoint to POST to - authorising the bot on
/// Discord *is* the action, and trying to mirror that with our own callback
/// would just be a second way to get it wrong.
pub async fn list_servers(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<ServersResponse>, AppError> {
    let guilds = guilds::list_guilds(&state.db)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let client_id = &state.discord_client_id;
    let invite_url = format!(
        "https://discord.com/oauth2/authorize?client_id={client_id}&scope=bot&permissions={BOT_PERMISSIONS}"
    );

    Ok(Json(ServersResponse { guilds, invite_url }))
}

/// The channels the bot could post in, for the picker on `/server`.
///
/// Addressed by `guilds.id`, not by the Discord snowflake, for two reasons:
/// it matches what `GET /api/guilds` hands out, and it means the only guilds
/// reachable here are ones this app has a row for. Taking a snowflake
/// straight from the client would turn the bot token into a general-purpose
/// "list any server's channels" proxy for anyone with a session.
pub async fn list_channels(
    _claims: Claims,
    State(state): State<AppState>,
    Path(guild_id): Path<Uuid>,
) -> Result<Json<Vec<ChannelInfo>>, AppError> {
    let bot_token = state.discord_bot_token.as_deref().ok_or_else(|| {
        AppError::ValidationError("DISCORD_BOT_TOKEN is not configured".to_string())
    })?;

    // NotFound rather than a validation error: from the caller's side an
    // unknown guild id is an unknown resource, and saying anything more
    // would confirm which ids exist.
    let discord_guild_id = guilds::discord_id_of(&state.db, guild_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::NotFound)?;

    let channels = discord_feed::list_text_channels(
        &state.discord_api_base,
        &state.http_client,
        bot_token,
        &discord_guild_id,
    )
    .await
    .map_err(|e| AppError::ExternalApiError(e.to_string()))?;

    Ok(Json(channels))
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

    async fn seed_user(db: &PgPool) -> crate::models::User {
        create_or_update_user(
            db,
            crate::models::DiscordUser {
                id: "me-discord".to_string(),
                username: "me".to_string(),
                discriminator: "0".to_string(),
                avatar: None,
                email: None,
            },
        )
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn lists_registered_servers_and_how_to_add_another(db: PgPool) {
        let user = seed_user(&db).await;

        crate::services::guilds::upsert_guild_metadata(&db, "111", "The Hangout", Some("abc"))
            .await
            .unwrap();
        // A guild the bot knows the id of but hasn't seen a guild_create for
        // yet - e.g. seeded from DISCORD_GUILD_ID at startup.
        crate::services::guilds::ensure_guild(&db, "222")
            .await
            .unwrap();

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/guilds")
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
        let json: Value = serde_json::from_slice(&bytes).unwrap();

        let guilds = json["guilds"].as_array().unwrap();
        assert_eq!(guilds.len(), 2);

        let named = guilds
            .iter()
            .find(|g| g["discord_guild_id"] == "111")
            .unwrap();
        assert_eq!(named["name"], "The Hangout");
        assert_eq!(
            named["icon_url"],
            "https://cdn.discordapp.com/icons/111/abc.png"
        );

        // A server registered by id alone has no name yet; guild_create fills
        // it in on the next connect, and the UI has to cope until then.
        let unnamed = guilds
            .iter()
            .find(|g| g["discord_guild_id"] == "222")
            .unwrap();
        assert!(unnamed["name"].is_null());
        assert!(unnamed["icon_url"].is_null());

        let invite = json["invite_url"].as_str().unwrap();
        assert!(
            invite.starts_with("https://discord.com/oauth2/authorize?client_id=test-client-id")
        );
        assert!(invite.contains("scope=bot"));
    }

    #[sqlx::test]
    async fn requires_authentication(db: PgPool) {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let app = crate::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/guilds")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    // --- GET /api/guilds/:id/channels ------------------------------------

    async fn channels_response(body: serde_json::Value) -> wiremock::MockServer {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/guilds/111/channels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        server
    }

    #[sqlx::test]
    async fn lists_the_channels_the_bot_can_post_in(db: PgPool) {
        let user = seed_user(&db).await;
        let guild = crate::services::guilds::ensure_guild(&db, "111")
            .await
            .unwrap();

        let server = channels_response(serde_json::json!([
            { "id": "c1", "name": "general", "type": 0, "position": 0, "parent_id": null },
            { "id": "c2", "name": "Voice", "type": 2, "position": 1, "parent_id": null }
        ]))
        .await;

        let state = AppState::for_test(db, server.uri());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();

        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri(format!("/api/guilds/{guild}/channels"))
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
        let channels: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(channels.as_array().unwrap().len(), 1);
        assert_eq!(channels[0]["name"], "general");
        assert_eq!(channels[0]["id"], "c1");
    }

    // A snowflake taken straight from the client would make the bot token a
    // "list any server's channels" proxy for anyone with a session. The id
    // is ours, and an unknown one is simply not found.
    #[sqlx::test]
    async fn an_unregistered_guild_is_not_found(db: PgPool) {
        let user = seed_user(&db).await;
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();

        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri(format!("/api/guilds/{}/channels", uuid::Uuid::new_v4()))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn listing_channels_requires_auth(db: PgPool) {
        let guild = crate::services::guilds::ensure_guild(&db, "111")
            .await
            .unwrap();
        let state = AppState::for_test(db, "http://unused.invalid".to_string());

        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri(format!("/api/guilds/{guild}/channels"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
