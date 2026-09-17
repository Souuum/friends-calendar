//! Turning an announcement that already exists in Discord into a calendar
//! event.
//!
//! ## Why this exists
//!
//! `services::calendar::create_event` announces *outwards*: it writes the
//! event, then posts a new Discord message and records the publication. That
//! only covers events born in this app. Everything else - the posts people
//! write by hand in the server, and every event created before this
//! deployment had a database - is a message with ✅ reactions on it that the
//! app has no row for. Those show up in the feed tagged "General" and their
//! reactions can never become RSVPs, because `services::reaction_sync` walks
//! `event_publications` and there is nothing there to walk.
//!
//! Adoption is the same wiring run backwards: create the event, then bind it
//! to a message that is *already posted* instead of posting one. Once the
//! publication row exists every downstream feature works with no special
//! case - `bot.rs` resolves new reactions, `reaction_sync` backfills the old
//! ones, `discord_feed` re-tags the post as an event, reminders find the
//! thread, and publication-scoped visibility lets the server see it.
//!
//! ⚠️ **Nothing here parses the message.** What the event says is taken from
//! the request, which the user confirmed in a form. The client prefills that
//! form by parsing the post (`lib/utils/announcementParse.ts`), but a guess
//! about someone's free-form French prose is not something to write into a
//! calendar unreviewed, and keeping the guess client-side means the server
//! contract stays explicit.

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{CalendarEvent, CreateEventRequest};
use crate::services::{calendar, guilds};

/// Why an announcement can't become an event. Separate from `AppError` so
/// this module stays free of HTTP concerns; the handler maps them.
#[derive(Debug, PartialEq, Eq)]
pub enum AdoptError {
    /// No `announcement_posts` row with that id.
    UnknownPost,
    /// The message already backs an event. Adopting twice would create a
    /// second event competing for the same reactions.
    AlreadyAdopted,
    /// No server is registered, so there's nothing to publish into. The
    /// gateway's `guild_create` populates `guilds` on connect, so this means
    /// the bot has never been online with this database.
    NoGuild,
}

impl std::fmt::Display for AdoptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPost => write!(f, "No such announcement"),
            Self::AlreadyAdopted => write!(
                f,
                "That announcement is already on the calendar as an event"
            ),
            Self::NoGuild => write!(
                f,
                "No Discord server is registered yet - the bot records one when it connects"
            ),
        }
    }
}

/// Where an adopted event would be published: the message it binds to, and
/// the server that message lives in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptionTarget {
    pub message_id: String,
    pub channel_id: String,
    /// `guilds.id`, not the Discord snowflake.
    pub guild_id: Uuid,
}

/// Resolves an announcement to its adoption target, or says why it can't be
/// adopted. Read-only, so the handler can reject before creating anything.
pub async fn resolve_target(
    db: &PgPool,
    announcement_id: Uuid,
) -> Result<std::result::Result<AdoptionTarget, AdoptError>> {
    let post: Option<(String, String)> = sqlx::query_as(
        "SELECT discord_message_id, channel_id FROM announcement_posts WHERE id = $1",
    )
    .bind(announcement_id)
    .fetch_optional(db)
    .await?;

    let Some((message_id, channel_id)) = post else {
        return Ok(Err(AdoptError::UnknownPost));
    };

    // The same check `discord_feed::infer_tag` makes to decide whether a post
    // shows as "Event" - so the button the client hides on an event post and
    // the refusal here are driven by one fact, not two that can disagree.
    if guilds::event_for_message(db, &message_id).await?.is_some() {
        return Ok(Err(AdoptError::AlreadyAdopted));
    }

    // Prefers a guild that has already published into this channel, falling
    // back to the oldest registered one - the same resolution
    // `discord_feed::list_posts` uses to build thread links, for the same
    // reason: with several servers the first registered need not own this
    // channel.
    let guild_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT g.id
        FROM guilds g
        LEFT JOIN event_publications p ON p.guild_id = g.id AND p.channel_id = $1
        ORDER BY (p.id IS NOT NULL) DESC, g.added_at
        LIMIT 1
        "#,
    )
    .bind(&channel_id)
    .fetch_optional(db)
    .await?;

    let Some(guild_id) = guild_id else {
        return Ok(Err(AdoptError::NoGuild));
    };

    Ok(Ok(AdoptionTarget {
        message_id,
        channel_id,
        guild_id,
    }))
}

/// Creates the event and binds it to the already-posted message.
///
/// `req.guild_ids` is ignored on purpose: the server is decided by where the
/// message actually lives, not by what the client asks for. Announcing an
/// adopted event anywhere else would post a *second* message for something
/// the server has already seen.
///
/// If binding fails the freshly-created event is deleted rather than left
/// behind. An event with no publication is invisible to everyone but its
/// participants and would let a retry create a duplicate, since the
/// already-adopted guard keys off the publication row.
pub async fn adopt(
    db: &PgPool,
    creator_id: Uuid,
    target: &AdoptionTarget,
    mut req: CreateEventRequest,
) -> Result<CalendarEvent> {
    // Belt and braces. Today `calendar::create_event` ignores `guild_ids`
    // entirely - announcing is the *handler's* job
    // (`announce_to_selected_servers`), and the adopt handler simply never
    // calls it - so clearing this changes nothing you can observe from here.
    // It is set so that if the service ever learns to announce, adoption
    // doesn't start posting duplicate messages by inheritance. The guarantee
    // that actually holds today is tested functionally: adopting posts no
    // message at all.
    req.guild_ids = None;

    let event = calendar::create_event(db, creator_id, req).await?;

    match bind(db, event.id, target).await {
        Ok(()) => Ok(event),
        Err(e) => {
            if let Err(cleanup) = sqlx::query("DELETE FROM calendar_events WHERE id = $1")
                .bind(event.id)
                .execute(db)
                .await
            {
                tracing::error!(
                    "⚠️  Failed to roll back event {} after a failed adoption: {:?}",
                    event.id,
                    cleanup
                );
            }
            Err(e)
        }
    }
}

async fn bind(db: &PgPool, event_id: Uuid, target: &AdoptionTarget) -> Result<()> {
    let publication_id =
        guilds::add_publication(db, event_id, target.guild_id, &target.channel_id).await?;

    // The message exists already, so this stamps rather than records a post
    // that is about to happen.
    guilds::mark_published(db, publication_id, &target.message_id).await?;

    // Re-tag immediately instead of waiting for the next channel sync. The
    // post is an event the moment it's bound, and leaving the feed showing
    // "General" until someone hits Sync would look like the adoption failed.
    sqlx::query("UPDATE announcement_posts SET tag = 'event' WHERE discord_message_id = $1")
        .bind(&target.message_id)
        .execute(db)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use sqlx::PgPool;

    use crate::models::DiscordUser;
    use crate::services::auth::create_or_update_user;

    const MSG: &str = "message-42";
    const CHANNEL: &str = "channel-7";

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

    /// A synced Discord message with nothing in the app behind it - the
    /// exact state every hand-written announcement is in.
    async fn a_post(db: &PgPool, message_id: &str, channel_id: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO announcement_posts
                (id, discord_message_id, channel_id, author_discord_id, author_username,
                 body, tag, posted_at, synced_at)
            VALUES ($1, $2, $3, 'julioo-discord', 'Julioo', 'Concert ce soir', 'general', now(), now())
            "#,
        )
        .bind(id)
        .bind(message_id)
        .bind(channel_id)
        .execute(db)
        .await
        .unwrap();
        id
    }

    fn a_request(title: &str) -> CreateEventRequest {
        let start = Utc::now() + Duration::days(10);
        CreateEventRequest {
            title: title.into(),
            description: None,
            start_time: start,
            end_time: start + Duration::hours(3),
            location: Some("Le Bikini".into()),
            visibility: None,
            participant_ids: None,
            price: None,
            link: None,
            // A server that doesn't exist, so an adoption that published
            // where the *request* asked would violate the foreign key rather
            // than quietly succeed.
            guild_ids: Some(vec![Uuid::new_v4()]),
            reminder_leads: Some(vec![]),
        }
    }

    async fn tag_of(db: &PgPool, post_id: Uuid) -> String {
        sqlx::query_scalar("SELECT tag FROM announcement_posts WHERE id = $1")
            .bind(post_id)
            .fetch_one(db)
            .await
            .unwrap()
    }

    #[sqlx::test]
    async fn an_unknown_post_cannot_be_adopted(db: PgPool) {
        let outcome = resolve_target(&db, Uuid::new_v4()).await.unwrap();
        assert_eq!(outcome, Err(AdoptError::UnknownPost));
    }

    // The bot only registers a guild when the gateway connects. Without one
    // there is nowhere to publish, and an event published nowhere is
    // invisible to everyone but its participants - so refusing beats
    // creating something that silently reaches nobody.
    #[sqlx::test]
    async fn a_post_cannot_be_adopted_before_any_server_is_registered(db: PgPool) {
        let post = a_post(&db, MSG, CHANNEL).await;

        let outcome = resolve_target(&db, post).await.unwrap();

        assert_eq!(outcome, Err(AdoptError::NoGuild));
    }

    #[sqlx::test]
    async fn a_post_resolves_to_the_registered_guild_and_its_own_message(db: PgPool) {
        let post = a_post(&db, MSG, CHANNEL).await;
        let guild = guilds::ensure_guild(&db, "guild-1").await.unwrap();

        let target = resolve_target(&db, post).await.unwrap().unwrap();

        assert_eq!(
            target,
            AdoptionTarget {
                message_id: MSG.into(),
                channel_id: CHANNEL.into(),
                guild_id: guild,
            }
        );
    }

    // With several servers registered, the one that already publishes into
    // this channel owns it - the oldest registered guild need not.
    #[sqlx::test]
    async fn the_guild_that_publishes_into_the_channel_wins_over_the_oldest(db: PgPool) {
        let post = a_post(&db, MSG, CHANNEL).await;
        let oldest = guilds::ensure_guild(&db, "guild-oldest").await.unwrap();
        let owner = guilds::ensure_guild(&db, "guild-owner").await.unwrap();

        // Give `owner` a publication in this channel, via an unrelated event.
        let creator = a_user(&db, "creator").await;
        let other = calendar::create_event(&db, creator, a_request("Something else"))
            .await
            .unwrap();
        guilds::add_publication(&db, other.id, owner, CHANNEL)
            .await
            .unwrap();

        let target = resolve_target(&db, post).await.unwrap().unwrap();

        assert_eq!(target.guild_id, owner);
        assert_ne!(target.guild_id, oldest);
    }

    #[sqlx::test]
    async fn adopting_binds_the_event_to_the_message_that_already_exists(db: PgPool) {
        let post = a_post(&db, MSG, CHANNEL).await;
        guilds::ensure_guild(&db, "guild-1").await.unwrap();
        let creator = a_user(&db, "soum").await;
        let target = resolve_target(&db, post).await.unwrap().unwrap();

        let event = adopt(&db, creator, &target, a_request("EsdeeKid"))
            .await
            .unwrap();

        assert_eq!(event.title, "EsdeeKid");
        // The binding is what makes every downstream feature work: this is
        // the lookup bot.rs and reaction_sync both make.
        assert_eq!(
            guilds::event_for_message(&db, MSG).await.unwrap(),
            Some(event.id)
        );
        // No second message was posted - the publication carries the id of
        // the message that was already there.
        let message_ids = guilds::published_message_ids(&db, event.id).await.unwrap();
        assert_eq!(message_ids, vec![MSG.to_string()]);
    }

    // Waiting for the next channel sync to re-tag would leave the feed
    // showing "General" on a post that is now an event, which reads as the
    // adoption having failed.
    #[sqlx::test]
    async fn adopting_retags_the_post_as_an_event_immediately(db: PgPool) {
        let post = a_post(&db, MSG, CHANNEL).await;
        guilds::ensure_guild(&db, "guild-1").await.unwrap();
        let creator = a_user(&db, "soum").await;
        assert_eq!(tag_of(&db, post).await, "general");

        let target = resolve_target(&db, post).await.unwrap().unwrap();
        adopt(&db, creator, &target, a_request("EsdeeKid"))
            .await
            .unwrap();

        assert_eq!(tag_of(&db, post).await, "event");
    }

    // One publication, for the server that owns the message - not for
    // whatever the request happened to ask for, and not an extra one
    // alongside it.
    #[sqlx::test]
    async fn adopting_publishes_to_exactly_the_server_that_owns_the_message(db: PgPool) {
        let post = a_post(&db, MSG, CHANNEL).await;
        let real_guild = guilds::ensure_guild(&db, "guild-1").await.unwrap();
        let creator = a_user(&db, "soum").await;
        let target = resolve_target(&db, post).await.unwrap().unwrap();

        let event = adopt(&db, creator, &target, a_request("EsdeeKid"))
            .await
            .unwrap();

        let guild_ids: Vec<Uuid> =
            sqlx::query_scalar("SELECT guild_id FROM event_publications WHERE event_id = $1")
                .bind(event.id)
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(guild_ids, vec![real_guild]);
    }

    // Adopting twice would leave two events competing for one message's
    // reactions - and `event_for_message` returns only one of them, so which
    // one an RSVP landed on would be arbitrary.
    #[sqlx::test]
    async fn a_post_cannot_be_adopted_twice(db: PgPool) {
        let post = a_post(&db, MSG, CHANNEL).await;
        guilds::ensure_guild(&db, "guild-1").await.unwrap();
        let creator = a_user(&db, "soum").await;
        let target = resolve_target(&db, post).await.unwrap().unwrap();
        adopt(&db, creator, &target, a_request("EsdeeKid"))
            .await
            .unwrap();

        let second = resolve_target(&db, post).await.unwrap();

        assert_eq!(second, Err(AdoptError::AlreadyAdopted));
    }

    // An event the app already announced is a post tagged "event"; it is
    // adopted by definition and must be refused by the same guard.
    #[sqlx::test]
    async fn a_post_this_app_announced_cannot_be_adopted(db: PgPool) {
        let post = a_post(&db, MSG, CHANNEL).await;
        let guild = guilds::ensure_guild(&db, "guild-1").await.unwrap();
        let creator = a_user(&db, "soum").await;
        let event = calendar::create_event(&db, creator, a_request("Announced normally"))
            .await
            .unwrap();
        let publication = guilds::add_publication(&db, event.id, guild, CHANNEL)
            .await
            .unwrap();
        guilds::mark_published(&db, publication, MSG).await.unwrap();

        let outcome = resolve_target(&db, post).await.unwrap();

        assert_eq!(outcome, Err(AdoptError::AlreadyAdopted));
    }
}
