use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{AnnouncementPostInfo, AnnouncementPostRow};

// Discord's own per-request cap on GET /channels/{id}/messages.
const MESSAGES_PAGE_LIMIT: usize = 50;

#[derive(Debug, Deserialize)]
struct DiscordMessageAuthor {
    id: String,
    username: String,
    avatar: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscordReaction {
    count: i32,
}

#[derive(Debug, Deserialize)]
struct DiscordThread {
    #[serde(default)]
    message_count: i32,
}

#[derive(Debug, Deserialize)]
struct DiscordMessage {
    id: String,
    author: DiscordMessageAuthor,
    content: String,
    timestamp: DateTime<Utc>,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    reactions: Vec<DiscordReaction>,
    thread: Option<DiscordThread>,
}

/// Discord messages don't have a title field. If the content has more than
/// one line, treat the first line as a title and the rest as the body -
/// otherwise there's no title, just body text. See
/// .claude/skills/mockup-announcements-feed/SKILL.md's open questions: this
/// is the "simplest honest option" rather than inventing a title.
fn split_title(content: &str) -> (Option<String>, String) {
    match content.split_once('\n') {
        Some((first_line, rest)) if !first_line.trim().is_empty() => {
            (Some(first_line.trim().to_string()), rest.trim().to_string())
        }
        _ => (None, content.trim().to_string()),
    }
}

/// A post tags as "event" when it's the exact message
/// `services::calendar::create_event`'s auto-announce created (matched by
/// `discord_message_id` against `calendar_events`) - there's no Discord-side
/// signal for what a "tag" even is, so "General" is the honest default for
/// everything else. See the skill's open questions for why "Poll" was
/// dropped entirely rather than guessed at.
async fn infer_tag(db: &PgPool, discord_message_id: &str) -> Result<&'static str> {
    let matched: Option<String> = sqlx::query_scalar(
        "SELECT discord_message_id FROM calendar_events WHERE discord_message_id = $1",
    )
    .bind(discord_message_id)
    .fetch_optional(db)
    .await?;

    Ok(if matched.is_some() {
        "event"
    } else {
        "general"
    })
}

/// Fetch the most recent messages in `channel_id` and upsert them into
/// `announcement_posts`. Mirrors `services::friends::sync_friends`'s
/// fetch-then-upsert pattern (base_url as a parameter so this is mockable
/// with wiremock - see .claude/skills/add-tests/SKILL.md). Unlike friend
/// sync, there's no "drop stale rows" pass: Discord messages aren't
/// un-synced, only edited (re-syncing picks up the new content via
/// `ON CONFLICT`) or deleted (left as a harmless stale row rather than
/// spending a second API call per sync just to detect deletions).
pub async fn sync_channel(
    base_url: &str,
    db: &PgPool,
    http: &Client,
    bot_token: &str,
    channel_id: &str,
) -> Result<usize> {
    let url = format!("{base_url}/channels/{channel_id}/messages?limit={MESSAGES_PAGE_LIMIT}");

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

    let messages: Vec<DiscordMessage> = response.json().await?;
    let now = Utc::now();
    let mut synced = 0;

    for message in &messages {
        // Nothing to show in a text feed for an empty-content message
        // (e.g. embed-only or system messages).
        if message.content.trim().is_empty() {
            continue;
        }

        let (title, body) = split_title(&message.content);
        let reaction_count: i32 = message.reactions.iter().map(|r| r.count).sum();
        let reply_count = message
            .thread
            .as_ref()
            .map(|t| t.message_count)
            .unwrap_or(0);
        let tag = infer_tag(db, &message.id).await?;

        sqlx::query(
            r#"
            INSERT INTO announcement_posts
                (id, discord_message_id, channel_id, author_discord_id, author_username,
                 author_avatar, title, body, tag, reaction_count, reply_count, pinned,
                 posted_at, synced_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (discord_message_id) DO UPDATE SET
                author_username = EXCLUDED.author_username,
                author_avatar = EXCLUDED.author_avatar,
                title = EXCLUDED.title,
                body = EXCLUDED.body,
                tag = EXCLUDED.tag,
                reaction_count = EXCLUDED.reaction_count,
                reply_count = EXCLUDED.reply_count,
                pinned = EXCLUDED.pinned,
                synced_at = EXCLUDED.synced_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(&message.id)
        .bind(channel_id)
        .bind(&message.author.id)
        .bind(&message.author.username)
        .bind(&message.author.avatar)
        .bind(&title)
        .bind(&body)
        .bind(tag)
        .bind(reaction_count)
        .bind(reply_count)
        .bind(message.pinned)
        .bind(message.timestamp)
        .bind(now)
        .execute(db)
        .await?;

        synced += 1;
    }

    Ok(synced)
}

/// Pinned posts first, newest first within each group.
pub async fn list_posts(db: &PgPool, channel_id: &str) -> Result<Vec<AnnouncementPostInfo>> {
    let rows = sqlx::query_as::<_, AnnouncementPostRow>(
        r#"
        SELECT id, author_discord_id, author_username,
               author_avatar, title, body, tag, reaction_count, reply_count, pinned, posted_at
        FROM announcement_posts
        WHERE channel_id = $1
        ORDER BY pinned DESC, posted_at DESC
        "#,
    )
    .bind(channel_id)
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(AnnouncementPostInfo::from).collect())
}

/// How many posts landed in `channel_id` since `since` - feeds the weekly
/// digest message (services::digest).
pub async fn count_posts_since(db: &PgPool, channel_id: &str, since: DateTime<Utc>) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM announcement_posts WHERE channel_id = $1 AND posted_at >= $2",
    )
    .bind(channel_id)
    .bind(since)
    .fetch_one(db)
    .await?;

    Ok(count)
}

/// Posts a plain text message to a channel via the bot token - shared by
/// the digest job (services::digest) so it doesn't need its own HTTP
/// plumbing or a second way of talking to Discord.
pub async fn send_channel_message(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    channel_id: &str,
    content: &str,
) -> Result<()> {
    let url = format!("{base_url}/channels/{channel_id}/messages");

    let response = http
        .post(&url)
        .header("Authorization", format!("Bot {bot_token}"))
        .json(&serde_json::json!({ "content": content }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Discord API error ({status}): {body}"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // --- unit: split_title -----------------------------------------------

    #[test]
    fn split_title_uses_first_line_when_multiline() {
        let (title, body) = split_title("Game night!\nBring snacks, starts at 7pm.");
        assert_eq!(title.as_deref(), Some("Game night!"));
        assert_eq!(body, "Bring snacks, starts at 7pm.");
    }

    #[test]
    fn split_title_has_no_title_for_single_line_content() {
        let (title, body) = split_title("Just a one-liner");
        assert!(title.is_none());
        assert_eq!(body, "Just a one-liner");
    }

    // --- integration: sync_channel ----------------------------------------

    fn discord_message(id: &str, content: &str, pinned: bool) -> serde_json::Value {
        json!({
            "id": id,
            "author": { "id": "author-1", "username": "alice", "avatar": null },
            "content": content,
            "timestamp": "2026-03-01T12:00:00Z",
            "pinned": pinned,
            "reactions": [{ "count": 3 }, { "count": 2 }],
            "thread": { "message_count": 5 }
        })
    }

    async fn seed_user(db: &PgPool, discord_id: &str, username: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, discord_id, username, created_at, updated_at) VALUES ($1, $2, $3, now(), now())")
            .bind(id)
            .bind(discord_id)
            .bind(username)
            .execute(db)
            .await
            .unwrap();
        id
    }

    #[sqlx::test]
    async fn sync_channel_upserts_messages_and_infers_the_event_tag(db: PgPool) {
        let creator_id = seed_user(&db, "author-1", "alice").await;

        // A calendar event already announced with this exact Discord
        // message id - should tag as "event", not "general".
        sqlx::query(
            r#"
            INSERT INTO calendar_events
                (id, creator_id, title, start_time, end_time, visibility, created_at, updated_at, discord_message_id)
            VALUES ($1, $2, 'Board Game Night', now(), now() + interval '1 hour', 'friends', now(), now(), 'msg-event')
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(creator_id)
        .execute(&db)
        .await
        .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages"))
            .and(header("Authorization", "Bot test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![
                discord_message("msg-event", "Board Game Night\nBring your own dice.", false),
                discord_message("msg-general", "Server maintenance this weekend.", true),
            ]))
            .mount(&server)
            .await;

        let http = Client::new();
        let synced = sync_channel(&server.uri(), &db, &http, "test-token", "chan1")
            .await
            .unwrap();
        assert_eq!(synced, 2);

        let posts = list_posts(&db, "chan1").await.unwrap();
        assert_eq!(posts.len(), 2);

        // Pinned first regardless of posted_at.
        assert_eq!(posts[0].title, None);
        assert_eq!(posts[0].body, "Server maintenance this weekend.");
        assert_eq!(posts[0].tag, "general");
        assert!(posts[0].pinned);

        let event_post = posts.iter().find(|p| p.tag == "event").unwrap();
        assert_eq!(event_post.title.as_deref(), Some("Board Game Night"));
        assert_eq!(event_post.body, "Bring your own dice.");
        assert_eq!(event_post.reaction_count, 5);
        assert_eq!(event_post.reply_count, 5);
        assert_eq!(event_post.author_username, "alice");
    }

    #[sqlx::test]
    async fn sync_channel_is_idempotent_and_picks_up_edits(db: PgPool) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(vec![discord_message(
                    "msg-1",
                    "Original content",
                    false,
                )]),
            )
            .mount(&server)
            .await;

        let http = Client::new();
        sync_channel(&server.uri(), &db, &http, "test-token", "chan1")
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM announcement_posts")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 1);

        // Re-sync with edited content for the same message id - should
        // update in place, not insert a duplicate row.
        server.reset().await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(vec![discord_message(
                    "msg-1",
                    "Edited content",
                    false,
                )]),
            )
            .mount(&server)
            .await;

        sync_channel(&server.uri(), &db, &http, "test-token", "chan1")
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM announcement_posts")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 1);

        let posts = list_posts(&db, "chan1").await.unwrap();
        assert_eq!(posts[0].body, "Edited content");
    }

    #[sqlx::test]
    async fn sync_channel_skips_empty_content_messages(db: PgPool) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(vec![discord_message("msg-1", "   ", false)]),
            )
            .mount(&server)
            .await;

        let http = Client::new();
        let synced = sync_channel(&server.uri(), &db, &http, "test-token", "chan1")
            .await
            .unwrap();
        assert_eq!(synced, 0);
    }

    #[sqlx::test]
    async fn sync_channel_propagates_discord_api_errors(db: PgPool) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages"))
            .respond_with(ResponseTemplate::new(403).set_body_string("missing access"))
            .mount(&server)
            .await;

        let http = Client::new();
        let err = sync_channel(&server.uri(), &db, &http, "test-token", "chan1")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("403"));
    }

    // --- integration (DB): count_posts_since -------------------------------

    #[sqlx::test]
    async fn count_posts_since_only_counts_posts_on_or_after_the_cutoff(db: PgPool) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(vec![discord_message(
                    "msg-1",
                    "Recent post",
                    false,
                )]),
            )
            .mount(&server)
            .await;

        let http = Client::new();
        sync_channel(&server.uri(), &db, &http, "test-token", "chan1")
            .await
            .unwrap();

        let since_before = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .into();
        let since_after = DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z")
            .unwrap()
            .into();

        assert_eq!(
            count_posts_since(&db, "chan1", since_before).await.unwrap(),
            1
        );
        assert_eq!(
            count_posts_since(&db, "chan1", since_after).await.unwrap(),
            0
        );
    }

    // --- integration: send_channel_message ---------------------------------

    #[tokio::test]
    async fn send_channel_message_posts_to_discord() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/channels/chan1/messages"))
            .and(header("Authorization", "Bot test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "sent-1" })))
            .mount(&server)
            .await;

        let http = Client::new();
        send_channel_message(&server.uri(), &http, "test-token", "chan1", "hello")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn send_channel_message_propagates_discord_api_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/channels/chan1/messages"))
            .respond_with(ResponseTemplate::new(403).set_body_string("missing access"))
            .mount(&server)
            .await;

        let http = Client::new();
        let err = send_channel_message(&server.uri(), &http, "test-token", "chan1", "hello")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("403"));
    }
}
