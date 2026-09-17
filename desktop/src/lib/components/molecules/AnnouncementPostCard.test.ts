import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import AnnouncementPostCard from './AnnouncementPostCard.svelte';
import type { AnnouncementPostInfo } from '$lib/types';

function makePost(overrides: Partial<AnnouncementPostInfo> = {}): AnnouncementPostInfo {
  return {
    id: 'p1',
    author_username: 'alice',
    author_avatar_url: undefined,
    title: undefined,
    body: 'Hello everyone',
    tag: 'general',
    reaction_count: 3,
    reply_count: 1,
    pinned: false,
    posted_at: '2026-03-01T12:00:00Z',
    ...overrides
  };
}

describe('AnnouncementPostCard', () => {
  it('renders the author, body, and reaction/reply counts', () => {
    render(AnnouncementPostCard, { post: makePost() });

    expect(screen.getByText('alice')).toBeInTheDocument();
    expect(screen.getByText('Hello everyone')).toBeInTheDocument();
    expect(screen.getByText('3')).toBeInTheDocument();
    expect(screen.getByText(/1\s+reply/)).toBeInTheDocument();
  });

  it('uses plural "replies" for anything other than exactly one', () => {
    render(AnnouncementPostCard, { post: makePost({ reply_count: 0 }) });
    expect(screen.getByText(/0\s+replies/)).toBeInTheDocument();
  });

  it('shows a title when present', () => {
    render(AnnouncementPostCard, { post: makePost({ title: 'Game Night' }) });
    expect(screen.getByText('Game Night')).toBeInTheDocument();
  });

  it('shows a pinned badge only when pinned', () => {
    const { rerender } = render(AnnouncementPostCard, { post: makePost({ pinned: false }) });
    expect(screen.queryByText('Pinned')).not.toBeInTheDocument();

    rerender({ post: makePost({ pinned: true }) });
    expect(screen.getByText('Pinned')).toBeInTheDocument();
  });

  it('labels the event tag distinctly from general', () => {
    render(AnnouncementPostCard, { post: makePost({ tag: 'event' }) });
    expect(screen.getByText('Event')).toBeInTheDocument();
  });

  describe('the "add to calendar" action', () => {
    it('is absent unless a handler is supplied', () => {
      render(AnnouncementPostCard, { props: { post: makePost({ tag: 'general' }) } });

      expect(screen.queryByRole('button', { name: /Add to calendar/ })).not.toBeInTheDocument();
    });

    it('offers a general post to the handler', async () => {
      const onAdopt = vi.fn();
      const post = makePost({ tag: 'general' });
      render(AnnouncementPostCard, { props: { post, onAdopt } });

      await fireEvent.click(screen.getByRole('button', { name: /Add to calendar/ }));

      expect(onAdopt).toHaveBeenCalledWith(post);
    });

    // A post tagged `event` already has a calendar row behind it. Offering
    // to add it again would promise a second event the backend refuses to
    // create - so the button must not be there at all.
    it('is absent on a post that is already an event', () => {
      render(AnnouncementPostCard, {
        props: { post: makePost({ tag: 'event' }), onAdopt: vi.fn() }
      });

      expect(screen.queryByRole('button', { name: /Add to calendar/ })).not.toBeInTheDocument();
    });
  });
});
