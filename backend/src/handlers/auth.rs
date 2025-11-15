use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, Scope,
    TokenResponse,
};

use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use chrono::{Utc, Duration};

use crate::{
    config::AppState,
    models::{User, DiscordUser},
    services::auth::{create_or_update_user, get_user_by_discord_id},
    middleware::auth::Claims,
    error::AppError,
};

#[derive(Debug, Deserialize)]
pub struct AuthCallback {
    code: String,
    state: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

// Discord login - generates authorization URL
pub async fn discord_login(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = state
        .oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("identify".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("guilds".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // Store PKCE verifier with CSRF token as key
    let csrf_secret = csrf_token.secret().clone();
    tracing::info!("🔑 Storing PKCE verifier with FULL state: '{}'", csrf_secret); // Changed
    state.pkce_verifiers.lock().unwrap().insert(csrf_secret, pkce_verifier);

    tracing::info!("🔗 Full auth URL: {}", auth_url);

    Redirect::to(auth_url.as_str())
}

// Discord callback - exchanges code for token
pub async fn discord_callback(
    Query(params): Query<AuthCallback>,
    State(state): State<AppState>,
) -> Result<Redirect, AppError> {
    tracing::info!("📥 Received callback with code: {}, FULL state: '{}'", &params.code[..10], &params.state); // Changed
    
    // Debug: Show what keys exist
    {
        let verifiers = state.pkce_verifiers.lock().unwrap();
        tracing::info!("🔍 Available states in HashMap: {:?}", verifiers.keys().collect::<Vec<_>>());
    }
    
    // Retrieve PKCE verifier using the state (CSRF token)
    let pkce_verifier = {
        let mut verifiers = state.pkce_verifiers.lock().unwrap();
        verifiers.remove(&params.state)
            .ok_or_else(|| {
                tracing::error!("❌ PKCE verifier not found for FULL state: '{}'", &params.state);
                AppError::OAuth2Error("PKCE verifier not found".to_string())
            })?
    };

    tracing::info!("✅ Found PKCE verifier for state");

    // Exchange authorization code for access token with PKCE verifier
    let token_result = state
        .oauth_client
        .exchange_code(AuthorizationCode::new(params.code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(oauth2::reqwest::async_http_client)
        .await
        .map_err(|e| {
            tracing::error!("❌ OAuth2 token exchange failed: {:?}", e);
            AppError::OAuth2Error(format!("Token exchange failed: {}", e))
        })?;

    let access_token = token_result.access_token().secret();
    tracing::info!("✅ Got access token");

    // Fetch user info from Discord
    let client = reqwest::Client::new();
    let response = client
        .get("https://discord.com/api/users/@me")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("❌ Discord API request failed: {:?}", e);
            AppError::ExternalApiError(e.to_string())
        })?;

    let status = response.status();
    tracing::info!("📡 Discord API response status: {}", status);
    
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        tracing::error!("❌ Discord API error: {}", error_text);
        return Err(AppError::ExternalApiError(format!("Discord API returned {}: {}", status, error_text)));
    }

    let discord_user: DiscordUser = response
        .json()
        .await
        .map_err(|e| {
            tracing::error!("❌ Failed to parse Discord user JSON: {:?}", e);
            AppError::ExternalApiError(e.to_string())
        })?;

    tracing::info!("👤 Got Discord user: {} (ID: {})", discord_user.username, discord_user.id);

    // Create or update user in database
    tracing::info!("💾 Attempting to save user to database...");
    let user = create_or_update_user(&state.db, discord_user)
        .await
        .map_err(|e| {
            tracing::error!("❌ Database error: {:?}", e);
            AppError::DatabaseError(e.to_string())
        })?;

    tracing::info!("✅ User saved to database: {:?}", user.id);

    // Generate JWT
    let jwt_token = generate_jwt(&user.discord_id, &state.jwt_secret)
        .map_err(|e| {
            tracing::error!("❌ JWT generation failed: {:?}", e);
            AppError::JwtError(e.to_string())
        })?;

    tracing::info!("🎫 JWT token generated successfully");

    // Redirect to frontend with token
    let redirect_url = format!("{}?token={}", state.frontend_url, jwt_token);
    tracing::info!("🔀 Redirecting to: {}", redirect_url);
    
    Ok(Redirect::to(&redirect_url))
}

// Get current user from JWT
pub async fn get_current_user(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<User>, AppError> {
    let user = get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    Ok(Json(user))
}

// Logout (client-side JWT removal)
pub async fn logout() -> impl IntoResponse {
    StatusCode::OK
}

// JWT helper functions
fn generate_jwt(discord_id: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: discord_id.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

pub fn verify_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )
    .map(|data| data.claims)
}