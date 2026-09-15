use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{config::AppState, error::AppError, middleware::auth::Claims, models::NotificationInfo, services::notifications};

const DEFAULT_LIST_LIMIT: i64 = 50;

pub async fn list_notifications(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<Vec<NotificationInfo>>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let list = notifications::list(&state.db, user.id, DEFAULT_LIST_LIMIT)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(list))
}

#[derive(Debug, Serialize)]
pub struct UnreadCountResponse {
    pub count: i64,
}

pub async fn unread_count(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<UnreadCountResponse>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let count = notifications::unread_count(&state.db, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(UnreadCountResponse { count }))
}

pub async fn mark_read(
    claims: Claims,
    State(state): State<AppState>,
    Path(notification_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let found = notifications::mark_read(&state.db, user.id, notification_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    if !found {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn mark_all_read(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    notifications::mark_all_read(&state.db, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

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
    use crate::services::notifications;

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

    async fn get(app: axum::Router, token: &str, uri: &str) -> axum::http::Response<Body> {
        app.oneshot(
            Request::builder()
                .uri(uri)
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    }

    async fn post(app: axum::Router, token: &str, uri: &str) -> axum::http::Response<Body> {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn list_and_unread_count_over_http(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;
        notifications::create(&db, user.id, "event_invite", None, None, "hi")
            .await
            .unwrap();

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = get(app.clone(), &token, "/api/notifications").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 1);

        let response = get(app, &token, "/api/notifications/unread-count").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["count"], 1);
    }

    #[sqlx::test]
    async fn requires_auth(db: PgPool) {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let app = crate::build_router(state);

        let response = app
            .oneshot(Request::builder().uri("/api/notifications").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test]
    async fn mark_read_404s_for_someone_elses_notification(db: PgPool) {
        let owner = seed_user(&db, "owner-discord", "owner").await;
        let other = seed_user(&db, "other-discord", "other").await;
        notifications::create(&db, owner.id, "event_invite", None, None, "hi")
            .await
            .unwrap();
        let notification_id = notifications::list(&db, owner.id, 10).await.unwrap()[0].id;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let other_token = generate_jwt(&other.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = post(app, &other_token, &format!("/api/notifications/{notification_id}/read")).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn mark_all_read_clears_unread_count(db: PgPool) {
        let user = seed_user(&db, "me-discord", "me").await;
        notifications::create(&db, user.id, "event_invite", None, None, "one").await.unwrap();
        notifications::create(&db, user.id, "event_invite", None, None, "two").await.unwrap();

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = post(app.clone(), &token, "/api/notifications/read-all").await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let response = get(app, &token, "/api/notifications/unread-count").await;
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["count"], 0);
    }
}
