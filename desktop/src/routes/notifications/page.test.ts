import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { unreadNotificationCount } from '$lib/stores';
import type { NotificationInfo } from '$lib/types';

const {
  getNotifications,
  markNotificationRead,
  markAllNotificationsRead,
  getUnreadNotificationCount,
  updateParticipation
} = vi.hoisted(() => ({
  getNotifications: vi.fn(),
  markNotificationRead: vi.fn(),
  markAllNotificationsRead: vi.fn(),
  getUnreadNotificationCount: vi.fn(),
  updateParticipation: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: {
    getNotifications,
    markNotificationRead,
    markAllNotificationsRead,
    getUnreadNotificationCount,
    updateParticipation,
    clearToken: vi.fn()
  }
}));

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

vi.mock('$app/stores', () => ({
  page: {
    subscribe: (run: (value: { url: URL }) => void) => {
      run({ url: new URL('http://localhost/notifications') });
      return () => {};
    }
  }
}));

const { default: NotificationsPage } = await import('./+page.svelte');

function notification(overrides: Partial<NotificationInfo>): NotificationInfo {
  return {
    id: 'n1',
    kind: 'event_invite',
    actor_username: 'alice',
    actor_avatar_url: undefined,
    event_id: undefined,
    message: 'alice invited you to Raclette night',
    read: false,
    created_at: '2026-01-01T12:00:00Z',
    ...overrides
  };
}

describe('notifications page', () => {
  beforeEach(() => {
    getNotifications.mockReset();
    markNotificationRead.mockReset();
    markAllNotificationsRead.mockReset();
    getUnreadNotificationCount.mockReset();
    updateParticipation.mockReset();
    unreadNotificationCount.set(0);
  });

  it('lists notifications and shows the unread count', async () => {
    getNotifications.mockResolvedValue([
      notification({ id: 'n1', read: false }),
      notification({ id: 'n2', read: true, message: 'bob accepted your event' })
    ]);

    render(NotificationsPage);

    await waitFor(() =>
      expect(screen.getByText('alice invited you to Raclette night')).toBeInTheDocument()
    );
    expect(screen.getByText('bob accepted your event')).toBeInTheDocument();
    expect(screen.getByText('1 unread')).toBeInTheDocument();
  });

  it('shows an empty state when there are no notifications', async () => {
    getNotifications.mockResolvedValue([]);
    render(NotificationsPage);

    await waitFor(() => expect(screen.getByText('Nothing yet.')).toBeInTheDocument());
  });

  it('marks a single notification read on click and refreshes the shared unread count', async () => {
    getNotifications.mockResolvedValue([notification({ id: 'n1', read: false })]);
    markNotificationRead.mockResolvedValue(undefined);
    getUnreadNotificationCount.mockResolvedValue(0);

    render(NotificationsPage);
    await waitFor(() => expect(screen.getByText('1 unread')).toBeInTheDocument());

    await fireEvent.click(screen.getByText('alice invited you to Raclette night'));

    await waitFor(() => expect(markNotificationRead).toHaveBeenCalledWith('n1'));
    await waitFor(() => expect(get(unreadNotificationCount)).toBe(0));
    expect(screen.getByText('0 unread')).toBeInTheDocument();
  });

  it('marks all read', async () => {
    getNotifications.mockResolvedValue([
      notification({ id: 'n1', read: false }),
      notification({ id: 'n2', read: false })
    ]);
    markAllNotificationsRead.mockResolvedValue(undefined);

    render(NotificationsPage);
    await waitFor(() => expect(screen.getByText('2 unread')).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Mark all read' }));

    await waitFor(() => expect(markAllNotificationsRead).toHaveBeenCalledOnce());
    expect(screen.getByText('0 unread')).toBeInTheDocument();
  });

  it('shows an error when loading fails', async () => {
    getNotifications.mockRejectedValue(new Error('network down'));
    render(NotificationsPage);

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('network down'));
  });

  it('offers inline RSVP only on event invites that still point at an event', async () => {
    getNotifications.mockResolvedValue([
      notification({ id: 'n1', kind: 'event_invite', event_id: 'e1' }),
      notification({
        id: 'n2',
        kind: 'friend_request',
        event_id: undefined,
        message: 'bob wants to be friends'
      }),
      // An invite whose event_id never made it onto the row has nothing to
      // answer - offering buttons here would 404 on click.
      notification({ id: 'n3', kind: 'event_invite', event_id: undefined, message: 'older invite' })
    ]);

    render(NotificationsPage);

    await waitFor(() =>
      expect(screen.getByText('alice invited you to Raclette night')).toBeInTheDocument()
    );
    expect(screen.getAllByRole('button', { name: 'Going' })).toHaveLength(1);
  });

  it('answers the invite and marks it read in one tap', async () => {
    getNotifications.mockResolvedValue([notification({ id: 'n1', event_id: 'e1', read: false })]);
    updateParticipation.mockResolvedValue(undefined);
    markNotificationRead.mockResolvedValue(undefined);
    getUnreadNotificationCount.mockResolvedValue(0);

    render(NotificationsPage);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Going' })).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Going' }));

    await waitFor(() => expect(updateParticipation).toHaveBeenCalledWith('e1', 'accepted'));
    // Acting on a notification is acknowledgement - no second tap needed.
    await waitFor(() => expect(markNotificationRead).toHaveBeenCalledWith('n1'));
    expect(screen.getByText('0 unread')).toBeInTheDocument();
  });

  it("sends the right status for Maybe and Can't", async () => {
    getNotifications.mockResolvedValue([notification({ id: 'n1', event_id: 'e1' })]);
    updateParticipation.mockResolvedValue(undefined);
    markNotificationRead.mockResolvedValue(undefined);
    getUnreadNotificationCount.mockResolvedValue(0);

    render(NotificationsPage);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Maybe' })).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Maybe' }));
    await waitFor(() => expect(updateParticipation).toHaveBeenCalledWith('e1', 'maybe'));

    await fireEvent.click(screen.getByRole('button', { name: "Can't" }));
    await waitFor(() => expect(updateParticipation).toHaveBeenCalledWith('e1', 'declined'));
  });

  // The event may have been deleted, or you removed from it, since the
  // notification was written.
  it('keeps the rest of the list when one RSVP fails', async () => {
    getNotifications.mockResolvedValue([
      notification({ id: 'n1', event_id: 'e1' }),
      notification({ id: 'n2', event_id: 'e2', message: 'bob invited you to Cinema' })
    ]);
    updateParticipation.mockRejectedValue(new Error('event is gone'));

    render(NotificationsPage);
    await waitFor(() => expect(screen.getAllByRole('button', { name: 'Going' })).toHaveLength(2));

    await fireEvent.click(screen.getAllByRole('button', { name: 'Going' })[0]);

    await waitFor(() => expect(screen.getByText('event is gone')).toBeInTheDocument());
    expect(screen.getByText('bob invited you to Cinema')).toBeInTheDocument();
    expect(screen.getByText('alice invited you to Raclette night')).toBeInTheDocument();
  });

  it('groups notifications by recency', async () => {
    const today = new Date();
    today.setHours(9, 0, 0, 0);
    const older = new Date();
    older.setDate(older.getDate() - 4);

    getNotifications.mockResolvedValue([
      notification({ id: 'n1', created_at: today.toISOString() }),
      notification({ id: 'n2', created_at: older.toISOString(), message: 'an older one' })
    ]);

    render(NotificationsPage);

    await waitFor(() => expect(screen.getByText('Today')).toBeInTheDocument());
    expect(screen.getByText('Earlier')).toBeInTheDocument();
  });
});

/**
 * Showing that an invite has been answered.
 *
 * ⚠️ Reported as "accept or deny event modal seems bugged / notification
 * stays even after the response is send". Nothing was failing: the RSVP went
 * through and the row was marked read, but `canRsvp` only checked the kind
 * and `event_id`, so the three buttons rendered identically before and
 * after. The only visible change was the unread dot, and on reload even that
 * came back looking unanswered because the notification carried no status.
 */
describe('notifications page, answered invites', () => {
  beforeEach(() => {
    getNotifications.mockReset();
    markNotificationRead.mockReset();
    markAllNotificationsRead.mockReset();
    getUnreadNotificationCount.mockReset();
    updateParticipation.mockReset();
    getUnreadNotificationCount.mockResolvedValue(0);
    markNotificationRead.mockResolvedValue(undefined);
    unreadNotificationCount.set(0);
  });

  it('says what you answered, straight from the server', async () => {
    getNotifications.mockResolvedValue([
      notification({ event_id: 'e1', my_status: 'accepted', read: true })
    ]);

    render(NotificationsPage);

    await waitFor(() =>
      expect(screen.getByTestId('rsvp-answer')).toHaveTextContent("You're going")
    );
  });

  // The half that survives a reload - this is what `my_status` is for.
  it('marks the answer on the control itself', async () => {
    getNotifications.mockResolvedValue([
      notification({ event_id: 'e1', my_status: 'declined', read: true })
    ]);

    render(NotificationsPage);

    await waitFor(() =>
      expect(screen.getByRole('button', { name: "Can't" })).toHaveAttribute('aria-pressed', 'true')
    );
    expect(screen.getByRole('button', { name: 'Going' })).toHaveAttribute('aria-pressed', 'false');
  });

  // ⚠️ `pending` is an answer nobody gave. It has to read as still open, or
  // every unanswered invite would claim you had replied.
  it('treats pending as unanswered', async () => {
    getNotifications.mockResolvedValue([
      notification({ event_id: 'e1', my_status: 'pending', read: false })
    ]);

    render(NotificationsPage);

    await waitFor(() => expect(screen.getByRole('button', { name: 'Going' })).toBeInTheDocument());
    expect(screen.queryByTestId('rsvp-answer')).not.toBeInTheDocument();
  });

  // The half that makes the tap feel like it did something, before any
  // refetch: the card updates from the answer just sent.
  it('updates the card immediately after answering', async () => {
    getNotifications.mockResolvedValue([
      notification({ event_id: 'e1', my_status: 'pending', read: false })
    ]);
    updateParticipation.mockResolvedValue(undefined);

    render(NotificationsPage);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Maybe' })).toBeInTheDocument());
    await fireEvent.click(screen.getByRole('button', { name: 'Maybe' }));

    await waitFor(() =>
      expect(screen.getByTestId('rsvp-answer')).toHaveTextContent('You said maybe')
    );
    expect(screen.getByRole('button', { name: 'Maybe' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('lets you change your mind', async () => {
    getNotifications.mockResolvedValue([
      notification({ event_id: 'e1', my_status: 'accepted', read: true })
    ]);
    updateParticipation.mockResolvedValue(undefined);

    render(NotificationsPage);
    await waitFor(() =>
      expect(screen.getByTestId('rsvp-answer')).toHaveTextContent("You're going")
    );

    await fireEvent.click(screen.getByRole('button', { name: "Can't" }));

    await waitFor(() =>
      expect(screen.getByTestId('rsvp-answer')).toHaveTextContent("You can't make it")
    );
    expect(updateParticipation).toHaveBeenLastCalledWith('e1', 'declined');
  });

  /**
   * ⚠️ `aria-pressed` alone was not enough. "Going" is the primary call to
   * action and was filled purple unconditionally, so after choosing "Can't"
   * the filled emphasis still sat on Going - the card announced one answer
   * and looked like another. Caught by looking at a screenshot; the
   * attribute assertions above all passed while it was wrong.
   */
  it('moves the filled emphasis onto the answer that was chosen', async () => {
    getNotifications.mockResolvedValue([
      notification({ event_id: 'e1', my_status: 'declined', read: true })
    ]);

    render(NotificationsPage);

    await waitFor(() => expect(screen.getByTestId('rsvp-answer')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Going' }).className).not.toContain('bg-primary');
  });

  it('keeps Going filled while the invite is still open', async () => {
    getNotifications.mockResolvedValue([
      notification({ event_id: 'e1', my_status: 'pending', read: false })
    ]);

    render(NotificationsPage);

    await waitFor(() => expect(screen.getByRole('button', { name: 'Going' })).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Going' }).className).toContain('bg-primary');
  });

  // A failed answer must not claim you answered.
  it('does not record an answer the server refused', async () => {
    getNotifications.mockResolvedValue([
      notification({ event_id: 'e1', my_status: 'pending', read: false })
    ]);
    updateParticipation.mockRejectedValue(new Error('Event not found'));

    render(NotificationsPage);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Going' })).toBeInTheDocument());
    await fireEvent.click(screen.getByRole('button', { name: 'Going' }));

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('Event not found'));
    expect(screen.queryByTestId('rsvp-answer')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Going' })).toHaveAttribute('aria-pressed', 'false');
  });
});
