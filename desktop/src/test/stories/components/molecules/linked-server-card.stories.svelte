<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import LinkedServerCard from '$lib/components/molecules/LinkedServerCard.svelte';
  import { LINKED_SERVER } from '../../fixtures';

  /**
   * A row showing a Discord server: its icon, its name and, when known, its
   * member count. Use it to show which server the app is linked to.
   *
   * ```svelte
   * {#if server}
   *   <LinkedServerCard {server} />
   * {/if}
   * ```
   *
   * Presentational only: it fetches nothing. The caller loads the server, for
   * example with `api.getLinkedServer()`.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/LinkedServerCard',
    component: LinkedServerCard,
    args: { server: LINKED_SERVER }
  });
</script>

<!-- A server with an icon and a member count. -->
<Story name="Default" />

<!-- Without `icon_url` and `approximate_member_count`: the avatar falls back and the members line is left out. -->
<Story
  name="No icon, no member count"
  args={{ server: { id: LINKED_SERVER.id, name: 'Unnamed Server' } }}
/>
