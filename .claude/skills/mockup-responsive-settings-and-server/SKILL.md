---
name: mockup-responsive-settings-and-server
description: Make the Settings and Discord server pages responsive per mobile mockup screens 10-11 - single-column "Me" layout, Server reachable as a pushed sub-page from Settings on mobile. Depends on mockup-responsive-shell. Smallest responsive skill. Use when asked to make settings/server responsive.
---

# Responsive settings + server ("Me")

Source: `Friends Calendar Mobile.dc.html` screens 10 (Me) and 11 (Discord
server). **Requires** `mockup-responsive-shell` first (the "Me" bottom tab
already points at `/settings` per that skill).

## What's already close

`/settings` (profile/timezone/default-visibility/notification-toggles/
delete-account) and `/server` (linked server card/channel config/digest
toggle/permissions) are already split into narrow, mostly-single-column
pages (`max-w-2xl mx-auto` containers) - this is the smallest responsive
skill, a verification-and-polish pass, not a rebuild.

## What to build

- Verify both pages render cleanly at 402px: form inputs, the delete-
  account confirmation flow, and the channel-config inputs shouldn't
  overflow or force horizontal scroll. Fix spacing, don't restructure.
- `/settings`: below `md:`, restyle the profile header row (avatar +
  username) to match the mockup's card treatment if it doesn't already
  look like one, and ensure the "Discord server" link/section reads as a
  clear push-through (chevron affordance, like the mockup's `TH ›` row)
  rather than an inline card, matching how mobile treats Server as a
  sub-page of Me.
- `/server`: below `md:`, the "‹ Me" back-link styling from the mockup
  (already how SvelteKit routing/back-nav works here, just needs the
  visual treatment) plus verifying the bot-channel-config rows and
  permission-chip wrap cleanly at 402px.

## Tests

No new behavior expected - existing `settings/page.test.ts` and
`server/page.test.ts` should keep passing unchanged. Add a test only if a
new interactive element is introduced (e.g. if the Settings→Server link
becomes a distinct new component worth testing in isolation).

## Verification

`yarn test`, `yarn run check`, `yarn build`, manual resize check. No
backend changes.
