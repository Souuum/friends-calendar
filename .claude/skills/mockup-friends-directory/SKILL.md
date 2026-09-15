---
name: mockup-friends-directory
description: Implement the Friends directory, friend-detail page, and the invite-friends picker in Create Event, from the "Friends Calendar Mockups" Claude Design project. Backend-free tier - uses only endpoints that already exist. Use when asked to build the friends directory/detail pages or to wire event-creation invites.
---

# Friends directory + friend detail + invite picker

Source: Claude Design project `Friends Calendar Mockups.dc.html`
(`0b825812-c8f2-4f1a-98d7-38900c6a0133`), screens `isFriends`/`isFriend`,
plus the invite-chips section of `isCreate`. Executed first (2026-09-15)
because it needs **no new backend** — everything comes from endpoints that
already exist: `GET /api/friends` (`FriendInfo[]`: user_id, username,
avatar_url, synced_at) and `GET /api/events`
(`EventWithParticipants[]`, includes full `participants[]`).

## Why this one first

This closes a real, already-documented gap: `CreateEventRequest.participant_ids`
is accepted by `services::calendar::create_event` (backend/src/services/calendar.rs)
but `CreateEventModal.svelte` never sends it — so today, nobody except the
creator ever gets invited to an event, no matter what `visibility` says.
See CLAUDE.md's Announcements section for the original write-up of this gap.

## What to build

1. **Invite picker in `CreateEventModal.svelte`**
   - On mount (or on open), `api.getFriends()`.
   - Render each friend as a toggleable chip (selected/unselected state,
     avatar + username) - mirror the mockup's `invitees` chip styling
     (`background:var(--subtle); border-radius:99px; padding:5px 11px 5px 5px;`).
   - Track selected friend IDs in local state; include
     `participant_ids: selectedIds` in the `api.createEvent(payload)` call
     (`desktop/src/lib/api.ts`'s `createEvent` already types `participant_ids?: string[]`
     - no api.ts change needed).
   - Don't build the mockup's live "Discord preview" panel or
     "Free tonight/Propose a time" bar in this pass - preview is cosmetic
     polish, propose-a-time depends on `.claude/skills/mockup-availability/SKILL.md`
     which hasn't been run yet.

2. **`desktop/src/routes/friends/+page.svelte`** - directory
   - `onMount`: `api.getFriends()`, same container/presentational split as
     `routes/settings/+page.svelte`.
   - Grid of cards (reuse styling patterns from `LinkedServerCard.svelte`/
     `FriendsList.svelte`), each showing avatar, username, and a "note" -
     for this pass, derive the note from shared upcoming events
     (`api.getEvents()`, cross-referenced by participant `user_id`) rather
     than the mockup's richer status pills (Going/Maybe/Free Sat/Idle/New) -
     those need availability data this tier doesn't have. Something like
     "Next: Raclette night, Fri" or "No shared events" is honest given what
     the backend actually knows right now.
   - Client-side search box (filter by username) and a single "All" filter
     chip - **do not** add the mockup's "Pending" filter chip, there's no
     friend-request concept yet (`.claude/skills/mockup-friend-requests/SKILL.md`).
   - Each card links to `/friends/[id]`.

3. **`desktop/src/routes/friends/[id]/+page.svelte`** - detail
   - SvelteKit dynamic route; get the friend's id from `$page.params.id`.
   - Fetch `api.getFriends()` and find the matching entry for header info
     (avatar, username). No dedicated `GET /api/friends/:id` endpoint exists
     or is needed for this - don't add one.
   - "Shared events": fetch `api.getEvents({ include_declined: true })`,
     filter to events where `participants` contains this friend's
     `user_id`, render with the same card style as `AnnouncementCard.svelte`
     minus the RSVP badge (or reuse it directly - it already renders
     title/date/location/price/link + participants cleanly).
   - **Adaptation, not a straight clone**: drop the mockup's "Mutual
     servers" card. This app links exactly one Discord server
     (`DISCORD_GUILD_ID`, see `services::friends`) - every friend page would
     show the identical single server, which tells the user nothing
     per-friend. If/when multi-server support ever lands, revisit.
   - Also omit the "Free this week" availability strip (belongs to
     `.claude/skills/mockup-availability/SKILL.md`) and "Invite to event" /
     "Message" action buttons (message has no backend concept at all;
     invite-to-event could link to `/` with the friend pre-selected as a
     nice follow-up, not required for this pass).

4. **Navigation**: add a "Friends" entry to `Frame.svelte`'s `navItems`
   (same array already holding Calendars/Announcement -
   `{ label: 'Friends', icon: '👥', view: '/friends' }`), following the
   routing pattern already established there (`$page.url.pathname` for
   highlighting, `goto` for navigation - see `Frame.test.ts` for how to
   test this against a mocked `$app/stores`/`$app/navigation`).

## Tests (`.claude/skills/add-tests/SKILL.md` conventions)

All frontend, no backend changed:
- Invite picker: toggling a chip updates selection; submitting includes
  the right `participant_ids` in the `api.createEvent` call (mock `$lib/api`).
- `/friends` page: loading/error/empty states, search filtering, mocking
  `$lib/api`.
- `/friends/[id]` page: shows the right friend, computes shared events
  correctly, 404-ish empty state if the id doesn't match any friend. Mock
  `$app/stores` for `$page.params` the same way `Frame.test.ts` mocks it
  for `$page.url`.

## Verification

`yarn test`, `yarn run check` (not `yarn check`), `yarn build` from
`desktop/`. No backend changes in this tier, so no `cargo` step.
