use crate::models::BotChannelConfig;
use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;

pub async fn get_config(db: &PgPool, guild_id: &str) -> Result<Option<BotChannelConfig>> {
    let config = sqlx::query_as::<_, BotChannelConfig>(
        "SELECT guild_id, events_channel_id, announcements_channel_id, reminders_channel_id FROM discord_bot_config WHERE guild_id = $1",
    )
    .bind(guild_id)
    .fetch_optional(db)
    .await?;

    Ok(config)
}

pub async fn upsert_config(
    db: &PgPool,
    guild_id: &str,
    events_channel_id: Option<String>,
    announcements_channel_id: Option<String>,
    reminders_channel_id: Option<String>,
) -> Result<BotChannelConfig> {
    let config = sqlx::query_as::<_, BotChannelConfig>(
        r#"
        INSERT INTO discord_bot_config (guild_id, events_channel_id, announcements_channel_id, reminders_channel_id, updated_at)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (guild_id) DO UPDATE SET
            events_channel_id = EXCLUDED.events_channel_id,
            announcements_channel_id = EXCLUDED.announcements_channel_id,
            reminders_channel_id = EXCLUDED.reminders_channel_id,
            updated_at = EXCLUDED.updated_at
        RETURNING guild_id, events_channel_id, announcements_channel_id, reminders_channel_id
        "#,
    )
    .bind(guild_id)
    .bind(&events_channel_id)
    .bind(&announcements_channel_id)
    .bind(&reminders_channel_id)
    .bind(Utc::now())
    .fetch_one(db)
    .await?;

    Ok(config)
}

/// Which channel new event announcements should post to. DB config wins if
/// present (live-editable from the /server page - a change here takes
/// effect on the very next event created, since this is a plain per-request
/// read, no caching); falls back to the env-var-configured
/// `AppState.discord_announcement_channel_id` if there's no DB row yet, so
/// existing deployments keep working unchanged until someone visits
/// /server. Deliberately does NOT cover the gateway bot's reaction-watching
/// channel (bot.rs captures that once at process startup) - see CLAUDE.md.
pub async fn resolve_announcement_channel_id(
    db: &PgPool,
    guild_id: &str,
    fallback: Option<u64>,
) -> Result<Option<u64>> {
    if let Some(config) = get_config(db, guild_id).await?
        && let Some(channel_id) = config.announcements_channel_id
        && let Ok(parsed) = channel_id.parse()
    {
        return Ok(Some(parsed));
    }

    Ok(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn get_config_is_none_before_anything_is_set(db: PgPool) {
        assert!(get_config(&db, "g1").await.unwrap().is_none());
    }

    #[sqlx::test]
    async fn upsert_creates_then_updates_the_same_row(db: PgPool) {
        let first = upsert_config(&db, "g1", Some("111".to_string()), None, None).await.unwrap();
        assert_eq!(first.events_channel_id.as_deref(), Some("111"));
        assert!(first.announcements_channel_id.is_none());

        let second = upsert_config(&db, "g1", Some("111".to_string()), Some("222".to_string()), None)
            .await
            .unwrap();
        assert_eq!(second.announcements_channel_id.as_deref(), Some("222"));

        // still one row, not two
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM discord_bot_config")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[sqlx::test]
    async fn resolve_announcement_channel_prefers_db_config_over_the_fallback(db: PgPool) {
        upsert_config(&db, "g1", None, Some("222".to_string()), None).await.unwrap();

        let resolved = resolve_announcement_channel_id(&db, "g1", Some(999)).await.unwrap();
        assert_eq!(resolved, Some(222));
    }

    #[sqlx::test]
    async fn resolve_announcement_channel_falls_back_when_no_db_config_exists(db: PgPool) {
        let resolved = resolve_announcement_channel_id(&db, "g1", Some(999)).await.unwrap();
        assert_eq!(resolved, Some(999));
    }

    #[sqlx::test]
    async fn resolve_announcement_channel_falls_back_when_db_value_is_unset(db: PgPool) {
        upsert_config(&db, "g1", Some("111".to_string()), None, None).await.unwrap();

        let resolved = resolve_announcement_channel_id(&db, "g1", Some(999)).await.unwrap();
        assert_eq!(resolved, Some(999));
    }
}
