//! Backfills RSVPs from reactions that are already on announcement
//! messages.
//!
//! `bot.rs` only ever sees reactions added *while it is connected*. Anything
//! reacted to before the bot existed, before an event was announced through
//! this app, or during any downtime, is invisible to it - so a member could
//! tick ✅ on Discord and find nothing in their calendar. This walks the
//! reactions Discord already has and records them.
//!
//! It deliberately reuses `bot::record_attendance`, so a backfilled RSVP and
//! a live one go through exactly the same path: same publication lookup,
//! same idempotent upsert, same treatment of a previous "declined".

use anyhow::Result;
use reqwest::Client;
use sqlx::PgPool;

use crate::bot::{ReactionOutcome, record_attendance};
use crate::services::discord_feed::{get_json, url_encode};

/// Must stay in step with `bot::is_attendance_emoji`. Only the plain check
/// mark is queried: Discord's reaction endpoint takes one emoji per call,
/// and this is the one the bot itself adds.
const RSVP_EMOJI: &str = "✅";

/// Discord returns at most 100 reactors per page.
const PAGE: usize = 100;

#[derive(Debug, Default, PartialEq, serde::Serialize)]
pub struct SyncReport {
    pub messages_checked: usize,
    pub reactions_seen: usize,
    pub rsvps_recorded: usize,
}

#[derive(serde::Deserialize)]
struct Reactor {
    id: String,
    username: String,
    /// Present and true for applications. The bot adds the ✅ itself, so
    /// without this it would become a participant in every event it
    /// announced.
    #[serde(default)]
    bot: bool,
}

/// Every publication that has actually been posted to Discord.
async fn published_messages(db: &PgPool) -> Result<Vec<(String, String)>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT channel_id, discord_message_id
        FROM event_publications
        WHERE discord_message_id IS NOT NULL
        "#,
    )
    .fetch_all(db)
    .await?;
    Ok(rows)
}

/// Records every ✅ currently on one message.
pub async fn sync_message(
    db: &PgPool,
    base_url: &str,
    http: &Client,
    bot_token: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<(usize, usize)> {
    let emoji = url_encode(RSVP_EMOJI);
    let mut after: Option<String> = None;
    let mut seen = 0usize;
    let mut recorded = 0usize;

    loop {
        let mut url = format!(
            "{base_url}/channels/{channel_id}/messages/{message_id}/reactions/{emoji}?limit={PAGE}"
        );
        if let Some(cursor) = &after {
            url.push_str(&format!("&after={cursor}"));
        }

        let page: Vec<Reactor> = get_json(http, bot_token, &url).await?;
        if page.is_empty() {
            break;
        }

        let last = page[page.len() - 1].id.clone();
        for reactor in &page {
            seen += 1;
            if reactor.bot {
                continue;
            }
            match record_attendance(db, message_id, &reactor.id, &reactor.username).await? {
                ReactionOutcome::Recorded { .. } => recorded += 1,
                // The message belongs to no event we know about, so nothing
                // further on this message will match either.
                ReactionOutcome::UnknownEvent => return Ok((seen, recorded)),
                ReactionOutcome::UnknownUser => {}
            }
        }

        if page.len() < PAGE {
            break;
        }
        after = Some(last);
    }

    Ok((seen, recorded))
}

/// Walks every announced message. A failure on one message is logged and
/// skipped rather than abandoning the rest: a single deleted message should
/// not stop the whole backfill.
pub async fn sync_all(
    db: &PgPool,
    base_url: &str,
    http: &Client,
    bot_token: &str,
) -> Result<SyncReport> {
    let mut report = SyncReport::default();

    for (channel_id, message_id) in published_messages(db).await? {
        report.messages_checked += 1;
        match sync_message(db, base_url, http, bot_token, &channel_id, &message_id).await {
            Ok((seen, recorded)) => {
                report.reactions_seen += seen;
                report.rsvps_recorded += recorded;
            }
            Err(e) => {
                tracing::warn!("⚠️  Could not sync reactions for message {message_id}: {e}");
            }
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::models::DiscordUser;
    use crate::services::auth::create_or_update_user;

    const MSG: &str = "msg-1";
    const CHANNEL: &str = "chan-1";

    async fn seed_event_with_publication(db: &PgPool) -> uuid::Uuid {
        let creator = create_or_update_user(
            db,
            DiscordUser {
                id: "creator-discord".into(),
                username: "creator".into(),
                discriminator: "0".into(),
                avatar: None,
                email: None,
            },
        )
        .await
        .unwrap();

        let event_id: uuid::Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO calendar_events (id, creator_id, title, start_time, end_time, created_at, updated_at)
            VALUES (gen_random_uuid(), $1, 'Concert', now() + interval '10 days',
                    now() + interval '10 days 3 hours', now(), now())
            RETURNING id
            "#,
        )
        .bind(creator.id)
        .fetch_one(db)
        .await
        .unwrap();

        let guild_id = crate::services::guilds::ensure_guild(db, "guild-1")
            .await
            .unwrap();
        sqlx::query(
            r#"
            INSERT INTO event_publications (id, event_id, guild_id, channel_id, discord_message_id, posted_at)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, now())
            "#,
        )
        .bind(event_id)
        .bind(guild_id)
        .bind(CHANNEL)
        .bind(MSG)
        .execute(db)
        .await
        .unwrap();

        event_id
    }

    fn reactors(body: serde_json::Value) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(body)
    }

    async fn mock_reactions(body: serde_json::Value) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/channels/{CHANNEL}/messages/{MSG}/reactions/%E2%9C%85"
            )))
            .respond_with(reactors(body))
            .mount(&server)
            .await;
        server
    }

    async fn status_of(db: &PgPool, event_id: uuid::Uuid, discord_id: &str) -> Option<String> {
        sqlx::query_scalar(
            r#"
            SELECT ep.status::text FROM event_participants ep
            JOIN users u ON u.id = ep.user_id
            WHERE ep.event_id = $1 AND u.discord_id = $2
            "#,
        )
        .bind(event_id)
        .bind(discord_id)
        .fetch_optional(db)
        .await
        .unwrap()
    }

    // The reported case: a reaction that was already on the message before
    // anything here watched for it.
    #[sqlx::test]
    async fn records_an_existing_reaction_as_going(db: PgPool) {
        let event_id = seed_event_with_publication(&db).await;
        let server = mock_reactions(json!([{ "id": "u-1", "username": "soum" }])).await;

        let report = sync_all(&db, &server.uri(), &reqwest::Client::new(), "token")
            .await
            .unwrap();

        assert_eq!(report.rsvps_recorded, 1);
        assert_eq!(
            status_of(&db, event_id, "u-1").await.as_deref(),
            Some("accepted")
        );
    }

    // The bot adds the ✅ itself, so without the `bot` check it would become
    // a participant in every event it ever announced.
    #[sqlx::test]
    async fn skips_the_bot_own_reaction(db: PgPool) {
        let event_id = seed_event_with_publication(&db).await;
        let server = mock_reactions(json!([
            { "id": "bot-1", "username": "friends-calendar", "bot": true },
            { "id": "u-1", "username": "soum" }
        ]))
        .await;

        let report = sync_all(&db, &server.uri(), &reqwest::Client::new(), "token")
            .await
            .unwrap();

        assert_eq!(report.rsvps_recorded, 1);
        assert!(status_of(&db, event_id, "bot-1").await.is_none());
        assert_eq!(
            status_of(&db, event_id, "u-1").await.as_deref(),
            Some("accepted")
        );
    }

    // Runs on every boot, so it must not churn the database.
    #[sqlx::test]
    async fn is_idempotent(db: PgPool) {
        let event_id = seed_event_with_publication(&db).await;
        let server = mock_reactions(json!([{ "id": "u-1", "username": "soum" }])).await;
        let http = reqwest::Client::new();

        sync_all(&db, &server.uri(), &http, "token").await.unwrap();
        sync_all(&db, &server.uri(), &http, "token").await.unwrap();

        let rows: i64 =
            sqlx::query_scalar("SELECT count(*) FROM event_participants WHERE event_id = $1")
                .bind(event_id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(rows, 1);
    }

    #[sqlx::test]
    async fn reports_nothing_when_no_one_reacted(db: PgPool) {
        seed_event_with_publication(&db).await;
        let server = mock_reactions(json!([])).await;

        let report = sync_all(&db, &server.uri(), &reqwest::Client::new(), "token")
            .await
            .unwrap();

        assert_eq!(report.messages_checked, 1);
        assert_eq!(report.rsvps_recorded, 0);
    }

    // A deleted message 404s. That must not abandon every later message.
    #[sqlx::test]
    async fn a_failing_message_does_not_stop_the_rest(db: PgPool) {
        seed_event_with_publication(&db).await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let report = sync_all(&db, &server.uri(), &reqwest::Client::new(), "token")
            .await
            .unwrap();

        assert_eq!(report.messages_checked, 1);
        assert_eq!(report.rsvps_recorded, 0);
    }
}
