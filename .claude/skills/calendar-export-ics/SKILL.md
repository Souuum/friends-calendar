---
name: calendar-export-ics
description: Publish each user's events as a subscribable iCalendar feed, so Google Calendar, Apple Calendar and Outlook can show them alongside everything else. Not yet executed as of 2026-09-17. Use when asked about exporting events, .ics, webcal, "add to my calendar", or syncing out to another calendar app.
---

# A subscribable .ics feed

One endpoint emitting RFC 5545, and Google, Apple and Outlook all understand
it. That is the whole appeal: **one implementation covers every provider**,
where importing needs a different integration per vendor (see
`.claude/skills/calendar-import-availability/SKILL.md`).

Do this one first. It takes no secrets *in*, it is small, and it proves the
shape before anything harder.

## Do these first

Nothing. Self-contained, one migration, one endpoint.

## The thing that makes this different from every other endpoint here

⚠️ **A calendar app cannot send an `Authorization` header.** It is given a URL
and it GETs it, forever, unattended. So this is the first route in the app
that authenticates on the **URL itself**, and that has consequences:

- The token is a **bearer credential in a string people paste around** - into
  Google, into a phone, sometimes into a chat. Treat it as leakable.
- It must be **revocable without touching the account**: one "regenerate my
  feed link" button that invalidates the old URL. Without that, the only
  remedy for a leaked link is deleting your account.
- It must be **per-user and unguessable** - a UUIDv4 is not enough on its
  own; use a random 32-byte value, base64url-encoded.
- ⚠️ **Do not reuse the JWT.** It expires, which would silently break the
  feed a week later, and it is a login credential - a calendar subscription
  is not a session.

## Schema (migration `017`, next free number)

```sql
ALTER TABLE users ADD COLUMN IF NOT EXISTS calendar_feed_token TEXT UNIQUE;
```

Nullable on purpose: the token is minted the first time somebody asks for
their link, so nobody who never uses the feature has a live credential
sitting in the database.

## Endpoints

- `POST /api/calendar/feed` - mint or rotate the token, return the URL.
  Authenticated normally.
- `GET /calendar/{token}.ics` - the feed. **No `Claims` extractor**; the
  token is the auth. Note it is outside `/api` because it isn't for the app.

⚠️ **CORS does not apply and auth middleware must not.** This is fetched by
Google's servers, not a browser tab. If it ends up behind the JWT middleware
it will 401 forever and the failure will look like "Google won't subscribe".

## Which events go in the feed

Events the user would see in the app: theirs, ones they're invited to, and
ones visible to them through publication-scoped visibility. **Reuse
`services::calendar::list_user_events`** rather than writing a second
visibility rule - CLAUDE.md records two separate occasions where a parallel
rule drifted from the real one.

## Getting the iCalendar right

This is where the bugs will be, because a malformed feed fails *silently* -
the calendar app just shows nothing.

- **CRLF line endings** (`\r\n`), everywhere. Not `\n`.
- **Fold lines at 75 octets**, continuation lines starting with a single
  space. Long descriptions are the common case here, and an unfolded long
  line is the single most common reason a feed is rejected.
- **Escape `,` `;` `\` and newlines** in TEXT values. An event titled
  "Drinks, then food" breaks the parse otherwise.
- **`UID` must be stable and globally unique** - use the event's UUID plus a
  domain (`{uuid}@friends-calendar`). An unstable UID makes every refresh
  look like a delete-and-recreate, which on a phone means a notification
  storm.
- **`DTSTAMP` is required**; `SEQUENCE` should increment on edit, which is
  what `updated_at` is for.
- **UTC with a trailing `Z`** (`DTSTART:20260301T190000Z`) - no VTIMEZONE to
  get wrong. The events are stored in UTC already.
- Map fields: `SUMMARY` = title, `DESCRIPTION` = description (append the
  `link` if set), `LOCATION` = location, `URL` = the app's event link.
- `X-WR-CALNAME:Friends Calendar` so it isn't called "Untitled" in the
  subscriber's sidebar.

## Decide first

1. **Does a declined event appear?** Recommend no - it is the one status that
   means "I'm not going", and a declined event in your personal calendar is
   noise. Say which you chose.
2. **How far back?** A feed carrying five years of history is slow to parse
   on a phone. Recommend a rolling window (past 3 months, future 12) and a
   comment saying so.
3. **Do you set `STATUS` / `PARTSTAT`?** Nice, but Google ignores `PARTSTAT`
   for subscribed (as opposed to invited) calendars. Don't spend time on it.

## Tests

- **Unit, pure**: the escaping and folding. Feed it a title with a comma, a
  description with a newline, and a 200-character line, and assert the exact
  output bytes - including CRLF. This is the part that silently breaks.
- **Unit**: a stable `UID` for the same event across two renders.
- **Functional**: `GET /calendar/{token}.ics` returns 200 with
  `Content-Type: text/calendar`, a bad token 404s (**not** 401 - don't
  confirm which tokens exist), and the route works with **no Authorization
  header at all**, which is the whole point.
- **Integration (`#[sqlx::test]`)**: the feed contains an event you're
  invited to and omits one you can't see - the same visibility rule the
  listing uses.
- **Rotation**: minting a new token makes the old URL 404.

## Frontend

A "Subscribe in your calendar app" row on `/settings`: the URL, a copy
button, a regenerate button behind a confirm (it breaks existing
subscriptions), and one line saying **refreshes are on the provider's
schedule and Google's can take hours** - otherwise the first bug report is
"it's not updating".

## Done when

Somebody can paste one URL into Google, Apple or Outlook and see the group's
events next to the rest of their life - and can revoke it without deleting
their account.
