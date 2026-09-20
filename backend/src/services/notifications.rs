use crate::models::{NotificationInfo, ParticipationStatus, User};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// The `users` column that gates a given notification `kind`, or `None`
/// for kinds that are always delivered.
///
/// Deliberately ungated, rather than mapped onto a loosely-related toggle:
/// - `friend_request` / `friend_accepted` have no preference of their own.
///   They're low-volume and directly actionable, and reusing
///   `notify_event_invites` for them would mean a user who turned off
///   *event invites* silently stopped receiving *friend requests*. If
///   these should be switchable, they need their own column - see
///   `.claude/skills/settings-integrity/SKILL.md`.
///
/// `notify_weekly_digest` intentionally appears nowhere here: the digest
/// posts one message to a shared Discord channel (services::digest, gated
/// by the guild-level `discord_bot_config.digest_enabled`), so there is no
/// per-user delivery for a per-user preference to filter.
fn preference_column_for(kind: &str) -> Option<&'static str> {
    match kind {
        "event_invite" => Some("notify_event_invites"),
        "rsvp_change" => Some("notify_rsvp_changes"),
        "announcement" => Some("notify_announcements"),
        "event_reminder" => Some("notify_event_reminders"),
        _ => None,
    }
}

/// Create a notification for `user_id`, unless the recipient has switched
/// that kind off. `message` is rendered by the caller (see the trigger
/// call sites in services::calendar) - this function is intentionally dumb
/// storage, not a template engine.
///
/// The preference check lives here rather than at each call site so that
/// every trigger - including ones added later - is gated by construction.
/// A new caller cannot forget to respect the user's preferences, because
/// it never sees them.
#[allow(clippy::too_many_arguments)]
pub async fn create(
    db: &PgPool,
    user_id: Uuid,
    kind: &str,
    actor_user_id: Option<Uuid>,
    event_id: Option<Uuid>,
    message: &str,
) -> Result<()> {
    if let Some(column) = preference_column_for(kind) {
        // Column name comes from the match above, never from the caller -
        // it cannot carry user input into the query.
        let enabled: Option<bool> =
            sqlx::query_scalar(&format!("SELECT {column} FROM users WHERE id = $1"))
                .bind(user_id)
                .fetch_optional(db)
                .await?;

        // A missing user row means there's nobody to notify; a disabled
        // preference means they asked not to be. Both are a no-op, not an
        // error - the callers treat a failure here as worth logging.
        if enabled != Some(true) {
            return Ok(());
        }
    }

    sqlx::query(
        r#"
        INSERT INTO notifications (id, user_id, kind, actor_user_id, event_id, message, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(kind)
    .bind(actor_user_id)
    .bind(event_id)
    .bind(message)
    .bind(Utc::now())
    .execute(db)
    .await?;

    Ok(())
}

#[derive(sqlx::FromRow)]
struct NotificationRow {
    id: Uuid,
    kind: String,
    actor_username: Option<String>,
    actor_discord_id: Option<String>,
    actor_avatar: Option<String>,
    event_id: Option<Uuid>,
    message: String,
    read_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    my_status: Option<ParticipationStatus>,
}

pub async fn list(db: &PgPool, user_id: Uuid, limit: i64) -> Result<Vec<NotificationInfo>> {
    let rows = sqlx::query_as::<_, NotificationRow>(
        r#"
        SELECT n.id, n.kind, u.username AS actor_username, u.discord_id AS actor_discord_id,
               u.avatar AS actor_avatar, n.event_id, n.message, n.read_at, n.created_at,
               ep.status AS my_status
        FROM notifications n
        LEFT JOIN users u ON u.id = n.actor_user_id
        -- The recipient's own answer, so the page can show an invite it has
        -- already been acted on as answered rather than as still open.
        -- ⚠️ Joined on n.user_id, never the actor: whose answer this is
        -- matters, and the actor is the person who *sent* the invite.
        LEFT JOIN event_participants ep
               ON ep.event_id = n.event_id AND ep.user_id = n.user_id
        WHERE n.user_id = $1
        ORDER BY n.created_at DESC
        LIMIT $2
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(db)
    .await?;

    let notifications = rows
        .into_iter()
        .map(|row| {
            let actor_avatar_url = row
                .actor_discord_id
                .as_deref()
                .map(|discord_id| User::build_avatar_url(discord_id, &row.actor_avatar))
                .unwrap_or(None);

            NotificationInfo {
                id: row.id,
                kind: row.kind,
                actor_username: row.actor_username,
                actor_avatar_url,
                event_id: row.event_id,
                message: row.message,
                read: row.read_at.is_some(),
                created_at: row.created_at,
                my_status: row.my_status,
            }
        })
        .collect();

    Ok(notifications)
}

/// Marks a notification read. Idempotent (COALESCE keeps the original
/// read_at if it was already set). Returns false if no matching
/// notification exists for this user - lets the handler tell "not yours /
/// doesn't exist" apart from a real failure.
pub async fn mark_read(db: &PgPool, user_id: Uuid, notification_id: Uuid) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE notifications SET read_at = COALESCE(read_at, $1) WHERE id = $2 AND user_id = $3",
    )
    .bind(Utc::now())
    .bind(notification_id)
    .bind(user_id)
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn mark_all_read(db: &PgPool, user_id: Uuid) -> Result<u64> {
    let result =
        sqlx::query("UPDATE notifications SET read_at = $1 WHERE user_id = $2 AND read_at IS NULL")
            .bind(Utc::now())
            .bind(user_id)
            .execute(db)
            .await?;

    Ok(result.rows_affected())
}

pub async fn unread_count(db: &PgPool, user_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(db)
    .await?;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed_user(db: &PgPool, discord_id: &str, username: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (id, discord_id, username, created_at, updated_at)
            VALUES ($1, $2, $3, now(), now())
            "#,
        )
        .bind(id)
        .bind(discord_id)
        .bind(username)
        .execute(db)
        .await
        .unwrap();
        id
    }

    async fn seed_event(db: &PgPool, creator: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO calendar_events
                (id, creator_id, title, start_time, end_time, visibility, created_at, updated_at)
            VALUES ($1, $2, 'Raclette', now() + interval '1 day',
                    now() + interval '1 day 2 hours', 'friends', now(), now())
            "#,
        )
        .bind(id)
        .bind(creator)
        .execute(db)
        .await
        .unwrap();
        id
    }

    async fn invite(db: &PgPool, event: Uuid, user: Uuid, status: &str) {
        sqlx::query(
            r#"
            INSERT INTO event_participants (id, event_id, user_id, status, invited_at)
            VALUES (gen_random_uuid(), $1, $2, $3::participation_status, now())
            ON CONFLICT (event_id, user_id) DO UPDATE SET status = EXCLUDED.status
            "#,
        )
        .bind(event)
        .bind(user)
        .bind(status)
        .execute(db)
        .await
        .unwrap();
    }

    // --- my_status: "seen" and "answered" are different facts -------------

    /// ⚠️ The bug this exists for: the notifications page showed three
    /// untouched Going/Maybe/Can't buttons whether or not you had already
    /// answered, because nothing on the notification said. Answering looked
    /// like it did nothing, and reloading brought the buttons back.
    #[sqlx::test]
    async fn an_answered_invite_reports_the_answer(db: PgPool) {
        let host = seed_user(&db, "host", "host").await;
        let guest = seed_user(&db, "guest", "guest").await;
        let event = seed_event(&db, host).await;
        invite(&db, event, guest, "accepted").await;
        create(
            &db,
            guest,
            "event_invite",
            Some(host),
            Some(event),
            "invited",
        )
        .await
        .unwrap();

        let listed = list(&db, guest, 20).await.unwrap();

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].my_status, Some(ParticipationStatus::Accepted));
    }

    #[sqlx::test]
    async fn an_unanswered_invite_reports_pending_not_nothing(db: PgPool) {
        let host = seed_user(&db, "host", "host").await;
        let guest = seed_user(&db, "guest", "guest").await;
        let event = seed_event(&db, host).await;
        invite(&db, event, guest, "pending").await;
        create(
            &db,
            guest,
            "event_invite",
            Some(host),
            Some(event),
            "invited",
        )
        .await
        .unwrap();

        let listed = list(&db, guest, 20).await.unwrap();

        // Pending is a real state and must stay distinguishable from "not on
        // the list at all" - the buttons should show for one and not the other.
        assert_eq!(listed[0].my_status, Some(ParticipationStatus::Pending));
    }

    #[sqlx::test]
    async fn a_notification_about_no_event_has_no_status(db: PgPool) {
        let user = seed_user(&db, "u1", "u1").await;
        create(
            &db,
            user,
            "friend_request",
            None,
            None,
            "wants to be friends",
        )
        .await
        .unwrap();

        let listed = list(&db, user, 20).await.unwrap();

        assert_eq!(listed[0].my_status, None);
    }

    /// ⚠️ Joined on the *recipient*, never the actor. The actor is whoever
    /// sent the invite, and reading their answer would show you their RSVP
    /// on your own card.
    #[sqlx::test]
    async fn the_status_is_the_recipients_not_the_senders(db: PgPool) {
        let host = seed_user(&db, "host", "host").await;
        let guest = seed_user(&db, "guest", "guest").await;
        let event = seed_event(&db, host).await;
        invite(&db, event, host, "accepted").await;
        invite(&db, event, guest, "declined").await;
        create(
            &db,
            guest,
            "event_invite",
            Some(host),
            Some(event),
            "invited",
        )
        .await
        .unwrap();

        let listed = list(&db, guest, 20).await.unwrap();

        assert_eq!(
            listed[0].my_status,
            Some(ParticipationStatus::Declined),
            "showed the sender's answer instead of the recipient's"
        );
    }

    // Being removed from an event after the notification was written leaves
    // the row pointing at an event you are no longer on.
    #[sqlx::test]
    async fn an_event_you_are_not_on_has_no_status(db: PgPool) {
        let host = seed_user(&db, "host", "host").await;
        let guest = seed_user(&db, "guest", "guest").await;
        let event = seed_event(&db, host).await;
        create(
            &db,
            guest,
            "event_invite",
            Some(host),
            Some(event),
            "invited",
        )
        .await
        .unwrap();

        let listed = list(&db, guest, 20).await.unwrap();

        assert_eq!(listed[0].my_status, None);
    }

    #[sqlx::test]
    async fn create_respects_the_recipients_notification_preferences(db: PgPool) {
        let user = seed_user(&db, "u1", "u1").await;

        // Default is on (migration 008), so this one lands.
        create(&db, user, "event_invite", None, None, "invited")
            .await
            .unwrap();
        assert_eq!(list(&db, user, 50).await.unwrap().len(), 1);

        sqlx::query("UPDATE users SET notify_event_invites = false WHERE id = $1")
            .bind(user)
            .execute(&db)
            .await
            .unwrap();

        create(&db, user, "event_invite", None, None, "invited again")
            .await
            .unwrap();
        assert_eq!(
            list(&db, user, 50).await.unwrap().len(),
            1,
            "a disabled preference must not write a notification row"
        );

        // A different kind with its own preference is unaffected by the
        // one we switched off.
        create(&db, user, "rsvp_change", None, None, "someone answered")
            .await
            .unwrap();
        assert_eq!(list(&db, user, 50).await.unwrap().len(), 2);
    }

    #[sqlx::test]
    async fn create_still_delivers_kinds_that_have_no_preference(db: PgPool) {
        let user = seed_user(&db, "u1", "u1").await;

        // Turning every existing toggle off must not silence friend
        // requests - they deliberately have no preference of their own.
        sqlx::query(
            "UPDATE users SET notify_event_invites = false, notify_rsvp_changes = false, \
             notify_announcements = false, notify_weekly_digest = false WHERE id = $1",
        )
        .bind(user)
        .execute(&db)
        .await
        .unwrap();

        create(
            &db,
            user,
            "friend_request",
            None,
            None,
            "wants to be friends",
        )
        .await
        .unwrap();
        create(&db, user, "friend_accepted", None, None, "accepted you")
            .await
            .unwrap();

        assert_eq!(list(&db, user, 50).await.unwrap().len(), 2);
    }

    #[sqlx::test]
    async fn create_and_list_returns_newest_first_with_actor_info(db: PgPool) {
        let recipient = seed_user(&db, "me-discord", "me").await;
        let actor = seed_user(&db, "alice-discord", "alice").await;

        create(
            &db,
            recipient,
            "event_invite",
            Some(actor),
            None,
            "alice invited you to something",
        )
        .await
        .unwrap();
        create(
            &db,
            recipient,
            "rsvp_change",
            Some(actor),
            None,
            "alice accepted your event",
        )
        .await
        .unwrap();

        let notifications = list(&db, recipient, 10).await.unwrap();

        assert_eq!(notifications.len(), 2);
        // newest first
        assert_eq!(notifications[0].message, "alice accepted your event");
        assert_eq!(notifications[0].actor_username.as_deref(), Some("alice"));
        assert!(!notifications[0].read);
    }

    #[sqlx::test]
    async fn list_does_not_leak_another_users_notifications(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let someone_else = seed_user(&db, "other-discord", "other").await;

        create(&db, someone_else, "event_invite", None, None, "not yours")
            .await
            .unwrap();

        let notifications = list(&db, me, 10).await.unwrap();
        assert!(notifications.is_empty());
    }

    #[sqlx::test]
    async fn mark_read_is_idempotent_and_scoped_to_the_owner(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let someone_else = seed_user(&db, "other-discord", "other").await;

        create(&db, me, "event_invite", None, None, "hi")
            .await
            .unwrap();
        let notification = &list(&db, me, 10).await.unwrap()[0];

        // someone else can't mark it read
        let stolen = mark_read(&db, someone_else, notification.id).await.unwrap();
        assert!(!stolen);
        assert!(!list(&db, me, 10).await.unwrap()[0].read);

        let ok = mark_read(&db, me, notification.id).await.unwrap();
        assert!(ok);
        assert!(list(&db, me, 10).await.unwrap()[0].read);

        // marking again doesn't error and stays read
        let ok_again = mark_read(&db, me, notification.id).await.unwrap();
        assert!(ok_again);
    }

    #[sqlx::test]
    async fn mark_all_read_and_unread_count(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;

        create(&db, me, "event_invite", None, None, "one")
            .await
            .unwrap();
        create(&db, me, "event_invite", None, None, "two")
            .await
            .unwrap();
        create(&db, me, "event_invite", None, None, "three")
            .await
            .unwrap();

        assert_eq!(unread_count(&db, me).await.unwrap(), 3);

        mark_read(&db, me, list(&db, me, 10).await.unwrap()[0].id)
            .await
            .unwrap();
        assert_eq!(unread_count(&db, me).await.unwrap(), 2);

        let marked = mark_all_read(&db, me).await.unwrap();
        assert_eq!(marked, 2);
        assert_eq!(unread_count(&db, me).await.unwrap(), 0);
    }
}
