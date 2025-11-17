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
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize application state
    let state = AppState::new().await?;

    // Start Discord bot in background
    let bot_token = env::var("DISCORD_BOT_TOKEN").expect("DISCORD_BOT_TOKEN must be set");
    let announcement_channel_id: u64 = env::var("DISCORD_ANNOUNCEMENT_CHANNEL_ID")
        .expect("DISCORD_ANNOUNCEMENT_CHANNEL_ID must be set")
        .parse()
        .expect("Invalid channel ID");

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

    let db_clone = state.db.clone();
    tokio::spawn(async move {
        if let Err(e) = bot::DiscordBot::start(bot_token, db_clone, announcement_channel_id).await {
            tracing::error!("❌ Discord bot error: {:?}", e);
        }
    });

    // Build application routes
    let app = Router::new()
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
        //bot routes
        .route("/api/events/:id/link-discord", post(handlers::calendar::link_discord_message))
        .layer(cors)
        .with_state(state);

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