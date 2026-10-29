-- Announcement notifications now exist (composing through the app notifies
-- server-mates), so their preference defaults on, like every other notify_*
-- column. It defaulted off since 008 because nothing ever sent one.
ALTER TABLE users ALTER COLUMN notify_announcements SET DEFAULT true;

-- Existing accounts are switched on too. A stored false is the old default,
-- not an opt-out: the toggle controlled nothing before this, and /settings
-- sent every toggle on each save, so saving any setting wrote back whatever
-- was loaded.
UPDATE users SET notify_announcements = true;
