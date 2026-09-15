use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use crate::{
    config::AppState,
    error::AppError,
    middleware::auth::Claims,
    models::{DeleteAccountRequest, UpdateProfileRequest, User},
    services::profile,
};

pub async fn update_profile(
    claims: Claims,
    State(state): State<AppState>,
    Json(req): Json<UpdateProfileRequest>,
) -> Result<Json<User>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let updated = profile::update_profile(&state.db, user.id, req)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::NotFound)?;

    Ok(Json(updated))
}

pub async fn delete_account(
    claims: Claims,
    State(state): State<AppState>,
    Json(req): Json<DeleteAccountRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let outcome = profile::delete_account(&state.db, user.id, &req.confirm_username)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    match outcome {
        profile::DeleteAccountOutcome::Deleted => {
            tracing::warn!("🗑️  Account deleted: {} ({})", user.username, user.id);
            Ok(StatusCode::NO_CONTENT)
        }
        profile::DeleteAccountOutcome::ConfirmationMismatch => Err(AppError::ValidationError(
            "Username confirmation didn't match".to_string(),
        )),
    }
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
    async fn updates_profile_over_http(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/auth/me")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"display_name":"Me!"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["display_name"], "Me!");
    }

    #[sqlx::test]
    async fn delete_account_400s_on_a_confirmation_mismatch_and_keeps_the_account(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/auth/me")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"confirm_username":"not-me"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // still logged in / account still exists
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/me")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn delete_account_removes_the_account_when_confirmed(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/auth/me")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"confirm_username":"me"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // token now refers to a deleted user - Claims resolution fails Unauthorized
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/me")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
