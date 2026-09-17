import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { AnnouncementPostInfo } from '$lib/types';

const { adoptAnnouncement } = vi.hoisted(() => ({ adoptAnnouncement: vi.fn() }));

vi.mock('$lib/api', () => ({
  api: { adoptAnnouncement }
}));

const { default: AdoptEventModal } = await import('./AdoptEventModal.svelte');

// The bot's own template, matching
// services::discord_announcement::format_event_message.
const ANNOUNCEMENT = [
  '@everyone',
  "## Proposition d'activité :",
  '> Date : **<t:1795806000:F>**',
  '> Activité : **EsdeeKid**',
  "> Lieu : **L'Olympia**",
  '> Prix : **59e20 fosse**',
  '> Lien : [OKAY](https://www.ticketmaster.fr/x)',
  '',
  '**Réagissez avec ✅ pour participer !**'
].join('\n');

function makePost(overrides: Partial<AnnouncementPostInfo> = {}): AnnouncementPostInfo {
  return {
    id: 'post-1',
    author_username: 'Julioo',
    author_avatar_url: undefined,
    title: undefined,
    body: ANNOUNCEMENT,
    tag: 'general',
    reaction_count: 3,
    reply_count: 0,
    pinned: false,
    posted_at: '2026-03-01T12:00:00Z',
    ...overrides
  };
}

describe('AdoptEventModal', () => {
  beforeEach(() => {
    adoptAnnouncement.mockReset();
    adoptAnnouncement.mockResolvedValue({
      event: { id: 'e1' },
      rsvps_recorded: 2,
      backfill_failed: false
    });
  });

  it('prefills the form from what the announcement says', () => {
    render(AdoptEventModal, { props: { post: makePost() } });

    expect(screen.getByLabelText(/Title/)).toHaveValue('EsdeeKid');
    expect(screen.getByLabelText(/Location/)).toHaveValue("L'Olympia");
    expect(screen.getByLabelText(/Price/)).toHaveValue('59e20 fosse');
    expect(screen.getByLabelText(/Link/)).toHaveValue('https://www.ticketmaster.fr/x');
    // Both ends are filled: a calendar row needs an end time and an
    // announcement never carries one, so a default is offered rather than
    // leaving a required field empty.
    expect(screen.getByLabelText(/Starts/)).not.toHaveValue('');
    expect(screen.getByLabelText(/Ends/)).not.toHaveValue('');
  });

  it('sends what is in the form, not what was parsed', async () => {
    render(AdoptEventModal, { props: { post: makePost() } });

    await fireEvent.input(screen.getByLabelText(/Title/), {
      target: { value: 'EsdeeKid — Paris' }
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Add to calendar' }));

    await waitFor(() => expect(adoptAnnouncement).toHaveBeenCalled());
    const [id, payload] = adoptAnnouncement.mock.calls[0];
    expect(id).toBe('post-1');
    expect(payload.title).toBe('EsdeeKid — Paris');
    expect(payload.location).toBe("L'Olympia");
    // The server this publishes to is decided by where the message lives -
    // the client must not be asking for one.
    expect(payload).not.toHaveProperty('guild_ids');
  });

  it('reports how many RSVPs were recovered', async () => {
    const adopted = vi.fn();
    // `events` is a Svelte mount option, not a prop, so props go in the
    // explicit wrapper - see CLAUDE.md on the reserved mount-option names.
    render(AdoptEventModal, { props: { post: makePost() }, events: { adopted } });

    await fireEvent.click(screen.getByRole('button', { name: 'Add to calendar' }));

    await waitFor(() => expect(adopted).toHaveBeenCalledTimes(1));
    expect(adopted.mock.calls[0][0].detail).toEqual({ rsvps: 2 });
  });

  // The parse is a guess. When it comes up empty the form has to say so,
  // or an empty required field reads as the user having deleted something.
  it('says which fields it could not read out of the post', () => {
    render(AdoptEventModal, { props: { post: makePost({ body: 'on se voit bientôt' }) } });

    expect(screen.getByText(/couldn't read the date/)).toBeInTheDocument();
    expect(screen.getByLabelText(/Starts/)).toHaveValue('');
  });

  it('says nothing about missing fields when the post parsed cleanly', () => {
    render(AdoptEventModal, { props: { post: makePost() } });

    expect(screen.queryByText(/couldn't read/)).not.toBeInTheDocument();
  });

  it('refuses an end time that is not after the start', async () => {
    render(AdoptEventModal, { props: { post: makePost() } });

    await fireEvent.input(screen.getByLabelText(/Ends/), {
      target: { value: '2020-01-01T10:00' }
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Add to calendar' }));

    expect(screen.getByText('End time must be after start time')).toBeInTheDocument();
    expect(adoptAnnouncement).not.toHaveBeenCalled();
  });

  // A post already adopted from another device, say. The backend refuses it,
  // and the modal has to stay open with the reason rather than closing as
  // though it worked.
  it('keeps the form open and shows why when the server refuses', async () => {
    adoptAnnouncement.mockRejectedValue(
      new Error('That announcement is already on the calendar as an event')
    );
    const adopted = vi.fn();
    render(AdoptEventModal, { props: { post: makePost() }, events: { adopted } });

    await fireEvent.click(screen.getByRole('button', { name: 'Add to calendar' }));

    await waitFor(() => expect(screen.getByText(/already on the calendar/)).toBeInTheDocument());
    expect(adopted).not.toHaveBeenCalled();
    expect(screen.getByLabelText(/Title/)).toHaveValue('EsdeeKid');
  });

  it('closes without adopting when cancelled', async () => {
    const close = vi.fn();
    render(AdoptEventModal, { props: { post: makePost() }, events: { close } });

    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    expect(close).toHaveBeenCalled();
    expect(adoptAnnouncement).not.toHaveBeenCalled();
  });
});
