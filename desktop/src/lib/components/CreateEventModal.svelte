<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { dismissable } from '$lib/actions/dismissable';
  import { api } from '$lib/api';
  import { user } from '$lib/stores';
  import { dateUtils } from '$lib/utils/dateUtils';
  import type { EventWithParticipants, FriendInfo, GuildInfo, Visibility } from '$lib/types';

  // Null = create a new event, an event = edit that one. One nullable
  // prop rather than a separate `isEditing` boolean, so the two can't
  // contradict each other.
  export let event: EventWithParticipants | null = null;
  /**
   * A date to open a *new* event on, when creation started from a specific
   * day (long-press / double-click on the month grid).
   *
   * A third nullable input rather than a `mode` flag, matching how `event`
   * already works: null means "no opinion", and it is ignored entirely when
   * editing, where the event's own times win.
   */
  export let initialDate: Date | null = null;

  const dispatch = createEventDispatcher();

  $: isEditing = event !== null;

  let title = '';
  let description = '';
  let startTime = '';
  let endTime = '';
  let location = '';
  // Preselected from the user's saved preference rather than sent as
  // `undefined` and resolved server-side, so the form shows what will
  // actually happen. The backend applies the same default when the field
  // is omitted (services::calendar::create_event), so the two agree.
  let visibility: Visibility = $user?.default_visibility ?? 'friends';

  // Minutes before the start time at which everyone going gets reminded.
  // Several can be picked - an event can nudge a week out, a day out and an
  // hour out - so "no reminder" is simply nothing selected rather than a
  // sentinel value (migration 012 stores one row per reminder).
  const REMINDER_CHOICES: { value: number; label: string }[] = [
    { value: 60, label: '1 hour' },
    { value: 180, label: '3 hours' },
    { value: 1440, label: '1 day' },
    { value: 2880, label: '2 days' },
    { value: 10080, label: '1 week' }
  ];
  let reminderLeads: number[] = [60];

  // Which servers to announce in. Nothing selected by default: publishing is
  // opt-in, so an event stays with its guest list unless you say otherwise.
  let servers: GuildInfo[] = [];
  let selectedGuildIds: string[] = [];

  function toggleServer(id: string) {
    selectedGuildIds = selectedGuildIds.includes(id)
      ? selectedGuildIds.filter((g) => g !== id)
      : [...selectedGuildIds, id];
  }

  // Picking no servers while asking for a non-private visibility is a
  // contradiction worth surfacing: reach is scoped to where an event was
  // published, so "public" plus "nowhere" means nobody outside the guest
  // list sees it.
  $: publishedNowhereButShared =
    !isEditing && selectedGuildIds.length === 0 && visibility !== 'private';

  function toggleReminder(value: number) {
    reminderLeads = reminderLeads.includes(value)
      ? reminderLeads.filter((v) => v !== value)
      : [...reminderLeads, value].sort((a, b) => a - b);
  }
  let loading = false;

  // Two-step wizard, mobile only. `step` is pure UI state - the submitted
  // payload is identical either way, so the desktop single-scroll form and
  // the mobile wizard can't drift into sending different things.
  //
  // Both steps' markup is always in the DOM; the `md:` classes decide what's
  // visible. That keeps one set of bindings (so "Back" can't lose your
  // input) and means desktop genuinely ignores `step` rather than being
  // driven by it.
  //
  // Editing stays single-scroll even on mobile: step 2 is the invite picker
  // and the Discord preview, and PUT /api/events/:id manages neither.
  let step: 1 | 2 = 1;
  $: wizard = !isEditing;
  $: showStep1 = !wizard || step === 1;
  $: showStep2 = !wizard || step === 2;

  let stepError = '';

  // Same checks the single-step submit makes, so step 2 is unreachable with
  // step-1 data that would be rejected on submit anyway.
  function validateStep1(): string {
    if (!title || !startTime || !endTime) return 'Please fill in all required fields';
    if (new Date(endTime) <= new Date(startTime)) return 'End time must be after start time';
    return '';
  }

  function goToStep2() {
    stepError = validateStep1();
    if (stepError) return;
    step = 2;
    loadPreview();
  }

  // Rendered by the backend from the same formatter the real announcement
  // uses, so it can't drift. Shown as raw message source - Discord markdown
  // and <t:...> timestamps included - because that's literally what gets
  // posted; Discord is what renders it.
  let preview = '';
  let previewError = '';

  async function loadPreview() {
    try {
      previewError = '';
      preview = await api.previewAnnouncement({
        title,
        description: description || undefined,
        start_time: new Date(startTime).toISOString(),
        end_time: new Date(endTime).toISOString(),
        location: location || undefined,
        price: price || undefined,
        link: link || undefined
      });
    } catch (err) {
      preview = '';
      previewError = err instanceof Error ? err.message : 'Could not load the preview';
    }
  }
  let error = '';
  let price = '';
  let link = '';

  // Prefill from the event being edited. Keyed on `event?.id` rather than
  // `event` so this doesn't re-run (and clobber half-typed edits) if the
  // parent hands down a new object for the same event after a refresh.
  /**
   * Same shape as the edit prefill below, and for the same reason: keyed on
   * the value so re-rendering the parent can't clobber what's been typed.
   * Only ever applies when creating - an edit carries its own times.
   */
  let prefilledDate: number | null = null;
  $: if (!event && initialDate && initialDate.getTime() !== prefilledDate) {
    prefilledDate = initialDate.getTime();
    startTime = dateUtils.toDatetimeLocalValue(initialDate.toISOString());
    endTime = dateUtils.toDatetimeLocalValue(
      new Date(initialDate.getTime() + 2 * 60 * 60 * 1000).toISOString()
    );
  }

  let prefilledId: string | null = null;
  $: if (event && event.id !== prefilledId) {
    prefilledId = event.id;
    title = event.title;
    description = event.description ?? '';
    startTime = dateUtils.toDatetimeLocalValue(event.start_time);
    endTime = dateUtils.toDatetimeLocalValue(event.end_time);
    location = event.location ?? '';
    visibility = event.visibility;
    reminderLeads = [...event.reminder_leads];
    price = event.price ?? '';
    link = event.link ?? '';
  }

  // Invite picker: participant_ids has always been accepted by the
  // backend (services::calendar::create_event), but nothing here ever
  // sent it - meaning nobody but the creator was ever actually invited,
  // regardless of `visibility`. See mockup-friends-directory skill.
  let friends: FriendInfo[] = [];
  let friendsError = '';
  let selectedFriendIds = new Set<string>();

  onMount(async () => {
    try {
      friends = await api.getFriends();
    } catch (err) {
      friendsError = err instanceof Error ? err.message : 'Failed to load friends';
    }

    try {
      servers = (await api.getServers()).guilds;
    } catch {
      // Best-effort, like the friend list: not being able to offer servers
      // shouldn't block creating an event that simply isn't announced.
      servers = [];
    }
  });

  function toggleFriend(userId: string) {
    const next = new Set(selectedFriendIds);
    if (next.has(userId)) {
      next.delete(userId);
    } else {
      next.add(userId);
    }
    selectedFriendIds = next;
  }

  async function handleSubmit() {
    if (!title || !startTime || !endTime) {
      error = 'Please fill in all required fields';
      return;
    }

    try {
      loading = true;
      error = '';

      if (event) {
        // No participant_ids: PUT /api/events/:id doesn't manage the
        // guest list (that's POST/DELETE .../participants), so sending it
        // would be silently ignored. Editing invitees is a separate flow.
        await api.updateEvent(event.id, {
          title,
          description: description || undefined,
          start_time: new Date(startTime).toISOString(),
          end_time: new Date(endTime).toISOString(),
          location: location || undefined,
          visibility,
          price: price || undefined,
          link: link || undefined,
          reminder_leads: reminderLeads
        });
      } else {
        await api.createEvent({
          title,
          description: description || undefined,
          start_time: new Date(startTime).toISOString(),
          end_time: new Date(endTime).toISOString(),
          location: location || undefined,
          visibility,
          price: price || undefined,
          link: link || undefined,
          reminder_leads: reminderLeads,
          guild_ids: selectedGuildIds,
          participant_ids: selectedFriendIds.size > 0 ? Array.from(selectedFriendIds) : undefined
        });
      }

      // One event for both paths - the parent just reloads either way.
      dispatch('saved');
    } catch (err) {
      error = err instanceof Error ? err.message : `Failed to ${event ? 'update' : 'create'} event`;
    } finally {
      loading = false;
    }
  }

  function handleClose() {
    dispatch('close');
  }
</script>

<!-- See AdoptEventModal for why this scrolls and aligns to the top rather
     than centring: a centred, unscrollable overlay puts the close controls
     off screen on a phone, where `vh` excludes the URL bar. -->
<div
  class="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black bg-opacity-50 p-4 anim-scrim"
  use:dismissable={handleClose}
  role="presentation"
>
  <div
    class="bg-surface my-auto w-full max-w-2xl rounded-2xl shadow-2xl anim-pop"
    role="dialog"
    aria-modal="true"
  >
    <div class="p-6">
      <div
        class="bg-surface sticky top-0 z-10 -mx-6 -mt-6 mb-6 flex items-center justify-between rounded-t-2xl px-6 pt-6"
      >
        <h2 class="text-2xl font-bold text-gray-900">
          {isEditing ? 'Edit event' : 'Create New Event'}
          {#if wizard}
            <span class="md:hidden font-mono text-xs text-muted align-middle ml-2">{step} / 2</span>
          {/if}
        </h2>
        <button
          type="button"
          on:click={handleClose}
          class="-mr-2 flex min-h-[44px] min-w-[44px] shrink-0 items-center justify-center rounded-lg text-2xl leading-none text-gray-400 hover:bg-gray-100 hover:text-gray-600"
          aria-label="Close">×</button
        >
      </div>

      {#if error}
        <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded mb-4">
          {error}
        </div>
      {/if}

      <form on:submit|preventDefault={handleSubmit} class="space-y-4">
        <!-- Step 1. `hidden md:block` rather than an {#if}: desktop must
             render everything regardless of `step`, and keeping both steps
             mounted is what lets "Back" return to filled-in fields. -->
        <div class="space-y-4 {showStep1 ? '' : 'hidden md:block'}">
          <div>
            <label for="title" class="block text-sm font-medium text-gray-700 mb-1">
              Event Title *
            </label>
            <input
              id="title"
              type="text"
              bind:value={title}
              required
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
              placeholder="Team Meeting"
            />
          </div>

          <div>
            <label for="description" class="block text-sm font-medium text-gray-700 mb-1">
              Description
            </label>
            <textarea
              id="description"
              bind:value={description}
              rows="3"
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple
            focus:border-transparent"
              placeholder="What's this event about?"
            >
            </textarea>
          </div>

          <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label for="startTime" class="block text-sm font-medium text-gray-700 mb-1">
                Start Time *
              </label>
              <input
                id="startTime"
                type="datetime-local"
                bind:value={startTime}
                required
                class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
              />
            </div>

            <div>
              <label for="endTime" class="block text-sm font-medium text-gray-700 mb-1">
                End Time *
              </label>
              <input
                id="endTime"
                type="datetime-local"
                bind:value={endTime}
                required
                class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
              />
            </div>
          </div>

          <div>
            <label for="location" class="block text-sm font-medium text-gray-700 mb-1">
              Location
            </label>
            <input
              id="location"
              type="text"
              bind:value={location}
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
              placeholder="Conference Room A"
            />
          </div>
          <div>
            <label for="price" class="block text-sm font-medium text-gray-700 mb-1"> Prix </label>
            <input
              id="price"
              type="text"
              bind:value={price}
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
              placeholder="20€ ou Gratuit"
            />
          </div>

          <div>
            <label for="link" class="block text-sm font-medium text-gray-700 mb-1"> Lien </label>
            <input
              id="link"
              type="url"
              bind:value={link}
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
              placeholder="https://example.com"
            />
          </div>

          <div>
            <label for="visibility" class="block text-sm font-medium text-gray-700 mb-1">
              Visibility
            </label>
            <select
              id="visibility"
              bind:value={visibility}
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            >
              <option value="private">Private (only you)</option>
              <option value="friends">Friends</option>
              <option value="public">Public</option>
            </select>
          </div>

          <fieldset class="border-0 p-0 m-0">
            <legend class="block text-sm font-medium text-gray-700 mb-1">
              Remind everyone going
            </legend>
            <!-- Checkboxes, not a dropdown: several reminders per event are
               allowed, and "none" is nothing ticked rather than a special
               option. -->
            <div class="flex flex-wrap gap-2">
              {#each REMINDER_CHOICES as choice (choice.value)}
                {@const selected = reminderLeads.includes(choice.value)}
                <button
                  type="button"
                  on:click={() => toggleReminder(choice.value)}
                  aria-pressed={selected}
                  class="rounded-full px-3 py-1.5 text-sm font-medium border transition {selected
                    ? 'bg-primary text-white border-primary'
                    : 'bg-gray-100 text-gray-700 border-transparent hover:bg-gray-200'}"
                >
                  {choice.label} before
                </button>
              {/each}
            </div>
            <p class="text-xs text-gray-500 mt-1">
              {reminderLeads.length === 0
                ? 'No reminders for this event.'
                : "Sent in the app, and in this event's Discord thread."}
            </p>
          </fieldset>
        </div>
        <!-- Step 2: who's coming, and what lands in Discord. -->
        <div class="space-y-4 {showStep2 ? '' : 'hidden md:block'}">
          <!-- Invite picker is create-only: PUT /api/events/:id doesn't touch
             the guest list (participants have their own endpoints), so
             showing it while editing would imply changes that never save. -->
          <div class:hidden={isEditing}>
            <span class="block text-sm font-medium text-gray-700 mb-1">Invite</span>
            {#if friendsError}
              <p class="text-sm text-red-600" role="alert">{friendsError}</p>
            {:else if friends.length === 0}
              <p class="text-sm text-gray-500">No friends synced yet.</p>
            {:else}
              <div class="flex flex-wrap gap-2">
                {#each friends as friend (friend.user_id)}
                  {@const selected = selectedFriendIds.has(friend.user_id)}
                  <button
                    type="button"
                    on:click={() => toggleFriend(friend.user_id)}
                    aria-pressed={selected}
                    class="inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-sm font-medium border transition {selected
                      ? 'bg-primary text-white border-primary'
                      : 'bg-gray-100 text-gray-700 border-transparent hover:bg-gray-200'}"
                  >
                    {#if friend.avatar_url}
                      <img src={friend.avatar_url} alt="" class="w-5 h-5 rounded-full" />
                    {/if}
                    {friend.username}
                  </button>
                {/each}
              </div>
            {/if}
          </div>

          {#if !isEditing}
            <fieldset class="border-0 p-0 m-0">
              <legend class="block text-sm font-medium text-gray-700 mb-1">Announce in</legend>
              {#if servers.length === 0}
                <p class="text-sm text-gray-500">
                  The bot isn't in any server yet, so this event won't be announced.
                </p>
              {:else}
                <div class="flex flex-wrap gap-2">
                  {#each servers as server (server.id)}
                    {@const selected = selectedGuildIds.includes(server.id)}
                    <button
                      type="button"
                      on:click={() => toggleServer(server.id)}
                      aria-pressed={selected}
                      class="inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-sm font-medium border transition {selected
                        ? 'bg-primary text-white border-primary'
                        : 'bg-gray-100 text-gray-700 border-transparent hover:bg-gray-200'}"
                    >
                      {#if server.icon_url}
                        <img src={server.icon_url} alt="" class="w-5 h-5 rounded" />
                      {/if}
                      {server.name ?? server.discord_guild_id}
                    </button>
                  {/each}
                </div>
                {#if publishedNowhereButShared}
                  <p class="text-xs text-gray-500 mt-1">
                    Not announcing anywhere — only people you invite will see this, even though it's
                    set to {visibility}.
                  </p>
                {/if}
              {/if}
            </fieldset>

            <div>
              <span class="block text-sm font-medium text-gray-700 mb-1">Discord preview</span>
              {#if previewError}
                <p class="text-sm text-red-600" role="alert">{previewError}</p>
              {:else if preview}
                <!-- The raw message source, not a rendering of it: this is
                   exactly what gets posted, and Discord is what turns the
                   markdown and <t:…> timestamps into formatted text. Faking
                   that rendering here would misrepresent it. -->
                <pre
                  class="bg-gray-50 border border-line rounded-lg p-3 text-xs whitespace-pre-wrap font-mono text-body overflow-x-auto">{preview}</pre>
              {:else}
                <p class="text-sm text-gray-500">
                  Fill in the details above to see what the bot will post.
                </p>
              {/if}
            </div>
          {/if}
        </div>

        {#if wizard}
          <!-- Mobile-only wizard controls; the desktop footer below stays
               the single Cancel/Create pair it has always been. -->
          <div class="flex md:hidden gap-3 pt-4">
            {#if step === 1}
              <button
                type="button"
                on:click={handleClose}
                class="flex-1 px-4 py-2 border border-gray-300 rounded-lg text-gray-700 font-medium"
              >
                Cancel
              </button>
              <button
                type="button"
                on:click={goToStep2}
                class="flex-1 px-4 py-2 bg-primary text-white rounded-lg font-medium"
              >
                Next · invite friends
              </button>
            {:else}
              <button
                type="button"
                on:click={() => (step = 1)}
                class="flex-1 px-4 py-2 border border-gray-300 rounded-lg text-gray-700 font-medium"
              >
                ‹ Back
              </button>
              <button
                type="submit"
                disabled={loading}
                class="flex-1 px-4 py-2 bg-primary text-white rounded-lg font-medium disabled:opacity-50"
              >
                {loading ? 'Creating...' : 'Create & post'}
              </button>
            {/if}
          </div>
          {#if stepError}
            <p class="md:hidden text-sm text-red-600 m-0" role="alert">{stepError}</p>
          {/if}
        {/if}

        <div class="{wizard ? 'hidden md:flex' : 'flex'} gap-3 pt-4">
          <button
            type="button"
            on:click={handleClose}
            class="flex-1 px-4 py-2 border border-gray-300 rounded-lg text-gray-700 hover:bg-gray-50 font-medium transition"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={loading}
            class="flex-1 px-4 py-2 border-2 border-primary text-primary rounded-lg hover:bg-primary hover:text-white font-medium transition disabled:opacity-50"
          >
            {#if loading}
              {isEditing ? 'Saving…' : 'Creating...'}
            {:else}
              {isEditing ? 'Save changes' : 'Create Event'}
            {/if}
          </button>
        </div>
      </form>
    </div>
  </div>
</div>
