-- Announcement notifications now exist (composing through the app notifies
-- server-mates), so their preference defaults on, like every other notify_*
-- column. It defaulted off since 008 because nothing ever sent one.
--
-- Only the default changes. Existing rows keep their stored value: false
-- there is indistinguishable from someone who switched it off on /settings,
-- and overriding a stated preference is worse than a missed notification.
ALTER TABLE users ALTER COLUMN notify_announcements SET DEFAULT true;
