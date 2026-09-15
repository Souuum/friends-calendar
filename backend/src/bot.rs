use serenity::async_trait;
use serenity::model::channel::Reaction;
use serenity::model::gateway::Ready;
use serenity::model::prelude::*;
use serenity::prelude::*;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct DiscordBot;

impl DiscordBot {
    pub async fn start(
        bot_token: String,
        db: PgPool,
        announcement_channel_id: u64,
    ) -> Result<(), serenity::Error> {
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

    // Uses runtime-checked sqlx::query/query_as (not the query! macro) to
    // match the rest of the codebase, and deliberately so: query! needs a
    // live, schema-matching DATABASE_URL at *compile* time, which would
    // make `cargo build` fail on a fresh clone/CI without a pre-seeded DB.
    async fn handle_event_reaction(
        &self,
        ctx: &Context,
        reaction: &Reaction,
    ) -> anyhow::Result<()> {
        let message_id = reaction.message_id.get().to_string();
        let user_id = reaction
            .user_id
            .ok_or_else(|| anyhow::anyhow!("No user ID"))?;

        let event: Option<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT id, creator_id FROM calendar_events WHERE discord_message_id = $1",
        )
        .bind(&message_id)
        .fetch_optional(&*self.db)
        .await?;

        let Some((event_id, creator_id)) = event else {
            tracing::warn!("⚠️  No event found for message {}", message_id);
            return Ok(());
        };

        let discord_user = user_id.to_user(&ctx.http).await?;
        let discord_id = discord_user.id.get().to_string();

        let user_db_id = self
            .get_or_create_user(&discord_id, &discord_user.name)
            .await?;

        let result = sqlx::query(
            r#"
            INSERT INTO event_participants (id, event_id, user_id, status, invited_at, responded_at)
            VALUES ($1, $2, $3, 'accepted', NOW(), NOW())
            ON CONFLICT (event_id, user_id)
            DO UPDATE SET status = 'accepted', responded_at = NOW()
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(event_id)
        .bind(user_db_id)
        .execute(&*self.db)
        .await?;

        if result.rows_affected() > 0 {
            if user_db_id == creator_id {
                tracing::info!(
                    "✅ Creator {} rejoined event {}",
                    discord_user.name,
                    event_id
                );
            } else {
                tracing::info!(
                    "✅ Added user {} as participant to event {}",
                    discord_user.name,
                    event_id
                );
            }
        }

        Ok(())
    }

    async fn handle_reaction_remove(
        &self,
        _ctx: &Context,
        reaction: &Reaction,
    ) -> anyhow::Result<()> {
        let message_id = reaction.message_id.get().to_string();
        let user_id = reaction
            .user_id
            .ok_or_else(|| anyhow::anyhow!("No user ID"))?;
        let discord_id = user_id.get().to_string();

        let event_id: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM calendar_events WHERE discord_message_id = $1")
                .bind(&message_id)
                .fetch_optional(&*self.db)
                .await?;

        let Some(event_id) = event_id else {
            return Ok(());
        };

        let user_db_id: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM users WHERE discord_id = $1")
                .bind(&discord_id)
                .fetch_optional(&*self.db)
                .await?;

        let Some(user_db_id) = user_db_id else {
            return Ok(());
        };

        sqlx::query(
            r#"
            UPDATE event_participants
            SET status = 'declined', responded_at = NOW()
            WHERE event_id = $1 AND user_id = $2
            "#,
        )
        .bind(event_id)
        .bind(user_db_id)
        .execute(&*self.db)
        .await?;

        tracing::info!("❌ User declined participation for event {}", event_id);

        Ok(())
    }

    async fn get_or_create_user(&self, discord_id: &str, username: &str) -> anyhow::Result<Uuid> {
        let existing: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM users WHERE discord_id = $1")
                .bind(discord_id)
                .fetch_optional(&*self.db)
                .await?;

        if let Some(id) = existing {
            return Ok(id);
        }

        let user_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (id, discord_id, username, discriminator, created_at, updated_at)
            VALUES ($1, $2, $3, '0', NOW(), NOW())
            "#,
        )
        .bind(user_id)
        .bind(discord_id)
        .bind(username)
        .execute(&*self.db)
        .await?;

        tracing::info!("👤 Created new user: {} ({})", username, discord_id);
        Ok(user_id)
    }
}
