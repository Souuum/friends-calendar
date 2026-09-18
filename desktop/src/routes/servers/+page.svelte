<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import AddServerForm from '$lib/components/molecules/AddServerForm.svelte';
  import type { GuildInfo } from '$lib/types';

  let guilds: GuildInfo[] = [];
  let inviteUrl = '';
  let loading = true;
  let error = '';

  /**
   * @param showSpinner ⚠️ False when refreshing after a server was added.
   * `loading` swaps this whole branch out, which unmounts `AddServerForm` -
   * taking its "Added X" confirmation with it the instant it appeared, and
   * resetting the field mid-interaction. The list still updates; only the
   * spinner is skipped.
   */
  async function load(showSpinner = true) {
    try {
      if (showSpinner) loading = true;
      error = '';
      const response = await api.getServers();
      guilds = response.guilds;
      inviteUrl = response.invite_url;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load servers';
    } finally {
      loading = false;
    }
  }

  onMount(load);
</script>

<svelte:head>
  <title>Servers - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl space-y-6 anim-fade-up">
    <div>
      <h1 class="text-2xl font-semibold m-0">Discord servers</h1>
      <p class="text-sm text-gray-500 m-0 mt-1">
        Events can be announced in any of these. Friends and announcements sync from them too.
      </p>
    </div>

    {#if error}
      <p class="text-sm text-red-600" role="alert">{error}</p>
    {:else if loading}
      <p class="text-sm text-gray-500">Loading…</p>
    {:else}
      {#if guilds.length === 0}
        <p class="text-sm text-gray-500">
          The bot isn't in any server yet. Add it to one to start announcing events.
        </p>
      {:else}
        <div class="flex flex-col gap-2">
          {#each guilds as guild (guild.id)}
            <div class="flex items-center gap-3 bg-surface border border-line rounded-xl px-4 py-3">
              {#if guild.icon_url}
                <img src={guild.icon_url} alt="" class="w-10 h-10 rounded-xl shrink-0" />
              {:else}
                <div class="w-10 h-10 rounded-xl bg-gray-200 shrink-0"></div>
              {/if}
              <div class="min-w-0 flex-1">
                <p class="text-sm font-semibold text-gray-900 truncate m-0">
                  {guild.name ?? 'Unnamed server'}
                </p>
                <p class="font-mono text-[11px] text-muted truncate m-0">
                  {#if guild.name}
                    {guild.discord_guild_id}
                  {:else}
                    <!-- guild_create fills the name in when the bot next
                         connects; until then all we have is the id. -->
                    {guild.discord_guild_id} — name appears once the bot reconnects
                  {/if}
                </p>
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Authorising on Discord *is* how a server gets added; the bot then
           registers itself over the gateway. There's nothing to submit here. -->
      <a
        href={inviteUrl}
        target="_blank"
        rel="noopener noreferrer"
        class="inline-block px-4 py-2 bg-primary text-white rounded-lg text-sm font-semibold no-underline"
      >
        Add the bot to a server
      </a>
      <p class="text-xs text-gray-500">
        Opens Discord. The server appears here once the bot has joined and reconnected.
      </p>

      <!-- The fallback for when it hasn't reconnected: the gateway is what
           normally registers a server, and it has to be running to do it. -->
      <AddServerForm {inviteUrl} on:registered={() => load(false)} />

      <section class="space-y-2">
        <h2 class="text-lg font-semibold">Bot permissions</h2>
        <ul class="text-sm text-gray-600 list-disc list-inside space-y-1">
          <li>Read and send messages in the announcement channel, and in event threads</li>
          <!-- This ✅ stays an emoji on purpose. It is not a UI icon: it
               names the literal Discord character people react with to
               RSVP, which bot.rs matches on. An SVG here would describe
               the wrong thing. -->
          <li>Add reactions (the ✅ people RSVP with)</li>
          <li>Create threads for event discussion</li>
          <li>View server members (used for friend sync)</li>
        </ul>
      </section>
    {/if}
  </div>
</Frame>
