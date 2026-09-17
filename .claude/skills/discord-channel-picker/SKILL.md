---
name: discord-channel-picker
description: Replace the raw "Channel ID" text field on /server with a picker listing the server's actual channels, fetched through the bot token. Not yet executed as of 2026-09-17. Use when asked about the channel setting, pasting channel IDs, /server configuration, or why announcements go to the wrong place.
---

# Pick a channel, don't type a snowflake

`/server` asks the user to paste a **Discord snowflake** into a text input
labelled "Channel ID". To get one you have to enable Developer Mode in
Discord, right-click a channel and choose "Copy Channel ID". Nothing in the
app says that, nothing validates what you paste, and a wrong-but-plausible
17-19 digit number fails silently: `resolve_announcement_channel_id` hands
it to Discord, the POST 404s, and `services::calendar::create_event`
tolerates the failure with a warning because announcing is best-effort.

So the single most consequential setting in the app is a free-text field
where a typo produces no error anywhere the user can see.

Both mockups show a picker. `Friends Calendar Mobile.dc.html`'s screen 11
caption: *"Channel pickers open as action sheets; permissions are read-only
chips."*

## Do these first

Nothing. This is self-contained and depends on no other skill.

## The endpoint

`GET /guilds/{guild_id}/channels` on the bot token. Returns every channel
in the guild; the fields that matter are `id`, `name`, `type`, `position`
and `parent_id`.

- **Filter to `type == 0`** (`GUILD_TEXT`). Announcement channels are type
  5 and are also postable, so accept `0` and `5` and nothing else - voice,
  category, forum and stage channels cannot take a message and must not be
  offered.
- **Categories are type 4.** Don't list them as choices, but *do* read
  them: grouping by `parent_id` and sorting by `position` is what makes a
  50-channel server navigable. A flat alphabetical list is not how anyone
  thinks about their server.
- ⚠️ **The bot only sees channels it has `VIEW_CHANNEL` on.** A channel the
  user expects will simply be missing rather than erroring. Say so in the
  UI - "only channels the bot can see are listed" - or the first support
  question is "why isn't #general in the list".

## Where the code goes

- `services::discord_feed` already owns `get_json` and `url_encode` as
  `pub(crate)` and is where every Discord REST call lives. Add
  `list_text_channels(base_url, http, bot_token, guild_id)` there. **Do not
  add a new module and do not reach for serenity** - see
  `.claude/skills/add-tests/SKILL.md` for why `base_url` as a parameter is
  non-negotiable (it is the only reason these are testable).
- `handlers::guilds` already exists and already talks about guilds. Add
  `GET /api/guilds/:id/channels` there rather than hanging it off
  `handlers::discord_config`, which is about the *stored* config.
- Return a shape the client can render directly:
  `{ id, name, category: Option<String> }`, already filtered and sorted.
  Sorting on the server means one implementation, not one per caller.

## Decide first

1. **Does the picker replace the text field, or sit beside it?** Recommend
   replace, with the raw id shown as mono helper text under the selection
   so anyone debugging can still see what's stored. Keeping both is how you
   end up with two sources of truth for one setting.
2. **What happens when the bot has no guild, or the fetch fails?** Fall
   back to the text input rather than blocking the page - a deployment
   whose bot is offline must still be configurable. This is the same
   degrade-don't-crash rule `config.rs` already follows for every Discord
   env var.
3. **Mobile presentation.** The mockup says action sheet. The app already
   has a sheet pattern (`EventPeekPanel` below `lg:`) and a dismissal
   action (`lib/actions/dismissable.ts`) - reuse both. A `<select>` is the
   lazy alternative and has a specific cost here: happy-dom cannot match
   `<select>` options by value, which is documented in CLAUDE.md as the
   reason the reminder picker uses buttons.

## Tests

- **Integration, wiremock**: `list_text_channels` filters out types 2/4/13,
  keeps 0 and 5, groups by category, sorts by position. Feed it a payload
  with a category, two text channels in it, one voice channel and one
  top-level text channel - and assert the *order*, not just membership.
- **Functional**: `GET /api/guilds/:id/channels` requires auth and 404s for
  a guild the app doesn't have.
- ⚠️ **One test must assert the picker writes the same column the old field
  did.** The whole failure mode this skill fixes is silent misconfiguration;
  a picker that saves to the wrong place would be worse than the text box.
  Send the payload the component sends, through the real router - see the
  enum-casing note in CLAUDE.md for why anything less has missed this exact
  class of bug before.
- **Component**: picking a channel and saving calls `api.updateBotConfig`
  with that channel's id; a failed channel fetch still renders the fallback
  input.

## Done when

Someone who has never heard of Developer Mode can point the bot at a
channel, and a deployment with an offline bot can still be configured.
