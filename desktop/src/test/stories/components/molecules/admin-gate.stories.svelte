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

  const { Story } = defineMeta({
    title: 'Components/Molecules/AdminGate',
    component: AdminGate
  });
</script>

{#snippet gated(args: ComponentProps<typeof AdminGate>)}
  <AdminGate {...args}>
    <p class="m-0 text-sm">The bot configuration form would be here.</p>
  </AdminGate>
{/snippet}

<Story name="Admin" beforeEach={as(true)} template={gated} />
<Story name="Not an admin" beforeEach={as(false)} template={gated} />
<Story
  name="Not an admin, custom action"
  args={{ what: 'add servers or invite the bot' }}
  beforeEach={as(false)}
  template={gated}
/>
<Story name="Still checking" beforeEach={as(null)} template={gated} />
