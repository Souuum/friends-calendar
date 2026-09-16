use crate::models::{FriendInfo, LinkedServerInfo, SyncFriendsResult, User};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

const MEMBERS_PAGE_SIZE: usize = 1000;

// Discord does not expose a user's real Friends/relationships list to bots
// or OAuth2 apps — that's a private, undocumented endpoint
// (`/users/@me/relationships`) gated behind a full user token, and using it
// from anything but the official client is against Discord's Developer
// Terms of Service. The closest ToS-compliant proxy for a small
// friend-group app like this one is: two app users are "friends" if they
// both belong to the Discord server (guild) this bot lives in. That's what
// this module syncs.
//
// Every function here takes `base_url` as a required parameter rather than
// hardcoding Discord's real API — production callers pass
// `&state.discord_api_base` (which defaults to the real API), tests point
// it at a `wiremock::MockServer`. See .claude/skills/add-tests/SKILL.md.

#[derive(Debug, Deserialize)]
struct DiscordGuildMemberUser {
    id: String,
    #[serde(default)]
    bot: bool,
}

#[derive(Debug, Deserialize)]
struct DiscordGuildMember {
    user: Option<DiscordGuildMemberUser>,
}

/// Page through the configured guild's member list via the bot's REST API
/// and return the discord_id of every non-bot member. Requires the bot's
/// application to have the privileged "Server Members Intent" enabled in
/// the Discord developer portal, same as the gateway bot does.
pub async fn fetch_guild_member_discord_ids(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    guild_id: &str,
) -> Result<Vec<String>> {
    fetch_guild_member_discord_ids_paged(base_url, MEMBERS_PAGE_SIZE, http, bot_token, guild_id)
        .await
}

// `page_size` only exists as a separate parameter so the pagination/"keep
// following `after`" branch is reachable in a test without a fixture of
// 1000+ fake members. Production always goes through
// `fetch_guild_member_discord_ids` above.
async fn fetch_guild_member_discord_ids_paged(
    base_url: &str,
    page_size: usize,
    http: &Client,
    bot_token: &str,
    guild_id: &str,
) -> Result<Vec<String>> {
    let mut discord_ids = Vec::new();
    let mut after: Option<String> = None;

    loop {
        let mut url = format!("{base_url}/guilds/{guild_id}/members?limit={page_size}");
        if let Some(cursor) = &after {
            url.push_str(&format!("&after={cursor}"));
        }

        let response = http
            .get(&url)
            .header("Authorization", format!("Bot {bot_token}"))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Discord API error ({status}): {body}"));
        }

        let members: Vec<DiscordGuildMember> = response.json().await?;
        let page_len = members.len();

        for member in &members {
            if let Some(user) = &member.user
                && !user.bot
            {
                discord_ids.push(user.id.clone());
            }
        }

        after = members
            .last()
            .and_then(|m| m.user.as_ref())
            .map(|u| u.id.clone());

        if page_len < page_size || after.is_none() {
            break;
        }
    }

    Ok(discord_ids)
}

/// Sync `user_id`'s friends against every other app user who currently
/// shares the configured Discord guild with them. Friendship rows are
/// stored in both directions and stamped with `synced_at` so a friend who
/// leaves the server (and is absent from the next sync) gets dropped.
pub async fn sync_friends(
    base_url: &str,
    db: &PgPool,
    http: &Client,
    bot_token: &str,
    guild_id: &str,
    user_id: Uuid,
    own_discord_id: &str,
) -> Result<SyncFriendsResult> {
    let member_discord_ids =
        fetch_guild_member_discord_ids(base_url, http, bot_token, guild_id).await?;

    let candidate_ids: Vec<String> = member_discord_ids
        .into_iter()
        .filter(|id| id != own_discord_id)
        .collect();

    // Only match against people who already have an account in this app —
    // we can't create friendships with Discord members who've never logged in.
    let matched_users = sqlx::query_as::<_, User>("SELECT * FROM users WHERE discord_id = ANY($1)")
        .bind(&candidate_ids)
        .fetch_all(db)
        .await?;

    // Record who is actually in this server while we have the answer. Friend
    // sync has always derived friendships from guild membership and then
    // thrown the membership away; publishing to a chosen set of servers needs
    // it kept. Best-effort: failing to record membership shouldn't fail the
    // friend sync the user asked for.
    match crate::services::guilds::ensure_guild(db, guild_id).await {
        Ok(guild) => {
            let mut members: Vec<Uuid> = matched_users.iter().map(|u| u.id).collect();
            members.push(user_id); // the syncing user is a member too
            if let Err(e) = crate::services::guilds::set_guild_members(db, guild, &members).await {
                tracing::warn!("Failed to record guild membership: {:?}", e);
            }
        }
        Err(e) => tracing::warn!("Failed to register guild {}: {:?}", guild_id, e),
    }

    let now = Utc::now();
    let mut synced_ids = Vec::with_capacity(matched_users.len());

    for friend in &matched_users {
        synced_ids.push(friend.id);

        for (a, b) in [(user_id, friend.id), (friend.id, user_id)] {
            sqlx::query(
                r#"
                INSERT INTO friendships (id, user_id, friend_id, source, synced_at, created_at)
                VALUES ($1, $2, $3, 'discord_guild', $4, $4)
                ON CONFLICT (user_id, friend_id)
                DO UPDATE SET synced_at = EXCLUDED.synced_at
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(a)
            .bind(b)
            .bind(now)
            .execute(db)
            .await?;
        }
    }

    // Figure out which previously-synced friends this pass no longer
    // confirms (e.g. they left the server), so `removed` counts distinct
    // friends the same way `synced` does above — not raw DB rows, which
    // would silently come out 2x since each friendship is stored as a pair.
    let previously_synced_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT friend_id FROM friendships WHERE user_id = $1 AND source = 'discord_guild'",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    let stale_ids: Vec<Uuid> = previously_synced_ids
        .into_iter()
        .filter(|id| !synced_ids.contains(id))
        .collect();

    for stale_id in &stale_ids {
        sqlx::query(
            r#"
            DELETE FROM friendships
            WHERE source = 'discord_guild'
              AND ((user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1))
            "#,
        )
        .bind(user_id)
        .bind(stale_id)
        .execute(db)
        .await?;
    }

    let removed = stale_ids.len();

    let friends = get_friends(db, user_id).await?;

    Ok(SyncFriendsResult {
        synced: matched_users.len(),
        removed,
        friends,
    })
}

pub async fn get_friends(db: &PgPool, user_id: Uuid) -> Result<Vec<FriendInfo>> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, Option<String>, DateTime<Utc>)>(
        r#"
        SELECT u.id, u.discord_id, u.username, u.avatar, f.synced_at
        FROM friendships f
        JOIN users u ON u.id = f.friend_id
        WHERE f.user_id = $1
        ORDER BY u.username ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    let friends = rows
        .into_iter()
        .map(
            |(user_id, discord_id, username, avatar, synced_at)| FriendInfo {
                user_id,
                username,
                avatar_url: User::build_avatar_url(&discord_id, &avatar),
                synced_at,
            },
        )
        .collect();

    Ok(friends)
}

#[derive(Debug, Deserialize)]
struct DiscordGuildResponse {
    id: String,
    name: String,
    icon: Option<String>,
    approximate_member_count: Option<u64>,
}

/// Basic public info (name, icon, approximate member count) for the Discord
/// server this app is linked to, via `GET /guilds/{id}`. Any bot member of
/// a guild can read this — no elevated permissions needed.
pub async fn get_linked_server_info(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    guild_id: &str,
) -> Result<LinkedServerInfo> {
    let url = format!("{base_url}/guilds/{guild_id}?with_counts=true");

    let response = http
        .get(&url)
        .header("Authorization", format!("Bot {bot_token}"))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Discord API error ({status}): {body}"));
    }

    let guild: DiscordGuildResponse = response.json().await?;

    Ok(LinkedServerInfo {
        icon_url: LinkedServerInfo::build_icon_url(&guild.id, guild.icon.as_deref()),
        id: guild.id,
        name: guild.name,
        approximate_member_count: guild.approximate_member_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    fn discord_member(id: &str, bot: bool) -> serde_json::Value {
        json!({ "user": { "id": id, "username": format!("user-{id}"), "bot": bot } })
    }

    /// Returns the second (smaller) page once the request carries an
    /// `after` cursor, the first page otherwise — lets a single mounted
    /// mock exercise the loop's continuation branch deterministically,
    /// without relying on wiremock's cross-mock priority rules.
    struct PagedMembers {
        first_page: Vec<serde_json::Value>,
        second_page: Vec<serde_json::Value>,
    }

    impl Respond for PagedMembers {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let has_after = request.url.query_pairs().any(|(k, _)| k == "after");
            let body = if has_after {
                &self.second_page
            } else {
                &self.first_page
            };
            ResponseTemplate::new(200).set_body_json(body)
        }
    }

    // --- unit: fetch_guild_member_discord_ids_paged / pagination -------

    #[tokio::test]
    async fn fetches_and_filters_bots_across_pages() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/guilds/g1/members"))
            .and(header("Authorization", "Bot test-token"))
            .respond_with(PagedMembers {
                // page size is 2 below; a bot is included to confirm it's filtered out.
                first_page: vec![discord_member("1", false), discord_member("2", true)],
                // shorter than the page size -> loop stops after this page.
                second_page: vec![discord_member("3", false)],
            })
            .mount(&server)
            .await;

        let http = Client::new();
        let ids = fetch_guild_member_discord_ids_paged(&server.uri(), 2, &http, "test-token", "g1")
            .await
            .unwrap();

        assert_eq!(ids, vec!["1".to_string(), "3".to_string()]);
    }

    #[tokio::test]
    async fn propagates_discord_api_errors() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/guilds/g1/members"))
            .respond_with(ResponseTemplate::new(403).set_body_string("missing access"))
            .mount(&server)
            .await;

        let http = Client::new();
        let err = fetch_guild_member_discord_ids(&server.uri(), &http, "test-token", "g1")
            .await
            .unwrap_err();

        assert!(err.to_string().contains("403"));
    }

    // --- integration: get_linked_server_info ----------------------------

    #[tokio::test]
    async fn get_linked_server_info_parses_guild_response() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/guilds/g1"))
            .and(header("Authorization", "Bot test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "g1",
                "name": "Test Friends",
                "icon": "abc123",
                "approximate_member_count": 12
            })))
            .mount(&server)
            .await;

        let http = Client::new();
        let info = get_linked_server_info(&server.uri(), &http, "test-token", "g1")
            .await
            .unwrap();

        assert_eq!(info.id, "g1");
        assert_eq!(info.name, "Test Friends");
        assert_eq!(
            info.icon_url.as_deref(),
            Some("https://cdn.discordapp.com/icons/g1/abc123.png")
        );
        assert_eq!(info.approximate_member_count, Some(12));
    }

    #[tokio::test]
    async fn get_linked_server_info_handles_missing_icon() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/guilds/g1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "g1",
                "name": "No Icon Server",
                "icon": null,
                "approximate_member_count": null
            })))
            .mount(&server)
            .await;

        let http = Client::new();
        let info = get_linked_server_info(&server.uri(), &http, "test-token", "g1")
            .await
            .unwrap();

        assert!(info.icon_url.is_none());
        assert!(info.approximate_member_count.is_none());
    }

    // --- integration (DB): sync_friends / get_friends -------------------

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

    async fn seed_friendship(db: &PgPool, user_id: Uuid, friend_id: Uuid) {
        sqlx::query(
            r#"
            INSERT INTO friendships (id, user_id, friend_id, source, synced_at, created_at)
            VALUES ($1, $2, $3, 'discord_guild', now(), now())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(friend_id)
        .execute(db)
        .await
        .unwrap();
    }

    #[sqlx::test]
    async fn sync_creates_symmetric_friendships_for_shared_guild_members(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let alice = seed_user(&db, "alice-discord", "alice").await;
        // "bob" is in the Discord server but never signed into this app, so
        // there's no `users` row for them — they must be silently skipped,
        // not turned into a friendship with a nonexistent user.

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/guilds/g1/members"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![
                discord_member("me-discord", false),
                discord_member("alice-discord", false),
                discord_member("bob-discord", false),
                discord_member("bot-discord", true),
            ]))
            .mount(&server)
            .await;

        let http = Client::new();
        let result = sync_friends(
            &server.uri(),
            &db,
            &http,
            "test-token",
            "g1",
            me,
            "me-discord",
        )
        .await
        .unwrap();

        assert_eq!(result.synced, 1);
        assert_eq!(result.removed, 0);
        assert_eq!(result.friends.len(), 1);
        assert_eq!(result.friends[0].user_id, alice);

        // Symmetric: alice should see `me` as a friend too, without alice
        // ever running a sync herself.
        let alice_friends = get_friends(&db, alice).await.unwrap();
        assert_eq!(alice_friends.len(), 1);
        assert_eq!(alice_friends[0].user_id, me);
    }

    #[sqlx::test]
    async fn sync_drops_friendships_no_longer_confirmed(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let alice = seed_user(&db, "alice-discord", "alice").await;

        // Friendship left over from a previous sync, both directions.
        seed_friendship(&db, me, alice).await;
        seed_friendship(&db, alice, me).await;

        // This time alice is no longer in the guild's member list.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/guilds/g1/members"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(vec![discord_member("me-discord", false)]),
            )
            .mount(&server)
            .await;

        let http = Client::new();
        let result = sync_friends(
            &server.uri(),
            &db,
            &http,
            "test-token",
            "g1",
            me,
            "me-discord",
        )
        .await
        .unwrap();

        assert_eq!(result.synced, 0);
        assert_eq!(result.removed, 1);
        assert!(get_friends(&db, me).await.unwrap().is_empty());
        assert!(get_friends(&db, alice).await.unwrap().is_empty());
    }

    #[sqlx::test]
    async fn get_friends_orders_by_username_and_builds_avatar_url(db: PgPool) {
        let me = seed_user(&db, "me-discord", "me").await;
        let zed = seed_user(&db, "zed-discord", "zed").await;
        let amy = seed_user(&db, "amy-discord", "amy").await;

        sqlx::query("UPDATE users SET avatar = 'abc123' WHERE id = $1")
            .bind(amy)
            .execute(&db)
            .await
            .unwrap();

        seed_friendship(&db, me, zed).await;
        seed_friendship(&db, me, amy).await;

        let friends = get_friends(&db, me).await.unwrap();

        assert_eq!(friends.len(), 2);
        assert_eq!(friends[0].username, "amy");
        assert_eq!(
            friends[0].avatar_url.as_deref(),
            Some("https://cdn.discordapp.com/avatars/amy-discord/abc123.png")
        );
        assert_eq!(friends[1].username, "zed");
        assert!(friends[1].avatar_url.is_none());
    }
}
