import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { User } from '$lib/types';

const {
  getCurrentUser,
  updateProfile,
  deleteAccount,
  getCalendarFeedLink,
  rotateCalendarFeedLink,
  getExternalCalendars,
  connectExternalCalendar,
  disconnectExternalCalendar
} = vi.hoisted(() => ({
  getCurrentUser: vi.fn(),
  updateProfile: vi.fn(),
  deleteAccount: vi.fn(),
  getCalendarFeedLink: vi.fn(),
  rotateCalendarFeedLink: vi.fn(),
  getExternalCalendars: vi.fn(),
  connectExternalCalendar: vi.fn(),
  disconnectExternalCalendar: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: {
    getCurrentUser,
    updateProfile,
    deleteAccount,
    clearToken: vi.fn(),
    getToken: vi.fn(),
    getUnreadNotificationCount: vi.fn().mockResolvedValue(0),
    getCalendarFeedLink,
    rotateCalendarFeedLink,
    getExternalCalendars,
    connectExternalCalendar,
    disconnectExternalCalendar
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
    notify_event_reminders: true,
    notify_discord_dm: true,
    ...overrides
  };
}

describe('settings page', () => {
  beforeEach(() => {
    getCurrentUser.mockReset();
    updateProfile.mockReset();
    deleteAccount.mockReset();
    getExternalCalendars.mockReset();
    connectExternalCalendar.mockReset();
    disconnectExternalCalendar.mockReset();
    getExternalCalendars.mockResolvedValue([]);
  });

  it('loads and displays the current profile', async () => {
    getCurrentUser.mockResolvedValue(makeUser({ display_name: 'Me!' }));

    render(SettingsPage);

    await waitFor(() => expect(screen.getByLabelText('Display name')).toHaveValue('Me!'));
    expect(screen.getByLabelText('Timezone')).toHaveValue('UTC');
  });

  it('round-trips the event-reminder preference', async () => {
    getCurrentUser.mockResolvedValue(makeUser({ notify_event_reminders: true }));
    updateProfile.mockResolvedValue(makeUser({ notify_event_reminders: false }));

    render(SettingsPage);

    const toggle = await waitFor(() => screen.getByLabelText(/Event reminders/));
    expect(toggle).toBeChecked();

    await fireEvent.click(toggle);
    await fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() =>
      expect(updateProfile).toHaveBeenCalledWith(
        expect.objectContaining({ notify_event_reminders: false })
      )
    );
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
  describe('the calendar subscription link', () => {
    const FEED = 'http://localhost:8080/calendar/abc123.ics';

    async function openSettings() {
      getCurrentUser.mockResolvedValue(makeUser());
      render(SettingsPage);
      await waitFor(() => expect(screen.getByLabelText('Display name')).toBeInTheDocument());
    }

    // Lazy on purpose: asking for it mints a credential, so nobody who never
    // opens this row ends up with a live one.
    it('does not fetch a link until asked', async () => {
      await openSettings();

      expect(getCalendarFeedLink).not.toHaveBeenCalled();
      expect(screen.getByRole('button', { name: /Show my calendar link/ })).toBeInTheDocument();
    });

    it('shows the link and says the URL is the credential', async () => {
      getCalendarFeedLink.mockResolvedValue(FEED);
      await openSettings();

      await fireEvent.click(screen.getByRole('button', { name: /Show my calendar link/ }));

      await waitFor(() => expect(screen.getByText(FEED)).toBeInTheDocument());
      // getAllByText: the connected-calendars card warns about its own URL
      // in the same words, and both warnings are correct.
      expect(screen.getAllByText(/treat it like a password/).length).toBeGreaterThan(0);
      // The support question this heads off.
      expect(screen.getByText(/can take\s+several hours/)).toBeInTheDocument();
    });

    // Regenerating breaks every existing subscription, so it asks first.
    it('confirms before regenerating', async () => {
      getCalendarFeedLink.mockResolvedValue(FEED);
      rotateCalendarFeedLink.mockResolvedValue('http://localhost:8080/calendar/new.ics');
      await openSettings();
      await fireEvent.click(screen.getByRole('button', { name: /Show my calendar link/ }));
      await waitFor(() => expect(screen.getByText(FEED)).toBeInTheDocument());

      await fireEvent.click(screen.getByRole('button', { name: 'Regenerate' }));
      expect(rotateCalendarFeedLink).not.toHaveBeenCalled();

      await fireEvent.click(screen.getByRole('button', { name: /Yes, break existing/ }));

      await waitFor(() => expect(rotateCalendarFeedLink).toHaveBeenCalled());
      expect(await screen.findByText(/calendar\/new\.ics/)).toBeInTheDocument();
    });

    it('reports a failure instead of showing nothing', async () => {
      getCalendarFeedLink.mockRejectedValue(new Error('nope'));
      await openSettings();

      await fireEvent.click(screen.getByRole('button', { name: /Show my calendar link/ }));

      await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('nope'));
    });
  });
  describe('connected calendars', () => {
    async function openSettings() {
      getCurrentUser.mockResolvedValue(makeUser());
      render(SettingsPage);
      await waitFor(() => expect(screen.getByLabelText('Display name')).toBeInTheDocument());
    }

    it('explains what the link grants and what is kept', async () => {
      await openSettings();

      expect(screen.getByText(/treat it like a password/)).toBeInTheDocument();
      // The privacy promise, where the person pasting the URL can read it.
      expect(screen.getByText(/only busy times/)).toBeInTheDocument();
      expect(screen.getByText(/never event titles/)).toBeInTheDocument();
    });

    it('connects a calendar and lists it', async () => {
      connectExternalCalendar.mockResolvedValue([
        { id: 'c1', provider: 'ics', label: 'Work', last_synced_at: '2026-03-01T10:00:00Z' }
      ]);
      await openSettings();

      await fireEvent.input(screen.getByLabelText(/Secret calendar address/), {
        target: { value: 'https://example.com/a.ics' }
      });
      await fireEvent.input(screen.getByLabelText('Calendar label'), {
        target: { value: 'Work' }
      });
      await fireEvent.click(screen.getByRole('button', { name: 'Connect calendar' }));

      await waitFor(() =>
        expect(connectExternalCalendar).toHaveBeenCalledWith('https://example.com/a.ics', 'Work')
      );
      expect(await screen.findByText('Work')).toBeInTheDocument();
    });

    // ⚠️ A silently dead connection is worse than none: availability looks
    // right and isn't.
    it('shows why a calendar stopped syncing', async () => {
      getExternalCalendars.mockResolvedValue([
        { id: 'c1', provider: 'ics', label: 'Work', last_error: 'Calendar returned 404 Not Found' }
      ]);
      await openSettings();

      expect(await screen.findByText(/Not syncing/)).toBeInTheDocument();
      expect(screen.getByText(/404/)).toBeInTheDocument();
    });

    it('disconnects and drops it from the list', async () => {
      getExternalCalendars.mockResolvedValue([{ id: 'c1', provider: 'ics', label: 'Work' }]);
      disconnectExternalCalendar.mockResolvedValue(undefined);
      await openSettings();
      await waitFor(() => expect(screen.getByText('Work')).toBeInTheDocument());

      await fireEvent.click(screen.getByRole('button', { name: 'Disconnect' }));

      await waitFor(() => expect(disconnectExternalCalendar).toHaveBeenCalledWith('c1'));
      await waitFor(() => expect(screen.queryByText('Work')).not.toBeInTheDocument());
    });

    it('reports a rejected URL instead of failing quietly', async () => {
      connectExternalCalendar.mockRejectedValue(
        new Error('Only http(s) calendar links are supported, not file')
      );
      await openSettings();

      await fireEvent.input(screen.getByLabelText(/Secret calendar address/), {
        target: { value: 'file:///etc/passwd' }
      });
      await fireEvent.click(screen.getByRole('button', { name: 'Connect calendar' }));

      await waitFor(() =>
        expect(screen.getByRole('alert')).toHaveTextContent('Only http(s) calendar links')
      );
    });
  });
  describe('the timezone picker', () => {
    async function openSettings(profile = makeUser()) {
      getCurrentUser.mockResolvedValue(profile);
      render(SettingsPage);
      await waitFor(() => expect(screen.getByLabelText('Display name')).toBeInTheDocument());
    }

    it('is a select, not free text', async () => {
      await openSettings();

      const field = screen.getByLabelText('Timezone');
      expect(field.tagName).toBe('SELECT');
      // Built from the platform rather than a hardcoded list, so it should
      // be the full IANA set rather than a handful.
      expect((field as HTMLSelectElement).options.length).toBeGreaterThan(50);
    });

    it('preselects what the account has stored', async () => {
      await openSettings(makeUser({ timezone: 'Europe/Paris' }));

      expect(screen.getByLabelText('Timezone')).toHaveValue('Europe/Paris');
    });

    it('groups zones by region', async () => {
      await openSettings();

      const groups = document.querySelectorAll('optgroup');
      expect(groups.length).toBeGreaterThan(1);
      expect(Array.from(groups).map((g) => g.label)).toContain('Europe');
    });

    // ⚠️ This field used to be free text, so an existing account can hold
    // something that isn't an IANA zone. Dropping it would make the select
    // fall back to its first option and the next save would silently
    // rewrite the person's setting.
    it('keeps an unrecognised legacy value rather than silently replacing it', async () => {
      await openSettings(makeUser({ timezone: 'GMT+2' }));

      expect(screen.getByLabelText('Timezone')).toHaveValue('GMT+2');
      expect(screen.getByText(/Not a recognised time zone/)).toBeInTheDocument();
    });

    it('saves the stored value untouched when nothing is changed', async () => {
      updateProfile.mockResolvedValue(makeUser({ timezone: 'Europe/Paris' }));
      await openSettings(makeUser({ timezone: 'Europe/Paris' }));

      await fireEvent.click(screen.getByRole('button', { name: /Save/ }));

      await waitFor(() => expect(updateProfile).toHaveBeenCalled());
      expect(updateProfile.mock.calls[0][0]).toEqual(
        expect.objectContaining({ timezone: 'Europe/Paris' })
      );
    });

    // Nobody wants to scroll 400 entries to find where they're sitting. This
    // is also the only way to *change* the value under happy-dom, which
    // cannot drive a <select> - see CLAUDE.md.
    it('offers a one-tap shortcut to the detected zone', async () => {
      await openSettings(makeUser({ timezone: 'America/New_York' }));
      updateProfile.mockResolvedValue(makeUser());

      const detected = Intl.DateTimeFormat().resolvedOptions().timeZone;
      await fireEvent.click(
        screen.getByRole('button', { name: new RegExp(`Use ${detected.replace(/_/g, ' ')}`) })
      );
      await fireEvent.click(screen.getByRole('button', { name: /Save/ }));

      await waitFor(() => expect(updateProfile).toHaveBeenCalled());
      expect(updateProfile.mock.calls[0][0]).toEqual(
        expect.objectContaining({ timezone: detected })
      );
    });

    it('hides the shortcut when you are already on that zone', async () => {
      const detected = Intl.DateTimeFormat().resolvedOptions().timeZone;
      await openSettings(makeUser({ timezone: detected }));

      expect(screen.queryByRole('button', { name: /^Use / })).not.toBeInTheDocument();
    });
  });
});
