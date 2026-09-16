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
  at repo root, resolver `"2"`. The manifest was committed as `cargo.toml`
  (lowercase) until 2026-09-16; **it is now `Cargo.toml`**, and that rename
  fixed real breakage rather than being cosmetic. On case-sensitive Linux
  (the CI runners, and the Proxmox container) cargo could not see a file
  named `cargo.toml`, so `backend` resolved as a *standalone* package there
  while resolving as a *workspace member* on macOS — two different dependency
  sets from the same commit. Consequences that were actually biting:
  - CI built against a stale, committed `backend/Cargo.lock` (last touched
    Nov 2025) instead of the root lockfile. That file has since been deleted;
    the root `Cargo.lock` is the single source of truth. Don't re-add a
    member-level lockfile — cargo ignores it for workspace builds, so it can
    only ever drift.
  - Build output location differs: inside the workspace the binary lands in
    the **shared root `target/`**, not `backend/target/`. `.github/workflows/cd.yml`
    depends on this path, and `terraform/templates/systemd.service` expects
    `/opt/friends-calendar/target/release/rust-friends-calendar`.
  - Build only the one package you want (`cargo build -p rust-friends-calendar`).
    A bare `cargo build` at the root now also builds the `desktop/src-tauri`
    member, which needs GTK/WebKit system libraries that no server or CI
    container has.
  - The VPS is unaffected: provisioning copies `backend/` alone to
    `/opt/friends-calendar`, so it builds there as a standalone crate.
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
  `default_visibility` **is** applied as a real default as of 2026-09-16:
  `services::calendar::create_event` looks it up when the request omits
  `visibility` (an explicit value in the request still wins — the
  preference is a default, not an override), and
  `CreateEventModal.svelte` preselects it from the `user` store so the
  form shows what will actually happen.
  ⚠️ **This endpoint could not accept its own frontend's payload until
  2026-09-16.** `Visibility` carried `#[sqlx(rename_all = "lowercase")]`
  but no serde rename, so JSON expected `"Friends"` while the settings
  page sent `"friends"` — every save 422'd. See the enum-casing note under
  "Testing" for why no test caught it.
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
- Notification toggles are **enforced** as of 2026-09-16, inside
  `services::notifications::create` rather than at each trigger call site,
  so every trigger — including ones added later — is gated by
  construction and a new caller cannot forget to check. The mapping lives
  in `preference_column_for(kind)`:
  `event_invite` → `notify_event_invites`, `rsvp_change` →
  `notify_rsvp_changes`, `announcement` → `notify_announcements`.
  - `friend_request`/`friend_accepted` are deliberately **ungated** — they
    have no preference of their own, and reusing `notify_event_invites`
    would mean switching off *event invites* silently killed *friend
    requests* too. Give them their own column if they should be
    switchable.
  - `notify_weekly_digest` is mapped to nothing and its checkbox has been
    **removed** from `/settings`, replaced by a line pointing at `/server`.
    The digest is one message to one shared channel
    (`services::digest`, gated by the guild-level
    `discord_bot_config.digest_enabled`), so there is no per-user delivery
    for a per-user preference to filter — the control could only ever look
    functional. The column still exists and is still round-tripped by
    `PATCH /api/auth/me`, so no data is lost if per-user delivery ever
    arrives.

### Server page (`/server`) — linked Discord server + bot channel config

Split out of the old `/settings` so "which Discord server, and which
channels does the bot use" has its own page, separate from personal
profile/preferences. ⚠️ **Superseded in part by `/servers`** (plural — see
the multi-server section below). This page still shows the one guild named
by `DISCORD_GUILD_ID` and owns the digest toggle; `/servers` is the one that
lists every server the bot is actually in and hands out the invite link.

- `GET /api/discord/server` (`handlers::discord::get_linked_server`,
  `services::friends::get_linked_server_info`) — unchanged from before,
  `GET /guilds/{id}` on the bot token, returns
  `{ id, name, icon_url, approximate_member_count }`.
⚠️ **`/server` has one channel field, not the mockup's three.**
`events_channel_id` and `reminders_channel_id` were dropped in migration
013: nothing ever read either. Event announcements use the announcements
channel, and reminders go into each event's own Discord thread
(`services::reminders`). Three inputs where one works is worse than one -
it invites configuring behaviour that will never happen. If a dedicated
destination is ever wanted, adding a column back is trivial.

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

✅ **The long-standing "visibility does nothing" limitation is fixed as of
2026-09-16** (`.claude/skills/event-visibility-listing/SKILL.md`). For the
record, since it stood for most of this project's life: `GET /api/events`
used to be participant-only, so a `public` event reached exactly the people
a `private` one would, while the single-event `GET /api/events/:id` *did*
check `OR e.visibility = 'public'` — the two endpoints disagreed.

`services::calendar::list_user_events` now matches an event if **any** of:
you're a participant, it's `public`, or it's `friends` and its creator is a
friend of yours. Points worth knowing before changing it:

- **`friends` means any row in `friendships`** — both `'discord_guild'`
  (guild-synced) and `'friend_request'` (explicitly accepted). That's a
  deliberate product decision, confirmed with the user, not an
  implementation accident. It does mean a `friends` event is visible to
  everyone in the linked guild, which is wider than "people I approved".
- **`get_event_with_participants` had to learn the same rule.**
  `list_user_events` calls it per event to build participant lists, so any
  event that function rejects is silently dropped from the listing no
  matter what the listing query matched. Keep the two in sync.
- **Declining wins over every visibility route.** The declined filter moved
  off the JOINed participant row onto a row-scoped `NOT EXISTS`, otherwise
  an event you turned down reappears through the public/friends clause.
  There's a test for exactly this.
- **`EXISTS` replaced `JOIN ... DISTINCT`** — an event can qualify by more
  than one clause at once, and `DISTINCT` was the only thing stopping it
  from listing twice.
- **New `is_participant` on `EventWithParticipants`.** `my_status` can't
  tell "invited, hasn't answered" from "not invited, just visible" — both
  are absent. Anything that reasons about the caller's relationship to an
  event needs this flag, and three places already did: the calendar's
  "Awaiting my answer" filter (which would otherwise sweep up every public
  event in the guild), `EventPeekPanel`'s RSVP row, and the "Shared events"
  / "Next:" lists on `/friends/[id]` and `/friends` (which would otherwise
  show events only the *friend* is in).

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
  - ⚠️ **Always give `beforeEach` a block body**, never the concise arrow
    `beforeEach(() => someMock.mockReset())`. Vitest treats a function
    *returned* from `beforeEach` as a teardown hook, and `mockReset()`
    returns the mock — so the concise form makes vitest **call the mock**
    after every test in that file. It's invisible while the mock resolves,
    and shows up as a bogus unhandled rejection the moment one test makes it
    reject: `servers/page.test.ts`'s failing-load test failed with the right
    error for entirely the wrong reason, and the page code it accused was
    correct. Found by logging `new Error().stack` from inside
    `mockImplementation` and seeing a second call whose stack came straight
    out of `callCleanupHooks` with no Svelte frames in it.
- **Layout tests (`desktop/e2e/`, Playwright + Chromium, added 2026-09-16).**
  Run `yarn test:layout` from `desktop/`; `yarn test:layout:ui` for the
  interactive runner. Gated in CI by its own `Layout Tests` job.
  - **Why a third tier exists:** happy-dom computes *no layout*. A div with
    an explicit `width: 402px` reports `getBoundingClientRect()` 0×0,
    `offsetWidth` 0, `scrollWidth` 0, and `window.innerWidth` is a fixed
    1024 unrelated to any breakpoint. So every vitest test here is a
    DOM-*structure* test, `hidden md:block` is just a string of characters
    to it, and the whole responsive pass was **structurally unverifiable**
    at that tier rather than merely untested.
  - **What belongs here:** assertions a machine can make without judgement -
    horizontal overflow, occlusion by the fixed tab bar, whether a
    breakpoint actually switches, tap-target size. Three viewport projects
    (`mobile-402` matching the mockup's own reference width, `tablet-768`,
    `desktop-1280`), so a failure names the width it failed at.
  - ⚠️ **What must NOT go here: pixel-diffed screenshot baselines.** Font
    rendering differs between a macOS dev machine and CI's Linux container,
    so committed baselines fail in CI immediately and permanently.
    Screenshots are written to `e2e/screenshots/<project>/` (gitignored,
    uploaded as a CI artifact) purely to be *looked at* - they are never a
    gate. The overflow test captures its screenshot **before** asserting, so
    a failing layout still leaves a picture behind.
  - No backend and no database: `e2e/fixtures.ts` intercepts `**/api/**` and
    serves fixtures, and seeds `localStorage.jwt_token` so pages render
    authenticated instead of falling back to `LoginScreen`. Fixture content
    is deliberately *long* (long titles, an unbroken URL, a two-digit badge)
    - an empty page never overflows, so short fixtures would make every
    assertion pass while proving nothing.
  - `vite.config.js` excludes `e2e/**` from vitest; without it vitest's
    default include pattern picks up the Playwright spec and fails on the
    `@playwright/test` import.
  - Selectors use `data-testid="sidebar"` / `data-testid="bottom-tab-bar"`
    because `EventPeekPanel` is also an `<aside>`, so the tag alone can't
    identify the shell.
  - **Three real bugs on its first run**, none of which any existing test
    could have caught: an unbroken URL in `EventRsvpCard` widened
    `/friends/[id]` by 61px at 402px (a URL has no spaces so it can't wrap,
    and a flex child's default `min-width: auto` refuses to shrink - fixed
    with `min-w-0` + `truncate`); bottom-tab targets were 37px against the
    44px floor (fixed by moving the bar's bottom safe-area padding onto the
    buttons, which renders identical pixels but counts toward the hit area);
    and `src/app.html` carried a stale `<link href="./app.css">` for a file
    that doesn't exist in the build, 404ing on every page load since
    `app.css` is bundled through `+layout.svelte`'s import.
  - **Two more found by *reading* a screenshot**, which is the part
    assertions can't do: full weekday names collided in the month grid's
    ~50px columns at 402px (they overlapped rather than widening the page,
    so no overflow assertion fired - `WeekdayHeader` now shows the
    abbreviated form below `md`), and the header avatar was broken for
    **every** user because `Frame.svelte` had `let avatarUrl = ...` instead
    of `$:` - evaluated once at init while `$user` is still null, so it
    froze at `.../avatars/undefined/undefined.png` forever. The username
    beside it looked right because that reads the store reactively. That is
    the third instance of this exact static-`$:`/stale-closure family in
    this codebase; see the `matchesFilter`/`eventsForDay` note under
    `mockup-calendar-redesign`.

⚠️ **Test across the JSON boundary, not just up to it.** Both `Visibility`
and `ParticipationStatus` shipped with `#[sqlx(rename_all = "lowercase")]`
and no serde rename, so their JSON form was `"Friends"`/`"Accepted"` while
the entire frontend sent and compared lowercase. Every RSVP click and every
settings save 422'd, and reading back was broken too (`my_status` came back
`"Accepted"`, so `Calendar.svelte`'s `my_status === 'accepted'` never
matched). **Every test on both sides passed the whole time**, because none
of them crossed the boundary: the backend service tests build
`Visibility::Public` in Rust, the Svelte component tests build fixtures in
TypeScript, and the one functional test for `PATCH /api/auth/me` only ever
sent `display_name`. When a type is shared across the wire, at least one
test must send the *literal payload the client sends* through the real
router — see `handlers::profile::tests::patch_me_accepts_the_payload_the_settings_page_actually_sends`
and `handlers::calendar::tests::update_participation_accepts_the_lowercase_status_the_client_sends`.

- `cargo clippy --all-targets --all-features -- -D warnings` is **clean as of
  2026-09-16** and gates CI — any new lint is yours, fix it rather than
  adding to a pile that no longer exists.
- `yarn run check` (not `yarn check`, which is yarn's own unrelated built-in
  command) is **also clean as of 2026-09-16** — the 4 long-standing errors
  (an unused `@ts-expect-error` in `vite.config.js`, three implicit-`any`s in
  `ViewSwitcherStory.svelte`) were fixed because CI runs this step and would
  otherwise have failed the Frontend job the moment the install was
  repaired. One non-fatal warning remains (`TimeSlot.svelte`'s unused `hour`
  export); svelte-check exits 0 on warnings.

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
- `.claude/skills/mockup-responsive-notifications-and-rsvp/SKILL.md` —
  **done 2026-09-16.** `/notifications` cards for `event_invite`
  notifications that still carry an `event_id` get inline Going/Maybe/Can't
  (`api.updateParticipation`), and answering marks the notification read in
  the same tap — acting on it *is* acknowledgement. A failed RSVP shows an
  error scoped to that one card (`rsvpErrors` keyed by notification id)
  rather than blanking the list: the event may have been deleted, or you
  removed from it, since the row was written. Notifications are grouped
  Today/Earlier by `groupByRecency` in `lib/utils/notificationUtils.ts` —
  extracted and unit-tested because "today" means the same *local calendar
  day*, not "within 24 hours" (23:00 yesterday is yesterday to a reader).
  Empty groups are omitted so no heading ever renders with nothing under it.
- `.claude/skills/mockup-responsive-create-event/SKILL.md` — **done
  2026-09-16.** `CreateEventModal` gains a two-step wizard below `md:`
  (step 1 when/where, step 2 invite + Discord preview); desktop is
  unchanged. `step` is pure UI state and **both steps stay mounted**,
  gated by `hidden md:block` rather than `{#if}` - that's what lets
  desktop ignore `step` entirely and "‹ Back" return to filled-in fields
  without any save/restore logic. A test asserts both paths send an
  identical payload, so they can't drift.
  - Editing deliberately stays single-scroll even on mobile: step 2 is the
    invite picker and the preview, and `PUT /api/events/:id` manages
    neither.
  - **New `POST /api/events/announcement-preview`** renders the real
    announcement for an unsaved event. It exists so the preview can't drift
    from what's actually posted: `format_event_message` was extracted from
    `DiscordAnnouncer` into a free `pub fn` and both call it, with a test
    asserting the endpoint's output *equals* the formatter's. The panel
    shows the raw message source (Discord markdown, `<t:…>` timestamps)
    because that is literally what gets sent - Discord is what renders it,
    and faking that rendering client-side would misrepresent it.
- `.claude/skills/mockup-responsive-calendar/SKILL.md` — **done
  2026-09-16, with one deliberate deviation.** The skill planned to revive
  `EventDetailsModal` as the mobile bottom sheet. It was instead **deleted**,
  and `EventPeekPanel` made responsive: a bottom sheet below `md:`
  (`fixed inset-x-0 bottom-0`, `anim-sheet`), the 296px side panel from
  `md:` up. The skill predates `event-edit-flow` and
  `event-visibility-listing`; by now `EventDetailsModal` carried the
  pre-mockup green/red/yellow status palette, `confirm()`/`alert()` dialogs,
  no edit affordance and no `is_participant` handling, so reusing it would
  have reintroduced all four on mobile only. One detail surface, two
  placements.
  - New `'list'` `ViewType` + `AgendaView.svelte`: upcoming events grouped by
    day, offered at every width but the thing that makes the calendar usable
    at 402px (a 7-column grid gets ~55px per day). Day/Week are `hidden
    md:block` in `ViewSwitcher` for the same reason.
  - Grouping lives in `dateUtils.groupEventsByDay` and is unit-tested: it
    drops already-started events (a list has no month/week anchor, so
    "upcoming" is the only sensible scope), sorts chronologically, and groups
    by *local* day so a 23:00 event lands where the reader would expect.
- `.claude/skills/mockup-responsive-friends`, `-add-friends`,
  `-announcements`, `-settings-and-server` — **done 2026-09-16** in one
  layout pass (they're the same kind of change: stack below `md:`, tighten
  side padding, stop things overflowing at 402px). Two carry real behaviour
  rather than CSS:
  - **`/server` was unreachable on mobile.** The sidebar that links to it is
    `hidden` below `md:`, and `BottomTabBar` folds Server under "Me" without
    giving it a tab. `/settings` now has an explicit push-through row
    (chevron) to `/server`, and `/server`'s back-link points at `‹ Me`
    rather than the calendar.
  - **Only the *first* pinned announcement gets the inverted card**
    (`AnnouncementPostCard`'s `featured` prop), not every pinned one - a
    column of dark cards would defeat the point of singling one out. Tested
    both ways.
- `.claude/skills/mockup-announcement-thread/SKILL.md` — **done
  2026-09-16.** Real replies, posted back into the announcement's Discord
  thread, not a read-only view.
  - `services::discord_feed` gained `fetch_or_create_thread` and
    `fetch_replies`, both `reqwest`-based with `base_url` as a parameter so
    they stay wiremock-testable — deliberately *not* serenity, which would
    have broken that (see the skill and `add-tests`).
  - **A thread started from a message is addressed by that message's id.**
    `fetch_or_create_thread` GETs the message first to see whether a thread
    exists, and falls back to the message id when the payload doesn't spell
    the thread id out. Same fact the reminder feature relies on.
  - `GET /api/announcements/:id/replies` and `POST /api/announcements/:id/reply`.
    Both resolve the local UUID to `discord_message_id`/`channel_id`
    **server-side** — `AnnouncementPostInfo` still exposes neither, and a
    test asserts that.
  - Replies are always a **live fetch**, never
    `announcement_posts.reply_count`, which is whatever the last sync saw.
    Posting deliberately does *not* bump that column: a locally-incremented
    count would be a second source of truth drifting from Discord. `POST`
    returns the refreshed thread so the client needs no second round-trip
    and no optimistic guess.
  - New `/announcements/[id]` route, built mobile-first (docked composer
    below `md:`, inline above). A failed send keeps the draft.
- `.claude/skills/mockup-responsive-calendar/SKILL.md` (original entry),
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

## Motion system (`desktop/src/app.css`)

2026-09-15, same pass as the mockup work above: the mockup's own `<style>`
block (identical in both `Friends Calendar Mockups.dc.html` and
`Friends Calendar Mobile.dc.html`) defines a full motion system - named
`@keyframes` (`om-fade-up`, `om-fade`, `om-pop`, `om-slide-left`,
`om-pulse`, `om-grow`, `om-scrim`, `om-sheet`), a global
`button, a, [role="button"] { transition: ... }` rule, a `button:active`
press-scale, and a `prefers-reduced-motion` override - that had never been
carried over into the app. `app.css` now has all of it, verbatim (same
keyframe names/durations/easings as the mockup, not approximated), plus a
matching set of `.anim-*` utility classes (`anim-fade-up`,
`anim-fade-up-stagger`, `anim-fade`, `anim-pop`, `anim-slide-left`,
`anim-grow`, `anim-pulse-dot`, `anim-scrim`, `anim-sheet`) so components
reference it by name instead of writing raw `animation:` declarations.
`om-sheet`/`anim-sheet` (the mobile bottom-sheet slide-up) has no caller
yet - it's there for whichever `mockup-responsive-*` skill ends up reusing
`EventDetailsModal.svelte` as a mobile sheet, not wired to anything today.

Applied to:
- Per-screen entrance: `Calendar.svelte`'s whole body, and the main
  content wrapper on `/friends`, `/friends/[id]`, `/friends/add`,
  `/announcements`, `/notifications`, `/settings`, `/server` all get
  `anim-fade-up` - plays once when the route mounts, mirroring the
  mockup's per-`sc-if`-block fade-up.
- Staggered lists: friend cards (`/friends`) and `AnnouncementPostCard`
  get `anim-fade-up-stagger` with an inline `animation-delay` keyed to
  list index (matches the mockup's `animation-delay:{{ p.delay }}`
  pattern); notification rows get `anim-slide-left` the same way.
- `EventPeekPanel.svelte`: wrapped in `{#key event.id}` so switching which
  event is selected actually replays `anim-fade` (Svelte would otherwise
  just patch text in place on the same DOM node) - plus `anim-grow` on the
  colored status bar, matching the mockup's `barStyle` growth animation.
- Unread notification dot: `anim-pulse-dot`.
- Modals: `BlurOverlay.svelte`/`ModalContainer.svelte` (the `BlurModal`
  stack `EventDetailsModal` sits on) and `CreateEventModal.svelte`'s own
  hand-rolled backdrop now use `anim-scrim`/`anim-pop`. **What they had
  before was dead code**, not a working-but-different transition: the
  `animate-in`/`fade-in`/`zoom-in-95 duration-200` classes are from the
  `tailwindcss-animate` plugin, which was never installed here (this repo
  is on Tailwind v4's CSS-only `@import 'tailwindcss'` with no plugin
  registered) - so those classes matched nothing and every modal open was
  an instant snap. `EventTooltip.svelte` had the same shape of dead
  transition (`transition-opacity` + `opacity-0`/`opacity-100` toggled on
  an element that's only ever mounted while already visible, so the
  "0" state never actually renders) - replaced with `anim-pop` on the
  tooltip card.

## Multi-server — done (steps 1-5, 2026-09-16)

`.claude/skills/multi-server/SKILL.md` (written 2026-09-16). The user
confirmed the shape: **one `calendar_events` row published to N servers**,
never mirrored copies. Two things make single-server an assumption rather
than a setting, and both are load-bearing:

- **`visibility: friends` would silently widen.** `friendships` come from
  shared guild membership (`source = 'discord_guild'`) and listing treats any
  row as "friend". With several servers that becomes "anyone I share *any*
  server with", so unrelated groups start seeing each other's events. The fix
  is visibility relative to *where an event is published*, and it has to ship
  with the feature, not after.
- **One event = one Discord message is in the schema.**
  `calendar_events.discord_message_id` is reverse-looked-up by `bot.rs` (✅ →
  participant), `services::reminders` (the thread it posts into *is* that
  message id) and `services::discord_feed` (event-tag inference). It becomes
  an `event_publications` child table.

Also new: `user_guilds`. Membership is *derived* into friendships today and
never stored, so there's currently no way to answer "which servers can this
person publish to?".

**Step 1 is done (2026-09-16, migration 014)**: `guilds`, `user_guilds` and
`event_publications` exist, and `calendar_events.discord_message_id`/
`discord_channel_id` are **gone** - the message id now lives on the
publication. `bot.rs` (reaction → RSVP), `services::discord_feed` (event-tag
inference) and `services::reminders` (which thread to post into) all resolve
through `services::guilds` instead. Reminders already post to *every*
publication's thread, so they need no further change when a second server
arrives. Nothing yet chooses more than one server - that's step 5.

⚠️ **Migration 014 can refuse to apply, on purpose.** It backfills the guild
from `discord_bot_config` (the only place the database records it -
`DISCORD_GUILD_ID` is an env var a migration can't read), and if any
announced event still has no publication row afterwards it raises rather
than dropping the columns. Losing a `discord_message_id` is silent and nasty:
RSVP-by-reaction stops resolving and reminders lose their thread. Before
deploying, check the target database has a config row:

```sql
SELECT count(*) FROM discord_bot_config;   -- must be >= 1 if any event has been announced
```

If it's 0 and events have been announced, insert a `guilds` row for the
deployment's `DISCORD_GUILD_ID` first. `main.rs` also calls
`services::guilds::ensure_guild` at startup, but that runs *after*
migrations, so it can't rescue this particular case.

Verified by hand on two scratch databases (the `#[sqlx::test]` harness only
ever migrates empty ones): one seeded with a config row and an announced
event, where everything carried across; and one without, where the migration
refused, rolled back all three tables, and left the message id intact.

**Step 2 is done (2026-09-16)**: visibility is now scoped to **where an
event was published**, not to a global friendship set. An event reaches you
if you're a participant, or if it was announced in a server you're in *and*
its visibility allows it there:

- `public` → anyone in a server it was published to
- `friends` → people in a published-to server who are **also** friends
- `private` → participants only, unchanged

Publishing decides reach; friendship only narrows it. Both halves are
necessary and there's a test for each direction - a friend who isn't in the
server doesn't see it, and a server-mate who isn't a friend doesn't either.

⚠️ **An event published nowhere is invisible to non-participants**, whatever
its visibility says. That's deliberate ("announce it nowhere but let
strangers find it" isn't coherent) but it *is* a behaviour change for events
created before the bot was configured - they were genuinely never broadcast.
Creators and invitees still see them.

`list_user_events` and `get_event_with_participants` carry the same clause,
and a test asserts they agree: the listing calls the fetch per event, so a
stricter fetch silently empties the list. That mismatch has bitten once.

**Steps 3+4 are done (2026-09-16)**, merged because step 3 on its own had no
consumer - resolving channels per guild changes nothing while there's one
server, and building it ahead of the invite flow would have been the same
build-before-the-caller mistake clippy caught elsewhere that day.

Step 3 then **collapsed instead of being built**. `bot.rs` used to filter
reactions against a single announcement channel captured at process start,
which is what couldn't survive a new server without a restart. But that
filter was only ever an optimisation: `record_attendance` already returns
`UnknownEvent` for a message we didn't announce. So the filter is *gone*
rather than hot-reloadable - the emoji check discards almost everything, and
what's left costs one indexed lookup on
`event_publications.discord_message_id`. A lookup can't go stale; a cached
channel set can.

Consequences:
- `DiscordBot::start` takes only a token now, and `main.rs` starts the bot
  without `DISCORD_ANNOUNCEMENT_CHANNEL_ID` - it watches every server it's in
  whether or not any announcement channel is configured.
- **Servers register themselves.** The gateway's `guild_create` fires on join
  *and* for every server on reconnect, and records name + icon (the only
  place those come from - nothing asks the user to type them). There is no
  "add server" endpoint: authorising the bot on Discord *is* the action, and
  a parallel callback of our own would just be a second way to get it wrong.
- `GET /api/guilds` returns the servers plus the bot invite URL, built from
  `DISCORD_CLIENT_ID` (now kept on `AppState`). The permissions bitfield is
  spelled out as a sum in `handlers::guilds` rather than pasted as a magic
  number, because checking such a number means decomposing it again.

⚠️ **Not verifiable here**: whether the invite link actually works, and
whether `guild_create` fires as expected, both need real Discord. The DB half
is tested; the gateway half is not.

**Step 5 is done (2026-09-16)** — the picker, and the feature is now usable
end to end:

- `CreateEventRequest.guild_ids: Option<Vec<Uuid>>`, and
  `handlers::calendar::announce_to_selected_servers` loops the announce,
  resolving the channel per guild. One `calendar_events` row, N
  `event_publications` rows, N Discord messages — the shape the user asked
  for, not mirrored events.
- `/servers` (`desktop/src/routes/servers/+page.svelte`) lists what the bot
  is in and links out to the invite URL. The invite is an `<a>`, not a
  button with a handler — authorising happens on Discord, there's nothing to
  submit.
- `CreateEventModal.svelte` grew server chips. **Nothing is selected by
  default** (the user chose this over pre-selecting every server): publishing
  to a server is a broadcast, and a default that broadcasts everywhere is the
  kind of default you only notice after it's wrong. The cost is that a
  `friends`/`public` event with no server chosen reaches nobody but its
  invitees, so the modal shows a `publishedNowhereButShared` warning rather
  than letting that happen silently.
- A server registered by id but never seen by the gateway has no name yet;
  both the page and the picker render the id with a "name appears once the
  bot reconnects" note instead of blank.

**Deliberately not done: editing publications after creation** (agreed with
the user). Un-publishing means deleting a Discord message that people may
have already reacted to — reactions *are* the RSVP record, so deleting one
destroys data. Adding a server later is the easy half, but shipping only
that reads as "publications are editable" when they're half-editable.

⚠️ **Still not verifiable here**: the invite link, `guild_create`, and the
per-guild announce all need real Discord. Everything DB-side is tested; the
gateway and REST halves are not.

## Feature backlog — 2026-09-16 triage

Triaged against the working tree (every claim below was verified by
grep/probe, not carried over from an earlier session's notes). The theme:
**most of what's missing isn't missing UI, it's controls that already
exist and silently do nothing.** Fix those before adding features — a
settings page that can't save undermines trust in every other control.

Run order matters twice: `settings-integrity` builds the preference gate
that `event-reminders` plugs into, and `event-visibility-listing` settles
"who can see this event", which is the same question as "who should be
reminded about it".

1. `.claude/skills/settings-integrity/SKILL.md` — **done 2026-09-16.**
   Turned out worse than the skill described: the casing split affected
   `ParticipationStatus` too, so **RSVP had never worked either** — not
   just settings. Both enums now carry `#[serde(rename_all = "lowercase")]`
   (the DB representation is untouched; the sqlx attribute is independent,
   so no migration), the four-times-duplicated visibility union is now a
   single exported `Visibility` type in `types.ts`, `default_visibility` is
   applied at event creation, and the `notify_*` toggles are enforced
   inside `services::notifications::create`. Original diagnosis, kept
   because the *reason* it went unnoticed still matters: `Visibility` derives `#[sqlx(rename_all = "lowercase")]` but no
   serde rename, so JSON is `"Friends"`; `/settings` sends `"friends"`
   (and `types.ts` types it lowercase), so **`PATCH /api/auth/me` has
   never succeeded** — the page cannot save anything, and the `<select>`
   can't display the loaded value either. `CreateEventModal` gets it right,
   which is why event creation works and the two pages disagree. Verified
   by probe: `to_string(Visibility::Friends)` => `"Friends"`, and
   `from_str("\"friends\"")` is an `Err`. Nothing caught it because the
   `services::profile` tests construct `Visibility::Public` in Rust and
   bypass serde entirely — the fix needs a *functional* test through the
   real router. Same skill then makes `default_visibility` apply at event
   creation and gates the four `notify_*` toggles (today: written by
   `services::profile`, read by nobody) inside
   `services::notifications::create`.
2. `.claude/skills/event-visibility-listing/SKILL.md` — **done 2026-09-16.**
   Decisions taken with the user: `friends` resolves to *both* friendship
   sources, and discovered events render visually distinct (dashed/muted)
   with no RSVP controls rather than looking like events you owe an answer
   on. See the visibility note further up for the mechanics. Original
   entry: `visibility` has no effect on listing. `list_user_events` is participant-only, so a `public`
   event reaches exactly the people who'd see it if it were `private`.
   Note `GET /api/events/:id` *does* check `OR e.visibility = 'public'`, so
   the two endpoints already disagree. Carries real privacy decisions
   (does `friends` mean guild-synced friends or only accepted requests?) —
   the skill flags them rather than guessing.
3. `.claude/skills/event-edit-flow/SKILL.md` — **done 2026-09-16.**
   `CreateEventModal.svelte` is now create-or-edit via one nullable `event`
   prop (null = create), rather than a second form that would drift from
   it; `Calendar.svelte` holds `editingEvent` as the mode switch instead of
   a second boolean, and both paths dispatch a single `saved` event.
   `EventPeekPanel`'s Edit button is live and dispatches the event upward.
   Delete was added to the peek panel too — it had only ever existed in the
   month-view hover tooltip (`EventCardImpl`), so week and day view had no
   way to delete anything; it uses a two-step inline confirm rather than
   `window.confirm()`, matching how the panel reports its other state.
   `Nudge no-answers` is *still* a disabled placeholder on purpose: no
   endpoint pings pending participants, and building one (a Discord DM path
   plus rate-limiting) is its own feature. `EventDetailsModal.svelte` was
   left alone for `mockup-responsive-calendar` to reuse as the mobile sheet.
   New `dateUtils.toDatetimeLocalValue()` converts the API's UTC RFC3339
   into the local `YYYY-MM-DDTHH:mm` a `datetime-local` input requires —
   **not** `toISOString().slice(0,16)`, which is both UTC and silently
   renders blank when rejected.
   Original entry:
   `PUT /api/events/:id` and `api.updateEvent()` both exist with **zero
   callers**, and `EventPeekPanel` ships a permanently `disabled` "Edit"
   button. Also settles the orphaned `EventDetailsModal` (kept alive only
   because `mockup-responsive-calendar` plans to reuse it as the mobile
   sheet) and the currently-unreachable delete path.
4. `.claude/skills/event-reminders/SKILL.md` — **done 2026-09-16.**
   `services::reminders`, modelled on `services::digest` (pure `is_due` +
   `send_due_reminders` + `spawn_reminder_loop`, spawned from `main.rs`).
   Decisions taken with the user: delivered **in-app and into the event's
   existing Discord thread** — *not* to `reminders_channel_id`, which
   therefore remains configurable-but-unused on `/server`.
   - **The thread is addressed by `discord_message_id`.** A Discord thread
     started from a message shares that message's id, so no separate thread
     id is stored. If thread creation failed when the event was announced
     (`discord_announcement` tolerates that with a warning), this POST 404s
     — it's logged and skipped, not fatal.
   - **Several reminders per event**, chosen by the creator (migration 012,
     `event_reminders` - one row per lead time, each with its own `sent_at`).
     An event can nudge a week out *and* a day out *and* an hour out; the
     due query joins the table so each row falls due and is stamped
     independently. `CreateEventModal` offers them as toggle chips.
     - **"No reminder" is now no rows.** Migration 011 needed a `0` sentinel
       because a NOT NULL column always holds *something*; a child table
       represents absence natively, so the sentinel and the
       `lead_minutes > 0` filters it required are gone. `CHECK
       (lead_minutes > 0)` now rejects what used to be meaningful.
     - `set_reminders` replaces the set wholesale and uses `ON CONFLICT DO
       NOTHING`, so a kept lead time **retains its `sent_at`** - editing an
       event's title can't re-notify everyone. Duplicates and non-positive
       values are dropped rather than rejected: they mean the same as
       leaving them out, and failing a whole save over one is unhelpful.
     - **Silence vs. an empty list differ on create**: no `reminder_leads`
       field means "the usual single reminder", `[]` means none.
     - 011's column was dropped in the same migration that backfills from
       it, rather than left behind - two sources of truth for "when does
       this remind people" is the drift this project keeps fixing. The
       backfill was verified against seeded data on a scratch database (the
       `#[sqlx::test]` harness only ever migrates empty tables, so nothing
       in the suite exercises it).
   - Polled every 5 minutes, which has to stay well under the shortest
     offered lead - polling hourly for a one-hour lead would let "starts in
     an hour" land up to an hour out.
   - **Rescheduling clears `reminder_sent_at`**, so moving an event lets the
     reminder fire again for the new time; editing anything else doesn't,
     so a rename can't re-notify everyone.
   - Reminds **accepted + maybe**; declined and never-answered are skipped.
   - Idempotent via `calendar_events.reminder_sent_at` (migration 010,
     with a partial index on the un-reminded rows). `is_due` also refuses
     events that already started, so an outage doesn't fire a batch of
     reminders for things already underway on restart.
   - `reminder_sent_at` is stamped even if the Discord post fails: the
     in-app reminders did go out, and retrying the event would re-notify
     everyone to chase one Discord message.
   - New `notify_event_reminders` preference (migration 010, default on,
     toggle on `/settings`) wired into `preference_column_for` — so the
     gate built by `settings-integrity` covers this new kind for free,
     which is exactly why that skill was sequenced first.
   - The thread message is **French**, matching
     `discord_announcement`'s existing format, and uses Discord's
     `<t:…:R>` timestamps so each reader sees their own timezone (which is
     also why it doesn't consult the unused `users.timezone`). The in-app
     notification stays English, like every other notification.
   Original entry: the one genuinely new feature. `discord_bot_config.reminders_channel_id` has been configurable
   on `/server` since migration 008 and is used by nothing. Model it on
   `services::digest` (pure `is_due` + `maybe_send_*` + `spawn_*_loop`),
   not on a new pattern.

Still open, not worth a skill file yet:
- `bot.rs` / `discord_announcement.rs` have no tests — see the Discord bot
  section below for why, and `.claude/skills/add-tests/SKILL.md` for the
  `wiremock` pattern that would work for the HTTP half.
- `users.timezone` is stored and never used for rendering; everything goes
  through `toLocaleDateString` on the browser's zone.
- The 8 written-but-unexecuted `mockup-responsive-*` /
  `mockup-announcement-thread` skills (see the roadmap above) are still
  valid and independent of all of the above.

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

  Both workflows' *first real runs* (2026-09-15) failed on all three jobs.
  Fixed 2026-09-16; each cause is worth knowing because none of them
  reproduce locally by default:
  - **Frontend Tests** died at `yarn install --frozen-lockfile`, not at any
    test. `vitest` declares `engines.node "^22.12.0 || ^24.0.0 || >=26.0.0"`
    and `@testing-library/jest-dom` declares `">=22"`; **yarn v1 treats an
    incompatible `engines` field as a hard error** (npm only warns), and the
    job pinned `node-version: "20"`. Now on 22. This is invisible locally
    whenever `node_modules/` already exists — yarn takes an "Already
    up-to-date" fast path and never re-checks engines.
  - **Test** died on `cargo clippy -- -D warnings` against 9 pre-existing
    lints (redundant/unused imports, a dead `AuthResponse` struct, a
    collapsible `if`, a derivable `Default`, four needless borrows). All
    fixed; clippy is now clean, so the "pre-existing clippy failures, don't
    chase them" caveat under Testing no longer applies to the backend.
  - **Security Audit** was auditing the wrong file — `cd backend && cargo
    audit` picked up the stale `backend/Cargo.lock` (see workspace notes
    above), reporting advisories already fixed in the real lockfile. It now
    runs from the repo root against the workspace `Cargo.lock`. Genuine
    findings were fixed by upgrading `sqlx` 0.7→0.8 (RUSTSEC-2024-0363) and
    `oauth2` 4.4→5.0 (which dragged in reqwest 0.11/hyper 0.14 → h2 0.3 and
    rustls 0.21 → rustls-webpki 0.101, five advisories in total). What's
    left is in `.cargo/audit.toml` with per-ID justification.

  ⚠️ When checking whether an advisory actually affects this app, use
  `cargo tree --workspace -i <crate> --target all --all-features`. Plain
  `cargo tree -i <crate>` silently misses target- and feature-gated paths —
  it reported `h2`/`rustls-webpki` as "not built" here when both were in
  fact compiled into the backend via `oauth2` and `serenity`.

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

**Tested as of 2026-09-16** — these were the last untested services:
- `bot.rs`: the gateway connection still isn't covered (that needs a live
  Discord socket), but every *decision* it makes on a reaction now is. The
  DB logic was lifted out of the serenity event handlers into free functions
  (`record_attendance`, `withdraw_attendance`, `is_attendance_emoji`) taking
  `&PgPool` and plain strings; the `EventHandler` impl is a thin adapter,
  and serenity types stop at that boundary. Only one thing genuinely needs
  Discord — resolving a reactor's *username*, since the reaction carries an
  id — so only that stayed in the adapter.
  Behaviour now pinned by tests, some of which was undocumented before:
  re-reacting is idempotent; un-reacting sets `declined` rather than
  deleting the row (so the creator still sees who pulled out); re-reacting
  after that flips back to `accepted`; a reaction on an unrelated message is
  a no-op that does **not** create a user; and neither does a stranger
  un-reacting.
- `discord_announcement.rs`: rewritten off serenity onto `reqwest`, matching
  `discord_feed`/`digest`, so it takes `base_url` and is wiremock-testable —
  it was the last Discord call in the codebase that wasn't. `announce_event`
  now composes `discord_feed`'s `post_message`/`add_reaction`/
  `fetch_or_create_thread`, and the reaction and thread steps are
  best-effort: failing to react or to open a thread logs a warning but still
  returns the message id, because the announcement itself did go out. A
  failed *post* does propagate — there'd be nothing to store.
- **`serenity` is now used only by `bot.rs`**, for the gateway. Everything
  else talks to Discord over `reqwest`.
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

1. ~~Root `cargo.toml` → rename to `Cargo.toml`~~ — **done 2026-09-16**, along
   with deleting the stale `backend/Cargo.lock`. See "Package managers /
   workspace setup" above for what it was breaking.
2. Remove `desktop/package-lock.json` (mixed npm/yarn artifacts; yarn is the
   one actually used).
3. Delete the now-fully-stale branches: `feat(Event)`, `feat(Storybook)`,
   `feat(terraform)` (local-only), `origin/dev/refacto`, `feat(Calendar)`,
   `feat(DiscordBot)` — all 0 ahead of `master` as of 2026-09-14, nothing
   left to merge from any of them.
4. ~~`bot.rs`/`discord_announcement.rs` have no automated tests~~ — **done
   2026-09-16**, see the Discord bot section above. The only Discord surface
   still uncovered is the gateway socket itself.
5. CI/CD is now fixed and real (see the Terraform/GitHub Actions section
   above and `docs/deployment.md`), but the pipeline can't deploy anything
   until a person does the one-time manual setup `docs/deployment.md`
   describes (create the `production` GitHub Environment, generate a
   deploy SSH key, add the four `DEPLOY_*`/`PROD_DOMAIN` repo secrets, and
   confirm what's actually running on the Proxmox host today via `pct
   list` before pointing a deploy pipeline at it).
6. `serenity` is stuck on a dependency chain (tokio-tungstenite 0.21 →
   rustls 0.22 → rustls-webpki 0.102) with four open RUSTSEC advisories and
   no fixed release available — 0.12.5 is the newest published version.
   Those four IDs are the bulk of `.cargo/audit.toml`'s ignore list; drop
   them the moment serenity ships on rustls 0.23+. Re-check on any serenity
   bump.
