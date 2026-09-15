---
name: mockup-friend-requests
description: Implement the "Add friends" flow from the Friends Calendar Mockups project - Discord-tag friend requests with accept/decline, invite links, and bot-invite for guild members who aren't on the app yet. Not yet executed as of 2026-09-15 - depends on nothing else, but pairs naturally with mockup-notifications for the accept/decline alert. Use when asked to build the add-friends/friend-request flow.
---

# Friend requests ("Add friends" screen)

Source: `isAdd` screen in `Friends Calendar Mockups.dc.html`. **Not yet
executed** - this is a genuinely new subsystem. Today, `friendships`
(`backend/migrations/003_create_friendships.sql`) only has rows created by
`services::friends::sync_friends` with `source = 'discord_guild'` - there is
no concept of a pending, one-directional request that needs accepting.

## Data model (new migration, next available number - check
`backend/migrations/` for the current highest before naming it)

```sql
CREATE TABLE IF NOT EXISTS friend_requests (
    id UUID PRIMARY KEY,
    from_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    to_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL DEFAULT 'pending', -- pending | accepted | declined
    created_at TIMESTAMPTZ NOT NULL,
    responded_at TIMESTAMPTZ,
    CONSTRAINT no_self_request CHECK (from_user_id <> to_user_id),
    UNIQUE(from_user_id, to_user_id)
);
CREATE INDEX idx_friend_requests_to_user_pending ON friend_requests(to_user_id) WHERE status = 'pending';
```

On accept: insert both directions into `friendships` with
`source = 'friend_request'` (a new source value alongside `'discord_guild'` -
`services::friends::sync_friends`'s stale-cleanup DELETE only touches rows
`WHERE source = 'discord_guild'`, so this won't collide with guild-sync
churn), then mark the request `accepted`.

## Backend

- `backend/src/models/friend_request.rs`: `FriendRequest`, `FriendRequestInfo`
  (id, from user info, created_at) for the incoming-requests list.
- `backend/src/services/friend_requests.rs`:
  - `send_request(db, from_user_id, to_username_or_discord_tag) -> Result<...>` -
    look up the target in `users` by username (there's no Discord-side tag
    lookup here; you can only request someone who already has an app
    account - same assumption `services::friends` makes).
  - `list_incoming(db, user_id)`, `respond(db, request_id, user_id, accept: bool)`.
- `backend/src/handlers/friend_requests.rs`: `POST /api/friend-requests`
  (body: `{ username }`), `GET /api/friend-requests` (incoming, pending),
  `POST /api/friend-requests/:id/accept`, `POST /api/friend-requests/:id/decline`.
- Bot-invite for missing members ("14 members of The Hangout aren't on
  Friends Calendar"): reuse `services::friends::fetch_guild_member_discord_ids`
  to get the guild roster, diff against `users.discord_id`, count the
  difference. "Post invite" reuses `services::discord_announcement`'s
  message-posting pattern to post into the configured channel.
- Invite link: out of scope for a first pass - a real implementation needs
  a signed/expiring token and a public (unauthenticated) redemption
  endpoint. Render the mockup's static-looking link section but wire
  "Copy" only; don't fake a working invite-link backend.

## Frontend

- `desktop/src/routes/friends/add/+page.svelte` (or `/friends` add a tab -
  match whatever nav structure `mockup-friends-directory` landed with).
  Discord-tag/username input + send button, pending-requests list with
  Accept/Decline, the bot-invite prompt card.
- `api.ts`: `sendFriendRequest(username)`, `listFriendRequests()`,
  `respondToFriendRequest(id, accept)`.
- After accept, the new friend should show up via the existing
  `api.getFriends()` (since accept writes real `friendships` rows) - no
  frontend friends-list change needed beyond a refetch.

## Depends on / pairs with

`.claude/skills/mockup-notifications/SKILL.md` - a received friend request
and an accepted request should both create a notification. Fine to ship
this skill without notifications first (the pending-requests list is
visible on the Add Friends page either way), but wire the notification
trigger in if that skill has already landed.

## Tests

Backend: `#[sqlx::test]` for send/accept/decline (including the
"can't send to someone who already has a pending/accepted relationship"
and "can't accept someone else's request" cases), unit test for the
username-lookup miss case. Frontend: component tests for the request
list + accept/decline actions (mock `$lib/api`), same pattern as
`FriendsList.test.ts`. Full three-tier guidance in
`.claude/skills/add-tests/SKILL.md`.
