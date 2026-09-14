import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import type { FriendInfo, LinkedServerInfo, SyncFriendsResult } from '$lib/types';

const { getLinkedServer, getFriends, syncFriends } = vi.hoisted(() => ({
  getLinkedServer: vi.fn(),
  getFriends: vi.fn(),
  syncFriends: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { getLinkedServer, getFriends, syncFriends, clearToken: vi.fn(), getToken: vi.fn() }
}));

vi.mock('$app/navigation', () => ({
  goto: vi.fn()
}));

// Frame.svelte (rendered inside this page) reads $page.url.pathname to
// highlight the active sidebar item - $app/stores isn't available outside
// a real SvelteKit runtime, so it needs a minimal store stand-in here.
vi.mock('$app/stores', () => ({
  page: {
    subscribe: (run: (value: { url: URL }) => void) => {
      run({ url: new URL('http://localhost/settings') });
      return () => {};
    }
  }
}));

// Loaded after the mocks above so the page picks up the mocked $lib/api.
const { default: SettingsPage } = await import('./+page.svelte');

const server: LinkedServerInfo = {
  id: 'g1',
  name: 'Friends Server',
  icon_url: undefined,
  approximate_member_count: 4
};

const alice: FriendInfo = {
  user_id: 'alice-id',
  username: 'alice',
  avatar_url: undefined,
  synced_at: '2026-01-01T00:00:00Z'
};

describe('settings page', () => {
  beforeEach(() => {
    getLinkedServer.mockReset();
    getFriends.mockReset();
    syncFriends.mockReset();
  });

  it('loads and displays the linked server and friends', async () => {
    getLinkedServer.mockResolvedValue(server);
    getFriends.mockResolvedValue([alice]);

    render(SettingsPage);

    await waitFor(() => expect(screen.getByText('Friends Server')).toBeInTheDocument());
    expect(screen.getByText('alice')).toBeInTheDocument();
  });

  it('shows an error if the linked server fails to load, without blocking friends', async () => {
    getLinkedServer.mockRejectedValue(new Error('No Discord server is linked'));
    getFriends.mockResolvedValue([alice]);

    render(SettingsPage);

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('No Discord server is linked'));
    expect(screen.getByText('alice')).toBeInTheDocument();
  });

  it('re-syncs friends when the sync button is clicked', async () => {
    getLinkedServer.mockResolvedValue(server);
    getFriends.mockResolvedValue([]);
    const result: SyncFriendsResult = { synced: 1, removed: 0, friends: [alice] };
    syncFriends.mockResolvedValue(result);

    render(SettingsPage);

    await waitFor(() => expect(screen.getByText('No friends synced yet.')).toBeInTheDocument());

    const { fireEvent } = await import('@testing-library/svelte');
    await fireEvent.click(screen.getByRole('button', { name: 'Sync friends' }));

    await waitFor(() => expect(screen.getByText('alice')).toBeInTheDocument());
    expect(syncFriends).toHaveBeenCalledOnce();
  });
});
