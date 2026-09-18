import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte';

const { getServers, registerServer } = vi.hoisted(() => ({
  getServers: vi.fn(),
  registerServer: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: {
    getServers,
    registerServer,
    getUnreadNotificationCount: vi.fn().mockResolvedValue(0),
    clearToken: vi.fn()
  }
}));

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/stores', () => ({
  page: {
    subscribe: (run: (value: { url: URL }) => void) => {
      run({ url: new URL('http://localhost/servers') });
      return () => {};
    }
  }
}));

const { default: ServersPage } = await import('./+page.svelte');

describe('servers page', () => {
  // Block body, not `() => getServers.mockReset()` - mockReset() returns the
  // mock, and vitest treats a function returned from beforeEach as a teardown
  // hook, so the concise form gets getServers *called* again after every test.
  beforeEach(() => {
    getServers.mockReset();
  });

  it('lists the servers the bot is in', async () => {
    getServers.mockResolvedValue({
      guilds: [
        { id: 'g1', discord_guild_id: '111', name: 'The Hangout', icon_url: undefined },
        { id: 'g2', discord_guild_id: '222', name: 'Board Club', icon_url: undefined }
      ],
      invite_url: 'https://discord.com/oauth2/authorize?client_id=abc'
    });

    render(ServersPage);

    await waitFor(() => expect(screen.getByText('The Hangout')).toBeInTheDocument());
    expect(screen.getByText('Board Club')).toBeInTheDocument();
  });

  it('offers the invite link as a real link, since authorising happens on Discord', async () => {
    getServers.mockResolvedValue({ guilds: [], invite_url: 'https://discord.com/invite-here' });

    render(ServersPage);

    const link = await screen.findByRole('link', { name: /Add the bot to a server/i });
    expect(link).toHaveAttribute('href', 'https://discord.com/invite-here');
  });

  it('explains an empty state rather than showing nothing', async () => {
    getServers.mockResolvedValue({ guilds: [], invite_url: 'https://invite' });

    render(ServersPage);

    await waitFor(() =>
      expect(screen.getByText(/bot isn't in any server yet/i)).toBeInTheDocument()
    );
  });

  // The gateway fills the name in on connect; until then all we have is the id.
  it('copes with a server registered by id before the bot has seen it', async () => {
    getServers.mockResolvedValue({
      guilds: [{ id: 'g1', discord_guild_id: '333', name: undefined, icon_url: undefined }],
      invite_url: 'https://invite'
    });

    render(ServersPage);

    await waitFor(() => expect(screen.getByText('Unnamed server')).toBeInTheDocument());
    expect(screen.getByText(/name appears once the bot reconnects/i)).toBeInTheDocument();
  });

  it('shows an error when the list fails to load', async () => {
    getServers.mockRejectedValue(new Error('network down'));

    render(ServersPage);

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('network down'));
  });
});

/**
 * Adding a server by id, from the page rather than the form component.
 *
 * ⚠️ The bug this exists for was in the *wiring*, not in either piece: the
 * page refreshed by calling `load()`, which sets `loading = true` and swaps
 * the whole branch out - unmounting `AddServerForm` and taking its "Added X"
 * confirmation with it the instant it appeared. Both components were fine
 * and all their own tests passed. Found by driving it in a browser.
 */
describe('servers page, adding by id', () => {
  beforeEach(() => {
    getServers.mockReset();
    registerServer.mockReset();
  });

  it('keeps the confirmation visible while the list refreshes', async () => {
    getServers.mockResolvedValue({ guilds: [], invite_url: 'https://discord.example/invite' });
    registerServer.mockResolvedValue({
      id: 'row-1',
      discord_guild_id: '123456789012345678',
      name: 'The Hangout',
      icon_url: undefined
    });

    render(ServersPage);
    await waitFor(() => expect(screen.getByLabelText('Discord server ID')).toBeInTheDocument());

    await fireEvent.input(screen.getByLabelText('Discord server ID'), {
      target: { value: '123456789012345678' }
    });
    await fireEvent.click(screen.getByRole('button', { name: /Add server/i }));

    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('The Hangout'));
    // And the list really was re-read, so the new server shows up.
    expect(getServers).toHaveBeenCalledTimes(2);
  });

  it('shows the newly added server in the list', async () => {
    getServers
      .mockResolvedValueOnce({ guilds: [], invite_url: 'https://discord.example/invite' })
      .mockResolvedValueOnce({
        guilds: [
          {
            id: 'row-1',
            discord_guild_id: '123456789012345678',
            name: 'The Hangout',
            icon_url: undefined
          }
        ],
        invite_url: 'https://discord.example/invite'
      });
    registerServer.mockResolvedValue({
      id: 'row-1',
      discord_guild_id: '123456789012345678',
      name: 'The Hangout',
      icon_url: undefined
    });

    render(ServersPage);
    await waitFor(() => expect(screen.getByLabelText('Discord server ID')).toBeInTheDocument());

    await fireEvent.input(screen.getByLabelText('Discord server ID'), {
      target: { value: '123456789012345678' }
    });
    await fireEvent.click(screen.getByRole('button', { name: /Add server/i }));

    await waitFor(() => expect(screen.getByText('The Hangout')).toBeInTheDocument());
  });
});
