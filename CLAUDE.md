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
   - **Lead time is per event, chosen by the creator** (migration 011,
     `calendar_events.reminder_lead_minutes`, default 60) - a picker in
     `CreateEventModal` offering none / 1h / 3h / 1 day / 2 days / 1 week.
     **0 means "no reminder", and needs no special case anywhere**: the
     window is `start_time > now AND start_time <= now + lead`, which is
     unsatisfiable at 0. The due-events query computes the bound per row
     with `make_interval(mins => reminder_lead_minutes)`.
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
- `discord_bot_config.reminders_channel_id` is *still* configurable on
  `/server` and used by nothing: reminders go to the event's own thread
  instead. Either wire it up as an additional destination or drop the
  field from the form.
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

1. ~~Root `cargo.toml` → rename to `Cargo.toml`~~ — **done 2026-09-16**, along
   with deleting the stale `backend/Cargo.lock`. See "Package managers /
   workspace setup" above for what it was breaking.
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
6. `serenity` is stuck on a dependency chain (tokio-tungstenite 0.21 →
   rustls 0.22 → rustls-webpki 0.102) with four open RUSTSEC advisories and
   no fixed release available — 0.12.5 is the newest published version.
   Those four IDs are the bulk of `.cargo/audit.toml`'s ignore list; drop
   them the moment serenity ships on rustls 0.23+. Re-check on any serenity
   bump.
