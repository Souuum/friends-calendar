use crate::models::BotChannelConfig;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

const CONFIG_COLUMNS: &str = "guild_id, events_channel_id, announcements_channel_id, reminders_channel_id, digest_enabled, last_digest_sent_at";

pub async fn get_config(db: &PgPool, guild_id: &str) -> Result<Option<BotChannelConfig>> {
    let config = sqlx::query_as::<_, BotChannelConfig>(&format!(
        "SELECT {CONFIG_COLUMNS} FROM discord_bot_config WHERE guild_id = $1"
    ))
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
    digest_enabled: Option<bool>,
) -> Result<BotChannelConfig> {
    let config = sqlx::query_as::<_, BotChannelConfig>(&format!(
        r#"
        INSERT INTO discord_bot_config (guild_id, events_channel_id, announcements_channel_id, reminders_channel_id, digest_enabled, updated_at)
        VALUES ($1, $2, $3, $4, COALESCE($5, false), $6)
        ON CONFLICT (guild_id) DO UPDATE SET
            events_channel_id = EXCLUDED.events_channel_id,
            announcements_channel_id = EXCLUDED.announcements_channel_id,
            reminders_channel_id = EXCLUDED.reminders_channel_id,
            digest_enabled = COALESCE($5, discord_bot_config.digest_enabled),
            updated_at = EXCLUDED.updated_at
        RETURNING {CONFIG_COLUMNS}
        "#
    ))
    .bind(guild_id)
    .bind(&events_channel_id)
    .bind(&announcements_channel_id)
    .bind(&reminders_channel_id)
    .bind(digest_enabled)
    .bind(Utc::now())
    .fetch_one(db)
    .await?;

    Ok(config)
}

/// Stamps `last_digest_sent_at` right after a weekly digest message is
/// successfully posted - see services::digest. Guild must already have a
/// config row (it does, by the time a digest could ever be due: the
/// `digest_enabled` toggle lives on this same row and can only be flipped
/// on through `upsert_config`).
pub async fn mark_digest_sent(db: &PgPool, guild_id: &str, sent_at: DateTime<Utc>) -> Result<()> {
    sqlx::query("UPDATE discord_bot_config SET last_digest_sent_at = $1 WHERE guild_id = $2")
        .bind(sent_at)
        .bind(guild_id)
        .execute(db)
        .await?;

    Ok(())
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
        let first = upsert_config(&db, "g1", Some("111".to_string()), None, None, None)
            .await
            .unwrap();
        assert_eq!(first.events_channel_id.as_deref(), Some("111"));
        assert!(first.announcements_channel_id.is_none());
        assert!(!first.digest_enabled);

        let second = upsert_config(
            &db,
            "g1",
            Some("111".to_string()),
            Some("222".to_string()),
            None,
            None,
        )
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
    async fn upsert_only_changes_digest_enabled_when_explicitly_sent(db: PgPool) {
        let first = upsert_config(&db, "g1", None, None, None, Some(true))
            .await
            .unwrap();
        assert!(first.digest_enabled);

        // A channel-only update (digest_enabled: None) must not silently
        // flip the toggle back off.
        let second = upsert_config(&db, "g1", Some("111".to_string()), None, None, None)
            .await
            .unwrap();
        assert!(second.digest_enabled);
    }

    #[sqlx::test]
    async fn resolve_announcement_channel_prefers_db_config_over_the_fallback(db: PgPool) {
        upsert_config(&db, "g1", None, Some("222".to_string()), None, None)
            .await
            .unwrap();

        let resolved = resolve_announcement_channel_id(&db, "g1", Some(999))
            .await
            .unwrap();
        assert_eq!(resolved, Some(222));
    }

    #[sqlx::test]
    async fn resolve_announcement_channel_falls_back_when_no_db_config_exists(db: PgPool) {
        let resolved = resolve_announcement_channel_id(&db, "g1", Some(999))
            .await
            .unwrap();
        assert_eq!(resolved, Some(999));
    }

    #[sqlx::test]
    async fn resolve_announcement_channel_falls_back_when_db_value_is_unset(db: PgPool) {
        upsert_config(&db, "g1", Some("111".to_string()), None, None, None)
            .await
            .unwrap();

        let resolved = resolve_announcement_channel_id(&db, "g1", Some(999))
            .await
            .unwrap();
        assert_eq!(resolved, Some(999));
    }

    #[sqlx::test]
    async fn mark_digest_sent_stamps_the_row(db: PgPool) {
        upsert_config(&db, "g1", None, None, None, Some(true))
            .await
            .unwrap();

        let sent_at = Utc::now();
        mark_digest_sent(&db, "g1", sent_at).await.unwrap();

        let config = get_config(&db, "g1").await.unwrap().unwrap();
        assert_eq!(
            config.last_digest_sent_at.unwrap().timestamp(),
            sent_at.timestamp()
        );
    }
}
