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
use reqwest::Client;
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

/// Discord's CDN URL for a guild icon hash.
///
/// Extracted because `list_guilds` and `register_guild_by_id` both need it,
/// and a second copy is how the two end up disagreeing about the path.
fn icon_url_for(discord_guild_id: &str, icon: Option<&str>) -> Option<String> {
    icon.map(|i| format!("https://cdn.discordapp.com/icons/{discord_guild_id}/{i}.png"))
}

/// Why registering a server by id didn't work.
#[derive(Debug, PartialEq, Eq)]
pub enum RegisterError {
    /// Not a Discord snowflake. Catching this here keeps a typo out of the
    /// URL we are about to build.
    NotASnowflake,
    /// Discord answered, and the bot is not a member of that server.
    BotNotInServer,
    /// No bot token configured, so nothing can be checked.
    NoBotToken,
    /// Discord could not be reached, or said something unexpected.
    Unreachable(String),
}

impl std::fmt::Display for RegisterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotASnowflake => write!(
                f,
                "That isn't a Discord server ID - it should be a long number, copied with Developer Mode on"
            ),
            Self::BotNotInServer => write!(
                f,
                "The bot isn't in that server yet. Use the invite link above to add it, then try again"
            ),
            Self::NoBotToken => write!(
                f,
                "This deployment has no Discord bot token configured, so servers can't be checked"
            ),
            Self::Unreachable(why) => write!(f, "Couldn't reach Discord: {why}"),
        }
    }
}

/// Registers a server the user names by id, **after checking with Discord**.
///
/// ⚠️ This deliberately does *not* trust the id. Servers normally register
/// themselves - the gateway's `guild_create` fires on join and for every
/// server on reconnect - so the only reason to type one in is that the
/// gateway hasn't been up to do it. That makes this a recovery path, and a
/// recovery path that writes whatever it is handed is worse than none: a
/// guild the bot is not in cannot be announced to, cannot list channels and
/// has no name, so it would sit in the picker looking real and fail at the
/// moment somebody published to it.
///
/// Asking Discord settles it, and returns the name and icon as a side
/// effect - so the row is complete immediately rather than blank until the
/// bot next reconnects.
pub async fn register_guild_by_id(
    db: &PgPool,
    http: &Client,
    base_url: &str,
    bot_token: Option<&str>,
    discord_guild_id: &str,
) -> std::result::Result<GuildInfo, RegisterError> {
    let id = discord_guild_id.trim();
    // Snowflakes are decimal digits. Anything else is a paste of the wrong
    // thing (an invite URL, a channel id with a `#`), and would otherwise be
    // interpolated straight into the request path.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(RegisterError::NotASnowflake);
    }

    let token = bot_token.ok_or(RegisterError::NoBotToken)?;

    let response = http
        .get(format!("{base_url}/guilds/{id}"))
        .header("Authorization", format!("Bot {token}"))
        .send()
        .await
        .map_err(|e| RegisterError::Unreachable(e.to_string()))?;

    // ⚠️ Discord answers 404 for a guild the bot isn't in, not 403 - it does
    // not distinguish "no such server" from "not your server", deliberately,
    // so this cannot be used to probe which ids exist. Both mean the same
    // thing to us.
    if response.status() == reqwest::StatusCode::NOT_FOUND
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Err(RegisterError::BotNotInServer);
    }
    if !response.status().is_success() {
        let status = response.status();
        return Err(RegisterError::Unreachable(format!("HTTP {status}")));
    }

    #[derive(serde::Deserialize)]
    struct GuildPayload {
        id: String,
        name: String,
        icon: Option<String>,
    }

    let guild: GuildPayload = response
        .json()
        .await
        .map_err(|e| RegisterError::Unreachable(e.to_string()))?;

    let row_id = upsert_guild_metadata(db, &guild.id, &guild.name, guild.icon.as_deref())
        .await
        .map_err(|e| RegisterError::Unreachable(e.to_string()))?;

    Ok(GuildInfo {
        id: row_id,
        icon_url: icon_url_for(&guild.id, guild.icon.as_deref()),
        discord_guild_id: guild.id,
        name: Some(guild.name),
    })
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
            icon_url: icon_url_for(&discord_guild_id, icon.as_deref()),
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

    // --- registering a server by id -------------------------------------

    async fn discord_serving(body: serde_json::Value, status: u16) -> wiremock::MockServer {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/guilds/123456789012345678"))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&server)
            .await;
        server
    }

    #[sqlx::test]
    async fn registering_records_the_name_and_icon_discord_gives_back(db: PgPool) {
        let server = discord_serving(
            serde_json::json!({
                "id": "123456789012345678",
                "name": "The Hangout",
                "icon": "abc123"
            }),
            200,
        )
        .await;

        let guild = register_guild_by_id(
            &db,
            &reqwest::Client::new(),
            &server.uri(),
            Some("bot-token"),
            "123456789012345678",
        )
        .await
        .unwrap();

        assert_eq!(guild.discord_guild_id, "123456789012345678");
        // ⚠️ The name comes from Discord, never from the request - there is no
        // field for a caller to supply one.
        assert_eq!(guild.name.as_deref(), Some("The Hangout"));
        assert_eq!(
            guild.icon_url.as_deref(),
            Some("https://cdn.discordapp.com/icons/123456789012345678/abc123.png")
        );

        // And it is really in the table, so the picker will see it.
        let listed = list_guilds(&db).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name.as_deref(), Some("The Hangout"));
    }

    /// ⚠️ The guard the whole endpoint exists around. A guild the bot is not
    /// in cannot be announced to, cannot list channels and has no name - so
    /// recording one would put a dead entry in the server picker that fails
    /// only at the moment somebody publishes to it.
    #[sqlx::test]
    async fn a_server_the_bot_is_not_in_is_refused_and_not_recorded(db: PgPool) {
        let server = discord_serving(serde_json::json!({ "message": "Unknown Guild" }), 404).await;

        let err = register_guild_by_id(
            &db,
            &reqwest::Client::new(),
            &server.uri(),
            Some("bot-token"),
            "123456789012345678",
        )
        .await
        .unwrap_err();

        assert_eq!(err, RegisterError::BotNotInServer);
        assert!(
            list_guilds(&db).await.unwrap().is_empty(),
            "a refused server must leave no row behind"
        );
    }

    // Discord answers 403 rather than 404 in some cases; both mean the same
    // thing here and neither should record anything.
    #[sqlx::test]
    async fn a_forbidden_guild_is_treated_the_same_way(db: PgPool) {
        let server = discord_serving(serde_json::json!({ "message": "Missing Access" }), 403).await;

        let err = register_guild_by_id(
            &db,
            &reqwest::Client::new(),
            &server.uri(),
            Some("bot-token"),
            "123456789012345678",
        )
        .await
        .unwrap_err();

        assert_eq!(err, RegisterError::BotNotInServer);
        assert!(list_guilds(&db).await.unwrap().is_empty());
    }

    // ⚠️ The id goes straight into a request path. Rejecting non-digits here
    // keeps a pasted invite URL or a `../` out of it.
    #[sqlx::test]
    async fn a_value_that_is_not_a_snowflake_never_reaches_discord(db: PgPool) {
        for bad in [
            "",
            "   ",
            "not-an-id",
            "https://discord.gg/abc",
            "12345/../999",
            "123 456",
        ] {
            let err = register_guild_by_id(
                &db,
                &reqwest::Client::new(),
                // Unroutable: if this were ever called the test would hang or
                // error, rather than quietly passing.
                "http://127.0.0.1:1",
                Some("bot-token"),
                bad,
            )
            .await
            .unwrap_err();

            assert_eq!(err, RegisterError::NotASnowflake, "accepted {bad:?}");
        }
    }

    #[sqlx::test]
    async fn surrounding_whitespace_is_forgiven(db: PgPool) {
        let server = discord_serving(
            serde_json::json!({ "id": "123456789012345678", "name": "The Hangout", "icon": null }),
            200,
        )
        .await;

        let guild = register_guild_by_id(
            &db,
            &reqwest::Client::new(),
            &server.uri(),
            Some("bot-token"),
            "  123456789012345678  ",
        )
        .await
        .unwrap();

        assert_eq!(guild.discord_guild_id, "123456789012345678");
        assert_eq!(guild.icon_url, None);
    }

    // Registering one that already registered itself through the gateway must
    // update it rather than duplicate it.
    #[sqlx::test]
    async fn registering_a_server_twice_leaves_one_row(db: PgPool) {
        let server = discord_serving(
            serde_json::json!({ "id": "123456789012345678", "name": "Renamed", "icon": null }),
            200,
        )
        .await;
        upsert_guild_metadata(&db, "123456789012345678", "Old name", None)
            .await
            .unwrap();

        register_guild_by_id(
            &db,
            &reqwest::Client::new(),
            &server.uri(),
            Some("bot-token"),
            "123456789012345678",
        )
        .await
        .unwrap();

        let listed = list_guilds(&db).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name.as_deref(), Some("Renamed"));
    }

    #[sqlx::test]
    async fn without_a_bot_token_it_says_so_rather_than_recording_anything(db: PgPool) {
        let err = register_guild_by_id(
            &db,
            &reqwest::Client::new(),
            "http://127.0.0.1:1",
            None,
            "123456789012345678",
        )
        .await
        .unwrap_err();

        assert_eq!(err, RegisterError::NoBotToken);
        assert!(list_guilds(&db).await.unwrap().is_empty());
    }

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
