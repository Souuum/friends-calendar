use oauth2::{
    AuthUrl, ClientId, ClientSecret, EndpointNotSet, EndpointSet, PkceCodeVerifier, RedirectUrl,
    TokenUrl, basic::BasicClient,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};

/// oauth2 5.x encodes which endpoints a client has configured in the type
/// itself, so `BasicClient` alone is no longer a complete type. This is the
/// shape this app builds: auth URI and token URI set, device-authorization /
/// introspection / revocation unused.
pub type DiscordOAuthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub oauth_client: DiscordOAuthClient,
    /// Also used to build the bot's invite URL (handlers::guilds), not just
    /// the OAuth login flow.
    pub discord_client_id: String,
    /// Dedicated HTTP client for the OAuth2 token exchange, built with
    /// redirects disabled. oauth2 5.x requires this: a redirect-following
    /// client can be steered into leaking the authorization code to another
    /// host (SSRF), so it must not be the general-purpose `http_client` below.
    pub oauth_http_client: reqwest::Client,
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
    // Base URL every services::friends Discord REST call is made against.
    // Always real Discord in production; tests point this at a
    // wiremock::MockServer instead so nothing ever hits the live API. See
    // .claude/skills/add-tests/SKILL.md.
    pub discord_api_base: String,
}

const DEFAULT_DISCORD_API_BASE: &str = "https://discord.com/api/v10";

fn build_oauth_http_client() -> reqwest::Client {
    reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("failed to build the OAuth2 HTTP client")
}

/// A required setting, where an *empty* value counts as missing.
///
/// `env::var` returns `Ok("")` for `FOO=` in a .env file, so a plain
/// `.expect()` accepts a blank as if it were configured. That matters
/// because the provisioning script writes the Discord keys as empty
/// placeholders to be filled in: leaving one blank used to start the server
/// with empty OAuth credentials and fail later, at Discord, with an error
/// that pointed nowhere near the cause. `DISCORD_BOT_TOKEN` already got this
/// right via `.filter(|s| !s.is_empty())`; the required ones did not.
fn require_env(key: &str) -> String {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => panic!("{key} must be set to a non-empty value (check /opt/friends-calendar/.env)"),
    }
}

impl AppState {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Database connection
        let database_url = require_env("DATABASE_URL");

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
        let discord_client_id_raw = require_env("DISCORD_CLIENT_ID");
        let discord_client_id = ClientId::new(discord_client_id_raw.clone());
        let discord_client_secret = ClientSecret::new(require_env("DISCORD_CLIENT_SECRET"));

        let auth_url = AuthUrl::new("https://discord.com/api/oauth2/authorize".to_string())
            .expect("Invalid authorization endpoint URL");
        let token_url = TokenUrl::new("https://discord.com/api/oauth2/token".to_string())
            .expect("Invalid token endpoint URL");

        // Where Discord sends the browser back to. This was hard-coded to
        // localhost:8080, which cannot work once deployed: the value is sent
        // to Discord as `redirect_uri`, Discord checks it against the
        // application's registered redirect list, and then sends the *user's
        // browser* there. In production that has to be the public API URL,
        // and the same string must be registered in the Discord developer
        // portal or the login is rejected before it starts.
        let public_api_url = env::var("PUBLIC_API_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "http://localhost:8080".to_string());
        let redirect_url = RedirectUrl::new(format!(
            "{}/api/auth/callback",
            public_api_url.trim_end_matches('/')
        ))
        .expect("PUBLIC_API_URL must be a valid URL, e.g. https://api.example.com");

        let oauth_client = BasicClient::new(discord_client_id)
            .set_client_secret(discord_client_secret)
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_url);

        let jwt_secret = require_env("JWT_SECRET");
        let frontend_url =
            env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:1420".to_string());

        // .filter(...) treats a present-but-blank var (e.g. `DISCORD_GUILD_ID=`
        // left unfilled in .env) the same as an unset one, rather than
        // silently trying to hit Discord with an empty guild id in the URL.
        let discord_bot_token = env::var("DISCORD_BOT_TOKEN").ok().filter(|s| !s.is_empty());
        let discord_guild_id = env::var("DISCORD_GUILD_ID").ok().filter(|s| !s.is_empty());
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
                    tracing::warn!(
                        "⚠️  DISCORD_ANNOUNCEMENT_CHANNEL_ID isn't a valid channel ID, ignoring it"
                    );
                    None
                }
            });

        let discord_api_base = env::var("DISCORD_API_BASE")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_DISCORD_API_BASE.to_string());

        Ok(Self {
            db,
            oauth_client,
            discord_client_id: discord_client_id_raw,
            oauth_http_client: build_oauth_http_client(),
            jwt_secret,
            frontend_url,
            pkce_verifiers: Arc::new(Mutex::new(HashMap::new())),
            discord_bot_token,
            discord_guild_id,
            http_client: reqwest::Client::new(),
            discord_announcement_channel_id,
            discord_api_base,
        })
    }
}

#[cfg(test)]
impl AppState {
    /// Minimal AppState for functional/router-level tests — skips env-var
    /// reading (and the real OAuth2/Discord config it requires) entirely.
    /// `oauth_client` is safe to construct directly like this: it's pure
    /// object construction, no network call happens until something
    /// actually exchanges a code, which no test here does.
    pub fn for_test(db: PgPool, discord_api_base: String) -> Self {
        let oauth_client = BasicClient::new(ClientId::new("test-client-id".to_string()))
            .set_client_secret(ClientSecret::new("test-client-secret".to_string()))
            .set_auth_uri(
                AuthUrl::new("https://discord.com/api/oauth2/authorize".to_string()).unwrap(),
            )
            .set_token_uri(
                TokenUrl::new("https://discord.com/api/oauth2/token".to_string()).unwrap(),
            );

        Self {
            db,
            oauth_client,
            discord_client_id: "test-client-id".to_string(),
            oauth_http_client: build_oauth_http_client(),
            jwt_secret: "test-jwt-secret".to_string(),
            frontend_url: "http://localhost:1420".to_string(),
            pkce_verifiers: Arc::new(Mutex::new(HashMap::new())),
            discord_bot_token: Some("test-bot-token".to_string()),
            discord_guild_id: Some("test-guild-id".to_string()),
            http_client: reqwest::Client::new(),
            discord_announcement_channel_id: Some(123456789),
            discord_api_base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::require_env;

    // Each test uses its own key: env is process-global and the test
    // harness runs threads in parallel, so a shared key would race.
    #[test]
    fn returns_a_configured_value() {
        unsafe { std::env::set_var("TEST_REQUIRE_ENV_OK", "hello") };
        assert_eq!(require_env("TEST_REQUIRE_ENV_OK"), "hello");
    }

    // The case that motivated this: the provisioning script writes the
    // Discord keys as empty placeholders, and `env::var` returns Ok("") for
    // `FOO=`, so `.expect()` used to sail straight past a blank.
    #[test]
    #[should_panic(expected = "must be set to a non-empty value")]
    fn an_empty_value_is_treated_as_missing() {
        unsafe { std::env::set_var("TEST_REQUIRE_ENV_EMPTY", "") };
        require_env("TEST_REQUIRE_ENV_EMPTY");
    }

    #[test]
    #[should_panic(expected = "must be set to a non-empty value")]
    fn a_whitespace_only_value_is_treated_as_missing() {
        unsafe { std::env::set_var("TEST_REQUIRE_ENV_BLANK", "   ") };
        require_env("TEST_REQUIRE_ENV_BLANK");
    }

    #[test]
    #[should_panic(expected = "must be set to a non-empty value")]
    fn an_absent_key_panics() {
        require_env("TEST_REQUIRE_ENV_DEFINITELY_NOT_SET");
    }
}
