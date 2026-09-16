use crate::models::CalendarEvent;
use crate::services::discord_feed;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

/// Used when a create request doesn't mention reminders at all. An explicit
/// empty list means "none" and is honoured as such - only *silence* gets a
/// default.
pub const DEFAULT_LEAD_MINUTES: i32 = 60;

// The set of lead times offered to the creator lives in the UI
// (CreateEventModal's REMINDER_CHOICES), not here. This module accepts any
// non-negative value, so a duplicate list on this side would be a claim
// nothing enforces - the column's CHECK constraint is the real contract.

/// How often the loop wakes. Deliberately much shorter than the lead time -
/// polling hourly for a one-hour lead would let a reminder land up to an
/// hour early or late, which for "starts in an hour" is the whole message.
const POLL_INTERVAL_SECS: u64 = 300;

/// One pending reminder joined to its event. `#[sqlx(flatten)]` maps the
/// `e.*` half straight into CalendarEvent, so this stays in step with that
/// struct automatically.
#[derive(sqlx::FromRow)]
struct DueReminder {
    reminder_id: Uuid,
    lead_minutes: i32,
    sent_at: Option<DateTime<Utc>>,
    #[sqlx(flatten)]
    event: CalendarEvent,
}

/// Whether a given reminder should fire now. Pure, so the scheduling rules
/// are testable without a database or a Discord mock - same split as
/// `services::digest::is_due`.
///
/// The `start_time > now` half is the one that's easy to forget: if the
/// process was down over the window, every event that started during the
/// outage would otherwise look due on restart and fire a reminder for
/// something already underway.
///
/// (A zero lead can't be due either, since the window would have to satisfy
/// `start > now AND start <= now` - but that's now a curiosity rather than
/// load-bearing: since migration 012 "no reminder" is the absence of a row,
/// not a 0 stored in one.)
pub fn is_due(
    now: DateTime<Utc>,
    start_time: DateTime<Utc>,
    lead_minutes: i32,
    reminder_sent_at: Option<DateTime<Utc>>,
) -> bool {
    if reminder_sent_at.is_some() {
        return false;
    }
    if start_time <= now {
        return false;
    }
    start_time <= now + Duration::minutes(lead_minutes as i64)
}

/// Reminders whose moment has arrived, with the event each belongs to.
///
/// One row per *reminder*, not per event: an event with 1-week/48h/1h
/// reminders appears three times across the day, once as each falls due,
/// and each is stamped independently.
///
/// `lead_minutes > 0` is gone - it was only needed while 0 meant "off". A
/// switched-off event now simply has no rows here.
async fn due_reminders(db: &PgPool, now: DateTime<Utc>) -> Result<Vec<DueReminder>> {
    let rows = sqlx::query_as::<_, DueReminder>(
        r#"
        SELECT r.id AS reminder_id, r.lead_minutes, r.sent_at, e.*
        FROM event_reminders r
        JOIN calendar_events e ON e.id = r.event_id
        WHERE r.sent_at IS NULL
          AND e.start_time > $1
          AND e.start_time <= $1 + make_interval(mins => r.lead_minutes)
        ORDER BY e.start_time ASC
        "#,
    )
    .bind(now)
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// Replaces an event's reminders with `leads`.
///
/// Wholesale replacement rather than a diff: the form always submits the
/// complete set, so reconciling additions and removals separately would be
/// more moving parts for the same result. Non-positive values and duplicates
/// are dropped rather than rejected - they describe the same intent as
/// leaving them out, and failing a whole event save over one is unhelpful.
///
/// Already-sent reminders that are kept retain their `sent_at`, so editing
/// an event's other fields can't re-notify anyone. `ON CONFLICT DO NOTHING`
/// is what preserves that.
pub async fn set_reminders(db: &PgPool, event_id: Uuid, leads: &[i32]) -> Result<()> {
    let mut wanted: Vec<i32> = leads.iter().copied().filter(|m| *m > 0).collect();
    wanted.sort_unstable();
    wanted.dedup();

    sqlx::query("DELETE FROM event_reminders WHERE event_id = $1 AND NOT (lead_minutes = ANY($2))")
        .bind(event_id)
        .bind(&wanted)
        .execute(db)
        .await?;

    for lead in &wanted {
        sqlx::query(
            r#"
            INSERT INTO event_reminders (id, event_id, lead_minutes)
            VALUES ($1, $2, $3)
            ON CONFLICT (event_id, lead_minutes) DO NOTHING
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(event_id)
        .bind(lead)
        .execute(db)
        .await?;
    }

    Ok(())
}

/// The offsets configured for an event, ascending.
pub async fn leads_for(db: &PgPool, event_id: Uuid) -> Result<Vec<i32>> {
    let leads = sqlx::query_scalar::<_, i32>(
        "SELECT lead_minutes FROM event_reminders WHERE event_id = $1 ORDER BY lead_minutes ASC",
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;

    Ok(leads)
}

/// Lets every reminder for an event fire again - used when the event moves,
/// since a reminder that already went out described the old time.
pub async fn clear_sent(db: &PgPool, event_id: Uuid) -> Result<()> {
    sqlx::query("UPDATE event_reminders SET sent_at = NULL WHERE event_id = $1")
        .bind(event_id)
        .execute(db)
        .await?;
    Ok(())
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

/// Renders a lead time for the in-app notification text.
///
/// The Discord message doesn't need this - `<t:…:R>` renders "in 2 days" on
/// its own, in the reader's locale. The in-app feed has no such primitive,
/// and the message is stored at write time (see models::notification), so
/// it has to read correctly forever rather than being recomputed later.
pub fn humanise_lead(lead_minutes: i32) -> String {
    match lead_minutes {
        m if m % 10080 == 0 && m >= 10080 => {
            let weeks = m / 10080;
            if weeks == 1 {
                "1 week".to_string()
            } else {
                format!("{weeks} weeks")
            }
        }
        m if m % 1440 == 0 && m >= 1440 => {
            let days = m / 1440;
            if days == 1 {
                "1 day".to_string()
            } else {
                format!("{days} days")
            }
        }
        m if m % 60 == 0 && m >= 60 => {
            let hours = m / 60;
            if hours == 1 {
                "1 hour".to_string()
            } else {
                format!("{hours} hours")
            }
        }
        m => format!("{m} minutes"),
    }
}

/// The message posted into the event's Discord thread.
///
/// French, and `<t:…:R>`/`<t:…:t>` for the times, to match
/// services::discord_announcement's existing announcement format - the
/// reminder lands in the same thread as that message, so a sudden switch to
/// English or to server-local timestamps would read as a different app.
/// Discord renders these timestamps in each reader's own timezone, which is
/// also why this doesn't try to use `users.timezone`.
pub fn format_reminder_message(event: &CalendarEvent, lead_minutes: i32) -> String {
    let ts = event.start_time.timestamp();
    let _ = lead_minutes; // <t:…:R> already says "in 2 days" in the reader's locale
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
    let due = due_reminders(db, now).await?;
    let mut sent = 0;

    for item in due {
        let event = &item.event;

        // Belt and braces: the query already encodes the window, but going
        // through is_due keeps one definition of "due" rather than two that
        // can drift.
        if !is_due(now, event.start_time, item.lead_minutes, item.sent_at) {
            continue;
        }

        let message = format!(
            "{} starts in {}",
            event.title,
            humanise_lead(item.lead_minutes)
        );
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
                &format_reminder_message(event, item.lead_minutes),
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
        // out, and retrying would double-notify everyone to chase one
        // Discord post. Scoped to this reminder row, so the event's other
        // lead times are untouched.
        sqlx::query("UPDATE event_reminders SET sent_at = $1 WHERE id = $2")
            .bind(now)
            .bind(item.reminder_id)
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
                reminder_leads: None,
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

    fn minimal_request_at(title: &str, start: DateTime<Utc>) -> CreateEventRequest {
        CreateEventRequest {
            title: title.to_string(),
            description: None,
            start_time: start,
            end_time: start + Duration::hours(2),
            location: None,
            visibility: None,
            participant_ids: None,
            price: None,
            link: None,
            reminder_leads: None,
        }
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

    const HOUR: i32 = 60;

    #[test]
    fn due_inside_the_lead_window() {
        let now = Utc::now();
        assert!(is_due(now, at(55), HOUR, None));
        assert!(is_due(now, at(1), HOUR, None));
    }

    #[test]
    fn not_due_before_the_window_opens() {
        let now = Utc::now();
        assert!(!is_due(now, at(6 * 60), HOUR, None));
        assert!(!is_due(now, at(61), HOUR, None));
    }

    // The creator's choice is what decides the window - the same event is
    // due or not depending only on the lead they picked.
    #[test]
    fn the_window_follows_the_events_own_lead_time() {
        let now = Utc::now();
        let in_two_days = at(2 * 1440 - 10);

        assert!(
            !is_due(now, in_two_days, HOUR, None),
            "1h lead: far too early"
        );
        assert!(
            !is_due(now, in_two_days, 1440, None),
            "1 day lead: still too early"
        );
        assert!(is_due(now, in_two_days, 2880, None), "2 day lead: due");
        assert!(is_due(now, in_two_days, 10080, None), "1 week lead: due");
    }

    // 0 isn't handled by a branch anywhere - it just can't satisfy
    // `start > now AND start <= now + 0`.
    #[test]
    fn a_zero_lead_never_fires() {
        let now = Utc::now();
        for mins in [1, 59, 1440, 100_000] {
            assert!(!is_due(now, at(mins), 0, None), "0 lead must never be due");
        }
    }

    #[test]
    fn not_due_once_already_reminded() {
        let now = Utc::now();
        assert!(!is_due(now, at(30), HOUR, Some(now - Duration::minutes(5))));
    }

    // The catch-up case: after an outage, everything that started during it
    // is inside no window we care about and must not fire.
    #[test]
    fn not_due_for_an_event_that_already_started() {
        let now = Utc::now();
        assert!(!is_due(now, at(-1), HOUR, None));
        assert!(!is_due(now, at(-180), HOUR, None));
        assert!(
            !is_due(now, at(-1), 10080, None),
            "a long lead doesn't resurrect a past event"
        );
        assert!(
            !is_due(now, now, HOUR, None),
            "an event starting exactly now is not upcoming"
        );
    }

    #[test]
    fn humanises_each_offered_lead_time() {
        assert_eq!(humanise_lead(60), "1 hour");
        assert_eq!(humanise_lead(180), "3 hours");
        assert_eq!(humanise_lead(1440), "1 day");
        assert_eq!(humanise_lead(2880), "2 days");
        assert_eq!(humanise_lead(10080), "1 week");
        assert_eq!(humanise_lead(20160), "2 weeks");
        assert_eq!(humanise_lead(30), "30 minutes");
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
            sqlx::query_scalar("SELECT sent_at FROM event_reminders WHERE event_id = $1")
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
    async fn the_window_is_per_event_not_global(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let now = Utc::now();

        // Same start time, different creator choices. Only the events whose
        // own lead reaches this far ahead should fire.
        for (title, lead) in [
            ("One hour lead", 60),
            ("One day lead", 1440),
            ("One week lead", 10080),
            ("No reminder", 0),
        ] {
            let mut req = minimal_request_at(title, now + Duration::minutes(2000));
            req.reminder_leads = Some(vec![lead]);
            calendar::create_event(&db, creator, req).await.unwrap();
        }

        let sent = send_due_reminders("http://unused.invalid", &db, &Client::new(), None, now)
            .await
            .unwrap();

        // 2000 minutes out: inside a 1-week window, outside a 1-day one.
        assert_eq!(sent, 1, "only the week-lead event is inside its own window");

        let reminded: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT e.title FROM calendar_events e
            JOIN event_reminders r ON r.event_id = e.id
            WHERE r.sent_at IS NOT NULL
            "#,
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(reminded, vec!["One week lead".to_string()]);
    }

    // The whole point of the child table: each lead fires on its own
    // schedule and is stamped on its own, so an event can have several.
    #[sqlx::test]
    async fn each_lead_time_fires_separately_as_it_comes_due(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let now = Utc::now();

        // Starts in ~25 hours, with 1-week / 1-day / 1-hour reminders.
        let mut req = minimal_request_at("Ski trip", now + Duration::minutes(1500));
        req.reminder_leads = Some(vec![10080, 1440, 60]);
        let event = calendar::create_event(&db, creator, req).await.unwrap().id;

        let server = mock_discord().await;
        let http = Client::new();

        // Right now only the 1-week reminder's window has opened.
        assert_eq!(
            send_due_reminders(&server.uri(), &db, &http, None, now)
                .await
                .unwrap(),
            1
        );

        // Half an hour later nothing new is due (1470 min still to go, so
        // the 1-day window hasn't opened) and the week one doesn't repeat.
        assert_eq!(
            send_due_reminders(&server.uri(), &db, &http, None, now + Duration::minutes(30))
                .await
                .unwrap(),
            0
        );

        // Inside a day of the start, the 1-day reminder comes due.
        assert_eq!(
            send_due_reminders(
                &server.uri(),
                &db,
                &http,
                None,
                now + Duration::minutes(120)
            )
            .await
            .unwrap(),
            1
        );

        // And finally the 1-hour one, an hour before it starts.
        assert_eq!(
            send_due_reminders(
                &server.uri(),
                &db,
                &http,
                None,
                now + Duration::minutes(1450)
            )
            .await
            .unwrap(),
            1
        );

        let sent: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM event_reminders WHERE event_id = $1 AND sent_at IS NOT NULL",
        )
        .bind(event)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(sent, 3, "all three fired, each exactly once");
    }

    #[sqlx::test]
    async fn set_reminders_replaces_the_set_but_keeps_sent_markers(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let now = Utc::now();

        let mut req = minimal_request_at("Party", now + Duration::minutes(30));
        req.reminder_leads = Some(vec![60, 1440]);
        let event = calendar::create_event(&db, creator, req).await.unwrap().id;

        let server = mock_discord().await;
        send_due_reminders(&server.uri(), &db, &Client::new(), None, now)
            .await
            .unwrap();

        // Keep the 60 (already sent) and swap 1440 for 180.
        set_reminders(&db, event, &[60, 180]).await.unwrap();
        assert_eq!(leads_for(&db, event).await.unwrap(), vec![60, 180]);

        let already_sent: Option<DateTime<Utc>> = sqlx::query_scalar(
            "SELECT sent_at FROM event_reminders WHERE event_id = $1 AND lead_minutes = 60",
        )
        .bind(event)
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(
            already_sent.is_some(),
            "editing the set must not re-arm a reminder that already went out"
        );
    }

    #[sqlx::test]
    async fn set_reminders_drops_duplicates_and_non_positive_values(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_upcoming_event(&db, creator, &[]).await;

        set_reminders(&db, event, &[60, 60, 0, -30, 1440])
            .await
            .unwrap();

        // Same intent as leaving them out, so they're dropped rather than
        // failing the whole save.
        assert_eq!(leads_for(&db, event).await.unwrap(), vec![60, 1440]);
    }

    #[sqlx::test]
    async fn an_empty_lead_list_means_no_reminders_at_all(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let now = Utc::now();

        let mut req = minimal_request_at("Silent", now + Duration::minutes(30));
        req.reminder_leads = Some(vec![]);
        let event = calendar::create_event(&db, creator, req).await.unwrap().id;

        assert!(leads_for(&db, event).await.unwrap().is_empty());
        assert_eq!(
            send_due_reminders("http://unused.invalid", &db, &Client::new(), None, now)
                .await
                .unwrap(),
            0
        );
    }

    #[sqlx::test]
    async fn omitting_leads_on_create_gets_the_default_reminder(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_upcoming_event(&db, creator, &[]).await;

        // Silence is not the same as an explicit empty list.
        assert_eq!(
            leads_for(&db, event).await.unwrap(),
            vec![DEFAULT_LEAD_MINUTES]
        );
    }

    #[sqlx::test]
    async fn deleting_an_event_takes_its_reminders_with_it(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_upcoming_event(&db, creator, &[]).await;

        calendar::delete_event(&db, event, creator).await.unwrap();

        let left: i64 =
            sqlx::query_scalar("SELECT count(*) FROM event_reminders WHERE event_id = $1")
                .bind(event)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(left, 0, "ON DELETE CASCADE, not orphaned rows");
    }

    #[sqlx::test]
    async fn an_event_with_reminders_off_is_never_picked_up(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let now = Utc::now();

        let mut req = minimal_request_at("Silent", now + Duration::minutes(30));
        req.reminder_leads = Some(vec![]);
        calendar::create_event(&db, creator, req).await.unwrap();

        let sent = send_due_reminders("http://unused.invalid", &db, &Client::new(), None, now)
            .await
            .unwrap();
        assert_eq!(sent, 0);
    }

    // Moving an event invalidates a reminder that already went out - it was
    // about the old time.
    #[sqlx::test]
    async fn rescheduling_lets_the_reminder_fire_again(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_upcoming_event(&db, creator, &[]).await;

        let server = mock_discord().await;
        let http = Client::new();

        assert_eq!(
            send_due_reminders(&server.uri(), &db, &http, Some("t"), Utc::now())
                .await
                .unwrap(),
            1
        );

        // Push it a week out, then back into the window.
        calendar::update_event(
            &db,
            event,
            creator,
            crate::models::UpdateEventRequest {
                title: None,
                description: None,
                start_time: Some(Utc::now() + Duration::minutes(45)),
                end_time: Some(Utc::now() + Duration::minutes(165)),
                location: None,
                visibility: None,
                price: None,
                link: None,
                reminder_leads: None,
            },
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(
            send_due_reminders(&server.uri(), &db, &http, Some("t"), Utc::now())
                .await
                .unwrap(),
            1,
            "a rescheduled event should be reminded about again"
        );
    }

    #[sqlx::test]
    async fn editing_something_other_than_the_time_does_not_re_remind(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_upcoming_event(&db, creator, &[]).await;

        let server = mock_discord().await;
        let http = Client::new();
        send_due_reminders(&server.uri(), &db, &http, Some("t"), Utc::now())
            .await
            .unwrap();

        calendar::update_event(
            &db,
            event,
            creator,
            crate::models::UpdateEventRequest {
                title: Some("Renamed".to_string()),
                description: None,
                start_time: None,
                end_time: None,
                location: None,
                visibility: None,
                price: None,
                link: None,
                reminder_leads: None,
            },
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(
            send_due_reminders(&server.uri(), &db, &http, Some("t"), Utc::now())
                .await
                .unwrap(),
            0,
            "renaming an event must not re-notify everyone"
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
                    reminder_leads: None,
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
        };

        let message = format_reminder_message(&event, DEFAULT_LEAD_MINUTES);
        // <t:...:R> renders in each reader's own timezone - which is why
        // this doesn't consult users.timezone.
        assert!(message.contains(&format!("<t:{}:R>", event.start_time.timestamp())));
        assert!(message.contains("Raclette"));
        assert!(message.contains("Chez Lina"));
    }
}
