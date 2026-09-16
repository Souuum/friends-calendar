import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { AnnouncementPostInfo, ReplyInfo } from '$lib/types';

const { getAnnouncements, getAnnouncementReplies, postAnnouncementReply } = vi.hoisted(() => ({
  getAnnouncements: vi.fn(),
  getAnnouncementReplies: vi.fn(),
  postAnnouncementReply: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: {
    getAnnouncements,
    getAnnouncementReplies,
    postAnnouncementReply,
    getUnreadNotificationCount: vi.fn().mockResolvedValue(0),
    clearToken: vi.fn()
  }
}));

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

vi.mock('$app/stores', () => ({
  page: {
    subscribe: (run: (value: { url: URL; params: { id: string } }) => void) => {
      run({ url: new URL('http://localhost/announcements/p1'), params: { id: 'p1' } });
      return () => {};
    }
  }
}));

const { default: ThreadPage } = await import('./+page.svelte');

const post: AnnouncementPostInfo = {
  id: 'p1',
  author_username: 'alice',
  author_avatar_url: undefined,
  title: 'Ski trip',
  body: 'Deposit due Friday',
  tag: 'general',
  reaction_count: 0,
  reply_count: 1,
  pinned: false,
  posted_at: '2026-03-01T12:00:00Z'
};

function reply(body: string): ReplyInfo {
  return {
    author_username: 'bob',
    author_avatar_url: undefined,
    body,
    posted_at: '2026-03-01T13:00:00Z'
  };
}

describe('announcement thread page', () => {
  beforeEach(() => {
    getAnnouncements.mockReset().mockResolvedValue([post]);
    getAnnouncementReplies.mockReset().mockResolvedValue([]);
    postAnnouncementReply.mockReset();
  });

  it('shows the post and its replies', async () => {
    getAnnouncementReplies.mockResolvedValue([reply("I'm in")]);

    render(ThreadPage);

    await waitFor(() => expect(screen.getByText('Ski trip')).toBeInTheDocument());
    expect(screen.getByText("I'm in")).toBeInTheDocument();
    expect(screen.getByText('1 reply')).toBeInTheDocument();
  });

  it('invites a first reply when the thread is empty', async () => {
    render(ThreadPage);
    await waitFor(() => expect(screen.getByText(/No replies yet/)).toBeInTheDocument());
  });

  it('posts a reply and renders the thread the server returns', async () => {
    postAnnouncementReply.mockResolvedValue([reply('posted!')]);

    render(ThreadPage);
    await waitFor(() => expect(screen.getByLabelText('Reply')).toBeInTheDocument());

    await fireEvent.input(screen.getByLabelText('Reply'), { target: { value: 'posted!' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() => expect(postAnnouncementReply).toHaveBeenCalledWith('p1', 'posted!'));
    // The endpoint returns the refreshed thread, so no second fetch is needed.
    await waitFor(() => expect(screen.getByText('posted!')).toBeInTheDocument());
    expect(getAnnouncementReplies).toHaveBeenCalledTimes(1);
  });

  it('keeps the draft and shows an error when sending fails', async () => {
    postAnnouncementReply.mockRejectedValue(new Error('Discord said no'));

    render(ThreadPage);
    await waitFor(() => expect(screen.getByLabelText('Reply')).toBeInTheDocument());

    await fireEvent.input(screen.getByLabelText('Reply'), { target: { value: 'keep me' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() => expect(screen.getByText('Discord said no')).toBeInTheDocument());
    // Losing what someone typed because the network blipped is the worst
    // possible response to a failure here.
    expect(screen.getByLabelText('Reply')).toHaveValue('keep me');
  });

  it('will not send a blank reply', async () => {
    render(ThreadPage);
    await waitFor(() => expect(screen.getByLabelText('Reply')).toBeInTheDocument());

    await fireEvent.input(screen.getByLabelText('Reply'), { target: { value: '   ' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Send' }));

    expect(postAnnouncementReply).not.toHaveBeenCalled();
  });
});
