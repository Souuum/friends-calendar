---
name: mockup-responsive-create-event
description: Give CreateEventModal a real two-step wizard below md (step 1 when/where/visibility, step 2 invite + Discord preview) per mobile mockup screens 12-13, while desktop keeps today's single-scroll modal. The user explicitly confirmed a real wizard over a responsive single form. Depends on mockup-responsive-shell. Use when asked to implement the mobile create-event flow.
---

# Responsive create event: two-step wizard

Source: `Friends Calendar Mobile.dc.html` screens 12 (New event · when &
where) and 13 (New event · invite). **Requires** `mockup-responsive-shell`
first. The user was asked explicitly whether mobile should get a real
two-step flow or just a responsively-stacked single form, and chose the
**real wizard** - same component, conditional layout, not two components.

## Current state

`desktop/src/lib/components/CreateEventModal.svelte` (252 lines) is a
single scrollable form: `title, startTime, endTime, location, visibility,
price, link` fields plus an already-built invite picker (`friends`,
`selectedFriendIds`, from `mockup-friends-directory`) that sends
`participant_ids` in the `api.createEvent(...)` payload. No Discord-preview
panel exists yet.

## What to build

1. **Add a `step` state** (`1 | 2`), gated on the `md:` breakpoint - at
   `md:` and up, ignore `step` entirely and render everything as it does
   today (single scroll, unchanged); below `md:`, render only the current
   step's fields.
2. **Step 1** ("1 / 2" header, mockup screen 12): title, starts/ends,
   location, price/link, visibility picker (the mockup shows visibility as
   three selectable cards with a checkmark - if the current visibility
   control is a `<select>`, restyle it as cards below `md:` reusing the
   same bound `visibility` value, don't fork the state). "Next · invite
   friends" button advances `step` to 2 - validate the same fields the
   current single-step submit already validates (end after start, title
   required) before allowing advance, so step 2 can't be reached with
   invalid step-1 data.
3. **Step 2** ("2 / 2" header, "‹ Back" returns to step 1 **without**
   clearing step-1 field values - keep them in the same component-level
   `let` bindings, don't reset on step change): the existing invite picker,
   plus a new **Discord preview** panel showing what
   `services::discord_announcement::DiscordAnnouncer::format_event_message`
   actually produces - don't invent new preview copy that could drift from
   the real announcement; either call a preview-only backend endpoint that
   reuses `format_event_message` (if one doesn't exist, a plain
   client-side reconstruction using the same field order/labels is
   acceptable *only if* kept next to a comment noting it must be updated
   if `format_event_message` changes - prefer exposing a real preview
   endpoint if time allows, to avoid that drift risk entirely). Submit
   button ("Create & post to Discord") calls the existing
   `api.createEvent(...)` - no payload shape change needed, `step` is
   pure UI state.

## Tests

Extend `CreateEventModal.test.ts` (check if one exists first) with cases
for: step 1 won't advance with invalid fields, step 2's "Back" preserves
step-1 values, and the final submit payload matches what the single-step
desktop path already sends (same `participant_ids`/field shape) - these
two code paths must not drift into sending different payloads for the same
user input.

## Verification

`yarn test`, `yarn run check`, `yarn build`, manual resize check. Backend
change only if a preview endpoint is added (see step 2) - if so, follow
`.claude/skills/add-tests/SKILL.md` for backend test tiers and re-run
`cargo test`/`cargo clippy`.
