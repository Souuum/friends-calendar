<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import LoginScreen from '$lib/components/LoginScreen.svelte';
  import { mockApi } from '../../mockApi';

  const { Story } = defineMeta({
    title: 'Components/LoginScreen',
    component: LoginScreen,
    parameters: { layout: 'fullscreen' },
    // No stored token, so it renders the sign-in screen rather than
    // trying to restore a session.
    beforeEach: () => mockApi({ getToken: () => null })
  });
</script>

<Story name="Signed out" />
