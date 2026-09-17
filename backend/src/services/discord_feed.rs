use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{AnnouncementPostInfo, AnnouncementPostRow, ReplyInfo, User};

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
    /// Optional so existing message fixtures that only carry
    /// `message_count` still parse. When absent we fall back to the message
    /// id, which is what Discord uses for a thread started from a message.
    #[serde(default)]
    id: Option<String>,
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
        "SELECT discord_message_id FROM event_publications WHERE discord_message_id = $1",
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
/// Resolves the thread hanging off an announcement message, creating one if
/// Discord doesn't have it yet. Returns the thread id.
///
/// A thread started *from a message* shares that message's id, so the happy
/// path is usually "the id you already have" - but only once a thread
/// exists. `GET`ting the message first tells us which case we're in without
/// relying on create-returns-409 behaviour.
///
/// reqwest rather than serenity on purpose: everything in this module takes
/// `base_url` as a parameter so it can be pointed at a wiremock server. See
/// .claude/skills/add-tests/SKILL.md.
pub async fn fetch_or_create_thread(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    channel_id: &str,
    message_id: &str,
    thread_name: &str,
) -> Result<String> {
    let message: DiscordMessage = get_json(
        http,
        bot_token,
        &format!("{base_url}/channels/{channel_id}/messages/{message_id}"),
    )
    .await?;

    if let Some(thread) = message.thread {
        // A thread started from a message is addressed by that message's id,
        // so the fallback is exact rather than a guess.
        return Ok(thread.id.unwrap_or_else(|| message_id.to_string()));
    }

    // Discord caps thread names at 100 chars; truncate on a char boundary,
    // not a byte one, or a multi-byte character straddling the cut panics.
    let name: String = thread_name.chars().take(90).collect();
    let name = if name.trim().is_empty() {
        "Thread".to_string()
    } else {
        name
    };

    let url = format!("{base_url}/channels/{channel_id}/messages/{message_id}/threads");
    let response = http
        .post(&url)
        .header("Authorization", format!("Bot {bot_token}"))
        .json(&serde_json::json!({ "name": name }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Discord API error ({status}): {body}"));
    }

    let created: DiscordThread = response.json().await?;
    Ok(created.id.unwrap_or_else(|| message_id.to_string()))
}

/// The replies in a thread, oldest first.
///
/// Deliberately a live fetch rather than a read of `announcement_posts` -
/// `reply_count` there is whatever the last sync saw, and a reply posted a
/// second ago wouldn't be in it.
pub async fn fetch_replies(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    thread_id: &str,
) -> Result<Vec<ReplyInfo>> {
    let messages: Vec<DiscordMessage> = get_json(
        http,
        bot_token,
        &format!("{base_url}/channels/{thread_id}/messages?limit=100"),
    )
    .await?;

    let mut replies: Vec<ReplyInfo> = messages
        .into_iter()
        .filter(|m| !m.content.trim().is_empty())
        .map(|m| ReplyInfo {
            author_username: m.author.username,
            author_avatar_url: User::build_avatar_url(&m.author.id, &m.author.avatar),
            body: m.content,
            posted_at: m.timestamp,
        })
        .collect();

    // Discord returns newest-first; a thread reads oldest-first.
    replies.reverse();
    Ok(replies)
}

pub(crate) async fn get_json<T: serde::de::DeserializeOwned>(
    http: &Client,
    bot_token: &str,
    url: &str,
) -> Result<T> {
    let response = http
        .get(url)
        .header("Authorization", format!("Bot {bot_token}"))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Discord API error ({status}): {body}"));
    }

    Ok(response.json().await?)
}

/// Posts a message and returns Discord's id for it.
///
/// The id matters for announcements: it's stored on the event, and it's also
/// how the event's thread is later addressed (a thread started from a
/// message shares that message's id).
pub async fn post_message(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    channel_id: &str,
    content: &str,
) -> Result<String> {
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

    #[derive(serde::Deserialize)]
    struct Posted {
        id: String,
    }
    let posted: Posted = response.json().await?;
    Ok(posted.id)
}

/// Fire-and-forget variant for callers that don't need the message id.
pub async fn send_channel_message(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    channel_id: &str,
    content: &str,
) -> Result<()> {
    post_message(base_url, http, bot_token, channel_id, content).await?;
    Ok(())
}

/// Adds a reaction as the bot, so people can RSVP by clicking it (see
/// bot.rs, which turns that click back into a participant row).
pub async fn add_reaction(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    channel_id: &str,
    message_id: &str,
    emoji: &str,
) -> Result<()> {
    // The emoji goes in the path, so it has to be percent-encoded - "✅"
    // unescaped would be an invalid URL.
    let encoded: String = url_encode(emoji);
    let url =
        format!("{base_url}/channels/{channel_id}/messages/{message_id}/reactions/{encoded}/@me");

    let response = http
        .put(&url)
        .header("Authorization", format!("Bot {bot_token}"))
        .header("Content-Length", "0")
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Discord API error ({status}): {body}"));
    }

    Ok(())
}

/// Minimal percent-encoding for a path segment. Enough for emoji, which is
/// all this is used for - not a general-purpose URL encoder.
pub(crate) fn url_encode(s: &str) -> String {
    s.bytes().map(|b| format!("%{b:02X}")).collect()
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
        // message id - should tag as "event", not "general". The message id
        // lives on the publication now, not the event.
        let event_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO calendar_events
                (id, creator_id, title, start_time, end_time, visibility, created_at, updated_at)
            VALUES ($1, $2, 'Board Game Night', now(), now() + interval '1 hour', 'friends', now(), now())
            "#,
        )
        .bind(event_id)
        .bind(creator_id)
        .execute(&db)
        .await
        .unwrap();

        let guild = crate::services::guilds::ensure_guild(&db, "test-guild")
            .await
            .unwrap();
        let publication = crate::services::guilds::add_publication(&db, event_id, guild, "chan1")
            .await
            .unwrap();
        crate::services::guilds::mark_published(&db, publication, "msg-event")
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

    #[tokio::test]
    async fn fetch_or_create_thread_reuses_an_existing_thread() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages/msg1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg1",
                "author": { "id": "1", "username": "alice", "avatar": null },
                "content": "hello",
                "timestamp": "2026-03-01T12:00:00Z",
                "thread": { "id": "thread-9", "message_count": 3 }
            })))
            .mount(&server)
            .await;

        let thread = fetch_or_create_thread(
            &server.uri(),
            &Client::new(),
            "token",
            "chan1",
            "msg1",
            "Ski trip",
        )
        .await
        .unwrap();

        // No POST mock is registered - creating one would 404 here, which is
        // the point: an existing thread must not be recreated.
        assert_eq!(thread, "thread-9");
    }

    #[tokio::test]
    async fn fetch_or_create_thread_falls_back_to_the_message_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages/msg1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg1",
                "author": { "id": "1", "username": "alice", "avatar": null },
                "content": "hello",
                "timestamp": "2026-03-01T12:00:00Z",
                // A thread whose payload we only partially parse - Discord
                // addresses a message-thread by the message id anyway.
                "thread": { "message_count": 3 }
            })))
            .mount(&server)
            .await;

        let thread = fetch_or_create_thread(
            &server.uri(),
            &Client::new(),
            "token",
            "chan1",
            "msg1",
            "Ski trip",
        )
        .await
        .unwrap();

        assert_eq!(thread, "msg1");
    }

    #[tokio::test]
    async fn fetch_or_create_thread_creates_one_when_the_message_has_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages/msg1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg1",
                "author": { "id": "1", "username": "alice", "avatar": null },
                "content": "hello",
                "timestamp": "2026-03-01T12:00:00Z"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/channels/chan1/messages/msg1/threads"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": "new-thread" })))
            .mount(&server)
            .await;

        let thread = fetch_or_create_thread(
            &server.uri(),
            &Client::new(),
            "token",
            "chan1",
            "msg1",
            "Ski trip",
        )
        .await
        .unwrap();

        assert_eq!(thread, "new-thread");
    }

    // Discord caps thread names at 100 chars. Truncating by bytes would
    // panic on a multi-byte character straddling the cut.
    #[tokio::test]
    async fn fetch_or_create_thread_truncates_a_long_multibyte_name() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages/msg1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg1",
                "author": { "id": "1", "username": "alice", "avatar": null },
                "content": "hello",
                "timestamp": "2026-03-01T12:00:00Z"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/channels/chan1/messages/msg1/threads"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": "t" })))
            .mount(&server)
            .await;

        let name = "é".repeat(200);
        let thread = fetch_or_create_thread(
            &server.uri(),
            &Client::new(),
            "token",
            "chan1",
            "msg1",
            &name,
        )
        .await
        .unwrap();
        assert_eq!(thread, "t");
    }

    #[tokio::test]
    async fn fetch_replies_returns_oldest_first_and_skips_empty_messages() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/thread-9/messages"))
            // Discord returns newest-first.
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {
                    "id": "m3",
                    "author": { "id": "3", "username": "carol", "avatar": null },
                    "content": "last",
                    "timestamp": "2026-03-01T14:00:00Z"
                },
                {
                    "id": "m2",
                    "author": { "id": "2", "username": "bob", "avatar": null },
                    "content": "   ",
                    "timestamp": "2026-03-01T13:00:00Z"
                },
                {
                    "id": "m1",
                    "author": { "id": "1", "username": "alice", "avatar": null },
                    "content": "first",
                    "timestamp": "2026-03-01T12:00:00Z"
                }
            ])))
            .mount(&server)
            .await;

        let replies = fetch_replies(&server.uri(), &Client::new(), "token", "thread-9")
            .await
            .unwrap();

        assert_eq!(
            replies.iter().map(|r| r.body.as_str()).collect::<Vec<_>>(),
            vec!["first", "last"],
            "a thread reads oldest-first, and an attachment-only message has no body to show"
        );
    }
}
