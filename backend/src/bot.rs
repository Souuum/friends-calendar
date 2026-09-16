use serenity::async_trait;
use serenity::model::channel::Reaction;
use serenity::model::gateway::Ready;
use serenity::model::prelude::*;
use serenity::prelude::*;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct DiscordBot;

impl DiscordBot {
    // Boxed error: serenity::Error is ~136 bytes, which clippy's
    // result_large_err rejects for a Result returned by value. Boxing costs
    // an allocation only on the failure path, and the sole caller (main.rs's
    // spawned task) just logs it.
    pub async fn start(
        bot_token: String,
        db: PgPool,
        announcement_channel_id: u64,
    ) -> Result<(), Box<serenity::Error>> {
        let intents = GatewayIntents::GUILD_MESSAGE_REACTIONS
            | GatewayIntents::GUILDS
            | GatewayIntents::GUILD_MEMBERS;

        let handler = Handler {
            db: Arc::new(db),
            announcement_channel_id,
        };

        let mut client = Client::builder(&bot_token, intents)
            .event_handler(handler)
            .await
            .map_err(Box::new)?;

        tracing::info!("🤖 Discord bot starting...");
        client.start().await.map_err(Box::new)?;

        Ok(())
    }
}

struct Handler {
    db: Arc<PgPool>,
    announcement_channel_id: u64,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        tracing::info!("🤖 Discord bot {} is connected!", ready.user.name);
        tracing::info!("📢 Monitoring channel ID: {}", self.announcement_channel_id);
    }

    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        // Only process reactions in the announcement channel
        if reaction.channel_id.get() != self.announcement_channel_id {
            return;
        }

        // Only process white check mark emoji
        if !is_attendance_emoji(&reaction.emoji) {
            return;
        }

        tracing::info!(
            "✅ User {} reacted with check mark to message {} in announcement channel",
            reaction.user_id.map(|id| id.get()).unwrap_or(0),
            reaction.message_id.get()
        );

        if let Err(e) = self.handle_event_reaction(&ctx, &reaction).await {
            tracing::error!("❌ Failed to handle reaction: {:?}", e);
        }
    }

    async fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        // Only process reactions in the announcement channel
        if reaction.channel_id.get() != self.announcement_channel_id {
            return;
        }

        // Only process white check mark emoji
        if !is_attendance_emoji(&reaction.emoji) {
            return;
        }

        tracing::info!(
            "❌ User {} removed check mark from message {}",
            reaction.user_id.map(|id| id.get()).unwrap_or(0),
            reaction.message_id.get()
        );

        if let Err(e) = self.handle_reaction_remove(&ctx, &reaction).await {
            tracing::error!("❌ Failed to handle reaction removal: {:?}", e);
        }
    }
}

impl Handler {
    // Thin adapters over the free functions below. Everything that touches
    // the database lives there, taking a &PgPool and plain strings, so it
    // can be exercised by #[sqlx::test] without a Discord gateway. serenity
    // types stop at this boundary on purpose - see the module tests.
    async fn handle_event_reaction(
        &self,
        ctx: &Context,
        reaction: &Reaction,
    ) -> anyhow::Result<()> {
        let user_id = reaction
            .user_id
            .ok_or_else(|| anyhow::anyhow!("No user ID"))?;

        // The one thing that genuinely needs Discord: the reaction carries
        // a user id, not a username, and we want a name on the row.
        let discord_user = user_id.to_user(&ctx.http).await?;

        let outcome = record_attendance(
            &self.db,
            &reaction.message_id.get().to_string(),
            &discord_user.id.get().to_string(),
            &discord_user.name,
        )
        .await?;

        match outcome {
            ReactionOutcome::UnknownEvent => {
                tracing::warn!(
                    "⚠️  No event found for message {}",
                    reaction.message_id.get()
                );
            }
            ReactionOutcome::Recorded { event_id, .. } => {
                tracing::info!("✅ {} is attending event {}", discord_user.name, event_id);
            }
            ReactionOutcome::UnknownUser => {}
        }

        Ok(())
    }

    async fn handle_reaction_remove(
        &self,
        _ctx: &Context,
        reaction: &Reaction,
    ) -> anyhow::Result<()> {
        // No Discord call needed: withdrawing only has to find an existing
        // user, and the reaction already carries their id.
        let user_id = reaction
            .user_id
            .ok_or_else(|| anyhow::anyhow!("No user ID"))?;

        let outcome = withdraw_attendance(
            &self.db,
            &reaction.message_id.get().to_string(),
            &user_id.get().to_string(),
        )
        .await?;

        if let ReactionOutcome::Recorded { event_id, .. } = outcome {
            tracing::info!("❌ User declined participation for event {}", event_id);
        }

        Ok(())
    }
}

/// What a ✅ reaction did, so callers (and tests) can tell "recorded" from
/// "that message isn't one of ours".
#[derive(Debug, PartialEq)]
pub enum ReactionOutcome {
    /// No calendar event is linked to that Discord message. Reactions on
    /// unrelated messages in the channel land here and are a no-op, not an
    /// error.
    UnknownEvent,
    /// Withdrawal only: the reactor has no account here, so there's no RSVP
    /// to withdraw. Deliberately does *not* create one - un-reacting is not
    /// a reason to appear in the database.
    UnknownUser,
    Recorded {
        event_id: Uuid,
        user_id: Uuid,
    },
}

/// Is this the emoji the bot treats as "I'm coming"?
pub fn is_attendance_emoji(emoji: &ReactionType) -> bool {
    match emoji {
        ReactionType::Unicode(s) => s == "✅" || s == "☑️",
        _ => false,
    }
}

/// Records a ✅ on an announcement as an accepted RSVP, creating the user
/// on first sight.
///
/// Uses runtime-checked sqlx::query/query_as (not the query! macro) to match
/// the rest of the codebase, and deliberately so: query! needs a live,
/// schema-matching DATABASE_URL at *compile* time, which would make
/// `cargo build` fail on a fresh clone or in CI without a pre-seeded DB.
pub async fn record_attendance(
    db: &PgPool,
    discord_message_id: &str,
    discord_id: &str,
    username: &str,
) -> anyhow::Result<ReactionOutcome> {
    let event: Option<(Uuid, Uuid)> =
        sqlx::query_as("SELECT id, creator_id FROM calendar_events WHERE discord_message_id = $1")
            .bind(discord_message_id)
            .fetch_optional(db)
            .await?;

    let Some((event_id, _creator_id)) = event else {
        return Ok(ReactionOutcome::UnknownEvent);
    };

    let user_id = get_or_create_user(db, discord_id, username).await?;

    // ON CONFLICT DO UPDATE, so re-reacting is idempotent and a previous
    // "declined" is flipped back rather than being left to contradict the
    // reaction that's visibly there.
    sqlx::query(
        r#"
        INSERT INTO event_participants (id, event_id, user_id, status, invited_at, responded_at)
        VALUES ($1, $2, $3, 'accepted', NOW(), NOW())
        ON CONFLICT (event_id, user_id)
        DO UPDATE SET status = 'accepted', responded_at = NOW()
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(event_id)
    .bind(user_id)
    .execute(db)
    .await?;

    Ok(ReactionOutcome::Recorded { event_id, user_id })
}

/// Removing the ✅ marks the RSVP declined rather than deleting the row, so
/// the creator can still see who pulled out.
pub async fn withdraw_attendance(
    db: &PgPool,
    discord_message_id: &str,
    discord_id: &str,
) -> anyhow::Result<ReactionOutcome> {
    let event_id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM calendar_events WHERE discord_message_id = $1")
            .bind(discord_message_id)
            .fetch_optional(db)
            .await?;

    let Some(event_id) = event_id else {
        return Ok(ReactionOutcome::UnknownEvent);
    };

    let user_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE discord_id = $1")
        .bind(discord_id)
        .fetch_optional(db)
        .await?;

    let Some(user_id) = user_id else {
        return Ok(ReactionOutcome::UnknownUser);
    };

    sqlx::query(
        r#"
        UPDATE event_participants
        SET status = 'declined', responded_at = NOW()
        WHERE event_id = $1 AND user_id = $2
        "#,
    )
    .bind(event_id)
    .bind(user_id)
    .execute(db)
    .await?;

    Ok(ReactionOutcome::Recorded { event_id, user_id })
}

async fn get_or_create_user(db: &PgPool, discord_id: &str, username: &str) -> anyhow::Result<Uuid> {
    let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE discord_id = $1")
        .bind(discord_id)
        .fetch_optional(db)
        .await?;

    if let Some(id) = existing {
        return Ok(id);
    }

    let user_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO users (id, discord_id, username, discriminator, created_at, updated_at)
        VALUES ($1, $2, $3, '0', NOW(), NOW())
        "#,
    )
    .bind(user_id)
    .bind(discord_id)
    .bind(username)
    .execute(db)
    .await?;

    tracing::info!("👤 Created new user: {} ({})", username, discord_id);
    Ok(user_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CreateEventRequest, ParticipationStatus};
    use chrono::{Duration, Utc};

    // First tests this module has ever had. The gateway connection itself
    // still isn't covered - that needs a live Discord socket - but every
    // decision the bot makes when a reaction arrives now is, because the
    // logic no longer lives inside the serenity event handlers.

    async fn seed_user(db: &PgPool, discord_id: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, discord_id, username, created_at, updated_at) VALUES ($1,$2,$3,now(),now())",
        )
        .bind(id)
        .bind(discord_id)
        .bind(discord_id)
        .execute(db)
        .await
        .unwrap();
        id
    }

    /// An announced event, i.e. one with a Discord message to react to.
    async fn seed_announced_event(db: &PgPool, creator: Uuid, message_id: &str) -> Uuid {
        let now = Utc::now();
        let event = crate::services::calendar::create_event(
            db,
            creator,
            CreateEventRequest {
                title: "Raclette".to_string(),
                description: None,
                start_time: now + Duration::days(2),
                end_time: now + Duration::days(2) + Duration::hours(2),
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

        sqlx::query("UPDATE calendar_events SET discord_message_id = $1 WHERE id = $2")
            .bind(message_id)
            .bind(event.id)
            .execute(db)
            .await
            .unwrap();

        event.id
    }

    async fn status_of(db: &PgPool, event_id: Uuid, user_id: Uuid) -> Option<ParticipationStatus> {
        sqlx::query_scalar("SELECT status FROM event_participants WHERE event_id=$1 AND user_id=$2")
            .bind(event_id)
            .bind(user_id)
            .fetch_optional(db)
            .await
            .unwrap()
    }

    #[test]
    fn only_check_marks_count_as_attendance() {
        assert!(is_attendance_emoji(&ReactionType::Unicode(
            "✅".to_string()
        )));
        assert!(is_attendance_emoji(&ReactionType::Unicode(
            "☑️".to_string()
        )));
        // Every other reaction on an announcement is just a reaction.
        assert!(!is_attendance_emoji(&ReactionType::Unicode(
            "👍".to_string()
        )));
        assert!(!is_attendance_emoji(&ReactionType::Unicode(
            "❌".to_string()
        )));
    }

    #[sqlx::test]
    async fn reacting_creates_the_user_and_marks_them_going(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_announced_event(&db, creator, "msg-1").await;

        let outcome = record_attendance(&db, "msg-1", "newcomer-discord", "newcomer")
            .await
            .unwrap();

        let ReactionOutcome::Recorded { event_id, user_id } = outcome else {
            panic!("expected the RSVP to be recorded, got {outcome:?}");
        };
        assert_eq!(event_id, event);
        assert_eq!(
            status_of(&db, event, user_id).await,
            Some(ParticipationStatus::Accepted)
        );

        // Someone who reacts in Discord without ever opening the app still
        // needs a row to be a participant.
        let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(username, "newcomer");
    }

    #[sqlx::test]
    async fn reacting_twice_is_idempotent(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_announced_event(&db, creator, "msg-1").await;

        record_attendance(&db, "msg-1", "u", "u").await.unwrap();
        record_attendance(&db, "msg-1", "u", "u").await.unwrap();

        let rows: i64 =
            sqlx::query_scalar("SELECT count(*) FROM event_participants WHERE event_id=$1")
                .bind(event)
                .fetch_one(&db)
                .await
                .unwrap();
        // The creator plus the one reactor - not a duplicate row each time.
        assert_eq!(rows, 2);
    }

    // The reaction is visibly there, so the stored answer has to agree with
    // it rather than keeping an older "declined".
    #[sqlx::test]
    async fn re_reacting_after_withdrawing_flips_back_to_going(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_announced_event(&db, creator, "msg-1").await;

        let ReactionOutcome::Recorded { user_id, .. } =
            record_attendance(&db, "msg-1", "u", "u").await.unwrap()
        else {
            panic!("expected recorded");
        };
        withdraw_attendance(&db, "msg-1", "u").await.unwrap();
        assert_eq!(
            status_of(&db, event, user_id).await,
            Some(ParticipationStatus::Declined)
        );

        record_attendance(&db, "msg-1", "u", "u").await.unwrap();
        assert_eq!(
            status_of(&db, event, user_id).await,
            Some(ParticipationStatus::Accepted)
        );
    }

    #[sqlx::test]
    async fn a_reaction_on_an_unrelated_message_is_a_no_op(db: PgPool) {
        seed_user(&db, "creator").await;

        let outcome = record_attendance(&db, "some-other-message", "u", "u")
            .await
            .unwrap();

        assert_eq!(outcome, ReactionOutcome::UnknownEvent);
        // And crucially it didn't create a user on the way to finding that out.
        let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE discord_id = 'u'")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(users, 0);
    }

    #[sqlx::test]
    async fn withdrawing_marks_declined_rather_than_deleting_the_row(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_announced_event(&db, creator, "msg-1").await;
        let ReactionOutcome::Recorded { user_id, .. } =
            record_attendance(&db, "msg-1", "u", "u").await.unwrap()
        else {
            panic!("expected recorded");
        };

        withdraw_attendance(&db, "msg-1", "u").await.unwrap();

        // Declined, not gone: the creator should still see who pulled out.
        assert_eq!(
            status_of(&db, event, user_id).await,
            Some(ParticipationStatus::Declined)
        );
    }

    #[sqlx::test]
    async fn withdrawing_by_a_stranger_does_not_create_an_account(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        seed_announced_event(&db, creator, "msg-1").await;

        let outcome = withdraw_attendance(&db, "msg-1", "never-seen-before")
            .await
            .unwrap();

        assert_eq!(outcome, ReactionOutcome::UnknownUser);
        let users: i64 =
            sqlx::query_scalar("SELECT count(*) FROM users WHERE discord_id = 'never-seen-before'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(
            users, 0,
            "un-reacting is not a reason to appear in the database"
        );
    }

    #[sqlx::test]
    async fn the_creator_can_rejoin_their_own_event(db: PgPool) {
        let creator = seed_user(&db, "creator-discord").await;
        let event = seed_announced_event(&db, creator, "msg-1").await;

        withdraw_attendance(&db, "msg-1", "creator-discord")
            .await
            .unwrap();
        assert_eq!(
            status_of(&db, event, creator).await,
            Some(ParticipationStatus::Declined)
        );

        record_attendance(&db, "msg-1", "creator-discord", "creator-discord")
            .await
            .unwrap();
        assert_eq!(
            status_of(&db, event, creator).await,
            Some(ParticipationStatus::Accepted)
        );
    }
}
