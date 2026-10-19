<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import AnnouncementPostCard from '$lib/components/molecules/AnnouncementPostCard.svelte';
  import { ANNOUNCEMENT_BOT, ANNOUNCEMENT_HANDWRITTEN } from '../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Molecules/AnnouncementPostCard',
    component: AnnouncementPostCard,
    args: { post: ANNOUNCEMENT_BOT }
  });
</script>

<Story name="Event announcement" />
<Story name="Featured (first pinned)" args={{ featured: true }} />
<Story name="Hand-written, adoptable" args={{ post: ANNOUNCEMENT_HANDWRITTEN, onAdopt: fn() }} />
<Story
  name="No thread link"
  args={{ post: { ...ANNOUNCEMENT_HANDWRITTEN, thread_url: undefined } }}
/>
