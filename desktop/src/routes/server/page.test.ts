import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { BotChannelConfig, LinkedServerInfo } from '$lib/types';

const { getLinkedServer, getDiscordConfig, updateDiscordConfig, getServers, getGuildChannels } =
  vi.hoisted(() => ({
    getLinkedServer: vi.fn(),
    getDiscordConfig: vi.fn(),
    updateDiscordConfig: vi.fn(),
    getServers: vi.fn(),
    getGuildChannels: vi.fn()
  }));

vi.mock('$lib/api', () => ({
  api: {
    getLinkedServer,
    getDiscordConfig,
    updateDiscordConfig,
    getServers,
    getGuildChannels,
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
  announcements_channel_id: undefined,
  digest_enabled: false,
  last_digest_sent_at: undefined
};

describe('server page', () => {
  beforeEach(() => {
    getLinkedServer.mockReset();
    getDiscordConfig.mockReset();
    updateDiscordConfig.mockReset();
    getServers.mockReset();
    getGuildChannels.mockReset();
    // Default: Discord is unreachable, so the page falls back to the raw id
    // field. Tests that want the picker opt in - which keeps the existing
    // tests, written against that field, meaningful as the fallback path.
    getServers.mockRejectedValue(new Error('no servers'));
    getGuildChannels.mockRejectedValue(new Error('no channels'));
  });

  /** Makes the channel picker available, with two channels in a category. */
  function withChannels() {
    getServers.mockResolvedValue({
      guilds: [{ id: 'guild-uuid', discord_guild_id: '123456789', name: 'Friends Server' }],
      invite_url: 'https://discord.com/x'
    });
    getGuildChannels.mockResolvedValue([
      { id: 'c0', name: 'lobby' },
      { id: 'c1', name: 'general', category: 'Text channels' },
      { id: 'c2', name: 'announcements', category: 'Text channels' }
    ]);
  }

  it('loads and displays the linked server and bot channel config', async () => {
    getLinkedServer.mockResolvedValue(server);
    getDiscordConfig.mockResolvedValue({ ...emptyConfig, announcements_channel_id: '12345' });

    render(ServerPage);

    await waitFor(() => expect(screen.getByText('Friends Server')).toBeInTheDocument());
    expect(screen.getByLabelText('Announcements channel')).toHaveValue('12345');
  });

  it('shows an error if the linked server fails to load, without blocking channel config', async () => {
    getLinkedServer.mockRejectedValue(new Error('No Discord server is linked'));
    getDiscordConfig.mockResolvedValue(emptyConfig);

    render(ServerPage);

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent('No Discord server is linked')
    );
    expect(screen.getByLabelText('Announcements channel')).toHaveValue('');
  });

  it('saves bot channel config changes', async () => {
    getLinkedServer.mockResolvedValue(server);
    getDiscordConfig.mockResolvedValue(emptyConfig);
    updateDiscordConfig.mockResolvedValue({ ...emptyConfig, announcements_channel_id: '999' });

    render(ServerPage);
    await waitFor(() => expect(screen.getByLabelText('Announcements channel')).toBeInTheDocument());

    await fireEvent.input(screen.getByLabelText('Announcements channel'), {
      target: { value: '999' }
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Save channel' }));

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
    await fireEvent.click(screen.getByRole('button', { name: 'Save channel' }));

    await waitFor(() =>
      expect(updateDiscordConfig).toHaveBeenCalledWith(
        expect.objectContaining({ digest_enabled: true })
      )
    );
  });
  describe('the channel picker', () => {
    it('offers the channels the bot can see, grouped by category', async () => {
      withChannels();
      getLinkedServer.mockResolvedValue(server);
      getDiscordConfig.mockResolvedValue(emptyConfig);

      render(ServerPage);

      await waitFor(() =>
        expect(screen.getByRole('button', { name: '#general' })).toBeInTheDocument()
      );
      expect(screen.getByText('Text channels')).toBeInTheDocument();
      // The bot only sees what it has VIEW_CHANNEL on, so a missing channel
      // is normal and the page has to say so.
      expect(screen.getByText(/Only channels the bot can see/)).toBeInTheDocument();
      // No snowflake field when the picker is available. By role, not by
      // label: the picker group carries that label now, and correctly so.
      expect(
        screen.queryByRole('textbox', { name: 'Announcements channel' })
      ).not.toBeInTheDocument();
    });

    // The whole failure this replaces is silent misconfiguration, so a
    // picker that saved to the wrong place would be worse than the text box.
    it('saves the picked channel to the same field the id box wrote', async () => {
      withChannels();
      getLinkedServer.mockResolvedValue(server);
      getDiscordConfig.mockResolvedValue(emptyConfig);
      updateDiscordConfig.mockResolvedValue(emptyConfig);

      render(ServerPage);
      await waitFor(() =>
        expect(screen.getByRole('button', { name: '#announcements' })).toBeInTheDocument()
      );

      await fireEvent.click(screen.getByRole('button', { name: '#announcements' }));
      await fireEvent.click(screen.getByRole('button', { name: /Save/ }));

      await waitFor(() => expect(updateDiscordConfig).toHaveBeenCalled());
      expect(updateDiscordConfig).toHaveBeenCalledWith(
        expect.objectContaining({ announcements_channel_id: 'c2' })
      );
    });

    it('preselects whatever is already configured', async () => {
      withChannels();
      getLinkedServer.mockResolvedValue(server);
      getDiscordConfig.mockResolvedValue({ ...emptyConfig, announcements_channel_id: 'c1' });

      render(ServerPage);

      await waitFor(() =>
        expect(screen.getByRole('button', { name: '#general' })).toHaveAttribute(
          'aria-pressed',
          'true'
        )
      );
      expect(screen.getByRole('button', { name: '#lobby' })).toHaveAttribute(
        'aria-pressed',
        'false'
      );
    });

    // A deployment whose bot is offline still has to be configurable.
    it('falls back to the id field when Discord cannot be reached', async () => {
      getLinkedServer.mockResolvedValue(server);
      getDiscordConfig.mockResolvedValue(emptyConfig);

      render(ServerPage);

      await waitFor(() =>
        expect(screen.getByLabelText('Announcements channel')).toBeInTheDocument()
      );
      expect(screen.getByText(/Copy Channel ID/)).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: '#general' })).not.toBeInTheDocument();
    });
  });
});
