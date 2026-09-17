//! Posting to Discord **as a person**, not as the bot.
//!
//! ## Why this module exists
//!
//! `POST /api/announcements/:id/reply` was removed on 2026-09-17 because
//! replies went out on the **bot token**, which has two problems that are
//! not cosmetic:
//!
//! - **No attribution.** The thread showed `friends-calendar` saying
//!   whatever a user typed. A reader could not tell who wrote it.
//! - ⚠️ **Mention escalation.** `post_message` sends `{"content": …}` with no
//!   `allowed_mentions`, so a user typing `@everyone` had the *bot* ping the
//!   server using the bot's permissions, not their own. Any app user could
//!   launder a mention through the bot that way.
//!
//! A webhook fixes both: `username`/`avatar_url` are per-message, so the post
//! carries the author's name and face, and `allowed_mentions` is set here
//! rather than left to default.
//!
//! ⚠️ **A webhook is still the app's credential, not the user's.** Discord
//! marks these posts with a BOT tag, and nothing stops someone setting a
//! misleading `username` if this is ever called with unvalidated input - the
//! name always comes from the signed-in user's own record, never from the
//! request body. Real per-user attribution needs OAuth message scopes, which
//! is a different feature.
//!
//! ## Why nothing is stored
//!
//! The webhook is found or created on each post rather than cached in the
//! database. A webhook token is equivalent in power to "post anything in
//! this channel", and keeping one at rest is a secret this app does not
//! otherwise have. One extra API call on an infrequent action is a good
//! trade for not storing it.

use anyhow::{Result, anyhow};
use reqwest::Client;

use crate::services::discord_feed::get_json;

/// How the app's own webhook is recognised among any others in the channel.
const WEBHOOK_NAME: &str = "Friends Calendar";

#[derive(serde::Deserialize)]
struct Webhook {
    id: String,
    token: Option<String>,
    name: Option<String>,
}

/// A webhook you can post through. The id and token are useless apart, so
/// they travel together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookTarget {
    pub id: String,
    pub token: String,
}

/// How a post is attributed. Always built from the signed-in user's own
/// record - see `handlers::announcements::author_of`.
#[derive(Debug, Clone)]
pub struct Author {
    pub name: String,
    pub avatar_url: Option<String>,
}

/// The app's webhook for `channel_id`, creating it if it isn't there.
///
/// ⚠️ Needs `MANAGE_WEBHOOKS`, which is why it was added to the invite
/// permissions in `handlers::guilds`. A server that authorised the bot
/// before that change will 403 here until it is re-invited - the caller
/// turns that into a message saying so rather than a bare failure.
pub async fn ensure_webhook(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    channel_id: &str,
) -> Result<WebhookTarget> {
    let url = format!("{base_url}/channels/{channel_id}/webhooks");
    let existing: Vec<Webhook> = get_json(http, bot_token, &url).await?;

    // Only ours, and only one that still carries a token - Discord omits the
    // token for webhooks the bot didn't create, and one without it is no use.
    if let Some(found) = existing
        .into_iter()
        .find(|w| w.name.as_deref() == Some(WEBHOOK_NAME) && w.token.is_some())
    {
        return Ok(WebhookTarget {
            id: found.id,
            token: found.token.expect("filtered on is_some"),
        });
    }

    let response = http
        .post(&url)
        .header("Authorization", format!("Bot {bot_token}"))
        .json(&serde_json::json!({ "name": WEBHOOK_NAME }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Could not create a webhook ({status}): {body}"));
    }

    let created: Webhook = response.json().await?;
    let token = created
        .token
        .ok_or_else(|| anyhow!("Discord created a webhook without a token"))?;

    Ok(WebhookTarget {
        id: created.id,
        token,
    })
}

/// Posts `content` attributed to `username`.
///
/// `thread_id` posts into a thread on the webhook's channel; `None` posts to
/// the channel itself.
///
/// ⚠️ **`allowed_mentions` is `parse: []`**, which suppresses every ping -
/// `@everyone`, `@here` and role mentions alike. That is the whole point:
/// the text is whatever a user typed, and without this they would be pinging
/// the server with the app's permissions rather than their own. Mentions
/// still *render* as text; they just don't notify.
pub async fn post_as(
    base_url: &str,
    http: &Client,
    webhook: &WebhookTarget,
    thread_id: Option<&str>,
    author: &Author,
    content: &str,
) -> Result<()> {
    let mut url = format!(
        "{base_url}/webhooks/{}/{}?wait=true",
        webhook.id, webhook.token
    );
    if let Some(thread_id) = thread_id {
        url.push_str(&format!("&thread_id={thread_id}"));
    }

    let mut body = serde_json::json!({
        "content": content,
        "username": author.name,
        "allowed_mentions": { "parse": [] },
    });
    if let Some(avatar_url) = &author.avatar_url {
        body["avatar_url"] = serde_json::Value::String(avatar_url.clone());
    }

    let response = http.post(&url).json(&body).send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("Discord webhook error ({status}): {text}"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use wiremock::matchers::{body_json_schema, method, path, query_param};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    #[tokio::test]
    async fn an_existing_webhook_of_ours_is_reused() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/c1/webhooks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": "w1", "token": "t1", "name": "Friends Calendar" }
            ])))
            .mount(&server)
            .await;
        // Creating one would be a POST; none is mounted, so a create attempt
        // fails the call rather than silently passing.

        let found = ensure_webhook(&server.uri(), &Client::new(), "bot", "c1")
            .await
            .unwrap();

        assert_eq!(
            found,
            WebhookTarget {
                id: "w1".into(),
                token: "t1".into()
            }
        );
    }

    // Discord omits the token for webhooks the bot didn't create, and one
    // without a token cannot be posted to - so it must not be adopted.
    #[tokio::test]
    async fn someone_elses_webhook_is_not_adopted() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/c1/webhooks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": "theirs", "token": null, "name": "Some other bot" },
                { "id": "ours-but-tokenless", "token": null, "name": "Friends Calendar" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/channels/c1/webhooks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({ "id": "new", "token": "fresh", "name": "Friends Calendar" }),
            ))
            .mount(&server)
            .await;

        let created = ensure_webhook(&server.uri(), &Client::new(), "bot", "c1")
            .await
            .unwrap();

        assert_eq!(created.id, "new");
    }

    #[tokio::test]
    async fn a_missing_manage_webhooks_permission_propagates() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/channels/c1/webhooks"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let out = ensure_webhook(&server.uri(), &Client::new(), "bot", "c1").await;

        let message = out.unwrap_err().to_string();
        // The handler keys off this to say "re-invite the bot".
        assert!(message.contains("403"), "got: {message}");
    }

    /// ⚠️ The reason this whole module exists. A reply posted on the bot
    /// token with no `allowed_mentions` let any user ping @everyone using
    /// the *bot's* permissions.
    #[tokio::test]
    async fn every_post_suppresses_mentions() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/webhooks/w1/t1"))
            .and(body_json_schema::<Value>)
            .respond_with(|req: &Request| {
                let body: Value = serde_json::from_slice(&req.body).unwrap();
                assert_eq!(
                    body["allowed_mentions"]["parse"],
                    json!([]),
                    "mentions must be suppressed"
                );
                ResponseTemplate::new(200)
            })
            .expect(1)
            .mount(&server)
            .await;

        post_as(
            &server.uri(),
            &Client::new(),
            &WebhookTarget {
                id: "w1".into(),
                token: "t1".into(),
            },
            None,
            &Author {
                name: "Soum".into(),
                avatar_url: None,
            },
            "@everyone come to the thing",
        )
        .await
        .unwrap();

        drop(server);
    }

    #[tokio::test]
    async fn the_author_is_carried_on_the_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/webhooks/w1/t1"))
            .respond_with(|req: &Request| {
                let body: Value = serde_json::from_slice(&req.body).unwrap();
                assert_eq!(body["username"], "Soum");
                assert_eq!(body["avatar_url"], "https://cdn/a.png");
                ResponseTemplate::new(200)
            })
            .expect(1)
            .mount(&server)
            .await;

        post_as(
            &server.uri(),
            &Client::new(),
            &WebhookTarget {
                id: "w1".into(),
                token: "t1".into(),
            },
            None,
            &Author {
                name: "Soum".into(),
                avatar_url: Some("https://cdn/a.png".into()),
            },
            "hello",
        )
        .await
        .unwrap();

        drop(server);
    }

    #[tokio::test]
    async fn a_reply_is_addressed_to_its_thread() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/webhooks/w1/t1"))
            .and(query_param("thread_id", "th1"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        post_as(
            &server.uri(),
            &Client::new(),
            &WebhookTarget {
                id: "w1".into(),
                token: "t1".into(),
            },
            Some("th1"),
            &Author {
                name: "Soum".into(),
                avatar_url: None,
            },
            "in the thread",
        )
        .await
        .unwrap();

        drop(server);
    }
}
