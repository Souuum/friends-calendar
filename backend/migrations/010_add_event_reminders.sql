-- Event reminders (services::reminders).
--
-- IF NOT EXISTS throughout, matching 004/005: these may already exist on a
-- dev database that ran a branch build before this landed, and re-applying
-- must not fail. See CLAUDE.md's migrations note for the renumbering
-- incident that made this the house style.

-- Idempotency marker for the reminder loop. The loop wakes repeatedly and
-- must not re-send; a timestamp rather than a boolean so it's also a record
-- of when the reminder actually went out.
ALTER TABLE calendar_events
    ADD COLUMN IF NOT EXISTS reminder_sent_at TIMESTAMPTZ;

-- Partial index: the due-events query only ever looks at events that have
-- not been reminded yet, which is a shrinking minority of the table.
CREATE INDEX IF NOT EXISTS idx_calendar_events_reminder_pending
    ON calendar_events (start_time)
    WHERE reminder_sent_at IS NULL;

-- Per-user switch for the in-app reminder, alongside the four toggles from
-- 008. Unlike notify_weekly_digest (which has no per-user delivery and so
-- was removed from the UI), reminders are delivered per user and are
-- recurring by nature, so this one is a real, enforceable preference -
-- services::notifications::create gates on it.
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS notify_event_reminders BOOLEAN NOT NULL DEFAULT true;
