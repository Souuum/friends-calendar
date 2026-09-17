<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import { user as userStore } from '$lib/stores';
  import Frame from '$lib/components/templates/Frame.svelte';
  import { theme, setTheme, type Theme } from '$lib/theme';
  import Icon from '$lib/components/atoms/Icon.svelte';
  import type { User, Visibility } from '$lib/types';

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
        notify_event_reminders: notifyEventReminders
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

  onMount(load);
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
