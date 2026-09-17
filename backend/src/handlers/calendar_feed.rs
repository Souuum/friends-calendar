use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
};

use crate::{config::AppState, error::AppError, middleware::auth::Claims, services::calendar_feed};

#[derive(serde::Serialize)]
pub struct FeedLink {
    /// The full URL to paste into a calendar app.
    pub url: String,
}

fn feed_url(state: &AppState, token: &str) -> String {
    // The API's own origin, not the frontend's: this is fetched by Google's
    // servers, not by a browser tab on the site.
    format!("{}/calendar/{token}.ics", state.public_api_url)
}

/// Returns the caller's feed URL, minting a token the first time.
pub async fn get_feed_link(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<FeedLink>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let token = calendar_feed::ensure_token(&state.db, user.id, false)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(FeedLink {
        url: feed_url(&state, &token),
    }))
}

/// Replaces the token, invalidating every existing subscription.
///
/// This is the entire revocation story: a link pasted somewhere it shouldn't
/// have been has to be killable without deleting the account.
pub async fn rotate_feed_link(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<FeedLink>, AppError> {
    let user = crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let token = calendar_feed::ensure_token(&state.db, user.id, true)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    tracing::info!("🔁 {} rotated their calendar feed link", user.username);

    Ok(Json(FeedLink {
        url: feed_url(&state, &token),
    }))
}

/// The feed itself.
///
/// ⚠️ **No `Claims` extractor, deliberately.** A calendar app is handed a URL
/// and GETs it unattended; it cannot send an `Authorization` header. The
/// token in the path *is* the authentication, which is why this route lives
/// outside `/api` and must never end up behind the JWT middleware - it would
/// 401 forever and the symptom would read as "Google won't subscribe".
pub async fn serve_feed(
    State(state): State<AppState>,
    Path(file): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = file.strip_suffix(".ics").unwrap_or(&file);

    // NotFound, not Unauthorized: a 401 would confirm which tokens exist.
    let user_id = calendar_feed::user_for_token(&state.db, token)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::NotFound)?;

    // The same visibility rule the app itself uses - not a second one. Two
    // rules for "what can this person see" have drifted twice in this
    // codebase already.
    let events = crate::services::calendar::list_user_events(&state.db, user_id, None, None, false)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let body = calendar_feed::render(&events, &state.frontend_url);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/calendar; charset=utf-8"),
            // Subscribers poll this; nothing should sit in front of it.
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    ))
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

    async fn seed_user(db: &PgPool, discord_id: &str) -> crate::models::User {
        create_or_update_user(
            db,
            crate::models::DiscordUser {
                id: discord_id.to_string(),
                username: discord_id.to_string(),
                discriminator: "0".to_string(),
                avatar: None,
                email: None,
            },
        )
        .await
        .unwrap()
    }

    async fn feed_link(db: PgPool, user: &crate::models::User, rotate: bool) -> (String, PgPool) {
        let state = AppState::for_test(db.clone(), "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .method(if rotate { "POST" } else { "GET" })
                    .uri("/api/calendar/feed")
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
        (json["url"].as_str().unwrap().to_string(), db)
    }

    /// The path a calendar app would actually GET.
    fn feed_path(url: &str) -> String {
        url.split_once("/calendar/")
            .map(|(_, rest)| format!("/calendar/{rest}"))
            .expect("the link should point at /calendar/")
    }

    // ⚠️ The whole point: a calendar app is given a URL and GETs it
    // unattended. If this ever needs an Authorization header it is broken,
    // and the symptom reads as "Google won't subscribe".
    #[sqlx::test]
    async fn the_feed_needs_no_authorization_header(db: PgPool) {
        let user = seed_user(&db, "me-discord").await;
        let (url, db) = feed_link(db, &user, false).await;

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri(feed_path(&url))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(content_type.starts_with("text/calendar"), "{content_type}");

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.starts_with("BEGIN:VCALENDAR\r\n"));
    }

    #[sqlx::test]
    async fn an_event_you_are_invited_to_is_in_the_feed(db: PgPool) {
        let me = seed_user(&db, "me-discord").await;
        let host = seed_user(&db, "host-discord").await;
        let event: uuid::Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO calendar_events (id, creator_id, title, start_time, end_time, created_at, updated_at)
            VALUES (gen_random_uuid(), $1, 'Raclette night', now() + interval '2 days',
                    now() + interval '2 days 3 hours', now(), now())
            RETURNING id
            "#,
        )
        .bind(host.id)
        .fetch_one(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO event_participants (id, event_id, user_id, status, invited_at)
             VALUES (gen_random_uuid(), $1, $2, 'accepted', now())",
        )
        .bind(event)
        .bind(me.id)
        .execute(&db)
        .await
        .unwrap();

        let (url, db) = feed_link(db, &me, false).await;
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri(feed_path(&url))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("SUMMARY:Raclette night"), "{text}");
    }

    // NotFound, not Unauthorized - a 401 would confirm which tokens exist.
    #[sqlx::test]
    async fn an_unknown_token_is_not_found(db: PgPool) {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri("/calendar/deadbeef.ics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // Rotation is the entire revocation story: a link pasted somewhere it
    // shouldn't have been must be killable without deleting the account.
    #[sqlx::test]
    async fn rotating_the_link_kills_the_old_one(db: PgPool) {
        let user = seed_user(&db, "me-discord").await;
        let (old_url, db) = feed_link(db, &user, false).await;
        let (new_url, db) = feed_link(db, &user, true).await;
        assert_ne!(old_url, new_url);

        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let app = crate::build_router(state);

        let dead = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(feed_path(&old_url))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(dead.status(), StatusCode::NOT_FOUND);

        let live = app
            .oneshot(
                Request::builder()
                    .uri(feed_path(&new_url))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(live.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn asking_for_the_link_twice_returns_the_same_one(db: PgPool) {
        let user = seed_user(&db, "me-discord").await;
        let (first, db) = feed_link(db, &user, false).await;
        let (second, _) = feed_link(db, &user, false).await;

        assert_eq!(first, second);
    }

    #[sqlx::test]
    async fn the_link_endpoint_itself_requires_auth(db: PgPool) {
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let response = crate::build_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/calendar/feed")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
