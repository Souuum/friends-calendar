use crate::models::{UpdateProfileRequest, User};
use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

/// Fetch-merge-update, same shape as services::calendar::update_event -
/// only the fields present in the request change.
pub async fn update_profile(
    db: &PgPool,
    user_id: Uuid,
    req: UpdateProfileRequest,
) -> Result<Option<User>> {
    let existing = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(db)
        .await?;
    let Some(mut user) = existing else {
        return Ok(None);
    };

    if let Some(display_name) = req.display_name {
        user.display_name = Some(display_name);
    }
    if let Some(timezone) = req.timezone {
        user.timezone = timezone;
    }
    if let Some(default_visibility) = req.default_visibility {
        user.default_visibility = default_visibility;
    }
    if let Some(v) = req.notify_event_invites {
        user.notify_event_invites = v;
    }
    if let Some(v) = req.notify_rsvp_changes {
        user.notify_rsvp_changes = v;
    }
    if let Some(v) = req.notify_announcements {
        user.notify_announcements = v;
    }
    if let Some(v) = req.notify_weekly_digest {
        user.notify_weekly_digest = v;
    }
    if let Some(v) = req.notify_event_reminders {
        user.notify_event_reminders = v;
    }

    let updated = sqlx::query_as::<_, User>(
        r#"
        UPDATE users
        SET display_name = $1, timezone = $2, default_visibility = $3,
            notify_event_invites = $4, notify_rsvp_changes = $5,
            notify_announcements = $6, notify_weekly_digest = $7,
            notify_event_reminders = $8, updated_at = $9
        WHERE id = $10
        RETURNING *
        "#,
    )
    .bind(&user.display_name)
    .bind(&user.timezone)
    .bind(&user.default_visibility)
    .bind(user.notify_event_invites)
    .bind(user.notify_rsvp_changes)
    .bind(user.notify_announcements)
    .bind(user.notify_weekly_digest)
    .bind(user.notify_event_reminders)
    .bind(Utc::now())
    .bind(user_id)
    .fetch_one(db)
    .await?;

    Ok(Some(updated))
}

pub enum DeleteAccountOutcome {
    Deleted,
    /// `confirm_username` didn't match the account's actual username - the
    /// safeguard against a stray/accidental request doing something
    /// irreversible. Distinguished from a generic failure so the handler
    /// can 400 with a clear message rather than pretending it's a server
    /// error.
    ConfirmationMismatch,
}

/// Cascades everywhere ON DELETE CASCADE is set up: friendships (both
/// directions), event_participants, calendar_events.creator_id (which
/// itself cascades to that event's participants/notifications),
/// friend_requests (both directions). notifications.actor_user_id is
/// ON DELETE SET NULL, not CASCADE - other people's notifications that
/// merely *mention* this user survive, just anonymized.
pub async fn delete_account(
    db: &PgPool,
    user_id: Uuid,
    confirm_username: &str,
) -> Result<DeleteAccountOutcome> {
    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(db)
        .await?;

    if username != confirm_username {
        return Ok(DeleteAccountOutcome::ConfirmationMismatch);
    }

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(db)
        .await?;

    Ok(DeleteAccountOutcome::Deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Visibility;

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

    #[sqlx::test]
    async fn update_profile_only_changes_fields_that_were_sent(db: PgPool) {
        let user_id = seed_user(&db, "me-discord", "me").await;

        let updated = update_profile(
            &db,
            user_id,
            UpdateProfileRequest {
                display_name: Some("Me!".to_string()),
                timezone: None,
                default_visibility: Some(Visibility::Public),
                notify_event_invites: None,
                notify_rsvp_changes: None,
                notify_announcements: None,
                notify_weekly_digest: None,
                notify_event_reminders: None,
            },
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.display_name.as_deref(), Some("Me!"));
        assert_eq!(updated.timezone, "UTC"); // untouched, default from the migration
        assert!(matches!(updated.default_visibility, Visibility::Public));
        assert!(updated.notify_event_invites); // untouched, default true
    }

    #[sqlx::test]
    async fn update_profile_returns_none_for_a_nonexistent_user(db: PgPool) {
        let result = update_profile(
            &db,
            Uuid::new_v4(),
            UpdateProfileRequest {
                display_name: None,
                timezone: None,
                default_visibility: None,
                notify_event_invites: None,
                notify_rsvp_changes: None,
                notify_announcements: None,
                notify_weekly_digest: None,
                notify_event_reminders: None,
            },
        )
        .await
        .unwrap();

        assert!(result.is_none());
    }

    #[sqlx::test]
    async fn delete_account_requires_the_username_to_match(db: PgPool) {
        let user_id = seed_user(&db, "me-discord", "me").await;

        let outcome = delete_account(&db, user_id, "not-me").await.unwrap();
        assert!(matches!(
            outcome,
            DeleteAccountOutcome::ConfirmationMismatch
        ));

        let still_there: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&db)
            .await
            .unwrap();
        assert!(still_there.is_some());
    }

    #[sqlx::test]
    async fn delete_account_removes_the_user_and_cascades(db: PgPool) {
        let user_id = seed_user(&db, "me-discord", "me").await;
        let event_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO calendar_events (id, creator_id, title, start_time, end_time, visibility, created_at, updated_at)
            VALUES ($1, $2, 'test event', now(), now() + interval '1 hour', 'private', now(), now())
            "#,
        )
        .bind(event_id)
        .bind(user_id)
        .execute(&db)
        .await
        .unwrap();

        let outcome = delete_account(&db, user_id, "me").await.unwrap();
        assert!(matches!(outcome, DeleteAccountOutcome::Deleted));

        let user_left: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&db)
            .await
            .unwrap();
        assert!(user_left.is_none());

        let event_left: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM calendar_events WHERE id = $1")
                .bind(event_id)
                .fetch_optional(&db)
                .await
                .unwrap();
        assert!(
            event_left.is_none(),
            "creator_id ON DELETE CASCADE should have removed the event too"
        );
    }
}
