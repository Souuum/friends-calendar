use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    config::AppState,
    error::AppError,
    middleware::auth::Claims,
    services::external_calendar::{self, ExternalBusy, ExternalCalendar},
};

#[derive(serde::Deserialize)]
pub struct ConnectRequest {
    pub url: String,
    pub label: Option<String>,
}

async fn current_user(state: &AppState, claims: &Claims) -> Result<crate::models::User, AppError> {
    crate::services::auth::get_user_by_discord_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or(AppError::Unauthorized)
}

pub async fn list_calendars(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<Vec<ExternalCalendar>>, AppError> {
    let user = current_user(&state, &claims).await?;

    let calendars = external_calendar::list_for_user(&state.db, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(calendars))
}

#[derive(serde::Deserialize)]
pub struct BusyWindow {
    pub from: chrono::DateTime<chrono::Utc>,
    pub to: chrono::DateTime<chrono::Utc>,
}

/// The caller's imported busy blocks, so the calendar can draw them.
///
/// ⚠️ Always the authenticated user - the window is the only thing taken
/// from the request. A `user_id` parameter here would turn raw calendar
/// intervals into something readable by friend id, which is a different and
/// much wider disclosure than the aggregate free/busy availability exposes.
pub async fn list_busy(
    claims: Claims,
    State(state): State<AppState>,
    axum::extract::Query(window): axum::extract::Query<BusyWindow>,
) -> Result<Json<Vec<ExternalBusy>>, AppError> {
    let user = current_user(&state, &claims).await?;

    let busy = external_calendar::busy_for_user(&state.db, user.id, window.from, window.to)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(busy))
}

/// Connects a secret `.ics` URL and syncs it immediately.
///
/// Syncing inline rather than waiting for the hourly loop: somebody who has
/// just pasted a link wants to see it working, and a connection that shows
/// "never synced" for an hour reads as broken.
pub async fn connect_calendar(
    claims: Claims,
    State(state): State<AppState>,
    Json(req): Json<ConnectRequest>,
) -> Result<Json<Vec<ExternalCalendar>>, AppError> {
    let user = current_user(&state, &claims).await?;

    let id = external_calendar::connect_ics(
        &state.db,
        user.id,
        &req.url,
        req.label.as_deref().filter(|l| !l.trim().is_empty()),
    )
    .await
    // The URL check lives in the service and its message is written for a
    // reader ("Only http(s) calendar links are supported"), so pass it on.
    .map_err(|e| AppError::ValidationError(e.to_string()))?;

    // A first sync that fails is recorded on the row, not returned as an
    // error: the connection exists, and the UI shows why it isn't working.
    if let Err(e) = external_calendar::sync_one(
        &state.db,
        &state.feed_http_client,
        id,
        &external_calendar::normalise_feed_url(&req.url).unwrap_or_default(),
        chrono::Utc::now(),
    )
    .await
    {
        tracing::warn!("First sync of calendar {id} failed: {e:?}");
        sqlx::query("UPDATE external_calendars SET last_error = $1 WHERE id = $2")
            .bind(e.to_string())
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    }

    let calendars = external_calendar::list_for_user(&state.db, user.id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(calendars))
}

/// Disconnects, taking the cached intervals with it.
pub async fn disconnect_calendar(
    claims: Claims,
    State(state): State<AppState>,
    Path(calendar_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user = current_user(&state, &claims).await?;

    let removed = external_calendar::disconnect(&state.db, user.id, calendar_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    // Scoped to the caller's own rows, so somebody else's id is simply not
    // found rather than a forbidden that confirms it exists.
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;

    use crate::config::AppState;
    use crate::handlers::auth::generate_jwt;
    use crate::services::auth::create_or_update_user;

    // Verifier (round 3): the connect request runs the first sync inline, so
    // the fetch deadline has to hold on this path too - through the real
    // router and the real `AppState.feed_http_client`, not a test-built client.
    // A server that accepts and never answers must cost the request ~30 s,
    // not hang it, and the reason must land on the row the page renders.
    #[sqlx::test]
    async fn connecting_a_feed_that_never_answers_returns_within_the_deadline(db: PgPool) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });
        let user = create_or_update_user(
            &db,
            crate::models::DiscordUser {
                id: "me-discord".into(),
                username: "me".into(),
                discriminator: "0".into(),
                avatar: None,
                email: None,
            },
        )
        .await
        .unwrap();
        let state = AppState::for_test(db, "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        let app = crate::build_router(state);
        let started = std::time::Instant::now();

        let response = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/calendar/external")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        json!({ "url": format!("http://calendar.test:{}/cal.ics", addr.port()), "label": "Work" })
                            .to_string(),
                    ))
                    .unwrap(),
            ),
        )
        .await
        .expect("POST /api/calendar/external still hanging after 60s")
        .unwrap();

        assert!(started.elapsed() >= std::time::Duration::from_secs(29));
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let error = json[0]["last_error"].as_str().unwrap_or_default();
        assert!(error.contains("took longer than 30 seconds"), "{json}");
        assert!(json[0]["last_synced_at"].is_null(), "{json}");
    }

    // --- private addresses (verifier repro) -------------------------------
    //
    // ⚠️ The connect form is a fetch-anything-for-me button: the server GETs
    // the pasted URL inline and reports the outcome in `last_error`. These
    // pin that an internal address is refused up front - 400, no row, no
    // request - and that a redirect cannot reach one either.

    async fn signed_in(db: &PgPool) -> (axum::Router, String) {
        let user = create_or_update_user(
            db,
            crate::models::DiscordUser {
                id: "me-discord".into(),
                username: "me".into(),
                discriminator: "0".into(),
                avatar: None,
                email: None,
            },
        )
        .await
        .unwrap();
        let state = AppState::for_test(db.clone(), "http://unused.invalid".to_string());
        let token = generate_jwt(&user.discord_id, &state.jwt_secret).unwrap();
        (crate::build_router(state), token)
    }

    async fn connect(app: axum::Router, token: &str, url: &str) -> (StatusCode, Value) {
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/calendar/external")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(json!({ "url": url }).to_string()))
                    .unwrap(),
            ),
        )
        .await
        .unwrap_or_else(|_| panic!("connecting {url} was still fetching after 10s"))
        .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }

    #[sqlx::test]
    async fn connecting_a_private_address_is_refused_and_nothing_is_fetched_or_stored(db: PgPool) {
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
        // Stands in for an internal service: anything reaching it is the bug.
        let internal = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART:20260302T090000Z\r\nDTEND:20260302T100000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            ))
            .expect(0)
            .mount(&internal)
            .await;
        let port = internal.address().port();
        let (app, token) = signed_in(&db).await;

        for url in [
            format!("{}/cal.ics", internal.uri()),
            format!("http://localhost:{port}/cal.ics"),
            format!("http://2130706433:{port}/cal.ics"),
            format!("http://[::ffff:127.0.0.1]:{port}/cal.ics"),
            "http://169.254.169.254/latest/meta-data/".to_string(),
            "http://10.0.0.5/cal.ics".to_string(),
            "http://192.168.1.1/cal.ics".to_string(),
            "http://[::1]:5432/".to_string(),
        ] {
            let (status, body) = connect(app.clone(), &token, &url).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{url}: {body}");
            let message = body["error"].as_str().unwrap_or_default();
            assert!(
                message.to_lowercase().contains("private"),
                "{url}: the 400 should say why: {body}"
            );
        }

        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM external_calendars")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(rows, 0, "a refused URL must not leave a connection behind");
        // `internal` drops here and verifies `.expect(0)`.
    }

    // ⚠️ Relies on `AppState::for_test`'s feed client, which resolves any
    // `*.test` name to 127.0.0.1 (a fake resolver, under the test-only
    // loopback allowance), and on the connect handler fetching with that
    // feed client, not the general `http_client`. The redirect *target* is a literal private address,
    // which no policy - test or production - may follow.
    #[sqlx::test]
    async fn connecting_a_feed_that_redirects_to_a_private_address_does_not_follow_it(db: PgPool) {
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
        let internal = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART:20260302T090000Z\r\nDTEND:20260302T100000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            ))
            .expect(0)
            .mount(&internal)
            .await;
        let (app, token) = signed_in(&db).await;

        for (i, target) in [
            format!("{}/cal.ics", internal.uri()),
            "http://169.254.169.254/latest/meta-data/".to_string(),
        ]
        .into_iter()
        .enumerate()
        {
            let public = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(302).insert_header("Location", target.as_str()))
                .mount(&public)
                .await;
            let url = format!(
                "http://calendar.test:{}/cal.ics?n={i}",
                public.address().port()
            );

            let (status, body) = connect(app.clone(), &token, &url).await;

            // The connection itself is fine (the pasted host is public); its
            // first sync must fail, and say why, on every row so far.
            assert_eq!(status, StatusCode::OK, "{url}: {body}");
            let rows = body.as_array().cloned().unwrap_or_default();
            assert_eq!(rows.len(), i + 1, "{body}");
            for row in rows {
                let error = row["last_error"].as_str().unwrap_or_default();
                assert!(
                    error.to_lowercase().contains("private"),
                    "redirect to {target} should be refused as private: {body}"
                );
                assert!(row["last_synced_at"].is_null(), "{body}");
            }
        }
        // `internal` drops here and verifies `.expect(0)`.
    }
}
