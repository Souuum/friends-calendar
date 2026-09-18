<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { api } from '$lib/api';
  import type { GuildInfo } from '$lib/types';

  /** Where to send someone who hasn't invited the bot yet. */
  export let inviteUrl = '';

  const dispatch = createEventDispatcher<{ registered: { guild: GuildInfo } }>();

  let serverId = '';
  let submitting = false;
  let error = '';
  let registered: GuildInfo | null = null;

  async function handleSubmit() {
    error = '';
    registered = null;

    // ⚠️ `novalidate` on the form, so this is the only validation. Native
    // constraint validation reports through a tooltip that is easy to miss
    // and bypasses the error box below entirely - the adopt modal shipped
    // stuck-on-submit that way. See CLAUDE.md.
    const id = serverId.trim();
    if (id === '') {
      error = 'Paste the server ID first';
      return;
    }

    try {
      submitting = true;
      const guild = await api.registerServer(id);
      registered = guild;
      serverId = '';
      dispatch('registered', { guild });
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not add that server';
    } finally {
      submitting = false;
    }
  }
</script>

<section class="rounded-xl border border-line bg-surface px-4 py-3.5">
  <h2 class="m-0 text-[15px] font-semibold">Add a server by ID</h2>
  <!--
    Being straight about what this does. The bot registers servers itself
    when the gateway connects, so this is for when that hasn't happened -
    not an alternative to inviting it, which is still required and which
    the server verifies before recording anything.
  -->
  <p class="m-0 mt-1 text-[13px] text-muted">
    Servers normally appear here on their own once the bot joins. Use this if one is missing — the
    bot still has to be in it, and we'll check with Discord before adding it.
  </p>

  <form on:submit|preventDefault={handleSubmit} novalidate class="mt-3 flex flex-wrap gap-2">
    <input
      id="server-id"
      bind:value={serverId}
      type="text"
      inputmode="numeric"
      autocomplete="off"
      placeholder="123456789012345678"
      aria-label="Discord server ID"
      class="min-w-0 flex-1 rounded-[11px] border border-line bg-transparent px-3 py-2 font-mono text-[13px]"
    />
    <button
      type="submit"
      disabled={submitting}
      class="min-h-[44px] shrink-0 rounded-[11px] bg-primary px-4 text-[13px] font-semibold text-white disabled:opacity-60"
    >
      {submitting ? 'Checking…' : 'Add server'}
    </button>
  </form>

  <p class="m-0 mt-2 text-[12px] text-muted">
    In Discord: enable <span class="font-medium">Developer Mode</span> in Settings → Advanced, then
    right-click the server and choose <span class="font-medium">Copy Server ID</span>.
  </p>

  {#if error}
    <p class="m-0 mt-2 text-[13px] text-red-600" role="alert">
      {error}
      {#if inviteUrl}
        <!-- The most common failure is that the bot was never invited, so the
             way out sits next to the message rather than further up the page. -->
        <a href={inviteUrl} target="_blank" rel="noopener noreferrer" class="underline">
          Invite the bot
        </a>
      {/if}
    </p>
  {/if}

  {#if registered}
    <p class="m-0 mt-2 text-[13px] font-medium" role="status">
      Added {registered.name ?? registered.discord_guild_id}.
    </p>
  {/if}
</section>
