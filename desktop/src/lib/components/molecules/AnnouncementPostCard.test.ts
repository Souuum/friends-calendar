import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
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
    expect(screen.getByText('👍 3')).toBeInTheDocument();
    expect(screen.getByText('💬 1 reply')).toBeInTheDocument();
  });

  it('uses plural "replies" for anything other than exactly one', () => {
    render(AnnouncementPostCard, { post: makePost({ reply_count: 0 }) });
    expect(screen.getByText('💬 0 replies')).toBeInTheDocument();
  });

  it('shows a title when present', () => {
    render(AnnouncementPostCard, { post: makePost({ title: 'Game Night' }) });
    expect(screen.getByText('Game Night')).toBeInTheDocument();
  });

  it('shows a pinned badge only when pinned', () => {
    const { rerender } = render(AnnouncementPostCard, { post: makePost({ pinned: false }) });
    expect(screen.queryByText('📌 Pinned')).not.toBeInTheDocument();

    rerender({ post: makePost({ pinned: true }) });
    expect(screen.getByText('📌 Pinned')).toBeInTheDocument();
  });

  it('labels the event tag distinctly from general', () => {
    render(AnnouncementPostCard, { post: makePost({ tag: 'event' }) });
    expect(screen.getByText('📅 Event')).toBeInTheDocument();
  });
});
