<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { dismissable } from '$lib/actions/dismissable';
  import { api } from '$lib/api';
  import { user } from '$lib/stores';
  import { dateUtils } from '$lib/utils/dateUtils';
  import { parseAnnouncement, DEFAULT_DURATION_MINUTES } from '$lib/utils/announcementParse';
  import type { AnnouncementPostInfo, Visibility } from '$lib/types';

  /**
   * Adds an announcement that already exists in Discord to the calendar.
   *
   * ## Why this isn't a third mode on CreateEventModal
   *
   * That component is already create-or-edit plus a mobile wizard, and the
   * three things it does that matter most here are all *wrong* for adoption:
   * the server picker (the server is fixed - it's wherever the message
   * lives), the Discord preview (the message is already posted; previewing
   * one that will never be sent would be a lie), and the invite picker
   * (`POST /api/announcements/:id/adopt` doesn't invite - the ✅ already on
   * the message do that). What's left is a short confirm-these-fields form
   * over a guess, which is a different job from composing an event.
   *
   * Reminders aren't offered: the backend applies its usual single default
   * when `reminder_leads` is omitted, and they're editable afterwards from
   * the calendar like any other event's.
   */
  export let post: AnnouncementPostInfo;

  const dispatch = createEventDispatcher<{ close: void; adopted: { rsvps: number } }>();

  // Parsed once, at construction. Re-deriving reactively would overwrite
  // whatever the user has corrected the moment the parent re-renders.
  const guess = parseAnnouncement(post);

  let title = guess.title ?? '';
  let location = guess.location ?? '';
  let price = guess.price ?? '';
  let link = guess.link ?? '';
  let visibility: Visibility = $user?.default_visibility ?? 'friends';

  // An announcement carries a start and never an end, so the end is offered
  // as a plausible default rather than as an empty required field.
  const start = guess.startTime ?? null;
  let startTime = start ? dateUtils.toDatetimeLocalValue(start.toISOString()) : '';
  let endTime = start
    ? dateUtils.toDatetimeLocalValue(
        new Date(start.getTime() + DEFAULT_DURATION_MINUTES * 60_000).toISOString()
      )
    : '';

  let loading = false;
  let error = '';

  // What the parser could not find, so the fields it left empty read as
  // "we couldn't tell" rather than as the user having cleared them.
  $: missing = [guess.title ? null : 'title', guess.startTime ? null : 'date'].filter(
    Boolean
  ) as string[];

  /**
   * Names what's missing rather than saying "required fields": the whole
   * reason a field is blank here is that the parser couldn't find it in the
   * post, so the reader has no idea which one it means.
   */
  function validate(): string {
    const missingNow = [
      title ? null : 'a title',
      startTime ? null : 'a start time',
      endTime ? null : 'an end time'
    ].filter(Boolean);

    if (missingNow.length > 0) return `This event still needs ${missingNow.join(', ')}.`;
    if (new Date(endTime) <= new Date(startTime)) return 'End time must be after start time';
    return '';
  }

  async function handleSubmit() {
    error = validate();
    if (error) return;

    try {
      loading = true;
      error = '';
      const result = await api.adoptAnnouncement(post.id, {
        title,
        start_time: new Date(startTime).toISOString(),
        end_time: new Date(endTime).toISOString(),
        location: location || undefined,
        visibility,
        price: price || undefined,
        link: link || undefined
      });
      dispatch('adopted', { rsvps: result.rsvps_recorded });
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not add this announcement';
    } finally {
      loading = false;
    }
  }
</script>

<!-- overflow-y-auto + items-start so the dialog can never be taller than
     the reachable area: on a phone `90vh` is measured against the viewport
     *without* the URL bar, so a centred, unscrollable overlay pushes the
     top and bottom of the dialog - where every close control lives - off
     screen with no way to bring them back. -->
<div
  class="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black bg-opacity-50 p-4 anim-scrim"
  use:dismissable={() => dispatch('close')}
  role="presentation"
>
  <div
    class="bg-surface my-auto w-full max-w-lg rounded-2xl shadow-2xl anim-pop"
    role="dialog"
    aria-modal="true"
    aria-label="Add to calendar"
  >
    <div class="p-6">
      <!-- sticky: the close control has to stay reachable however far down
           the form you have scrolled. -->
      <div
        class="bg-surface sticky top-0 z-10 -mx-6 -mt-6 mb-1 flex items-start justify-between gap-3 rounded-t-2xl px-6 pt-6"
      >
        <h2 class="text-2xl font-bold text-gray-900 m-0">Add to calendar</h2>
        <button
          type="button"
          on:click={() => dispatch('close')}
          class="-mr-2 -mt-1 flex min-h-[44px] min-w-[44px] shrink-0 items-center justify-center rounded-lg text-2xl leading-none text-gray-400 hover:bg-gray-100 hover:text-gray-600"
          aria-label="Close">×</button
        >
      </div>
      <p class="text-sm text-gray-500 mt-0 mb-4">
        {post.author_username}'s announcement stays where it is — this links it to an event so the
        ✅ already on it count as RSVPs.
      </p>

      {#if missing.length > 0}
        <p class="text-sm text-gray-500 bg-gray-100 rounded-lg px-3 py-2 mb-4">
          We couldn't read the {missing.join(' or ')} out of the post — please fill {missing.length >
          1
            ? 'them'
            : 'it'} in.
        </p>
      {/if}

      {#if error}
        <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded mb-4">
          {error}
        </div>
      {/if}

      <!-- novalidate, deliberately. The fields are `required` and the date
           often can't be parsed out of a post, so the browser was refusing
           the submit before `on:submit` ever ran: no request, no error, no
           visible reason - the modal just sat there. Constraint validation
           reports through a native tooltip that is easy to miss inside a
           scrolling container, and it bypasses the error box entirely. This
           form validates itself, in one place, visibly. -->
      <form on:submit|preventDefault={handleSubmit} novalidate class="space-y-4">
        <div>
          <label for="adopt-title" class="block text-sm font-medium text-gray-700 mb-1">
            Title *
          </label>
          <input
            id="adopt-title"
            type="text"
            bind:value={title}
            required
            class="w-full px-3 py-2 border border-gray-300 rounded-lg bg-surface text-gray-900 focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
          />
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div>
            <label for="adopt-start" class="block text-sm font-medium text-gray-700 mb-1">
              Starts *
            </label>
            <input
              id="adopt-start"
              type="datetime-local"
              bind:value={startTime}
              required
              class="w-full px-3 py-2 border border-gray-300 rounded-lg bg-surface text-gray-900 focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            />
          </div>
          <div>
            <label for="adopt-end" class="block text-sm font-medium text-gray-700 mb-1">
              Ends *
            </label>
            <input
              id="adopt-end"
              type="datetime-local"
              bind:value={endTime}
              required
              class="w-full px-3 py-2 border border-gray-300 rounded-lg bg-surface text-gray-900 focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            />
          </div>
        </div>

        <div>
          <label for="adopt-location" class="block text-sm font-medium text-gray-700 mb-1">
            Location
          </label>
          <input
            id="adopt-location"
            type="text"
            bind:value={location}
            class="w-full px-3 py-2 border border-gray-300 rounded-lg bg-surface text-gray-900 focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
          />
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div>
            <label for="adopt-price" class="block text-sm font-medium text-gray-700 mb-1">
              Price
            </label>
            <input
              id="adopt-price"
              type="text"
              bind:value={price}
              class="w-full px-3 py-2 border border-gray-300 rounded-lg bg-surface text-gray-900 focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            />
          </div>
          <div>
            <label for="adopt-visibility" class="block text-sm font-medium text-gray-700 mb-1">
              Visible to
            </label>
            <select
              id="adopt-visibility"
              bind:value={visibility}
              class="w-full px-3 py-2 border border-gray-300 rounded-lg bg-surface text-gray-900 focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            >
              <option value="friends">Friends</option>
              <option value="public">Everyone in the server</option>
              <option value="private">Only people I invite</option>
            </select>
          </div>
        </div>

        <div>
          <label for="adopt-link" class="block text-sm font-medium text-gray-700 mb-1">Link</label>
          <input
            id="adopt-link"
            type="url"
            bind:value={link}
            class="w-full px-3 py-2 border border-gray-300 rounded-lg bg-surface text-gray-900 focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
          />
        </div>

        <div class="flex gap-3 pt-2">
          <button
            type="button"
            on:click={() => dispatch('close')}
            class="flex-1 px-4 py-2 border border-gray-300 rounded-lg text-gray-700 hover:bg-gray-100"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={loading}
            class="flex-1 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-active disabled:opacity-50"
          >
            {loading ? 'Adding…' : 'Add to calendar'}
          </button>
        </div>
      </form>
    </div>
  </div>
</div>
