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
  <div class="max-w-2xl mx-auto py-6 px-3 sm:px-4 space-y-6 md:space-y-8 anim-fade-up">
    <button
      class="inline-flex items-center gap-1 text-sm text-discord-blurple hover:underline"
      on:click={() => goto('/')}
    >
      <Icon name="back" size={14} /> Back to calendar
    </button>

    <h1 class="text-2xl font-semibold m-0">Settings</h1>

    {#if loadError}
      <p class="text-sm text-red-600" role="alert">{loadError}</p>
    {:else if loading}
      <p class="text-sm text-gray-500">Loading…</p>
    {:else if profile}
      <section class="space-y-4">
        <h2 class="text-lg font-semibold">Profile</h2>

        <div>
          <label for="display-name" class="block text-sm font-medium text-gray-700 mb-1"
            >Display name</label
          >
          <input
            id="display-name"
            type="text"
            bind:value={displayName}
            placeholder={profile.username}
            class="w-full max-w-sm px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
          />
        </div>

        <div>
          <label for="timezone" class="block text-sm font-medium text-gray-700 mb-1">Timezone</label
          >
          <input
            id="timezone"
            type="text"
            bind:value={timezone}
            placeholder="UTC"
            class="w-full max-w-sm px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
          />
        </div>

        <div>
          <label for="default-visibility" class="block text-sm font-medium text-gray-700 mb-1">
            Default event visibility
          </label>
          <select
            id="default-visibility"
            bind:value={defaultVisibility}
            class="w-full max-w-sm px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
          >
            <option value="private">Private</option>
            <option value="friends">Friends</option>
            <option value="public">Public</option>
          </select>
        </div>

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
          <p class="text-xs text-gray-500 mt-1">
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

      <section class="space-y-3">
        <h2 class="text-lg font-semibold">Notifications</h2>

        <label class="flex items-center gap-2 text-sm">
          <input type="checkbox" bind:checked={notifyEventInvites} />
          Event invites
        </label>
        <label class="flex items-center gap-2 text-sm">
          <input type="checkbox" bind:checked={notifyRsvpChanges} />
          RSVP changes
        </label>
        <label class="flex items-center gap-2 text-sm">
          <input type="checkbox" bind:checked={notifyAnnouncements} />
          Announcements
        </label>
        <label class="flex items-center gap-2 text-sm">
          <input type="checkbox" bind:checked={notifyEventReminders} />
          Event reminders
          <span class="text-gray-500">— an hour before an event you're going to</span>
        </label>
        <!-- No per-user "Weekly digest" toggle here on purpose. The digest
             is a single message posted to one shared Discord channel
             (services::digest), so there is no per-user delivery for a
             per-user preference to switch off - a checkbox here could only
             ever look functional. The real switch is the guild-level one
             on /server. -->
        <p class="text-sm text-gray-500">
          The weekly digest is posted once to a shared Discord channel, so it's configured for the
          whole server on the <a href="/server" class="text-discord-blurple hover:underline"
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
        class="px-4 py-2 bg-primary text-white rounded-lg text-sm font-semibold disabled:opacity-50"
      >
        {saving ? 'Saving…' : 'Save changes'}
      </button>

      <section class="space-y-3 border border-red-200 rounded-lg p-4">
        <h2 class="text-lg font-semibold text-red-700">Delete account</h2>
        <p class="text-sm text-gray-600">
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
          class="px-4 py-2 bg-red-600 text-white rounded-lg text-sm font-semibold disabled:opacity-50"
        >
          {deleting ? 'Deleting…' : 'Delete my account'}
        </button>
      </section>
    {/if}
  </div>
</Frame>
