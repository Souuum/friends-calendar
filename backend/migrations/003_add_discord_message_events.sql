ALTER TABLE calendar_events ADD COLUMN discord_message_id VARCHAR(255);
ALTER TABLE calendar_events ADD COLUMN discord_channel_id VARCHAR(255);

CREATE INDEX idx_events_discord_message_id ON calendar_events(discord_message_id);