import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { FriendInfo } from '$lib/types';

const { getFriends, createEvent } = vi.hoisted(() => ({
  getFriends: vi.fn(),
  createEvent: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { getFriends, createEvent }
}));

const { default: CreateEventModal } = await import('./CreateEventModal.svelte');

const alice: FriendInfo = {
  user_id: 'alice-id',
  username: 'alice',
  avatar_url: undefined,
  synced_at: '2026-01-01T00:00:00Z'
};
const bob: FriendInfo = {
  user_id: 'bob-id',
  username: 'bob',
  avatar_url: undefined,
  synced_at: '2026-01-01T00:00:00Z'
};

async function fillRequiredFields() {
  await fireEvent.input(screen.getByLabelText('Event Title *'), { target: { value: 'Board games' } });
  await fireEvent.input(screen.getByLabelText('Start Time *'), { target: { value: '2026-03-01T19:00' } });
  await fireEvent.input(screen.getByLabelText('End Time *'), { target: { value: '2026-03-01T22:00' } });
}

describe('CreateEventModal invite picker', () => {
  beforeEach(() => {
    getFriends.mockReset();
    createEvent.mockReset();
    createEvent.mockResolvedValue({});
  });

  it('renders a chip per synced friend', async () => {
    getFriends.mockResolvedValue([alice, bob]);
    render(CreateEventModal);

    await waitFor(() => expect(screen.getByText('alice')).toBeInTheDocument());
    expect(screen.getByText('bob')).toBeInTheDocument();
  });

  it('shows an empty state when there are no friends to invite', async () => {
    getFriends.mockResolvedValue([]);
    render(CreateEventModal);

    await waitFor(() => expect(screen.getByText('No friends synced yet.')).toBeInTheDocument());
  });

  it('surfaces an error without blocking the rest of the form', async () => {
    getFriends.mockRejectedValue(new Error('backend down'));
    render(CreateEventModal);

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('backend down'));
    expect(screen.getByLabelText('Event Title *')).toBeInTheDocument();
  });

  it('omits participant_ids when nobody is selected', async () => {
    getFriends.mockResolvedValue([alice]);
    render(CreateEventModal);
    await waitFor(() => expect(screen.getByText('alice')).toBeInTheDocument());

    await fillRequiredFields();
    await fireEvent.click(screen.getByRole('button', { name: 'Create Event' }));

    await waitFor(() => expect(createEvent).toHaveBeenCalledOnce());
    expect(createEvent.mock.calls[0][0].participant_ids).toBeUndefined();
  });

  it('sends the selected friend ids as participant_ids, and toggling deselects', async () => {
    getFriends.mockResolvedValue([alice, bob]);
    render(CreateEventModal);
    await waitFor(() => expect(screen.getByText('alice')).toBeInTheDocument());

    const aliceChip = screen.getByRole('button', { name: /alice/ });
    const bobChip = screen.getByRole('button', { name: /bob/ });

    await fireEvent.click(aliceChip);
    await fireEvent.click(bobChip);
    expect(aliceChip).toHaveAttribute('aria-pressed', 'true');
    expect(bobChip).toHaveAttribute('aria-pressed', 'true');

    // deselect bob
    await fireEvent.click(bobChip);
    expect(bobChip).toHaveAttribute('aria-pressed', 'false');

    await fillRequiredFields();
    await fireEvent.click(screen.getByRole('button', { name: 'Create Event' }));

    await waitFor(() => expect(createEvent).toHaveBeenCalledOnce());
    expect(createEvent.mock.calls[0][0].participant_ids).toEqual(['alice-id']);
  });
});
