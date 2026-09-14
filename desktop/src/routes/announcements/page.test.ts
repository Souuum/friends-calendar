import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import type { EventWithParticipants } from '$lib/types';

const { getEvents } = vi.hoisted(() => ({
  getEvents: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { getEvents, clearToken: vi.fn(), getToken: vi.fn() }
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
      run({ url: new URL('http://localhost/announcements') });
      return () => {};
    }
  }
}));

const { default: AnnouncementsPage } = await import('./+page.svelte');

function makeEvent(overrides: Partial<EventWithParticipants>): EventWithParticipants {
  return {
    id: overrides.id ?? 'e1',
    creator_id: 'me-id',
    title: overrides.title ?? 'Untitled',
    start_time: '2026-03-01T19:00:00Z',
    end_time: '2026-03-01T22:00:00Z',
    visibility: 'friends',
    created_at: '2026-02-01T00:00:00Z',
    updated_at: '2026-02-01T00:00:00Z',
    is_creator: true,
    my_status: 'accepted',
    participants: [{ user_id: 'me-id', username: 'me', status: 'accepted' }],
    ...overrides
  };
}

describe('announcements page', () => {
  beforeEach(() => {
    getEvents.mockReset();
  });

  it('only lists events that were actually announced to Discord', async () => {
    getEvents.mockResolvedValue([
      makeEvent({ id: 'announced', title: 'Announced Party', discord_message_id: 'msg1' }),
      makeEvent({ id: 'not-announced', title: 'Private Draft', discord_message_id: undefined })
    ]);

    render(AnnouncementsPage);

    await waitFor(() => expect(screen.getByText('Announced Party')).toBeInTheDocument());
    expect(screen.queryByText('Private Draft')).not.toBeInTheDocument();
  });

  it('shows an empty state when nothing has been announced', async () => {
    getEvents.mockResolvedValue([makeEvent({ discord_message_id: undefined })]);

    render(AnnouncementsPage);

    await waitFor(() =>
      expect(screen.getByText('No events have been announced to Discord yet.')).toBeInTheDocument()
    );
  });

  it('shows an error when loading fails', async () => {
    getEvents.mockRejectedValue(new Error('network down'));

    render(AnnouncementsPage);

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('network down'));
  });

  it('requests declined events too, so a declined announcement still shows up', async () => {
    getEvents.mockResolvedValue([]);

    render(AnnouncementsPage);

    await waitFor(() => expect(getEvents).toHaveBeenCalledWith({ include_declined: true }));
  });
});
