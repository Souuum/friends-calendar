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

Server binds `BIND_ADDR` (default `127.0.0.1:8080`). It became configurable
on 2026-09-16 because where it binds depends on where `cloudflared` runs:
inside the same container, loopback is right; a tunnel on the Proxmox host
can't reach loopback and needs `0.0.0.0:8080`. Loopback stays the default so
widening exposure is a deliberate act — nothing authenticates in front of
this port, only the JWT middleware behind it.

**CORS** (`main.rs::allowed_origins`) is an explicit list built from
`FRONTEND_URL`, plus the packaged-Tauri origins (`tauri://localhost`,
`https://tauri.localhost`) and the `http://localhost:1420` dev origin.
`allow_credentials(true)` forbids the `*` wildcard, so it has to be a list.

⚠️ **Until 2026-09-16 this was the single hard-coded literal
`http://localhost:1420` — the Tauri *dev server* origin, which is the origin
of nothing in production.** `FRONTEND_URL` existed on `AppState` but only
fed the OAuth redirect (`handlers::auth`), so a deployed API would have
completed the login redirect and then had **every** subsequent browser
request blocked, the packaged desktop app included. Found while preparing
the first real deployment, not by any test — and the two router-level tests
added with the fix were verified to fail against the old code first. A
related subtlety: a *single* hard-coded origin makes tower-http echo
`access-control-allow-origin` to every requester; a list correctly withholds
it, which is why the "unrelated origin is not granted" test also failed
before.

(This is also why `terraform/templates/nginx.conf` doesn't set its own CORS
headers — see the Terraform section below for why that combination breaks
browsers when both layers do it. That file is unused now; see
`terraform/README.md`.)

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
POST   /api/announcements/:id/adopt                 handlers::announcements::adopt_announcement
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
  - ⚠️ **happy-dom also implements no HTML5 constraint validation.** A
    `required` field the browser refuses to submit past is, to happy-dom, an
    ordinary attribute - so `fireEvent.click` on the submit button runs the
    handler that a real Chrome would never have reached. The adopt modal
    shipped stuck-on-submit with eight passing component tests over it,
    including ones that clicked submit. Anything that asserts a form
    *submits* - or refuses to - needs a real browser, same as layout.
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
    `app.css` is bundled through `+layout.svelte`'s import. ⚠️ The tap-target check
    walked only the five bottom-tab buttons, so modals went unchecked until
    the 14.7px modal close button turned up - see the modal-dismissal note
    below.
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

## Feature backlog from the mockups - 2026-09-17 (written, not executed)

Derived from a pass over **all 13 mobile screens and all 8 desktop ones**,
rendered side by side against the running app. The mobile file's per-screen
captions are the useful part - they state the gesture and behaviour each
screen expects, which is the closest thing to a spec this project has. Every
gap below was checked against the code, not inferred.

Run in this order; only the last pair has a real dependency.

1. `.claude/skills/discord-channel-picker/SKILL.md` - **done 2026-09-17.**
   `/server` used to ask for a raw Discord snowflake in a text box: you
   needed Developer Mode to get one, nothing validated it, and a
   wrong-but-plausible id failed silently because announcing is
   best-effort. Now `GET /api/guilds/:id/channels` +
   `discord_feed::list_text_channels`, and the page offers the channels
   grouped by category in Discord's own order.
   - ⚠️ **Keyed by `guilds.id`, not the Discord snowflake.** A snowflake
     taken from the client would make the bot token a "list any server's
     channels" proxy for anyone with a session. `/server` holds the
     snowflake, so it resolves one to the other through `GET /api/guilds` -
     one extra request on a settings page, and it keeps a client-supplied
     id out of the path.
   - **Types 0 and 5 only** (text, announcement). Voice, category, stage and
     forum channels cannot take a message, and offering an option that
     cannot work is the failure the picker exists to end.
   - ⚠️ **Discord only returns channels the bot has `VIEW_CHANNEL` on**, so a
     channel the user expects is simply absent rather than an error. The
     page says so outright.
   - **The raw id field survives as the fallback** when there's no guild, no
     bot token, or Discord refuses - a deployment whose bot is offline still
     has to be configurable. The existing tests, written against that field,
     now cover that path.
   - The label is a `<span id>` + `aria-labelledby`, not `<label for>`: which
     control it names depends on the branch, and a `for` pointing at an input
     that doesn't exist names nothing.
2. `.claude/skills/calendar-day-interactions/SKILL.md` - **done 2026-09-17.**
   Month day cells carried `role="button"`, `tabindex="0"` and
   `cursor-pointer` with **no click handler** - 35 fake buttons per screen,
   and worse than a plain div because a screen reader announced them as
   buttons. They now select a day (listing its events under the grid, with
   Enter/Space doing the same) and long-press / double-click starts an event
   on that date. Swipe-between-months stayed out of scope: there are no
   gestures in this app, and building a gesture abstraction as a side effect
   of adding a click handler is how one component ends up owning a swipe
   library.
   - `lib/actions/longPress.ts` - ⚠️ **cancel on movement, not just on
     pointerup.** A finger that presses then drags is *scrolling*, and
     without a threshold every scroll starting on a day cell opened the
     create form. Mutation-tested: removing the check fails the browser
     test. Mouse `pointerdown` is ignored on purpose (a click-and-think
     would fire it); double-click is the desktop equivalent.
   - ⚠️ **The event chip needed `stopPropagation`.** It sits *inside* the
     cell, so one tap both opened the event and changed which day filtered
     the list. There's a test for exactly that.
   - `CreateEventModal` takes a third nullable input, `initialDate`, rather
     than a `mode` flag - matching how `event` already works. It is cleared
     on close, or the next "+ New event" opens on whatever day was last
     long-pressed.
   - Long-press cancellation is a **browser** test: happy-dom has no
     pointer-movement model, so the threshold is unassertable there.
3. `.claude/skills/event-nudge-no-answers/SKILL.md` - **done 2026-09-17.**
   `POST /api/events/:id/nudge` + `services::nudge`, migration **015**
   (`calendar_events.nudged_at`). This was the last control in the app that
   existed and did nothing.
   - ⚠️ **The first user-triggered outbound send in this app.** Everything
     else is a consequence of creating something or a scheduled job, so this
     is the first thing that can be used to annoy people. The 24h limit is a
     **column**, not a disabled button: the button is a suggestion, the
     endpoint is the surface. A refused nudge does **not** reset the clock.
   - **Creator-only, enforced in the same query that fetches the event** so
     there's no check-then-act window - and it answers **404, not 403**,
     because a 403 confirms the event exists and belongs to somebody else.
   - ⚠️ **`pending` only.** `maybe` *is* an answer; nudging it turns a
     considerate feature into pestering. Mutation-tested, along with the
     creator check.
   - **Delivered in-app + into the event's own Discord thread**, *not* by
     DM. A DM is the only thing that reliably reaches someone who doesn't
     open the app, and also the most annoying thing this app could learn to
     do - left as a separate decision, deliberately not taken.
   - Notifications go through `services::notifications::create` with kind
     `event_invite`, so the preference gate applies for free. Reusing that
     column is right *here* - a nudge is a second ask about an invitation -
     unlike `friend_request`, which CLAUDE.md notes must not.
   - ⚠️ **`nudged_at` is stamped even when the Discord post fails.** The
     in-app half went out, and letting a failed thread post buy another
     nudge would defeat the limit.
   - ⚠️ Clippy caught a `pending_count` service function with **no caller** -
     the panel already has the participant list on the wire and counts it
     there. Removed rather than kept "for later": a second source of truth
     for a number the client can already see.
   - Migration 015 was applied by hand to a scratch database **seeded with an
     existing event**, and re-applied to confirm it's a no-op. The
     `#[sqlx::test]` harness only ever migrates empty databases, so nothing
     in the suite covers that.
4. `.claude/skills/availability-best-overlap/SKILL.md` - **done 2026-09-17.**
   `GET /api/availability/best-slot` + `availability::rank_slots`. The
   mockup's "Best overlap this week: Fri 20:00, 7 free" now renders on the
   calendar bar, and "Propose a time" opens the create form **on that slot**.
   - ⚠️ **A slot-level computation beside the day one, not a replacement.**
     `compute_free_users_per_day` is day-granularity on purpose (it backs
     the weekly strip) and is correct as it stands - but it cannot answer
     "Fri 20:00": someone with a 09:00 dentist appointment is "busy Friday"
     and would be excluded from every Friday evening, which is exactly the
     population this feature exists to find.
   - ⚠️ **Free for the whole slot, not just at its start.** A slot that
     begins in a gap and runs into an event is not a time you can meet.
     Half-open at both ends, matching `compute_free_users_at`, so an event
     ending exactly when a slot starts does not block it. Mutation-tested.
   - **Candidate slots are evenings every day plus weekend afternoons**, not
     every 30 minutes across the week - that would be 336 candidates of
     mostly nonsense (03:00 Tuesday), and a suggestion nobody would act on
     is noise.
   - **Ties break toward the soonest**, and the order is total - otherwise
     the suggestion shuffles between page loads for no reason.
   - ⚠️ **Timezone: the client sends its UTC offset; `users.timezone` is
     still unread.** That column is free text, defaults to UTC and almost
     nobody fills it in, while the browser knows the real answer. The offset
     is fixed rather than a timezone, so a window spanning a DST change is
     out by an hour on the far side - a rounding error within the week this
     is asked about, versus a timezone database for one hour a year.
   - **Ranked over the caller's friends, filtered to slots the caller is
     also free for** - suggesting a time you're busy is worse than
     suggesting nothing - and the caller is not in the count, because "7
     free" meaning six friends plus yourself is a worse number.
   - ⚠️ **Send `toISOString()` (`…Z`), not `+00:00`**, for the `from`/`to`
     query params: a bare `+` decodes as a space and the request 400s. Cost
     two test runs to spot.
5. `.claude/skills/invite-friend-to-event/SKILL.md` - ⚠️ `/friends/[id]`
   has **no invite action at all** (zero hits for "invite"). Both mockups
   make it that screen's primary action. Works standalone; much better
   after 4, which turns "invite them" into "invite them to a time you're
   both free".

**Considered and not written** (the user chose the four above out of five):
posting announcements from the app, which both mockups show. It is blocked
by the same problem that made replies read-only - a bot-token post has no
attribution and can launder `@everyone` - and the fix is a Discord webhook
with per-user `username`/`avatar_url` override plus `allowed_mentions`,
which would also unblock reviving replies. Worth doing, carries a security
design decision, deliberately deferred.

**Also seen in the mockups and deliberately not turned into skills:** the
mobile gesture layer (swipe the grid for months, pull-to-refresh the
agenda, swipe an agenda row to RSVP, swipe a friend row to invite, swipe an
alert row to mark read, drag the event sheet between peek and full height) -
the most work on the list for the least functional gain, since every one of
those flows already works by tapping. The friends filter chips (All / Free
this week / Pending / Recently added) need per-friend availability that no
endpoint returns today; `availability-best-overlap` would supply most of it.

## Matching the desktop mockup, screen by screen (2026-09-17)

Driven by a real side-by-side: the `.dc.html` mockup was rendered in
Chromium (it needs React on `window` before `support.js`, and its screens
are switched by the nav buttons in its own chrome) and every screen
captured at 1440x1000, then the same routes captured from `yarn preview`
with `e2e/fixtures.ts` stubbing the API. Comparing pictures, not reading
CSS, is what surfaced most of the below - several of these had been wrong
since the screens were built.

**The shell** (`Frame`, `ViewButton`, `Header`) now follows the mockup's own
values: a 216px rail on `surface` with a right hairline, a mono "Navigate"
label, the signed-in user pinned to the foot of the rail, and a header that
names the screen you're on.

- ⚠️ **Three routes were missing from the rail entirely** - Add friends,
  Discord server and Settings were reachable only from inside another page.
- The mockup's **"New event" row is deliberately not reproduced**: here that
  is a modal owned by the calendar, not a route, so the row could only
  navigate to the calendar without opening anything.
- ⚠️ **The rail uses dots, not icons.** That is the mockup's own design, and
  it applies *only* to the desktop rail - `MobileTabBar.dc.html` uses icons,
  so `BottomTabBar` keeps them. If the icons are wanted back, that's a
  deliberate departure from the mockup, not a regression.
- **One content container for every screen** (1080px, 26/24/60 padding),
  where each page previously carried its own `max-w-*` and padding and no two
  agreed. Pages keep a narrower `max-w-2xl` only where the content is a feed.
- ⚠️ **`md:h-screen` + a scrolling content column.** Without it the rail
  stretches to the height of the *page*, so its footer sits below the fold on
  any long screen. Below `md:` the page scrolls normally.
- The header handle was `text-xl`, making it the largest thing on every
  screen - louder than the page title. It's the mockup's 14px/600 now.

**Calendar.** Event chips are the mockup's 3px status rail + title +
headcount, not filled tint pills - the filled version made every event read
as "going" regardless of status. Cells carry the mockup's 126px minimum and
its two dim levels (0.45 outside the month, 0.72 in the past, which were one
level before), day numbers sit in a fixed 25px box so they line up whether
or not today's circle is drawn, and the grid gained the rounded outer border
it never had.

⚠️ **`STATUS` now lives in `lib/utils/eventStatus.ts`.** It was inlined in
`EventPeekPanel` and needed again by the chip, which is exactly how two
components end up disagreeing about what "maybe" looks like.

⚠️ **The peek panel docks at `lg:`, not `md:`.** At 768 the 216px rail plus
its 296px column left ~192px for seven day cells - 27px each - and every
chip title truncated to **zero width**. Found because a modal-dismiss test
started timing out on a zero-width click target, not by looking. Below
`lg:` it is the sheet, which means tablet now gets sheet behaviour and the
dismissal tests are split on `desktop-1280` rather than `mobile-402`.

⚠️ **`EventPeekPanel` has `data-testid="event-peek"`.** The sidebar is an
`<aside>` too and is visible from `md:` up, so `getByRole('complementary')`
matched two elements at tablet width. Same trap the sidebar's own testid
already existed for.

**Friends.** Cards are the mockup's 250px auto-fill grid with a 42px avatar
carrying a presence dot, a mono sub-line, and a divider above the footer
row. ⚠️ The "Free now" pill was **green**, which the mockup's palette has
nowhere - "available" is the same affirmative as "Going" and takes
tint/accent. The dot means *free right now*, the only presence this app
actually knows; it is not Discord's online status.

**Settings** was the furthest off: one narrow column of bare form controls
against the mockup's cards. Now grouped into Profile / visibility /
appearance / notifications / delete cards, with segmented visibility options
(each carrying the line that says what it means, which a `<select>` has
nowhere to put) and real toggle switches.

⚠️ **The switches are still `<input type="checkbox">`**, `sr-only` with a
sibling span drawn by `peer-checked:`. A div with a click handler would look
identical and silently drop the label association, the keyboard behaviour
and `checked` - which every existing test reads.

⚠️ **`after:content-[""]` breaks the Svelte parser**: the double quote closes
the `class` attribute. Use `after:content-['']`.

**Tokens.** `--color-ink`, `--color-bg` and `--color-subtle` were the last
mockup `:root` entries with no app equivalent; `body` now paints with the
mockup's own ground and ink rather than the nearest Tailwind greys.
⚠️ They needed **dark values too** - adding them light-only turned the whole
page light in dark mode, which `theme.spec.ts` caught immediately.

⚠️ **`theme.spec.ts` measured `.bg-surface`, which is now the hidden rail.**
It samples `main .bg-surface` instead. Not `:visible` - that is a Playwright
locator pseudo-class and the helper resolves its selector with
`querySelector` in page context.

**Known gaps, left on purpose:** the mockup's friends filter chips (All /
Free this week / Pending / Recently added) need per-friend availability that
no endpoint returns; the announcements right rail (Channels / Digest cards)
would duplicate `/server`, which owns that config; and the "Poll" tag has no
signal behind it (see the announcements feed note).

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

## Two bugs the first real deployment surfaced (2026-09-17)

Both pre-existing, both invisible locally because local dev always had
seeded data and nobody read the compiled CSS.

- ⚠️ **An empty calendar was a dead end.** `CalendarView.svelte` replaced
  the whole calendar with a "No events yet" message when
  `events.length === 0` - but `+ New Event` lives in `CalendarHeader`,
  *inside* `Calendar`. So a fresh account had no way to create its first
  event and stayed empty permanently, which is exactly the state a new
  deployment starts in. The calendar now always renders and the hint sits
  above it. `CalendarView` also carried a `showCreateModal` +
  `CreateEventModal` that nothing ever set to true - dead since
  `Calendar` took ownership of the modal; removed.
- ⚠️ **`discord-blurple` was never defined**, in `@theme` or anywhere else,
  yet 31 classes referenced it: 16 `focus:ring-`, 10 `text-`, 3 `bg-`,
  2 `border-`. All compiled to **nothing**. `text-discord-blurple`
  inherited and looked plausible, which is why it survived so long; but
  `bg-discord-blurple` with `text-white` rendered white text on no
  background - the announcements "Sync now" button was invisible until
  hover, where a real `hover:bg-blue-600` took over. Now defined (`#5865f2`,
  lifted in dark mode).

`e2e/layout.spec.ts` gained **"no control is invisible against its own
background"**, which generalises the second one: for every button and link
it compares text luminance against the nearest ancestor that actually
paints a background. ⚠️ Its first version *passed while the bug was still
present* - the parser only understood `rgb()`, so the `oklch()` page ground
returned null, the ancestor walk found nothing, and every element was
skipped. Both this and `theme.spec.ts` now parse oklch; keep them in step.
Validated by reintroducing the bug and confirming the check fails.

## Mobile week & day views (2026-09-17)

Day and Week were `hidden md:block` in `ViewSwitcher` because their grids
assume seven columns, which leaves **~39px per day at 402px** - too narrow
for an event chip. Both are now offered at every width.

⚠️ **The mobile mockup has no week or day screen.** Its calendar toggle is
"Grid" and "List" only, and there is no hour-grid markup anywhere in the
75KB file - checked before building, not assumed. So these layouts were
designed here, not ported, and the shape was chosen with the user:

- **Week, below `md:`** becomes a **day strip plus one day's hours**
  (`data-testid="week-view-mobile"`). The week is still the unit of
  navigation; only the display narrows. Both layouts are mounted and gated
  by `hidden`/`md:` classes, so each has its own testid.
  - The selected day resets via `$: weekDays, (selectedIndex = ...)` -
    depending on `weekDays` **alone**, so paging to another week re-anchors
    on today while tapping a day inside the current week is left alone.
    There's a test for the tap case; it fails if that dependency widens.
- **Both grids scroll to the first event** on open (`anchorToFirstEvent` in
  `lib/utils/timeGrid.ts`), one hour early so it isn't flush to the top,
  falling back to 08:00 on an empty day. They render all 24 hours, so
  before this they opened at midnight and you scrolled past the small hours
  every time.
- `HOUR_HEIGHT` now lives in `timeGrid.ts`. It was a bare `80` written twice
  inside `TimedEvent` with nothing tying it to `TimeSlot`'s `h-20` - if they
  drift, events slide away from their hour further down the grid.
- **DayView's empty state used to render *below* the grid**, so an empty day
  meant scrolling past 24 hours of nothing to be told there was nothing. It
  replaces the grid now.
- **DayView no longer renders its own date heading.** `Calendar.svelte`'s
  `headerDate` already emits the identical string when `view === 'day'`, so
  it was the same line twice. Its `currentDate` prop went with it - events
  arrive pre-filtered, so the component had no other use for it.

⚠️ **`Frame.svelte`'s `<main>` needed `min-w-0`**, found by this work but
pre-existing: a flex child defaults to `min-width: auto` and refuses to
shrink below its content, so `md:w-14/16` (87.5%) beside the 192px sidebar
wanted 864px inside a 768px viewport and overflowed by 93px. Month view hid
it by being narrow enough to shrink anyway; the week grid is not. Same trap
as the unwrappable URL in `EventRsvpCard`.

## Demo data (`scripts/demo-data.sh`, 2026-09-17)

`seed <who>` / `status` / `clean`. Fills the database with six people,
eleven events spread from last week to next month, RSVPs, reminders,
publications, notifications and announcement posts, so the app can be
looked at with content in it.

- **Cleanup is exact by construction**: every demo person has
  `discord_id LIKE 'demo-%'` and every demo event is *created by* one of
  them, so `DELETE FROM users WHERE discord_id LIKE 'demo-%'` cascades
  through events, participants, reminders, publications and friendships and
  cannot touch a real row. Verified by checksumming real users and events
  either side of a clean - both unchanged, zero residue.
- `notifications.actor_user_id` is `ON DELETE SET NULL`, **not** cascade, so
  the clean deletes those rows explicitly *before* the users. Otherwise they
  survive, pointing at nobody.
- ⚠️ **Pass the account you log in with** (`seed soum`). The default is
  merely the oldest non-demo account, which on this dev database is a
  leftover `TestUser` - seeding against it leaves your own calendar empty,
  which is exactly what happened the first time. The script now raises
  rather than silently seeding an unreachable account.
- No demo event is owned by *you*, deliberately: owning some would mean
  rows cascade cannot identify. The cost is that the calendar's "Created by
  me" filter stays empty.

## Announcement replies are read-only (2026-09-17)

`POST /api/announcements/:id/reply` **has been removed**, along with the
composer, `api.postAnnouncementReply`, and its tests. `GET .../replies`
stays - reading a thread in the app is fine.

The reason is not that it was unused. Replies were sent with the **bot
token**, so:

- **No attribution.** The Discord thread showed `friends-calendar` saying
  whatever a user typed. A reader could not tell who wrote it.
- ⚠️ **Mention escalation.** `post_message` sends `{"content": …}` with **no
  `allowed_mentions`**, so a user typing `@everyone` in a reply had the
  *bot* ping the server - using the bot's permissions rather than their own.
  Any app user could launder a mention through the bot this way.

The fix is a deep link instead: `AnnouncementPostInfo.thread_url`, built
server-side as `https://discord.com/channels/{guild}/{message_id}` (a thread
started from a message shares that message's id). Ids still never reach the
client - there is a test asserting that - so the URL is assembled in
`list_posts` rather than exposing `discord_message_id`. Absent config means
no link, and the page says so rather than rendering a dead button.

⚠️ **`thread_url` resolves the guild from `guilds`, not `discord_bot_config`.**
The first version read the latter, which only has a row once somebody saves
the `/server` form - and the announcements feed never needs that, because
`resolve_announcement_channel_id` falls back to the env var. So a perfectly
working deployment reported "no server is linked yet". `guilds` is populated
automatically by the gateway's `guild_create`.

A functional test asserts the route returns **404**, not merely that the UI
stopped calling it.

⚠️ If a write path is ever wanted again, a bot token is the wrong mechanism.
The options are a Discord webhook with per-user `username`/`avatar_url`
override (looks attributed, still the app's credential) or real OAuth
message scopes. Either way `allowed_mentions` must be set to suppress
`@everyone`/`@here`.

## Discord markdown & reaction backfill (2026-09-17)

**`lib/utils/discordMarkdown.ts`** renders Discord's message flavour to a
closed set of HTML tags. The announcements mirror showed raw markup before -
`**Date :**`, `<t:1795806000:F>`, `<:hmm3:141715...>`, `[OKAY](url)` - because
the body was rendered as plain text.

- ⚠️ **The output goes through `{@html}` and the input is whatever anyone in
  the server typed.** Safety comes from ordering: every `<...>` construct and
  code span is lifted into placeholders *first*, everything left is escaped,
  formatting is applied to the escaped text, then the placeholders are
  substituted with HTML this module built from validated values. Emoji ids
  must be digits (they go into a CDN URL); hrefs must be http(s), so
  `javascript:` and `data:` stay inert text. Written by hand rather than
  pulled from npm because a general markdown library renders far more than
  Discord does and would need sanitising anyway.
- Chips use **translucent** neutrals (`bg-gray-500/20`), not a fixed grey:
  this HTML lands inside ordinary cards *and* the inverted featured card.
  The contrast audit measured the timestamp chip at **1.08:1** with a fixed
  `bg-gray-100` - it caught brand-new code minutes after it was written.
- ⚠️ **The contrast audit had to learn alpha compositing** for that fix. It
  stopped at the first non-transparent ancestor and used its raw rgb, so a
  20%-alpha chip over a dark card reported 2.15:1 for something that renders
  fine. It now composites down the ancestor chain until opaque. Verified it
  still catches the real 1.08:1 case afterwards.

**The announcement template** (`format_event_message`) was reshaped
2026-09-17 to match how people in the server already write these by hand: a
`##` heading, one `> ` quoted line per field, and the **value** emphasised
rather than the label. It read as a form before. Three deliberate
departures from the hand-written posts:

- the `[label](url)` masked link is kept over a bare URL - ticketing links
  carry long tracking query strings that otherwise swamp the post;
- `@everyone` is not spoiler-wrapped (`||@everyone ||` is one person's
  habit, and it hides the mention text);
- the "Réagissez avec ✅" line stays, because here the reaction **is** the
  RSVP - dropping it to match the template exactly would make the feature
  undiscoverable.

The format had no tests at all before this; it does now, and
`discordMarkdown.test.ts` renders the same template so the two stay in step.

**`services/reaction_sync.rs`** backfills RSVPs from reactions already on
announcement messages. `bot.rs` only ever sees reactions added *while it is
connected*, so anything ticked before an event was announced through this
app - or during downtime - never became an RSVP, which is why a ✅ could
leave the calendar empty.

- Reuses `bot::record_attendance`, so a backfilled RSVP and a live one take
  exactly the same path: same publication lookup, same idempotent upsert,
  same handling of a previous "declined".
- ⚠️ **Skips reactors flagged `bot`.** The bot adds the ✅ itself, so without
  that check it would become a participant in every event it announced.
- Runs once at startup (spawned, not awaited - one Discord call per
  announced message) and is exposed as `POST /api/events/sync-reactions`.
  Idempotent, so booting repeatedly is harmless.
- A failing message is logged and skipped rather than abandoning the run: a
  single deleted message should not stop the rest.

## Adopting an existing announcement (2026-09-17)

`POST /api/announcements/:id/adopt` turns a message that is **already in
Discord** into a calendar event, binding the event to that message instead
of posting a new one. `services::event_adoption`.

The gap it closes: `create_event` only ever announces *outwards*, so the app
has a row for exactly the events it posted itself. Everything else - posts
people write by hand, and every event created before a deployment had a
database - is a message carrying ✅ reactions that nothing can resolve.
`reaction_sync` walks `event_publications`, finds nothing, and correctly
records nothing. That is why a ✅ on the EsdeeKid post left the calendar
empty on the first real deployment: production's database is fresh, the
events were created on a dev machine, and the post is tagged "General"
because no publication row matches it.

Adoption is the same wiring run backwards, so nothing downstream needs a
special case: once the publication row exists, `bot.rs` resolves new
reactions, `reaction_sync` backfills the old ones, `discord_feed` tags the
post as an event, reminders find the thread, and publication-scoped
visibility lets the server see it.

- ⚠️ **Nothing server-side parses the message.** The event's fields come
  from the request, which the user confirmed in a form. The *client* guesses
  them (`lib/utils/announcementParse.ts`) to prefill that form, but a regex
  over someone's free-form French is not something to write into a calendar
  unreviewed. The parser handles both styles in use - the bot's
  `> Activité : **X**` (value bold) and the hand-written `**Activité :** X`
  (label bold, so its closing `**` lands at the *start* of the value) - and
  returns `undefined` per field rather than guessing; the modal then names
  what it couldn't read instead of showing an empty required field.
- **Refused twice over.** `resolve_target` rejects a message that already
  backs an event, using `guilds::event_for_message` - the same lookup
  `discord_feed::infer_tag` makes to decide whether a post shows as "Event",
  so the hidden button and the server's refusal are driven by one fact
  rather than two that can disagree. Adopting twice would leave two events
  competing for one message's reactions, and `event_for_message` returns
  only one of them.
- **Adopting posts nothing.** A functional test mounts the "create message"
  endpoint with wiremock's `.expect(0)`, so a regression that announced an
  adopted event fails the test rather than spamming the channel. (Verified
  by flipping it to `.expect(1)` and watching it fail - `.expect` is checked
  when the mock server drops.)
- **Binding failure rolls the event back.** An event with no publication is
  invisible to everyone but its participants, *and* a retry would create a
  duplicate, since the already-adopted guard keys off the publication row.
- The guild is resolved the same way `list_posts` resolves it for thread
  links: prefer a guild that has published into this channel, else the
  oldest registered. With no guild at all it refuses rather than creating an
  event published nowhere.
- ⚠️ `req.guild_ids` is cleared in `adopt()` but that is **belt and braces,
  not a tested guarantee** - `calendar::create_event` ignores the field
  entirely today, announcing happens in the handler, and a mutation test
  confirmed removing the line changes nothing observable. The guarantee that
  holds is the `.expect(0)` above.
- ⚠️ **The adopt form is `novalidate`, deliberately.** Its fields are
  `required`, and the date frequently *can't* be parsed out of a post - so
  Chrome refused the submit before `on:submit` ever ran. No request, no
  error, no visible reason: the modal just sat there, which is exactly how
  it was reported ("the modal stays no matter what"). Constraint validation
  reports through a native tooltip that is easy to miss inside a scrolling
  container, and it bypasses the component's own error box entirely. The
  form validates itself instead, in one place, naming the field it wants -
  "This event still needs a start time" rather than "required fields",
  because the reason it's blank is that the parser couldn't find it.
  `CreateEventModal` was checked for the same trap and is safe: its
  `validateStep1` means the mobile wizard can't reach the submit button with
  an unfilled required field sitting in the `hidden` step.
- UI: `AnnouncementPostCard` takes an optional `onAdopt`, and shows "Add to
  calendar" only when a handler is supplied *and* the post isn't already an
  event. `AdoptEventModal` (organisms) is a **separate** component rather
  than a third mode on `CreateEventModal` - the three things that component
  does most prominently are all wrong here: the server picker (the server is
  wherever the message lives), the Discord preview (previewing a message
  that will never be sent is a lie), and the invite picker (adoption doesn't
  invite - the ✅ already do). Reminders aren't offered either; the backend's
  default single reminder applies and is editable afterwards like any
  event's.

## Icons (2026-09-17)

**`atoms/Icon.svelte` replaced every emoji in the UI with outline SVGs** -
24x24, 2px strokes, `currentColor`. The emoji were full-colour bitmaps the
theme had no say over, which mattered once dark mode landed; they also
render differently per platform and can't be sized reliably.

- Names describe **meaning, not shape** (`price`, not `euro-sign`), so a
  call site reads as what it is and the glyph can change without every
  usage lying.
- `shrink-0` is applied unconditionally in the component. The Notifications
  sidebar row proved why: it's the only nav item with a badge, and its icon
  collapsed to a sliver while every other row looked fine.
- Icons are `aria-hidden` by default - nearly all sit beside their own text
  label, and announcing both is just repetition. Pass `label` for the cases
  where the icon *is* the content.
- **Hub and Alerts both used 🔔**, so two different tabs were
  indistinguishable. They're now a megaphone and a bell.
- ⚠️ **One emoji stays on purpose**: the ✅ in `/servers`' permissions list.
  It isn't a UI icon - it names the literal Discord character people react
  with to RSVP, which `bot.rs` matches on. An SVG there would describe the
  wrong thing, and there's a comment saying so.
- Splitting glyphs out of strings (`'✓ You accepted'` → icon + `'You
  accepted'`) changed what tests can match on. One assertion became
  genuinely ambiguous: the thread page renders the reply count twice (card
  footer *and* section heading), which `💬 1 reply` had accidentally
  disambiguated.

⚠️ **`discord-fuchsia` was undefined too**, alongside `discord-blurple` -
used once, by the login screen's gradient, so half of it silently resolved
to nothing. Both are defined now.

## Dark mode (2026-09-17)

**Implemented by redefining Tailwind's colour variables under `.dark`, not
by adding `dark:` utilities to components.** Tailwind v4 compiles every
colour utility to a variable — `.bg-white{background-color:var(--color-white)}`,
`.text-gray-500{color:var(--color-gray-500)}` — so overriding those in one
block re-themes all **272 raw colour usages across ~40 of the 68
components** without editing any of them. The `dark:` alternative is the
same work repeated 272 times, and silently incomplete the moment anyone
writes a new component.

- **The grey ramp is inverted, not replaced** (50↔900, 100↔800, …).
  `text-gray-900` means "strongest text" and `bg-gray-100` means "just off
  the surface"; both keep their *meaning* when the ramp flips, which is what
  makes untouched markup work. Picking arbitrary darks would invert some
  pairs and not others.
- ⚠️ **`--color-white` must stay meaningfully lighter than `--color-gray-50`.**
  In dark mode `bg-white` is the *elevated* card surface and `gray-50` is the
  page ground beneath it. The first attempt set them to 20.5% and 21% — a
  lightness difference of 0.000004, so every card boundary in the app
  vanished. `e2e/theme.spec.ts` has a test for exactly this.
- `@custom-variant dark (&:where(.dark, .dark *))` exists for the cases a
  token swap can't cover (an inverted shadow, say). Class-based, not
  media-based, because the theme is a user choice with a system default.
- `lib/theme.ts` owns the state: `'light' | 'dark' | 'system'`, persisted in
  `localStorage`. **`'system'` is a real third state**, not a snapshot — it
  keeps following the OS via `watchSystemTheme()`, wired up in
  `+layout.svelte`. Every storage access is try/caught (private browsing
  throws rather than returning null) and `matchMedia` is feature-detected.
- **The class is applied by an inline script in `app.html`, before paint.**
  Doing it from the bundle would paint light first and flip on hydration — a
  white flash on every load. It deliberately duplicates a few lines of
  `theme.ts`; nothing is importable that early, and the comment says so.
- The toggle lives on `/settings` but is **outside the Save button's
  scope** — it's a per-device preference, not a column on `users`. Routing
  it through `PATCH /api/auth/me` would mean one browser's choice changing
  the theme on someone's phone.
- Buttons, not a `<select>`: happy-dom can't match `<select>` options by
  value, which is how the reminder-picker tests once passed by accident (see
  the Testing section).
- ⚠️ **`--color-white` is NOT overridden in `.dark`, on purpose.** It backs
  `text-white`, which means "ink on a coloured fill" and must stay light in
  both themes. Dark mode originally redefined it to serve as the card
  surface, which rendered every such label near-black - measured
  `oklch(0.22)` on an `oklch(0.62)` purple button, so "Save changes" and
  "Delete my account" were dark-on-colour app-wide. The surface meaning now
  has its own token, **`--color-surface`**, and all 39 `bg-white` usages
  became `bg-surface`. One variable cannot mean both a surface and its ink.
- ⚠️ **Never write an arbitrary colour** (`bg-[#f4f4f7]`). Tailwind compiles
  it to a literal — `.bg-\[\#f4f4f7\]{background-color:#f4f4f7}` — not to
  `var(--color-*)`, so the theme swap is structurally unable to reach it and
  it silently stays light. That is how the view switcher shipped with a
  white background in dark mode. `src/lib/theming.test.ts` is a lint that
  fails with file and line if one reappears; add a `@theme` token instead.
  The tokens added for the ones that existed: `--color-invert` /
  `--color-on-invert` (the featured announcement card, values from the
  mockup's own `--invert`) and `--color-primary-active`.
- ⚠️ **Every tint ramp is inverted, not just the greys.** Inverting only
  grey left `bg-red-100`, `bg-indigo-50` and friends light in dark mode
  while the text on them flipped light: the unread notification row and the
  "N members aren't on Friends Calendar yet" banner both measured
  **1.01:1** - text exactly the colour of its background. Mid-ramp fills
  (400-600) are deliberately left alone: they carry white ink in both
  themes. Only shades actually used are defined; a new one that needs a
  dark value fails the contrast audit rather than shipping broken.
- ⚠️ **`--color-primary` has the same fill-vs-ink split as white.** White
  ink on a primary *fill* wants the colour darker; primary used as *text*
  on a dark surface wants it lighter. One value measured 3.89:1 and 2.93:1
  respectively. The token is the fill; `.dark .text-primary` overrides the
  ink via `--color-primary-ink`.
- **`e2e/contrast.spec.ts` audits real WCAG contrast** across every route in
  both themes, converting colours through a canvas so oklch/oklab/rgb are
  all handled exactly. It gates at **3:1**, not AA's 4.5: at 4.5 it fails on
  ~27 light-mode elements using `--color-muted` (#7c7c83) and Discord's
  blurple (#5865f2), which sit at 3.8-4.4. Those are the mockup's and
  Discord's own values - a real gap, but a design decision rather than
  something a test should force. Everything between 3 and AA is printed in
  the failure message so the gap stays visible.

- **Switching themes cross-fades** (`.theme-transition` in app.css, driven
  by `applyTheme`). The class is added for the duration of a change and
  removed again - never left on. A standing `* { transition }` would animate
  the *first* paint (a dark-mode user watching the page fade in from white)
  and every hover state thereafter. It uses `transition-property/duration`
  longhand rather than the shorthand so the `prefers-reduced-motion` block
  below it can still override the duration alone, and is unlayered so it
  beats Tailwind's `transition-*` utilities without `!important`. Both of
  those claims have browser tests.
- ⚠️ **Chrome reports interpolated colours as `oklab()`** while a transition
  runs, so the luminance parsers in `theme.spec.ts` and `layout.spec.ts`
  handle oklab as well as oklch. Tests that measure colour after a theme
  change must wait for `.theme-transition` to come off, or they sample
  mid-fade and read the *old* colour.
- **A sun/moon toggle also sits in the header** (`Header.svelte`,
  `data-testid="theme-toggle"`), next to the notifications bell. It reads
  `resolvedTheme`, **not** `theme`: with the setting on `'system'` the
  stored value isn't what's on screen, so the button would show the wrong
  icon and flipping the stored value could appear to do nothing (setting
  `'light'` while the OS still says dark). `toggleTheme()` therefore flips
  away from what is *rendered*, and always lands on an explicit choice -
  `'system'` stays reachable from `/settings`. The icon shows the mode you'd
  switch **to**, with an `aria-label` that says so outright.

⚠️ **A test-tooling trap this surfaced**: Chrome's `getComputedStyle`
returns colours authored as `oklch()` **as `oklch()`**, not converted to
rgb. The first version of `theme.spec.ts`'s luminance helper regexed three
numbers out of `oklch(0.21 0.006 285)` and treated the *hue* (285) as a blue
channel, reporting light-mode backgrounds as dark. It now branches on the
format; for oklch the first component already *is* perceptual lightness.

## The mobile sheet was never pinned to the viewport (2026-09-17)

Surfaced as a **CI-only** layout failure: the peek sheet's close button
measured 5.5px below the fold on the Linux runner and nowhere else. It was
not a rendering difference - it was a real bug that macOS happened to hide.

⚠️ **`animation-fill-mode: both` makes an element a containing block for
every `position: fixed` descendant, forever.** `forwards` keeps the animated
property *in effect* after the animation ends, and an in-effect
`transform: none` computes to the **identity matrix**, not the `none`
keyword - and any transform other than `none` re-parents fixed descendants
to that element.

`Calendar.svelte`'s body carries `anim-fade-up`. `EventPeekPanel`'s mobile
sheet lives inside it. So `fixed inset-x-0 bottom-0` was resolving against
the calendar's *content box* rather than the viewport: the sheet sat 20px
below the fold on macOS, further on Linux, and its full-screen backdrop
never covered the viewport either.

Every keyframe in `app.css` ends at the element's natural state
(`opacity: 1`, `transform: none`), so `forwards` pinned nothing they
wouldn't have had anyway - it was pure cost. All eight finite animations are
`backwards` now, which still holds the `from` state through `anim-sheet`'s
0.1s delay, the only thing the fill mode was needed for. ⚠️ This is a
**deliberate divergence from the mockup**, which uses `both` throughout; it
has no fixed-position child inside an animated wrapper, so it never hit
this.

`e2e/modal-dismiss.spec.ts` asserts both halves directly: the sheet's bottom
equals the viewport height, and no ancestor has a transform/filter/contain.
Reverting the CSS fails it on any platform - the old tap-target test passed
on macOS with the bug present, which is precisely why it only ever failed
in CI.

### Measuring an animating element: three wrong ways

All three were tried here before the fourth worked.

1. **`element.getAnimations()` right after it appears returns an empty
   list** - the browser has not created the animation yet - so the wait
   resolves instantly and you measure mid-flight.
2. **Polling for a *stable* bounding box is fooled by `animation-delay`.**
   `anim-sheet` waits 100ms parked at `translateY(100%)`, so two reads 50ms
   apart agree on a position that is entirely off screen.
3. **Waiting for `document.getAnimations()` to empty hangs** on
   `anim-pulse-dot`, which is `infinite`.

What works: poll `document.getAnimations()` until every animation either has
`iterations === Infinity` or `playState === 'finished'`. The delay phase
already reports `running`, so it is covered.

### Reading CI failures without admin rights

⚠️ **The job log needs admin on the repository.** `playwright.config.ts` now
adds the `github` reporter in CI, which emits check annotations carrying the
test name, file, line and message - and annotations are readable from the
**public** API:

```
GET /repos/{owner}/{repo}/actions/runs/{run_id}/jobs      # find the job id
GET /repos/{owner}/{repo}/check-runs/{job_id}/annotations # the failures
```

The uploaded `layout-screenshots` artifact needs auth to download, so it is
not a substitute.

## Modals had one way out, and it was 14.7px wide (2026-09-17)

Reported as "the event modal is still persisting on mobile, i have to do a
gesture back to get rid of it but lands me on the previous page". Both
halves were real, and neither was specific to one modal - `CreateEventModal`
and `AdoptEventModal` shared the same overlay.

- ⚠️ **The only close control was a bare `×` glyph with no padding**,
  measured at **14.7 × 32px** - a third of the 44px floor `e2e/layout.spec.ts`
  holds the bottom tab bar to. That test only ever walked the five tab
  buttons, so nothing checked a modal. `Cancel`, the other way out, sat
  **1005px down** a scroll container on a 620px-tall viewport.
- ⚠️ **The overlay was `flex items-center` with no overflow**, and the dialog
  `max-h-[90vh]`. On a phone `vh` is measured against the viewport *without*
  the URL bar, so a dialog sized to 90vh is taller than what you can see, and
  centring it puts the header *and* the footer off screen with nothing to
  scroll. The overlay now scrolls (`overflow-y-auto`, `items-start`,
  `my-auto` on the dialog) and the header is `sticky`, so the close control
  is reachable at any scroll position and any height.
- **Nothing else dismissed them**: no Escape, no backdrop click. So a phone
  user's only remaining move was the back gesture, which navigated off the
  page - the second half of the report.

⚠️ **`EventPeekPanel` had the same problem and was missed the first time.**
It isn't a modal - it's the event-detail *sheet* below `md:` - so it wasn't
covered by the modal fix, and it shipped with **no dismissal path at all**:
no close control, no backdrop, no `close` dispatch on the component and
nothing listening for one on `Calendar`. Selecting a different event was
the only thing that could change it. It now has a transparent backdrop
(`use:dismissable`) and a 44px close control, **both only below `md:`** -
from `md:` up it's a static column with its own "select an event" empty
state, and is supposed to persist. The breakpoint is read with `matchMedia`
rather than a `md:hidden` backdrop, because the backdrop and its history
entry must not *exist* on desktop, not merely be invisible. There's a test
asserting the desktop column survives a stray click.

`lib/actions/dismissable.ts` now gives both modals Escape, backdrop-tap, and
**back-gesture** dismissal.

- The back gesture only closes a modal if the modal *is* a history entry, so
  opening one pushes state and closing one pops it. **SvelteKit's own state
  is spread through** (`{ ...history.state }`) - the router keys navigation
  off an index it keeps there, and replacing the object wholesale breaks the
  next real navigation.
- `ourEntryIsLive` distinguishes "closed by a button" (pop our entry with
  `history.back()`, so leaving the page still takes one press) from "the
  browser already popped it" (do nothing - calling `back()` there would
  navigate away, which is the bug). There's a test for each direction;
  removing the pushState makes the back test land on `/settings`, which is
  precisely what was reported.
- Backdrop dismissal keys off `pointerdown` **and** `pointerup` both landing
  on the overlay, so dragging to select text inside the dialog and releasing
  outside it doesn't close the form.

⚠️ **`e2e/modal-dismiss.spec.ts` has to be a browser test.** happy-dom has no
layout (so it cannot see that a control is off screen), no `history` for a
back gesture to act on, and no constraint validation. All of this was
invisible to the component tier, which was passing throughout.

⚠️ **`tablet-768` renders the *desktop* layout.** `md:` is `min-width: 768px`,
so the tablet project is on the far side of the breakpoint - a test for
mobile-only behaviour has to skip everything except `mobile-402`, not just
`desktop-1280`. Skipping only the latter made three sheet tests fail for a
reason that had nothing to do with the code.

⚠️ **Wait for `anim-pop` (and `anim-sheet`) before measuring anything.** `boundingBox()` during
the modal's entrance reports the *scaled* size - a 44px control measures
43.0 - so the tap-target assertion failed for a reason unrelated to the CSS.
`open()` awaits `getAnimations({ subtree: true })`. Same trap as sampling a
colour mid theme-transition.

⚠️ **`h-11` is not 44px here.** This app's root font size isn't 16px, so
2.75rem lands at 43.0 and the tap-target floor fails by a hair. Tap targets
are spelled `min-h-[44px]`/`min-w-[44px]` in pixels. (Arbitrary *colours*
are still banned - see the dark mode note - but an arbitrary length is fine
and is what the codebase already does for `max-h`/`w-[1.25em]`.)

⚠️ `playwright.config.ts` sets `reuseExistingServer: !process.env.CI`, and
the server command is `yarn build && yarn preview`. **A preview server left
running locally serves a stale build**, so a CSS change appears to have no
effect. Kill port 4173 before concluding a style fix didn't work.

⚠️ **`actions/clickOutside.ts` leaked every listener it ever added** - found
while reading it for this work, unrelated to the reported bug. It added on
the bubble phase and removed with `capture: true`; a mismatched flag means
`removeEventListener` matches nothing. Only `Header.svelte` uses it, so the
leak was one dead listener per profile-menu mount.

⚠️ `BlurModal`/`ModalContainer`/`BlurOverlay` already implement backdrop and
escape handling and are used by **nothing** - dead since `EventDetailsModal`
was deleted. They were deliberately not resurrected here: adopting them
would have restyled two working modals (the blur overlay is a different
look) to reuse untested code. Delete them or adopt them, but don't leave a
third half-built modal system.

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

- **Terraform** (`terraform/`): ⚠️ **written, audited, and never applied —
  and as of 2026-09-16 it does not describe how this app is deployed.**
  There is no state file, no `terraform.tfvars` and no `.terraform/`; it has
  never been run. The real topology (home Proxmox behind NAT, published
  through a Cloudflare tunnel, deployed by a self-hosted runner) breaks most
  of its assumptions — `terraform/README.md` has the file-by-file table, and
  `docs/deployment.md` describes what actually happens. The one salvageable
  piece is that the Proxmox API answers through the tunnel
  (`https://proxmox.<domain>/api2/json/version` → 401), so container
  *creation* could be driven remotely; everything downstream of creation is
  handled differently now. The historical audit notes below are kept because
  they document bugs that were real, not because the code is in use.
  `main.tf` provisions a `proxmox_lxc` container, plus
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
  ⚠️ `POST /api/events/:id/link-discord` used to exist for manually linking
  an event to an existing Discord message. **It was removed 2026-09-17**:
  migration 014 moved `discord_message_id`/`discord_channel_id` off
  `calendar_events` onto `event_publications`, and the handler was never
  updated, so it had been writing to two columns that no longer exist - a
  guaranteed 500. Nothing called it (no frontend caller, no test), which is
  why it went unnoticed. Adoption replaces it; see below.

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
5. CI/CD is real, and `docs/deployment.md` now describes the **actual**
   topology (2026-09-16/17): home Proxmox behind NAT, published through an
   existing Cloudflare tunnel, with the GitHub Actions runner installed **on
   the app container** so the deploy is a local `mv` + `systemctl restart`.
   Proven by probe: 443 through the tunnel is open, 22 and 8006 are
   filtered, so a GitHub-hosted runner has no SSH route in and `cd.yml`'s
   original `scp`/`ssh` shape could never have worked. That collapse means
   `PROD_DOMAIN` is the **only** secret the deploy needs — the three
   `DEPLOY_SSH_*` ones are gone. Remaining manual setup: create the
   `production` Environment, run `scripts/provision-container.sh` on the
   container, register the runner as the `github-runner` user it creates,
   and add the tunnel route. ⚠️ The trade taken deliberately: a runner
   inside a down container can't deploy its own fix, so recovery is a manual
   SSH from the LAN.
6. `serenity` is stuck on a dependency chain (tokio-tungstenite 0.21 →
   rustls 0.22 → rustls-webpki 0.102) with four open RUSTSEC advisories and
   no fixed release available — 0.12.5 is the newest published version.
   Those four IDs are the bulk of `.cargo/audit.toml`'s ignore list; drop
   them the moment serenity ships on rustls 0.23+. Re-check on any serenity
   bump.
