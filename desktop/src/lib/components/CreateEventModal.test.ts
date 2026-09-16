import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/svelte';
import { writable } from 'svelte/store';
import { dateUtils } from '$lib/utils/dateUtils';
import type { EventWithParticipants, FriendInfo } from '$lib/types';

const { getFriends, createEvent, updateEvent, previewAnnouncement } = vi.hoisted(() => ({
  getFriends: vi.fn(),
  createEvent: vi.fn(),
  updateEvent: vi.fn(),
  previewAnnouncement: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { getFriends, createEvent, updateEvent, previewAnnouncement }
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
    reminder_leads: [60],
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

  async function toggleReminder(label: string) {
    await fireEvent.click(screen.getByRole('button', { name: `${label} before` }));
  }

  it('sends every reminder the creator picked', async () => {
    render(CreateEventModal, { props: { event: null } });

    await fillRequiredFields();
    // 1 hour is on by default; add a day and a week.
    await toggleReminder('1 day');
    await toggleReminder('1 week');
    await fireEvent.click(screen.getByRole('button', { name: 'Create Event' }));

    await waitFor(() => expect(createEvent).toHaveBeenCalled());
    // Ascending, so the payload doesn't depend on click order.
    expect(createEvent.mock.calls[0][0].reminder_leads).toEqual([60, 1440, 10080]);
  });

  it('treats nothing selected as no reminders', async () => {
    render(CreateEventModal, { props: { event: null } });

    await fillRequiredFields();
    await toggleReminder('1 hour'); // the default, off again
    await fireEvent.click(screen.getByRole('button', { name: 'Create Event' }));

    await waitFor(() => expect(createEvent).toHaveBeenCalled());
    // An explicit empty list, not an omitted field - the backend treats
    // silence as "give it the default", which is the opposite intent.
    expect(createEvent.mock.calls[0][0].reminder_leads).toEqual([]);
  });

  it('prefills every reminder when editing', () => {
    render(CreateEventModal, { props: { event: makeEvent({ reminder_leads: [60, 2880] }) } });

    expect(screen.getByRole('button', { name: '1 hour before' })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
    expect(screen.getByRole('button', { name: '2 days before' })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
    expect(screen.getByRole('button', { name: '1 week before' })).toHaveAttribute(
      'aria-pressed',
      'false'
    );
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

// Mobile two-step wizard. Component tests don't evaluate CSS breakpoints, so
// these assert the step *state machine* - which fields/controls are gated,
// what survives Back, and that the payload matches the desktop path. The
// "does it actually look like two steps at 402px" half is only checkable in
// a browser.
describe('CreateEventModal mobile wizard', () => {
  beforeEach(() => {
    getFriends.mockReset().mockResolvedValue([]);
    createEvent.mockReset().mockResolvedValue({});
    updateEvent.mockReset().mockResolvedValue({});
    previewAnnouncement.mockReset().mockResolvedValue('@everyone\nProposition...');
  });

  it('refuses to advance to step 2 while step 1 is invalid', async () => {
    render(CreateEventModal, { props: { event: null } });

    await fireEvent.click(screen.getByRole('button', { name: /Next · invite friends/ }));

    expect(screen.getByText('Please fill in all required fields')).toBeInTheDocument();
    expect(previewAnnouncement).not.toHaveBeenCalled();
    // Still on step 1, so the submit control hasn't appeared.
    expect(screen.queryByRole('button', { name: '‹ Back' })).not.toBeInTheDocument();
  });

  it('refuses to advance when the end time is before the start', async () => {
    render(CreateEventModal, { props: { event: null } });

    await fireEvent.input(screen.getByLabelText('Event Title *'), { target: { value: 'Pizza' } });
    await fireEvent.input(screen.getByLabelText('Start Time *'), {
      target: { value: '2026-03-01T22:00' }
    });
    await fireEvent.input(screen.getByLabelText('End Time *'), {
      target: { value: '2026-03-01T19:00' }
    });
    await fireEvent.click(screen.getByRole('button', { name: /Next · invite friends/ }));

    expect(screen.getByText('End time must be after start time')).toBeInTheDocument();
  });

  it('advances with valid input and loads the Discord preview', async () => {
    render(CreateEventModal, { props: { event: null } });

    await fillRequiredFields();
    await fireEvent.click(screen.getByRole('button', { name: /Next · invite friends/ }));

    await waitFor(() => expect(previewAnnouncement).toHaveBeenCalled());
    expect(screen.getByRole('button', { name: '‹ Back' })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText(/Proposition/)).toBeInTheDocument());
  });

  it('keeps step-1 values when going Back', async () => {
    render(CreateEventModal, { props: { event: null } });

    await fillRequiredFields();
    await fireEvent.click(screen.getByRole('button', { name: /Next · invite friends/ }));
    await fireEvent.click(await screen.findByRole('button', { name: '‹ Back' }));

    // Same bindings throughout - nothing is reset on step change.
    expect(screen.getByLabelText('Event Title *')).toHaveValue('Board games');
    expect(screen.getByLabelText('Start Time *')).toHaveValue('2026-03-01T19:00');
  });

  it('submits the same payload the desktop single-step path sends', async () => {
    render(CreateEventModal, { props: { event: null } });

    await fillRequiredFields();
    await fireEvent.click(screen.getByRole('button', { name: /Next · invite friends/ }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Create & post' }));

    await waitFor(() => expect(createEvent).toHaveBeenCalled());
    const viaWizard = createEvent.mock.calls[0][0];

    // Unmount before the second render: auto-cleanup only runs between
    // tests, and two mounted copies make every getByLabelText ambiguous.
    cleanup();
    createEvent.mockClear();
    render(CreateEventModal, { props: { event: null } });
    await fillRequiredFields();
    await fireEvent.click(screen.getAllByRole('button', { name: 'Create Event' })[0]);
    await waitFor(() => expect(createEvent).toHaveBeenCalled());

    expect(viaWizard).toEqual(createEvent.mock.calls[0][0]);
  });

  it('does not wizard the edit form - PUT manages neither invites nor the preview', async () => {
    render(CreateEventModal, { props: { event: makeEvent() } });

    expect(screen.queryByRole('button', { name: /Next · invite friends/ })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save changes' })).toBeInTheDocument();
  });
});
