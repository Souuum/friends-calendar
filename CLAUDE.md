# CLAUDE.md

This file gives Claude Code (and other agents) grounding in how this repo is
actually structured today, as opposed to how it might look from commit
messages or branch names alone. Everything below was verified against the
working tree and git history on 2026-09-14.

## What this is

A Discord-OAuth "friends calendar" app: a Rust/Axum backend, a
Svelte/SvelteKit + Tauri desktop client, and (mostly unmerged) a Discord bot
that announces events into a Discord channel. Infra is Proxmox LXC via
Terraform, deployed through GitHub Actions.

## Branches

```
* master                    <- default branch, active
  remotes/origin/master
  remotes/origin/dev/refacto        0 ahead / 1 behind master  -> fully merged, stale
  feat(Calendar)                    3 ahead / 5 behind master  -> diverged, needs rebase
  feat(DiscordBot)                  7 ahead / 0 behind master  -> real, unmerged feature
  feat(Event)                       0 ahead / 28 behind master -> fully merged, stale
  feat(Storybook)                   0 ahead / 19 behind master -> fully merged, stale
  feat(terraform)                   identical to master        -> fully merged, stale
```

`master` is both the current and default branch (`origin/HEAD -> origin/master`).

**Branches that are safe to delete** (their work is already in `master`,
diffing them against `master` only shows master having moved on):
`feat(Event)`, `feat(Storybook)`, `feat(terraform)`, `origin/dev/refacto`.

**Branches with real unmerged work:**

- **`feat(DiscordBot)`** — the only branch with substantial, non-trivial
  unmerged code. See [Discord bot branch](#discord-bot-branch-featdiscordbot)
  below for what's there and what's left.
- **`feat(Calendar)`** — small frontend cleanups only: shortens a popup delay
  to 0.5s, removes an unused `Status` type alias and an unused `formatDate`
  helper in `dateUtils.ts`, one formatting commit. It's *behind* master by 5
  commits (predates the Storybook merge), so it needs a rebase before it can
  land — the diff currently shows large spurious "deletions" of Storybook
  story files that actually already exist in `master`, that's just branch
  staleness, not real removal.

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
│   └── 002_create_events_table.sql
└── src/
    ├── main.rs                # entrypoint, router, CORS, server bootstrap
    ├── config.rs               # AppState: db pool, oauth2 client, jwt secret, pkce store
    ├── error.rs                 # AppError -> HTTP response mapping
    ├── handlers/
    │   ├── mod.rs
    │   ├── auth.rs              # Discord OAuth2 login/callback/me/logout, verify_jwt()
    │   └── calendar.rs          # CRUD for events + participants
    ├── middleware/
    │   ├── mod.rs
    │   └── auth.rs              # Claims extractor (FromRequestParts) backing JWT auth
    ├── models/
    │   ├── mod.rs
    │   ├── user.rs               # User, DiscordUser
    │   └── calendar_event.rs     # CalendarEvent, CreateEventRequest, UpdateEventRequest, Visibility, ParticipationStatus, etc.
    └── services/
        ├── mod.rs
        ├── auth.rs
        └── calendar.rs
```

Runs on `axum = "0.7"`, `sqlx` (Postgres, runtime-tokio-native-tls),
`oauth2`, `jsonwebtoken`, `serenity` is **not** a dependency on `master`
(only on `feat(DiscordBot)`, see below).

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
```

Frontend (`desktop/src/lib/api.ts`) targets `http://localhost:8080` by
default (`VITE_API_URL` override), which matches the backend's bind address.

### DB migrations (on `master`)

```
001_create_users_table.sql    users table, unique discord_id index
002_create_events_table.sql   calendar_events + event_participants,
                               visibility & participation_status enums,
                               time-range CHECK constraint, several indexes
```

Two more migrations exist **only on `feat(DiscordBot)`** (not yet in
`master`) — see below.

## `desktop/` (SvelteKit + Tauri)

```
desktop/
├── src-tauri/                 # Tauri Rust shell — part of the Cargo workspace
│   ├── Cargo.toml             # crate "desktop" / lib "desktop_lib"
│   ├── src/{main.rs,lib.rs}
│   ├── capabilities/, gen/, icons/
│   └── tauri.conf.json
├── src/
│   ├── routes/                # SvelteKit routes: +layout.svelte, +page.svelte
│   ├── lib/
│   │   ├── api.ts             # fetch wrapper, JWT storage in localStorage
│   │   ├── stores.ts, types.ts
│   │   ├── actions/clickOutside.ts
│   │   ├── utils/{dateUtils.ts,tooltipUtils.ts}
│   │   └── components/
│   │       ├── CalendarView.svelte, CreateEventModal.svelte,
│   │       │   EventCardImpl.svelte, LoginScreen.svelte
│   │       ├── atoms/          # Button, Avatar, CalendarDay, CalendarEvent,
│   │       │                   # TimeSlot, ViewSwitcher, event/ subfolder (Event,
│   │       │                   # EventCard, CompactEvent, DetailedEvent, ...)
│   │       ├── molecules/      # CalendarHeader, EventList, EventTooltip,
│   │       │                   # ModalContainer, ProfileMenu/, TimedEvent
│   │       ├── organisms/      # DayView, WeekView, MonthView, Header,
│   │       │                   # EventDetailsModal, BlurModal
│   │       └── templates/      # Calendar, Frame, ViewButton
│   └── test/stories/           # Storybook stories (atoms + ProfileMenu)
├── .storybook/                 # Storybook + SvelteKit config
├── build/                      # committed SvelteKit build output — see flag below
└── static/
```

Atomic-design component layout (atoms → molecules → organisms → templates).
Storybook is wired up for the atoms and the ProfileMenu molecule only; most
molecules/organisms/templates have no stories yet.

⚠️ `desktop/build/` (compiled SvelteKit/Tauri output, including hashed JS
chunks) is committed to the repo. This is generated output and should
normally be gitignored, not checked in — it will drift from source and bloat
the repo on every rebuild.

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

- **Discord bot**: code exists, but **only on the unmerged `feat(DiscordBot)`
  branch**, not on `master`. See below.

### Discord bot branch (`feat(DiscordBot)`)

7 commits ahead of `master`, 0 behind — clean to merge/rebase. Adds:

- `backend/src/bot.rs` — a `serenity`-based Discord gateway bot
  (`DiscordBot::start`), listens for `reaction_add`/`reaction_remove` in the
  announcement channel, reacts to a ✅ check-mark emoji to track RSVPs.
- `backend/src/services/discord_announcement.rs` — `DiscordAnnouncer`,
  posts a formatted event announcement message via the bot HTTP token, adds
  a ✅ reaction, and spins up a Discord thread under the message for
  discussion.
- Wires into `main.rs`: spawns the bot as a background tokio task at
  startup, reading `DISCORD_BOT_TOKEN` / `DISCORD_ANNOUNCEMENT_CHANNEL_ID`
  from env (both already present in `backend/.env` and in `ci.yml`'s env
  block, and in the Terraform `tfvars` templates — so the rest of the repo
  was already prepared for this branch to land).
  Adds `POST /api/events/:id/link-discord` route
  (`handlers::calendar::link_discord_message`).
- New migrations (not on `master` yet):
  - `003_add_discord_message_events.sql` — adds `discord_message_id`,
    `discord_channel_id` columns + index on `calendar_events`.
  - `004_add_prince_and_link.sql` — adds `price`, `link` columns. ⚠️ The
    filename says "prince" — looks like a typo for "price" (that's what the
    migration actually adds); worth renaming/fixing before merge, migration
    filenames become part of `sqlx`'s applied-migrations history so this is
    easiest to fix now, before it ships.
- `Cargo.toml` gains `serenity = "0.12"` (rustls backend, client/gateway/model
  features).

**To finish this feature for a merge to `master`:**
1. Fix the `004_add_prince_and_link.sql` filename/typo before it's applied
   anywhere.
2. Rebase onto current `master` (it's 0 behind currently, but re-check before
   merging since `master` may move).
3. `handlers::calendar::create_event` doesn't appear to call
   `DiscordAnnouncer::announce_event` automatically on this branch — check
   whether posting the announcement is meant to be automatic on event
   creation or manual via `/link-discord`; currently the only wiring is the
   explicit `link-discord` endpoint, so an event created via the API won't
   auto-announce unless the frontend/another path calls it.
4. No tests were added for `bot.rs` / `discord_announcement.rs`.
5. Bot token/channel ID are `expect()`-ed at startup (`main.rs`), so if
   Discord bot env vars are unset the *entire backend* fails to boot, not
   just the bot — worth deciding if the bot should be optional.

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
5. Decide whether `desktop/build/` should be committed; if not, gitignore it.
6. Merge or delete stale branches: `feat(Event)`, `feat(Storybook)`,
   `feat(terraform)`, `origin/dev/refacto` are fully absorbed into `master`
   already.
7. Rebase `feat(Calendar)` onto `master` (small, easy cleanups).
8. Land `feat(DiscordBot)` — see checklist above; it's the one branch with
   real, unmerged, non-trivial functionality.
