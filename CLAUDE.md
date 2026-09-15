# CLAUDE.md

This file gives Claude Code (and other agents) grounding in how this repo is
actually structured today, as opposed to how it might look from commit
messages or branch names alone. Everything below was verified against the
working tree and git history, most recently on 2026-09-14 (feature-branch
merge pass).

## What this is

A Discord-OAuth "friends calendar" app: a Rust/Axum backend, a
Svelte/SvelteKit + Tauri desktop client, and a Discord bot that announces
events into a Discord channel and tracks RSVPs via reactions. Infra is
Proxmox LXC via Terraform, deployed through GitHub Actions.

Testing is a standing requirement here, not optional polish — see
"Testing" below and `.claude/skills/add-tests/SKILL.md`.

## Branches

```
* master                    <- default branch, active
  remotes/origin/master
  remotes/origin/dev/refacto        0 ahead / 18 behind master -> fully merged, stale
  feat(Calendar)                    0 ahead / 19 behind master -> fully merged, stale
  feat(DiscordBot)                  0 ahead / 10 behind master -> fully merged, stale
  feat(Event)                       0 ahead / 45 behind master -> fully merged, stale
  feat(Storybook)                   0 ahead / 36 behind master -> fully merged, stale
  feat(terraform)                   0 ahead / 17 behind master -> fully merged, stale (local-only, no remote counterpart)
```

`master` is both the current and default branch (`origin/HEAD -> origin/master`).
As of the merges on 2026-09-14, **every branch is fully absorbed into
`master`** — all 0 ahead. None of them have unique work left, so all are
safe to delete (`git push origin --delete <branch>` for the remote ones,
`git branch -d <branch>` locally) whenever someone gets around to it; that
cleanup hasn't been done yet, the branches are just stale pointers now.

`feat(DiscordBot)` and `feat(Calendar)` were merged via explicit merge
commits (`6047c67`, `d9e7c42`) rather than fast-forwarded, since both needed
real conflict resolution / fixes along the way — see
[Discord bot](#discord-bot-bot-rs-services-discord_announcement-rs) below
for what `feat(DiscordBot)`'s merge involved. `feat(Calendar)`'s merge kept
two genuine changes (tooltip popup delay 1000ms → 500ms, and deleting a
dead `formatHeaderDate()` function already superseded by a reactive block)
but reverted one part of its "fixed format" commit that silently collapsed
the day-view-aware header date logic — that looked like unintentional
collateral damage from the formatting pass, not a deliberate change, since
nothing else in the branch or its commit messages explains it.

## Package managers / workspace setup

- **Cargo workspace**: `[workspace] members = ["backend", "desktop/src-tauri"]`
  at repo root, resolver `"2"`. ⚠️ **The manifest is committed as
  `cargo.toml` (lowercase), not `Cargo.toml`.** This only works by accident on
  case-insensitive filesystems (macOS/Windows default). On case-sensitive
  Linux (e.g. the `ubuntu-latest` CI runners) `cargo` at the repo root won't
  find it, since Cargo requires the exact filename `Cargo.toml`. Every
  workflow in `.github/workflows/` avoids this by explicitly `cd backend`
  first, so it hasn't bitten yet, but it should be renamed.
- **JS/TS**: yarn at the root (`yarn.lock` present, no root `package-lock.json`).
  Root `package.json` is a thin orchestration layer (`concurrently` to run
  backend + desktop dev servers). It is **not** an npm/yarn workspaces
  manifest — no `workspaces` field, `desktop` is just `cd`'d into by scripts.
  ⚠️ `desktop/` has **both** `yarn.lock` and `package-lock.json` committed —
  mixed package manager artifacts. Treat `yarn.lock` as authoritative (root
  scripts all invoke `yarn`) and remove `desktop/package-lock.json`.

## `backend/` (Rust / Axum)

```
backend/
├── .env                       # not committed content matters here — see Secrets note below
├── Cargo.toml / Cargo.lock
├── migrations/
│   ├── 001_create_users_table.sql
│   ├── 002_create_events_table.sql
│   ├── 003_create_friendships.sql          # friend-list sync, see below
│   ├── 004_add_discord_message_events.sql  # discord_message_id/discord_channel_id on calendar_events
│   ├── 005_add_price_and_link.sql          # price/link on calendar_events
│   ├── 006_create_notifications.sql        # notifications table, see below
│   └── 007_create_friend_requests.sql      # friend_requests table, see mockup roadmap below
└── src/
    ├── main.rs                # entrypoint, build_router() (pub(crate), reused by functional tests), CORS, server bootstrap, spawns the Discord bot
    ├── config.rs               # AppState: db pool, oauth2 client, jwt secret, pkce store, discord bot token/guild id/announcement channel/api base, http client. #[cfg(test)] AppState::for_test(..)
    ├── bot.rs                  # Discord gateway bot (serenity) — reaction-based RSVP tracking
    ├── error.rs                 # AppError -> HTTP response mapping
    ├── handlers/
    │   ├── mod.rs
    │   ├── announcements.rs     # list/sync the Discord channel message mirror
    │   ├── auth.rs              # Discord OAuth2 login/callback/me/logout, verify_jwt(), generate_jwt() (pub(crate), reused by functional tests)
    │   ├── availability.rs      # friends-now, week
    │   ├── calendar.rs          # CRUD for events + participants + link_discord_message
    │   ├── discord.rs           # get_linked_server — which Discord server this app is linked to
    │   ├── discord_config.rs    # get/update DB-backed bot channel config
    │   ├── friend_requests.rs   # send/list/accept/decline + missing-members/post-invite
    │   ├── friends.rs           # list/sync friends
    │   ├── notifications.rs     # list/mark-read/mark-all-read/unread-count
    │   └── profile.rs           # update profile/preferences, delete account
    ├── middleware/
    │   ├── mod.rs
    │   └── auth.rs              # Claims extractor (FromRequestParts) backing JWT auth
    ├── models/
    │   ├── mod.rs
    │   ├── user.rs                 # User, DiscordUser, UpdateProfileRequest, DeleteAccountRequest
    │   ├── calendar_event.rs       # CalendarEvent, CreateEventRequest, UpdateEventRequest, Visibility, ParticipationStatus, etc.
    │   ├── friendship.rs           # FriendInfo, SyncFriendsResult
    │   ├── discord_guild.rs        # LinkedServerInfo
    │   ├── discord_bot_config.rs   # BotChannelConfig, UpdateBotChannelConfigRequest
    │   ├── notification.rs         # NotificationInfo
    │   ├── friend_request.rs       # FriendRequestInfo
    │   └── announcement.rs         # AnnouncementPostRow (DB), AnnouncementPostInfo (API)
    └── services/
        ├── mod.rs
        ├── auth.rs
        ├── availability.rs           # pure interval free/busy logic + free_users_now/week_availability
        ├── calendar.rs               # also owns the event_invite/rsvp_change notification triggers, see below
        ├── digest.rs                  # weekly announcements digest: is_due + maybe_send_weekly_digest + spawn_digest_loop
        ├── discord_announcement.rs   # posts event announcements + creates discussion threads
        ├── discord_config.rs          # DB-backed bot channel config, resolve_announcement_channel_id
        ├── discord_feed.rs            # sync/list Discord channel messages into announcement_posts
        ├── friends.rs                # Discord guild member fetch + friendship sync + get_linked_server_info
        ├── friend_requests.rs        # send/list/respond + friend_request/friend_accepted notification triggers + missing-members/post-invite
        ├── notifications.rs          # create/list/mark-read/mark-all-read/unread-count
        └── profile.rs                 # update_profile, delete_account (with confirm-username guard)
```

Runs on `axum = "0.7"`, `sqlx` (Postgres, runtime-tokio-native-tls),
`oauth2`, `jsonwebtoken`, `serenity = "0.12"` (Discord gateway bot, rustls
backend), `tower` with the `util` feature enabled specifically for
`ServiceExt::oneshot` in functional tests.

Server binds `127.0.0.1:8080`. CORS is hard-coded to allow only
`http://localhost:1420` (the Tauri dev origin) with credentials. (This is
also why `terraform/templates/nginx.conf` doesn't set its own CORS headers
— see the Terraform section below for why that combination breaks browsers
when both layers do it.)

### Implemented API endpoints (from `backend/src/main.rs`)

```
GET    /                                       root()  — plaintext banner

# Auth
GET    /api/auth/discord                       handlers::auth::discord_login
GET    /api/auth/callback                      handlers::auth::discord_callback
GET    /api/auth/me                             handlers::auth::get_current_user
PATCH  /api/auth/me                             handlers::profile::update_profile
DELETE /api/auth/me                             handlers::profile::delete_account
POST   /api/auth/logout                        handlers::auth::logout

# Calendar events
POST   /api/events                              handlers::calendar::create_event
GET    /api/events                              handlers::calendar::list_events
GET    /api/events/:id                          handlers::calendar::get_event
PUT    /api/events/:id                          handlers::calendar::update_event
DELETE /api/events/:id                          handlers::calendar::delete_event

# Participants
POST   /api/events/:id/participants             handlers::calendar::invite_participants
PUT    /api/events/:id/participation             handlers::calendar::update_participation
DELETE /api/events/:id/participants/:user_id    handlers::calendar::remove_participant

# Friends
GET    /api/friends                              handlers::friends::list_friends
POST   /api/friends/sync                         handlers::friends::sync_friends

# Discord bot
POST   /api/events/:id/link-discord             handlers::calendar::link_discord_message

# Discord server info
GET    /api/discord/server                       handlers::discord::get_linked_server

# Notifications
GET    /api/notifications                        handlers::notifications::list_notifications
GET    /api/notifications/unread-count           handlers::notifications::unread_count
POST   /api/notifications/read-all               handlers::notifications::mark_all_read
POST   /api/notifications/:id/read               handlers::notifications::mark_read

# Friend requests
POST   /api/friend-requests                       handlers::friend_requests::send_request
GET    /api/friend-requests                       handlers::friend_requests::list_incoming
POST   /api/friend-requests/:id/accept            handlers::friend_requests::accept_request
POST   /api/friend-requests/:id/decline           handlers::friend_requests::decline_request
GET    /api/friend-requests/missing-members       handlers::friend_requests::missing_members
POST   /api/friend-requests/post-invite           handlers::friend_requests::post_invite

# Availability
GET    /api/availability/friends-now              handlers::availability::friends_now
GET    /api/availability/week                      handlers::availability::week

# Discord bot channel config (DB-backed, see Settings/Server pages below)
GET    /api/discord/config                         handlers::discord_config::get_config
PUT    /api/discord/config                         handlers::discord_config::update_config

# Announcements (Discord channel message mirror, see below - replaced the
# old event-RSVP-tracking /announcements)
GET    /api/announcements                          handlers::announcements::list_announcements
POST   /api/announcements/sync                      handlers::announcements::sync_announcements
```

Frontend (`desktop/src/lib/api.ts`) targets `http://localhost:8080` by
default (`VITE_API_URL` override), which matches the backend's bind address.

### DB migrations (on `master`)

```
001_create_users_table.sql            users table, unique discord_id index
002_create_events_table.sql           calendar_events + event_participants,
                                       visibility & participation_status enums,
                                       time-range CHECK constraint, several indexes
003_create_friendships.sql            friendships table (see Friend-list sync below)
004_add_discord_message_events.sql    discord_message_id/discord_channel_id on calendar_events
005_add_price_and_link.sql            price/link on calendar_events
006_create_notifications.sql          notifications table (see Notifications below)
007_create_friend_requests.sql        friend_requests table (see mockup roadmap below)
008_add_profile_and_bot_config.sql    profile/preference columns on users (display_name,
                                       timezone, default_visibility, notify_* booleans) +
                                       discord_bot_config table (see Settings/Server below)
009_add_announcement_feed.sql         announcement_posts table (cached Discord channel
                                       messages) + digest_enabled/last_digest_sent_at on
                                       discord_bot_config (see Announcements page below)
```

`004`/`005` originated on `feat(DiscordBot)` as its own `003`/`004` (see
that branch's history) — renumbered during the merge to avoid colliding
with `master`'s own, different `003`, and rewritten with
`ADD COLUMN IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` so they're safe
to apply against a dev database that already has these columns from
testing that branch locally pre-merge (this happened for real on one
machine — `sqlx::migrate!` failed with `Error: VersionMissing(4)` on boot
until the stale `_sqlx_migrations` rows from the old numbering were
cleared: `DELETE FROM _sqlx_migrations WHERE version IN (3, 4);`, without
touching the columns themselves since they can hold real local data).
`005`'s filename also fixes a typo from the original branch ("prince" →
"price") before it became permanent migration history.

If you're setting up fresh (no local history with the old numbering),
none of this matters — migrations just apply 001 through 005 in order.

### Friend-list sync (`GET /api/friends`, `POST /api/friends/sync`)

Implemented in `backend/src/services/friends.rs` /
`backend/src/handlers/friends.rs`, backed by `003_create_friendships.sql`
(a `friendships` table storing directed edges in both directions, tagged
with a `source` and `synced_at`).

⚠️ **Important constraint that shaped this design:** Discord does not expose
a user's real Friends/relationships list to bots or OAuth2 apps — that's the
private, undocumented `/users/@me/relationships` endpoint, gated behind a
full user token, and calling it from anything but the official client
violates Discord's Developer Terms of Service. So "friend-list sync" here
means something narrower and ToS-compliant: **two app users are synced as
friends if they both belong to the Discord guild the bot lives in**,
checked via the bot's REST API (`GET /guilds/{id}/members`, paginated).
This also happens to be exactly what the `visibility: 'friends'` field on
`calendar_events` (present since `002_create_events_table.sql`) needed but
never had an implementation for.

- `POST /api/friends/sync` — fetches the configured guild's member list via
  the bot token, matches non-bot members against existing `users` rows by
  `discord_id`, upserts `friendships` rows both directions with a fresh
  `synced_at`, and deletes any previously-synced `discord_guild` edge that
  this pass no longer confirms (e.g. someone left the server).
- `GET /api/friends` — plain read of the current user's synced friends, no
  Discord call.
- Requires two new env vars beyond what `feat(DiscordBot)` already needed:
  `DISCORD_BOT_TOKEN` (already present in `backend/.env`) and
  `DISCORD_GUILD_ID` (newly added, currently blank — fill in the target
  server's ID). Also requires the "Server Members Intent" enabled for the
  bot application in the Discord developer portal, same requirement
  `feat(DiscordBot)`'s gateway bot already has.
- Both are read as `Option<String>` in `AppState` (`config.rs`), not
  `.expect()`-ed at startup — deliberately, to avoid the failure mode
  flagged elsewhere in this doc where a missing Discord env var takes down
  the entire backend. If unset, `/api/friends/sync` returns a 400 instead.
- This feature only needs `reqwest` (already a dependency) — it talks to
  Discord's REST API directly rather than depending on `serenity` or
  `feat(DiscordBot)`'s gateway bot, so it works standalone on `master`.
- No automatic re-sync (e.g. on login, or on a schedule) — it's purely
  on-demand, triggered from the `/friends` directory page via the "Sync
  friends" button (moved there from `/settings` when that page was split —
  see Settings/Server pages below).

Tests: `backend/src/services/friends.rs`'s `#[cfg(test)] mod tests` — see
"Testing" further down for the general policy and where the patterns are
documented.

### Settings page (`/settings`) — profile, preferences, account deletion

`/settings` used to hold linked-server info and the friends list; both moved
out (server → `/server` below, friends → the `/friends` directory's own
"Sync friends" button) so it no longer shows the same friend/server info in
three places. What's left is genuinely account-scoped:

- `PATCH /api/auth/me` (`handlers::profile::update_profile`,
  `services::profile::update_profile`) — fetch-merge-update over the new
  `users` columns from `008_add_profile_and_bot_config.sql`: `display_name`,
  `timezone`, `default_visibility`, and four `notify_*` booleans
  (`notify_event_invites`, `notify_rsvp_changes`, `notify_announcements`,
  `notify_weekly_digest`). Only fields present in the request body change —
  same pattern `services::calendar::update_event` already established.
  `default_visibility` is stored but not yet consumed as an actual default
  anywhere `CreateEventModal.svelte` builds a request — that wiring is left
  for whoever builds on this next.
- `DELETE /api/auth/me` (`handlers::profile::delete_account`,
  `services::profile::delete_account`) — real, cascading account deletion
  (`ON DELETE CASCADE` on `calendar_events`/`event_participants`/etc.,
  verified via a real test, not just assumed from the schema). Guarded: the
  request body must echo the account's own `confirm_username`, or it 400s
  and leaves the account untouched (`DeleteAccountOutcome::ConfirmationMismatch`)
  — a valid session alone isn't enough. `desktop/src/routes/settings/+page.svelte`
  disables the delete button client-side until the typed confirmation text
  matches the username too, as a first line of defense before the request
  even goes out.
- Notification toggles are stored but **not yet read** by
  `services::notifications::create` or anywhere else — they're
  user-editable preferences with no enforcement point wired up yet, same
  kind of gap as `default_visibility` above.

### Server page (`/server`) — linked Discord server + bot channel config

Split out of the old `/settings` so "which Discord server, and which
channels does the bot use" has its own page, separate from personal
profile/preferences. Single-server only — `DISCORD_GUILD_ID` is one guild,
not a list; multi-server support would need a real data model change, not
just this UI.

- `GET /api/discord/server` (`handlers::discord::get_linked_server`,
  `services::friends::get_linked_server_info`) — unchanged from before,
  `GET /guilds/{id}` on the bot token, returns
  `{ id, name, icon_url, approximate_member_count }`.
- `GET`/`PUT /api/discord/config` (`handlers::discord_config`,
  `services::discord_config`, backed by `discord_bot_config`, one row per
  guild) — DB-backed, user-editable channel IDs for events/announcements/
  reminders, **replacing** the old single-purpose
  `DISCORD_ANNOUNCEMENT_CHANNEL_ID` env var as the source of truth for
  where auto-announced events get posted. `GET` never 404s — returns an
  all-null `BotChannelConfig` if nothing's been saved yet, so the page can
  render an empty form instead of an error state.
  - **Important, deliberate limitation:** this migration is partial.
    `services::calendar::create_event`'s per-request auto-announce now
    resolves the channel via `services::discord_config::resolve_announcement_channel_id`
    (DB config first, env var as fallback if nothing's configured), so a
    change here takes effect on the very next event created. `bot.rs`'s
    gateway `Handler`, however, still reads `announcement_channel_id` once
    at process startup from the env var and is **not** hot-reloaded — a
    change made on this page won't reach the long-lived gateway connection
    until the process restarts. Not fixed here because it would mean either
    polling the DB from the gateway handler or a restart-signal mechanism,
    both bigger than this skill's scope; flagging it rather than silently
    leaving a half-working "live-editable" claim.
- `desktop/src/routes/server/+page.svelte` — reachable via `Frame.svelte`'s
  new "Discord server" sidebar item. Renders `LinkedServerCard.svelte`
  (unchanged, reused as-is) plus the channel-config form, and a static
  bot-permissions list (no endpoint backs this — it documents the
  permissions the bot needs, same content on every load).
- `AppState.discord_api_base: String` (defaults to the real Discord API,
  overridable via `DISCORD_API_BASE` — mainly for tests) is unchanged from
  before — this endpoint and `services::friends` still share it.

### Announcements page (`/announcements`) — Discord channel message mirror

Replaced 2026-09-15 (`mockup-announcements-feed`, after the user explicitly
confirmed "replace" over "keep alongside") — this used to show *calendar
events that got RSVP-announced to Discord*; it now shows the linked
channel's actual Discord messages, a real mirror rather than a
calendar-events filter. See below for what happened to the old view.

- `009_add_announcement_feed.sql`: `announcement_posts` (one row per synced
  Discord message: author, title/body, tag, reaction/reply counts, pinned,
  posted_at) + `digest_enabled`/`last_digest_sent_at` on `discord_bot_config`.
- `services::discord_feed` — `sync_channel` (`GET /channels/{id}/messages`,
  base_url as a parameter so it's `wiremock`-testable, same pattern as
  `services::friends`) upserts by `discord_message_id` (`ON CONFLICT DO
  UPDATE` — a re-sync picks up edits, doesn't duplicate); `list_posts`
  (pinned first, then newest); `count_posts_since` (feeds the digest, see
  below); `send_channel_message` (shared HTTP-POST helper, also used by the
  digest job). No "drop rows no longer confirmed" pass like friend sync has
  — a deleted Discord message just becomes a harmless stale local row
  rather than costing a second API call per sync to detect deletions.
  - **Tag** (`event`/`general`): inferred, not stored in Discord. A message
    tags `event` iff its `discord_message_id` matches an existing
    `calendar_events` row (i.e. it's the exact message
    `services::calendar::create_event`'s auto-announce posted); everything
    else defaults to `general`. The mockup's third tag, "Poll", was dropped
    entirely rather than guessed at — there's no real signal for it in a
    plain message fetch.
  - **Title**: Discord messages don't have one. If the content has more
    than one line, the first line becomes the title and the rest the body;
    a single-line message has no title, just body text.
  - **Reactions/replies**: `reaction_count` sums `message.reactions[].count`;
    `reply_count` reads `message.thread.message_count` (0 if the message
    has no thread) — both are Discord's own numbers, not recomputed here.
  - **Pinned**: read straight from the message object's own `pinned` field
    (Discord already tracks this via `GET /channels/{id}/pins`-backed
    state) — no app-side pinning concept was invented.
- `handlers::announcements` — `GET /api/announcements` (list from the local
  cache), `POST /api/announcements/sync` (fetch + upsert, then return the
  fresh list). Both resolve the channel the same DB-config-first,
  env-var-fallback way `create_event`'s auto-announce does
  (`services::discord_config::resolve_announcement_channel_id`), so the
  feed always mirrors whatever channel events actually get announced to.
  Single channel only — matches this app's single-guild scope, not the
  mockup's multi-channel sidebar.
- `desktop/src/routes/announcements/+page.svelte` — "Sync now" button
  (`POST /api/announcements/sync`) plus the cached list
  (`GET /api/announcements` on mount). A sync failure shows an error
  *without* clearing whatever posts already loaded (the error and the list
  are independent conditionals, not an `{#if error}...{:else}` pair — the
  first draft of this page got that wrong and a failed sync briefly wiped
  the visible feed, caught by `page.test.ts`'s
  `shows a sync error without clearing the existing posts` test).
  `AnnouncementPostCard.svelte` (new, presentational, `molecules/`) renders
  each post: author/avatar, title/body, tag badge, pinned badge, reaction
  and reply counts. Read-only, like the page it replaced.
- **Weekly digest** — a real scheduled job, not just a toggle that does
  nothing (the skill explicitly warned against that). `services::digest`:
  `is_due(now, last_sent)` (pure, unit-tested — due on Monday at/after
  9:00 UTC, and either nothing's ever been sent or it's been ≥6 days since
  the last one, the 6-day floor guarding against re-firing on every hourly
  poll through the same Monday morning) and `maybe_send_weekly_digest`
  (checks the guild's `discord_bot_config.digest_enabled`, posts a
  one-line "N new posts this week" message via `discord_feed::send_channel_message`,
  stamps `last_digest_sent_at`). `spawn_digest_loop` is spawned from
  `main.rs` (same conditional-spawn-if-bot-token-and-guild-id-configured
  shape as the Discord bot itself) on an hourly `tokio::time::interval` —
  polling hourly rather than trying to wake exactly at 9:00 costs nothing
  given `is_due`'s tolerance. The toggle lives on the `/server` page (a
  **guild-level** setting on `discord_bot_config`), deliberately separate
  from the per-user `notify_weekly_digest` preference added in
  `008_add_profile_and_bot_config.sql` — that one is still stored-but-unread
  (see Settings page above), since digests post to one shared channel, not
  per-user, so a guild-level switch is what actually needed a mechanism.

**What happened to the old event-RSVP view**: `AnnouncementCard.svelte` (the
component, not the page) is still real and still used — renamed to
`EventRsvpCard.svelte` since "Announcement" now means something else, and
kept wired into `/friends/[id]`'s "Shared events" section, the one other
place in the app that shows calendar events with a read-only RSVP badge.
Nothing about *that* feature changed; only its old top-level page and
component name did.

⚠️ **Real limitation surfaced while building the old view — half-fixed
since:** `GET /api/events` (`services::calendar::list_user_events`) only
returns events where you're already a row in `event_participants` — it does
not consult `visibility` at all for listing (unlike the single-event `GET
/api/events/:id`, which does check `OR e.visibility = 'public'`).
`CreateEventModal.svelte` didn't send `participant_ids` either — there was
no UI for inviting anyone at creation time. **The invite-picker half of
this is now fixed** (see "Friends directory" below,
`.claude/skills/mockup-friends-directory/SKILL.md`) — you can now actually
invite friends when creating an event. The listing-query half is still
open: `visibility: 'friends' | 'public'` still doesn't make an event
appear for anyone who wasn't explicitly invited, even though the field
implies it should. That's still a real backend feature (a listing query
that also matches on visibility, not just direct participancy), not
something to bolt on silently. (This limitation is about `GET /api/events`,
used by `/friends/[id]`'s shared-events section and the calendar itself —
it no longer has anything to do with `/announcements`, which now reads
from `announcement_posts` instead.)

## Testing

**Standing policy for this repo, not just this feature: every backend or
frontend change that adds or changes behavior gets tests at whichever tiers
apply (unit / integration / functional for backend, component tests for
frontend) — it's part of finishing the feature, not optional follow-up.**

Full conventions, worked examples, and a pre-flight checklist live in
`.claude/skills/add-tests/SKILL.md` — read that before writing tests here
rather than re-deriving the patterns. Summary:

- Backend tests live in `#[cfg(test)] mod tests` at the bottom of the file
  under test (no separate `tests/` directory). Run:
  `DATABASE_URL=$(grep DATABASE_URL backend/.env | cut -d= -f2-) cargo test`
  from `backend/` — `.env` is only loaded by `main()`, not by `cargo test`.
  - Unit: pure functions, plain `#[test]`. Example:
    `models::discord_guild::LinkedServerInfo::build_icon_url`.
  - Integration: DB logic via `#[sqlx::test]` (real schema, scratch
    database per test); Discord/external-HTTP calls via `wiremock`
    (never the real API) — needs the function under test to take the
    Discord API base URL as a parameter (`&state.discord_api_base` in
    production) rather than hardcoding it. Example:
    `services::friends`'s whole test module.
  - Functional: a real request through the actual `axum::Router` via
    `tower::ServiceExt::oneshot` (`tower`'s `util` feature, enabled for
    this), against a test-only `AppState::for_test(db, discord_api_base)`
    (`#[cfg(test)]`-gated in `config.rs`) and a JWT from
    `handlers::auth::generate_jwt` (`pub(crate)`, for this reason).
    `main.rs`'s router-building was pulled out into `pub(crate) fn
    build_router(state) -> Router` specifically so tests exercise the
    exact same routing/CORS setup as the real server. Example:
    `handlers::discord`'s test module.
- Frontend: `vitest` + `@testing-library/svelte` + `@testing-library/jest-dom`
  + `happy-dom`, co-located `<Component>.test.ts`. Run `yarn test` from
  `desktop/`. Prefer presentational/props-driven components (see
  `LinkedServerCard.svelte`) — trivially testable
  without mocking anything. Page-level components that own their own
  `onMount` fetch (see `routes/settings/+page.svelte`) need `$lib/api` (and
  often `$app/navigation`) mocked via `vi.mock(...)` — see
  `routes/settings/page.test.ts`. Anything rendering `Frame.svelte` (most
  pages) also needs `$app/stores`'s `page` mocked, since `Frame` reads
  `$page.url.pathname` for sidebar highlighting — see `Frame.test.ts` for
  the settable-store mock pattern (`__setPathname`), or
  `routes/friends/[id]/page.test.ts` for a version that also supplies
  `$page.params` for a dynamic route.
- `cargo clippy --all-targets --all-features -- -D warnings` and
  `yarn run check` (not `yarn check`, which is yarn's own unrelated
  built-in command) both have pre-existing failures unrelated to any given
  change — don't chase those, but make sure new code doesn't add to the
  pile.

## Friends Calendar Mockups (Claude Design project) — roadmap

A Claude Design project (`Friends Calendar Mockups.dc.html`, project id
`0b825812-c8f2-4f1a-98d7-38900c6a0133`, read via the `DesignSync` MCP tool)
was imported 2026-09-15 as the design for this app's next stage. It's
close to a full redesign — 9 screens, several needing backend subsystems
that don't exist yet — so it was split into one skill per feature area
rather than attempted as one change. All 6 executed as of 2026-09-15
(friends-directory, notifications, friend-requests, availability,
settings-and-server, announcements-feed), each on its own branch, merged
and pushed once its own tests/build were green. The first four ran
autonomously back-to-back while the user was away; `mockup-settings-and-server`
(including its live account-deletion endpoint) and `mockup-announcements-feed`
(including replacing the old `/announcements` view entirely) each ran
after the user explicitly answered the open question blocking it — a
general "go ahead" was deliberately *not* treated as answering either
question on its own. Status per skill:

- `.claude/skills/mockup-friends-directory/SKILL.md` — **done.** Friends
  directory (`/friends`), friend detail (`/friends/[id]`), and the
  invite-picker in `CreateEventModal.svelte`. No new backend needed.
- `.claude/skills/mockup-friend-requests/SKILL.md` — **done.**
  `friend_requests` table (`007_create_friend_requests.sql`) alongside
  guild-sync friendships, not replacing them —
  `services::friend_requests`/`handlers::friend_requests`
  (`POST`/`GET /api/friend-requests`, `POST .../:id/accept`,
  `POST .../:id/decline`, `GET .../missing-members`,
  `POST .../post-invite`). Sending, accepting, and declining all go
  through `services::friend_requests`, which also wires the
  `friend_request`/`friend_accepted` notification triggers. A mutual
  pending request auto-accepts instead of creating a duplicate row; a
  declined request can be re-sent later rather than being permanently
  blocked by the table's `UNIQUE(from_user_id, to_user_id)`. Accepting
  writes symmetric `friendships` rows with `source = 'friend_request'`
  (a new source value alongside `services::friends`'s `'discord_guild'`,
  so the sync's stale-cleanup query — scoped to `source = 'discord_guild'`
  — never touches manually-added friends). `/friends/add` page: send by
  username (not the mockup's "Discord tag" — there's no tag-based lookup,
  only app usernames, so the copy was adapted rather than cloned),
  incoming-requests list with accept/decline, and the "N members aren't on
  Friends Calendar yet" bot-invite prompt.
- `.claude/skills/mockup-notifications/SKILL.md` — **done.** `notifications`
  table (`006_create_notifications.sql`), `services::notifications`,
  `handlers::notifications` (`GET /api/notifications`, `GET
  /api/notifications/unread-count`, `POST /api/notifications/:id/read`,
  `POST /api/notifications/read-all`), triggers wired into
  `services::calendar::create_event` (invite) and
  `update_participation_status` (RSVP change) — the two flows that already
  existed and needed no new data to notify about. `/notifications` page,
  a shared `unreadNotificationCount` store (`stores.ts`) so the header
  bell and the sidebar's Notifications badge agree without each fetching
  independently, and `ViewButton.svelte` gained badge rendering. The
  announcement-posting trigger is intentionally **not** wired (the skill
  flags it as ambiguous — who should be notified on a post? - pending
  `mockup-announcements-feed`).
- `.claude/skills/mockup-announcements-feed/SKILL.md` — **done.** The user
  confirmed "replace" over "keep alongside" for the scope question the
  skill flagged. See "Announcements page" above for the full picture:
  `announcement_posts` cache table, `services::discord_feed` (sync/list,
  tag inference, title-splitting), `handlers::announcements`
  (`GET`/`POST .../sync`), the rebuilt `/announcements` page, and a real
  weekly-digest scheduled job (`services::digest`, gated by a new
  guild-level `discord_bot_config.digest_enabled` toggle on `/server`) so
  that setting isn't a no-op switch. The old event-RSVP card survives as
  `EventRsvpCard.svelte`, still used by `/friends/[id]`'s shared-events
  section — only the top-level `/announcements` page and the component's
  name changed. `notifications`'s announcement-posting trigger (flagged
  above as pending this skill) is still **not** wired — deciding who
  should be notified on a synced post, and whether "sync" should even be
  the trigger point vs. Discord posting in real time, is a separate call
  from what this skill's scope question actually asked.
- `.claude/skills/mockup-settings-and-server/SKILL.md` — **done.** Split
  the old `/settings` (linked server + friends) into a real
  profile/preferences page (`/settings` — display name, timezone, default
  visibility, notification toggles, guarded account deletion) and a
  separate `/server` page (linked server card + DB-backed, user-editable
  multi-channel bot config, replacing the single `DISCORD_ANNOUNCEMENT_CHANNEL_ID`
  env var for the per-request auto-announce path only — see Server page
  above for the deliberate gap where `bot.rs`'s gateway handler still
  isn't hot-reloaded). The "Sync friends" button that used to live on
  `/settings` moved to the `/friends` directory page instead of being
  dropped, since removing it from `/settings` without adding it anywhere
  else would have silently taken away the only way to trigger a sync.
- `.claude/skills/mockup-availability/SKILL.md` — **done**, partially:
  `services::availability` (pure interval logic, heavily unit-tested, plus
  `free_users_now`/`week_availability` on top of it) and
  `handlers::availability` (`GET /api/availability/friends-now`,
  `GET /api/availability/week?with=<friend_id>&week_start=<ISO date>` -
  both scoped to the caller's actual friends, `week` 400s otherwise so you
  can't probe a stranger's calendar by guessing a user id). "Free" is
  day-granularity (matches the mockup's weekly strip - one cell per day,
  not an hourly grid), not "any event at all" - `pending`/`declined`
  participation doesn't count as busy. Wired into the friends directory
  (a live "Free now" pill next to `noteFor`'s existing shared-event text)
  and friend detail (`/friends/[id]`'s "Free this week" strip, finally
  filled in - it was explicitly left out when that page was built).
  The Calendar screen's own "Free tonight" bar and "Propose a time" button
  were left for a later pass — **now done**, see `mockup-calendar-redesign`
  in the mobile/responsive roadmap below.

Each skill file is a concrete, runnable playbook (schema sketches, file
paths, endpoint shapes, test plan) — treat "run `.claude/skills/mockup-*`"
as a real, actionable request, not just documentation to reference.

## Mobile mockup + responsive design — roadmap

2026-09-15 the user added 13 mobile screens (`Friends Calendar Mobile.dc.html`,
402×874 reference) and a shared `MobileTabBar.dc.html` component to the same
Claude Design project, and asked for (a) any desktop screen from the design
that wasn't fully built yet — the Calendar screen's "Free tonight" bar/peek
panel, specifically — and (b) every screen made responsive to match the
mobile mockup. Confirmed via `grep` that **zero** responsive styling
(`@media`, `sm:`/`md:` Tailwind prefixes) existed anywhere in `desktop/src`
before this. Split into 10 skills (same tiering approach as the earlier
mockup work); 2 executed so far, 8 written and ready. Breakpoint convention
for all of them: Tailwind's `md:` (768px), sidebar/desktop layout at `md:`
and up, bottom-tab-bar/mobile layout below it — comfortably clears the
mockup's 402px reference width, so no narrower `sm:` tier was added
speculatively.

- `.claude/skills/mockup-calendar-redesign/SKILL.md` — **done.** Backend-free
  (`GET /api/availability/friends-now`/`week` already existed but were never
  wired into the Calendar screen). `Calendar.svelte` gained a Free-tonight
  bar (`api.getFreeFriendsNow()` + `api.getFriends()`, avatar stack, "N
  friends have nothing on") and functional filter chips (All/Going/Awaiting/
  Mine — client-side over the already-fetched `events`, no new endpoint).
  `EventPeekPanel.svelte` (new, `organisms/`) is a persistent 296px side
  `aside` showing the selected event with inline Going/Maybe/Can't
  (`api.updateParticipation`, mirrors `EventDetailsModal.svelte`'s
  `handleStatusChange`) — it **replaces** month view's old hover-tooltip-only
  interaction and week/day's modal-on-click for now (an always-visible
  panel, not a hover/click popup); `EventDetailsModal.svelte` itself is kept
  unused-but-not-deleted, since `mockup-responsive-calendar` reuses it as
  the mobile bottom sheet. "Propose a time" and the header's "+ New Event"
  now open the *same* `CreateEventModal` instance — `showCreateModal` was
  lifted out of `CalendarHeader.svelte` (which used to own it) into
  `Calendar.svelte`, with `CalendarHeader` taking a new `onNewEvent`
  callback prop instead.
  - **Bug caught by this skill's own tests, not shipped**: `matchesFilter`
    read `activeFilter` from its enclosing closure, but the `$: filteredEvents
    = events.filter(matchesFilter)` reactive statement only saw `events` and
    the `matchesFilter` *reference* as dependencies — Svelte's reactive-`$:`
    dependency tracking is static (identifiers textually present in the `$:`
    statement itself), not a trace of what a called function transitively
    reads. Clicking a filter chip silently did nothing. Fixed by passing
    `activeFilter` as an explicit argument referenced directly in the `$:`
    line. A second, related instance: `eventsForDay` was a plain (non-reactive)
    function closing over `filteredEvents` — `MonthView`/`WeekView`/`DayView`
    receive it as a prop, and a child component only re-invokes a function
    prop when *the prop's own reference* changes, not when something the
    closure reads changes underneath it, so the grid never updated after a
    filter change even once `filteredEvents` itself was correct. Fixed by
    declaring `eventsForDay` with `$:` too, so its reference changes whenever
    `filteredEvents` does. Both were caught by `Calendar.test.ts`'s filter
    test failing, not by inspection — a concrete argument for the "always add
    tests" policy paying for itself.
  - Also (a test-tooling gotcha worth knowing about, not an app bug): calling
    `render(Calendar, { events: [...] })` in a test silently drops the
    `events` prop — `@testing-library/svelte`'s `render()` second argument is
    Svelte's own mount-options object (`target`/`anchor`/`props`/`events`/
    `context`/`intro`), and a prop literally named `events` collides with
    Svelte's own `events` mount option. Any component with a prop named one
    of those six words needs `render(Component, { props: { events: [...] } })`
    (the explicit wrapper), not the flat shorthand other tests in this repo
    use for differently-named props.
- `.claude/skills/mockup-responsive-shell/SKILL.md` — **done.**
  `BottomTabBar.svelte` (new, `templates/`) mirrors `MobileTabBar.dc.html`'s
  5 tabs — Calendar (`/`), Friends (`/friends`), Hub (`/announcements`),
  Alerts (`/notifications`, badge = `$unreadNotificationCount`), Me
  (`/settings`) — note this is a **different** 5 than `Frame.svelte`'s
  desktop sidebar `navItems` (which also lists `/server` on its own): the
  mobile mockup folds "Discord server" under the "Me" tab instead of giving
  it a 6th tab, reached by drilling in from `/settings`
  (`mockup-responsive-settings-and-server`'s job, not built yet).
  `Frame.svelte` hides the sidebar (`hidden md:flex`) and shows
  `<BottomTabBar>` (`md:hidden`, fixed to the viewport bottom) below `md:`;
  `<main>` gets bottom padding below `md:` so content doesn't sit under the
  fixed bar. Route content itself is unchanged by this skill — shell only.
- `.claude/skills/mockup-responsive-calendar/SKILL.md`,
  `mockup-responsive-friends/SKILL.md`,
  `mockup-responsive-add-friends/SKILL.md`,
  `mockup-announcement-thread/SKILL.md`,
  `mockup-responsive-announcements/SKILL.md`,
  `mockup-responsive-notifications-and-rsvp/SKILL.md`,
  `mockup-responsive-settings-and-server/SKILL.md`,
  `mockup-responsive-create-event/SKILL.md` — written, not yet executed.
  The user was asked up front (2026-09-15) and confirmed: announcement
  replies should be **real** (posted back to Discord via a new backend
  endpoint, not a read-only thread view — `mockup-announcement-thread`),
  mobile event creation should be a **real two-step wizard** (not just a
  responsively-stacked single form — `mockup-responsive-create-event`), and
  notification cards **should** gain inline Going/Maybe/Can't for event
  invites (`mockup-responsive-notifications-and-rsvp`) — these three are
  genuine product-scope decisions baked into their skill files already, not
  open questions left for whoever runs them next.

## `desktop/` (SvelteKit + Tauri)

```
desktop/
├── src-tauri/                 # Tauri Rust shell — part of the Cargo workspace
│   ├── Cargo.toml             # crate "desktop" / lib "desktop_lib"
│   ├── src/{main.rs,lib.rs}
│   ├── capabilities/, gen/, icons/
│   └── tauri.conf.json
├── src/
│   ├── routes/
│   │   ├── +layout.svelte, +page.svelte    # root: login screen or CalendarView
│   │   ├── settings/+page.svelte           # profile/preferences + account deletion, see above
│   │   ├── server/+page.svelte             # linked Discord server + bot channel config, see above
│   │   ├── announcements/+page.svelte      # Discord channel message mirror, see above
│   │   ├── friends/                        # directory (+page.svelte, incl. "Sync friends") + detail ([id]/+page.svelte) + add/+page.svelte (requests), see mockup roadmap below
│   │   └── notifications/+page.svelte      # see mockup roadmap below
│   ├── lib/
│   │   ├── api.ts             # fetch wrapper, JWT storage in localStorage
│   │   ├── stores.ts          # user/isAuthenticated/isLoading + unreadNotificationCount (shared by Header's bell and Frame's sidebar badge)
│   │   ├── types.ts
│   │   ├── actions/clickOutside.ts
│   │   ├── utils/{dateUtils.ts,tooltipUtils.ts}
│   │   └── components/
│   │       ├── CalendarView.svelte, CreateEventModal.svelte,
│   │       │   EventCardImpl.svelte, LoginScreen.svelte
│   │       ├── atoms/          # Button, Avatar, CalendarDay, CalendarEvent,
│   │       │                   # TimeSlot, ViewSwitcher, event/ subfolder (Event,
│   │       │                   # EventCard, CompactEvent, DetailedEvent, ...)
│   │       ├── molecules/      # CalendarHeader, EventList, EventTooltip,
│   │       │                   # ModalContainer, ProfileMenu/, TimedEvent,
│   │       │                   # LinkedServerCard, EventRsvpCard, AnnouncementPostCard
│   │       ├── organisms/      # DayView, WeekView, MonthView, Header,
│   │       │                   # EventDetailsModal, EventPeekPanel, BlurModal
│   │       └── templates/      # Calendar, Frame, BottomTabBar, ViewButton
│   └── test/stories/           # Storybook stories (atoms + ProfileMenu)
├── .storybook/                 # Storybook + SvelteKit config
├── build/                      # yarn build output, gitignored (`/build` in desktop/.gitignore) — not committed
└── static/
```

Atomic-design component layout (atoms → molecules → organisms → templates).
Storybook is wired up for the atoms and the ProfileMenu molecule only; most
molecules/organisms/templates have no stories yet.

## Terraform / GitHub Actions / Discord bot — real vs. discussed-only

All three exist in the repo, at different levels of completeness:

- **Terraform** (`terraform/`): real, and now fixed up (2026-09-15) rather
  than just present. `main.tf` provisions a `proxmox_lxc` container, plus
  `cloudflare.tf` (DNS), `ssl.tf`, `provisioning.tf`, `outputs.tf`,
  `variables.tf`, and templates for nginx/systemd/env. This matches
  `Makefile`'s `init/plan/apply/destroy/ssh/logs/status/update` targets and
  the `scripts/` helpers (`ssh.sh`, `logs.sh`, `status.sh`, `update.sh`,
  `backup.sh`). The `feat(terraform)` branch is fully merged (identical to
  `master`) — that branch can be deleted. Real bugs found and fixed in this
  pass: `templates/systemd.service`'s `ExecStart` pointed at a binary named
  `friends-calendar` — the actual binary (from `backend/Cargo.toml`'s
  package name) is `rust-friends-calendar`, so the service could never
  have started after a real build; `templates/nginx.conf` set its own CORS
  headers on top of the backend's own `CorsLayer` (`main.rs`), which gets a
  response rejected by browsers for carrying `Access-Control-Allow-Origin`
  twice — removed, the backend owns CORS exclusively; `scripts/update.sh`'s
  rsync had no `--exclude '.env'`, so running it would have overwritten the
  VPS's real `.env` with whatever `.env` sits in the local working tree
  (see Secrets note below — a real one exists there); `scripts/backup.sh`
  referenced a `proxmox_host` Terraform output that didn't exist in
  `outputs.tf` (added); `env.tpl`/`variables.tf` never provisioned
  `DISCORD_GUILD_ID`, needed since this session's friend-sync/
  `/api/discord/config`/digest work (added). See `docs/deployment.md` for
  the full provisioning + deploy flow, including why Terraform apply is
  deliberately a manual step, not run from CI.

- **GitHub Actions** (`.github/workflows/`): `ci.yml` and `cd.yml`, both
  fixed/rewritten 2026-09-15 — the previous `ci.yml`/`cd-staging.yml`/
  `cd-production.yml` never actually ran successfully (see git history for
  what was wrong: wrong branch names throughout — `main`/`develop` instead
  of this repo's actual `master`; `ci.yml`'s frontend job targeted a
  nonexistent `frontend/` directory with `npm` instead of the real
  `desktop/` with `yarn`; both CD workflows ran `terraform apply
  -auto-approve` on every push against a Terraform setup with no remote
  state backend, which would either collide with existing infra or lose
  track of it entirely between runs, and even if that worked, the
  provisioning `null_resource` has no `triggers`, so a re-`apply` was
  never actually going to redeploy new code anyway).
  - `ci.yml`: fmt/clippy/test/build for the backend, test/check/build for
    `desktop/` (now actually pointed at the right directory and package
    manager), `cargo audit`. Triggers on `master` only.
  - `cd.yml`: triggers via `workflow_run` once `ci.yml` succeeds on
    `master` — builds the release binary in a `rust:1-bookworm` container
    (glibc-matched to the container's `debian-12-standard` template, since
    plain `ubuntu-latest` is newer and produces a binary that won't run
    there), then ships just the binary over SSH and restarts the systemd
    unit. No Terraform involved — see `docs/deployment.md`.

- **Discord bot**: real, and merged into `master` since 2026-09-14
  (`backend/src/bot.rs` + `services/discord_announcement.rs`). See below.

### Discord bot (`bot.rs`, `services/discord_announcement.rs`)

Originated on `feat(DiscordBot)`, merged into `master` via `6047c67`.

- `backend/src/bot.rs` — a `serenity`-based Discord gateway bot
  (`DiscordBot::start`), listens for `reaction_add`/`reaction_remove` in the
  announcement channel, reacts to a ✅ check-mark emoji to track RSVPs
  (creates/finds the reacting user, marks them `accepted`/`declined` on the
  matching `calendar_events` row via `discord_message_id`).
- `backend/src/services/discord_announcement.rs` — `DiscordAnnouncer`,
  posts a formatted event announcement message via the bot HTTP token, adds
  a ✅ reaction, and spins up a Discord thread under the message for
  discussion.
- `main.rs` spawns the bot as a background tokio task at startup if
  configured; `handlers::calendar::create_event` auto-announces new events
  to Discord if configured (both read `AppState.discord_bot_token` /
  `discord_announcement_channel_id`, not raw env vars — see below).
  `POST /api/events/:id/link-discord` (`handlers::calendar::link_discord_message`)
  exists for manually linking an event to an existing Discord message
  instead.

**What changed from the original branch during the merge** (see `6047c67`'s
full commit message for the complete list):
- Bot startup no longer `.expect()`s `DISCORD_BOT_TOKEN`/
  `DISCORD_ANNOUNCEMENT_CHANNEL_ID` and crashing the whole backend if
  unset — same "degrade gracefully" treatment as friend sync. Both are now
  read once into `AppState` (`config.rs`: `discord_bot_token: Option<String>`,
  `discord_announcement_channel_id: Option<u64>`) instead of each call site
  (`main.rs`'s bot spawn, `create_event`'s auto-announce) re-reading env
  vars independently — they can no longer disagree about whether Discord is
  configured.
- `bot.rs` was converted from the compile-time-checked `sqlx::query!` macro
  to the runtime `sqlx::query`/`query_as` style used everywhere else in
  this codebase. `query!` needs a live, schema-matching `DATABASE_URL` at
  **compile** time (not just runtime) — on a fresh clone/CI without a
  pre-seeded DB, `cargo build` would simply fail. Also dropped an unused
  `DiscordBot::new()` constructor and fields that nothing called (`Handler`
  is constructed directly in `start()`).
- `update_event` was missing `price`/`link` wiring that `create_event`
  already had — `UpdateEventRequest` declared the fields but nothing read
  them (dead-code warning caught this). Fixed on both backend
  (`services::calendar::update_event`) and the matching
  `desktop/src/lib/api.ts` `updateEvent()` params.
- Migration renumbering + `IF NOT EXISTS` — see the migrations section above.

**Still true / not done:**
- No automated tests for `bot.rs` / `discord_announcement.rs` (they need a
  live Discord gateway connection to exercise meaningfully; the friend-sync
  tests show the pattern for mocking Discord's REST API with `wiremock` if
  someone wants to test `discord_announcement.rs`'s HTTP calls that way).
- Requires the bot application's "Server Members Intent" enabled in the
  Discord developer portal (same requirement friend sync has, for the
  `GUILD_MEMBERS` gateway intent `bot.rs` requests).

## Secrets note

`backend/.env` is present in the working tree (gitignored — `.gitignore`
excludes `.env`/`.env.local`) and contains real-looking
`DATABASE_URL`, `DISCORD_CLIENT_ID/SECRET`, `DISCORD_BOT_TOKEN`,
`DISCORD_ANNOUNCEMENT_CHANNEL_ID`, `JWT_SECRET`. Don't print, log, or commit
its contents.

## Summary of things to clean up

1. Root `cargo.toml` → rename to `Cargo.toml` (case bug, silently works only
   on case-insensitive filesystems).
2. Remove `desktop/package-lock.json` (mixed npm/yarn artifacts; yarn is the
   one actually used).
3. Delete the now-fully-stale branches: `feat(Event)`, `feat(Storybook)`,
   `feat(terraform)` (local-only), `origin/dev/refacto`, `feat(Calendar)`,
   `feat(DiscordBot)` — all 0 ahead of `master` as of 2026-09-14, nothing
   left to merge from any of them.
4. `bot.rs`/`discord_announcement.rs` have no automated tests (see Discord
   bot section above for why and what a first pass could look like).
5. CI/CD is now fixed and real (see the Terraform/GitHub Actions section
   above and `docs/deployment.md`), but the pipeline can't deploy anything
   until a person does the one-time manual setup `docs/deployment.md`
   describes (create the `production` GitHub Environment, generate a
   deploy SSH key, add the four `DEPLOY_*`/`PROD_DOMAIN` repo secrets, and
   confirm what's actually running on the Proxmox host today via `pct
   list` before pointing a deploy pipeline at it).
