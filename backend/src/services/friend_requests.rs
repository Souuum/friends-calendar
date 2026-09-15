use crate::models::{FriendRequestInfo, User};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

pub enum SendRequestOutcome {
    Sent,
    /// The recipient had already sent *us* a pending request - rather than
    /// creating a redundant second row, this accepts theirs. Mutual intent
    /// either way, no reason to make the user do two separate actions.
    AutoAccepted,
    UserNotFound,
    CannotRequestSelf,
    AlreadyFriends,
    AlreadyPending,
}

pub async fn send_request(
    db: &PgPool,
    from_user_id: Uuid,
    to_username: &str,
) -> Result<SendRequestOutcome> {
    let to_user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE username = $1")
        .bind(to_username)
        .fetch_optional(db)
        .await?;
    let Some(to_user) = to_user else {
        return Ok(SendRequestOutcome::UserNotFound);
    };

    if to_user.id == from_user_id {
        return Ok(SendRequestOutcome::CannotRequestSelf);
    }

    let already_friends: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM friendships WHERE user_id = $1 AND friend_id = $2)",
    )
    .bind(from_user_id)
    .bind(to_user.id)
    .fetch_one(db)
    .await?;
    if already_friends {
        return Ok(SendRequestOutcome::AlreadyFriends);
    }

    let reverse_pending: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM friend_requests WHERE from_user_id = $1 AND to_user_id = $2 AND status = 'pending'",
    )
    .bind(to_user.id)
    .bind(from_user_id)
    .fetch_optional(db)
    .await?;
    if let Some(reverse_id) = reverse_pending {
        respond(db, reverse_id, from_user_id, true).await?;
        return Ok(SendRequestOutcome::AutoAccepted);
    }

    let existing: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, status FROM friend_requests WHERE from_user_id = $1 AND to_user_id = $2",
    )
    .bind(from_user_id)
    .bind(to_user.id)
    .fetch_optional(db)
    .await?;

    match existing {
        Some((_, status)) if status == "pending" => return Ok(SendRequestOutcome::AlreadyPending),
        Some((id, _)) => {
            // Previously declined (or some other terminal state) - reset it
            // rather than erroring on the UNIQUE(from_user_id, to_user_id)
            // constraint. Being declined once shouldn't permanently block
            // ever asking again.
            sqlx::query("UPDATE friend_requests SET status = 'pending', created_at = $1, responded_at = NULL WHERE id = $2")
                .bind(Utc::now())
                .bind(id)
                .execute(db)
                .await?;
        }
        None => {
            sqlx::query(
                "INSERT INTO friend_requests (id, from_user_id, to_user_id, status, created_at) VALUES ($1, $2, $3, 'pending', $4)",
            )
            .bind(Uuid::new_v4())
            .bind(from_user_id)
            .bind(to_user.id)
            .bind(Utc::now())
            .execute(db)
            .await?;
        }
    }

    if let Some(from_username) =
        sqlx::query_scalar::<_, Option<String>>("SELECT username FROM users WHERE id = $1")
            .bind(from_user_id)
            .fetch_one(db)
            .await?
    {
        let message = format!("{from_username} sent you a friend request");
        if let Err(e) = crate::services::notifications::create(
            db,
            to_user.id,
            "friend_request",
            Some(from_user_id),
            None,
            &message,
        )
        .await
        {
            tracing::warn!("Failed to create friend-request notification: {:?}", e);
        }
    }

    Ok(SendRequestOutcome::Sent)
}

pub async fn list_incoming(db: &PgPool, user_id: Uuid) -> Result<Vec<FriendRequestInfo>> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<String>, DateTime<Utc>)>(
        r#"
        SELECT fr.id, u.id, u.username, u.discord_id, u.avatar, fr.created_at
        FROM friend_requests fr
        JOIN users u ON u.id = fr.from_user_id
        WHERE fr.to_user_id = $1 AND fr.status = 'pending'
        ORDER BY fr.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, from_user_id, username, discord_id, avatar, created_at)| FriendRequestInfo {
                id,
                from_user_id,
                from_username: username,
                from_avatar_url: User::build_avatar_url(&discord_id, &avatar),
                created_at,
            },
        )
        .collect())
}

pub enum RespondOutcome {
    Responded,
    /// No pending request with this id addressed to this user - covers
    /// both "doesn't exist" and "not yours" without telling a caller which
    /// (same NotFound-not-Forbidden choice made elsewhere in this API).
    NotFound,
}

pub async fn respond(
    db: &PgPool,
    request_id: Uuid,
    responder_id: Uuid,
    accept: bool,
) -> Result<RespondOutcome> {
    let new_status = if accept { "accepted" } else { "declined" };

    let from_user_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        UPDATE friend_requests
        SET status = $1, responded_at = $2
        WHERE id = $3 AND to_user_id = $4 AND status = 'pending'
        RETURNING from_user_id
        "#,
    )
    .bind(new_status)
    .bind(Utc::now())
    .bind(request_id)
    .bind(responder_id)
    .fetch_optional(db)
    .await?;

    let Some(from_user_id) = from_user_id else {
        return Ok(RespondOutcome::NotFound);
    };

    if accept {
        let now = Utc::now();
        for (a, b) in [(responder_id, from_user_id), (from_user_id, responder_id)] {
            sqlx::query(
                r#"
                INSERT INTO friendships (id, user_id, friend_id, source, synced_at, created_at)
                VALUES ($1, $2, $3, 'friend_request', $4, $4)
                ON CONFLICT (user_id, friend_id) DO UPDATE SET synced_at = EXCLUDED.synced_at
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(a)
            .bind(b)
            .bind(now)
            .execute(db)
            .await?;
        }

        if let Some(responder_username) =
            sqlx::query_scalar::<_, Option<String>>("SELECT username FROM users WHERE id = $1")
                .bind(responder_id)
                .fetch_one(db)
                .await?
        {
            let message = format!("{responder_username} accepted your friend request");
            if let Err(e) = crate::services::notifications::create(
                db,
                from_user_id,
                "friend_accepted",
                Some(responder_id),
                None,
                &message,
            )
            .await
            {
                tracing::warn!("Failed to create friend-accepted notification: {:?}", e);
            }
        }
    }

    Ok(RespondOutcome::Responded)
}

/// How many members of the linked Discord guild don't have an account on
/// this app yet - the "N members aren't on Friends Calendar" prompt.
pub async fn count_guild_members_without_accounts(
    db: &PgPool,
    base_url: &str,
    http: &Client,
    bot_token: &str,
    guild_id: &str,
) -> Result<usize> {
    let member_discord_ids = crate::services::friends::fetch_guild_member_discord_ids(
        base_url, http, bot_token, guild_id,
    )
    .await?;

    let existing: Vec<String> =
        sqlx::query_scalar("SELECT discord_id FROM users WHERE discord_id = ANY($1)")
            .bind(&member_discord_ids)
            .fetch_all(db)
            .await?;
    let existing: std::collections::HashSet<String> = existing.into_iter().collect();

    Ok(member_discord_ids
        .into_iter()
        .filter(|id| !existing.contains(id))
        .count())
}

/// Posts a simple text prompt into the configured channel encouraging
/// non-member guild members to sign up. Uses the same base_url-as-parameter
/// pattern as services::friends (mockable, no live Discord call in tests) -
/// deliberately doesn't reuse services::discord_announcement's
/// DiscordAnnouncer, which wraps serenity's own Http client rather than a
/// plain reqwest::Client and so isn't mockable the same way.
pub async fn post_guild_invite_prompt(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    channel_id: &str,
    missing_count: usize,
) -> Result<()> {
    let plural = if missing_count == 1 { "" } else { "s" };
    let content = format!(
        "👋 {missing_count} member{plural} of this server aren't on Friends Calendar yet. Sign in with Discord to join!"
    );

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
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
    async fn send_request_creates_a_pending_row_and_notifies_the_recipient(db: PgPool) {
        let alice = seed_user(&db, "alice-discord", "alice").await;
        let bob = seed_user(&db, "bob-discord", "bob").await;

        let outcome = send_request(&db, alice, "bob").await.unwrap();
        assert!(matches!(outcome, SendRequestOutcome::Sent));

        let incoming = list_incoming(&db, bob).await.unwrap();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].from_username, "alice");

        let bob_notifications = crate::services::notifications::list(&db, bob, 10)
            .await
            .unwrap();
        assert_eq!(bob_notifications.len(), 1);
        assert_eq!(bob_notifications[0].kind, "friend_request");
    }

    #[sqlx::test]
    async fn send_request_rejects_self_unknown_user_and_existing_friends(db: PgPool) {
        let alice = seed_user(&db, "alice-discord", "alice").await;
        let bob = seed_user(&db, "bob-discord", "bob").await;

        assert!(matches!(
            send_request(&db, alice, "alice").await.unwrap(),
            SendRequestOutcome::CannotRequestSelf
        ));
        assert!(matches!(
            send_request(&db, alice, "nobody").await.unwrap(),
            SendRequestOutcome::UserNotFound
        ));

        sqlx::query(
            r#"
            INSERT INTO friendships (id, user_id, friend_id, source, synced_at, created_at)
            VALUES ($1, $2, $3, 'discord_guild', now(), now())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(alice)
        .bind(bob)
        .execute(&db)
        .await
        .unwrap();

        assert!(matches!(
            send_request(&db, alice, "bob").await.unwrap(),
            SendRequestOutcome::AlreadyFriends
        ));
    }

    #[sqlx::test]
    async fn send_request_rejects_duplicate_pending_but_allows_resend_after_decline(db: PgPool) {
        let alice = seed_user(&db, "alice-discord", "alice").await;
        let bob = seed_user(&db, "bob-discord", "bob").await;

        send_request(&db, alice, "bob").await.unwrap();
        assert!(matches!(
            send_request(&db, alice, "bob").await.unwrap(),
            SendRequestOutcome::AlreadyPending
        ));

        let request_id = list_incoming(&db, bob).await.unwrap()[0].id;
        respond(&db, request_id, bob, false).await.unwrap();

        // Declined once shouldn't permanently block asking again.
        assert!(matches!(
            send_request(&db, alice, "bob").await.unwrap(),
            SendRequestOutcome::Sent
        ));
        assert_eq!(list_incoming(&db, bob).await.unwrap().len(), 1);
    }

    #[sqlx::test]
    async fn mutual_requests_auto_accept_instead_of_duplicating(db: PgPool) {
        let alice = seed_user(&db, "alice-discord", "alice").await;
        let bob = seed_user(&db, "bob-discord", "bob").await;

        send_request(&db, alice, "bob").await.unwrap();
        let outcome = send_request(&db, bob, "alice").await.unwrap();

        assert!(matches!(outcome, SendRequestOutcome::AutoAccepted));
        assert!(list_incoming(&db, alice).await.unwrap().is_empty());
        assert!(list_incoming(&db, bob).await.unwrap().is_empty());

        let alice_friends = crate::services::friends::get_friends(&db, alice)
            .await
            .unwrap();
        assert_eq!(alice_friends.len(), 1);
        assert_eq!(alice_friends[0].username, "bob");
    }

    #[sqlx::test]
    async fn accepting_creates_symmetric_friendship_and_notifies_the_sender(db: PgPool) {
        let alice = seed_user(&db, "alice-discord", "alice").await;
        let bob = seed_user(&db, "bob-discord", "bob").await;

        send_request(&db, alice, "bob").await.unwrap();
        let request_id = list_incoming(&db, bob).await.unwrap()[0].id;

        let outcome = respond(&db, request_id, bob, true).await.unwrap();
        assert!(matches!(outcome, RespondOutcome::Responded));

        assert_eq!(
            crate::services::friends::get_friends(&db, alice)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            crate::services::friends::get_friends(&db, bob)
                .await
                .unwrap()
                .len(),
            1
        );

        let alice_notifications = crate::services::notifications::list(&db, alice, 10)
            .await
            .unwrap();
        assert_eq!(alice_notifications.len(), 1);
        assert_eq!(alice_notifications[0].kind, "friend_accepted");
    }

    #[sqlx::test]
    async fn declining_does_not_create_a_friendship(db: PgPool) {
        let alice = seed_user(&db, "alice-discord", "alice").await;
        let bob = seed_user(&db, "bob-discord", "bob").await;

        send_request(&db, alice, "bob").await.unwrap();
        let request_id = list_incoming(&db, bob).await.unwrap()[0].id;

        respond(&db, request_id, bob, false).await.unwrap();

        assert!(
            crate::services::friends::get_friends(&db, alice)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test]
    async fn only_the_recipient_can_respond(db: PgPool) {
        let alice = seed_user(&db, "alice-discord", "alice").await;
        let bob = seed_user(&db, "bob-discord", "bob").await;
        let mallory = seed_user(&db, "mallory-discord", "mallory").await;

        send_request(&db, alice, "bob").await.unwrap();
        let request_id = list_incoming(&db, bob).await.unwrap()[0].id;

        let outcome = respond(&db, request_id, mallory, true).await.unwrap();
        assert!(matches!(outcome, RespondOutcome::NotFound));
        assert!(
            crate::services::friends::get_friends(&db, alice)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test]
    async fn counts_guild_members_without_accounts(db: PgPool) {
        seed_user(&db, "known-discord", "known").await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/guilds/g1/members"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "user": { "id": "known-discord", "username": "known", "bot": false } },
                { "user": { "id": "unknown-1", "username": "u1", "bot": false } },
                { "user": { "id": "unknown-2", "username": "u2", "bot": false } },
                { "user": { "id": "bot-discord", "username": "bot", "bot": true } },
            ])))
            .mount(&server)
            .await;

        let http = Client::new();
        let count =
            count_guild_members_without_accounts(&db, &server.uri(), &http, "test-token", "g1")
                .await
                .unwrap();

        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn posts_the_invite_prompt_with_the_right_count() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/channels/chan1/messages"))
            .and(header("Authorization", "Bot test-token"))
            .and(body_json(json!({ "content": "👋 3 members of this server aren't on Friends Calendar yet. Sign in with Discord to join!" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "msg1" })))
            .mount(&server)
            .await;

        let http = Client::new();
        post_guild_invite_prompt(&server.uri(), &http, "test-token", "chan1", 3)
            .await
            .unwrap();
    }
}
