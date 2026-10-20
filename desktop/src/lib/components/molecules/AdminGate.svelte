<script lang="ts">
  import { isAdmin } from '$lib/admin';

  /** What the page would let an admin do, for the refusal message. */
  export let what = "change the bot's configuration";
</script>

<!--
  Shows its content only to admins of the linked Discord server. This is the
  courtesy half: the backend refuses config changes from anyone else on its
  own, so this decides what is worth showing, not what is allowed.
-->
{#if $isAdmin === null}
  <p class="m-0 text-sm text-muted" role="status">Checking your permissions…</p>
{:else if $isAdmin}
  <slot />
{:else}
  <section class="rounded-xl border border-line bg-surface px-4 py-3.5" role="status">
    <h1 class="m-0 text-[15px] font-semibold">Admins only</h1>
    <p class="m-0 mt-1 text-[13px] text-muted">
      Only the server's admins can {what}: its owner, or anyone with Administrator or Manage Server
      on Discord. Ask one of them if something needs changing.
    </p>
  </section>
{/if}
