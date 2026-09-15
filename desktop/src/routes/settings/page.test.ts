import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { User } from '$lib/types';

const { getCurrentUser, updateProfile, deleteAccount } = vi.hoisted(() => ({
  getCurrentUser: vi.fn(),
  updateProfile: vi.fn(),
  deleteAccount: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: {
    getCurrentUser,
    updateProfile,
    deleteAccount,
    clearToken: vi.fn(),
    getToken: vi.fn(),
    getUnreadNotificationCount: vi.fn().mockResolvedValue(0)
  }
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

function makeUser(overrides: Partial<User> = {}): User {
  return {
    id: 'me-id',
    discord_id: 'me-discord',
    username: 'me',
    timezone: 'UTC',
    default_visibility: 'friends',
    notify_event_invites: true,
    notify_rsvp_changes: true,
    notify_announcements: false,
    notify_weekly_digest: true,
    ...overrides
  };
}

describe('settings page', () => {
  beforeEach(() => {
    getCurrentUser.mockReset();
    updateProfile.mockReset();
    deleteAccount.mockReset();
  });

  it('loads and displays the current profile', async () => {
    getCurrentUser.mockResolvedValue(makeUser({ display_name: 'Me!' }));

    render(SettingsPage);

    await waitFor(() => expect(screen.getByLabelText('Display name')).toHaveValue('Me!'));
    expect(screen.getByLabelText('Timezone')).toHaveValue('UTC');
  });

  it('shows an error if the profile fails to load', async () => {
    getCurrentUser.mockRejectedValue(new Error('network down'));

    render(SettingsPage);

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('network down'));
  });

  it('saves profile changes', async () => {
    getCurrentUser.mockResolvedValue(makeUser());
    updateProfile.mockResolvedValue(makeUser({ display_name: 'New Name' }));

    render(SettingsPage);
    await waitFor(() => expect(screen.getByLabelText('Display name')).toBeInTheDocument());

    await fireEvent.input(screen.getByLabelText('Display name'), { target: { value: 'New Name' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(screen.getByText('Saved.')).toBeInTheDocument());
    expect(updateProfile).toHaveBeenCalledWith(
      expect.objectContaining({ display_name: 'New Name' })
    );
  });

  it('only allows deleting the account once the username is typed to confirm', async () => {
    getCurrentUser.mockResolvedValue(makeUser());

    render(SettingsPage);
    await waitFor(() => expect(screen.getByLabelText('Display name')).toBeInTheDocument());

    const deleteButton = screen.getByRole('button', { name: 'Delete my account' });
    expect(deleteButton).toBeDisabled();

    await fireEvent.input(screen.getByLabelText(/Type/), { target: { value: 'me' } });
    expect(deleteButton).not.toBeDisabled();

    deleteAccount.mockResolvedValue(undefined);
    await fireEvent.click(deleteButton);

    await waitFor(() => expect(deleteAccount).toHaveBeenCalledWith('me'));
  });
});
