CREATE TYPE visibility AS ENUM ('private', 'friends', 'public');
CREATE TYPE participation_status AS ENUM ('pending', 'accepted', 'declined', 'maybe');

CREATE TABLE IF NOT EXISTS calendar_events (
    id UUID PRIMARY KEY,
    creator_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    location VARCHAR(255),
    visibility visibility NOT NULL DEFAULT 'private',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT valid_time_range CHECK (end_time > start_time)
);

CREATE TABLE IF NOT EXISTS event_participants (
    id UUID PRIMARY KEY,
    event_id UUID NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status participation_status NOT NULL DEFAULT 'pending',
    invited_at TIMESTAMPTZ NOT NULL,
    responded_at TIMESTAMPTZ,
    UNIQUE(event_id, user_id)
);

CREATE INDEX idx_events_creator_id ON calendar_events(creator_id);
CREATE INDEX idx_events_start_time ON calendar_events(start_time);
CREATE INDEX idx_events_visibility ON calendar_events(visibility);
CREATE INDEX idx_participants_event_id ON event_participants(event_id);
CREATE INDEX idx_participants_user_id ON event_participants(user_id);
CREATE INDEX idx_participants_status ON event_participants(status);