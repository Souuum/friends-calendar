---
name: mockup-notifications
description: Implement the Notifications screen and system from the Friends Calendar Mockups project - a notifications table, triggers wired into existing invite/RSVP/friend-request flows, a list/mark-read API, and a header bell badge. Not yet executed as of 2026-09-15. Use when asked to build notifications or a header notification badge.
---

# Notifications

Source: `isNotif` screen in `Friends Calendar Mockups.dc.html`, plus the
bell icon + red dot in the header chrome (`goNotifications`,
`nav` badge count "4"). **Not yet executed** - net-new subsystem, nothing
in the backend tracks "things a user hasn't seen yet" today.

## Data model (new migration, next available number)

```sql
CREATE TABLE IF NOT EXISTS notifications (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE, -- recipient
    kind VARCHAR(30) NOT NULL, -- event_invite | rsvp_change | friend_request | friend_accepted | announcement
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL, -- who caused it, nullable (e.g. system/bot-originated)
    event_id UUID REFERENCES calendar_events(id) ON DELETE CASCADE,
    message TEXT NOT NULL, -- pre-rendered text, e.g. "Lina Ruiz invited you to Raclette night"
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_notifications_user_unread ON notifications(user_id) WHERE read_at IS NULL;
CREATE INDEX idx_notifications_user_created ON notifications(user_id, created_at DESC);
```

Pre-rendering `message` at write time (rather than composing it at read
time from `kind`/`actor`/`event`) keeps the read path a single simple
query and matches the mockup's plain-text rows - accept that renames
(a user changing their display name later) won't retroactively update old
notification text, same tradeoff most apps make here.

## Backend

- `backend/src/models/notification.rs`: `Notification`, `NotificationInfo`
  (adds `actor` username/avatar for the row's avatar circle).
- `backend/src/services/notifications.rs`: `create(db, user_id, kind,
  actor_user_id, event_id, message)`, `list(db, user_id, limit)`,
  `mark_read(db, user_id, notification_id)`, `mark_all_read(db, user_id)`,
  `unread_count(db, user_id)`.
- `backend/src/handlers/notifications.rs`: `GET /api/notifications`,
  `POST /api/notifications/:id/read`, `POST /api/notifications/read-all`,
  `GET /api/notifications/unread-count` (for the header badge - cheap
  `SELECT count(*)`, don't make the frontend fetch and count the full list
  just to show a number).
- **Wire triggers into existing flows** (this is most of the real work -
  the table+CRUD above is the easy part):
  - `services::calendar::create_event`'s per-invitee insert loop → notify
    each invited user (`kind: event_invite`).
  - `services::calendar::update_participation_status` → notify the event's
    `creator_id` when someone else responds (`kind: rsvp_change`) - skip
    when the responder *is* the creator.
  - `.claude/skills/mockup-friend-requests/SKILL.md`'s send/accept →
    `kind: friend_request` / `friend_accepted`.
  - Announcement posting (`services::discord_announcement` or
    `.claude/skills/mockup-announcements-feed/SKILL.md`, whichever lands) →
    `kind: announcement`, fanned out to... everyone? Just people who
    reacted/participate? Needs a real decision when this is picked up -
    don't guess it away silently, ask or default to "nobody" (i.e. skip this
    one trigger) until it's clarified, since notifying the whole server on
    every post could be noisy/wrong.

## Frontend

- `desktop/src/routes/notifications/+page.svelte`: list, unread rows
  visually distinct (`var(--accent-soft)` background per the mockup),
  actionable rows (event invites get inline Going/Maybe buttons - reuse
  `api.updateParticipation`, same call `EventDetailsModal.svelte` already
  makes), "Mark all read" button.
- Header bell badge: `Header.svelte` gains a bell button
  (`api.getUnreadNotificationCount()` on mount, small red dot if > 0) that
  navigates to `/notifications` - this is genuinely new UI in `Header.svelte`,
  not present today at all.
- `Frame.svelte` nav badge: the existing `navItems` array already has a
  `badge` field (see `item.badge`/`item.badgeStyle` in the mockup - our
  current `Frame.svelte` doesn't use it yet, `ViewButton.svelte` doesn't
  render it either). Extending `ViewButton.svelte` to show a badge and
  wiring the Notifications nav item's badge to the unread count is a
  reasonable, small addition alongside this skill.

## Tests

Backend: `#[sqlx::test]` for create/list/mark-read/unread-count, and
(important) that the *existing* flows above actually insert the right
notification rows - test through `services::calendar::create_event` /
`update_participation_status`, not just `services::notifications` in
isolation, or the trigger wiring itself goes untested. Frontend: list
page states (mock `$lib/api`), header badge component test. Full
guidance in `.claude/skills/add-tests/SKILL.md`.
