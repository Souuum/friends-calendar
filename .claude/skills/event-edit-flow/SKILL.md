---
name: event-edit-flow
description: Give event editing a UI. The backend PUT /api/events/:id and the api.updateEvent() client both already exist with zero callers, and EventPeekPanel ships a permanently disabled "Edit" button. Also settles the fate of the orphaned EventDetailsModal and the unreachable delete path. Not yet executed as of 2026-09-16. Use when asked to edit, update, reschedule or delete an event.
---

# Event editing: the cheapest real feature left

Everything below the UI is already built and tested:

- `PUT /api/events/:id` -> `handlers::calendar::update_event` ->
  `services::calendar::update_event`, fetch-merge-update so only fields
  present in the request change. `price`/`link` wiring was fixed during the
  `feat(DiscordBot)` merge.
- `desktop/src/lib/api.ts::updateEvent()` exists and is **called by
  nothing**.
- `EventPeekPanel.svelte` renders `Edit` and `Nudge no-answers` as
  `disabled` placeholders for the creator, purely so the layout matches the
  mockup. They have never done anything.

So this is mostly a frontend task against a working API.

## Scope decision first

`Nudge no-answers` has **no backend at all** - there is no endpoint that
pings pending participants. Don't quietly build one as a side effect of
this skill. Either:
- (a) keep it disabled and out of scope, and say so in the commit; or
- (b) treat it as its own feature (it needs a Discord DM or channel-mention
  path through `services::discord_announcement`, plus rate-limiting
  thought - "nudge" is a button that spams people).

Default to (a). This skill is about `Edit`.

## What to build

Reuse `CreateEventModal.svelte` rather than writing a second form. It
already has every field (`title`, `description`, `start_time`, `end_time`,
`location`, `price`, `link`, `visibility`) plus the friend invite-picker,
and duplicating that is how the two drift apart.

Refactor it into create-or-edit:

- Add an optional `event: EventWithParticipants | null = null` prop. When
  present, prefill every field, set the heading to "Edit event", the submit
  label to "Save changes", and call `api.updateEvent(event.id, payload)`
  instead of `api.createEvent(payload)`.
- Keep dispatching a single event upward (`created` / `updated`, or one
  `saved`) so `Calendar.svelte` just calls `loadEvents()` either way - it
  already does this for creation via `handleEventCreated`.
- `showCreateModal` already lives in `Calendar.svelte` (it was lifted out
  of `CalendarHeader` during `mockup-calendar-redesign` so the Free-tonight
  bar and the header button share one instance). Add an
  `editingEvent: EventWithParticipants | null` alongside it rather than a
  second boolean - one piece of state that is either null (create) or an
  event (edit) can't get into a contradictory combination.
- Wire `EventPeekPanel`'s `Edit` button: drop `disabled`, dispatch an
  `edit` event with the current event, let `Calendar.svelte` set
  `editingEvent`.

### Datetime prefill gotcha

The form binds `<input type="datetime-local">`, which requires
`YYYY-MM-DDTHH:mm` in **local** time with no timezone suffix and no
seconds. The API returns RFC3339 UTC (`2026-09-20T18:00:00Z`). Assigning
that string straight into the input silently leaves the field blank in
some browsers - it does not throw. Convert both directions, and check
`dateUtils` for an existing helper before writing another one.

This is also where `users.timezone` should arguably be honoured; it is
currently stored and never used for rendering (everything goes through
`toLocaleDateString` on the browser's zone). Out of scope here, but note it
rather than pretending the field is respected.

## Delete, while you're here

`api.deleteEvent()` has two callers - `EventCardImpl.svelte` and
`EventDetailsModal.svelte`. `EventDetailsModal` is **orphaned**: since
`mockup-calendar-redesign` replaced the click-to-modal interaction with the
persistent peek panel, nothing renders it. It was deliberately kept, not
deleted, because `.claude/skills/mockup-responsive-calendar/SKILL.md`
plans to reuse it as the mobile bottom sheet.

Decide explicitly, and write the decision into CLAUDE.md:

- If `mockup-responsive-calendar` is happening soon, leave it alone and
  give the peek panel its own delete affordance.
- If not, delete `EventDetailsModal.svelte` and move its delete flow into
  the peek panel. Dead components that look alive are how the disabled
  Edit button happened in the first place.

Either way the creator needs *some* reachable delete. Use a real
confirmation, and note `EventDetailsModal` uses `confirm()`/`alert()` -
match the surrounding app's inline-error style instead (see
`EventPeekPanel`'s `error` binding) rather than copying the browser
dialogs.

## Tests

Component tests in `EventPeekPanel.test.ts` / a new `CreateEventModal`
test:

- Creator sees an enabled `Edit`; non-creator sees the RSVP row instead
  (the existing test `shows Edit/Nudge instead of RSVP buttons for your own
  event` already covers the split - extend it rather than duplicating).
- Clicking `Edit` opens the modal prefilled with the event's values,
  including a correctly-formatted datetime-local string.
- Saving calls `api.updateEvent` with the event id and the changed fields,
  and dispatches the refresh.

`render(Component, { props: { ... } })` - always the explicit wrapper here,
because `CreateEventModal`/`EventPeekPanel` tests pass props named `event`/
`events`, and `events` collides with Svelte's own mount option and is
silently dropped by the flat shorthand.

Backend needs no new tests unless you change `update_event`; it already has
coverage.

## Done when

The creator of an event can change its details from the calendar and see
the grid update, there are no `disabled` placeholder buttons left in the
peek panel, and delete is reachable from wherever the app now shows event
details.
