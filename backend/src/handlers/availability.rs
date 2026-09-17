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

#[derive(Debug, Deserialize)]
pub struct BestSlotQuery {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    /// How long the thing being planned runs. The overlap answer depends on
    /// it - seven people may be free for an hour and three for four hours -
    /// so the client sends the duration it actually has in the form.
    #[serde(default = "default_slot_minutes")]
    pub duration_minutes: i64,
    /// The requester's offset from UTC, so "evening" means their evening.
    /// Sent by the client rather than read from `users.timezone`: that
    /// column is free text, defaults to UTC and is filled in by almost
    /// nobody, while the browser knows the real answer.
    #[serde(default)]
    pub tz_offset_minutes: i64,
}

fn default_slot_minutes() -> i64 {
    120
}

#[derive(Debug, Serialize)]
pub struct BestSlotResponse {
    /// Best first. Several, because the calendar bar shows one and the
    /// create form wants alternatives - two endpoints for one computation
    /// would drift.
    pub slots: Vec<SlotResponse>,
}

#[derive(Debug, Serialize)]
pub struct SlotResponse {
    pub start: DateTime<Utc>,
    /// How many *friends* are free. The caller isn't counted - they know
    /// whether they're free, and "7 free" reading as 6 friends plus yourself
    /// is a worse number than 7 friends.
    pub free_count: usize,
    pub free_friend_ids: Vec<Uuid>,
}

/// When the group could actually meet.
///
/// Ranked over the caller's friends, and **filtered to slots the caller is
/// also free for** - suggesting a time you are busy is worse than
/// suggesting nothing. See `services::availability::rank_slots` for the
/// candidate set and the timezone caveat.
pub async fn best_slot(
    claims: Claims,
    State(state): State<AppState>,
    Query(query): Query<BestSlotQuery>,
) -> Result<Json<BestSlotResponse>, AppError> {
    if query.to <= query.from {
        return Err(AppError::ValidationError(
            "`to` must be after `from`".to_string(),
        ));
    }
    if query.duration_minutes <= 0 {
        return Err(AppError::ValidationError(
            "`duration_minutes` must be positive".to_string(),
        ));
    }

    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    // The caller's own friends, never an id list from the request - the same
    // rule `friends_now` follows, so this can't become a way to probe a
    // stranger's calendar.
    let friends = services::friends::get_friends(&state.db, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    let friend_ids: Vec<Uuid> = friends.into_iter().map(|f| f.user_id).collect();

    // The caller is included in the computation so their own commitments can
    // rule a slot out, then stripped from the count below.
    let mut everyone = friend_ids.clone();
    everyone.push(user.id);

    let slots = services::availability::best_slots(
        &state.db,
        &everyone,
        query.from,
        query.to,
        query.duration_minutes,
        query.tz_offset_minutes,
        // Ranked over every candidate; trimmed after filtering below.
        usize::MAX,
    )
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let slots: Vec<SlotResponse> = slots
        .into_iter()
        .filter(|slot| slot.free_user_ids.contains(&user.id))
        .map(|slot| {
            let free_friend_ids: Vec<Uuid> = slot
                .free_user_ids
                .into_iter()
                .filter(|id| *id != user.id)
                .collect();
            SlotResponse {
                start: slot.start,
                free_count: free_friend_ids.len(),
                free_friend_ids,
            }
        })
        .take(5)
        .collect();

    Ok(Json(BestSlotResponse { slots }))
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
    // --- GET /api/availability/best-slot ---------------------------------

    async fn best_slot_request(
        db: PgPool,
        user: &crate::models::User,
        query: &str,
    ) -> axum::http::Response<Body> {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri(format!("/api/availability/best-slot?{query}"))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    /// ⚠️ `Z` form, not `to_rfc3339()`'s `+00:00`: a bare `+` in a query
    /// string decodes as a space, so the offset form 400s unless the client
    /// percent-encodes it. `Date.toISOString()` - what the real client sends -
    /// is the `Z` form, so this matches it.
    fn iso(at: chrono::DateTime<Utc>) -> String {
        at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    fn week_query() -> String {
        let from = Utc::now();
        let to = from + Duration::days(7);
        format!(
            "from={}&to={}&duration_minutes=120&tz_offset_minutes=0",
            iso(from),
            iso(to)
        )
    }

    #[sqlx::test]
    async fn best_slot_ranks_over_the_callers_friends(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let friend = seed_user(&db, "friend-discord", "friend").await;
        befriend(&db, me.id, friend.id).await;

        let response = best_slot_request(db, &me, &week_query()).await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let slots = json["slots"].as_array().unwrap();

        assert!(!slots.is_empty(), "a free week should offer some slots");
        // The caller is not counted among the free friends - "7 free"
        // meaning six friends plus yourself is a worse number.
        assert_eq!(slots[0]["free_count"], 1);
        assert_eq!(slots[0]["free_friend_ids"].as_array().unwrap().len(), 1);
    }

    // A stranger's availability must not leak, and the only ids that reach
    // the computation are the caller's own friends.
    #[sqlx::test]
    async fn best_slot_ignores_people_who_are_not_your_friends(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        seed_user(&db, "stranger-discord", "stranger").await;

        let response = best_slot_request(db, &me, &week_query()).await;

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let slots = json["slots"].as_array().unwrap();

        assert!(slots.iter().all(|s| s["free_count"] == 0));
    }

    #[sqlx::test]
    async fn best_slot_rejects_a_backwards_window(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let now = Utc::now();
        let query = format!("from={}&to={}", iso(now), iso(now - Duration::days(1)));

        let response = best_slot_request(db, &me, &query).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test]
    async fn best_slot_requires_auth(db: PgPool) {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/availability/best-slot?from=2026-03-01T00:00:00Z&to=2026-03-08T00:00:00Z")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
