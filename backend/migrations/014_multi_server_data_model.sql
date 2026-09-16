-- Step 1 of .claude/skills/multi-server/SKILL.md: the data model that makes
-- "one event, N announcements" representable. No user-visible behaviour
-- changes here - this only moves where the Discord posting facts live.

-- Servers the bot is in. Identity only: per-guild *settings* already live in
-- discord_bot_config, which has been keyed by guild_id since 008 and so is
-- already multi-guild-shaped. Duplicating announcements_channel_id onto this
-- table would create exactly the second source of truth this project keeps
-- removing.
CREATE TABLE IF NOT EXISTS guilds (
    id UUID PRIMARY KEY,
    discord_guild_id VARCHAR(64) NOT NULL UNIQUE,
    -- Nullable: nothing stores the display name today (services::friends
    -- fetches it live from Discord), so there's nothing to backfill. Filled
    -- on the next sync.
    name VARCHAR(255),
    icon VARCHAR(255),
    added_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Which servers a user belongs to. Genuinely new information: membership is
-- currently *derived* into friendships by services::friends and then thrown
-- away, so nothing can answer "which servers can this person publish to?".
CREATE TABLE IF NOT EXISTS user_guilds (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    guild_id UUID NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, guild_id)
);

-- One row per place an event was (or will be) announced.
CREATE TABLE IF NOT EXISTS event_publications (
    id UUID PRIMARY KEY,
    event_id UUID NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
    guild_id UUID NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    channel_id VARCHAR(64) NOT NULL,
    -- NULL until the announcement actually posts: the row records the
    -- *intent* (this event goes to this server), the id records the result.
    -- A Discord failure then loses nothing and can be retried, instead of
    -- silently dropping a server from the selection.
    discord_message_id VARCHAR(64),
    posted_at TIMESTAMPTZ,
    UNIQUE (event_id, guild_id)
);

-- bot.rs looks an event up by the message someone reacted to, on every
-- reaction in a watched channel.
CREATE INDEX IF NOT EXISTS idx_event_publications_message
    ON event_publications (discord_message_id);

-- The one guild this app has had until now. discord_bot_config is the only
-- place the database records it - DISCORD_GUILD_ID is an env var, which a
-- migration can't read.
INSERT INTO guilds (id, discord_guild_id)
SELECT gen_random_uuid(), guild_id FROM discord_bot_config
ON CONFLICT (discord_guild_id) DO NOTHING;

-- Approximate, and deliberately so: anyone with a guild-sourced friendship
-- was a member of the guild at the last sync, which is the best the database
-- knows. services::friends corrects it on the next sync.
INSERT INTO user_guilds (user_id, guild_id)
SELECT DISTINCT f.user_id, g.id
FROM friendships f
CROSS JOIN (SELECT id FROM guilds ORDER BY added_at LIMIT 1) g
WHERE f.source = 'discord_guild'
ON CONFLICT DO NOTHING;

-- Every event already announced becomes one publication. LIMIT 1 is safe
-- because exactly one guild exists at this point - and if that stops being
-- true, the guard below refuses rather than guessing which server an event
-- was posted to.
INSERT INTO event_publications (id, event_id, guild_id, channel_id, discord_message_id, posted_at)
SELECT gen_random_uuid(), e.id, g.id, e.discord_channel_id, e.discord_message_id, e.created_at
FROM calendar_events e
CROSS JOIN (SELECT id FROM guilds ORDER BY added_at LIMIT 1) g
WHERE e.discord_message_id IS NOT NULL
  AND e.discord_channel_id IS NOT NULL
ON CONFLICT (event_id, guild_id) DO NOTHING;

-- Refuse to drop the columns if anything couldn't be carried across.
--
-- Losing a discord_message_id is silent and nasty: RSVP-by-reaction stops
-- resolving (bot.rs looks events up by it) and reminders lose the thread
-- they post into. A failed migration is loud and fixable; silently dropped
-- ids are neither. This fires if no guild row could be created - e.g. a
-- deployment that announced events via the DISCORD_GUILD_ID env var without
-- ever saving channel config on /server.
DO $$
DECLARE orphaned INTEGER;
BEGIN
    SELECT count(*) INTO orphaned
    FROM calendar_events e
    WHERE e.discord_message_id IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM event_publications p WHERE p.event_id = e.id);

    IF orphaned > 0 THEN
        RAISE EXCEPTION
            'refusing to drop discord_message_id: % announced event(s) have no publication row. Insert a guilds row for this deployment''s DISCORD_GUILD_ID and re-run.', orphaned;
    END IF;
END $$;

ALTER TABLE calendar_events DROP COLUMN IF EXISTS discord_message_id;
ALTER TABLE calendar_events DROP COLUMN IF EXISTS discord_channel_id;
