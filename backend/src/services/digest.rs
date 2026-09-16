use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Timelike, Utc, Weekday};
use reqwest::Client;
use sqlx::PgPool;

use crate::services::{discord_config, discord_feed};

/// Checked roughly hourly from a background loop spawned in main.rs (see
/// `spawn_digest_loop` below and its call site). Posts a one-line weekly
/// summary to the guild's configured announcements channel iff:
/// `digest_enabled` is on (a real toggle on the /server page - see
/// 009_add_announcement_feed.sql's comment on why this is a *guild-level*
/// setting, separate from the per-user `notify_weekly_digest` preference
/// that has no send mechanism behind it), a channel is configured, and
/// it's actually due. Returns whether a digest was sent, mainly so tests
/// can assert on it without needing a second query.
pub async fn maybe_send_weekly_digest(
    base_url: &str,
    db: &PgPool,
    http: &Client,
    bot_token: &str,
    guild_id: &str,
    now: DateTime<Utc>,
) -> Result<bool> {
    let Some(config) = discord_config::get_config(db, guild_id).await? else {
        return Ok(false);
    };
    if !config.digest_enabled {
        return Ok(false);
    }
    let Some(channel_id) = config.announcements_channel_id else {
        return Ok(false);
    };

    if !is_due(now, config.last_digest_sent_at) {
        return Ok(false);
    }

    let count = discord_feed::count_posts_since(db, &channel_id, now - Duration::days(7)).await?;
    let message = format!(
        "📬 **Weekly digest** — {count} new post{} this week.",
        if count == 1 { "" } else { "s" }
    );

    discord_feed::send_channel_message(base_url, http, bot_token, &channel_id, &message).await?;
    discord_config::mark_digest_sent(db, guild_id, now).await?;

    Ok(true)
}

/// Due when it's Monday, 9am UTC or later ("One summary posted every
/// Monday at 9:00" per the mockup), and either nothing's ever been sent or
/// it's been at least 6 days since the last one - the 6-day floor (not 7)
/// guards against re-firing on every hourly poll throughout the same
/// Monday morning without needing a more precise "did we already send
/// today" check.
fn is_due(now: DateTime<Utc>, last_sent: Option<DateTime<Utc>>) -> bool {
    if now.weekday() != Weekday::Mon || now.hour() < 9 {
        return false;
    }

    match last_sent {
        None => true,
        Some(last) => now - last >= Duration::days(6),
    }
}

/// Spawned once at startup (main.rs) when a guild + bot token are
/// configured, same conditional-spawn shape as `bot::DiscordBot::start`.
/// Polls hourly rather than trying to wake exactly at Monday 9am - cheap,
/// and `is_due`'s 6-day floor makes the exact poll cadence not matter for
/// correctness, only for how late in the 9am hour the digest might land.
pub async fn spawn_digest_loop(
    base_url: String,
    db: PgPool,
    http: Client,
    bot_token: String,
    guild_id: String,
) {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3600));
    loop {
        ticker.tick().await;
        match maybe_send_weekly_digest(&base_url, &db, &http, &bot_token, &guild_id, Utc::now())
            .await
        {
            Ok(true) => tracing::info!("📬 Sent weekly announcements digest for guild {guild_id}"),
            Ok(false) => {}
            Err(e) => tracing::warn!("⚠️  Failed to send weekly digest: {:?}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn dt(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().into()
    }

    // --- unit: is_due -------------------------------------------------

    #[test]
    fn not_due_outside_monday_morning() {
        let tuesday_9am = dt("2026-03-03T09:00:00Z");
        assert!(!is_due(tuesday_9am, None));

        let monday_8am = dt("2026-03-02T08:59:00Z");
        assert!(!is_due(monday_8am, None));
    }

    #[test]
    fn due_on_first_monday_morning_with_nothing_sent_yet() {
        let monday_9am = dt("2026-03-02T09:00:00Z");
        assert!(is_due(monday_9am, None));
    }

    #[test]
    fn not_due_again_within_the_same_week() {
        let monday_9am = dt("2026-03-02T09:00:00Z");
        let sent_earlier_that_morning = dt("2026-03-02T09:05:00Z");
        assert!(!is_due(monday_9am, Some(sent_earlier_that_morning)));

        let following_wednesday = dt("2026-03-04T10:00:00Z");
        assert!(!is_due(
            following_wednesday,
            Some(sent_earlier_that_morning)
        ));
    }

    #[test]
    fn due_again_the_next_monday() {
        let last_sent = dt("2026-03-02T09:05:00Z");
        let next_monday = dt("2026-03-09T09:10:00Z");
        assert!(is_due(next_monday, Some(last_sent)));
    }

    // --- integration: maybe_send_weekly_digest -------------------------

    #[sqlx::test]
    async fn sends_and_stamps_when_due_and_enabled(db: PgPool) {
        discord_config::upsert_config(&db, "g1", Some("chan1".to_string()), Some(true))
            .await
            .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/channels/chan1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "sent-1" })))
            .mount(&server)
            .await;

        let http = Client::new();
        let monday_9am = dt("2026-03-02T09:00:00Z");
        let sent =
            maybe_send_weekly_digest(&server.uri(), &db, &http, "test-token", "g1", monday_9am)
                .await
                .unwrap();
        assert!(sent);

        let config = discord_config::get_config(&db, "g1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            config.last_digest_sent_at.unwrap().timestamp(),
            monday_9am.timestamp()
        );
    }

    #[sqlx::test]
    async fn does_not_send_outside_the_due_window(db: PgPool) {
        discord_config::upsert_config(&db, "g1", Some("chan1".to_string()), Some(true))
            .await
            .unwrap();

        // No mock mounted for POST /channels/.../messages - if the code
        // tried to send, this test would fail with a connection error
        // rather than silently passing.
        let http = Client::new();
        let tuesday = dt("2026-03-03T09:00:00Z");
        let sent = maybe_send_weekly_digest(
            "http://unused.invalid",
            &db,
            &http,
            "test-token",
            "g1",
            tuesday,
        )
        .await
        .unwrap();
        assert!(!sent);
    }

    #[sqlx::test]
    async fn does_not_send_when_digest_is_disabled(db: PgPool) {
        discord_config::upsert_config(&db, "g1", Some("chan1".to_string()), Some(false))
            .await
            .unwrap();

        let http = Client::new();
        let monday_9am = dt("2026-03-02T09:00:00Z");
        let sent = maybe_send_weekly_digest(
            "http://unused.invalid",
            &db,
            &http,
            "test-token",
            "g1",
            monday_9am,
        )
        .await
        .unwrap();
        assert!(!sent);
    }

    #[sqlx::test]
    async fn does_not_send_without_a_configured_channel(db: PgPool) {
        discord_config::upsert_config(&db, "g1", None, Some(true))
            .await
            .unwrap();

        let http = Client::new();
        let monday_9am = dt("2026-03-02T09:00:00Z");
        let sent = maybe_send_weekly_digest(
            "http://unused.invalid",
            &db,
            &http,
            "test-token",
            "g1",
            monday_9am,
        )
        .await
        .unwrap();
        assert!(!sent);
    }

    #[sqlx::test]
    async fn does_not_send_before_any_config_row_exists(db: PgPool) {
        let http = Client::new();
        let monday_9am = dt("2026-03-02T09:00:00Z");
        let sent = maybe_send_weekly_digest(
            "http://unused.invalid",
            &db,
            &http,
            "test-token",
            "g1",
            monday_9am,
        )
        .await
        .unwrap();
        assert!(!sent);
    }
}
