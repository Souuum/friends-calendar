<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import AddServerForm from '$lib/components/molecules/AddServerForm.svelte';
  import { GUILDS, INVITE_URL } from '../../fixtures';
  import { mockApi, delay } from '../../mockApi';

  const { Story } = defineMeta({
    title: 'Components/Molecules/AddServerForm',
    component: AddServerForm,
    args: { inviteUrl: INVITE_URL }
  });
</script>

<!-- Submit any all-digit id to see each outcome. -->
<Story name="Succeeds" beforeEach={() => mockApi({ registerServer: () => delay(GUILDS[0]) })} />
<Story
  name="Bot not in that server"
  beforeEach={() =>
    mockApi({
      registerServer: () =>
        delay(null, 400).then(() => {
          throw new Error("The bot isn't in that server. Invite it first, then try again.");
        })
    })}
/>
