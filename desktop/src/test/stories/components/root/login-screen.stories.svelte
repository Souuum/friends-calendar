<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import LoginScreen from '$lib/components/LoginScreen.svelte';
  import { mockApi } from '../../mockApi';

  /**
   * The signed-out screen: a full-height gradient with one card, "Login with Discord". The
   * root route renders it instead of the calendar while nobody is signed in.
   *
   * ```svelte
   * {#if $isAuthenticated}
   *   <Frame>
   *     <CalendarView />
   *   </Frame>
   * {:else}
   *   <LoginScreen />
   * {/if}
   * ```
   *
   * - No props, no slot, no events: it reports back through the stores. A successful sign-in
   *   sets `user` and `isAuthenticated`, which is what makes the parent swap it out.
   * - On mount it reads `api.getToken()`. With a stored token it calls `api.getCurrentUser()`
   *   to restore the session, and clears the token if that fails. Either way it then sets
   *   `isLoading` to false.
   * - "Login with Discord" opens the API's Discord sign-in in a new tab and swaps the button
   *   for a field to paste the token into, with Cancel and Submit. Submit stores the token and
   *   calls `api.getCurrentUser()`; a rejected token is cleared and the error shown in the card.
   * - Fills the viewport (`min-h-screen`), so it is a page on its own, not something to nest.
   */
  const { Story } = defineMeta({
    title: 'Components/LoginScreen',
    component: LoginScreen,
    parameters: {
      layout: 'fullscreen',
      // The screen is viewport-height and the story stubs `api`, so on the docs page it
      // needs a frame of its own.
      docs: { story: { inline: false, height: '700px' } }
    },
    // No stored token, so it renders the sign-in screen rather than
    // trying to restore a session.
    beforeEach: () => mockApi({ getToken: () => null })
  });
</script>

<!-- No stored token: the card with "Login with Discord". Clicking it shows the token field. -->
<Story name="Signed out" />
