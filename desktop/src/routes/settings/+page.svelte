<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import { user as userStore } from '$lib/stores';
  import Frame from '$lib/components/templates/Frame.svelte';
  import { theme, setTheme, type Theme } from '$lib/theme';
  import Icon from '$lib/components/atoms/Icon.svelte';
  import type { ExternalCalendar, User, Visibility } from '$lib/types';

  let profile: User | null = null;
  let loading = true;
  let loadError = '';

  // Each option carries the line that says what it means - the mockup shows
  // these as three cards, not as options in a dropdown, precisely so it can.
  const VISIBILITY_CHOICES: { value: Visibility; label: string; hint: string }[] = [
    { value: 'friends', label: 'Friends', hint: 'Everyone you synced' },
    { value: 'private', label: 'Private', hint: 'Only people you invite' },
    { value: 'public', label: 'Public', hint: 'Anyone in a server it is posted to' }
  ];

  // Same derivation Frame uses for the header avatar, including the
  // default-avatar fallback for accounts that never set one.
  $: avatarUrl = profile?.avatar
    ? `https://cdn.discordapp.com/avatars/${profile.discord_id}/${profile.avatar}.png`
    : `https://cdn.discordapp.com/embed/avatars/${defaultAvatarIndex(profile?.discord_id)}.png`;

  function defaultAvatarIndex(discordId: string | undefined): number {
    if (!discordId) return 0;
    try {
      return Number((BigInt(discordId) >> 22n) % 6n);
    } catch {
      return 0;
    }
  }

  let displayName = '';
  let timezone = 'UTC';
  let defaultVisibility: Visibility = 'friends';
  let notifyEventInvites = true;
  let notifyRsvpChanges = true;
  let notifyAnnouncements = false;
  let notifyWeeklyDigest = true;
  let notifyEventReminders = true;
  let notifyDiscordDm = true;

  // The .ics subscription link. Loaded lazily - asking for it mints a
  // credential, so nobody who never opens this row gets one.
  // Calendars feeding availability. Loaded on mount - unlike the feed link,
  // asking for this list mints nothing.
  let externalCalendars: ExternalCalendar[] = [];
  let connectUrl = '';
  let connectLabel = '';
  let connecting = false;
  let connectError = '';

  async function loadExternalCalendars() {
    try {
      externalCalendars = await api.getExternalCalendars();
    } catch {
      // Non-critical: the rest of settings still works.
    }
  }

  async function connectCalendar() {
    if (!connectUrl.trim()) return;
    try {
      connecting = true;
      connectError = '';
      externalCalendars = await api.connectExternalCalendar(
        connectUrl.trim(),
        connectLabel.trim() || undefined
      );
      connectUrl = '';
      connectLabel = '';
    } catch (err) {
      connectError = err instanceof Error ? err.message : 'Could not connect that calendar';
    } finally {
      connecting = false;
    }
  }

  async function disconnectCalendar(id: string) {
    try {
      await api.disconnectExternalCalendar(id);
      externalCalendars = externalCalendars.filter((c) => c.id !== id);
    } catch (err) {
      connectError = err instanceof Error ? err.message : 'Could not disconnect';
    }
  }

  let feedUrl = '';
  let feedError = '';
  let copied = false;
  let confirmingRotate = false;

  async function loadFeedUrl() {
    try {
      feedError = '';
      feedUrl = await api.getCalendarFeedLink();
    } catch (err) {
      feedError = err instanceof Error ? err.message : 'Could not get your calendar link';
    }
  }

  async function rotateFeedUrl() {
    try {
      feedError = '';
      feedUrl = await api.rotateCalendarFeedLink();
      confirmingRotate = false;
    } catch (err) {
      feedError = err instanceof Error ? err.message : 'Could not regenerate the link';
    }
  }

  async function copyFeedUrl() {
    try {
      await navigator.clipboard.writeText(feedUrl);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch {
      // Clipboard access is refused in some browsers and contexts; the URL
      // is on screen and selectable either way.
    }
  }

  let saving = false;
  let saveError = '';
  let saved = false;

  let confirmUsername = '';
  let deleting = false;
  let deleteError = '';

  async function load() {
    try {
      loading = true;
      loadError = '';
      profile = await api.getCurrentUser();
      displayName = profile.display_name ?? '';
      timezone = profile.timezone;
      defaultVisibility = profile.default_visibility;
      notifyEventInvites = profile.notify_event_invites;
      notifyRsvpChanges = profile.notify_rsvp_changes;
      notifyAnnouncements = profile.notify_announcements;
      notifyWeeklyDigest = profile.notify_weekly_digest;
      notifyEventReminders = profile.notify_event_reminders;
      notifyDiscordDm = profile.notify_discord_dm;
    } catch (err) {
      loadError = err instanceof Error ? err.message : 'Failed to load profile';
    } finally {
      loading = false;
    }
  }

  async function handleSave() {
    try {
      saving = true;
      saveError = '';
      saved = false;
      const updated = await api.updateProfile({
        display_name: displayName.trim() === '' ? undefined : displayName.trim(),
        timezone,
        default_visibility: defaultVisibility,
        notify_event_invites: notifyEventInvites,
        notify_rsvp_changes: notifyRsvpChanges,
        notify_announcements: notifyAnnouncements,
        notify_weekly_digest: notifyWeeklyDigest,
        notify_event_reminders: notifyEventReminders,
        notify_discord_dm: notifyDiscordDm
      });
      profile = updated;
      userStore.set(updated);
      saved = true;
    } catch (err) {
      saveError = err instanceof Error ? err.message : 'Failed to save profile';
    } finally {
      saving = false;
    }
  }

  async function handleDelete() {
    if (!profile) return;
    try {
      deleting = true;
      deleteError = '';
      await api.deleteAccount(confirmUsername);
      api.clearToken();
      window.location.href = '/';
    } catch (err) {
      deleteError = err instanceof Error ? err.message : 'Failed to delete account';
    } finally {
      deleting = false;
    }
  }

  onMount(() => {
    load();
    loadExternalCalendars();
  });
</script>

<svelte:head>
  <title>Settings - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="space-y-4 anim-fade-up">
    <!-- md:hidden: the rail lists Settings from md: up, so this is only a way
         back on mobile, where the rail isn't rendered. -->
    <button
      class="inline-flex items-center gap-1 text-sm text-discord-blurple hover:underline md:hidden"
      on:click={() => goto('/')}
    >
      <Icon name="back" size={14} /> Back to calendar
    </button>

    <h1 class="m-0 text-[28px] font-bold tracking-[-0.02em]">Settings</h1>

    {#if loadError}
      <p class="text-sm text-red-600" role="alert">{loadError}</p>
    {:else if loading}
      <p class="text-sm text-gray-500">Loading…</p>
    {:else if profile}
      <!-- Grouped into cards, per the mockup: it separates Profile, default
           visibility, notifications and the destructive action instead of
           running them together as one column of form controls. -->
      <section class="rounded-[14px] border border-line bg-surface p-[18px] space-y-4">
        <h2 class="m-0 text-[15px] font-semibold">Profile</h2>

        <div class="flex items-center gap-3.5">
          <img src={avatarUrl} alt="" class="h-[54px] w-[54px] shrink-0 rounded-full" />
          <div class="min-w-0">
            <div class="truncate text-[15px] font-semibold">{profile.username}</div>
            <div class="truncate font-mono text-[12px] text-muted">synced from Discord</div>
          </div>
        </div>

        <div class="grid gap-4 sm:grid-cols-2">
          <div>
            <label for="display-name" class="mb-1.5 block text-[13px] font-medium text-body"
              >Display name</label
            >
            <input
              id="display-name"
              type="text"
              bind:value={displayName}
              placeholder={profile.username}
              class="w-full rounded-[9px] border border-line bg-surface px-3 py-[9px] text-[13px] outline-none focus:border-primary"
            />
          </div>

          <div>
            <label for="timezone" class="mb-1.5 block text-[13px] font-medium text-body"
              >Timezone</label
            >
            <input
              id="timezone"
              type="text"
              bind:value={timezone}
              placeholder="UTC"
              class="w-full rounded-[9px] border border-line bg-surface px-3 py-[9px] text-[13px] outline-none focus:border-primary"
            />
          </div>
        </div>
      </section>

      <!-- Segmented options rather than a <select>, per the mockup - each
           choice can then carry the one line that says what it actually
           means, which a dropdown has nowhere to put. (It also sidesteps
           happy-dom's inability to match <select> options by value, the trap
           noted in CLAUDE.md.) -->
      <section class="rounded-[14px] border border-line bg-surface p-[18px] space-y-3.5">
        <div>
          <h2 class="m-0 text-[15px] font-semibold">Default event visibility</h2>
          <p class="m-0 mt-1 text-[13px] text-muted">Applied to every event you create.</p>
        </div>
        <div class="grid gap-2.5 sm:grid-cols-3" role="group" aria-label="Default event visibility">
          {#each VISIBILITY_CHOICES as choice (choice.value)}
            <button
              type="button"
              aria-pressed={defaultVisibility === choice.value}
              on:click={() => (defaultVisibility = choice.value)}
              class="rounded-[11px] border p-3 text-left transition-colors {defaultVisibility ===
              choice.value
                ? 'border-primary bg-tint'
                : 'border-line bg-surface hover:bg-subtle'}"
            >
              <span class="block text-[14px] font-semibold">{choice.label}</span>
              <span class="block text-[12px] text-muted">{choice.hint}</span>
            </button>
          {/each}
        </div>
      </section>

      <section class="rounded-[14px] border border-line bg-surface p-[18px] space-y-3.5">
        <!-- Deliberately outside the Save button's scope: this is a
             per-device preference kept in localStorage, not a column on
             `users`. Routing it through PATCH /api/auth/me would mean one
             browser's choice silently changing the theme on someone's
             phone, and would need a migration for no benefit. It applies
             the moment it's clicked. -->
        <div>
          <span class="block text-sm font-medium text-gray-700 mb-1">Appearance</span>
          <div class="flex gap-2" role="group" aria-label="Appearance">
            {#each [{ value: 'system', label: 'System' }, { value: 'light', label: 'Light' }, { value: 'dark', label: 'Dark' }] as option (option.value)}
              <button
                type="button"
                aria-pressed={$theme === option.value}
                on:click={() => setTheme(option.value as Theme)}
                class="px-3 py-2 rounded-lg border text-sm transition-colors {$theme ===
                option.value
                  ? 'border-primary bg-tint text-primary font-semibold'
                  : 'border-gray-300 text-gray-700 hover:bg-gray-100'}"
              >
                {option.label}
              </button>
            {/each}
          </div>
          <p class="mt-1 text-[12px] text-muted">
            Saved on this device only. “System” follows your OS setting as it changes.
          </p>
        </div>
      </section>

      <section class="rounded-[14px] border border-line bg-surface p-[18px] space-y-3">
        <div>
          <h2 class="m-0 text-[15px] font-semibold">Connected calendars</h2>
          <p class="m-0 mt-1 text-[13px] text-muted">
            Bring in your real calendar so "free" means actually free — otherwise a week of work
            meetings still looks wide open.
          </p>
        </div>

        {#if externalCalendars.length > 0}
          <div class="flex flex-col gap-1.5">
            {#each externalCalendars as calendar (calendar.id)}
              <div
                class="flex flex-wrap items-center gap-2 rounded-[11px] border border-line px-3 py-2.5"
              >
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-[14px] font-semibold">
                    {calendar.label ?? 'Calendar'}
                  </span>
                  <!-- The error is shown, not swallowed: a dead connection
                       makes availability look right when it isn't. -->
                  {#if calendar.last_error}
                    <span class="block text-[12px] text-red-600">
                      Not syncing — {calendar.last_error}
                    </span>
                  {:else if calendar.last_synced_at}
                    <span class="block font-mono text-[11px] text-muted">
                      synced {new Date(calendar.last_synced_at).toLocaleString()}
                    </span>
                  {:else}
                    <span class="block font-mono text-[11px] text-muted">not synced yet</span>
                  {/if}
                </span>
                <button
                  type="button"
                  on:click={() => disconnectCalendar(calendar.id)}
                  class="shrink-0 rounded-[9px] border border-line bg-surface px-3 py-[7px] text-[12px] font-semibold hover:bg-subtle"
                >
                  Disconnect
                </button>
              </div>
            {/each}
          </div>
        {/if}

        {#if connectError}
          <p class="m-0 text-[13px] text-red-600" role="alert">{connectError}</p>
        {/if}

        <!-- novalidate, and `type=text` rather than `type=url`. The service
             deliberately accepts `webcal://`, which is what Apple Calendar
             hands you, and the browser's url validator rejects or questions
             it - silently, by refusing to submit with no in-page error. The
             server validates the scheme properly and returns a message
             written for a reader, so that is where the check belongs. -->
        <form on:submit|preventDefault={connectCalendar} novalidate class="flex flex-col gap-2">
          <label for="ics-url" class="text-[13px] font-medium text-body">
            Secret calendar address (.ics)
          </label>
          <input
            id="ics-url"
            type="text"
            inputmode="url"
            bind:value={connectUrl}
            placeholder="https://calendar.google.com/calendar/ical/…/basic.ics"
            class="w-full rounded-[9px] border border-line bg-surface px-3 py-[9px] text-[13px] outline-none focus:border-primary"
          />
          <input
            type="text"
            bind:value={connectLabel}
            placeholder="Label (optional) — e.g. Work"
            aria-label="Calendar label"
            class="w-full rounded-[9px] border border-line bg-surface px-3 py-[9px] text-[13px] outline-none focus:border-primary"
          />
          <!-- Both halves said plainly: what it grants, and what we keep. -->
          <p class="m-0 text-[12px] text-muted">
            In Google, Apple or Outlook this is the private “secret address in iCal format”. ⚠️
            Anyone with it can read that calendar, so treat it like a password. We store
            <strong>only busy times</strong> from it — never event titles, descriptions or attendees.
          </p>
          <button
            type="submit"
            disabled={connecting || connectUrl.trim() === ''}
            class="self-start rounded-[9px] bg-primary px-3.5 py-[9px] text-[13px] font-semibold text-white hover:bg-primary-active disabled:opacity-50"
          >
            {connecting ? 'Connecting…' : 'Connect calendar'}
          </button>
        </form>
      </section>

      <section class="rounded-[14px] border border-line bg-surface p-[18px] space-y-3">
        <div>
          <h2 class="m-0 text-[15px] font-semibold">Subscribe in your calendar app</h2>
          <p class="m-0 mt-1 text-[13px] text-muted">
            A read-only feed of your events for Google Calendar, Apple Calendar or Outlook.
          </p>
        </div>

        {#if feedError}
          <p class="m-0 text-[13px] text-red-600" role="alert">{feedError}</p>
        {/if}

        {#if !feedUrl}
          <button
            type="button"
            on:click={loadFeedUrl}
            class="rounded-[9px] border border-line bg-surface px-3.5 py-[9px] text-[13px] font-semibold hover:bg-subtle"
          >
            Show my calendar link
          </button>
        {:else}
          <!-- ⚠️ The URL is the credential: anyone with it can read your
               events. Said plainly rather than left for people to infer. -->
          <p
            class="m-0 break-all rounded-[9px] border border-line bg-subtle px-3 py-2.5 font-mono text-[12px]"
          >
            {feedUrl}
          </p>
          <div class="flex flex-wrap gap-2">
            <button
              type="button"
              on:click={copyFeedUrl}
              class="rounded-[9px] bg-primary px-3.5 py-[9px] text-[13px] font-semibold text-white hover:bg-primary-active"
            >
              {copied ? 'Copied' : 'Copy link'}
            </button>
            {#if confirmingRotate}
              <button
                type="button"
                on:click={rotateFeedUrl}
                class="rounded-[9px] border border-red-300 bg-surface px-3.5 py-[9px] text-[13px] font-semibold text-red-600 hover:bg-subtle"
              >
                Yes, break existing subscriptions
              </button>
              <button
                type="button"
                on:click={() => (confirmingRotate = false)}
                class="rounded-[9px] border border-line bg-surface px-3.5 py-[9px] text-[13px] font-semibold hover:bg-subtle"
              >
                Cancel
              </button>
            {:else}
              <button
                type="button"
                on:click={() => (confirmingRotate = true)}
                class="rounded-[9px] border border-line bg-surface px-3.5 py-[9px] text-[13px] font-semibold hover:bg-subtle"
              >
                Regenerate
              </button>
            {/if}
          </div>
          <p class="m-0 text-[12px] text-muted">
            Anyone with this link can read your events — treat it like a password, and regenerate it
            if it gets out. ⚠️ Calendar apps refresh on their own schedule; Google's can take
            several hours, so a new event won't appear instantly.
          </p>
        {/if}
      </section>

      <!-- The sidebar that links to /server is hidden below md:, and the
           mobile tab bar folds Server under "Me" without a tab of its own -
           so without this row /server is unreachable on a phone. Styled as a
           push-through (chevron) because that's what it is on mobile; it's
           harmless duplication of the sidebar link on desktop. -->
      <a
        href="/server"
        class="flex items-center gap-3 bg-surface border border-line rounded-xl px-4 py-3 no-underline hover:bg-gray-50"
      >
        <Icon name="bot" size={20} />
        <span class="flex-1 min-w-0">
          <span class="block text-sm font-semibold text-gray-900">Discord server</span>
          <span class="block text-xs text-muted">Linked server, bot channels, weekly digest</span>
        </span>
        <Icon name="chevron-right" size={16} class="text-muted" />
      </a>

      <!-- The mockup's notification rows: label over an explanatory line,
           with the switch on the right and a hairline between rows.

           Each switch is still a real `<input type="checkbox">`, visually
           hidden and drawn by the sibling span. That keeps the label
           association, the keyboard behaviour and `checked` exactly as they
           were - a div with a click handler would have looked the same and
           broken all three. -->
      <section class="rounded-[14px] border border-line bg-surface p-[18px]">
        <h2 class="m-0 mb-1 text-[15px] font-semibold">Notifications</h2>

        <label class="flex items-center gap-4 border-b border-subtle py-3.5">
          <span class="min-w-0 flex-1">
            <span class="block text-[14px]">Event invites</span>
            <span class="block text-[12px] text-muted">When a friend invites you to something</span>
          </span>
          <input type="checkbox" bind:checked={notifyEventInvites} class="peer sr-only" />
          <span
            class="relative h-[22px] w-[38px] shrink-0 rounded-full bg-line transition-colors peer-checked:bg-primary peer-focus-visible:ring-2 peer-focus-visible:ring-primary after:absolute after:left-0.5 after:top-0.5 after:h-[18px] after:w-[18px] after:rounded-full after:bg-white after:transition-transform after:content-[''] peer-checked:after:translate-x-4"
          ></span>
        </label>
        <label class="flex items-center gap-4 border-b border-subtle py-3.5">
          <span class="min-w-0 flex-1">
            <span class="block text-[14px]">RSVP changes</span>
            <span class="block text-[12px] text-muted"
              >When someone answers an event you created</span
            >
          </span>
          <input type="checkbox" bind:checked={notifyRsvpChanges} class="peer sr-only" />
          <span
            class="relative h-[22px] w-[38px] shrink-0 rounded-full bg-line transition-colors peer-checked:bg-primary peer-focus-visible:ring-2 peer-focus-visible:ring-primary after:absolute after:left-0.5 after:top-0.5 after:h-[18px] after:w-[18px] after:rounded-full after:bg-white after:transition-transform after:content-[''] peer-checked:after:translate-x-4"
          ></span>
        </label>
        <label class="flex items-center gap-4 border-b border-subtle py-3.5">
          <span class="min-w-0 flex-1">
            <span class="block text-[14px]">Announcements</span>
            <span class="block text-[12px] text-muted">New posts in the mirrored channel</span>
          </span>
          <input type="checkbox" bind:checked={notifyAnnouncements} class="peer sr-only" />
          <span
            class="relative h-[22px] w-[38px] shrink-0 rounded-full bg-line transition-colors peer-checked:bg-primary peer-focus-visible:ring-2 peer-focus-visible:ring-primary after:absolute after:left-0.5 after:top-0.5 after:h-[18px] after:w-[18px] after:rounded-full after:bg-white after:transition-transform after:content-[''] peer-checked:after:translate-x-4"
          ></span>
        </label>
        <label class="flex items-center gap-4 py-3.5">
          <span class="min-w-0 flex-1">
            <span class="block text-[14px]">Event reminders</span>
            <span class="block text-[12px] text-muted">An hour before an event you're going to</span
            >
          </span>
          <input type="checkbox" bind:checked={notifyEventReminders} class="peer sr-only" />
          <span
            class="relative h-[22px] w-[38px] shrink-0 rounded-full bg-line transition-colors peer-checked:bg-primary peer-focus-visible:ring-2 peer-focus-visible:ring-primary after:absolute after:left-0.5 after:top-0.5 after:h-[18px] after:w-[18px] after:rounded-full after:bg-white after:transition-transform after:content-[''] peer-checked:after:translate-x-4"
          ></span>
        </label>

        <!-- ⚠️ The recipient's switch for a *private* message from the bot.
             Nudges are the only thing that uses it today. It sits with the
             other notification toggles rather than somewhere separate,
             because from the reader's side it is the same question: how do
             you want to be told. -->
        <label class="flex items-center gap-4 border-t border-subtle py-3.5">
          <span class="min-w-0 flex-1">
            <span class="block text-[14px]">Discord DMs</span>
            <span class="block text-[12px] text-muted">
              Let the bot message you privately when someone chases your answer
            </span>
          </span>
          <input type="checkbox" bind:checked={notifyDiscordDm} class="peer sr-only" />
          <span
            class="relative h-[22px] w-[38px] shrink-0 rounded-full bg-line transition-colors peer-checked:bg-primary peer-focus-visible:ring-2 peer-focus-visible:ring-primary after:absolute after:left-0.5 after:top-0.5 after:h-[18px] after:w-[18px] after:rounded-full after:bg-white after:transition-transform after:content-[''] peer-checked:after:translate-x-4"
          ></span>
        </label>
        <!-- No per-user "Weekly digest" toggle here on purpose. The digest
             is a single message posted to one shared Discord channel
             (services::digest), so there is no per-user delivery for a
             per-user preference to switch off - a checkbox here could only
             ever look functional. The real switch is the guild-level one
             on /server. -->
        <p class="m-0 pt-3 text-[12px] text-muted">
          The weekly digest is posted once to a shared Discord channel, so it's configured for the
          whole server on the <a href="/server" class="text-primary hover:underline"
            >Discord server</a
          > page rather than per person.
        </p>
      </section>

      {#if saveError}
        <p class="text-sm text-red-600" role="alert">{saveError}</p>
      {/if}
      {#if saved}
        <p class="text-sm text-green-700">Saved.</p>
      {/if}

      <button
        on:click={handleSave}
        disabled={saving}
        class="rounded-[9px] bg-primary px-3.5 py-[9px] text-[13px] font-semibold text-white hover:bg-primary-active disabled:opacity-50"
      >
        {saving ? 'Saving…' : 'Save changes'}
      </button>

      <section class="space-y-3 rounded-[14px] border border-red-300 bg-surface p-[18px]">
        <h2 class="m-0 text-[15px] font-semibold text-red-600">Delete account</h2>
        <p class="m-0 text-[13px] text-muted">
          This permanently deletes your account and everything tied to it (events you created,
          RSVPs, friend links). This can't be undone.
        </p>
        <label for="confirm-username" class="block text-sm font-medium text-gray-700">
          Type <span class="font-mono">{profile.username}</span> to confirm
        </label>
        <input
          id="confirm-username"
          type="text"
          bind:value={confirmUsername}
          class="w-full max-w-sm px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-red-500 focus:border-transparent"
        />
        {#if deleteError}
          <p class="text-sm text-red-600" role="alert">{deleteError}</p>
        {/if}
        <button
          on:click={handleDelete}
          disabled={deleting || confirmUsername !== profile.username}
          class="rounded-[9px] bg-red-600 px-3.5 py-[9px] text-[13px] font-semibold text-white disabled:opacity-50"
        >
          {deleting ? 'Deleting…' : 'Delete my account'}
        </button>
      </section>
    {/if}
  </div>
</Frame>
