use serenity::async_trait;
use serenity::model::channel::Reaction;
use serenity::model::gateway::Ready;
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::builder::CreateMessage;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct DiscordBot {
    db: Arc<PgPool>,
    announcement_channel_id: u64,
}

impl DiscordBot {
    pub fn new(db: Arc<PgPool>, announcement_channel_id: u64) -> Self {
        Self {
            db,
            announcement_channel_id,
        }
    }

    pub async fn start(bot_token: String, db: PgPool, announcement_channel_id: u64) -> Result<(), serenity::Error> {
        let intents = GatewayIntents::GUILD_MESSAGE_REACTIONS 
            | GatewayIntents::GUILDS
            | GatewayIntents::GUILD_MEMBERS;

        let handler = Handler {
            db: Arc::new(db),
            announcement_channel_id,
        };

        let mut client = Client::builder(&bot_token, intents)
            .event_handler(handler)
            .await?;

        tracing::info!("🤖 Discord bot starting...");
        client.start().await?;

        Ok(())
    }
}

struct Handler {
    db: Arc<PgPool>,
    announcement_channel_id: u64,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        tracing::info!("🤖 Discord bot {} is connected!", ready.user.name);
        tracing::info!("📢 Monitoring channel ID: {}", self.announcement_channel_id);
    }

    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        // Only process reactions in the announcement channel
        if reaction.channel_id.get() != self.announcement_channel_id {
            return;
        }

        // Only process white check mark emoji
        if !Self::is_check_mark(&reaction.emoji) {
            return;
        }

        tracing::info!(
            "✅ User {} reacted with check mark to message {} in announcement channel",
            reaction.user_id.map(|id| id.get()).unwrap_or(0),
            reaction.message_id.get()
        );

        if let Err(e) = self.handle_event_reaction(&ctx, &reaction).await {
            tracing::error!("❌ Failed to handle reaction: {:?}", e);
        }
    }

    async fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        // Only process reactions in the announcement channel
        if reaction.channel_id.get() != self.announcement_channel_id {
            return;
        }

        // Only process white check mark emoji
        if !Self::is_check_mark(&reaction.emoji) {
            return;
        }

        tracing::info!(
            "❌ User {} removed check mark from message {}",
            reaction.user_id.map(|id| id.get()).unwrap_or(0),
            reaction.message_id.get()
        );

        if let Err(e) = self.handle_reaction_remove(&ctx, &reaction).await {
            tracing::error!("❌ Failed to handle reaction removal: {:?}", e);
        }
    }
}

impl Handler {
    fn is_check_mark(emoji: &ReactionType) -> bool {
        match emoji {
            ReactionType::Unicode(s) => s == "✅" || s == "☑️",
            _ => false,
        }
    }

    async fn handle_event_reaction(&self, ctx: &Context, reaction: &Reaction) -> anyhow::Result<()> {
        let message_id = reaction.message_id.get().to_string();
        let user_id = reaction.user_id.ok_or_else(|| anyhow::anyhow!("No user ID"))?;

        // Find event linked to this message
        let event_result = sqlx::query!(
            r#"
            SELECT id, creator_id 
            FROM calendar_events 
            WHERE discord_message_id = $1
            "#,
            message_id
        )
        .fetch_optional(&*self.db)
        .await?;

        let Some(event) = event_result else {
            tracing::warn!("⚠️  No event found for message {}", message_id);
            return Ok(());
        };

        // Get Discord user info
        let discord_user = user_id.to_user(&ctx.http).await?;
        let discord_id = discord_user.id.get().to_string();

        // Find or create user in our database
        let user_db_id = self.get_or_create_user(&discord_id, &discord_user.name).await?;

        // Don't add creator again (they're already in)
        if user_db_id == event.creator_id {
            tracing::info!("👤 User is event creator, already a participant");
            return Ok(());
        }

        // Add user as participant with "accepted" status
        let result = sqlx::query!(
            r#"
            INSERT INTO event_participants (id, event_id, user_id, status, invited_at, responded_at)
            VALUES ($1, $2, $3, 'accepted', NOW(), NOW())
            ON CONFLICT (event_id, user_id) 
            DO UPDATE SET status = 'accepted', responded_at = NOW()
            "#,
            Uuid::new_v4(),
            event.id,
            user_db_id
        )
        .execute(&*self.db)
        .await?;

        if result.rows_affected() > 0 {
            tracing::info!("✅ Added user {} as participant to event {}", discord_user.name, event.id);
        }

        Ok(())
    }

    async fn handle_reaction_remove(&self, _ctx: &Context, reaction: &Reaction) -> anyhow::Result<()> {
        let message_id = reaction.message_id.get().to_string();
        let user_id = reaction.user_id.ok_or_else(|| anyhow::anyhow!("No user ID"))?;
        let discord_id = user_id.get().to_string();

        // Find event
        let event_result = sqlx::query!(
            r#"
            SELECT id FROM calendar_events 
            WHERE discord_message_id = $1
            "#,
            message_id
        )
        .fetch_optional(&*self.db)
        .await?;

        let Some(event) = event_result else {
            return Ok(());
        };

        // Find user in database
        let user_result = sqlx::query!(
            r#"SELECT id FROM users WHERE discord_id = $1"#,
            discord_id
        )
        .fetch_optional(&*self.db)
        .await?;

        let Some(user) = user_result else {
            return Ok(());
        };

        // Update participation to declined (or remove)
        sqlx::query!(
            r#"
            UPDATE event_participants
            SET status = 'declined', responded_at = NOW()
            WHERE event_id = $1 AND user_id = $2
            "#,
            event.id,
            user.id
        )
        .execute(&*self.db)
        .await?;

        tracing::info!("❌ User declined participation for event {}", event.id);

        Ok(())
    }

    async fn get_or_create_user(&self, discord_id: &str, username: &str) -> anyhow::Result<Uuid> {
        // Try to find existing user
        let existing = sqlx::query!(
            r#"SELECT id FROM users WHERE discord_id = $1"#,
            discord_id
        )
        .fetch_optional(&*self.db)
        .await?;

        if let Some(user) = existing {
            return Ok(user.id);
        }

        // Create new user
        let user_id = Uuid::new_v4();
        sqlx::query!(
            r#"
            INSERT INTO users (id, discord_id, username, discriminator, created_at, updated_at)
            VALUES ($1, $2, $3, '0', NOW(), NOW())
            "#,
            user_id,
            discord_id,
            username
        )
        .execute(&*self.db)
        .await?;

        tracing::info!("👤 Created new user: {} ({})", username, discord_id);
        Ok(user_id)
    }
}