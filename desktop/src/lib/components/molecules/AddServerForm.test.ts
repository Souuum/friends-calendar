import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import AddServerForm from './AddServerForm.svelte';

const { registerServer } = vi.hoisted(() => ({ registerServer: vi.fn() }));
vi.mock('$lib/api', () => ({ api: { registerServer } }));

const GUILD = {
  id: 'row-1',
  discord_guild_id: '123456789012345678',
  name: 'The Hangout',
  icon_url: undefined
};

describe('AddServerForm', () => {
  // ⚠️ Block body, not the concise arrow: vitest treats a function *returned*
  // from beforeEach as a teardown hook, and mockReset() returns the mock - so
  // the concise form calls the mock after every test. See CLAUDE.md.
  beforeEach(() => {
    registerServer.mockReset();
  });

  it('sends the id that was typed', async () => {
    registerServer.mockResolvedValue(GUILD);
    render(AddServerForm);

    await fireEvent.input(screen.getByLabelText('Discord server ID'), {
      target: { value: '123456789012345678' }
    });
    await fireEvent.click(screen.getByRole('button', { name: /Add server/i }));

    await waitFor(() => expect(registerServer).toHaveBeenCalledWith('123456789012345678'));
  });

  it('confirms with the name Discord gave back', async () => {
    registerServer.mockResolvedValue(GUILD);
    render(AddServerForm);

    await fireEvent.input(screen.getByLabelText('Discord server ID'), {
      target: { value: '123456789012345678' }
    });
    await fireEvent.click(screen.getByRole('button', { name: /Add server/i }));

    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('The Hangout'));
  });

  it('clears the field after a success so the next one starts empty', async () => {
    registerServer.mockResolvedValue(GUILD);
    render(AddServerForm);

    const field = screen.getByLabelText('Discord server ID') as HTMLInputElement;
    await fireEvent.input(field, { target: { value: '123456789012345678' } });
    await fireEvent.click(screen.getByRole('button', { name: /Add server/i }));

    await waitFor(() => expect(field.value).toBe(''));
  });

  // ⚠️ The server's message is the useful one - "the bot isn't in that server
  // yet" tells you what to do, where a generic failure doesn't.
  it('shows the reason the server gave', async () => {
    registerServer.mockRejectedValue(new Error("The bot isn't in that server yet"));
    render(AddServerForm);

    await fireEvent.input(screen.getByLabelText('Discord server ID'), {
      target: { value: '123456789012345678' }
    });
    await fireEvent.click(screen.getByRole('button', { name: /Add server/i }));

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent("The bot isn't in that server yet")
    );
  });

  // The commonest cause of that failure is never having invited the bot, so
  // the way out sits with the message.
  it('offers the invite link alongside a failure', async () => {
    registerServer.mockRejectedValue(new Error('nope'));
    render(AddServerForm, { props: { inviteUrl: 'https://discord.com/oauth2/authorize?x=1' } });

    await fireEvent.input(screen.getByLabelText('Discord server ID'), {
      target: { value: '123456789012345678' }
    });
    await fireEvent.click(screen.getByRole('button', { name: /Add server/i }));

    await waitFor(() =>
      expect(screen.getByRole('link', { name: /Invite the bot/i })).toHaveAttribute(
        'href',
        'https://discord.com/oauth2/authorize?x=1'
      )
    );
  });

  // ⚠️ happy-dom implements no HTML5 constraint validation, so a `required`
  // attribute would do nothing here *and* nothing visible in a real browser
  // either (the native tooltip bypasses the error box). The form validates
  // itself, and this asserts it rather than the attribute.
  it('refuses an empty submit in-page instead of calling the API', async () => {
    render(AddServerForm);

    await fireEvent.click(screen.getByRole('button', { name: /Add server/i }));

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
    expect(registerServer).not.toHaveBeenCalled();
  });

  it('refuses whitespace the same way', async () => {
    render(AddServerForm);

    await fireEvent.input(screen.getByLabelText('Discord server ID'), { target: { value: '   ' } });
    await fireEvent.click(screen.getByRole('button', { name: /Add server/i }));

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
    expect(registerServer).not.toHaveBeenCalled();
  });

  it('tells the page so it can refresh the list', async () => {
    registerServer.mockResolvedValue(GUILD);
    const onRegistered = vi.fn();
    // Svelte 5 dropped `component.$on`; the mount options carry listeners.
    render(AddServerForm, { events: { registered: onRegistered } });

    await fireEvent.input(screen.getByLabelText('Discord server ID'), {
      target: { value: '123456789012345678' }
    });
    await fireEvent.click(screen.getByRole('button', { name: /Add server/i }));

    await waitFor(() => expect(onRegistered).toHaveBeenCalledTimes(1));
    expect(onRegistered.mock.calls[0][0].detail.guild.name).toBe('The Hangout');
  });
});
