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

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::ics_parse;

/// How far back busy blocks are cached.
///
/// ⚠️ The cache serves two readers. Availability only looks forward, but the
/// calendar draws these blocks too, and its month grid opens on a six-week
/// view that starts before today. Caching from `now` left everything earlier
/// in the visible month blank - including this morning's meetings once they
/// ended. Six weeks back covers the whole grid on any day of the month.
pub const SYNC_LOOKBACK_DAYS: i64 = 42;

/// How far ahead busy blocks are cached: about six months, so paging the
/// calendar forward a few months still shows them. Bounded on purpose - an
/// endless recurring rule would otherwise expand forever - and availability
/// never looks further than a week or two, so past and far-future rows cost
/// it nothing (its queries are windowed).
pub const SYNC_LOOKAHEAD_DAYS: i64 = 183;

/// Sent with every feed fetch. Some providers (Microsoft's among them)
/// refuse or throttle requests with no User-Agent at all, and reqwest sends
/// none by default.
const FEED_USER_AGENT: &str = concat!("friends-calendar/", env!("CARGO_PKG_VERSION"));

/// How often the loop runs. Hourly is plenty: a calendar changing between
/// polls costs at most an hour of staleness, against a request per user per
/// tick.
pub const SYNC_INTERVAL_MINUTES: u64 = 60;

/// The largest feed fetched. A packed work calendar with years of history is
/// well under a megabyte; the URL is whatever somebody pasted, and reading
/// an unbounded body would let one link exhaust the server's memory.
pub const MAX_FEED_BYTES: usize = 5 * 1024 * 1024;

/// The longest one feed's fetch may take, from connecting to the last byte.
///
/// ⚠️ Overall, not per read. A server that accepts and never answers, or
/// trickles a byte every few seconds under [`MAX_FEED_BYTES`], otherwise
/// holds its sync - and, while feeds ran one at a time, everybody's - for as
/// long as it likes. Real providers answer in a second or two.
pub const FEED_FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Feeds synced at once by [`sync_all`]. Enough that one slow feed (up to
/// [`FEED_FETCH_TIMEOUT`]) doesn't hold up the rest; few enough that an
/// hourly pass doesn't open a connection per user all at once.
const SYNC_CONCURRENCY: usize = 8;

/// Reads a feed's body, refusing it once it passes [`MAX_FEED_BYTES`].
///
/// ⚠️ Streamed and counted, not `text()` then measured: the point is to stop
/// reading, and a missing or lying `Content-Length` must not get around it.
async fn read_capped(mut response: reqwest::Response) -> Result<String> {
    let too_big = || {
        anyhow!(
            "This calendar is larger than {} MB, too big to import",
            MAX_FEED_BYTES / (1024 * 1024)
        )
    };
    if response
        .content_length()
        .is_some_and(|n| n > MAX_FEED_BYTES as u64)
    {
        return Err(too_big());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > MAX_FEED_BYTES {
            return Err(too_big());
        }
        body.extend_from_slice(&chunk);
    }
    // Lossy is safe here: the parser reads only ASCII property names and
    // values, never the text a non-UTF-8 charset would garble.
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// Fetches a feed's body within [`FEED_FETCH_TIMEOUT`].
///
/// The deadline is applied here, per request, rather than on the shared
/// client: whatever client a caller hands in, the feed is bounded. The
/// `tokio::time::timeout` covers connect, headers and the whole body read;
/// reqwest's per-request timeout is the same bound from inside, so the
/// connection is torn down rather than left open.
async fn fetch_feed(http: &Client, url: &str) -> Result<String> {
    let fetch = async {
        let response = http
            .get(url)
            .header(reqwest::header::USER_AGENT, FEED_USER_AGENT)
            .timeout(FEED_FETCH_TIMEOUT)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!("Calendar returned {}", response.status()));
        }
        read_capped(response).await
    };
    let too_slow = || {
        anyhow!(
            "This calendar took longer than {} seconds to download",
            FEED_FETCH_TIMEOUT.as_secs()
        )
    };
    match tokio::time::timeout(FEED_FETCH_TIMEOUT, fetch).await {
        Ok(Err(e))
            if e.downcast_ref::<reqwest::Error>()
                .is_some_and(reqwest::Error::is_timeout) =>
        {
            Err(too_slow())
        }
        Ok(result) => result,
        Err(_) => Err(too_slow()),
    }
}

/// What turns a feed into busy intervals. A parameter so tests can stand in
/// a parser that panics - the real one no longer does on any input known.
type Parser = fn(&str, DateTime<Utc>, DateTime<Utc>) -> Result<Vec<ics_parse::BusyInterval>>;

/// All-day blocks are excluded: a day-long "Annual leave" should not rule
/// out a 19:00 slot, and treating it as busy would make whole weeks
/// disappear from the suggestions.
fn parse_feed(
    body: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<ics_parse::BusyInterval>> {
    ics_parse::busy_intervals(body, from, to, false)
}

/// Runs CPU-bound work off the async runtime, turning a panic into an error.
///
/// ⚠️ Defence in depth. The feed is any URL a user pastes, the parser is
/// several hundred lines of date arithmetic over it, and `sync_all` runs
/// every user's feed in one task: before this, one feed that panicked the
/// parser stopped *everyone's* sync until restart. `spawn_blocking` also
/// keeps a slow parse from stalling the request-serving threads.
async fn run_isolated<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    match tokio::task::spawn_blocking(work).await {
        Ok(result) => result,
        Err(e) if e.is_panic() => {
            tracing::error!("Parsing an imported calendar panicked: {e:?}");
            bail!("This calendar couldn't be read")
        }
        Err(e) => bail!("Calendar sync was interrupted: {e}"),
    }
}

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

/// One block of time the user is committed, with **nothing** about what it
/// is.
///
/// ⚠️ There is deliberately no title field here, and none in `external_busy`
/// either - see the module docs and `ics_parse`, which never reads `SUMMARY`.
/// Rendering these on the calendar shows *that* you are busy, never *why*,
/// and that is the whole privacy position of the import feature. Adding a
/// label to this struct would mean reversing it in the parser first.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct ExternalBusy {
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

/// The caller's own imported busy blocks overlapping a window.
///
/// ⚠️ Scoped to one `user_id` by construction, and the handler passes the
/// authenticated user rather than anything from the request. Availability
/// exposes other people's *aggregate* free/busy; raw intervals are the
/// user's own business and must not become readable by friend id.
pub async fn busy_for_user(
    db: &PgPool,
    user_id: Uuid,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<ExternalBusy>> {
    let rows = sqlx::query_as::<_, ExternalBusy>(
        r#"
        SELECT b.starts_at, b.ends_at
        FROM external_busy b
        JOIN external_calendars c ON c.id = b.calendar_id
        WHERE c.user_id = $1
          AND b.starts_at < $3
          AND b.ends_at > $2
        ORDER BY b.starts_at
        "#,
    )
    .bind(user_id)
    .bind(from)
    .bind(to)
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
    sync_one_with(db, http, calendar_id, credential, now, parse_feed).await
}

async fn sync_one_with(
    db: &PgPool,
    http: &Client,
    calendar_id: Uuid,
    credential: &str,
    now: DateTime<Utc>,
    parse: Parser,
) -> Result<usize> {
    let from = now - Duration::days(SYNC_LOOKBACK_DAYS);
    let to = now + Duration::days(SYNC_LOOKAHEAD_DAYS);

    let body = fetch_feed(http, credential).await?;

    let intervals = run_isolated(move || parse(&body, from, to)).await?;

    let mut tx = db.begin().await?;
    // ⚠️ One writer per calendar. Two syncs at once (the connect request's
    // inline sync overlapping the hourly pass) each DELETE then INSERT; under
    // READ COMMITTED the second DELETE cannot see the first's uncommitted
    // rows, so both windows survived. The row lock serialises them, and the
    // second DELETE then sees the first's committed rows.
    sqlx::query("SELECT 1 FROM external_calendars WHERE id = $1 FOR UPDATE")
        .bind(calendar_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM external_busy WHERE calendar_id = $1")
        .bind(calendar_id)
        .execute(&mut *tx)
        .await?;

    // One statement for the whole window: a busy work calendar is a few
    // thousand rows over seven months, and a round-trip each adds up.
    let starts: Vec<DateTime<Utc>> = intervals.iter().map(|i| i.start).collect();
    let ends: Vec<DateTime<Utc>> = intervals.iter().map(|i| i.end).collect();
    sqlx::query(
        "INSERT INTO external_busy (id, calendar_id, starts_at, ends_at)
         SELECT gen_random_uuid(), $1, s, e FROM UNNEST($2::timestamptz[], $3::timestamptz[]) AS t(s, e)",
    )
    .bind(calendar_id)
    .bind(&starts)
    .bind(&ends)
    .execute(&mut *tx)
    .await?;

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
    sync_all_with(db, http, now, parse_feed).await
}

async fn sync_all_with(
    db: &PgPool,
    http: &Client,
    now: DateTime<Utc>,
    parse: Parser,
) -> Result<usize> {
    let calendars: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, credential FROM external_calendars")
            .fetch_all(db)
            .await?;

    // A few at a time, so a slow feed costs its own deadline rather than
    // everybody's. Each feed is its own task, so a panic anywhere in its
    // sync - not only the parse, which `run_isolated` already covers - is a
    // `JoinError` for that feed rather than the end of the pass.
    let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(SYNC_CONCURRENCY));
    let mut tasks = tokio::task::JoinSet::new();
    for (id, credential) in calendars {
        let (task_db, task_http, permits) = (db.clone(), http.clone(), permits.clone());
        let task = tokio::spawn(async move {
            let _permit = permits.acquire_owned().await;
            sync_one_with(&task_db, &task_http, id, &credential, now, parse).await
        });
        tasks.spawn(async move {
            let outcome = task
                .await
                .unwrap_or_else(|e| Err(anyhow!("Calendar sync was interrupted: {e}")));
            (id, outcome)
        });
    }

    let mut synced = 0;
    while let Some(joined) = tasks.join_next().await {
        // The outer task only awaits the inner one and cannot panic.
        let Ok((id, outcome)) = joined else { continue };
        match outcome {
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

    // --- busy_for_user: what the calendar draws --------------------------

    #[sqlx::test]
    async fn busy_for_user_returns_the_intervals_in_the_window(db: PgPool) {
        let user = a_user(&db, "alice").await;
        let server = serving(FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, Some("Work")).await.unwrap();
        sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();

        let busy = busy_for_user(
            &db,
            user,
            Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 3, 8, 0, 0, 0).unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(busy.len(), 1);
        assert_eq!(
            busy[0].starts_at,
            Utc.with_ymd_and_hms(2026, 3, 2, 9, 0, 0).unwrap()
        );
        assert_eq!(
            busy[0].ends_at,
            Utc.with_ymd_and_hms(2026, 3, 2, 10, 0, 0).unwrap()
        );
    }

    // ⚠️ The point of the whole import design: the feed above has a SUMMARY,
    // a DESCRIPTION and a LOCATION, and none of them can reach a caller
    // because none of them were ever stored. This is what lets the calendar
    // render these blocks without leaking what they are.
    #[sqlx::test]
    async fn busy_blocks_carry_no_trace_of_what_the_event_was(db: PgPool) {
        let user = a_user(&db, "alice").await;
        let server = serving(FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, Some("Work")).await.unwrap();
        sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();

        let busy = busy_for_user(
            &db,
            user,
            Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 3, 8, 0, 0, 0).unwrap(),
        )
        .await
        .unwrap();

        let rendered = serde_json::to_string(&busy).unwrap();
        for leak in ["Oncology", "private", "Hospital"] {
            assert!(
                !rendered.contains(leak),
                "{leak} reached the client: {rendered}"
            );
        }
    }

    // ⚠️ Raw intervals are the user's own. Availability deliberately exposes
    // only *aggregate* free/busy about other people; if this ever returned
    // somebody else's rows it would be a far wider disclosure.
    #[sqlx::test]
    async fn one_users_busy_blocks_are_invisible_to_another(db: PgPool) {
        let alice = a_user(&db, "alice").await;
        let bob = a_user(&db, "bob").await;
        let server = serving(FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, alice, &url, Some("Work")).await.unwrap();
        sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();

        let theirs = busy_for_user(
            &db,
            bob,
            Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 3, 8, 0, 0, 0).unwrap(),
        )
        .await
        .unwrap();

        assert!(theirs.is_empty(), "bob saw alice's calendar");
    }

    #[sqlx::test]
    async fn blocks_outside_the_window_are_left_out(db: PgPool) {
        let user = a_user(&db, "alice").await;
        let server = serving(FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, Some("Work")).await.unwrap();
        sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();

        let busy = busy_for_user(
            &db,
            user,
            Utc.with_ymd_and_hms(2026, 3, 5, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 3, 8, 0, 0, 0).unwrap(),
        )
        .await
        .unwrap();

        assert!(busy.is_empty());
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

    // Some providers refuse a request with no User-Agent, and reqwest sends
    // none unless told to.
    #[sqlx::test]
    async fn feed_fetches_identify_themselves(db: PgPool) {
        let user = a_user(&db, "me").await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cal.ics"))
            .and(wiremock::matchers::header_exists("user-agent"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FEED))
            .mount(&server)
            .await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, None).await.unwrap();

        let count = sync_one(&db, &Client::new(), id, &url, now()).await;

        assert_eq!(count.unwrap(), 1);
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

    // --- Hostile feeds ----------------------------------------------------

    async fn last_error(db: &PgPool, id: Uuid) -> Option<String> {
        sqlx::query_scalar("SELECT last_error FROM external_calendars WHERE id = $1")
            .bind(id)
            .fetch_one(db)
            .await
            .unwrap()
    }

    async fn cached(db: &PgPool, id: Uuid) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM external_busy WHERE calendar_id = $1")
            .bind(id)
            .fetch_one(db)
            .await
            .unwrap()
    }

    /// The real parser, except that it panics on a marker - standing in for
    /// whatever input the next parser bug turns out to need.
    fn panics_on_marker(
        body: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<ics_parse::BusyInterval>> {
        if body.contains("PANIC-ME") {
            panic!("parser bug");
        }
        parse_feed(body, from, to)
    }

    // ⚠️ `sync_all` runs every user's feed in one spawned task. Before each
    // feed was isolated, one panic in one parse ended that task, and every
    // other user's calendar stopped syncing until the process restarted.
    #[sqlx::test]
    async fn a_feed_that_panics_the_parser_does_not_stop_the_others(db: PgPool) {
        let alice = a_user(&db, "alice").await;
        let bob = a_user(&db, "bob").await;
        let hostile = serving("BEGIN:VCALENDAR\r\nX-PANIC-ME:1\r\nEND:VCALENDAR\r\n").await;
        let good = serving(FEED).await;
        // The hostile one first, so a panic that escaped would stop the loop
        // before it reached the good one.
        let bad_id = connect_ics(&db, alice, &format!("{}/cal.ics", hostile.uri()), None)
            .await
            .unwrap();
        let ok_id = connect_ics(&db, bob, &format!("{}/cal.ics", good.uri()), None)
            .await
            .unwrap();

        let synced = sync_all_with(&db, &Client::new(), now(), panics_on_marker)
            .await
            .unwrap();

        assert_eq!(synced, 1);
        assert_eq!(cached(&db, ok_id).await, 1);
        assert_eq!(
            last_error(&db, bad_id).await.as_deref(),
            Some("This calendar couldn't be read")
        );
    }

    // The connect handler's inline first sync goes through `sync_one`, so
    // the same isolation has to hold there: an error, not a panic.
    #[sqlx::test]
    async fn sync_one_turns_a_parser_panic_into_an_error(db: PgPool) {
        let user = a_user(&db, "me").await;
        let hostile = serving("BEGIN:VCALENDAR\r\nX-PANIC-ME:1\r\nEND:VCALENDAR\r\n").await;
        let url = format!("{}/cal.ics", hostile.uri());
        let id = connect_ics(&db, user, &url, None).await.unwrap();

        let err = sync_one_with(&db, &Client::new(), id, &url, now(), panics_on_marker)
            .await
            .unwrap_err();

        assert_eq!(err.to_string(), "This calendar couldn't be read");
    }

    #[sqlx::test]
    async fn an_oversized_feed_is_refused_and_keeps_the_cached_window(db: PgPool) {
        let user = a_user(&db, "me").await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cal.ics"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FEED))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cal.ics"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("X".repeat(MAX_FEED_BYTES + 1)),
            )
            .mount(&server)
            .await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, None).await.unwrap();
        sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();

        sync_all(&db, &Client::new(), now()).await.unwrap();

        assert_eq!(cached(&db, id).await, 1);
        let error = last_error(&db, id).await.unwrap();
        assert!(error.contains("too big to import"), "{error}");
    }

    // ⚠️ The cap must hold without a Content-Length too - a chunked response
    // has none, and a server could simply omit or understate it. This
    // serves one by hand, since wiremock always sets the header.
    #[sqlx::test]
    async fn a_chunked_feed_is_cut_off_at_the_cap(db: PgPool) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request).await;
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await;
            // Endless 64 KiB chunks: the client has to be the one to stop.
            let chunk = format!("{:x}\r\n{}\r\n", 65536, "X".repeat(65536));
            for _ in 0..(MAX_FEED_BYTES / 65536 + 64) {
                if socket.write_all(chunk.as_bytes()).await.is_err() {
                    return;
                }
            }
            let _ = socket.write_all(b"0\r\n\r\n").await;
        });
        let user = a_user(&db, "me").await;
        let url = format!("http://{addr}/cal.ics");
        let id = connect_ics(&db, user, &url, None).await.unwrap();

        let err = sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap_err();

        assert!(err.to_string().contains("too big to import"), "{err}");
    }

    // ⚠️ Verifier repro. The fetch has no timeout (`AppState.http_client` is
    // a bare `Client::new()`, and `sync_one` sets none per request), and
    // `sync_all` awaits feeds one after another. So a pasted URL whose
    // server accepts the connection and never answers - or trickles a byte
    // every few seconds, staying under the size cap - parks the loop on that
    // feed forever: nobody else's calendar syncs again until a restart, and
    // the connect request that triggers the first sync never returns. The
    // same "one feed stops everyone's sync" failure the panic isolation
    // fixed, by a different route.
    #[sqlx::test]
    async fn a_feed_that_never_answers_does_not_stop_the_others(db: PgPool) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                // Hold the connection open and say nothing.
                held.push(socket);
            }
        });
        let alice = a_user(&db, "alice").await;
        let bob = a_user(&db, "bob").await;
        let good = serving(FEED).await;
        // The silent one first, so a stall blocks the good one behind it.
        let silent_id = connect_ics(&db, alice, &format!("http://{addr}/cal.ics"), None)
            .await
            .unwrap();
        let ok_id = connect_ics(&db, bob, &format!("{}/cal.ics", good.uri()), None)
            .await
            .unwrap();

        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(90),
            sync_all(&db, &Client::new(), now()),
        )
        .await;

        assert!(
            outcome.is_ok(),
            "sync_all was still waiting on the silent feed after 90s"
        );
        assert_eq!(cached(&db, ok_id).await, 1);
        assert!(last_error(&db, silent_id).await.is_some());
        // Builder: and it says why, in words the settings card can show.
        let why = last_error(&db, silent_id).await.unwrap();
        assert!(why.contains("took longer than 30 seconds"), "{why}");
    }

    /// A server that answers 200 at once, then trickles one byte a second
    /// forever: always under the size cap, never finished. Every read
    /// succeeds, so only an overall deadline - not a per-read one - ends it.
    async fn trickling_server() -> std::net::SocketAddr {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut request = [0u8; 4096];
                    let _ = socket.read(&mut request).await;
                    let _ = socket
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/calendar\r\nTransfer-Encoding: chunked\r\n\r\n")
                        .await;
                    loop {
                        if socket.write_all(b"1\r\nX\r\n").await.is_err() {
                            return;
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                });
            }
        });
        addr
    }

    // Verifier (round 3): the silent-server repro only proves the deadline
    // covers *waiting for headers*. This one gets headers at once and a
    // body byte every second, so it proves the deadline covers the body read
    // too - and that the feeds queued behind it still sync.
    #[sqlx::test]
    async fn a_feed_that_trickles_bytes_is_cut_at_the_deadline(db: PgPool) {
        let addr = trickling_server().await;
        let alice = a_user(&db, "alice").await;
        let bob = a_user(&db, "bob").await;
        let good = serving(FEED).await;
        let slow_id = connect_ics(&db, alice, &format!("http://{addr}/cal.ics"), None)
            .await
            .unwrap();
        let ok_id = connect_ics(&db, bob, &format!("{}/cal.ics", good.uri()), None)
            .await
            .unwrap();
        let started = std::time::Instant::now();

        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            sync_all(&db, &Client::new(), now()),
        )
        .await;

        assert!(
            outcome.is_ok(),
            "sync_all still reading the trickle after 60s"
        );
        assert!(started.elapsed() >= std::time::Duration::from_secs(29));
        assert_eq!(cached(&db, ok_id).await, 1);
        let why = last_error(&db, slow_id).await.unwrap();
        assert!(why.contains("took longer than 30 seconds"), "{why}");
    }

    // Verifier (round 3): concurrency must not cost isolation. Ten feeds -
    // more than `SYNC_CONCURRENCY`, so some wait for a permit - one of which
    // panics the parser and one of which trickles until the deadline. The
    // other eight all sync, each failure is recorded against its own row,
    // and the slow one's permit doesn't starve the queue (the whole pass
    // takes one deadline, not two).
    #[sqlx::test]
    async fn ten_feeds_one_panicking_one_slow_the_rest_all_sync(db: PgPool) {
        let slow = trickling_server().await;
        let hostile = serving("BEGIN:VCALENDAR\r\nX-PANIC-ME:1\r\nEND:VCALENDAR\r\n").await;
        let good = serving(FEED).await;
        let mut ok_ids = Vec::new();
        let slow_id = connect_ics(
            &db,
            a_user(&db, "slow").await,
            &format!("http://{slow}/cal.ics"),
            None,
        )
        .await
        .unwrap();
        let bad_id = connect_ics(
            &db,
            a_user(&db, "bad").await,
            &format!("{}/cal.ics", hostile.uri()),
            None,
        )
        .await
        .unwrap();
        for i in 0..8 {
            let user = a_user(&db, &format!("user{i}")).await;
            // Distinct URLs, so connect_ics's upsert doesn't fold them.
            let id = connect_ics(&db, user, &format!("{}/cal.ics?u={i}", good.uri()), None)
                .await
                .unwrap();
            ok_ids.push(id);
        }
        let started = std::time::Instant::now();

        let synced = tokio::time::timeout(
            std::time::Duration::from_secs(55),
            sync_all_with(&db, &Client::new(), now(), panics_on_marker),
        )
        .await
        .expect("sync pass outlived one deadline plus slack")
        .unwrap();

        eprintln!("10-feed pass took {:?}", started.elapsed());
        assert_eq!(synced, 8);
        for id in ok_ids {
            assert_eq!(cached(&db, id).await, 1);
            assert_eq!(last_error(&db, id).await, None);
        }
        assert_eq!(
            last_error(&db, bad_id).await.as_deref(),
            Some("This calendar couldn't be read")
        );
        assert!(
            last_error(&db, slow_id)
                .await
                .unwrap()
                .contains("took longer than 30 seconds")
        );
    }

    // Verifier (round 3): two syncs of the *same* calendar at once - the
    // connect request's inline sync overlapping the hourly pass, say. Each
    // replaces the window in its own transaction (DELETE then INSERT); under
    // READ COMMITTED the second DELETE cannot see the first's uncommitted
    // INSERT, so both sets can survive. Asserts the window is never doubled.
    //
    // FAILED on 6b9b591 ("left 4 copies of one block"): pre-existing, not
    // introduced by the concurrent `sync_all` (which never syncs one id
    // twice). Fixed by locking the external_calendars row at the start of
    // the write transaction (`SELECT ... FOR UPDATE`).
    #[sqlx::test]
    async fn concurrent_syncs_of_one_calendar_do_not_double_its_window(db: PgPool) {
        let user = a_user(&db, "me").await;
        let server = serving(FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, None).await.unwrap();

        let http = Client::new();
        let mut worst = 0;
        for _ in 0..30 {
            sqlx::query("DELETE FROM external_busy WHERE calendar_id = $1")
                .bind(id)
                .execute(&db)
                .await
                .unwrap();
            let (a, b, c, d) = tokio::join!(
                sync_one(&db, &http, id, &url, now()),
                sync_one(&db, &http, id, &url, now()),
                sync_one(&db, &http, id, &url, now()),
                sync_one(&db, &http, id, &url, now()),
            );
            for r in [a, b, c, d] {
                r.unwrap();
            }
            worst = worst.max(cached(&db, id).await);
        }

        assert_eq!(
            worst, 1,
            "a concurrent re-sync left {worst} copies of one block"
        );
    }

    // ⚠️ A cap, not a truncation: storing the first N blocks would make
    // somebody look free at times they aren't.
    #[sqlx::test]
    async fn a_feed_with_too_many_blocks_is_refused_with_a_readable_error(db: PgPool) {
        let user = a_user(&db, "me").await;
        let events: String = (0..=ics_parse::MAX_INTERVALS_PER_FEED)
            .map(|_| {
                "BEGIN:VEVENT\r\nDTSTART:20260302T090000Z\r\nDTEND:20260302T100000Z\r\nEND:VEVENT\r\n"
            })
            .collect();
        let body: &'static str =
            Box::leak(format!("BEGIN:VCALENDAR\r\n{events}END:VCALENDAR\r\n").into_boxed_str());
        assert!(
            body.len() < MAX_FEED_BYTES,
            "must trip the interval cap, not the size cap"
        );
        let server = serving(body).await;
        let id = connect_ics(&db, user, &format!("{}/cal.ics", server.uri()), None)
            .await
            .unwrap();

        sync_all(&db, &Client::new(), now()).await.unwrap();

        assert_eq!(cached(&db, id).await, 0, "nothing partial is stored");
        let error = last_error(&db, id).await.unwrap();
        assert!(error.contains("too many to import"), "{error}");
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

    // --- Repros: the cached window vs. what the calendar shows -----------
    //
    // `sync_one` caches `[now, now + 30 days)` - the window availability
    // needs. But `GET /api/calendar/external/busy` serves the *calendar*,
    // whose default month view opens on a six-week grid that starts before
    // today, and whose "next month" arrow reaches well past 30 days. Anything
    // outside the cached window is simply not in `external_busy`, so it can
    // never be drawn: earlier this month (including this morning's meetings
    // once they've ended) is always blank, and so is most of next month.

    const MID_MONTH_FEED: &str = "BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\nDTSTART:20260302T090000Z\r\nDTEND:20260302T100000Z\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART:20260315T080000Z\r\nDTEND:20260315T083000Z\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART:20260420T090000Z\r\nDTEND:20260420T100000Z\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n";

    /// Synced at 10:00 on the 15th, as the hourly loop would be.
    fn mid_month() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, 15, 10, 0, 0).unwrap()
    }

    #[sqlx::test]
    async fn earlier_this_month_is_still_drawn_after_a_sync(db: PgPool) {
        let user = a_user(&db, "alice").await;
        let server = serving(MID_MONTH_FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, Some("Work")).await.unwrap();
        sync_one(&db, &Client::new(), id, &url, mid_month())
            .await
            .unwrap();

        // March's month grid: Mon 23 Feb to Sun 5 Apr.
        let busy = busy_for_user(
            &db,
            user,
            Utc.with_ymd_and_hms(2026, 2, 23, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 4, 6, 0, 0, 0).unwrap(),
        )
        .await
        .unwrap();

        let starts: Vec<_> = busy.iter().map(|b| b.starts_at).collect();
        assert!(
            starts.contains(&Utc.with_ymd_and_hms(2026, 3, 2, 9, 0, 0).unwrap()),
            "the 2 March meeting is missing from March's grid: {starts:?}"
        );
        assert!(
            starts.contains(&Utc.with_ymd_and_hms(2026, 3, 15, 8, 0, 0).unwrap()),
            "this morning's meeting vanished once it ended: {starts:?}"
        );
    }

    #[sqlx::test]
    async fn next_month_is_drawn_after_a_sync(db: PgPool) {
        let user = a_user(&db, "alice").await;
        let server = serving(MID_MONTH_FEED).await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, Some("Work")).await.unwrap();
        sync_one(&db, &Client::new(), id, &url, mid_month())
            .await
            .unwrap();

        // April's month grid, one click of the "next" arrow away: Mon 30 Mar
        // to Sun 10 May. 20 April is 36 days after the sync.
        let busy = busy_for_user(
            &db,
            user,
            Utc.with_ymd_and_hms(2026, 3, 30, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 5, 11, 0, 0, 0).unwrap(),
        )
        .await
        .unwrap();

        let starts: Vec<_> = busy.iter().map(|b| b.starts_at).collect();
        assert!(
            starts.contains(&Utc.with_ymd_and_hms(2026, 4, 20, 9, 0, 0).unwrap()),
            "the 20 April meeting is missing from April's grid: {starts:?}"
        );
    }

    // --- Verifier: a feed that is not UTF-8 -------------------------------
    //
    // `read_capped` decodes lossily rather than honouring the charset. The
    // only parsed text that can carry non-ASCII is an Outlook display-name
    // TZID, so a Windows-1252 `Bogotá` becomes `Bogot\u{fffd}`. That must
    // still land every meeting at the right instant: via the next city IANA
    // knows, via the feed's own VTIMEZONE (decoded the same way, so the key
    // still matches), or via the `(UTC-04:00)` prefix.
    #[sqlx::test]
    async fn a_windows_1252_feed_still_places_meetings_correctly(db: PgPool) {
        let user = a_user(&db, "me").await;
        let text = "BEGIN:VCALENDAR\r\n\
BEGIN:VTIMEZONE\r\nTZID:(UTC-03:00) Brasília\r\nBEGIN:STANDARD\r\nDTSTART:16010101T000000\r\nTZOFFSETFROM:-0300\r\nTZOFFSETTO:-0300\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n\
BEGIN:VEVENT\r\nSUMMARY:Réunion équipe\r\nDTSTART;TZID=\"(UTC-05:00) Bogotá, Lima, Quito\":20260302T090000\r\nDTEND;TZID=\"(UTC-05:00) Bogotá, Lima, Quito\":20260302T100000\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;TZID=\"(UTC-03:00) Brasília\":20260303T090000\r\nDTEND;TZID=\"(UTC-03:00) Brasília\":20260303T100000\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;TZID=\"(UTC-04:00) Cuiabá\":20260304T090000\r\nDTEND;TZID=\"(UTC-04:00) Cuiabá\":20260304T100000\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n";
        // Windows-1252: every character here is Latin-1, one byte each.
        let bytes: Vec<u8> = text.chars().map(|c| c as u32 as u8).collect();
        assert!(
            String::from_utf8(bytes.clone()).is_err(),
            "really not UTF-8"
        );
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cal.ics"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(bytes, "text/calendar; charset=windows-1252"),
            )
            .mount(&server)
            .await;
        let url = format!("{}/cal.ics", server.uri());
        let id = connect_ics(&db, user, &url, None).await.unwrap();

        sync_one(&db, &Client::new(), id, &url, now())
            .await
            .unwrap();

        let starts: Vec<_> = busy_for_user(
            &db,
            user,
            Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 3, 8, 0, 0, 0).unwrap(),
        )
        .await
        .unwrap()
        .into_iter()
        .map(|b| b.starts_at)
        .collect();
        assert_eq!(
            starts,
            vec![
                Utc.with_ymd_and_hms(2026, 3, 2, 14, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2026, 3, 3, 12, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2026, 3, 4, 13, 0, 0).unwrap(),
            ]
        );
    }
}
