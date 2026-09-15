---
name: mockup-responsive-notifications-and-rsvp
description: Make the Notifications/Alerts page responsive per mobile mockup screen 09, and add inline Going/Maybe/Can't buttons on event_invite notification cards. The user explicitly confirmed adding inline RSVP over keeping notifications read-only. Depends on mockup-responsive-shell. Use when asked to make notifications responsive or to add inline RSVP from notifications.
---

# Responsive notifications + inline RSVP ("Alerts")

Source: `Friends Calendar Mobile.dc.html` screen 09 (Alerts). **Requires**
`mockup-responsive-shell` first. The user was asked explicitly whether to
add inline RSVP (today's notifications are tap-through/read-only) and
confirmed **yes** - this skill has a small real behavior change alongside
the layout work, not just CSS.

## Current state

`desktop/src/routes/notifications/+page.svelte` lists `NotificationInfo`
(`id, kind, actor_username?, actor_avatar_url?, event_id?, message, read,
created_at`) flat, with `markRead`/`markAllRead` already wired to
`api.markNotificationRead`/`api.markAllNotificationsRead`. No grouping, no
inline actions.

## What to build

1. **Inline RSVP** on cards where `kind === 'event_invite'` and `event_id`
   is present: Going/Maybe/Can't buttons calling
   `api.updateParticipation(event_id, status)` (existing method, already
   used by `EventDetailsModal.svelte` - reuse the same status-color/label
   mapping that component already has rather than redefining it). On
   success, mark the notification read too (`markRead`) since acting on it
   implies acknowledgment - don't require a second tap. Handle the case
   where the event was deleted/the user was removed since the notification
   was created (the update call 404s) by showing an inline error on that
   card, not crashing the whole list.
2. **Grouped-by-recency layout** ("Today" / "Earlier", per the mockup) -
   group `notifications` by whether `created_at` falls on the current
   calendar day; this is presentational grouping only, no new backend
   query (the existing `GET /api/notifications` already returns
   newest-first per CLAUDE.md).
3. Responsive card width/padding at 402px, verify the three RSVP buttons
   fit on one row without wrapping (mockup uses two flexible buttons +
   one fixed-width "Can't" button - match that ratio rather than three
   equal-width buttons if they don't fit).

## Tests

New test cases in `notifications/page.test.ts` for: inline RSVP buttons
render only on `event_invite` notifications with an `event_id`, clicking
one calls `api.updateParticipation` with the right args and marks the
notification read, and a failed update shows an error without removing
other notifications from the list. Grouping logic (if extracted into a
helper) gets a unit test.

## Verification

`yarn test`, `yarn run check`, `yarn build`, manual resize check. No
backend changes (`updateParticipation`/`markNotificationRead` already
exist).
