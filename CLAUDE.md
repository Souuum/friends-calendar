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
├── .github/workflows/rust.yml # dead: GitHub only reads .github/workflows at repo ROOT, this one never runs
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
    │   ├── auth.rs              # Discord OAuth2 login/callback/me/logout, verify_jwt(), generate_jwt() (pub(crate), reused by functional tests)
    │   ├── calendar.rs          # CRUD for events + participants + link_discord_message
    │   ├── discord.rs           # get_linked_server — which Discord server this app is linked to
    │   ├── friend_requests.rs   # send/list/accept/decline + missing-members/post-invite
    │   ├── friends.rs           # list/sync friends
    │   └── notifications.rs     # list/mark-read/mark-all-read/unread-count
    ├── middleware/
    │   ├── mod.rs
    │   └── auth.rs              # Claims extractor (FromRequestParts) backing JWT auth
    ├── models/
    │   ├── mod.rs
    │   ├── user.rs               # User, DiscordUser
    │   ├── calendar_event.rs     # CalendarEvent, CreateEventRequest, UpdateEventRequest, Visibility, ParticipationStatus, etc.
    │   ├── friendship.rs         # FriendInfo, SyncFriendsResult
    │   ├── discord_guild.rs      # LinkedServerInfo
    │   ├── notification.rs       # NotificationInfo
    │   └── friend_request.rs     # FriendRequestInfo
    └── services/
        ├── mod.rs
        ├── auth.rs
        ├── calendar.rs               # also owns the event_invite/rsvp_change notification triggers, see below
        ├── friends.rs                # Discord guild member fetch + friendship sync + get_linked_server_info
        ├── friend_requests.rs        # send/list/respond + friend_request/friend_accepted notification triggers + missing-members/post-invite
        ├── discord_announcement.rs   # posts event announcements + creates discussion threads
        └── notifications.rs          # create/list/mark-read/mark-all-read/unread-count
```

Runs on `axum = "0.7"`, `sqlx` (Postgres, runtime-tokio-native-tls),
`oauth2`, `jsonwebtoken`, `serenity = "0.12"` (Discord gateway bot, rustls
backend), `tower` with the `util` feature enabled specifically for
`ServiceExt::oneshot` in functional tests.

Server binds `127.0.0.1:8080`. CORS is hard-coded to allow only
`http://localhost:1420` (the Tauri dev origin) with credentials.

⚠️ `backend/.github/workflows/rust.yml` is leftover scaffolding from a point
when `backend/` was presumably its own repo — GitHub Actions never looks
inside `backend/.github`, only the top-level `.github/workflows`, so this
file is inert. Either delete it or fold anything useful into the root
`ci.yml`.

### Implemented API endpoints (from `backend/src/main.rs`)

```
GET    /                                       root()  — plaintext banner

# Auth
GET    /api/auth/discord                       handlers::auth::discord_login
GET    /api/auth/callback                      handlers::auth::discord_callback
GET    /api/auth/me                             handlers::auth::get_current_user
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
  on-demand, triggered from the settings page (below) via the "Sync
  friends" button.

Tests: `backend/src/services/friends.rs`'s `#[cfg(test)] mod tests` — see
"Testing" further down for the general policy and where the patterns are
documented.

### Settings page (`/settings`) — linked Discord server + friends

The "user page" from the task that added this: what Discord server this app
is linked to, and who else from that server also uses the app (friends,
above). Single-server only — `DISCORD_GUILD_ID` is one guild, not a list;
multi-server support would need a real data model change, not just this UI.

- `GET /api/discord/server` (`handlers::discord::get_linked_server`,
  `services::friends::get_linked_server_info`) — `GET /guilds/{id}` on the
  bot token, returns `{ id, name, icon_url, approximate_member_count }`.
  Same "400 if Discord isn't configured" treatment as friend sync, same
  `AppState.discord_bot_token`/`discord_guild_id` fields.
- `desktop/src/routes/settings/+page.svelte` — a route (not a modal/panel),
  reachable from the profile menu's "Settings" item in `Header.svelte`
  (previously a dead `console.log` stub — now `goto('/settings')`). Fetches
  the linked server and friends independently on mount; owns loading/error
  state for both, same container/presentational split as
  `CalendarView.svelte`. Renders `LinkedServerCard.svelte` (new,
  presentational) for the server and the already-existing `FriendsList.svelte`
  for friends — the latter is now actually wired into the app for the first
  time.
- `AppState` gained `discord_api_base: String` (defaults to the real
  Discord API, overridable via `DISCORD_API_BASE` — mainly for tests) so
  this endpoint and `services::friends` share one source of truth for
  "where is Discord" instead of each hardcoding it separately.

### Announcements page (`/announcements`) — events posted to Discord + RSVPs

Second sidebar item, previously dead: `Frame.svelte`'s `navItems` (Calendars
/ Announcement) has existed since early on, but `currentView` was local
component state nothing outside `Frame` could read or change — clicking
"Announcement" did nothing observable. Fixed by switching the sidebar to
real routing (`$app/stores`'s `page.url.pathname` for highlighting,
`$app/navigation`'s `goto` for clicking) instead of local state, and adding
the second route to navigate to.

- No backend change needed — `GET /api/events` (already
  `EventWithParticipants[]`, includes `discord_message_id`, `my_status`,
  and full `participants[]`) has everything this page shows. It filters to
  `discord_message_id != null` client-side in
  `desktop/src/routes/announcements/+page.svelte` (fetched with
  `include_declined: true`, since a declined event should still show up in
  "what got announced", just not in the calendar view's default list).
- `AnnouncementCard.svelte` (new, presentational, in `molecules/`) renders
  each one: title/description/date/location/price/link, a read-only "you
  accepted/declined/said maybe/haven't responded" badge, and everyone
  else's response via the already-existing `EventCardParticipant.svelte`
  atom. Deliberately **read-only** — changing your own RSVP already exists
  via `EventDetailsModal.svelte` (opened from the calendar view, calls
  `api.updateParticipation`); didn't duplicate that flow here since nothing
  asked for it and a second code path for the same mutation is how they
  drift out of sync.
- Found but did **not** reuse: `atoms/event/EventCard.svelte` (+
  `EventCardStatusBar.svelte`) is a more fully-featured card that already
  exists in the repo and looks built for exactly this — but it was (and
  still is) completely unused anywhere in the app, and its status bar
  dispatches `accepted`/`maybe`/`declined` events that nothing listens for,
  so clicking those buttons changes local UI state without ever calling
  the API. Wiring that up properly (mirroring `EventDetailsModal`'s
  `handleStatusChange`) would make interactive RSVP-from-the-announcements-
  page a small follow-up, but it's a separate decision from what was asked
  here (viewing, not editing) — flagging it rather than fixing it blind.

⚠️ **Real limitation surfaced while building this — half-fixed since:**
`GET /api/events` (`services::calendar::list_user_events`) only returns
events where you're already a row in `event_participants` — it does not
consult `visibility` at all for listing (unlike the single-event `GET
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
something to bolt on silently.

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
  `FriendsList.svelte`, `LinkedServerCard.svelte`) — trivially testable
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
rather than attempted as one change. Status:

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
- `.claude/skills/mockup-announcements-feed/SKILL.md` — not started, and
  has open design questions (see the skill) rather than being fully
  shovel-ready. Would **replace** the current `/announcements` (event-RSVP
  tracking, see above) with a real Discord-channel message mirror — these
  are two different features that happen to share a name; read the skill
  before starting, it flags a scope decision that needs the user's input.
- `.claude/skills/mockup-settings-and-server/SKILL.md` — not started.
  Splits the current `/settings` (linked server + friends) into a real
  profile/preferences page and a separate `/server` page with DB-backed,
  user-editable multi-channel bot config (replacing today's single
  `DISCORD_ANNOUNCEMENT_CHANNEL_ID` env var).
- `.claude/skills/mockup-availability/SKILL.md` — not started. Cross-user
  free/busy computation, consumed by the other skills' "Free tonight" /
  weekly-overlap / "Propose a time" UI — several of those were built
  without this and explicitly note where they simplified as a result (e.g.
  the friends directory's per-friend "note" uses shared-events instead of
  the mockup's richer availability-based status pills).

Each skill file is a concrete, runnable playbook (schema sketches, file
paths, endpoint shapes, test plan) — treat "run `.claude/skills/mockup-*`"
as a real, actionable request, not just documentation to reference.

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
│   │   ├── settings/+page.svelte           # linked Discord server + friends, see above
│   │   ├── announcements/+page.svelte      # events posted to Discord + RSVPs, see above
│   │   ├── friends/                        # directory (+page.svelte) + detail ([id]/+page.svelte) + add/+page.svelte (requests), see mockup roadmap below
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
│   │       │                   # FriendsList, LinkedServerCard, AnnouncementCard
│   │       ├── organisms/      # DayView, WeekView, MonthView, Header,
│   │       │                   # EventDetailsModal, BlurModal
│   │       └── templates/      # Calendar, Frame, ViewButton
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

- **Terraform** (`terraform/`): **real and complete-looking**, not just
  discussed. `main.tf` provisions a `proxmox_lxc` container, plus
  `cloudflare.tf` (DNS), `ssl.tf`, `provisioning.tf`, `outputs.tf`,
  `variables.tf`, and templates for nginx/systemd/env. This matches
  `Makefile`'s `init/plan/apply/destroy/ssh/logs/status/update` targets and
  the `scripts/` helpers (`ssh.sh`, `logs.sh`, `status.sh`, `update.sh`,
  `backup.sh`). The `feat(terraform)` branch is fully merged (identical to
  `master`) — that branch can be deleted.

- **GitHub Actions** (`.github/workflows/`): real and present —
  `ci.yml`, `cd-staging.yml`, `cd-production.yml`.
  ⚠️ Internally inconsistent with the rest of the repo, though:
  - `ci.yml`'s `frontend-test` job does `cd frontend && npm ci` / `npm test`
    / `npm run build` / `npm run check` — **there is no `frontend/`
    directory**; the actual frontend lives at `desktop/` and uses `yarn`,
    not `npm`. This job will fail as soon as it triggers.
  - `cd-staging.yml` deploys on push to `develop`, `cd-production.yml` on
    push to `main` / tags `v*`. **Neither `develop` nor `main` branches
    exist** in this repo — the working/default branch is `master`. As
    written, these two deploy workflows can never fire.
  - `ci.yml` triggers on PRs/pushes to `[main, develop]`, same mismatch.
  This all reads like the CI/CD was authored against an assumed
  `main`/`develop`/`frontend` layout that doesn't match how the repo was
  actually set up (`master` branch, `desktop/` directory). Needs
  reconciling one way or the other — either rename the branches/dir, or fix
  the workflow branch filters and paths.

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
3. Delete `backend/.github/workflows/rust.yml` (dead — GH Actions doesn't
   read non-root `.github`) or merge its intent into root `ci.yml`.
4. Fix `.github/workflows/ci.yml`'s `frontend/` → `desktop/` path + `npm` →
   `yarn`, and reconcile the `main`/`develop` branch names in `ci.yml`,
   `cd-staging.yml`, `cd-production.yml` against the real `master` branch (or
   rename branches to match).
5. Delete the now-fully-stale branches: `feat(Event)`, `feat(Storybook)`,
   `feat(terraform)` (local-only), `origin/dev/refacto`, `feat(Calendar)`,
   `feat(DiscordBot)` — all 0 ahead of `master` as of 2026-09-14, nothing
   left to merge from any of them.
6. `bot.rs`/`discord_announcement.rs` have no automated tests (see Discord
   bot section above for why and what a first pass could look like).
