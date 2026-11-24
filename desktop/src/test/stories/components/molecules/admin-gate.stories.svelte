<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import type { ComponentProps } from 'svelte';
  import AdminGate from '$lib/components/molecules/AdminGate.svelte';
  import { isAdmin } from '$lib/admin';

  /** Sets the shared store for one story and clears it afterwards. */
  const as = (value: boolean | null) => () => {
    isAdmin.set(value);
    return () => isAdmin.set(null);
  };

  /**
   * Shows its slot only to admins of the linked Discord server. Wrap the content
   * of a page that only admins can use.
   *
   * ```svelte
   * <AdminGate what="add servers or invite the bot">
   *   <ServerSettings />
   * </AdminGate>
   * ```
   *
   * - Takes no admin flag: it reads the shared `isAdmin` store from `$lib/admin`.
   *   `null` shows "Checking your permissions…", `true` renders the slot, `false`
   *   shows an "Admins only" card that uses `what` in its sentence.
   * - It only hides things. The backend refuses non-admins on its own.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/AdminGate',
    component: AdminGate,
    // Each story sets the shared `isAdmin` store, so on the docs page they need a frame each.
    parameters: { docs: { story: { inline: false, height: '140px' } } }
  });
</script>

{#snippet gated(args: ComponentProps<typeof AdminGate>)}
  <AdminGate {...args}>
    <p class="m-0 text-sm">The bot configuration form would be here.</p>
  </AdminGate>
{/snippet}

<!-- `isAdmin` is `true`: the slot content is rendered as is. -->
<Story name="Admin" beforeEach={as(true)} template={gated} />

<!-- `isAdmin` is `false`: the "Admins only" card with the default `what`. -->
<Story name="Not an admin" beforeEach={as(false)} template={gated} />

<!-- `what` replaces the action named in the refusal sentence. -->
<Story
  name="Not an admin, custom action"
  args={{ what: 'add servers or invite the bot' }}
  beforeEach={as(false)}
  template={gated}
/>

<!-- `isAdmin` is still `null`: a "Checking your permissions…" line and no slot content. -->
<Story name="Still checking" beforeEach={as(null)} template={gated} />
