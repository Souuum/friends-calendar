import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import EventPeekPanel from './EventPeekPanel.svelte';
import type { EventWithParticipants } from '$lib/types';

const { updateParticipation } = vi.hoisted(() => ({
  updateParticipation: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { updateParticipation }
}));

function makeEvent(overrides: Partial<EventWithParticipants> = {}): EventWithParticipants {
  return {
    id: 'e1',
    creator_id: 'other-id',
    title: 'Raclette night',
    start_time: '2026-03-01T19:00:00Z',
    end_time: '2026-03-01T22:00:00Z',
    location: "Chez Lina",
    visibility: 'friends',
    created_at: '2026-02-01T00:00:00Z',
    updated_at: '2026-02-01T00:00:00Z',
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

  it('shows Edit/Nudge instead of RSVP buttons for your own event', () => {
    render(EventPeekPanel, { event: makeEvent({ is_creator: true }) });

    expect(screen.getByRole('button', { name: 'Edit' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Nudge no-answers' })).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'Maybe' })).not.toBeInTheDocument();
  });

  it('shows the mockup status labels, not the raw enum values', () => {
    render(
      EventPeekPanel,
      {
        event: makeEvent({
          my_status: 'pending',
          participants: [
            { user_id: 'me-id', username: 'me', status: 'pending' },
            { user_id: 'bob-id', username: 'bob', status: 'accepted' }
          ]
        })
      }
    );

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
});
