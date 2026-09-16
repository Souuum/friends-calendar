<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import LinkedServerCard from '$lib/components/molecules/LinkedServerCard.svelte';
  import type { LinkedServerInfo, BotChannelConfig } from '$lib/types';

  let server: LinkedServerInfo | null = null;
  let serverError = '';
  let serverLoading = true;

  let config: BotChannelConfig | null = null;
  let configError = '';
  let configLoading = true;

  let eventsChannelId = '';
  let announcementsChannelId = '';
  let remindersChannelId = '';
  let digestEnabled = false;

  let saving = false;
  let saveError = '';
  let saved = false;

  async function loadServer() {
    try {
      serverLoading = true;
      serverError = '';
      server = await api.getLinkedServer();
    } catch (err) {
      serverError = err instanceof Error ? err.message : 'Failed to load linked server';
    } finally {
      serverLoading = false;
    }
  }

  async function loadConfig() {
    try {
      configLoading = true;
      configError = '';
      config = await api.getDiscordConfig();
      eventsChannelId = config.events_channel_id ?? '';
      announcementsChannelId = config.announcements_channel_id ?? '';
      remindersChannelId = config.reminders_channel_id ?? '';
      digestEnabled = config.digest_enabled;
    } catch (err) {
      configError = err instanceof Error ? err.message : 'Failed to load bot channel config';
    } finally {
      configLoading = false;
    }
  }

  async function handleSave() {
    try {
      saving = true;
      saveError = '';
      saved = false;
      config = await api.updateDiscordConfig({
        events_channel_id: eventsChannelId.trim() === '' ? undefined : eventsChannelId.trim(),
        announcements_channel_id:
          announcementsChannelId.trim() === '' ? undefined : announcementsChannelId.trim(),
        reminders_channel_id: remindersChannelId.trim() === '' ? undefined : remindersChannelId.trim(),
        digest_enabled: digestEnabled
      });
      saved = true;
    } catch (err) {
      saveError = err instanceof Error ? err.message : 'Failed to save bot channel config';
    } finally {
      saving = false;
    }
  }

  onMount(() => {
    loadServer();
    loadConfig();
  });
</script>

<svelte:head>
  <title>Discord Server - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl mx-auto py-6 px-3 sm:px-4 space-y-6 md:space-y-8 anim-fade-up">
    <!-- Mobile reaches this page by pushing in from Settings ("Me"), so the
         back affordance points there; on desktop the sidebar is visible and
         either destination is one click away. -->
    <button class="text-sm text-discord-blurple hover:underline" on:click={() => goto('/settings')}>
      ‹ Me
    </button>

    <h1 class="text-2xl font-semibold m-0">Discord server</h1>

    <section>
      {#if serverError}
        <p class="text-sm text-red-600" role="alert">{serverError}</p>
      {:else if serverLoading}
        <p class="text-sm text-gray-500">Loading…</p>
      {:else if server}
        <LinkedServerCard {server} />
      {/if}
    </section>

    <section class="space-y-4">
      <h2 class="text-lg font-semibold">Bot channels</h2>
      <p class="text-sm text-gray-500">
        Discord channel IDs the bot posts to. Changes to the announcements channel take effect
        immediately for new events; the bot's own gateway connection only picks up a change on
        restart.
      </p>

      {#if configError}
        <p class="text-sm text-red-600" role="alert">{configError}</p>
      {:else if configLoading}
        <p class="text-sm text-gray-500">Loading…</p>
      {:else if config}
        <div>
          <label for="events-channel" class="block text-sm font-medium text-gray-700 mb-1">
            Events channel ID
          </label>
          <input
            id="events-channel"
            type="text"
            bind:value={eventsChannelId}
            placeholder="Channel ID"
            class="w-full max-w-sm px-3 py-2 border border-gray-300 rounded-lg text-sm font-mono focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
          />
        </div>

        <div>
          <label for="announcements-channel" class="block text-sm font-medium text-gray-700 mb-1">
            Announcements channel ID
          </label>
          <input
            id="announcements-channel"
            type="text"
            bind:value={announcementsChannelId}
            placeholder="Channel ID"
            class="w-full max-w-sm px-3 py-2 border border-gray-300 rounded-lg text-sm font-mono focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
          />
        </div>

        <div>
          <label for="reminders-channel" class="block text-sm font-medium text-gray-700 mb-1">
            Reminders channel ID
          </label>
          <input
            id="reminders-channel"
            type="text"
            bind:value={remindersChannelId}
            placeholder="Channel ID"
            class="w-full max-w-sm px-3 py-2 border border-gray-300 rounded-lg text-sm font-mono focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
          />
        </div>

        <label class="flex items-center gap-2 text-sm">
          <input type="checkbox" bind:checked={digestEnabled} />
          Post a weekly digest to the announcements channel (every Monday, 9:00 UTC)
        </label>
        {#if config.last_digest_sent_at}
          <p class="text-xs text-gray-500">
            Last sent {new Date(config.last_digest_sent_at).toLocaleString()}
          </p>
        {/if}

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
          {saving ? 'Saving…' : 'Save channels'}
        </button>
      {/if}
    </section>

    <section class="space-y-2">
      <h2 class="text-lg font-semibold">Bot permissions</h2>
      <ul class="text-sm text-gray-600 list-disc list-inside space-y-1">
        <li>Read and send messages in the channels above</li>
        <li>View server members (used for friend sync)</li>
        <li>Create and manage invite links</li>
      </ul>
    </section>
  </div>
</Frame>
