use axum::{
    Router,
    routing::{get, post, put, delete},
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tracing_subscriber;
use axum::http::{HeaderValue, Method};

mod config;
mod models;
mod handlers;
mod services;
mod middleware;
mod error;
mod bot;

use config::AppState;

/// The full application router. Pulled out of `main()` so functional
/// (router-level) tests can build and drive the exact same routing/CORS
/// setup the real server runs — see .claude/skills/add-tests/SKILL.md and
/// handlers::discord's test module for an example.
pub(crate) fn build_router(state: AppState) -> Router {
    // Build CORS layer - specific origins and headers when using credentials
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:1420".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
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
        .route("/api/auth/logout", post(handlers::auth::logout))
        // Calendar event routes
        .route("/api/events", post(handlers::calendar::create_event))
        .route("/api/events", get(handlers::calendar::list_events))
        .route("/api/events/:id", get(handlers::calendar::get_event))
        .route("/api/events/:id", put(handlers::calendar::update_event))
        .route("/api/events/:id", delete(handlers::calendar::delete_event))
        // Participant routes
        .route("/api/events/:id/participants", post(handlers::calendar::invite_participants))
        .route("/api/events/:id/participation", put(handlers::calendar::update_participation))
        .route("/api/events/:id/participants/:user_id", delete(handlers::calendar::remove_participant))
        // Friends routes
        .route("/api/friends", get(handlers::friends::list_friends))
        .route("/api/friends/sync", post(handlers::friends::sync_friends))
        // Discord bot routes
        .route("/api/events/:id/link-discord", post(handlers::calendar::link_discord_message))
        // Discord server info
        .route("/api/discord/server", get(handlers::discord::get_linked_server))
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

    // Start the Discord bot in the background if it's configured. Unlike
    // this branch's original .expect()-based setup, missing config here
    // doesn't take down the whole backend — same reasoning as friend sync
    // in services::friends: a Discord integration being unconfigured
    // shouldn't block booting the rest of the API.
    match (&state.discord_bot_token, state.discord_announcement_channel_id) {
        (Some(bot_token), Some(announcement_channel_id)) => {
            let bot_token = bot_token.clone();
            let db_clone = state.db.clone();
            tokio::spawn(async move {
                if let Err(e) = bot::DiscordBot::start(bot_token, db_clone, announcement_channel_id).await {
                    tracing::error!("❌ Discord bot error: {:?}", e);
                }
            });
        }
        _ => {
            tracing::warn!(
                "⚠️  DISCORD_BOT_TOKEN / DISCORD_ANNOUNCEMENT_CHANNEL_ID not set — Discord bot will not start"
            );
        }
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
