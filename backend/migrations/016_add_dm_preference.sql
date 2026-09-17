-- Nudges can reach people by Discord DM, which is the only delivery that
-- finds someone who doesn't open the app - and also the most intrusive
-- thing this app can do.
--
-- ⚠️ This is the *recipient's* switch, not the sender's. A nudge is already
-- creator-triggered, rate-limited to once a day and sent only to people who
-- were invited and never answered, so the blast radius is small - but "a bot
-- messaged me privately" is a change in kind, not degree, and the person
-- receiving it is the one who should decide.
--
-- Default true, matching every other notify_* column: a preference that
-- starts off makes the feature look broken rather than considerate, and the
-- toggle sits next to the others on /settings.
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS notify_discord_dm BOOLEAN NOT NULL DEFAULT true;
