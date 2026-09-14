use axum::{extract::State, Json};

use crate::{
    config::AppState,
    error::AppError,
    middleware::auth::Claims,
    models::{FriendInfo, SyncFriendsResult},
    services::friends,
};

// List your currently-synced friends (other app users sharing the
// configured Discord server with you).
pub async fn list_friends(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<Vec<FriendInfo>>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let friend_list = friends::get_friends(&state.db, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(friend_list))
}

// Re-sync your friend list against the configured Discord server's current
// member list. See services::friends for why this is guild co-membership
// rather than Discord's actual Friends list (Discord doesn't expose that to
// bots/apps).
pub async fn sync_friends(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<SyncFriendsResult>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let (bot_token, guild_id) = match (&state.discord_bot_token, &state.discord_guild_id) {
        (Some(token), Some(guild)) => (token, guild),
        _ => {
            return Err(AppError::ValidationError(
                "Friend sync isn't configured on this server (set DISCORD_BOT_TOKEN and DISCORD_GUILD_ID)"
                    .to_string(),
            ))
        }
    };

    let result = friends::sync_friends(
        &state.db,
        &state.http_client,
        bot_token,
        guild_id,
        user.id,
        &user.discord_id,
    )
    .await
    .map_err(|e| AppError::ExternalApiError(e.to_string()))?;

    tracing::info!(
        "🔄 Friend sync for {}: {} synced, {} removed",
        user.username,
        result.synced,
        result.removed
    );

    Ok(Json(result))
}
