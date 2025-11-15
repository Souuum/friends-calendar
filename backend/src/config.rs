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
            .unwrap_or_else(|_| "http://localhost:5173".to_string());

        Ok(Self {
            db,
            oauth_client,
            jwt_secret,
            frontend_url,
            pkce_verifiers: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}