-- Importing a real calendar so availability reflects someone's whole life,
-- not just this app. See .claude/skills/calendar-import-availability/SKILL.md.

CREATE TABLE IF NOT EXISTS external_calendars (
    id              UUID PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- 'ics' today. OAuth providers ('google', 'microsoft') slot in beside it
    -- without a schema change.
    provider        VARCHAR(20) NOT NULL,
    -- ⚠️ For 'ics' this is a secret URL granting read access to somebody's
    -- entire calendar. It is the one credential of its kind in this database:
    -- we cannot scope it and cannot revoke it from our side, only delete it.
    -- Disconnecting deletes the row, and the cascade below takes the cached
    -- intervals with it.
    credential      TEXT NOT NULL,
    label           TEXT,
    last_synced_at  TIMESTAMPTZ,
    -- Shown in the UI. A silently dead connection is worse than no
    -- connection at all, because availability looks right and isn't.
    last_error      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Per URL, not per provider: work and personal calendars are both 'ics'
    -- and somebody may well connect both.
    UNIQUE (user_id, credential)
);

-- ⚠️ Intervals only. No title, no description, no attendees, no location.
--
-- Importing a work calendar into a *social* app is a privacy problem the
-- moment you keep titles - "Interview with...", "Oncology appointment".
-- Availability only ever needs (start, end), so everything else is discarded
-- at the boundary and never reaches this table.
CREATE TABLE IF NOT EXISTS external_busy (
    id           UUID PRIMARY KEY,
    calendar_id  UUID NOT NULL REFERENCES external_calendars(id) ON DELETE CASCADE,
    starts_at    TIMESTAMPTZ NOT NULL,
    ends_at      TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_external_busy_window
    ON external_busy (calendar_id, starts_at, ends_at);

CREATE INDEX IF NOT EXISTS idx_external_calendars_user
    ON external_calendars (user_id);
