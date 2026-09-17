//! Connecting a real calendar, so availability knows about the rest of
//! someone's life.
//!
//! Before this, "free" meant "no Friends Calendar event" - somebody with a
//! full week of work meetings showed as free all week, and `best-slot`
//! confidently suggested a time they were in a standup.
//!
//! ## Where it plugs in
//!
//! ⚠️ **One place.** `services::availability::fetch_busy_intervals` is the
//! only function that builds availability data, and four pure consumers take
//! plain `(user_id, start, end)` tuples. An external calendar is *more rows
//! in that list*; nothing in the ranking or free/busy logic changes.
//!
//! ## What is stored
//!
//! ⚠️ **Intervals, never event content.** See `services::ics_parse` - the
//! titles are not read, not merely discarded. Importing a work calendar into
//! a social app is a privacy problem the moment you keep "Interview with…".

use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::ics_parse;

/// How far ahead busy blocks are cached. Availability never looks further,
/// and an unbounded window would expand recurring rules forever.
pub const SYNC_WINDOW_DAYS: i64 = 30;

/// How often the loop runs. Hourly is plenty: a calendar changing between
/// polls costs at most an hour of staleness, against a request per user per
/// tick.
pub const SYNC_INTERVAL_MINUTES: u64 = 60;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ExternalCalendar {
    pub id: Uuid,
    pub provider: String,
    pub label: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    /// Surfaced in the UI on purpose. A silently dead connection is worse
    /// than no connection, because availability looks right and isn't.
    pub last_error: Option<String>,
}

/// ⚠️ Rejects anything that isn't plain http(s).
///
/// The URL is fetched by the server, so `file://`, `gopher://` and friends
/// would make this a file-read / SSRF primitive. Webcal is Apple's scheme
/// for "subscribe to this https URL" and is rewritten rather than refused,
/// because it is exactly what people copy out of a calendar app.
pub fn normalise_feed_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    let rewritten = trimmed
        .strip_prefix("webcal://")
        .map(|rest| format!("https://{rest}"))
        .unwrap_or_else(|| trimmed.to_string());

    let parsed = url::Url::parse(&rewritten).map_err(|_| anyhow!("That isn't a valid URL"))?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed.to_string()),
        other => Err(anyhow!(
            "Only http(s) calendar links are supported, not {other}"
        )),
    }
}

pub async fn list_for_user(db: &PgPool, user_id: Uuid) -> Result<Vec<ExternalCalendar>> {
    let rows = sqlx::query_as::<_, ExternalCalendar>(
        "SELECT id, provider, label, last_synced_at, last_error
         FROM external_calendars WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(rows)
}

pub async fn connect_ics(
    db: &PgPool,
    user_id: Uuid,
    raw_url: &str,
    label: Option<&str>,
) -> Result<Uuid> {
    let url = normalise_feed_url(raw_url)?;

    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO external_calendars (id, user_id, provider, credential, label)
        VALUES (gen_random_uuid(), $1, 'ics', $2, $3)
        ON CONFLICT (user_id, credential) DO UPDATE SET label = EXCLUDED.label
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(&url)
    .bind(label)
    .fetch_one(db)
    .await?;

    Ok(id)
}

/// Removes the connection **and** its cached intervals.
///
/// The cascade on `external_busy` does the second half; disconnecting has to
/// take the data with it, or availability keeps reporting from a calendar
/// the user thinks they unlinked.
pub async fn disconnect(db: &PgPool, user_id: Uuid, calendar_id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM external_calendars WHERE id = $1 AND user_id = $2")
        .bind(calendar_id)
        .bind(user_id)
        .execute(db)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Fetches one calendar and replaces its cached window wholesale.
///
/// Wholesale rather than incrementally: sync tokens are an optimisation and
/// a source of drift, and the window is small.
pub async fn sync_one(
    db: &PgPool,
    http: &Client,
    calendar_id: Uuid,
    credential: &str,
    now: DateTime<Utc>,
) -> Result<usize> {
    let to = now + Duration::days(SYNC_WINDOW_DAYS);

    let response = http.get(credential).send().await?;
    if !response.status().is_success() {
        return Err(anyhow!("Calendar returned {}", response.status()));
    }
    let body = response.text().await?;

    // All-day blocks are excluded: a day-long "Annual leave" should not rule
    // out a 19:00 slot, and treating it as busy would make whole weeks
    // disappear from the suggestions.
    let intervals = ics_parse::busy_intervals(&body, now, to, false)?;

    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM external_busy WHERE calendar_id = $1")
        .bind(calendar_id)
        .execute(&mut *tx)
        .await?;

    for interval in &intervals {
        sqlx::query(
            "INSERT INTO external_busy (id, calendar_id, starts_at, ends_at)
             VALUES (gen_random_uuid(), $1, $2, $3)",
        )
        .bind(calendar_id)
        .bind(interval.start)
        .bind(interval.end)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "UPDATE external_calendars SET last_synced_at = $1, last_error = NULL WHERE id = $2",
    )
    .bind(now)
    .bind(calendar_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(intervals.len())
}

/// Records why a calendar didn't sync, without touching its cached window.
///
/// ⚠️ Stale availability beats suddenly-everyone-is-free. Wiping the cache on
/// a transient failure would make a whole team look available for an hour.
async fn record_failure(db: &PgPool, calendar_id: Uuid, message: &str) -> Result<()> {
    sqlx::query("UPDATE external_calendars SET last_error = $1 WHERE id = $2")
        .bind(message)
        .bind(calendar_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Syncs every connected calendar. One failure is logged and skipped.
pub async fn sync_all(db: &PgPool, http: &Client, now: DateTime<Utc>) -> Result<usize> {
    let calendars: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, credential FROM external_calendars")
            .fetch_all(db)
            .await?;

    let mut synced = 0;
    for (id, credential) in calendars {
        match sync_one(db, http, id, &credential, now).await {
            Ok(_) => synced += 1,
            Err(e) => {
                // A revoked link must not stop everyone else's sync - the
                // same rule reaction_sync already follows.
                tracing::warn!("External calendar {id} failed to sync: {e:?}");
                let _ = record_failure(db, id, &e.to_string()).await;
            }
        }
    }

    Ok(synced)
}

/// Hourly sync, spawned from `main.rs`.
pub fn spawn_sync_loop(db: PgPool, http: Client) {
    tokio::spawn(async move {
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs(SYNC_INTERVAL_MINUTES * 60));
        loop {
            ticker.tick().await;
            if let Err(e) = sync_all(&db, &http, Utc::now()).await {
                tracing::error!("External calendar sync pass failed: {e:?}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::models::DiscordUser;
    use crate::services::auth::create_or_update_user;

    const FEED: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Oncology appointment\r\nDESCRIPTION:private\r\nLOCATION:Hospital\r\nDTSTART:20260302T090000Z\r\nDTEND:20260302T100000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap()
    }

    async fn a_user(db: &PgPool, name: &str) -> Uuid {
        create_or_update_user(
            db,
            DiscordUser {
                id: format!("{name}-discord"),
                username: name.into(),
                discriminator: "0".into(),
                avatar: None,
                email: None,
            },
        )
        .await
        .unwrap()
        .id
    }

    async fn serving(body: &'static str) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cal.ics"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        server
    }

    // --- the URL check is a security boundary ----------------------------

    #[test]
    fn webcal_is_rewritten_rather_than_refused() {
        // It is exactly what people copy out of Apple Calendar.
        assert_eq!(
            normalise_feed_url("webcal://example.com/a.ics").unwrap(),
            "https://example.com/a.ics"
        );
    }

    // ⚠️ The server fetches this URL. Without the scheme check it becomes a
    // file-read / SSRF primitive.
    #[test]
    fn non_http_schemes_are_refused() {
        for bad in [
            "file:///etc/passwd",
            "gopher://example.com/",
            "data:text/plain,hi",
            "not a url",
        ] {
            assert!(normalise_feed_url(bad).is_err(), "should refuse {bad}");
        }
    }

    // --- syncing ----------------------------------------------------------

    #[sqlx::test]
    async fn syncing_caches_the_intervals(db: PgPool) {
        let user = a_user(&db, "me").await;
        let server = serving(FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, Some("Work")).await.unwrap();

        let count = sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();

        assert_eq!(count, 1);
        let rows: Vec<(DateTime<Utc>, DateTime<Utc>)> =
            sqlx::query_as("SELECT starts_at, ends_at FROM external_busy WHERE calendar_id = $1")
                .bind(id)
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].0,
            Utc.with_ymd_and_hms(2026, 3, 2, 9, 0, 0).unwrap()
        );
    }

    /// ⚠️ The promise the whole feature is sold on. Asserted against the
    /// database rather than trusted to the mapping code.
    #[sqlx::test]
    async fn no_event_content_reaches_the_database(db: PgPool) {
        let user = a_user(&db, "me").await;
        let server = serving(FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, None).await.unwrap();
        sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();

        // Every text column in both tables, concatenated.
        let dump: String = sqlx::query_scalar(
            "SELECT coalesce(string_agg(x, ' '), '') FROM (
                 SELECT coalesce(provider,'') || ' ' || coalesce(label,'') || ' ' ||
                        coalesce(credential,'') || ' ' || coalesce(last_error,'') AS x
                 FROM external_calendars
             ) t",
        )
        .fetch_one(&db)
        .await
        .unwrap();

        for secret in ["Oncology", "private", "Hospital"] {
            assert!(
                !dump.contains(secret),
                "{secret:?} leaked into the database"
            );
        }
    }

    #[sqlx::test]
    async fn re_syncing_replaces_the_window_rather_than_duplicating(db: PgPool) {
        let user = a_user(&db, "me").await;
        let server = serving(FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, None).await.unwrap();

        sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();
        sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();

        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM external_busy WHERE calendar_id = $1")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(count, 1);
    }

    // ⚠️ Stale availability beats suddenly-everyone-is-free. Wiping the cache
    // on a transient failure would make a whole group look available.
    #[sqlx::test]
    async fn a_failing_sync_keeps_the_cached_window_and_records_why(db: PgPool) {
        let user = a_user(&db, "me").await;
        let server = serving(FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, None).await.unwrap();
        sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();

        // The feed goes away.
        drop(server);
        let dead = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cal.ics"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&dead)
            .await;
        sqlx::query("UPDATE external_calendars SET credential = $1 WHERE id = $2")
            .bind(format!("{}/cal.ics", dead.uri()))
            .bind(id)
            .execute(&db)
            .await
            .unwrap();

        sync_all(&db, &Client::new(), now()).await.unwrap();

        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM external_busy WHERE calendar_id = $1")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(count, 1, "the cached window must survive a failed sync");

        let error: Option<String> =
            sqlx::query_scalar("SELECT last_error FROM external_calendars WHERE id = $1")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(error.unwrap().contains("404"));
    }

    // One revoked link must not stop everybody else's sync.
    #[sqlx::test]
    async fn one_broken_calendar_does_not_stop_the_others(db: PgPool) {
        let user = a_user(&db, "me").await;
        let good = serving(FEED).await;
        let bad = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cal.ics"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&bad)
            .await;

        connect_ics(&db, user, &format!("{}/cal.ics", bad.uri()), None)
            .await
            .unwrap();
        let ok_id = connect_ics(&db, user, &format!("{}/cal.ics", good.uri()), None)
            .await
            .unwrap();

        let synced = sync_all(&db, &Client::new(), now()).await.unwrap();

        assert_eq!(synced, 1);
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM external_busy WHERE calendar_id = $1")
                .bind(ok_id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(count, 1);
    }

    #[sqlx::test]
    async fn disconnecting_removes_the_cached_intervals_too(db: PgPool) {
        let user = a_user(&db, "me").await;
        let server = serving(FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, None).await.unwrap();
        sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();

        assert!(disconnect(&db, user, id).await.unwrap());

        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM external_busy")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(
            count, 0,
            "availability must stop reading a disconnected calendar"
        );
    }

    #[sqlx::test]
    async fn you_cannot_disconnect_somebody_elses_calendar(db: PgPool) {
        let me = a_user(&db, "me").await;
        let them = a_user(&db, "them").await;
        let server = serving(FEED).await;
        let id = connect_ics(&db, them, &format!("{}/cal.ics", server.uri()), None)
            .await
            .unwrap();

        assert!(!disconnect(&db, me, id).await.unwrap());
    }

    #[sqlx::test]
    async fn connecting_the_same_url_twice_updates_rather_than_duplicates(db: PgPool) {
        let user = a_user(&db, "me").await;
        let url = "https://example.com/a.ics";

        let first = connect_ics(&db, user, url, Some("Work")).await.unwrap();
        let second = connect_ics(&db, user, url, Some("Personal")).await.unwrap();

        assert_eq!(first, second);
        let calendars = list_for_user(&db, user).await.unwrap();
        assert_eq!(calendars.len(), 1);
        assert_eq!(calendars[0].label.as_deref(), Some("Personal"));
    }
}
