---
name: add-tests
description: Add unit, integration, and/or functional tests for a backend (Rust/Axum/sqlx) or frontend (Svelte/vitest) change in this repo, plus a Storybook story for every new or changed component, following the conventions already established here. Use whenever implementing or reviewing a backend or frontend feature - tests and stories are not optional follow-up in this repo, they're part of finishing the feature.
---

# Adding tests in rust-friends-calendar

Standing rule for this repo: **every backend or frontend change that adds or
changes behavior gets tests at whichever tiers below actually apply.** Don't
treat this as optional polish - a feature isn't done until it has tests, the
same way it isn't done until it compiles.

This repo had zero test tooling before `services::friends` (friend-list
sync) and `FriendsList.svelte` established the patterns below. Follow them
rather than inventing new ones - consistency here matters more than any
individual choice.

## Three tiers (use the ones that apply, not always all three)

1. **Unit** - a pure function, no I/O. Plain `#[test]`, no macros, no
   database, no network. Example: `User::build_avatar_url` in
   `backend/src/models/user.rs` - given a discord_id and an avatar hash,
   return a URL string. If a piece of logic can be pulled out of a bigger
   async function into something like this, testing it this way is cheapest
   and most stable - do that extraction when it's easy.
2. **Integration** - a `services::*` function that touches the DB and/or
   calls an external HTTP API (Discord), tested against a *real* scratch
   Postgres and/or a *mocked* HTTP server - never a live network call to a
   third party in a test. See "Backend: DB-touching code" and "Backend:
   Discord/external-HTTP code" below.
3. **Functional** - drives a request through the actual `axum::Router`
   (real routing, real middleware/auth, real handler) via
   `tower::ServiceExt::oneshot`, asserting on the HTTP response. This is the
   closest thing to an end-to-end test without a running server. Reach for
   this when a bug could live in the wiring (route path, extractor order,
   auth requirement, status code, JSON shape) rather than in the service
   logic itself - which the integration tier already covers. See "Backend:
   functional tests" below.

## Backend (Rust)

Tests live in a `#[cfg(test)] mod tests { use super::*; ... }` block at the
bottom of the file they're testing (see `backend/src/services/friends.rs`
for the reference example covering all three tiers). Don't create a
separate `tests/` directory - nothing else in this codebase does that.

Run them with:
```
cd backend
DATABASE_URL=$(grep DATABASE_URL .env | cut -d= -f2- ) cargo test
```
`.env` is only loaded by `main()` (`dotenvy::dotenv()`), never by
`cargo test` - you must pass `DATABASE_URL` explicitly or `#[sqlx::test]`
has nothing to connect to.

### DB-touching code

Use `#[sqlx::test]`, not a hand-rolled pool:
```rust
#[sqlx::test]
async fn does_the_thing(db: PgPool) {
    // db is a fresh, empty database with every migration in
    // backend/migrations/ already applied - real schema, not a mock.
    // It's created before the test and dropped after, so tests are
    // isolated from each other and from your real dev DB.
    ...
}
```
Needs a reachable Postgres server (the same one `DATABASE_URL` points at
for local dev is fine - `#[sqlx::test]` creates its own throwaway database
alongside it, it doesn't touch your dev data).

### Discord / external-HTTP code

Never call the real Discord API from a test. Use `wiremock` (already a
dev-dependency):
```rust
let server = MockServer::start().await;
Mock::given(method("GET"))
    .and(path("/guilds/g1/members"))
    .respond_with(ResponseTemplate::new(200).set_body_json(vec![...]))
    .mount(&server)
    .await;
```
For this to work, the function under test must take the Discord API base
URL as a parameter rather than hardcoding it - see how
`services::friends::sync_friends` and `services::friends::get_linked_server_info`
both take `base_url: &str` (in production, callers pass
`&state.discord_api_base`, which defaults to the real API but can be
pointed at a `MockServer` in tests). If you add a new function that calls
Discord, thread the base URL through the same way - don't hardcode
`https://discord.com/api/v10` inside the function itself, or it becomes
untestable without hitting the real API.

### Functional tests

Build the real router against a test `AppState`, then drive it with
`tower::ServiceExt::oneshot`:
```rust
#[sqlx::test]
async fn get_endpoint_returns_expected_shape(db: PgPool) {
    let server = MockServer::start().await; // if the route calls Discord

    let state = AppState::for_test(db, server.uri()); // see note below
    let app = build_router(state); // extract the Router::new()...routes
                                    // chain out of main.rs into a
                                    // reusable fn if one doesn't exist yet

    let token = make_test_jwt(&claims_for(&user), &state.jwt_secret);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/whatever")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```
`AppState::new()` reads everything from env vars via `.expect(...)` and
isn't test-friendly directly - build the test `AppState` by hand (or add a
small test-only constructor) using the `#[sqlx::test]`-provided pool, a
directly-constructed `oauth2::BasicClient` (pure object construction, no
network), and `discord_api_base` pointed at the `wiremock` server. See
`backend/src/handlers/discord.rs`'s test module for a worked example.

### Lint

Run `cargo clippy --all-targets --all-features -- -D warnings` after adding
tests. This repo has pre-existing clippy failures unrelated to any given
change (unused imports in `middleware/auth.rs`, a few style lints
elsewhere) - don't chase those down, but make sure your *new* code doesn't
add to the pile. Compare against a `git stash`/clean baseline if unsure
whether a warning is pre-existing.

## Frontend (Svelte / TypeScript)

`vitest` + `@testing-library/svelte` + `@testing-library/jest-dom` +
`happy-dom` are already configured (`vite.config.js`'s `test` block,
`tsconfig.json`'s `types` entry). Run with `yarn test` (or `yarn test:watch`)
from `desktop/`.

Co-locate `<Component>.test.ts` next to `<Component>.svelte` - see
`FriendsList.svelte` / `FriendsList.test.ts` for the reference example.

Prefer presentational, props-driven components (data comes in via `export
let`, not an internal `onMount` fetch) - see `EventList.svelte` and
`FriendsList.svelte`. These are trivial to test: `render(Component, {
props })`, assert on what's in the DOM via `screen`, `fireEvent.click(...)`
for interactions. No mocking needed.

If a component *does* call `api` directly (page-level components like
`CalendarView.svelte` that own their own `onMount` fetch), mock the module
rather than hitting the real backend:
```ts
vi.mock('$lib/api', () => ({
  api: { getFriends: vi.fn(), syncFriends: vi.fn(), getLinkedServer: vi.fn() }
}));
```

After adding/changing tests, also run `yarn run check` (svelte-check) - not
`yarn check`, which is yarn's own built-in command and silently runs
something else entirely.

## Storybook stories (every component, no exceptions)

**Every `.svelte` file under `desktop/src/lib/components` has a story.** A new
component isn't done without one, and a changed component's stories get
updated when its props or states change. This is enforced:
`src/test/stories/coverage.test.ts` runs in `yarn test` and fails, naming the
file, for any component that no story imports by its `$lib` path.

Where it goes: `desktop/src/test/stories/components/`, mirroring the
component's folder (`atoms/`, `molecules/`, `organisms/`, `templates/`,
`icons/`, and `root/` for files directly in `components/`). Name it
`<kebab-name>.stories.svelte`, titled `Components/<Layer>/<Name>`. Copy the
shape of a neighbour, e.g. `molecules/event-rsvp-card.stories.svelte`:

```svelte
<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventRsvpCard from '$lib/components/molecules/EventRsvpCard.svelte';
  import { EVENT_HOSTED, EVENT_INVITED } from '../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Molecules/EventRsvpCard',
    component: EventRsvpCard,
    args: { event: EVENT_HOSTED }
  });
</script>

<Story name="Going" />
<Story name="Awaiting answer" args={{ event: EVENT_INVITED }} />
```

What a story set should cover:

- **One story per state a user can actually see**: loaded, empty, loading,
  error, and each variant a prop switches (RSVP status, `featured`, creator
  vs invitee). The same states the component's `.test.ts` covers.
- **Long content** where layout could break: long titles, unbroken URLs,
  narrow containers. Wrap the component in a sized `<div>` as the story's
  children.
- Callbacks as `fn()` from `storybook/test`, so clicks show in the Actions
  panel.

Data and mocking:

- **Fixtures from `src/test/stories/fixtures.ts`**, not inline objects. Add
  one there if it's missing; times relative to now via `at(dayOffset, hour)`.
- **Components that call `api`**: stub with `mockApi({...})` from
  `src/test/stories/mockApi.ts` in `beforeEach`, and *return* its result
  (that's the cleanup). `delay(value)` shows an in-flight state, `pending()`
  a permanent loading state. Stubs are typed against `api`, so a wrong
  response shape fails `yarn run check`. Fix the fixture, don't cast it.
- **Stores** (`user`, `unreadNotificationCount`): `.set()` them in the same
  `beforeEach` and reset them in the cleanup.
- **`$app/stores` page**: `parameters.sveltekit_experimental.stores.page`,
  see `templates/frame.stories.svelte`.
- ⚠️ **The site is public** (`/storybook/` on the deployed frontend). Fixture
  data only: never a real backend URL, never real user data.

Gotchas already hit here:

- A prop named `events` (or any Svelte mount option) must be passed through
  a `template` snippet, not `args`. See `templates/calendar.stories.svelte`.
- `Header`'s `user` prop is the *store*, not a `User`.
- Snippets in a `<script module lang="ts">` file need typed parameters:
  `{#snippet row(args: ComponentProps<typeof X>)}`.

Before calling it done, run **`yarn test:storybook`**. It builds Storybook
and opens every story in Chromium, failing on any that throws or logs an
error; type-checking and the build both pass on a story that crashes on
mount. CI runs it as the **Storybook** job.

## Checklist before calling a feature done

- [ ] Pure logic pulled out and unit-tested where it was easy to do so
- [ ] Service-layer DB logic covered via `#[sqlx::test]`
- [ ] Any Discord/external-HTTP call covered via `wiremock`, base URL threaded
      through as a parameter so it's mockable
- [ ] At least one functional (router-level) test for new endpoints where
      the wiring itself (route, auth, status/shape) is worth locking down
- [ ] New Svelte components have a co-located `.test.ts` covering their
      states (empty/loading/error/loaded, key interactions)
- [ ] Every new or changed component has stories for those same states
      under `src/test/stories/components/` (`coverage.test.ts` enforces
      that one exists), and `yarn test:storybook` passes
- [ ] `cargo test`, `cargo clippy -D warnings` (backend) and `yarn test`,
      `yarn run check` (frontend) all pass, with no *new* warnings beyond
      this repo's known pre-existing ones
