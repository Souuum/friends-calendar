---
name: mockup-announcement-thread
description: Add a real reply-thread view for announcement posts - a new backend endpoint that posts replies back to Discord (threaded under the original message) plus a desktop+mobile thread page with a composer. The user explicitly confirmed real Discord write-back over a read-only view. Use when asked to implement the announcement thread/reply feature.
---

# Announcement reply threads

Source: `Friends Calendar Mobile.dc.html` screens 07 (Hub · announcements,
"Open" action) and 08 (Announcement thread). The user was asked
explicitly (2026-09-15) whether replies should be real (posted back to
Discord) or a read-only view, and chose **real** - this is the one skill
in this round with genuine new backend surface, not just responsive CSS.

## Current state (what exists, what doesn't)

- `announcement_posts` (`009_add_announcement_feed.sql`) caches synced
  Discord messages: `id` (local UUID), `discord_message_id`, `channel_id`,
  author fields, title/body, tag, `reaction_count`, `reply_count`,
  `pinned`, `posted_at`. `AnnouncementPostInfo` (the API-facing type,
  `models/announcement.rs`) exposes `id` but **not** `discord_message_id`/
  `channel_id` - those stay internal, resolved server-side from `id`.
- `services::discord_feed::send_channel_message(base_url, http, bot_token,
  channel_id, content)` already posts a plain message to a channel -
  reused here, but a reply needs to land in the message's **thread**, not
  the channel itself.
- `services::discord_announcement::create_thread_from_message` (via
  `serenity`) already creates a thread from a message when an event is
  auto-announced - but `discord_feed`/`digest` intentionally use `reqwest`
  directly (not `serenity`) so they stay `wiremock`-testable (see
  `.claude/skills/add-tests/SKILL.md`). Don't mix `serenity` into
  `discord_feed` - instead add a small `reqwest`-based thread helper there,
  following the same `base_url`-as-parameter pattern as everything else in
  that file.
- `reply_count` on `announcement_posts` is a cached count from the last
  sync (`message.thread.message_count`), not live - posting a reply won't
  update it until the next `POST /api/announcements/sync`. That's fine
  (same "cache, not realtime" tradeoff the whole feed already accepts) but
  the thread page's reply list should be its own live fetch, not read from
  the stale cache.

## What to build

### Backend

1. **`services::discord_feed`**: add
   `fetch_or_create_thread(base_url, http, bot_token, channel_id,
   message_id) -> Result<String>` (thread id) - `POST /channels/{message_id}/threads`
   with a body like `{"name": <first ~90 chars of the post's title/body>}`
   (Discord's message-to-thread endpoint uses the *message id* as the
   path, matching `create_thread_from_message`'s intent) - if Discord
   already auto-created a thread for the message (has one from the
   original post), reuse it: `GET /channels/{channel_id}/threads/active`
   or check the message's own `thread` field from a
   `GET /channels/{channel_id}/messages/{message_id}` call first, per
   Discord's API - verify the exact shape against Discord's docs before
   assuming, same as `services::discord_feed`'s existing comments do for
   reactions/thread counts.
   Add `fetch_replies(base_url, http, bot_token, thread_id) ->
   Result<Vec<ReplyInfo>>` (`GET /channels/{thread_id}/messages`, reuse
   the same message-parsing shape `sync_channel` already has rather than
   duplicating the `DiscordMessage`/author-parsing structs - consider
   extracting a shared internal parser if the two functions would
   otherwise be near-identical).
2. **`models::announcement`**: add `ReplyInfo { author_username,
   author_avatar_url, body, posted_at }` (serialize-only, mirrors
   `AnnouncementPostInfo`'s shape).
3. **`handlers::announcements`**: add
   `GET /api/announcements/:id/replies` (resolve `id` → `discord_message_id`/
   `channel_id` from `announcement_posts`, fetch/create the thread, list
   replies) and `POST /api/announcements/:id/reply` (`{ "body": string }`
   request, same resolution, then `send_channel_message` into the thread
   id rather than the channel id - note `send_channel_message`'s existing
   signature already takes an arbitrary `channel_id: &str`, and Discord
   treats thread ids as channel ids for message-posting purposes, so no
   signature change needed there, just pass the thread id through).
   Both require `Claims` auth like every other handler in this file.
4. **`main.rs`**: register the two new routes under the existing
   `/api/announcements` group.
5. Tests: `wiremock`-backed integration tests for the new `discord_feed`
   functions (thread lookup/creation, reply fetch, matching the existing
   test module's style in that file) and functional tests for the two new
   handlers (round-trip: sync a post, fetch its replies, post a reply,
   fetch again and see it - mirrors `handlers::announcements`'s existing
   `sync_then_list_round_trips_over_http` test).

### Frontend

- `desktop/src/routes/announcements/[id]/+page.svelte` (new dynamic
  route) - header with back link, the original post (reuse
  `AnnouncementPostCard.svelte`), a reply list, and a composer (text input
  + send button, calls the new `POST /api/announcements/:id/reply`, then
  refetches replies - same optimistic-vs-refetch tradeoff other pages in
  this app already make, prefer refetch here since Discord round-trips
  aren't instant to predict). `AnnouncementPostCard`'s existing "Open"
  affordance (if the card doesn't currently link anywhere, add one) routes
  here.
- `api.ts`: `getAnnouncementReplies(id)`, `postAnnouncementReply(id, body)`.
- `types.ts`: `ReplyInfo` matching the backend shape.
- Responsive layout for this new page is this skill's own job (build it
  mobile-first per screen 08 - single column, docked composer - rather
  than building desktop-only and leaving it to `mockup-responsive-announcements`,
  since the thread page doesn't exist on desktop today either).

## Verification

`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`
(must not exceed baseline - check current count first per
`.claude/skills/add-tests/SKILL.md`), `yarn test`, `yarn run check`,
`yarn build`.
