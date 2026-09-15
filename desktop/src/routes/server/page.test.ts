import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { BotChannelConfig, LinkedServerInfo } from '$lib/types';

const { getLinkedServer, getDiscordConfig, updateDiscordConfig } = vi.hoisted(() => ({
  getLinkedServer: vi.fn(),
  getDiscordConfig: vi.fn(),
  updateDiscordConfig: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: {
    getLinkedServer,
    getDiscordConfig,
    updateDiscordConfig,
    clearToken: vi.fn(),
    getToken: vi.fn(),
    getUnreadNotificationCount: vi.fn().mockResolvedValue(0)
  }
}));

vi.mock('$app/navigation', () => ({
  goto: vi.fn()
}));

vi.mock('$app/stores', () => ({
  page: {
    subscribe: (run: (value: { url: URL }) => void) => {
      run({ url: new URL('http://localhost/server') });
      return () => {};
    }
  }
}));

const { default: ServerPage } = await import('./+page.svelte');

const server: LinkedServerInfo = {
  id: 'g1',
  name: 'Friends Server',
  icon_url: undefined,
  approximate_member_count: 4
};

const emptyConfig: BotChannelConfig = {
  guild_id: 'g1',
  events_channel_id: undefined,
  announcements_channel_id: undefined,
  reminders_channel_id: undefined,
  digest_enabled: false,
  last_digest_sent_at: undefined
};

describe('server page', () => {
  beforeEach(() => {
    getLinkedServer.mockReset();
    getDiscordConfig.mockReset();
    updateDiscordConfig.mockReset();
  });

  it('loads and displays the linked server and bot channel config', async () => {
    getLinkedServer.mockResolvedValue(server);
    getDiscordConfig.mockResolvedValue({ ...emptyConfig, announcements_channel_id: '12345' });

    render(ServerPage);

    await waitFor(() => expect(screen.getByText('Friends Server')).toBeInTheDocument());
    expect(screen.getByLabelText('Announcements channel ID')).toHaveValue('12345');
  });

  it('shows an error if the linked server fails to load, without blocking channel config', async () => {
    getLinkedServer.mockRejectedValue(new Error('No Discord server is linked'));
    getDiscordConfig.mockResolvedValue(emptyConfig);

    render(ServerPage);

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('No Discord server is linked'));
    expect(screen.getByLabelText('Announcements channel ID')).toHaveValue('');
  });

  it('saves bot channel config changes', async () => {
    getLinkedServer.mockResolvedValue(server);
    getDiscordConfig.mockResolvedValue(emptyConfig);
    updateDiscordConfig.mockResolvedValue({ ...emptyConfig, announcements_channel_id: '999' });

    render(ServerPage);
    await waitFor(() => expect(screen.getByLabelText('Announcements channel ID')).toBeInTheDocument());

    await fireEvent.input(screen.getByLabelText('Announcements channel ID'), {
      target: { value: '999' }
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Save channels' }));

    await waitFor(() => expect(screen.getByText('Saved.')).toBeInTheDocument());
    expect(updateDiscordConfig).toHaveBeenCalledWith(
      expect.objectContaining({ announcements_channel_id: '999' })
    );
  });

  it('toggles the weekly digest setting', async () => {
    getLinkedServer.mockResolvedValue(server);
    getDiscordConfig.mockResolvedValue(emptyConfig);
    updateDiscordConfig.mockResolvedValue({ ...emptyConfig, digest_enabled: true });

    render(ServerPage);
    await waitFor(() => expect(screen.getByLabelText(/weekly digest/i)).not.toBeChecked());

    await fireEvent.click(screen.getByLabelText(/weekly digest/i));
    await fireEvent.click(screen.getByRole('button', { name: 'Save channels' }));

    await waitFor(() =>
      expect(updateDiscordConfig).toHaveBeenCalledWith(expect.objectContaining({ digest_enabled: true }))
    );
  });
});
