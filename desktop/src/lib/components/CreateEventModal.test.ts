import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { writable } from 'svelte/store';
import { dateUtils } from '$lib/utils/dateUtils';
import type { EventWithParticipants, FriendInfo } from '$lib/types';

const { getFriends, createEvent, updateEvent } = vi.hoisted(() => ({
  getFriends: vi.fn(),
  createEvent: vi.fn(),
  updateEvent: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { getFriends, createEvent, updateEvent }
}));

// The component reads $user for the default-visibility preselect. Mocked
// so the preselect is deterministic rather than depending on whatever the
// real store was last set to.
vi.mock('$lib/stores', () => ({
  user: writable({ default_visibility: 'friends' })
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

function makeEvent(overrides: Partial<EventWithParticipants> = {}): EventWithParticipants {
  return {
    id: 'e1',
    creator_id: 'me-id',
    title: 'Raclette night',
    description: 'bring cheese',
    start_time: '2026-03-01T19:00:00Z',
    end_time: '2026-03-01T22:00:00Z',
    location: 'Chez Lina',
    visibility: 'friends',
    price: '15',
    link: 'https://example.com',
    created_at: '2026-02-01T00:00:00Z',
    updated_at: '2026-02-01T00:00:00Z',
    is_participant: true,
    is_creator: true,
    my_status: 'accepted',
    participants: [],
    ...overrides
  };
}

describe('CreateEventModal invite picker', () => {
  beforeEach(() => {
    getFriends.mockReset();
    createEvent.mockReset();
    updateEvent.mockReset();
    createEvent.mockResolvedValue({});
    updateEvent.mockResolvedValue({});
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

// Edit mode: the same component doubles as the edit form, rather than a
// second form that would drift from this one. See
// .claude/skills/event-edit-flow/SKILL.md.
describe('CreateEventModal edit mode', () => {
  beforeEach(() => {
    getFriends.mockReset().mockResolvedValue([]);
    createEvent.mockReset().mockResolvedValue({});
    updateEvent.mockReset().mockResolvedValue({});
  });

  it('creates rather than updates when given no event', async () => {
    render(CreateEventModal, { props: { event: null } });

    expect(screen.getByText('Create New Event')).toBeInTheDocument();

    await fillRequiredFields();
    await fireEvent.click(screen.getByRole('button', { name: 'Create Event' }));

    await waitFor(() => expect(createEvent).toHaveBeenCalled());
    expect(updateEvent).not.toHaveBeenCalled();
  });

  it('prefills every field from the event being edited', () => {
    const event = makeEvent();
    render(CreateEventModal, { props: { event } });

    expect(screen.getByText('Edit event')).toBeInTheDocument();
    expect(screen.getByLabelText('Event Title *')).toHaveValue('Raclette night');
    expect(screen.getByLabelText('Location')).toHaveValue('Chez Lina');

    // The datetime-local input needs local `YYYY-MM-DDTHH:mm`, not the UTC
    // ISO string the API returns. Asserted via the same helper the
    // component uses, so this holds in any timezone the runner is in - and
    // an unparseable value would render blank rather than throwing, hence
    // the explicit non-empty check.
    expect(screen.getByLabelText('Start Time *')).toHaveValue(
      dateUtils.toDatetimeLocalValue(event.start_time)
    );
    expect(screen.getByLabelText('Start Time *')).not.toHaveValue('');
  });

  it('updates rather than creates when editing, and sends the event id', async () => {
    render(CreateEventModal, { props: { event: makeEvent() } });

    await fireEvent.input(screen.getByLabelText('Event Title *'), {
      target: { value: 'Raclette night (moved)' }
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(updateEvent).toHaveBeenCalled());
    expect(createEvent).not.toHaveBeenCalled();
    expect(updateEvent.mock.calls[0][0]).toBe('e1');
    expect(updateEvent.mock.calls[0][1]).toMatchObject({ title: 'Raclette night (moved)' });
  });

  it('dispatches saved once the update succeeds', async () => {
    const onSaved = vi.fn();
    render(CreateEventModal, {
      props: { event: makeEvent() },
      events: { saved: onSaved }
    });

    await fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onSaved).toHaveBeenCalledTimes(1));
  });

  it('shows an error inline when the update fails, and stays open', async () => {
    updateEvent.mockRejectedValue(new Error('server said no'));
    const onSaved = vi.fn();
    render(CreateEventModal, {
      props: { event: makeEvent() },
      events: { saved: onSaved }
    });

    await fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(screen.getByText('server said no')).toBeInTheDocument());
    expect(onSaved).not.toHaveBeenCalled();
  });
});
