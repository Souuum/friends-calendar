<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import AddServerForm from '$lib/components/molecules/AddServerForm.svelte';
  import { GUILDS, INVITE_URL } from '../../fixtures';
  import { mockApi, delay } from '../../mockApi';

  /**
   * A card with one field for registering a Discord server by its id. It is the
   * fallback on `/servers` for a server the bot is in but has not registered on
   * its own yet.
   *
   * ```svelte
   * <AddServerForm {inviteUrl} on:registered={() => load(false)} />
   * ```
   *
   * - On submit it calls `api.registerServer(id)` itself. An empty field shows
   *   "Paste the server ID first" and sends nothing.
   * - Dispatches `registered` with `{ guild }` once the server is added, and
   *   shows "Added <name>." under the form.
   * - A failure shows the error message, followed by an "Invite the bot" link
   *   when `inviteUrl` is set.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/AddServerForm',
    component: AddServerForm,
    args: { inviteUrl: INVITE_URL },
    // Each story stubs `api.registerServer`, so on the docs page they need a frame each.
    parameters: { docs: { story: { inline: false, height: '300px' } } }
  });
</script>

<!-- Submit any id: the stubbed call succeeds and the "Added ..." confirmation appears. -->
<Story name="Succeeds" beforeEach={() => mockApi({ registerServer: () => delay(GUILDS[0]) })} />

<!-- Submit any id: the stubbed call fails, showing the error with the "Invite the bot" link from `inviteUrl`. -->
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

<!-- Without `inviteUrl` (the default), a failure shows the message alone, with no link. -->
<Story
  name="Failure without an invite link"
  args={{ inviteUrl: '' }}
  beforeEach={() =>
    mockApi({
      registerServer: () =>
        delay(null, 400).then(() => {
          throw new Error("The bot isn't in that server. Invite it first, then try again.");
        })
    })}
/>
