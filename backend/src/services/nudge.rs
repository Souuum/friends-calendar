//! Chasing the people who never answered.
//!
//! The peek panel has shipped a **disabled** "Nudge no-answers" button since
//! `event-edit-flow`, with a comment saying it needed its own feature rather
//! than being wired up as a side effect. This is that feature.
//!
//! ## What makes it different from every other outbound message here
//!
//! Everything else this app sends is either a consequence of creating
//! something (`create_event`'s announce) or a scheduled job (`digest`,
//! `reminders`). A nudge is the first thing **a person can fire at other
//! people on demand**, which makes it the first that can be used to annoy
//! them. Hence the rate limit in the database (migration 015) rather than a
//! disabled button, and hence the creator-only check on the endpoint rather
//! than only in the UI.
//!
//! ## Where it lands
//!
//! In-app, and into the event's existing Discord thread. **Not** a Discord
//! DM per person: that is the only option that unambiguously *reaches*
//! someone who doesn't open the app, and it is also the most annoying thing
//! this app could learn to do. It is a separate decision, deliberately not
//! taken here.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::{discord_feed, notifications};

/// How long a creator has to wait before nudging the same event again.
///
/// A day is long enough that a nudge stays an event rather than a stream,
/// and short enough to chase something happening this week.
pub const NUDGE_COOLDOWN_HOURS: i64 = 24;

/// Pure, so the rule can be tested without a database - the same shape
/// `digest::is_due` and `reminders::is_due` use, and for the same reason.
pub fn can_nudge(now: DateTime<Utc>, nudged_at: Option<DateTime<Utc>>) -> bool {
    match nudged_at {
        None => true,
        Some(last) => now >= last + Duration::hours(NUDGE_COOLDOWN_HOURS),
    }
}

/// When the next nudge becomes allowed, for telling the caller rather than
/// failing opaquely.
pub fn next_nudge_at(nudged_at: DateTime<Utc>) -> DateTime<Utc> {
    nudged_at + Duration::hours(NUDGE_COOLDOWN_HOURS)
}

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
pub struct NudgeReport {
    /// How many people were actually notified.
    pub nudged: usize,
    /// True when the Discord half failed. The in-app half still went out.
    pub discord_failed: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum NudgeError {
    /// No such event, or not this user's to nudge. One variant, so the
    /// response can't be used to discover which events exist.
    NotYours,
    /// Already nudged inside the cooldown.
    TooSoon(DateTime<Utc>),
}

struct EventRow {
    title: String,
    start_time: DateTime<Utc>,
    nudged_at: Option<DateTime<Utc>>,
}

/// Everyone invited who has not answered.
///
/// Note there is deliberately **no** `pending_count` endpoint beside this:
/// the panel already has the participant list on the wire and counts it
/// there, so a server-side count would be a second source of truth for a
/// number the client can already see - and clippy flagged it as dead the
/// moment it was written.
///
/// ⚠️ `pending` only. `maybe` **is** an answer, and nudging it turns a
/// considerate feature into pestering.
async fn pending_participants(db: &PgPool, event_id: Uuid) -> Result<Vec<Uuid>> {
    let ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM event_participants WHERE event_id = $1 AND status = 'pending'",
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;

    Ok(ids)
}

#[allow(clippy::too_many_arguments)]
pub async fn nudge(
    db: &PgPool,
    base_url: &str,
    http: &Client,
    bot_token: Option<&str>,
    event_id: Uuid,
    creator_id: Uuid,
    now: DateTime<Utc>,
) -> Result<std::result::Result<NudgeReport, NudgeError>> {
    // Creator-only, enforced in the same query that fetches the event so
    // there is no window between checking and acting. The UI hides the
    // button for non-creators; that is a convenience, not the guard.
    let event: Option<EventRow> = sqlx::query_as::<_, (String, DateTime<Utc>, Option<DateTime<Utc>>)>(
        "SELECT title, start_time, nudged_at FROM calendar_events WHERE id = $1 AND creator_id = $2",
    )
    .bind(event_id)
    .bind(creator_id)
    .fetch_optional(db)
    .await?
    .map(|(title, start_time, nudged_at)| EventRow {
        title,
        start_time,
        nudged_at,
    });

    let Some(event) = event else {
        return Ok(Err(NudgeError::NotYours));
    };

    if !can_nudge(now, event.nudged_at) {
        // Unwrap is safe: can_nudge only returns false when there is a
        // previous nudge to wait on.
        return Ok(Err(NudgeError::TooSoon(next_nudge_at(
            event.nudged_at.expect("cooldown implies a previous nudge"),
        ))));
    }

    let pending = pending_participants(db, event_id).await?;

    // `event_invite`, not a new preference: a nudge *is* a second ask about
    // an invitation, so someone who switched event invites off has already
    // said they don't want this. (Contrast friend_request, which CLAUDE.md
    // notes must NOT reuse that column - there the two are unrelated.)
    let message = format!("Still waiting on your answer for \"{}\"", event.title);
    let mut nudged = 0;
    for user_id in &pending {
        match notifications::create(
            db,
            *user_id,
            "event_invite",
            Some(creator_id),
            Some(event_id),
            &message,
        )
        .await
        {
            // `create` is a no-op when the preference is off, so this counts
            // attempts rather than deliveries. Close enough for the creator,
            // and the alternative is reporting other people's settings.
            Ok(()) => nudged += 1,
            Err(e) => tracing::warn!("Failed to notify {user_id} about nudge: {e:?}"),
        }
    }

    // Stamped even if Discord fails below: the in-app half went out, and
    // letting a failed thread post buy another nudge would defeat the limit.
    sqlx::query("UPDATE calendar_events SET nudged_at = $1 WHERE id = $2")
        .bind(now)
        .bind(event_id)
        .execute(db)
        .await?;

    let mut discord_failed = false;
    if let Some(bot_token) = bot_token {
        // A thread started from a message shares that message's id - the
        // same fact reminders relies on.
        for thread_id in crate::services::guilds::published_message_ids(db, event_id).await? {
            if let Err(e) = discord_feed::send_channel_message(
                base_url,
                http,
                bot_token,
                &thread_id,
                &format_nudge_message(&event.title, event.start_time, pending.len()),
            )
            .await
            {
                // Tolerated: thread creation is best-effort when an event is
                // announced, so a 404 here means there was never a thread.
                tracing::warn!("Failed to post nudge in thread for event {event_id}: {e:?}");
                discord_failed = true;
            }
        }
    }

    Ok(Ok(NudgeReport {
        nudged,
        discord_failed,
    }))
}

/// French, matching `discord_announcement` and `reminders`, and using
/// Discord's `<t:…:R>` so each reader sees their own timezone.
fn format_nudge_message(title: &str, start_time: DateTime<Utc>, pending: usize) -> String {
    format!(
        "⏳ **{}** c'est <t:{}:R> — il manque encore {} réponse{}.",
        title,
        start_time.timestamp(),
        pending,
        if pending > 1 { "s" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::models::DiscordUser;
    use crate::services::auth::create_or_update_user;

    fn at(hour: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap() + Duration::hours(hour)
    }

    // --- the rule, with no database in sight ------------------------------

    #[test]
    fn a_never_nudged_event_can_be_nudged() {
        assert!(can_nudge(at(0), None));
    }

    #[test]
    fn nudging_again_inside_the_cooldown_is_refused() {
        assert!(!can_nudge(at(1), Some(at(0))));
        assert!(!can_nudge(at(23), Some(at(0))));
    }

    #[test]
    fn the_cooldown_boundary_is_inclusive() {
        // Exactly 24h later is allowed - a strict `>` would make "once a
        // day" mean "once every 24h and a bit", which drifts.
        assert!(can_nudge(at(24), Some(at(0))));
    }

    #[test]
    fn next_nudge_at_is_the_cooldown_after_the_last_one() {
        assert_eq!(next_nudge_at(at(0)), at(24));
    }

    // --- the DB behaviour -------------------------------------------------

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

    async fn an_event(db: &PgPool, creator: Uuid) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO calendar_events (id, creator_id, title, start_time, end_time, created_at, updated_at)
            VALUES (gen_random_uuid(), $1, 'Raclette night', now() + interval '3 days',
                    now() + interval '3 days 3 hours', now(), now())
            RETURNING id
            "#,
        )
        .bind(creator)
        .fetch_one(db)
        .await
        .unwrap()
    }

    async fn invite(db: &PgPool, event: Uuid, user: Uuid, status: &str) {
        sqlx::query(&format!(
            "INSERT INTO event_participants (id, event_id, user_id, status, invited_at) VALUES (gen_random_uuid(), $1, $2, '{status}', now())"
        ))
        .bind(event)
        .bind(user)
        .execute(db)
        .await
        .unwrap();
    }

    async fn notification_count(db: &PgPool, user: Uuid) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM notifications WHERE user_id = $1")
            .bind(user)
            .fetch_one(db)
            .await
            .unwrap()
    }

    async fn run(
        db: &PgPool,
        event: Uuid,
        creator: Uuid,
        now: DateTime<Utc>,
    ) -> std::result::Result<NudgeReport, NudgeError> {
        nudge(
            db,
            "http://unused.invalid",
            &Client::new(),
            None,
            event,
            creator,
            now,
        )
        .await
        .unwrap()
    }

    // `maybe` IS an answer. Nudging it turns a considerate feature into
    // pestering, so only `pending` is chased.
    #[sqlx::test]
    async fn only_people_who_never_answered_are_nudged(db: PgPool) {
        let creator = a_user(&db, "creator").await;
        let event = an_event(&db, creator).await;

        let no_answer = a_user(&db, "noanswer").await;
        let maybe = a_user(&db, "maybe").await;
        let going = a_user(&db, "going").await;
        let declined = a_user(&db, "declined").await;
        invite(&db, event, no_answer, "pending").await;
        invite(&db, event, maybe, "maybe").await;
        invite(&db, event, going, "accepted").await;
        invite(&db, event, declined, "declined").await;

        let report = run(&db, event, creator, Utc::now()).await.unwrap();

        assert_eq!(report.nudged, 1);
        assert_eq!(notification_count(&db, no_answer).await, 1);
        for answered in [maybe, going, declined] {
            assert_eq!(notification_count(&db, answered).await, 0);
        }
    }

    // The gate lives inside services::notifications::create, so this comes
    // for free - but only as long as the nudge keeps going through it.
    #[sqlx::test]
    async fn someone_who_switched_event_invites_off_is_not_nudged(db: PgPool) {
        let creator = a_user(&db, "creator").await;
        let event = an_event(&db, creator).await;
        let quiet = a_user(&db, "quiet").await;
        invite(&db, event, quiet, "pending").await;
        sqlx::query("UPDATE users SET notify_event_invites = false WHERE id = $1")
            .bind(quiet)
            .execute(&db)
            .await
            .unwrap();

        run(&db, event, creator, Utc::now()).await.unwrap();

        assert_eq!(notification_count(&db, quiet).await, 0);
    }

    #[sqlx::test]
    async fn only_the_creator_can_nudge(db: PgPool) {
        let creator = a_user(&db, "creator").await;
        let stranger = a_user(&db, "stranger").await;
        let event = an_event(&db, creator).await;
        let pending = a_user(&db, "pending").await;
        invite(&db, event, pending, "pending").await;

        let outcome = run(&db, event, stranger, Utc::now()).await;

        assert_eq!(outcome, Err(NudgeError::NotYours));
        // And nothing happened.
        assert_eq!(notification_count(&db, pending).await, 0);
    }

    #[sqlx::test]
    async fn a_second_nudge_inside_the_cooldown_is_refused_and_changes_nothing(db: PgPool) {
        let creator = a_user(&db, "creator").await;
        let event = an_event(&db, creator).await;
        let pending = a_user(&db, "pending").await;
        invite(&db, event, pending, "pending").await;

        let first = Utc::now();
        run(&db, event, creator, first).await.unwrap();
        let stamped: Option<DateTime<Utc>> =
            sqlx::query_scalar("SELECT nudged_at FROM calendar_events WHERE id = $1")
                .bind(event)
                .fetch_one(&db)
                .await
                .unwrap();

        let outcome = run(&db, event, creator, first + Duration::hours(1)).await;

        assert!(matches!(outcome, Err(NudgeError::TooSoon(_))));
        // No second notification, and the clock was not reset - a refused
        // nudge must not extend the cooldown.
        assert_eq!(notification_count(&db, pending).await, 1);
        let after: Option<DateTime<Utc>> =
            sqlx::query_scalar("SELECT nudged_at FROM calendar_events WHERE id = $1")
                .bind(event)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(after, stamped);
    }

    #[sqlx::test]
    async fn nudging_again_after_the_cooldown_works(db: PgPool) {
        let creator = a_user(&db, "creator").await;
        let event = an_event(&db, creator).await;
        let pending = a_user(&db, "pending").await;
        invite(&db, event, pending, "pending").await;

        let first = Utc::now();
        run(&db, event, creator, first).await.unwrap();
        let second = run(&db, event, creator, first + Duration::hours(25)).await;

        assert_eq!(second.unwrap().nudged, 1);
        assert_eq!(notification_count(&db, pending).await, 2);
    }

    // A thread that was never created 404s - announcing tolerates that with
    // a warning, so the nudge must too. The in-app half already went out.
    #[sqlx::test]
    async fn a_failed_thread_post_does_not_fail_the_nudge(db: PgPool) {
        let creator = a_user(&db, "creator").await;
        let event = an_event(&db, creator).await;
        let pending = a_user(&db, "pending").await;
        invite(&db, event, pending, "pending").await;

        let guild = crate::services::guilds::ensure_guild(&db, "g1")
            .await
            .unwrap();
        let publication = crate::services::guilds::add_publication(&db, event, guild, "chan-1")
            .await
            .unwrap();
        crate::services::guilds::mark_published(&db, publication, "msg-1")
            .await
            .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/channels/msg-1/messages"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let outcome = nudge(
            &db,
            &server.uri(),
            &Client::new(),
            Some("token"),
            event,
            creator,
            Utc::now(),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(outcome.nudged, 1);
        assert!(outcome.discord_failed);
        assert_eq!(notification_count(&db, pending).await, 1);
    }

    #[sqlx::test]
    async fn the_nudge_is_posted_into_the_events_own_thread(db: PgPool) {
        let creator = a_user(&db, "creator").await;
        let event = an_event(&db, creator).await;
        let pending = a_user(&db, "pending").await;
        invite(&db, event, pending, "pending").await;

        let guild = crate::services::guilds::ensure_guild(&db, "g1")
            .await
            .unwrap();
        let publication = crate::services::guilds::add_publication(&db, event, guild, "chan-1")
            .await
            .unwrap();
        crate::services::guilds::mark_published(&db, publication, "msg-1")
            .await
            .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            // A thread started from a message shares that message's id.
            .and(path("/channels/msg-1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "x"})))
            .expect(1)
            .mount(&server)
            .await;

        let outcome = nudge(
            &db,
            &server.uri(),
            &Client::new(),
            Some("token"),
            event,
            creator,
            Utc::now(),
        )
        .await
        .unwrap()
        .unwrap();

        assert!(!outcome.discord_failed);
        drop(server);
    }

}
