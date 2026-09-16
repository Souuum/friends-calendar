use crate::models::CalendarEvent;
use crate::services::discord_feed;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

/// How far ahead of an event its reminder goes out.
///
/// Fixed rather than configurable: per-event or per-user lead times need a
/// column and a UI, and nobody has asked for them. If that changes, this is
/// the single place it's decided.
const LEAD_TIME_MINUTES: i64 = 60;

/// How often the loop wakes. Deliberately much shorter than the lead time -
/// polling hourly for a one-hour lead would let a reminder land up to an
/// hour early or late, which for "starts in an hour" is the whole message.
const POLL_INTERVAL_SECS: u64 = 300;

/// Whether an event needs its reminder sent now. Pure, so the scheduling
/// rules are testable without a database or a Discord mock - same split as
/// `services::digest::is_due`.
///
/// Three conditions, and the third is the one that's easy to forget: if the
/// process was down over the reminder window, every event that started
/// during the outage would otherwise look "due" on restart and fire a
/// reminder for something already underway.
pub fn is_due(
    now: DateTime<Utc>,
    start_time: DateTime<Utc>,
    reminder_sent_at: Option<DateTime<Utc>>,
) -> bool {
    if reminder_sent_at.is_some() {
        return false;
    }
    if start_time <= now {
        return false;
    }
    start_time <= now + Duration::minutes(LEAD_TIME_MINUTES)
}

/// Events starting inside the reminder window that haven't been reminded.
///
/// The `start_time > now` half of `is_due` is applied here too rather than
/// only in Rust, so a long outage doesn't drag every past event into memory
/// just to discard it.
async fn due_events(db: &PgPool, now: DateTime<Utc>) -> Result<Vec<CalendarEvent>> {
    let events = sqlx::query_as::<_, CalendarEvent>(
        r#"
        SELECT * FROM calendar_events
        WHERE reminder_sent_at IS NULL
          AND start_time > $1
          AND start_time <= $2
        ORDER BY start_time ASC
        "#,
    )
    .bind(now)
    .bind(now + Duration::minutes(LEAD_TIME_MINUTES))
    .fetch_all(db)
    .await?;

    Ok(events)
}

/// Who hears about it: everyone who said yes or maybe.
///
/// Declined is excluded deliberately - reminding someone about a thing they
/// turned down is spam. Pending is excluded too: they never answered, and
/// the invite notification already asked them.
async fn recipients(db: &PgPool, event_id: Uuid) -> Result<Vec<Uuid>> {
    let ids = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT user_id FROM event_participants
        WHERE event_id = $1 AND status IN ('accepted', 'maybe')
        "#,
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;

    Ok(ids)
}

/// The message posted into the event's Discord thread.
///
/// French, and `<t:…:R>`/`<t:…:t>` for the times, to match
/// services::discord_announcement's existing announcement format - the
/// reminder lands in the same thread as that message, so a sudden switch to
/// English or to server-local timestamps would read as a different app.
/// Discord renders these timestamps in each reader's own timezone, which is
/// also why this doesn't try to use `users.timezone`.
pub fn format_reminder_message(event: &CalendarEvent) -> String {
    let ts = event.start_time.timestamp();
    let mut message = format!("⏰ **Rappel :** {} commence <t:{ts}:R> !\n", event.title);
    message.push_str(&format!("**Heure :** <t:{ts}:t>\n"));

    if let Some(location) = &event.location {
        message.push_str(&format!("**Lieu :** {location}\n"));
    }
    if let Some(price) = &event.price {
        message.push_str(&format!("**Prix :** {price}\n"));
    }

    message.push_str("\nÀ tout à l'heure ! 🎉");
    message
}

/// Sends any reminders that are due. Returns how many events were reminded,
/// so tests and the loop can report without a second query.
pub async fn send_due_reminders(
    base_url: &str,
    db: &PgPool,
    http: &Client,
    bot_token: Option<&str>,
    now: DateTime<Utc>,
) -> Result<usize> {
    let events = due_events(db, now).await?;
    let mut sent = 0;

    for event in events {
        // Belt and braces: the query already encodes the window, but going
        // through is_due keeps one definition of "due" rather than two that
        // can drift.
        if !is_due(now, event.start_time, event.reminder_sent_at) {
            continue;
        }

        let message = format!("{} starts in about an hour", event.title);
        for user_id in recipients(db, event.id).await? {
            // No preference check here - services::notifications::create
            // owns that, so this can't forget to respect it.
            if let Err(e) = crate::services::notifications::create(
                db,
                user_id,
                "event_reminder",
                None,
                Some(event.id),
                &message,
            )
            .await
            {
                tracing::warn!("Failed to create reminder notification: {:?}", e);
            }
        }

        // A thread started from a message has the same id as that message,
        // so discord_message_id addresses the thread directly - no separate
        // thread id is stored. If thread creation failed back when the
        // event was announced (discord_announcement tolerates that with a
        // warning), this POST 404s; log and carry on rather than letting one
        // event's missing thread stop the whole pass.
        if let (Some(bot_token), Some(thread_id)) = (bot_token, &event.discord_message_id)
            && let Err(e) = discord_feed::send_channel_message(
                base_url,
                http,
                bot_token,
                thread_id,
                &format_reminder_message(&event),
            )
            .await
        {
            tracing::warn!(
                "Failed to post reminder in thread for event {}: {:?}",
                event.id,
                e
            );
        }

        // Stamped even if Discord failed above: the in-app reminders did go
        // out, and retrying the whole event would double-notify everyone to
        // chase one Discord post.
        sqlx::query("UPDATE calendar_events SET reminder_sent_at = $1 WHERE id = $2")
            .bind(now)
            .bind(event.id)
            .execute(db)
            .await?;

        sent += 1;
    }

    Ok(sent)
}

pub async fn spawn_reminder_loop(base_url: String, db: PgPool, http: Client, bot_token: String) {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(POLL_INTERVAL_SECS));

    loop {
        ticker.tick().await;
        match send_due_reminders(&base_url, &db, &http, Some(&bot_token), Utc::now()).await {
            Ok(0) => {}
            Ok(n) => tracing::info!("⏰ Sent reminders for {n} event(s)"),
            Err(e) => tracing::error!("Reminder pass failed: {:?}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CreateEventRequest, ParticipationStatus};
    use crate::services::{calendar, notifications};
    use serde_json::json;
    use sqlx::PgPool;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn seed_user(db: &PgPool, discord_id: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (id, discord_id, username, created_at, updated_at)
            VALUES ($1, $2, $3, now(), now())
            "#,
        )
        .bind(id)
        .bind(discord_id)
        .bind(discord_id)
        .execute(db)
        .await
        .unwrap();
        id
    }

    /// An event starting inside the reminder window, with the given people
    /// invited and a Discord announcement message already linked (so the
    /// thread post has somewhere to go).
    async fn seed_upcoming_event(db: &PgPool, creator: Uuid, invitees: &[Uuid]) -> Uuid {
        let now = Utc::now();
        let event = calendar::create_event(
            db,
            creator,
            CreateEventRequest {
                title: "Raclette".to_string(),
                description: None,
                start_time: now + Duration::minutes(30),
                end_time: now + Duration::minutes(150),
                location: Some("Chez Lina".to_string()),
                visibility: None,
                participant_ids: Some(invitees.to_vec()),
                price: None,
                link: None,
            },
        )
        .await
        .unwrap();

        sqlx::query("UPDATE calendar_events SET discord_message_id = $1 WHERE id = $2")
            .bind("thread-123")
            .bind(event.id)
            .execute(db)
            .await
            .unwrap();

        event.id
    }

    async fn mock_discord() -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/channels/thread-123/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "m1" })))
            .mount(&server)
            .await;
        server
    }

    fn at(mins_from_now: i64) -> DateTime<Utc> {
        Utc::now() + Duration::minutes(mins_from_now)
    }

    #[test]
    fn due_inside_the_lead_window() {
        let now = Utc::now();
        assert!(is_due(now, at(55), None));
        assert!(is_due(now, at(1), None));
    }

    #[test]
    fn not_due_before_the_window_opens() {
        let now = Utc::now();
        assert!(!is_due(now, at(6 * 60), None));
        assert!(!is_due(now, at(61), None));
    }

    #[test]
    fn not_due_once_already_reminded() {
        let now = Utc::now();
        assert!(!is_due(now, at(30), Some(now - Duration::minutes(5))));
    }

    // The catch-up case: after an outage, everything that started during it
    // is inside no window we care about and must not fire.
    #[test]
    fn not_due_for_an_event_that_already_started() {
        let now = Utc::now();
        assert!(!is_due(now, at(-1), None));
        assert!(!is_due(now, at(-180), None));
        assert!(
            !is_due(now, now, None),
            "an event starting exactly now is not upcoming"
        );
    }

    #[sqlx::test]
    async fn reminds_accepted_and_maybe_but_not_declined_or_pending(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let going = seed_user(&db, "going").await;
        let maybe = seed_user(&db, "maybe").await;
        let declined = seed_user(&db, "declined").await;
        let no_answer = seed_user(&db, "noanswer").await;

        let event = seed_upcoming_event(&db, creator, &[going, maybe, declined, no_answer]).await;
        calendar::update_participation_status(&db, event, going, ParticipationStatus::Accepted)
            .await
            .unwrap();
        calendar::update_participation_status(&db, event, maybe, ParticipationStatus::Maybe)
            .await
            .unwrap();
        calendar::update_participation_status(&db, event, declined, ParticipationStatus::Declined)
            .await
            .unwrap();

        let server = mock_discord().await;
        let sent = send_due_reminders(
            &server.uri(),
            &db,
            &Client::new(),
            Some("test-token"),
            Utc::now(),
        )
        .await
        .unwrap();
        assert_eq!(sent, 1);

        for (user, expected, label) in [
            (going, 1, "accepted"),
            (maybe, 1, "maybe"),
            (creator, 1, "creator (auto-accepted)"),
            (declined, 0, "declined"),
            (no_answer, 0, "never answered"),
        ] {
            let reminders = notifications::list(&db, user, 50)
                .await
                .unwrap()
                .into_iter()
                .filter(|n| n.kind == "event_reminder")
                .count();
            assert_eq!(
                reminders, expected,
                "{label} should have {expected} reminder(s)"
            );
        }
    }

    #[sqlx::test]
    async fn a_second_pass_sends_nothing(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_upcoming_event(&db, creator, &[]).await;

        let server = mock_discord().await;
        let http = Client::new();

        let first = send_due_reminders(&server.uri(), &db, &http, Some("t"), Utc::now())
            .await
            .unwrap();
        assert_eq!(first, 1);

        let second = send_due_reminders(&server.uri(), &db, &http, Some("t"), Utc::now())
            .await
            .unwrap();
        assert_eq!(second, 0, "restarting the loop must not double-send");

        let stamped: Option<DateTime<Utc>> =
            sqlx::query_scalar("SELECT reminder_sent_at FROM calendar_events WHERE id = $1")
                .bind(event)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(stamped.is_some());
    }

    #[sqlx::test]
    async fn respects_the_users_reminder_preference(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        sqlx::query("UPDATE users SET notify_event_reminders = false WHERE id = $1")
            .bind(creator)
            .execute(&db)
            .await
            .unwrap();

        let _event = seed_upcoming_event(&db, creator, &[]).await;

        let server = mock_discord().await;
        send_due_reminders(&server.uri(), &db, &Client::new(), Some("t"), Utc::now())
            .await
            .unwrap();

        let reminders = notifications::list(&db, creator, 50)
            .await
            .unwrap()
            .into_iter()
            .filter(|n| n.kind == "event_reminder")
            .count();
        assert_eq!(
            reminders, 0,
            "the gate in notifications::create must apply here too"
        );
    }

    #[sqlx::test]
    async fn ignores_events_outside_the_window(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let now = Utc::now();

        for (title, starts_in) in [("Too far off", 300i64), ("Already started", -30)] {
            calendar::create_event(
                &db,
                creator,
                CreateEventRequest {
                    title: title.to_string(),
                    description: None,
                    start_time: now + Duration::minutes(starts_in),
                    end_time: now + Duration::minutes(starts_in + 60),
                    location: None,
                    visibility: None,
                    participant_ids: None,
                    price: None,
                    link: None,
                },
            )
            .await
            .unwrap();
        }

        let sent = send_due_reminders("http://unused.invalid", &db, &Client::new(), Some("t"), now)
            .await
            .unwrap();
        assert_eq!(sent, 0);
    }

    #[test]
    fn the_thread_message_uses_discord_timestamps_not_server_local_time() {
        let event = CalendarEvent {
            id: Uuid::new_v4(),
            creator_id: Uuid::new_v4(),
            title: "Raclette".to_string(),
            description: None,
            start_time: Utc::now() + Duration::minutes(30),
            end_time: Utc::now() + Duration::minutes(150),
            location: Some("Chez Lina".to_string()),
            visibility: crate::models::Visibility::Friends,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            discord_message_id: None,
            discord_channel_id: None,
            price: Some("15".to_string()),
            link: None,
            reminder_sent_at: None,
        };

        let message = format_reminder_message(&event);
        // <t:...:R> renders in each reader's own timezone - which is why
        // this doesn't consult users.timezone.
        assert!(message.contains(&format!("<t:{}:R>", event.start_time.timestamp())));
        assert!(message.contains("Raclette"));
        assert!(message.contains("Chez Lina"));
    }
}
