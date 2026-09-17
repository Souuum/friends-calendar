-- Subscribable .ics feed. See .claude/skills/calendar-export-ics/SKILL.md.
--
-- ⚠️ This token authenticates on the URL itself, because a calendar app is
-- handed a URL and GETs it unattended - it cannot send an Authorization
-- header. So it is a bearer credential that people paste into Google, into
-- a phone, sometimes into a chat. It has to be revocable on its own
-- (regenerate the link) without touching the account.
--
-- Nullable on purpose: minted the first time somebody asks for their link,
-- so nobody who never uses the feature has a live credential sitting here.
ALTER TABLE users ADD COLUMN IF NOT EXISTS calendar_feed_token TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_calendar_feed_token
    ON users (calendar_feed_token)
    WHERE calendar_feed_token IS NOT NULL;
