---
name: mockup-calendar-redesign
description: Rebuild the desktop Calendar screen (month + week) to match the "new calendar design" in the Friends Calendar Mockups project - a Free-tonight bar, functional filter chips, and a persistent side peek-panel with inline RSVP, replacing today's hover-tooltip/modal interaction. Backend-free - uses only endpoints that already exist. Use when asked to implement the redesigned calendar or its Free-tonight/peek-panel UI, or as the template for extracting exact values from any other mockup-* screen.
---

# Calendar redesign: Free-tonight bar, filters, peek panel

Source: Claude Design project `Friends Calendar Mockups.dc.html`
(`0b825812-c8f2-4f1a-98d7-38900c6a0133`), the `calendar` screen (`isCalendar`
section). Executed first (alongside `mockup-responsive-shell`) because it
needs **no new backend** - `GET /api/availability/friends-now` and
`GET /api/availability/week` already exist but were never wired into the
Calendar screen itself.

**Post-mortem note:** the first pass at this skill (2026-09-15) shipped a
UI that used the *right components in the right places* but the *wrong
colors, font, and spacing* almost everywhere, because the skill described
things in prose ("styled like the mockup's rounded bordered bar") instead
of extracted values. This revision replaces every such description with a
literal, checkable value pulled from the mockup's own inline `style="..."`
strings - the mockup is live HTML, not a picture; there is no excuse for
approximating a value that's sitting in the source in plain text. See
"What went wrong last time" at the bottom for the specific list, kept as a
regression checklist.

## Step 0: extract exact values before writing any component

Do this before touching `Calendar.svelte`. Fetch the mockup screen's HTML
via `DesignSync` (`get_file` on `Friends Calendar Mockups.dc.html`) and
read the literal `style="..."` attributes for every element you're about
to build - don't rely on memory of "roughly what it looked like" from an
earlier read in the same session; re-fetch if the exact string isn't in
front of you. For each element, record:

- **Color**: the literal hex or `var(--token)` used for `background`,
  `color`, `border`. If it's a `var(--token)`, resolve it against the
  mockup's own `:root`/`[data-theme]` block (near the top of the file) to
  get the hex.
- **Spacing**: the literal `padding`/`gap`/`margin` values, in px, exactly
  as written - `padding:11px 14px` means `padding:11px 14px`, not "looks
  like p-3ish." Tailwind's default scale (4px steps) will not always land
  on the mockup's number; use an arbitrary-value class
  (`px-[14px] py-[11px]`) rather than rounding to the nearest step that
  isn't actually correct.
- **Border/radius**: literal `border-radius` in px, mapped to the closest
  *exact* Tailwind radius (`rounded-lg` = 8px, `rounded-xl` = 12px,
  `rounded-2xl` = 16px) - if the mockup's value doesn't hit one of those
  exactly, use `rounded-[Npx]`, don't pick the nearest named class.
- **Typography**: `font-family`, `font-size`, `font-weight`,
  `letter-spacing`, `text-transform` - all four, every time, not just size.
- **Component state**: for anything interactive, the mockup usually
  encodes state as a ternary in its script (`on ? styleA : styleB`) -
  capture *both* branches, not just whichever one happens to be visible in
  the screenshot-like default render.

If a value truly isn't present anywhere in the fetched HTML/script (e.g.
the mockup relies on a hover state that isn't statically renderable, or a
section got truncated in your fetch and re-fetching still doesn't show
it), **say so explicitly in the resulting code as a comment**, and do one
of: (a) re-fetch the specific screen/section via `DesignSync` again with a
narrower target, (b) ask the user for the exact spec value rather than
guessing, or (c) if it's genuinely inferrable from an identical pattern
used elsewhere in the *same* mockup file (e.g. every other eyebrow label
in this file uses the same JetBrains Mono/10px/0.1em/uppercase treatment),
use that pattern but flag in a comment that it was inferred by pattern,
not read directly for this element - so a future pass knows to verify it
rather than trusting it as confirmed.

## Design tokens - add these to `app.css` before writing components

The mockup defines its own CSS custom properties (`:root` block at the top
of the file). Cross-reference every one you'll use against
`desktop/src/app.css`'s `@theme` block **first** - do not write a
component against a color that doesn't have a real token yet, and do not
substitute a Tailwind stock color "because it looks close."

| Mockup token | Hex (light theme) | `app.css` status | Action |
|---|---|---|---|
| `--accent-text` | `#5030e5` | `--color-primary` exists | Use `bg-primary`/`text-primary`/`border-primary` |
| `--tint` | `#eee8ff` | Not defined (`--color-primary-hover` is a translucent overlay, not the same thing) | Add `--color-tint: #eee8ff;` |
| `--line` | `#ebebeb` | Not defined (components fell back to Tailwind's `gray-200` = `#e5e7eb`) | Add `--color-line: #ebebeb;` |
| `--muted` | `#7c7c83` | Not defined (fell back to `gray-400`/`gray-500`) | Add `--color-muted: #7c7c83;` |
| `--body` | `#5c5c61` | Not defined (fell back to `gray-600`/`gray-700`) | Add `--color-body: #5c5c61;` |
| `--surface` | `#ffffff` | `--color-white` exists | Use `bg-white` |
| yellow (Maybe status) | `#fbb13c` | `--color-yellow: #fbb13c` **already exists** | Use `bg-yellow`/`text-yellow` - **do not use Tailwind's stock `yellow-100/500/800`**, they resolve to a different hex than this app's own token |

**Status colors are not green/red/yellow.** The mockup's actual mapping
(from its own script, `STATUS` object) is:

```js
accepted: { bar: primary, bg: 'var(--tint)',              fg: 'var(--accent-text)' }  // "Going" - tint pill, NOT green
maybe:    { bar: yellow,  bg: 'rgba(251,177,60,0.18)',     fg: '#a9700f' }             // "Maybe"
declined: { bar: red,     bg: 'rgba(251,44,44,0.14)',      fg: '#c01a1a' }             // "Can't"
pending:  { bar: 'var(--muted)', bg: 'var(--subtle)',      fg: 'var(--body)' }         // "No answer"
```

This app has a pre-existing `getStatusColor`/`statusColor` helper
(`EventDetailsModal.svelte`, copied into `EventRsvpCard.svelte`,
`AnnouncementPostCard.svelte`, and this skill's `EventPeekPanel.svelte`)
that uses `bg-green-100 text-green-800` for `accepted`. **Do not copy that
helper's colors into new mockup-driven components** just because you're
reusing its *behavior* (the RSVP call, the button layout) - behavior and
color are separable; check the color independently every time, even when
the interaction pattern is a legitimate reuse. Also use the mockup's own
status *labels* ("Going" / "Maybe" / "Can't" / "No answer"), not the raw
enum values (`accepted`/`maybe`/`declined`/`pending`) - if a participant
list is rendering `{participant.status}` directly, that's the enum, not
the label; map it through a `label()` helper.

## Typography - JetBrains Mono is a signature, not decoration

Every eyebrow/label/timestamp string in this mockup ("Free tonight", "N
invited", date/time text, "Best overlap this week") uses:
`font-family:'JetBrains Mono',monospace; font-size:10px (or 11px);
letter-spacing:0.1em; text-transform:uppercase; color:var(--muted);` (no
`font-weight` override → normal/400, **not** semibold).

This font is loaded via a Google Fonts `<link>` in the mockup's `<helmet>`
block and is **not currently loaded anywhere in this app**
(`desktop/src/app.html` has no font `<link>` at all). Before using it:

1. Add to `app.html`'s `<head>`:
   ```html
   <link rel="preconnect" href="https://fonts.googleapis.com" />
   <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
   <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet" />
   ```
2. Add `--font-mono: 'JetBrains Mono', monospace;` to `app.css`'s `@theme`
   block so `font-mono` resolves to it (Tailwind's default `font-mono` is
   a system stack, not this specific font).
3. For letter-spacing 0.1em, use `tracking-widest` (Tailwind: 0.1em) -
   **not** `tracking-wide` (0.025em, less than a third as wide - this was
   the exact mistake made last time).

Everything else in the mockup uses `-apple-system, BlinkMacSystemFont,
'Segoe UI', 'Helvetica Neue', sans-serif` (already this app's default body
font, no change needed) - JetBrains Mono is *only* for the small
label/timestamp/mono-number text described above, not general body copy.

## Component specs (measured from the mockup, 2026-09-15 fetch)

**Free-tonight bar**: `bg-white border border-line rounded-[11px]
px-[14px] py-[11px] gap-3`. "Free tonight" label per the Typography
section above. Avatars: 26×26px (`w-[26px] h-[26px]`), `border-2
border-white`, overlapped `-mr-[7px]`. "N friends have nothing on":
13px, default ink color, no explicit weight. A 1px×18px `bg-line` divider
sits between the avatar group and the "Best overlap" text (that text is
**intentionally omitted** in this app per the skill's own earlier
reasoning about not fabricating a real-looking-but-fake computed overlap
figure - keep that decision, but the divider itself is a layout element,
not a data claim; there's no reason to have dropped it too, add it back
if there's a reason to keep visual balance, otherwise the bar can end
after the avatar count). "Propose a time" button:
`border border-primary bg-white text-primary rounded-lg px-3 py-[7px]
text-xs font-semibold` (this one was close in the first pass - keep it as
the reference for what "close" should look like everywhere else).

**Peek panel (`EventPeekPanel.svelte`)**: `w-[296px] bg-white border
border-line rounded-xl p-[18px]`. A colored top accent strip
(`h-1 rounded-full mb-3.5`, color = the event's status `bar` color from
the STATUS table above) sits above the date/status row - this was missing
entirely in the first pass, add it. Date/time row: JetBrains Mono, 11px,
`text-muted` (no letter-spacing/uppercase here - that eyebrow treatment is
for section labels like "Free tonight", not for a date value), next to a
status pill using the STATUS colors (tint/fg, not green). Title:
`text-[17px] tracking-[-0.01em] font-bold` - **note the `font-bold` is
required, not optional**: the mockup's raw `<h2>` relies on the browser's
native bold-by-default heading style, but this app runs Tailwind Preflight,
which resets `h2` to `font-weight: inherit` (i.e. normal) - omitting an
explicit weight class here, as an earlier draft of this skill said to do,
would under-weight the title relative to the mockup, not match it; the
"don't force a weight" instinct only holds in an unreset browser context.
Location/price: `text-xs text-body` (use the new `--color-body` token, not
`text-gray-600`), `gap-[5px]`.

**RSVP action row** (source: the `peek.notMine`/`peek.mine` branches) -
these are two *different* rows depending on whether you created the
event, not one row with conditional active-state styling:
- Not your event: `Going` = `flex-1 py-2 rounded-lg text-xs font-semibold
  bg-primary text-white` (solid fill, **always** - it's the primary
  action, not a "you already said yes" indicator); `Maybe`/`Can't` =
  `border border-line bg-white text-muted` (outlined, muted, no fill),
  with `Can't` turning `border-red text-red` **only on hover**. There is
  **no tint/fg "currently selected" treatment on these buttons** -
  that pattern belongs to the small status pill in the header row above,
  not to the action buttons. (An earlier draft of this skill claimed the
  opposite - traced back, that claim wasn't sourced from this exact
  element, it was inferred by analogy from the STATUS map used elsewhere.
  Lesson: a color/style claim that isn't paired with the literal
  `style="..."` string it came from should be treated as unverified, even
  in this skill's own text - re-check it against source before trusting
  it, the same rule as re-checking the app.)
- Your own event: `Edit` and `Nudge no-answers` (not RSVP buttons at all),
  same outlined/muted button style as `Maybe`/`Can't` above. The current
  build just hides the RSVP row entirely for `is_creator` events with no
  replacement - add these two actions rather than leaving a blank gap
  where the mockup has a full-width two-button row (`Edit` can be a no-op/
  disabled placeholder if there's no edit-event flow yet; don't block this
  fix on building one, but don't render nothing either).

**Header/toolbar** (existing `CalendarHeader.svelte`, extended by this
skill, not written fresh - verify it against the mockup too since it sits
directly next to new elements): prev/next buttons are `32px × 32px`,
`border border-line bg-white rounded-lg`, not bare icon buttons with only
padding. They, the title, and "Today" all sit together on the left; the
Month/Week toggle and "+ New Event" are pushed right as a separate group
- if the current markup interleaves them differently, that's a layout
bug, not a style nit. "Today": `border border-line bg-white rounded-lg
px-3 py-[7px] text-xs font-semibold` (not `text-sm font-medium` with no
border). "+ New Event": `bg-primary` (**not** `bg-secondary` - that
button is currently pink; the mockup wants primary purple, matching
"Propose a time"'s outline color, since both open the same modal and
should read as the same action family).

**Filter chips** (source: `calFilters:` in the mockup's script - re-fetched
and confirmed, this was left unmeasured in the first revision of this
skill): labels are **"All events" / "Going" / "Awaiting my answer" /
"Created by me"** - not "Awaiting my response" / "Mine", which is what got
built; match the mockup's exact copy, not a paraphrase. Style, per chip,
`on` = whether it's the active filter:
`padding:7px 12px; border:1px solid {on ? primary : line}; background:
{on ? tint : white}; color:{on ? accent-text : muted}; border-radius:8px;
font-size:12px; font-weight:600;` → Tailwind:
`px-3 py-[7px] rounded-lg text-xs font-semibold border` +
(`border-primary bg-tint text-primary` when active,
`border-line bg-white text-muted` when inactive). This is the *same*
tint/accent-text "active" pattern the RSVP buttons above do **not** use -
don't cross-apply one section's active-state convention to another
without checking each one's own source.

## What to build (behavior, unchanged from the original scope)

1. **Free-tonight bar**: `api.getFreeFriendsNow()` on mount. "Best overlap
   this week" text: don't add a new backend aggregate for this - see
   above, this was a deliberate scope decision, not a styling gap.
   "Propose a time" opens `CreateEventModal` - no time-suggestion logic.
2. **Functional filter chips**: client-side filter over the `events` prop
   already passed into `Calendar.svelte`. **Reactivity gotcha**: Svelte's
   `$:` dependency tracking is static - it only sees identifiers that
   appear *directly* in the reactive statement's own source text, not
   identifiers read inside a function that statement merely calls by
   reference. `$: filteredEvents = events.filter(matchesFilter)` will
   **not** re-run when a variable read inside `matchesFilter`'s body
   changes - write `$: filteredEvents = events.filter((e) =>
   matchesFilter(e, activeFilter))` instead, with `activeFilter` appearing
   literally in the `$:` line. The same applies to any derived function
   passed as a prop to a child (e.g. `eventsForDay`): if a child only
   re-invokes a function prop when *the prop's reference itself* changes,
   declare that function with `$:` too, not as a plain top-level
   `function` declaration, or the child will silently keep rendering
   stale data after the first render. This isn't a style bug but it will
   look like one (chips that appear to do nothing) - the two bugs from
   the first pass were exactly this, caught by `Calendar.test.ts`'s filter
   test failing, not by inspection.
3. **`EventPeekPanel.svelte`** (new, `organisms/`): selected event details
   + inline RSVP, replacing month view's tooltip-only interaction and
   week/day's modal-on-click at `md:` and up (mobile keeps the existing
   modal via `mockup-responsive-calendar`, not built in this pass).
   `EventDetailsModal.svelte` stays, reused later as the mobile bottom
   sheet - don't delete it.

## Tests

Component test for `EventPeekPanel.svelte` (presentational, props-driven).
A `Calendar.svelte` test covering the free-tonight bar and that filter
chips actually change which events are visible.
**Testing-library gotcha**: `render(Component, { events: [...] })` will
silently drop an `events` prop, because `@testing-library/svelte`'s
`render()` second argument doubles as Svelte's own mount-options object,
and `events` is one of Svelte's reserved option keys (`target`, `anchor`,
`props`, `events`, `context`, `intro`). Any prop sharing one of those six
names needs the explicit wrapper: `render(Component, { props: { events:
[...] } })`. Full conventions in `.claude/skills/add-tests/SKILL.md`.

## Self-verification (do this before declaring the screen done)

1. Start the dev server (`yarn dev` from `desktop/`, or the Tauri dev
   window) and actually open the Calendar screen - `yarn test`/`yarn run
   check`/`yarn build` passing verifies the code compiles and the tested
   *behavior* works, it does not verify the screen *looks* like the
   mockup. Do not report this skill done on the strength of green tests
   alone.
2. Re-open the mockup screen (`DesignSync get_file` again, or the
   project's live preview if the user has it open) side by side with the
   running app.
3. Walk the "Component specs" table above element by element against what
   actually rendered. For each row, note: match / close-but-off (say by
   how much) / missing / wrong.
4. List every remaining delta explicitly in your response to the user -
   including ones you're choosing not to fix (e.g. a deliberate scope cut)
   and any you couldn't verify against exact source (state so explicitly;
   don't present an inferred/pattern-matched value with the same
   confidence as one read directly from a `style="..."` string - see the
   RSVP-button correction above for what happens when that line gets
   blurred). Do not silently ship a partial match and call it done; "done"
   means either every row matches or every mismatch is named and
   justified.
5. If new tokens were added to `app.css` (`--color-line`, `--color-muted`,
   `--color-body`, `--color-tint`, `--font-mono`) or `app.html`
   (the font `<link>`), grep the rest of the app for places that could
   now use the real token instead of a stock-Tailwind approximation they
   were using as a stand-in before this skill ran - fixing the token
   source without updating already-existing lookalike usages elsewhere
   leaves the app visually inconsistent between old and new screens.

## What went wrong last time (regression checklist for this exact skill)

- [ ] `--color-line`/`--color-muted`/`--color-body`/`--color-tint` added
      to `app.css`, not substituted with Tailwind stock grays
- [ ] `--color-yellow` (already existed) actually used instead of stock
      `yellow-*`
- [ ] Status colors follow the STATUS table (tint/accent-text for
      "accepted"), not green/red/yellow-100
- [ ] Status *labels* ("Going"/"Maybe"/"Can't"/"No answer") shown, not raw
      enum values
- [ ] JetBrains Mono loaded (`app.html`) and used for every eyebrow/label/
      timestamp string, with `tracking-widest` not `tracking-wide`
- [ ] Free-tonight bar padding is `14px 11px` via arbitrary values, not
      the nearest Tailwind step
- [ ] Peek panel's colored top accent strip is present
- [ ] Peek panel title uses `font-bold` explicitly (Tailwind Preflight
      strips the browser's native `<h2>` bold, so omitting a weight class
      under-weights it relative to the mockup)
- [ ] Peek panel RSVP row: `Going` solid `bg-primary text-white` always
      (not status-conditional), `Maybe`/`Can't` outlined/muted, `Can't`
      red only on hover - not a tint/fg "selected" pill anywhere in this
      row (that pattern belongs to the filter chips and the status badge,
      not the RSVP buttons - don't cross-apply it)
- [ ] Creator's own event shows `Edit`/`Nudge no-answers` instead of a
      blank gap where RSVP buttons would be
- [ ] "+ New Event" is `bg-primary`, not `bg-secondary`
- [ ] Header groups (`‹ › title Today` vs `toggle + New Event`) match the
      mockup's left/right split
- [ ] Filter chips use the mockup's exact labels ("Awaiting my answer",
      "Created by me") and the tint/accent-text active-state styling
