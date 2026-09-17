import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import EventPeekPanel from './EventPeekPanel.svelte';
import type { EventWithParticipants } from '$lib/types';

const { updateParticipation, deleteEvent, nudgeNoAnswers } = vi.hoisted(() => ({
  updateParticipation: vi.fn(),
  deleteEvent: vi.fn(),
  nudgeNoAnswers: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { updateParticipation, deleteEvent, nudgeNoAnswers }
}));

function makeEvent(overrides: Partial<EventWithParticipants> = {}): EventWithParticipants {
  return {
    id: 'e1',
    creator_id: 'other-id',
    title: 'Raclette night',
    start_time: '2026-03-01T19:00:00Z',
    end_time: '2026-03-01T22:00:00Z',
    location: 'Chez Lina',
    visibility: 'friends',
    created_at: '2026-02-01T00:00:00Z',
    updated_at: '2026-02-01T00:00:00Z',
    reminder_leads: [60],
    is_participant: true,
    is_creator: false,
    my_status: 'pending',
    participants: [
      { user_id: 'me-id', username: 'me', status: 'pending' },
      { user_id: 'bob-id', username: 'bob', status: 'accepted' }
    ],
    ...overrides
  };
}

describe('EventPeekPanel', () => {
  beforeEach(() => {
    updateParticipation.mockReset();
    deleteEvent.mockReset();
  });

  it('shows an empty state when nothing is selected', () => {
    render(EventPeekPanel, { event: null });
    expect(screen.getByText('Select an event to see its details here.')).toBeInTheDocument();
  });

  it('renders the selected event and its participants', () => {
    render(EventPeekPanel, { event: makeEvent() });

    expect(screen.getByText('Raclette night')).toBeInTheDocument();
    expect(screen.getByText('bob')).toBeInTheDocument();
    expect(screen.getByText('2 invited')).toBeInTheDocument();
  });

  it('shows RSVP buttons for events you did not create, but not your own', () => {
    const { rerender } = render(EventPeekPanel, { event: makeEvent({ is_creator: false }) });
    expect(screen.getByRole('button', { name: 'Going' })).toBeInTheDocument();

    rerender({ event: makeEvent({ is_creator: true }) });
    expect(screen.queryByRole('button', { name: 'Going' })).not.toBeInTheDocument();
  });

  it('offers no RSVP on an event you can see but were never invited to', () => {
    render(EventPeekPanel, {
      event: makeEvent({ is_creator: false, is_participant: false, my_status: undefined })
    });

    // my_status alone can't distinguish this from an unanswered invite -
    // both are absent - which is why is_participant exists.
    expect(screen.queryByRole('button', { name: 'Going' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Maybe' })).not.toBeInTheDocument();
    expect(screen.getByText(/not invited to this one/i)).toBeInTheDocument();
  });

  it('explains why a discovered event is visible, per its visibility', () => {
    const { rerender } = render(EventPeekPanel, {
      event: makeEvent({ is_creator: false, is_participant: false, visibility: 'public' })
    });
    expect(screen.getByText(/it's public/i)).toBeInTheDocument();

    rerender({
      event: makeEvent({ is_creator: false, is_participant: false, visibility: 'friends' })
    });
    expect(screen.getByText(/friends with the organiser/i)).toBeInTheDocument();
  });

  it('shows Edit/Nudge instead of RSVP buttons for your own event', () => {
    render(EventPeekPanel, { event: makeEvent({ is_creator: true }) });

    // Both are live now - Nudge was a placeholder until the endpoint
    // existed, and this assertion is what pinned that.
    expect(screen.getByRole('button', { name: 'Edit' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Nudge no-answers' })).toBeEnabled();
    expect(screen.queryByRole('button', { name: 'Maybe' })).not.toBeInTheDocument();
  });

  it('dispatches the event to edit when Edit is clicked', async () => {
    const onEdit = vi.fn();
    // `events` is a Svelte mount option, not a prop, so props have to go in
    // the explicit `props` wrapper here - see CLAUDE.md's note on the
    // reserved mount-option names. (component.$on() is gone in Svelte 5.)
    render(EventPeekPanel, {
      props: { event: makeEvent({ is_creator: true }) },
      events: { edit: onEdit }
    });

    await fireEvent.click(screen.getByRole('button', { name: 'Edit' }));

    expect(onEdit).toHaveBeenCalledTimes(1);
    expect(onEdit.mock.calls[0][0].detail).toMatchObject({ id: 'e1' });
  });

  it('requires a second click to delete, and only offers it on your own event', async () => {
    const { rerender } = render(EventPeekPanel, { event: makeEvent({ is_creator: false }) });
    expect(screen.queryByRole('button', { name: 'Delete event' })).not.toBeInTheDocument();

    await rerender({ event: makeEvent({ is_creator: true }) });
    await fireEvent.click(screen.getByRole('button', { name: 'Delete event' }));

    // First click only arms the confirm - nothing has been deleted yet.
    expect(deleteEvent).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByRole('button', { name: 'Really delete' }));
    await waitFor(() => expect(deleteEvent).toHaveBeenCalledWith('e1'));
  });

  it('surfaces a delete failure inline instead of leaving the panel silent', async () => {
    deleteEvent.mockRejectedValue(new Error('nope'));
    render(EventPeekPanel, { event: makeEvent({ is_creator: true }) });

    await fireEvent.click(screen.getByRole('button', { name: 'Delete event' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Really delete' }));

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('nope'));
  });

  it('shows the mockup status labels, not the raw enum values', () => {
    render(EventPeekPanel, {
      event: makeEvent({
        my_status: 'pending',
        participants: [
          { user_id: 'me-id', username: 'me', status: 'pending' },
          { user_id: 'bob-id', username: 'bob', status: 'accepted' }
        ]
      })
    });

    expect(screen.getAllByText('No answer').length).toBeGreaterThan(0);
    expect(screen.getByText('Going', { selector: 'span' })).toBeInTheDocument();
    expect(screen.queryByText('pending')).not.toBeInTheDocument();
    expect(screen.queryByText('accepted')).not.toBeInTheDocument();
  });

  it('updates participation on RSVP click', async () => {
    updateParticipation.mockResolvedValue(undefined);
    render(EventPeekPanel, { event: makeEvent() });

    await fireEvent.click(screen.getByRole('button', { name: 'Going' }));

    await waitFor(() => expect(updateParticipation).toHaveBeenCalledWith('e1', 'accepted'));
  });

  it('shows an error without crashing when the RSVP call fails', async () => {
    updateParticipation.mockRejectedValue(new Error('network down'));
    render(EventPeekPanel, { event: makeEvent() });

    await fireEvent.click(screen.getByRole('button', { name: 'Going' }));

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('network down'));
  });
  describe('nudging the no-answers', () => {
    function withPending(count: number) {
      return makeEvent({
        is_creator: true,
        participants: [
          { user_id: 'me-id', username: 'me', status: 'accepted' },
          ...Array.from({ length: count }, (_, i) => ({
            user_id: `p${i}`,
            username: `person${i}`,
            status: 'pending' as const
          }))
        ]
      });
    }

    it('nudges and reports how many were reminded', async () => {
      nudgeNoAnswers.mockResolvedValue({ nudged: 2, discord_failed: false });
      render(EventPeekPanel, { props: { event: withPending(2) } });

      await fireEvent.click(screen.getByRole('button', { name: 'Nudge no-answers' }));

      await waitFor(() =>
        expect(screen.getByRole('status')).toHaveTextContent('Reminded 2 people')
      );
      expect(nudgeNoAnswers).toHaveBeenCalledWith('e1');
    });

    // Enabled-and-silently-doing-nothing is what it did for the whole time
    // it was a placeholder; the reason belongs on the button.
    it('is disabled with a reason when everyone has answered', () => {
      render(EventPeekPanel, { props: { event: withPending(0) } });

      const button = screen.getByRole('button', { name: 'Nudge no-answers' });
      expect(button).toBeDisabled();
      expect(button).toHaveAttribute('title', 'Everyone has answered');
    });

    // The rate-limit response names when the next one is allowed, which is
    // more use than "failed".
    it('shows why the server refused', async () => {
      nudgeNoAnswers.mockRejectedValue(
        new Error('Already nudged recently - you can nudge again after 09:00 on 2 Mar')
      );
      render(EventPeekPanel, { props: { event: withPending(1) } });

      await fireEvent.click(screen.getByRole('button', { name: 'Nudge no-answers' }));

      await waitFor(() =>
        expect(screen.getByRole('alert')).toHaveTextContent('you can nudge again after')
      );
    });

    it('says when the in-app half worked but Discord did not', async () => {
      nudgeNoAnswers.mockResolvedValue({ nudged: 1, discord_failed: true });
      render(EventPeekPanel, { props: { event: withPending(1) } });

      await fireEvent.click(screen.getByRole('button', { name: 'Nudge no-answers' }));

      await waitFor(() =>
        expect(screen.getByRole('status')).toHaveTextContent("Couldn't post in the Discord thread")
      );
    });

    it('is not offered on an event you did not create', () => {
      render(EventPeekPanel, { props: { event: makeEvent({ is_creator: false }) } });

      expect(screen.queryByRole('button', { name: 'Nudge no-answers' })).not.toBeInTheDocument();
    });
  });
});
