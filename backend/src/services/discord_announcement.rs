use crate::models::CalendarEvent;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serenity::builder::{CreateMessage, CreateThread};
use serenity::http::Http;
use serenity::model::channel::AutoArchiveDuration;
use serenity::model::id::ChannelId;

pub struct DiscordAnnouncer {
    http: Http,
    channel_id: ChannelId,
}

impl DiscordAnnouncer {
    pub fn new(bot_token: String, channel_id: u64) -> Self {
        Self {
            http: Http::new(&bot_token),
            channel_id: ChannelId::new(channel_id),
        }
    }

    pub async fn announce_event(&self, event: &CalendarEvent) -> Result<String> {
        let message = format_event_message(event);

        // Send the announcement message
        let sent_message = self
            .channel_id
            .send_message(&self.http, CreateMessage::new().content(message))
            .await?;

        // Add ✅ reaction automatically
        sent_message.react(&self.http, '✅').await?;

        // Create a thread from the message
        let thread_name = if event.title.len() > 100 {
            format!("{}...", &event.title[..97]) // Discord thread names max 100 chars
        } else {
            event.title.clone()
        };

        // Use the channel to create a thread from the message
        match self
            .channel_id
            .create_thread_from_message(
                &self.http,
                sent_message.id,
                CreateThread::new(thread_name.clone())
                    .auto_archive_duration(AutoArchiveDuration::OneDay),
            )
            .await
        {
            Ok(thread) => {
                tracing::info!(
                    "🧵 Created thread '{}' (ID: {}) for event {}",
                    thread_name,
                    thread.id,
                    event.id
                );

                // Send a welcome message in the thread
                if let Err(e) = thread
                    .id
                    .send_message(
                        &self.http,
                        CreateMessage::new()
                            .content("💬 Discutez ici de l'organisation de cette activité !"),
                    )
                    .await
                {
                    tracing::warn!("⚠️  Failed to send welcome message in thread: {:?}", e);
                }
            }
            Err(e) => {
                tracing::warn!("⚠️  Failed to create thread: {:?}", e);
            }
        }

        tracing::info!("📢 Announced event {} to Discord", event.id);

        Ok(sent_message.id.get().to_string())
    }
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
