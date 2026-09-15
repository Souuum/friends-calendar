---
name: mockup-responsive-add-friends
description: Make the Add friends page responsive per mobile mockup screen 06 - full-bleed sheet-style layout below md. Depends on mockup-responsive-shell. Use when asked to make the add-friends screen responsive.
---

# Responsive add-friends

Source: `Friends Calendar Mobile.dc.html` screen 06 (Add friends).
**Requires** `mockup-responsive-shell` first. Smallest of the responsive
skills - `desktop/src/routes/friends/add/+page.svelte` is already a full
SvelteKit route (not a modal), so most of the mockup's "modal sheet" feel
already exists structurally.

## What to build

- Below `md:`, drop side padding/max-width constraints so the form,
  pending-requests list, and invite-link card go full-bleed edge-to-edge
  like the mockup (`Cancel` / title / `Send` header row already likely
  exists in some form - check the current header and restyle to match the
  mockup's three-part header row rather than adding a second one).
- Verify the username-search input and pending-request rows don't overflow
  at 402px; the invite-link card's `frcal.app/i/...`-style text should
  truncate or wrap rather than causing horizontal scroll (check whether
  this app's invite-link feature exists yet at all before assuming it does
  - `.claude/skills/mockup-friend-requests/SKILL.md` covers what was
  actually built for `/friends/add`; if there's no invite-link UI today,
  don't add one here, this skill is layout-only).

## Tests

No new behavior, so no new test cases expected beyond confirming existing
`friends/add/page.test.ts` still passes after any markup changes.

## Verification

`yarn test`, `yarn run check`, `yarn build`, manual resize check. No
backend changes.
