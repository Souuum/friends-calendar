use axum::http::{HeaderValue, Method};
use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use std::env;
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

/// Origins the API accepts browser requests from.
///
/// This used to be the single hard-coded literal `http://localhost:1420` -
/// the Tauri *dev server* origin. That is not the origin of anything in
/// production: a packaged Tauri app sends `tauri://localhost` (macOS/Linux)
/// or `https://tauri.localhost` (Windows), and a hosted web build sends its
/// own domain. `FRONTEND_URL` already existed on `AppState` but only fed the
/// OAuth redirect, so a deployed API answered the redirect and then had every
/// subsequent request blocked by CORS.
///
/// `allow_credentials(true)` forbids the `*` wildcard, so this has to be an
/// explicit list rather than "allow anything".
fn allowed_origins(frontend_url: &str) -> Vec<HeaderValue> {
    [
        frontend_url,
        // Kept so `yarn tauri:dev` / `vite dev` still work against a
        // deployed API without needing FRONTEND_URL repointed.
        "http://localhost:1420",
        "tauri://localhost",
        "https://tauri.localhost",
    ]
    .iter()
    // A malformed FRONTEND_URL drops that one entry rather than panicking
    // the whole server at startup over a config typo.
    .filter_map(|origin| origin.trim_end_matches('/').parse::<HeaderValue>().ok())
    // FRONTEND_URL defaults to the dev origin, which is also in the list
    // below it - dedupe so it isn't sent twice.
    .fold(Vec::new(), |mut acc, origin| {
        if !acc.contains(&origin) {
            acc.push(origin);
        }
        acc
    })
}

/// The full application router. Pulled out of `main()` so functional
/// (router-level) tests can build and drive the exact same routing/CORS
/// setup the real server runs — see .claude/skills/add-tests/SKILL.md and
/// handlers::discord's test module for an example.
pub(crate) fn build_router(state: AppState) -> Router {
    // Build CORS layer - specific origins and headers when using credentials
    let cors = CorsLayer::new()
        .allow_origin(allowed_origins(&state.frontend_url))
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
        // Creator-only, rate-limited in the DB - see services::nudge.
        .route(
            "/api/events/:id/nudge",
            post(handlers::calendar::nudge_no_answers),
        )
        .route(
            "/api/events/sync-reactions",
            post(handlers::calendar::sync_reactions),
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
        // Channels the bot can post in, for the /server picker - so nobody
        // has to paste a snowflake.
        .route(
            "/api/guilds/:id/channels",
            get(handlers::guilds::list_channels),
        )
        // Friends routes
        .route("/api/friends", get(handlers::friends::list_friends))
        .route("/api/friends/sync", post(handlers::friends::sync_friends))
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
        // Binds an already-posted message to a new event, instead of
        // announcing one - see services::event_adoption.
        .route(
            "/api/announcements/:id/adopt",
            post(handlers::announcements::adopt_announcement),
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

    // One backfill pass at startup, so reactions that arrived while the bot
    // was down - or that predate the event ever being announced through this
    // app - become RSVPs without anyone having to ask. Spawned rather than
    // awaited: it makes one Discord call per announced message, and the
    // server should not wait on that to start listening. Idempotent, so
    // running it on every boot is harmless.
    if let Some(bot_token) = &state.discord_bot_token {
        let db = state.db.clone();
        let base = state.discord_api_base.clone();
        let http = state.http_client.clone();
        let token = bot_token.clone();
        tokio::spawn(async move {
            match services::reaction_sync::sync_all(&db, &base, &http, &token).await {
                Ok(report) => tracing::info!(
                    "✅ Reaction backfill: {} message(s), {} reaction(s), {} RSVP(s) recorded",
                    report.messages_checked,
                    report.reactions_seen,
                    report.rsvps_recorded
                ),
                Err(e) => tracing::warn!("⚠️  Reaction backfill failed: {e}"),
            }
        });
    }

    let app = build_router(state);

    // Start server
    // Configurable because where this binds depends on where the Cloudflare
    // tunnel runs. `cloudflared` inside the same container reaches
    // 127.0.0.1; a tunnel on the Proxmox host (or any other machine) cannot,
    // and needs BIND_ADDR=0.0.0.0:8080. Defaults to loopback so that
    // widening the exposure is always a deliberate act.
    //
    // There is no authentication in front of this port - every route is
    // guarded by the JWT middleware, but 0.0.0.0 still means anything on the
    // LAN can reach it, so it belongs on a trusted network only.
    let addr: SocketAddr = env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
        .parse()
        .expect("BIND_ADDR must look like 127.0.0.1:8080 or 0.0.0.0:8080");
    tracing::info!("🚀 Server starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> &'static str {
    "Friends Calendar API - Discord OAuth2"
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use sqlx::PgPool;
    use tower::ServiceExt;

    #[test]
    fn the_configured_frontend_origin_is_allowed() {
        let origins = allowed_origins("https://calendar.example.com");
        assert!(origins.contains(&HeaderValue::from_static("https://calendar.example.com")));
    }

    // A packaged Tauri app does not send the dev-server origin, which is all
    // the old hard-coded CorsLayer accepted.
    #[test]
    fn packaged_tauri_origins_are_allowed() {
        let origins = allowed_origins("https://calendar.example.com");
        assert!(origins.contains(&HeaderValue::from_static("tauri://localhost")));
        assert!(origins.contains(&HeaderValue::from_static("https://tauri.localhost")));
    }

    #[test]
    fn the_dev_origin_survives_a_production_frontend_url() {
        let origins = allowed_origins("https://calendar.example.com");
        assert!(origins.contains(&HeaderValue::from_static("http://localhost:1420")));
    }

    // FRONTEND_URL defaults to the dev origin, which is also hard-coded in
    // the list - it must not be sent twice.
    #[test]
    fn the_default_frontend_url_is_not_duplicated() {
        let origins = allowed_origins("http://localhost:1420");
        let count = origins
            .iter()
            .filter(|o| *o == HeaderValue::from_static("http://localhost:1420"))
            .count();
        assert_eq!(count, 1, "dev origin listed {count} times");
    }

    // A trailing slash is the most likely way FRONTEND_URL gets written by
    // hand; an Origin header never has one, so it would never match.
    #[test]
    fn a_trailing_slash_in_frontend_url_is_normalised() {
        let origins = allowed_origins("https://calendar.example.com/");
        assert!(origins.contains(&HeaderValue::from_static("https://calendar.example.com")));
    }

    #[test]
    fn a_malformed_frontend_url_does_not_take_the_server_down() {
        // Newlines can't go in a header value; the entry is dropped and the
        // rest of the list survives.
        let origins = allowed_origins("https://bad\norigin");
        assert!(origins.contains(&HeaderValue::from_static("tauri://localhost")));
    }

    // The real router, so this can't pass while the deployed server rejects
    // the browser - the exact gap that let the hard-coded origin survive.
    #[sqlx::test]
    async fn preflight_from_the_configured_frontend_is_accepted(db: PgPool) {
        let mut state = AppState::for_test(db, "http://unused.invalid".to_string());
        state.frontend_url = "https://calendar.example.com".to_string();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/events")
                    .header("Origin", "https://calendar.example.com")
                    .header("Access-Control-Request-Method", "GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .and_then(|v| v.to_str().ok()),
            Some("https://calendar.example.com")
        );
    }

    #[sqlx::test]
    async fn preflight_from_an_unrelated_origin_is_not_granted(db: PgPool) {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/events")
                    .header("Origin", "https://evil.example.com")
                    .header("Access-Control-Request-Method", "GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response
                .headers()
                .get("access-control-allow-origin")
                .is_none(),
            "an unlisted origin must not be granted CORS"
        );
    }
}
