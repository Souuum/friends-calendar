use axum::http::{HeaderValue, Method};
use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod bot;
mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod services;

use config::AppState;

/// The full application router. Pulled out of `main()` so functional
/// (router-level) tests can build and drive the exact same routing/CORS
/// setup the real server runs — see .claude/skills/add-tests/SKILL.md and
/// handlers::discord's test module for an example.
pub(crate) fn build_router(state: AppState) -> Router {
    // Build CORS layer - specific origins and headers when using credentials
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:1420".parse::<HeaderValue>().unwrap())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
        ])
        .allow_credentials(true);

    Router::new()
        .route("/", get(root))
        // Auth routes
        .route("/api/auth/discord", get(handlers::auth::discord_login))
        .route("/api/auth/callback", get(handlers::auth::discord_callback))
        .route("/api/auth/me", get(handlers::auth::get_current_user))
        .route("/api/auth/me", patch(handlers::profile::update_profile))
        .route("/api/auth/me", delete(handlers::profile::delete_account))
        .route("/api/auth/logout", post(handlers::auth::logout))
        // Calendar event routes
        .route(
            "/api/events/announcement-preview",
            post(handlers::calendar::preview_announcement),
        )
        .route("/api/events", post(handlers::calendar::create_event))
        .route("/api/events", get(handlers::calendar::list_events))
        .route("/api/events/:id", get(handlers::calendar::get_event))
        .route("/api/events/:id", put(handlers::calendar::update_event))
        .route("/api/events/:id", delete(handlers::calendar::delete_event))
        // Participant routes
        .route(
            "/api/events/:id/participants",
            post(handlers::calendar::invite_participants),
        )
        .route(
            "/api/events/:id/participation",
            put(handlers::calendar::update_participation),
        )
        .route(
            "/api/events/:id/participants/:user_id",
            delete(handlers::calendar::remove_participant),
        )
        // Servers the bot is in
        .route("/api/guilds", get(handlers::guilds::list_servers))
        // Friends routes
        .route("/api/friends", get(handlers::friends::list_friends))
        .route("/api/friends/sync", post(handlers::friends::sync_friends))
        // Discord bot routes
        .route(
            "/api/events/:id/link-discord",
            post(handlers::calendar::link_discord_message),
        )
        // Discord server info
        .route(
            "/api/discord/server",
            get(handlers::discord::get_linked_server),
        )
        // Notifications
        .route(
            "/api/notifications",
            get(handlers::notifications::list_notifications),
        )
        .route(
            "/api/notifications/unread-count",
            get(handlers::notifications::unread_count),
        )
        .route(
            "/api/notifications/read-all",
            post(handlers::notifications::mark_all_read),
        )
        .route(
            "/api/notifications/:id/read",
            post(handlers::notifications::mark_read),
        )
        // Friend requests
        .route(
            "/api/friend-requests",
            post(handlers::friend_requests::send_request),
        )
        .route(
            "/api/friend-requests",
            get(handlers::friend_requests::list_incoming),
        )
        .route(
            "/api/friend-requests/:id/accept",
            post(handlers::friend_requests::accept_request),
        )
        .route(
            "/api/friend-requests/:id/decline",
            post(handlers::friend_requests::decline_request),
        )
        .route(
            "/api/friend-requests/missing-members",
            get(handlers::friend_requests::missing_members),
        )
        .route(
            "/api/friend-requests/post-invite",
            post(handlers::friend_requests::post_invite),
        )
        // Availability
        .route(
            "/api/availability/friends-now",
            get(handlers::availability::friends_now),
        )
        .route("/api/availability/week", get(handlers::availability::week))
        // Discord bot channel config
        .route(
            "/api/discord/config",
            get(handlers::discord_config::get_config),
        )
        .route(
            "/api/discord/config",
            put(handlers::discord_config::update_config),
        )
        // Announcements: a mirror of the linked channel's Discord messages
        // (replaces the old event-RSVP-tracking /announcements view)
        .route(
            "/api/announcements",
            get(handlers::announcements::list_announcements),
        )
        .route(
            "/api/announcements/sync",
            post(handlers::announcements::sync_announcements),
        )
        .route(
            "/api/announcements/:id/replies",
            get(handlers::announcements::list_replies),
        )
        .route(
            "/api/announcements/:id/reply",
            post(handlers::announcements::post_reply),
        )
        .layer(cors)
        .with_state(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize application state
    let state = AppState::new().await?;

    // Make sure this deployment's guild has a row before anything tries to
    // attach a publication to it. Migration 014 can only seed from
    // discord_bot_config - a migration can't read DISCORD_GUILD_ID - so a
    // deployment that never saved channel config on /server would otherwise
    // have no guild at all. Idempotent.
    if let Some(guild_id) = &state.discord_guild_id {
        match services::guilds::ensure_guild(&state.db, guild_id).await {
            Ok(_) => tracing::info!("🏠 Guild {} registered", guild_id),
            Err(e) => tracing::error!("❌ Failed to register guild {}: {:?}", guild_id, e),
        }
    }

    // Start the Discord bot in the background if it's configured.
    //
    // Only the token is needed now. It used to also require
    // DISCORD_ANNOUNCEMENT_CHANNEL_ID, because the gateway handler captured a
    // single channel to filter reactions against - it no longer filters by
    // channel at all, so the bot can watch every server it's in whether or
    // not any announcement channel has been configured.
    if let Some(bot_token) = &state.discord_bot_token {
        let bot_token = bot_token.clone();
        let db_clone = state.db.clone();
        tokio::spawn(async move {
            if let Err(e) = bot::DiscordBot::start(bot_token, db_clone).await {
                tracing::error!("❌ Discord bot error: {:?}", e);
            }
        });
    } else {
        tracing::warn!("⚠️  DISCORD_BOT_TOKEN not set — Discord bot will not start");
    }

    // Weekly announcements digest — same conditional-spawn shape as the
    // Discord bot above. Only needs bot_token + guild_id (not the
    // announcement-channel env var): the actual send target is resolved
    // per-tick from services::discord_config, since it's meant to follow
    // whatever's configured on the /server page.
    if let (Some(bot_token), Some(guild_id)) = (&state.discord_bot_token, &state.discord_guild_id) {
        let bot_token = bot_token.clone();
        let guild_id = guild_id.clone();
        let db_clone = state.db.clone();
        let http_clone = state.http_client.clone();
        let base_url = state.discord_api_base.clone();
        tokio::spawn(services::digest::spawn_digest_loop(
            base_url, db_clone, http_clone, bot_token, guild_id,
        ));
    }

    // Reminders need a bot token (to post in the event's thread) but no
    // guild id - unlike the digest, they're addressed per event and per
    // user, not per guild.
    if let Some(bot_token) = &state.discord_bot_token {
        tokio::spawn(services::reminders::spawn_reminder_loop(
            state.discord_api_base.clone(),
            state.db.clone(),
            state.http_client.clone(),
            bot_token.clone(),
        ));
    }

    let app = build_router(state);

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    tracing::info!("🚀 Server starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> &'static str {
    "Friends Calendar API - Discord OAuth2"
}
