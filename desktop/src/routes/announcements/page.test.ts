import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { AnnouncementPostInfo } from '$lib/types';

const { getAnnouncements, syncAnnouncements, adoptAnnouncement } = vi.hoisted(() => ({
  getAnnouncements: vi.fn(),
  syncAnnouncements: vi.fn(),
  adoptAnnouncement: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: {
    getAnnouncements,
    syncAnnouncements,
    adoptAnnouncement,
    clearToken: vi.fn(),
    getToken: vi.fn()
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
      run({ url: new URL('http://localhost/announcements') });
      return () => {};
    }
  }
}));

const { default: AnnouncementsPage } = await import('./+page.svelte');

function makePost(overrides: Partial<AnnouncementPostInfo> = {}): AnnouncementPostInfo {
  return {
    id: 'p1',
    author_username: 'alice',
    author_avatar_url: undefined,
    title: undefined,
    body: 'Hello everyone',
    tag: 'general',
    reaction_count: 0,
    reply_count: 0,
    pinned: false,
    posted_at: '2026-03-01T12:00:00Z',
    ...overrides
  };
}

describe('announcements page', () => {
  beforeEach(() => {
    getAnnouncements.mockReset();
    syncAnnouncements.mockReset();
    adoptAnnouncement.mockReset();
  });

  it('lists synced posts from the linked channel', async () => {
    getAnnouncements.mockResolvedValue([makePost({ id: 'p1', body: 'Game night this Friday!' })]);

    render(AnnouncementsPage);

    await waitFor(() => expect(screen.getByText('Game night this Friday!')).toBeInTheDocument());
    expect(screen.getByText('alice')).toBeInTheDocument();
  });

  it('shows an empty state when nothing has been synced yet', async () => {
    getAnnouncements.mockResolvedValue([]);

    render(AnnouncementsPage);

    await waitFor(() => expect(screen.getByText(/Nothing synced yet/)).toBeInTheDocument());
  });

  it('shows an error when loading fails', async () => {
    getAnnouncements.mockRejectedValue(new Error('network down'));

    render(AnnouncementsPage);

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('network down'));
  });

  it('re-syncs when "Sync now" is clicked', async () => {
    getAnnouncements.mockResolvedValue([]);
    syncAnnouncements.mockResolvedValue([makePost({ id: 'p2', body: 'Fresh from Discord' })]);

    render(AnnouncementsPage);
    await waitFor(() => expect(screen.getByText(/Nothing synced yet/)).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Sync now' }));

    await waitFor(() => expect(screen.getByText('Fresh from Discord')).toBeInTheDocument());
    expect(syncAnnouncements).toHaveBeenCalledOnce();
  });

  it('shows a sync error without clearing the existing posts', async () => {
    getAnnouncements.mockResolvedValue([makePost({ id: 'p1', body: 'Existing post' })]);
    syncAnnouncements.mockRejectedValue(new Error('No announcements channel configured'));

    render(AnnouncementsPage);
    await waitFor(() => expect(screen.getByText('Existing post')).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Sync now' }));

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent('No announcements channel configured')
    );
    expect(screen.getByText('Existing post')).toBeInTheDocument();
  });

  it('features only the first pinned post, not every pinned one', async () => {
    getAnnouncements.mockResolvedValue([
      makePost({ id: 'p1', pinned: true, title: 'Pinned one' }),
      makePost({ id: 'p2', pinned: true, title: 'Pinned two' }),
      makePost({ id: 'p3', pinned: false, title: 'Regular' })
    ]);

    const { container } = render(AnnouncementsPage);
    await waitFor(() => expect(screen.getByText('Pinned one')).toBeInTheDocument());

    // A feed of inverted cards would defeat the point of singling one out,
    // so exactly one gets the treatment.
    const featured = container.querySelectorAll('.bg-invert');
    expect(featured).toHaveLength(1);
  });

  it('features nothing when no post is pinned', async () => {
    getAnnouncements.mockResolvedValue([
      makePost({ id: 'p1', pinned: false, title: 'Just a post' })
    ]);

    const { container } = render(AnnouncementsPage);
    await waitFor(() => expect(screen.getByText('Just a post')).toBeInTheDocument());

    expect(container.querySelectorAll('.bg-invert')).toHaveLength(0);
  });

  describe('adding an announcement to the calendar', () => {
    const ANNOUNCEMENT = [
      "## Proposition d'activité :",
      '> Date : **<t:1795806000:F>**',
      '> Activité : **EsdeeKid**'
    ].join('\n');

    it('adopts the post and reports the RSVPs it recovered', async () => {
      getAnnouncements
        .mockResolvedValueOnce([makePost({ id: 'p1', tag: 'general', body: ANNOUNCEMENT })])
        .mockResolvedValueOnce([makePost({ id: 'p1', tag: 'event', body: ANNOUNCEMENT })]);
      adoptAnnouncement.mockResolvedValue({
        event: { id: 'e1' },
        rsvps_recorded: 3,
        backfill_failed: false
      });

      render(AnnouncementsPage);
      await waitFor(() =>
        expect(screen.getByRole('button', { name: /Add to calendar/ })).toBeInTheDocument()
      );

      await fireEvent.click(screen.getByRole('button', { name: /Add to calendar/ }));
      // The modal's own submit button carries the same label.
      const submit = screen.getAllByRole('button', { name: /Add to calendar/ }).at(-1)!;
      await fireEvent.click(submit);

      await waitFor(() =>
        expect(screen.getByRole('status')).toHaveTextContent('3 people already going')
      );
      expect(adoptAnnouncement).toHaveBeenCalledWith(
        'p1',
        expect.objectContaining({
          title: 'EsdeeKid'
        })
      );
      // Reloaded, so the post now shows its Event tag instead of staying
      // "General" until the next sync.
      expect(getAnnouncements).toHaveBeenCalledTimes(2);
      expect(screen.getByText('Event')).toBeInTheDocument();
    });

    // Nobody had reacted yet. Saying "0 people already going" would read as
    // a failure; the useful thing to say is what happens from now on.
    it('says what adopting achieved even when no one had reacted', async () => {
      getAnnouncements.mockResolvedValue([
        makePost({ id: 'p1', tag: 'general', body: ANNOUNCEMENT })
      ]);
      adoptAnnouncement.mockResolvedValue({
        event: { id: 'e1' },
        rsvps_recorded: 0,
        backfill_failed: false
      });

      render(AnnouncementsPage);
      await waitFor(() =>
        expect(screen.getByRole('button', { name: /Add to calendar/ })).toBeInTheDocument()
      );

      await fireEvent.click(screen.getByRole('button', { name: /Add to calendar/ }));
      await fireEvent.click(screen.getAllByRole('button', { name: /Add to calendar/ }).at(-1)!);

      await waitFor(() =>
        expect(screen.getByRole('status')).toHaveTextContent(/will now count as RSVPs/)
      );
    });

    it('offers nothing to adopt on a post that is already an event', async () => {
      getAnnouncements.mockResolvedValue([makePost({ id: 'p1', tag: 'event' })]);

      render(AnnouncementsPage);
      await waitFor(() => expect(screen.getByText('Event')).toBeInTheDocument());

      expect(screen.queryByRole('button', { name: /Add to calendar/ })).not.toBeInTheDocument();
    });
  });
});
