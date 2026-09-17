---
name: invite-friend-to-event
description: Invite a specific friend from their profile or from the friends list - to an event that already exists, or to a new one started with them preselected. /friends/[id] currently has no invite action at all. Best paired with availability-best-overlap. Not yet executed as of 2026-09-17. Use when asked about inviting someone, the friend profile's missing action, or starting an event with a particular person.
---

# You can see a friend, and do nothing about it

`/friends/[id]` shows who someone is, when they're free this week, and
which events you share. It offers **no way to invite them to anything** -
grep it for "invite" and there are zero hits. To get a friend into an
event you must go to the calendar, open the create modal, and find them in
the invite picker.

Both mockups treat this as the friend screen's primary action.
`Friends Calendar Mobile.dc.html` screen 05: *"The invite button is pinned
above the home indicator."* Screen 04, on the list: *"Swipe a row left to
invite them to something; tap for the profile."*

## Do these first

- **`.claude/skills/availability-best-overlap/SKILL.md`**, if it's planned.
  This screen already shows "Free this week" for one person, which is
  exactly the input for "suggest a time you're both free" - and inviting
  someone to a slot they're free for is the whole point. It works without
  it; it's much better with it.

## The endpoint already exists

`POST /api/events/:id/participants` (`handlers::calendar::invite_participants`)
takes participant ids and is already wired into `create_event`'s notification
trigger. **Do not add a second invite path.** This skill is mostly a
frontend one plus a list endpoint.

What's missing server-side is the question "which of my events could I add
this person to?" - i.e. my upcoming events where they are *not* already a
participant. That is a real query and belongs on the backend, not as a
client-side filter over `GET /api/events`, because the client would have to
fetch every event to answer it.

Suggested: `GET /api/events/invitable?user_id=<uuid>` - my upcoming events,
excluding ones that user is already on.

⚠️ **Guard it the way the availability endpoints are guarded.** Asking
about a non-friend must 400, same as `handlers::availability::week`, or
this becomes a way to enumerate a stranger's event membership.

## Decide first

1. **Two actions or one?** The mockup's button is singular. But "invite to
   an existing event" and "start a new event with them" are different
   flows, and an app with only the first is useless when you haven't made
   the event yet. Recommend one button opening a sheet that offers both:
   a short list of upcoming events, and "New event with {name}" beneath it.
2. **What does "New event with them" do?** Open `CreateEventModal` with
   that friend preselected in the invite picker. The modal already takes
   `participant_ids` on create - preselect, don't bypass the picker, so the
   user can still add others before saving.
3. **Swipe-to-invite on the list** (mockup screen 04) is a gesture and
   there are no gestures in this app yet. Out of scope here - the profile
   button is the same capability without inventing a gesture layer. Note
   it rather than half-building it.

## The bit that will be wrong

`/friends/[id]` already computes shared events, and it had to learn
`is_participant` to do it correctly - CLAUDE.md records that "Shared
events" would otherwise list events only *the friend* is in. The invitable
list has the mirror-image bug waiting: it must exclude events **they** are
already on, and include events **you** created but haven't invited anyone
to. Write the test for both directions before the query.

Also: an event you can see but didn't create is not necessarily one you can
invite to. `invite_participants` already decides that - **read what it
enforces and make the list agree**, rather than inventing a second rule.
Two places disagreeing about who may invite is the same class of bug as the
listing/fetch mismatch that has already bitten twice in this codebase.

## Tests

- **Integration (`#[sqlx::test]`)**: the invitable list excludes events the
  friend is already on, excludes events that have already started, and
  includes your own uninvited ones. One test per clause - a single
  omnibus test will pass for the wrong reason.
- **Functional**: asking about a non-friend 400s; inviting through the
  existing endpoint still creates the `event_invite` notification (it will,
  via `services::notifications::create`'s gate - assert it rather than
  assuming).
- **Component**: the button opens the sheet; picking an event calls
  `api.inviteParticipants` with that event and that friend; "New event
  with…" opens the modal **with the friend already selected** - assert the
  selection, not that a prop was passed.
- ⚠️ The sheet is a mobile surface pinned above the home indicator. Reuse
  `lib/actions/dismissable.ts` so it dismisses by backdrop, Escape and the
  back gesture like every other sheet - and put the reachability assertion
  in `e2e/`, not vitest, for the reasons in `modal-dismiss.spec.ts`'s
  header.

## Done when

Looking at a friend and thinking "we should do something" takes one tap,
and - if `availability-best-overlap` has landed - the time is suggested
rather than invented.
