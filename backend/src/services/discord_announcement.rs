use crate::models::CalendarEvent;
use crate::services::discord_feed;
use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;

/// Announces an event: posts the message, adds the ✅ people RSVP with, and
/// opens a thread for discussion. Returns the message id, which is stored on
/// the event and later doubles as the thread id.
///
/// reqwest rather than serenity, matching discord_feed and digest: it takes
/// `base_url` as a parameter, so the whole flow is wiremock-testable. This
/// was the last Discord call in the codebase that wasn't.
///
/// The thread and its welcome message are best-effort. Failing to open a
/// thread shouldn't fail the announcement - the event is posted and people
/// can still react; they just don't get a discussion thread. Reminders
/// handle the resulting "message with no thread" case explicitly.
pub async fn announce_event(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    channel_id: &str,
    event: &CalendarEvent,
) -> Result<String> {
    let message_id = discord_feed::post_message(
        base_url,
        http,
        bot_token,
        channel_id,
        &format_event_message(event),
    )
    .await?;

    if let Err(e) =
        discord_feed::add_reaction(base_url, http, bot_token, channel_id, &message_id, "✅").await
    {
        // Without the reaction people can't RSVP from Discord, but the
        // announcement itself is out - worth a warning, not a failure.
        tracing::warn!("⚠️  Failed to add the ✅ reaction: {:?}", e);
    }

    match discord_feed::fetch_or_create_thread(
        base_url,
        http,
        bot_token,
        channel_id,
        &message_id,
        &event.title,
    )
    .await
    {
        Ok(thread_id) => {
            if let Err(e) = discord_feed::send_channel_message(
                base_url,
                http,
                bot_token,
                &thread_id,
                "💬 Discutez ici de l'organisation de cette activité !",
            )
            .await
            {
                tracing::warn!("⚠️  Failed to send welcome message in thread: {:?}", e);
            }
        }
        Err(e) => tracing::warn!("⚠️  Failed to create thread: {:?}", e),
    }

    tracing::info!("📢 Announced event {} to Discord", event.id);
    Ok(message_id)
}

/// The exact text posted to Discord when an event is announced.
///
/// A free function rather than a method so `POST /api/events/announcement-preview`
/// can render the real thing without a bot token or a live announcer. That
/// endpoint exists specifically so the preview shown while creating an event
/// cannot drift from what actually gets posted - if you change this, the
/// preview changes with it.
pub fn format_event_message(event: &CalendarEvent) -> String {
    let start = format_datetime(&event.start_time);

    let mut message = String::from("@everyone\n__Proposition d'activité :__\n\n");

    message.push_str(&format!("**Date :** {}\n", start));

    message.push_str(&format!("**Activité :** {}\n", event.title));

    if let Some(location) = &event.location {
        message.push_str(&format!("**Lieu :** {}\n", location));
    } else {
        message.push_str("**Lieu :** Non spécifié\n");
    }

    if let Some(price) = &event.price {
        message.push_str(&format!("**Prix :** {}\n", price));
    } else {
        message.push_str("**Prix :** À définir\n");
    }

    if let Some(link) = &event.link {
        let link_name = if let Some(desc) = &event.description {
            desc.clone()
        } else {
            "Informations".to_string()
        };
        message.push_str(&format!("**Lien :** [{}]({})\n", link_name, link));
    }

    message.push_str("\n**Réagissez avec ✅ pour participer !**");

    message
}

/// Discord's own timestamp markup: the client renders it in each reader's
/// local timezone, so nothing here needs to know about `users.timezone`.
fn format_datetime(dt: &DateTime<Utc>) -> String {
    // Discord timestamp format for local time
    let timestamp = dt.timestamp();
    format!("<t:{}:F>", timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Visibility;
    use chrono::Duration;
    use serde_json::json;
    use uuid::Uuid;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn event(title: &str) -> CalendarEvent {
        let now = Utc::now();
        CalendarEvent {
            id: Uuid::new_v4(),
            creator_id: Uuid::new_v4(),
            title: title.to_string(),
            description: None,
            start_time: now + Duration::days(2),
            end_time: now + Duration::days(2) + Duration::hours(2),
            location: Some("Chez Lina".to_string()),
            visibility: Visibility::Friends,
            created_at: now,
            updated_at: now,
            discord_message_id: None,
            discord_channel_id: None,
            price: Some("15".to_string()),
            link: None,
        }
    }

    async fn mock_full_announce_flow() -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/channels/chan1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "msg-1" })))
            .mount(&server)
            .await;
        // The emoji is percent-encoded into the path.
        Mock::given(method("PUT"))
            .and(path(
                "/channels/chan1/messages/msg-1/reactions/%E2%9C%85/@me",
            ))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages/msg-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg-1",
                "author": { "id": "1", "username": "bot", "avatar": null },
                "content": "announced",
                "timestamp": "2026-03-01T12:00:00Z"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/channels/chan1/messages/msg-1/threads"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": "thread-1" })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/channels/thread-1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "welcome" })))
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn announcing_posts_reacts_and_opens_a_thread() {
        let server = mock_full_announce_flow().await;

        let message_id = announce_event(
            &server.uri(),
            &Client::new(),
            "token",
            "chan1",
            &event("Raclette"),
        )
        .await
        .unwrap();

        // The returned id is what gets stored on the event - and what the
        // thread is later addressed by.
        assert_eq!(message_id, "msg-1");
    }

    // The announcement is the part that matters; the extras are decoration.
    #[tokio::test]
    async fn a_failed_thread_does_not_fail_the_announcement() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/channels/chan1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "msg-1" })))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path(
                "/channels/chan1/messages/msg-1/reactions/%E2%9C%85/@me",
            ))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        // Thread lookup 500s, and no thread-creation mock exists either.
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages/msg-1"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let message_id = announce_event(
            &server.uri(),
            &Client::new(),
            "token",
            "chan1",
            &event("Raclette"),
        )
        .await
        .unwrap();

        assert_eq!(message_id, "msg-1");
    }

    #[tokio::test]
    async fn a_failed_reaction_does_not_fail_the_announcement() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/channels/chan1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "msg-1" })))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path(
                "/channels/chan1/messages/msg-1/reactions/%E2%9C%85/@me",
            ))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/channels/chan1/messages/msg-1"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        assert!(
            announce_event(
                &server.uri(),
                &Client::new(),
                "token",
                "chan1",
                &event("Raclette")
            )
            .await
            .is_ok()
        );
    }

    // If the post itself fails there's nothing to store, so this one does
    // propagate.
    #[tokio::test]
    async fn a_failed_post_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/channels/chan1/messages"))
            .respond_with(ResponseTemplate::new(500).set_body_string("nope"))
            .mount(&server)
            .await;

        assert!(
            announce_event(
                &server.uri(),
                &Client::new(),
                "token",
                "chan1",
                &event("Raclette")
            )
            .await
            .is_err()
        );
    }
}
