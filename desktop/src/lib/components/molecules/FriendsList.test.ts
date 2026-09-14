import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import FriendsList from './FriendsList.svelte';
import type { FriendInfo } from '$lib/types';

const alice: FriendInfo = {
  user_id: 'alice-id',
  username: 'alice',
  avatar_url: 'https://cdn.discordapp.com/avatars/alice-discord/abc123.png',
  synced_at: '2026-01-01T00:00:00Z'
};

const bob: FriendInfo = {
  user_id: 'bob-id',
  username: 'bob',
  avatar_url: undefined,
  synced_at: '2026-01-01T00:00:00Z'
};

describe('FriendsList', () => {
  it('shows an empty state when there are no synced friends', () => {
    render(FriendsList, { friends: [] });

    expect(screen.getByText('No friends synced yet.')).toBeInTheDocument();
  });

  it('renders one row per friend with their username', () => {
    render(FriendsList, { friends: [alice, bob] });

    expect(screen.getByText('alice')).toBeInTheDocument();
    expect(screen.getByText('bob')).toBeInTheDocument();
    expect(screen.queryByText('No friends synced yet.')).not.toBeInTheDocument();
  });

  it('surfaces an error instead of the friend list or empty state', () => {
    render(FriendsList, { friends: [alice], error: 'Friend sync is not configured' });

    expect(screen.getByRole('alert')).toHaveTextContent('Friend sync is not configured');
    expect(screen.queryByText('alice')).not.toBeInTheDocument();
  });

  it('only renders a sync button when an onSync handler is provided', () => {
    const { rerender } = render(FriendsList, { friends: [] });
    expect(screen.queryByRole('button')).not.toBeInTheDocument();

    rerender({ friends: [], onSync: () => {} });
    expect(screen.getByRole('button', { name: 'Sync friends' })).toBeInTheDocument();
  });

  it('calls onSync when the sync button is clicked', async () => {
    const onSync = vi.fn();
    render(FriendsList, { friends: [], onSync });

    await fireEvent.click(screen.getByRole('button', { name: 'Sync friends' }));

    expect(onSync).toHaveBeenCalledOnce();
  });

  it('disables the sync button and relabels it while syncing', () => {
    render(FriendsList, { friends: [], onSync: () => {}, syncing: true });

    const button = screen.getByRole('button', { name: 'Syncing…' });
    expect(button).toBeDisabled();
  });
});
