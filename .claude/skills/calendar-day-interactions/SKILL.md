---
name: calendar-day-interactions
description: Make month-grid day cells actually do something - tap a day to filter the list beneath it, long-press an empty day to start an event on that date. The cells already claim to be buttons and aren't. Not yet executed as of 2026-09-17. Use when asked about tapping days, creating an event on a specific date, or the month grid feeling inert.
---

# The day cells already say they're buttons

`MonthView.svelte` renders every day cell with `role="button"`,
`tabindex="0"` and `cursor-pointer` - and **no click handler**. The only
thing bound is `mouseenter`/`mouseleave` for the tooltip. So the grid
announces itself to a screen reader as 35 buttons, shows a pointer cursor
on hover, and does nothing on click or Enter.

That is worse than a plain `<div>`: it is a promise the component doesn't
keep, and it's the same family of defect as the disabled-looking-live
controls this codebase keeps having to undo (`bg-discord-blurple` compiling
to nothing, "Nudge no-answers", the settings page that couldn't save).

`Friends Calendar Mobile.dc.html` screen 01 says what the promise should
be: *"Swipe the grid sideways for months. Tap a day to filter the list
under it; long-press an empty day to start an event there."*

## Scope this skill to the two tap behaviours

Swiping between months is a gesture-layer concern and is deliberately **out
of scope** - there are no touch gestures anywhere in this app yet, and
building a gesture abstraction as a side effect of adding a click handler
is how you end up with one component owning a swipe library. Note it and
leave it.

## Do these first

Nothing. Self-contained, frontend-only, no migration and no endpoint.

## Behaviour 1: tap a day to filter

- Selecting a day sets state on `Calendar.svelte` (which already owns
  `selectedEvent`, `activeFilter` and `currentDate` - keep them together).
- Below `md:` the mockup shows the selected day's events as a list under
  the grid, headed "Tuesday 15 / 2 events". From `md:` up there is no room
  question: the peek panel is the detail surface, so a day tap should
  filter without competing with it.
- ⚠️ **Tapping a day and tapping an event inside that day are different
  actions.** The event chip is *inside* the cell, so a naive handler on the
  cell fires for both. The chip's handler must `stopPropagation`, and there
  must be a test that clicking a chip opens the peek panel and does **not**
  also change the selected day.
- **Tapping the selected day again clears it.** Otherwise the only way out
  of a filter is to find an empty day, and on a busy month there isn't one.

## Behaviour 2: long-press an empty day to create

- `CreateEventModal` is already create-or-edit via one nullable `event`
  prop. This needs a third input - a *date* to prefill - and the temptation
  is a second boolean. Don't: add one optional `initialDate: Date | null`
  prop and have the existing prefill logic read it when `event` is null.
- ⚠️ **Long-press has no native event.** Implement it as a `pointerdown`
  timer cancelled by `pointerup`, `pointercancel`, `pointerleave` **and any
  movement past a small threshold** - without the movement check, scrolling
  the grid with a finger fires it. Put it in `lib/actions/` next to
  `clickOutside` and `dismissable`; that's where this codebase keeps
  behaviours that attach to a node.
- **Desktop needs an equivalent that isn't a long press.** Nobody holds a
  mouse button down on a calendar. Double-click is the convention; the
  mockup only specifies the phone. Offer both from one action.
- "Empty day" in the caption is a hint, not a rule - long-pressing a day
  that already has events is still a reasonable way to add another. Allow
  it rather than building an exception.

## The trap this will hit

`eventsForDay` is declared with `$:` **specifically so its reference
changes** when `filteredEvents` does - a child only re-invokes a function
prop when the prop's own reference changes, not when something its closure
reads changes underneath it. See the `matchesFilter`/`eventsForDay` note in
CLAUDE.md; this is the third instance of that family in this codebase.
Any new derived value here (the selected day's events, say) must be
declared with `$:` and must name every input **directly in the `$:` line**,
because Svelte's dependency tracking is static.

## Tests

- **Component**: clicking a day selects it; clicking it again clears it;
  clicking an event chip opens the peek panel and leaves the selection
  alone.
- **Component**: long-press (or double-click) opens the create modal with
  the date prefilled - assert the *value in the input*, not that a flag
  was set.
- ⚠️ **The long-press cancellation needs a real browser.** happy-dom has no
  layout and no pointer-movement model, so "scrolling doesn't trigger a
  long press" is unassertable there. Put it in `e2e/` - the same reason
  `modal-dismiss.spec.ts` lives there.
- **Accessibility**: Enter and Space on a focused day do what a tap does.
  The cells have claimed `role="button"` since they were written; this is
  the skill that makes that true.

## Done when

The grid's pointer cursor is no longer a lie, and a fresh account can
create its first event by long-pressing the day they mean.
