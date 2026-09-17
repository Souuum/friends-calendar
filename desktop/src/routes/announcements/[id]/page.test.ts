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
  posted_at: '2026-03-01T12:00:00Z',
  thread_url: 'https://discord.com/channels/g1/m1'
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
    // Twice on this page, and that is the real markup: the card's own
    // footer stats and the replies section heading both show the count.
    // It only became ambiguous once the icon was split out of the string -
    // the card used to render "💬 1 reply", which didn't match this.
    expect(screen.getAllByText(/1\s+reply/)).toHaveLength(2);
  });

  // Replying moved to Discord: the bot used to send these, so a thread
  // showed "friends-calendar" saying whatever a user typed, and with no
  // allowed_mentions guard a user could make the bot ping @everyone using
  // the bot's permissions rather than their own. Reading stays in the app.
  // Replying was removed on 2026-09-17 because the bot sent it: no
  // attribution, and a user's @everyone pinged the server with the bot's
  // permissions. It is back on a webhook, which fixes both - so these
  // assertions moved from "the composer is gone" to "it posts as you".
  it('posts a reply and shows the refreshed thread', async () => {
    postAnnouncementReply.mockResolvedValue([
      {
        author_username: 'me',
        author_avatar_url: undefined,
        body: "I'm in",
        posted_at: '2026-03-01T13:00:00Z'
      }
    ]);
    render(ThreadPage);
    await waitFor(() => expect(screen.getByText('Ski trip')).toBeInTheDocument());

    await fireEvent.input(screen.getByLabelText('Reply'), { target: { value: "I'm in" } });
    await fireEvent.click(screen.getByRole('button', { name: 'Reply' }));

    await waitFor(() =>
      // 'p1' is the mocked route param, which is what the page posts to -
      // not the post's own id.
      expect(postAnnouncementReply).toHaveBeenCalledWith('p1', "I'm in")
    );
    // The response *is* the refreshed thread, so no second fetch.
    expect(await screen.findByText("I'm in")).toBeInTheDocument();
    expect(screen.getByLabelText('Reply')).toHaveValue('');
  });

  // Retyping a lost reply is the annoying half of a failed send.
  it('keeps the draft when posting fails', async () => {
    postAnnouncementReply.mockRejectedValue(new Error('Discord said no'));
    render(ThreadPage);
    await waitFor(() => expect(screen.getByText('Ski trip')).toBeInTheDocument());

    await fireEvent.input(screen.getByLabelText('Reply'), { target: { value: 'worth keeping' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Reply' }));

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('Discord said no'));
    expect(screen.getByLabelText('Reply')).toHaveValue('worth keeping');
  });

  it('will not post an empty reply', async () => {
    render(ThreadPage);
    await waitFor(() => expect(screen.getByText('Ski trip')).toBeInTheDocument());

    expect(screen.getByRole('button', { name: 'Reply' })).toBeDisabled();
  });

  // The deep link stays alongside the composer: some people would rather
  // answer in Discord proper.
  it('still links out to the thread', async () => {
    render(ThreadPage);

    const link = await screen.findByRole('link', { name: /Open in Discord/i });
    expect(link).toHaveAttribute('href', 'https://discord.com/channels/g1/m1');
    expect(link).toHaveAttribute('target', '_blank');
  });

  // No linked server means no URL to offer - but the composer still works,
  // because posting goes through the app, not the link.
  it('still offers the composer when there is no thread link', async () => {
    getAnnouncements.mockResolvedValue([{ ...post, thread_url: undefined }]);

    render(ThreadPage);

    await waitFor(() => expect(screen.getByLabelText('Reply')).toBeInTheDocument());
    expect(screen.queryByRole('link', { name: /Open in Discord/i })).not.toBeInTheDocument();
  });

  it('invites a first reply when the thread is empty', async () => {
    render(ThreadPage);
    await waitFor(() => expect(screen.getByText(/No replies yet/)).toBeInTheDocument());
  });
});
