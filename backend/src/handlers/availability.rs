use axum::{
    Json,
    extract::{Query, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{config::AppState, error::AppError, middleware::auth::Claims, services};

#[derive(Debug, Serialize)]
pub struct FreeNowResponse {
    pub free_friend_ids: Vec<Uuid>,
}

/// Which of the current user's synced friends are free right now - the
/// "Free tonight" bar's data, from the caller's own friend list
/// (services::friends::get_friends), not an arbitrary id list, so nobody
/// can probe a stranger's availability by guessing user ids.
pub async fn friends_now(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<FreeNowResponse>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let friends = services::friends::get_friends(&state.db, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    let friend_ids: Vec<Uuid> = friends.into_iter().map(|f| f.user_id).collect();

    let free_friend_ids = services::availability::free_users_now(&state.db, &friend_ids)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(FreeNowResponse { free_friend_ids }))
}

#[derive(Debug, Deserialize)]
pub struct WeekQuery {
    /// Which friend to compare against - must actually be a friend of the
    /// caller, checked below (same "your own friend list, not an arbitrary
    /// id" reasoning as friends_now).
    pub with: Uuid,
    pub week_start: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct DayAvailabilityResponse {
    pub date: DateTime<Utc>,
    pub free_user_ids: Vec<Uuid>,
}

pub async fn week(
    claims: Claims,
    State(state): State<AppState>,
    Query(query): Query<WeekQuery>,
) -> Result<Json<Vec<DayAvailabilityResponse>>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let friends = services::friends::get_friends(&state.db, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    if !friends.iter().any(|f| f.user_id == query.with) {
        return Err(AppError::ValidationError(
            "Not one of your friends".to_string(),
        ));
    }

    let days = services::availability::week_availability(
        &state.db,
        &[user.id, query.with],
        query.week_start,
    )
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(
        days.into_iter()
            .map(|d| DayAvailabilityResponse {
                date: d.date,
                free_user_ids: d.free_user_ids,
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use chrono::{Duration, Utc};
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

    async fn befriend(db: &PgPool, a: uuid::Uuid, b: uuid::Uuid) {
        for (x, y) in [(a, b), (b, a)] {
            sqlx::query(
                r#"
                INSERT INTO friendships (id, user_id, friend_id, source, synced_at, created_at)
                VALUES ($1, $2, $3, 'discord_guild', now(), now())
                "#,
            )
            .bind(uuid::Uuid::new_v4())
            .bind(x)
            .bind(y)
            .execute(db)
            .await
            .unwrap();
        }
    }

    #[sqlx::test]
    async fn friends_now_only_reports_actual_friends(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let friend = seed_user(&db, "friend-discord", "friend").await;
        let _stranger = seed_user(&db, "stranger-discord", "stranger").await;
        befriend(&db, me.id, friend.id).await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&me.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/availability/friends-now")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let free_ids = json["free_friend_ids"].as_array().unwrap();
        assert_eq!(free_ids.len(), 1);
        assert_eq!(free_ids[0], friend.id.to_string());
    }

    #[sqlx::test]
    async fn week_rejects_a_non_friend(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let stranger = seed_user(&db, "stranger-discord", "stranger").await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&me.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        // .replace: RFC3339's "+00:00" offset contains a literal '+', which
        // application/x-www-form-urlencoded query parsing (what axum's
        // Query extractor uses) treats as an encoded space - has to be
        // percent-encoded or the timestamp gets corrupted before it even
        // reaches the handler. desktop/src/lib/api.ts uses
        // URLSearchParams for this exact reason - it encodes automatically.
        let week_start = (Utc::now() + Duration::days(0))
            .to_rfc3339()
            .replace('+', "%2B");
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/availability/week?with={}&week_start={week_start}",
                        stranger.id
                    ))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test]
    async fn week_returns_seven_days_for_a_real_friend(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let friend = seed_user(&db, "friend-discord", "friend").await;
        befriend(&db, me.id, friend.id).await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&me.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);

        let week_start = Utc::now().to_rfc3339().replace('+', "%2B");
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/availability/week?with={}&week_start={week_start}",
                        friend.id
                    ))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 7);
    }
}
