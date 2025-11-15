use crate::models::{User, DiscordUser};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;
use anyhow::Result;

pub async fn create_or_update_user(
    db: &PgPool,
    discord_user: DiscordUser,
) -> Result<User> {
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (id, discord_id, username, discriminator, avatar, email, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (discord_id) 
        DO UPDATE SET 
            username = EXCLUDED.username,
            discriminator = EXCLUDED.discriminator,
            avatar = EXCLUDED.avatar,
            email = EXCLUDED.email,
            updated_at = EXCLUDED.updated_at
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&discord_user.id)
    .bind(&discord_user.username)
    .bind(&discord_user.discriminator)
    .bind(&discord_user.avatar)
    .bind(&discord_user.email)
    .bind(Utc::now())
    .bind(Utc::now())
    .fetch_one(db)
    .await?;

    Ok(user)
}

pub async fn get_user_by_discord_id(db: &PgPool, discord_id: &str) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE discord_id = $1"
    )
    .bind(discord_id)
    .fetch_optional(db)
    .await?;

    Ok(user)
}