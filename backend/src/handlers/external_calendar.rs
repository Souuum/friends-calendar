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
        &state.http_client,
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
