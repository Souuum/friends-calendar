use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{config::AppState, error::AppError, middleware::auth::Claims, models::FriendRequestInfo, services::friend_requests};

#[derive(Debug, Deserialize)]
pub struct SendFriendRequestBody {
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct SendFriendRequestResponse {
    pub status: String, // "sent" | "auto_accepted"
}

pub async fn send_request(
    claims: Claims,
    State(state): State<AppState>,
    Json(body): Json<SendFriendRequestBody>,
) -> Result<Json<SendFriendRequestResponse>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let outcome = friend_requests::send_request(&state.db, user.id, body.username.trim())
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    use friend_requests::SendRequestOutcome::*;
    match outcome {
        Sent => Ok(Json(SendFriendRequestResponse { status: "sent".to_string() })),
        AutoAccepted => Ok(Json(SendFriendRequestResponse { status: "auto_accepted".to_string() })),
        UserNotFound => Err(AppError::NotFound),
        CannotRequestSelf => Err(AppError::ValidationError(
            "You can't send a friend request to yourself".to_string(),
        )),
        AlreadyFriends => Err(AppError::ValidationError("You're already friends".to_string())),
        AlreadyPending => Err(AppError::ValidationError("A request is already pending".to_string())),
    }
}

pub async fn list_incoming(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<Vec<FriendRequestInfo>>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let requests = friend_requests::list_incoming(&state.db, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(requests))
}

async fn respond(claims: Claims, state: AppState, request_id: Uuid, accept: bool) -> Result<impl IntoResponse, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let outcome = friend_requests::respond(&state.db, request_id, user.id, accept)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    match outcome {
        friend_requests::RespondOutcome::Responded => Ok(StatusCode::NO_CONTENT),
        friend_requests::RespondOutcome::NotFound => Err(AppError::NotFound),
    }
}

pub async fn accept_request(
    claims: Claims,
    State(state): State<AppState>,
    Path(request_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    respond(claims, state, request_id, true).await
}

pub async fn decline_request(
    claims: Claims,
    State(state): State<AppState>,
    Path(request_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    respond(claims, state, request_id, false).await
}

#[derive(Debug, Serialize)]
pub struct MissingMembersResponse {
    pub count: usize,
}

pub async fn missing_members(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<MissingMembersResponse>, AppError> {
    let (bot_token, guild_id) = match (&state.discord_bot_token, &state.discord_guild_id) {
        (Some(t), Some(g)) => (t, g),
        _ => return Err(AppError::ValidationError("No Discord server is linked".to_string())),
    };

    let count = friend_requests::count_guild_members_without_accounts(
        &state.db,
        &state.discord_api_base,
        &state.http_client,
        bot_token,
        guild_id,
    )
    .await
    .map_err(|e| AppError::ExternalApiError(e.to_string()))?;

    Ok(Json(MissingMembersResponse { count }))
}

pub async fn post_invite(_claims: Claims, State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let (bot_token, guild_id) = match (&state.discord_bot_token, &state.discord_guild_id) {
        (Some(t), Some(g)) => (t, g),
        _ => return Err(AppError::ValidationError("No Discord server is linked".to_string())),
    };
    let Some(channel_id) = state.discord_announcement_channel_id else {
        return Err(AppError::ValidationError("No announcement channel configured".to_string()));
    };

    let count = friend_requests::count_guild_members_without_accounts(
        &state.db,
        &state.discord_api_base,
        &state.http_client,
        bot_token,
        guild_id,
    )
    .await
    .map_err(|e| AppError::ExternalApiError(e.to_string()))?;

    friend_requests::post_guild_invite_prompt(
        &state.discord_api_base,
        &state.http_client,
        bot_token,
        &channel_id.to_string(),
        count,
    )
    .await
    .map_err(|e| AppError::ExternalApiError(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
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

    async fn seed_user(db: &PgPool, discord_id: &str, username: &str) -> crate::models::User {
        create_or_update_user(
            db,
            crate::models::DiscordUser {
                id: discord_id.to_string(),
                username: username.to_string(),
                discriminator: "0".to_string(),
                avatar: None,
                email: None,
            },
        )
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn send_list_and_accept_over_http(db: PgPool) {
        let alice = seed_user(&db, "alice-discord", "alice").await;
        let bob = seed_user(&db, "bob-discord", "bob").await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let alice_token = generate_jwt(&alice.discord_id, &state.jwt_secret).unwrap();
        let bob_token = generate_jwt(&bob.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        // alice sends bob a request
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/friend-requests")
                    .header("Authorization", format!("Bearer {alice_token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"username":"bob"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "sent");

        // bob sees it incoming
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/friend-requests")
                    .header("Authorization", format!("Bearer {bob_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let requests: Value = serde_json::from_slice(&body).unwrap();
        let requests = requests.as_array().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["from_username"], "alice");
        let request_id = requests[0]["id"].as_str().unwrap().to_string();

        // bob accepts
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/friend-requests/{request_id}/accept"))
                    .header("Authorization", format!("Bearer {bob_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // now they're friends
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/friends")
                    .header("Authorization", format!("Bearer {alice_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let friends: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(friends.as_array().unwrap().len(), 1);
        assert_eq!(friends[0]["username"], "bob");
    }

    #[sqlx::test]
    async fn rejects_a_request_to_yourself_with_400(db: PgPool) {
        let alice = seed_user(&db, "alice-discord", "alice").await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&alice.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/friend-requests")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"username":"alice"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test]
    async fn requires_auth(db: PgPool) {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let app = crate::build_router(state);

        let response = app
            .oneshot(Request::builder().uri("/api/friend-requests").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
