import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { AnnouncementPostInfo } from '$lib/types';

const { getAnnouncements, syncAnnouncements } = vi.hoisted(() => ({
  getAnnouncements: vi.fn(),
  syncAnnouncements: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { getAnnouncements, syncAnnouncements, clearToken: vi.fn(), getToken: vi.fn() }
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

    await waitFor(() =>
      expect(screen.getByText(/Nothing synced yet/)).toBeInTheDocument()
    );
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
    getAnnouncements.mockResolvedValue([makePost({ id: 'p1', pinned: false, title: 'Just a post' })]);

    const { container } = render(AnnouncementsPage);
    await waitFor(() => expect(screen.getByText('Just a post')).toBeInTheDocument());

    expect(container.querySelectorAll('.bg-invert')).toHaveLength(0);
  });
});
