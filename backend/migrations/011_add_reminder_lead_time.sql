-- Per-event reminder lead time, chosen by the creator (1 week / 48h / 1h /
-- ...) instead of the fixed one-hour constant services::reminders shipped
-- with in 010.
--
-- NOT NULL DEFAULT 60 rather than nullable, so existing events keep exactly
-- the behaviour they had. **0 means "no reminder"** - it isn't a sentinel
-- bolted on, it falls out of the rule itself: a reminder fires when
-- `start_time > now AND start_time <= now + lead`, and with a lead of 0
-- those two can never both hold. One rule, no special case.
ALTER TABLE calendar_events
    ADD COLUMN IF NOT EXISTS reminder_lead_minutes INTEGER NOT NULL DEFAULT 60;

-- Postgres has no ADD CONSTRAINT IF NOT EXISTS, so drop-then-add to keep
-- this re-runnable like the rest of the file.
ALTER TABLE calendar_events
    DROP CONSTRAINT IF EXISTS reminder_lead_minutes_non_negative;
ALTER TABLE calendar_events
    ADD CONSTRAINT reminder_lead_minutes_non_negative
    CHECK (reminder_lead_minutes >= 0);

-- 010's partial index assumed every pending event was a candidate. With a
-- per-event lead, events with reminders switched off never are, so keep
-- them out of the index the reminder pass scans.
DROP INDEX IF EXISTS idx_calendar_events_reminder_pending;
CREATE INDEX IF NOT EXISTS idx_calendar_events_reminder_pending
    ON calendar_events (start_time)
    WHERE reminder_sent_at IS NULL AND reminder_lead_minutes > 0;
