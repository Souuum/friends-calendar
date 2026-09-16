-- Multiple reminders per event ("1 week before" AND "48h before" AND
-- "1h before"), replacing the single calendar_events.reminder_lead_minutes
-- column from 011.
--
-- The shape change is the point: with rows, **"no reminder" is no rows**.
-- 011 needed a 0 sentinel because a NOT NULL column always holds something;
-- absence is the natural representation here, so the sentinel and the
-- `lead_minutes > 0` filters it required both go away.

CREATE TABLE IF NOT EXISTS event_reminders (
    id UUID PRIMARY KEY,
    event_id UUID NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
    -- Minutes before start_time. Strictly positive: a zero or negative lead
    -- would be a reminder at or after the thing it reminds you about.
    lead_minutes INTEGER NOT NULL CHECK (lead_minutes > 0),
    -- Per-reminder marker. The whole reason this is a table: each lead time
    -- has to be sendable and stampable independently of the others.
    sent_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Asking for two reminders at the same offset is a no-op, not an error
    -- worth storing twice; the insert path relies on this for ON CONFLICT.
    UNIQUE (event_id, lead_minutes)
);

CREATE INDEX IF NOT EXISTS idx_event_reminders_pending
    ON event_reminders (event_id)
    WHERE sent_at IS NULL;

-- Carry 011's per-event choice across, preserving whether it had already
-- fired. Events with reminders switched off (the 0 sentinel) correctly
-- produce no rows.
INSERT INTO event_reminders (id, event_id, lead_minutes, sent_at, created_at)
SELECT gen_random_uuid(), id, reminder_lead_minutes, reminder_sent_at, now()
FROM calendar_events
WHERE reminder_lead_minutes > 0
ON CONFLICT (event_id, lead_minutes) DO NOTHING;

-- Dropped rather than left in place: two sources of truth for "when does
-- this event remind people" is exactly the drift this project keeps finding
-- and fixing. The backfill above runs first, in the same migration.
ALTER TABLE calendar_events DROP COLUMN IF EXISTS reminder_lead_minutes;
ALTER TABLE calendar_events DROP COLUMN IF EXISTS reminder_sent_at;

-- 011's partial index lived on those columns and went with them.
DROP INDEX IF EXISTS idx_calendar_events_reminder_pending;
