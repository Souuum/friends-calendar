---
name: settings-integrity
description: Make every control on /settings actually do what it claims - fix the Visibility JSON casing bug that makes saving fail outright, apply default_visibility when creating events, and gate notification writes on the four notify_* toggles. Not yet executed as of 2026-09-16. Use when asked to fix settings, user preferences, notification toggles, or default event visibility.
---

# Settings integrity: stop shipping controls that don't work

Five controls on `desktop/src/routes/settings/+page.svelte` are currently
inert, and one of them breaks the whole page. This skill makes all of them
real. Do it in the order below - step 1 is a live bug, steps 2-3 are
features the UI already promises.

## Step 1 (bug, do first): `/settings` cannot save

`Visibility` in `backend/src/models/calendar_event.rs` derives
`#[sqlx(type_name = "visibility", rename_all = "lowercase")]`. That
`rename_all` is **sqlx's**, and controls the Postgres enum representation
only. There is no `#[serde(rename_all = ...)]`, so JSON uses the Rust
variant names verbatim. Verified by probe, not inferred:

```
serde_json::to_string(&Visibility::Friends)  => "Friends"
from_str::<Visibility>("\"friends\"")        => Err     <-- what /settings sends
from_str::<Visibility>("\"Friends\"")        => Ok
```

So `PATCH /api/auth/me` rejects the body every time, because
`settings/+page.svelte` always includes `default_visibility` in
`handleSave`. The page has never been able to save anything - not the
display name, not the timezone, not the toggles. The GET direction is
broken too: the API returns `"Friends"`, and the `<select>` options are
`value="private|friends|public"`, so the loaded value never matches an
option.

`CreateEventModal.svelte` has it right (`'Private' | 'Friends' | 'Public'`),
which is why event creation works. The two pages simply disagree.

**Decide first - which side moves?** Two defensible fixes, pick one and
apply it everywhere:

- **(a) Frontend adopts capitalised values.** Change `types.ts`
  (`default_visibility`, `visibility`, and the `UpdateProfileRequest`
  variant - three places), `settings/+page.svelte`'s local type and its
  three `<option value="...">`s. Smallest diff, matches what the API
  already emits and what `CreateEventModal` already sends. **Recommended.**
- **(b) Backend adopts lowercase JSON.** Add
  `#[serde(rename_all = "lowercase")]` to `Visibility`. Arguably nicer
  wire format, but it silently changes the contract for
  `CreateEventModal`, every `CalendarEvent` already serialized to clients,
  and any stored payloads - so it needs the frontend updated in the same
  commit anyway, plus `EventPeekPanel`/`EventRsvpCard` status rendering
  checked. Only pick this if you want lowercase as the long-term wire
  format.

Whichever you choose, apply it to `ParticipationStatus` too if it has the
same split - check before assuming it does.

**Test that would have caught this** (write it first, watch it fail):
a functional test through the real router, not a service-level one. The
existing `services::profile` tests construct `Visibility::Public` in Rust
and never touch serde, which is exactly why this survived. Use the
`build_router` + `generate_jwt` pattern from `handlers::discord`'s test
module:

```rust
// PATCH /api/auth/me with the exact JSON the frontend sends must 200,
// and the response body must round-trip back into the same select option.
```

## Step 2: apply `default_visibility` when creating an event

`users.default_visibility` is written by `services::profile::update_profile`
and read by nobody. `services::calendar::create_event` falls back to
`req.visibility.unwrap_or(Visibility::Private)` - a hardcoded default that
ignores the user's stored preference.

Backend (`services/calendar.rs::create_event`): when `req.visibility` is
`None`, look up the creator's `default_visibility` and use that, falling
back to `Private` only if the user row somehow has none. Keep the explicit
request value winning - the preference is a *default*, not an override.

Frontend: `CreateEventModal.svelte` currently hardcodes
`let visibility = 'Friends'`. Either preselect it from the current user
(`$user.default_visibility`, already on the store) or stop sending
`visibility` when the user hasn't touched the field so the backend default
applies. Preselecting is better UX - the user can see what will happen.

Tests: a `#[sqlx::test]` proving a user with `default_visibility = Public`
who creates an event without specifying visibility gets a `Public` event,
and that an explicit `Private` in the request still wins.

## Step 3: honour the four `notify_*` toggles

`notify_event_invites`, `notify_rsvp_changes`, `notify_announcements`,
`notify_weekly_digest` are columns on `users` (migration 008) that nothing
reads. Confirmed: grep for them outside `models/user.rs` and
`services/profile.rs` returns only comments.

There are four live notification triggers today, all calling
`services::notifications::create(db, user_id, kind, actor, event_id, message)`:

| trigger | file | maps to preference |
|---|---|---|
| event invite | `services/calendar.rs` (~line 91) | `notify_event_invites` |
| RSVP change | `services/calendar.rs` (~line 449) | `notify_rsvp_changes` |
| friend request | `services/friend_requests.rs` (~line 101) | *(none today)* |
| friend accepted | `services/friend_requests.rs` (~line 206) | *(none today)* |

**Do the gating inside `services::notifications::create`, not at each call
site.** One place to enforce it, and any future trigger (see
`.claude/skills/event-reminders/SKILL.md`, which depends on this) is gated
automatically instead of being a second unfiltered path. Map `kind` ->
preference column, look up the recipient's preference, and return `Ok(())`
without inserting when it's off.

**Decide first:** friend-request notifications have no matching toggle.
Either (a) always deliver them (they're low-volume and directly
actionable - reasonable), or (b) add a `notify_friend_requests` column in
migration `010`. Don't invent a mapping to one of the existing four - a
friend request is not an "event invite", and silently reusing that toggle
would surprise the user. Pick (a) unless the user asks otherwise, and write
the choice into the function's doc comment so the next person sees the
`kind` values that deliberately bypass the gate.

Note `notify_weekly_digest` is a **per-user** preference while the digest
posts to one shared Discord channel (`services::digest`, gated by the
guild-level `discord_bot_config.digest_enabled`). There is no per-user
delivery to filter, so this toggle stays unreadable-by-design until digests
are ever delivered per user. Say so in the UI or remove the toggle - do not
leave it looking functional. This is the one case where deleting the
control is the honest fix.

Tests: a `#[sqlx::test]` per gated kind - preference off means no row is
inserted and `unread_count` stays put; preference on inserts as before.
Add one asserting the ungated kinds still deliver regardless.

## Gotchas specific to this repo

- Run backend tests with
  `DATABASE_URL=$(grep DATABASE_URL backend/.env | cut -d= -f2-) cargo test`
  from `backend/` - `.env` is only loaded by `main()`.
- `yarn run check`, not `yarn check`. Both it and
  `cargo clippy --all-targets --all-features -- -D warnings` are **clean as
  of 2026-09-16** and gate CI - any new warning is yours.
- `cargo audit` also gates CI; if you add a dependency, check it against
  `.cargo/audit.toml` before pushing.
- Frontend component tests: `render(Component, { props: { ... } })`, never
  the flat shorthand, if any prop is named `events`/`target`/`anchor`/
  `context`/`intro` - those collide with Svelte's own mount options and get
  silently swallowed.
- If you touch `settings/+page.svelte`, its existing `page.test.ts` mocks
  `$lib/api` and `$app/navigation`; anything rendering `Frame.svelte` also
  needs `$app/stores` mocked (see `Frame.test.ts`'s `__setPathname`).

## Done when

- `/settings` saves successfully and reloads showing the saved values.
- A user whose default visibility is Public gets Public events by default.
- Turning off "Event invites" means no notification row on the next invite.
- Every remaining control on the page either works or is gone.
