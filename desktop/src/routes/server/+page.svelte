<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import LinkedServerCard from '$lib/components/molecules/LinkedServerCard.svelte';
  import type { LinkedServerInfo, BotChannelConfig, ChannelInfo } from '$lib/types';

  let server: LinkedServerInfo | null = null;
  let serverError = '';
  let serverLoading = true;

  let config: BotChannelConfig | null = null;
  let configError = '';
  let configLoading = true;

  let announcementsChannelId = '';
  let digestEnabled = false;

  /**
   * The channels the bot can post in.
   *
   * `null` means we couldn't get them - no guild registered, no bot token,
   * or Discord refused - and the form falls back to the raw id field rather
   * than blocking the page. A deployment whose bot is offline still has to
   * be configurable; that's the same degrade-don't-crash rule `config.rs`
   * follows for every Discord env var.
   */
  let channels: ChannelInfo[] | null = null;
  let channelsLoading = true;

  async function loadChannels() {
    try {
      channelsLoading = true;
      // `/server` knows the Discord snowflake; the channels endpoint is
      // keyed by our own guilds.id, so resolve one to the other. One extra
      // request on a settings page, and it keeps the snowflake out of a
      // client-supplied path.
      const { guilds } = await api.getServers();
      const match = guilds.find((g) => g.discord_guild_id === server?.id) ?? guilds[0];
      channels = match ? await api.getGuildChannels(match.id) : null;
    } catch {
      // Deliberately silent: the fallback field is the error message. A
      // banner here would be shouting about a degraded path that still works.
      channels = null;
    } finally {
      channelsLoading = false;
    }
  }

  /** Grouped for the picker, in the order the backend already sorted them. */
  $: groupedChannels = (channels ?? []).reduce<{ category: string | null; items: ChannelInfo[] }[]>(
    (groups, channel) => {
      const category = channel.category ?? null;
      const last = groups[groups.length - 1];
      if (last && last.category === category) last.items.push(channel);
      else groups.push({ category, items: [channel] });
      return groups;
    },
    []
  );

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
      announcementsChannelId = config.announcements_channel_id ?? '';
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
        announcements_channel_id:
          announcementsChannelId.trim() === '' ? undefined : announcementsChannelId.trim(),
        digest_enabled: digestEnabled
      });
      saved = true;
    } catch (err) {
      saveError = err instanceof Error ? err.message : 'Failed to save bot channel config';
    } finally {
      saving = false;
    }
  }

  onMount(async () => {
    // Channels need the server's identity, so it has to land first.
    await loadServer();
    loadConfig();
    loadChannels();
  });
</script>

<svelte:head>
  <title>Discord Server - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="space-y-4 anim-fade-up">
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
        The Discord channel the bot posts announcements to. A change takes effect immediately for
        new events; the bot's own gateway connection only picks up a change on restart. Event
        reminders go into each event's own thread, so they need no channel of their own.
      </p>

      {#if configError}
        <p class="text-sm text-red-600" role="alert">{configError}</p>
      {:else if configLoading}
        <p class="text-sm text-gray-500">Loading…</p>
      {:else if config}
        <div>
          <!-- The label is a <span> with an id rather than a <label for>,
               because which control it names depends on the branch below:
               a group of buttons when the picker is available, a text input
               when it is not. A `for` pointing at an input that doesn't
               exist names nothing. -->
          <span
            id="announcements-channel-label"
            class="mb-1 block text-sm font-medium text-gray-700"
          >
            Announcements channel
          </span>

          {#if channelsLoading}
            <p class="text-sm text-muted">Loading channels…</p>
          {:else if channels && channels.length > 0}
            <!-- A picker, not a snowflake field. Getting an id out of Discord
                 means enabling Developer Mode and right-clicking a channel;
                 nothing validated what you pasted, and a plausible-but-wrong
                 id failed silently because announcing is best-effort. -->
            <div
              class="flex flex-wrap gap-2"
              role="group"
              aria-labelledby="announcements-channel-label"
            >
              {#each groupedChannels as group (group.category ?? '__none')}
                <div class="w-full">
                  {#if group.category}
                    <p
                      class="m-0 mb-1.5 font-mono text-[10px] uppercase tracking-[0.1em] text-muted"
                    >
                      {group.category}
                    </p>
                  {/if}
                  <div class="mb-3 flex flex-wrap gap-2">
                    {#each group.items as channel (channel.id)}
                      <button
                        type="button"
                        aria-pressed={announcementsChannelId === channel.id}
                        on:click={() => (announcementsChannelId = channel.id)}
                        class="rounded-[9px] border px-3 py-[7px] text-[13px] transition-colors {announcementsChannelId ===
                        channel.id
                          ? 'border-primary bg-tint font-semibold text-primary'
                          : 'border-line bg-surface hover:bg-subtle'}"
                      >
                        #{channel.name}
                      </button>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
            <p class="m-0 text-[12px] text-muted">
              Only channels the bot can see are listed.{#if announcementsChannelId}
                Selected id <span class="font-mono">{announcementsChannelId}</span>.{/if}
            </p>
          {:else}
            <!-- No guild registered, no bot token, or Discord refused. The
                 raw field still works, so the page stays usable. -->
            <input
              id="announcements-channel"
              aria-labelledby="announcements-channel-label"
              type="text"
              bind:value={announcementsChannelId}
              placeholder="Channel ID"
              class="w-full max-w-sm px-3 py-2 border border-gray-300 rounded-lg text-sm font-mono focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            />
            <p class="m-0 mt-1 text-[12px] text-muted">
              Couldn't reach Discord to list channels, so paste the id instead — enable Developer
              Mode in Discord, right-click the channel and choose "Copy Channel ID".
            </p>
          {/if}
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
          {saving ? 'Saving…' : 'Save channel'}
        </button>
      {/if}
    </section>

    <section class="space-y-2">
      <h2 class="text-lg font-semibold">Bot permissions</h2>
      <ul class="text-sm text-gray-600 list-disc list-inside space-y-1">
        <li>Read and send messages in the channel above, and in event threads</li>
        <li>View server members (used for friend sync)</li>
        <li>Create and manage invite links</li>
      </ul>
    </section>
  </div>
</Frame>
