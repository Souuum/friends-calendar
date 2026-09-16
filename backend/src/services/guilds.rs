//! Servers the bot is in, and where each event has been announced.
//!
//! Step 1 of `.claude/skills/multi-server/SKILL.md`. Nothing here yet decides
//! *which* servers an event goes to - `handlers::calendar::create_event`
//! still publishes to exactly one. This is the shape that makes more than one
//! representable.
//!
//! Deliberately no `Publication` struct or membership *readers* yet: the
//! server picker (step 5) is what needs to read this back, and building the
//! accessors ahead of a caller is how unused API accumulates.

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

/// Finds or creates the guild row for a Discord snowflake.
///
/// Called at startup for `DISCORD_GUILD_ID` so a deployment that has never
/// saved channel config on /server still has a guild to attach publications
/// to - migration 014 can only seed from `discord_bot_config`, since a
/// migration can't read env vars.
pub async fn ensure_guild(db: &PgPool, discord_guild_id: &str) -> Result<Uuid> {
    let existing: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM guilds WHERE discord_guild_id = $1")
            .bind(discord_guild_id)
            .fetch_optional(db)
            .await?;

    if let Some(id) = existing {
        return Ok(id);
    }

    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO guilds (id, discord_guild_id) VALUES ($1, $2) ON CONFLICT (discord_guild_id) DO NOTHING")
        .bind(id)
        .bind(discord_guild_id)
        .execute(db)
        .await?;

    // Re-read rather than assuming our insert won: two processes starting at
    // once would race, and ON CONFLICT DO NOTHING means the loser's id is not
    // the one in the table.
    let id: Uuid = sqlx::query_scalar("SELECT id FROM guilds WHERE discord_guild_id = $1")
        .bind(discord_guild_id)
        .fetch_one(db)
        .await?;

    Ok(id)
}

/// A server the bot is in, as shown on /servers.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct GuildInfo {
    pub id: Uuid,
    pub discord_guild_id: String,
    pub name: Option<String>,
    pub icon_url: Option<String>,
}

/// Records what Discord calls a server, from the gateway's `guild_create`.
///
/// Separate from `ensure_guild` because that one runs where only the id is
/// known (startup, publishing); this runs where Discord has just handed us
/// the name and icon, which is the only place they come from - nothing in
/// this app asks the user to type them.
pub async fn upsert_guild_metadata(
    db: &PgPool,
    discord_guild_id: &str,
    name: &str,
    icon: Option<&str>,
) -> Result<Uuid> {
    let id = ensure_guild(db, discord_guild_id).await?;

    sqlx::query("UPDATE guilds SET name = $1, icon = $2 WHERE id = $3")
        .bind(name)
        .bind(icon)
        .bind(id)
        .execute(db)
        .await?;

    Ok(id)
}

/// Every server the bot is in.
pub async fn list_guilds(db: &PgPool) -> Result<Vec<GuildInfo>> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
        "SELECT id, discord_guild_id, name, icon FROM guilds ORDER BY name NULLS LAST, added_at",
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, discord_guild_id, name, icon)| GuildInfo {
            icon_url: icon
                .as_deref()
                .map(|i| format!("https://cdn.discordapp.com/icons/{discord_guild_id}/{i}.png")),
            id,
            discord_guild_id,
            name,
        })
        .collect())
}

/// The Discord snowflake for a guild row.
pub async fn discord_id_of(db: &PgPool, guild_id: Uuid) -> Result<Option<String>> {
    let id = sqlx::query_scalar("SELECT discord_guild_id FROM guilds WHERE id = $1")
        .bind(guild_id)
        .fetch_optional(db)
        .await?;

    Ok(id)
}

/// Records that an event is to be announced in a server, before anything is
/// posted. Idempotent, so re-publishing to the same server is a no-op rather
/// than a duplicate.
pub async fn add_publication(
    db: &PgPool,
    event_id: Uuid,
    guild_id: Uuid,
    channel_id: &str,
) -> Result<Uuid> {
    sqlx::query(
        r#"
        INSERT INTO event_publications (id, event_id, guild_id, channel_id)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (event_id, guild_id) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(event_id)
    .bind(guild_id)
    .bind(channel_id)
    .execute(db)
    .await?;

    let id: Uuid = sqlx::query_scalar(
        "SELECT id FROM event_publications WHERE event_id = $1 AND guild_id = $2",
    )
    .bind(event_id)
    .bind(guild_id)
    .fetch_one(db)
    .await?;

    Ok(id)
}

/// Stamps the Discord message id once the announcement has actually posted.
pub async fn mark_published(db: &PgPool, publication_id: Uuid, message_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE event_publications SET discord_message_id = $1, posted_at = now() WHERE id = $2",
    )
    .bind(message_id)
    .bind(publication_id)
    .execute(db)
    .await?;

    Ok(())
}

/// The event a Discord message belongs to, for turning a reaction back into
/// an RSVP (bot.rs) and for tagging synced feed posts (discord_feed).
pub async fn event_for_message(db: &PgPool, discord_message_id: &str) -> Result<Option<Uuid>> {
    let event_id =
        sqlx::query_scalar("SELECT event_id FROM event_publications WHERE discord_message_id = $1")
            .bind(discord_message_id)
            .fetch_optional(db)
            .await?;

    Ok(event_id)
}

/// The Discord messages announcing an event - one per server it reached.
///
/// A thread is addressed by the message that started it, so these double as
/// the event's threads, which is what reminders post into. Publications that
/// never posted (Discord failed at announce time) have no message id and are
/// skipped here rather than by every caller.
pub async fn published_message_ids(db: &PgPool, event_id: Uuid) -> Result<Vec<String>> {
    let ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT discord_message_id FROM event_publications
        WHERE event_id = $1 AND discord_message_id IS NOT NULL
        ORDER BY posted_at
        "#,
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;

    Ok(ids)
}

/// Replaces the membership of one server with what a sync just observed.
///
/// Scoped to a single guild rather than "here are all of this user's
/// servers", because that's the shape the data arrives in: a guild sync
/// enumerates one server's members. Doing it per-user would mean a sync of
/// server A wrongly implying someone had left server B.
///
/// Rows for members who are no longer in the guild are removed, mirroring
/// how services::friends prunes stale `discord_guild` friendships.
pub async fn set_guild_members(db: &PgPool, guild_id: Uuid, user_ids: &[Uuid]) -> Result<()> {
    sqlx::query("DELETE FROM user_guilds WHERE guild_id = $1 AND NOT (user_id = ANY($2))")
        .bind(guild_id)
        .bind(user_ids)
        .execute(db)
        .await?;

    for user_id in user_ids {
        sqlx::query(
            r#"
            INSERT INTO user_guilds (user_id, guild_id, synced_at)
            VALUES ($1, $2, now())
            ON CONFLICT (user_id, guild_id) DO UPDATE SET synced_at = now()
            "#,
        )
        .bind(user_id)
        .bind(guild_id)
        .execute(db)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateEventRequest;
    use chrono::{Duration, Utc};

    async fn seed_user(db: &PgPool, discord_id: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, discord_id, username, created_at, updated_at) VALUES ($1,$2,$3,now(),now())")
            .bind(id)
            .bind(discord_id)
            .bind(discord_id)
            .execute(db)
            .await
            .unwrap();
        id
    }

    async fn seed_event(db: &PgPool, creator: Uuid) -> Uuid {
        let now = Utc::now();
        crate::services::calendar::create_event(
            db,
            creator,
            CreateEventRequest {
                title: "Raclette".to_string(),
                description: None,
                start_time: now + Duration::days(1),
                end_time: now + Duration::days(1) + Duration::hours(2),
                location: None,
                visibility: None,
                participant_ids: None,
                price: None,
                link: None,
                guild_ids: None,
                reminder_leads: Some(vec![]),
            },
        )
        .await
        .unwrap()
        .id
    }

    #[sqlx::test]
    async fn ensure_guild_is_idempotent(db: PgPool) {
        let first = ensure_guild(&db, "g1").await.unwrap();
        let second = ensure_guild(&db, "g1").await.unwrap();
        assert_eq!(first, second, "a restart must not create a second row");

        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM guilds")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    // The row records intent, the message id records the result - so a
    // Discord failure at announce time leaves the publication behind rather
    // than losing which server was chosen.
    #[sqlx::test]
    async fn a_publication_exists_before_anything_is_posted(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_event(&db, creator).await;
        let guild = ensure_guild(&db, "g1").await.unwrap();

        let publication = add_publication(&db, event, guild, "chan1").await.unwrap();

        // The row exists, but nothing resolves to it - nothing was posted.
        let row_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM event_publications WHERE event_id = $1")
                .bind(event)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(row_count, 1);
        assert!(published_message_ids(&db, event).await.unwrap().is_empty());
        assert!(event_for_message(&db, "anything").await.unwrap().is_none());

        mark_published(&db, publication, "msg-1").await.unwrap();
        assert_eq!(
            event_for_message(&db, "msg-1").await.unwrap(),
            Some(event),
            "this is how a ✅ reaction finds its event"
        );
    }

    #[sqlx::test]
    async fn publishing_to_the_same_server_twice_is_one_row(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_event(&db, creator).await;
        let guild = ensure_guild(&db, "g1").await.unwrap();

        let a = add_publication(&db, event, guild, "chan1").await.unwrap();
        let b = add_publication(&db, event, guild, "chan1").await.unwrap();

        assert_eq!(a, b);
        let row_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM event_publications WHERE event_id = $1")
                .bind(event)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(row_count, 1);
    }

    // The shape the whole change exists for.
    #[sqlx::test]
    async fn one_event_can_be_published_to_several_servers(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_event(&db, creator).await;

        for (guild, channel, message) in [("g1", "chan1", "msg-1"), ("g2", "chan2", "msg-2")] {
            let g = ensure_guild(&db, guild).await.unwrap();
            let p = add_publication(&db, event, g, channel).await.unwrap();
            mark_published(&db, p, message).await.unwrap();
        }

        assert_eq!(published_message_ids(&db, event).await.unwrap().len(), 2);
        // A reaction in either server resolves to the same event.
        assert_eq!(event_for_message(&db, "msg-1").await.unwrap(), Some(event));
        assert_eq!(event_for_message(&db, "msg-2").await.unwrap(), Some(event));
    }

    #[sqlx::test]
    async fn deleting_an_event_takes_its_publications_with_it(db: PgPool) {
        let creator = seed_user(&db, "creator").await;
        let event = seed_event(&db, creator).await;
        let guild = ensure_guild(&db, "g1").await.unwrap();
        add_publication(&db, event, guild, "chan1").await.unwrap();

        crate::services::calendar::delete_event(&db, event, creator)
            .await
            .unwrap();

        let left: i64 =
            sqlx::query_scalar("SELECT count(*) FROM event_publications WHERE event_id = $1")
                .bind(event)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(left, 0, "ON DELETE CASCADE, not orphaned rows");
    }

    async fn members_of(db: &PgPool, guild: Uuid) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT u.username FROM user_guilds ug JOIN users u ON u.id = ug.user_id WHERE ug.guild_id = $1 ORDER BY u.username",
        )
        .bind(guild)
        .fetch_all(db)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn syncing_a_guild_replaces_only_that_guilds_membership(db: PgPool) {
        let alice = seed_user(&db, "alice").await;
        let bob = seed_user(&db, "bob").await;
        let g1 = ensure_guild(&db, "g1").await.unwrap();
        let g2 = ensure_guild(&db, "g2").await.unwrap();

        set_guild_members(&db, g1, &[alice, bob]).await.unwrap();
        set_guild_members(&db, g2, &[alice]).await.unwrap();
        assert_eq!(members_of(&db, g1).await, vec!["alice", "bob"]);
        assert_eq!(members_of(&db, g2).await, vec!["alice"]);

        // bob leaves g1. Syncing g1 must not touch g2 - that's the reason
        // this is scoped per guild rather than per user.
        set_guild_members(&db, g1, &[alice]).await.unwrap();
        assert_eq!(members_of(&db, g1).await, vec!["alice"]);
        assert_eq!(members_of(&db, g2).await, vec!["alice"]);
    }
}
