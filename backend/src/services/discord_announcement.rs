use serenity::http::Http;
use serenity::model::id::ChannelId;
use serenity::builder::CreateMessage;
use anyhow::Result;
use crate::models::CalendarEvent;
use chrono::{DateTime, Utc};

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
        let message = self.format_event_message(event);

        let sent_message = self
            .channel_id
            .send_message(&self.http, CreateMessage::new().content(message))
            .await?;

        // Add ✅ reaction automatically
        sent_message
            .react(&self.http, '✅')
            .await?;

        tracing::info!("📢 Announced event {} to Discord", event.id);

        Ok(sent_message.id.get().to_string())
    }
fn format_event_message(&self, event: &CalendarEvent) -> String {
    let start = Self::format_datetime(&event.start_time);

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

    fn format_datetime(dt: &DateTime<Utc>) -> String {
        // Discord timestamp format for local time
        let timestamp = dt.timestamp();
        format!("<t:{}:F>", timestamp)
    }
}