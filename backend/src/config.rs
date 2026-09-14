use oauth2::{
    basic::BasicClient,
    AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl, PkceCodeVerifier,
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub oauth_client: BasicClient,
    pub jwt_secret: String,
    pub frontend_url: String,
    pub pkce_verifiers: Arc<Mutex<HashMap<String, PkceCodeVerifier>>>,
    // Used by services::friends to sync friend lists via the Discord bot's
    // REST API. Optional: unlike the OAuth vars above, the server still
    // boots without these — /api/friends/sync just returns a clear 400
    // until they're configured, instead of failing the whole process.
    pub discord_bot_token: Option<String>,
    pub discord_guild_id: Option<String>,
    pub http_client: reqwest::Client,
    // Used by the Discord gateway bot (bot.rs / services::discord_announcement)
    // to know which channel to post event announcements in and watch for
    // ✅-reaction RSVPs. Same "optional, degrade gracefully" treatment as
    // the friend-sync fields above — see main.rs.
    pub discord_announcement_channel_id: Option<u64>,
}

impl AppState {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Database connection
        let database_url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");
        
        tracing::info!("🔌 Connecting to database...");
        
        let db = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;

        tracing::info!("✅ Database connected successfully");

        // Run migrations
        tracing::info!("🔄 Running migrations...");
        sqlx::migrate!("./migrations").run(&db).await?;
        tracing::info!("✅ Migrations complete");

        // OAuth2 client setup
        let discord_client_id = ClientId::new(
            env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID must be set")
        );
        let discord_client_secret = ClientSecret::new(
            env::var("DISCORD_CLIENT_SECRET").expect("DISCORD_CLIENT_SECRET must be set")
        );
        
        let auth_url = AuthUrl::new("https://discord.com/api/oauth2/authorize".to_string())
            .expect("Invalid authorization endpoint URL");
        let token_url = TokenUrl::new("https://discord.com/api/oauth2/token".to_string())
            .expect("Invalid token endpoint URL");
        
        let redirect_url = RedirectUrl::new("http://localhost:8080/api/auth/callback".to_string())
            .expect("Invalid redirect URL");

        let oauth_client = BasicClient::new(
            discord_client_id,
            Some(discord_client_secret),
            auth_url,
            Some(token_url),
        )
        .set_redirect_uri(redirect_url);

        let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
        let frontend_url = env::var("FRONTEND_URL")
            .unwrap_or_else(|_| "http://localhost:1420".to_string());

        // .filter(...) treats a present-but-blank var (e.g. `DISCORD_GUILD_ID=`
        // left unfilled in .env) the same as an unset one, rather than
        // silently trying to hit Discord with an empty guild id in the URL.
        let discord_bot_token = env::var("DISCORD_BOT_TOKEN")
            .ok()
            .filter(|s| !s.is_empty());
        let discord_guild_id = env::var("DISCORD_GUILD_ID")
            .ok()
            .filter(|s| !s.is_empty());
        if discord_bot_token.is_none() || discord_guild_id.is_none() {
            tracing::warn!(
                "⚠️  DISCORD_BOT_TOKEN / DISCORD_GUILD_ID not set — /api/friends/sync will be unavailable"
            );
        }

        let discord_announcement_channel_id = env::var("DISCORD_ANNOUNCEMENT_CHANNEL_ID")
            .ok()
            .filter(|s| !s.is_empty())
            .and_then(|s| match s.parse() {
                Ok(id) => Some(id),
                Err(_) => {
                    tracing::warn!("⚠️  DISCORD_ANNOUNCEMENT_CHANNEL_ID isn't a valid channel ID, ignoring it");
                    None
                }
            });

        Ok(Self {
            db,
            oauth_client,
            jwt_secret,
            frontend_url,
            pkce_verifiers: Arc::new(Mutex::new(HashMap::new())),
            discord_bot_token,
            discord_guild_id,
            http_client: reqwest::Client::new(),
            discord_announcement_channel_id,
        })
    }
}