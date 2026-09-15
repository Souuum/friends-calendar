import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { FriendRequestInfo } from '$lib/types';

const {
  sendFriendRequest,
  listFriendRequests,
  acceptFriendRequest,
  declineFriendRequest,
  getMissingMembersCount,
  postGuildInvite,
  goto
} = vi.hoisted(() => ({
  sendFriendRequest: vi.fn(),
  listFriendRequests: vi.fn(),
  acceptFriendRequest: vi.fn(),
  declineFriendRequest: vi.fn(),
  getMissingMembersCount: vi.fn(),
  postGuildInvite: vi.fn(),
  goto: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: {
    sendFriendRequest,
    listFriendRequests,
    acceptFriendRequest,
    declineFriendRequest,
    getMissingMembersCount,
    postGuildInvite,
    clearToken: vi.fn()
  }
}));

vi.mock('$app/navigation', () => ({ goto }));

vi.mock('$app/stores', () => ({
  page: {
    subscribe: (run: (value: { url: URL }) => void) => {
      run({ url: new URL('http://localhost/friends/add') });
      return () => {};
    }
  }
}));

const { default: AddFriendPage } = await import('./+page.svelte');

function request(overrides: Partial<FriendRequestInfo>): FriendRequestInfo {
  return {
    id: 'r1',
    from_user_id: 'alice-id',
    from_username: 'alice',
    from_avatar_url: undefined,
    created_at: '2026-01-01T00:00:00Z',
    ...overrides
  };
}

describe('add friends page', () => {
  beforeEach(() => {
    sendFriendRequest.mockReset();
    listFriendRequests.mockReset();
    acceptFriendRequest.mockReset();
    declineFriendRequest.mockReset();
    getMissingMembersCount.mockReset();
    postGuildInvite.mockReset();
    goto.mockReset();
    listFriendRequests.mockResolvedValue([]);
    getMissingMembersCount.mockResolvedValue(0);
  });

  it('sends a friend request by username', async () => {
    sendFriendRequest.mockResolvedValue({ status: 'sent' });
    render(AddFriendPage);

    await fireEvent.input(screen.getByLabelText('Username'), { target: { value: 'bob' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Send request' }));

    await waitFor(() => expect(sendFriendRequest).toHaveBeenCalledWith('bob'));
    expect(screen.getByText('Request sent.')).toBeInTheDocument();
  });

  it('shows a distinct message when the request auto-accepts', async () => {
    sendFriendRequest.mockResolvedValue({ status: 'auto_accepted' });
    render(AddFriendPage);

    await fireEvent.input(screen.getByLabelText('Username'), { target: { value: 'bob' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Send request' }));

    await waitFor(() => expect(screen.getByText('You and bob are now friends!')).toBeInTheDocument());
  });

  it('surfaces an error from the backend (e.g. already pending)', async () => {
    sendFriendRequest.mockRejectedValue(new Error('A request is already pending'));
    render(AddFriendPage);

    await fireEvent.input(screen.getByLabelText('Username'), { target: { value: 'bob' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Send request' }));

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('A request is already pending'));
  });

  it('lists incoming requests and accepts one', async () => {
    listFriendRequests.mockResolvedValue([request({ id: 'r1', from_username: 'alice' })]);
    acceptFriendRequest.mockResolvedValue(undefined);

    render(AddFriendPage);
    await waitFor(() => expect(screen.getByText('alice')).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Accept' }));

    await waitFor(() => expect(acceptFriendRequest).toHaveBeenCalledWith('r1'));
    await waitFor(() => expect(screen.queryByText('alice')).not.toBeInTheDocument());
  });

  it('declines a request', async () => {
    listFriendRequests.mockResolvedValue([request({ id: 'r1', from_username: 'alice' })]);
    declineFriendRequest.mockResolvedValue(undefined);

    render(AddFriendPage);
    await waitFor(() => expect(screen.getByText('alice')).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Decline' }));

    await waitFor(() => expect(declineFriendRequest).toHaveBeenCalledWith('r1'));
    await waitFor(() => expect(screen.queryByText('alice')).not.toBeInTheDocument());
  });

  it('shows the missing-members prompt and posts an invite', async () => {
    getMissingMembersCount.mockResolvedValue(5);
    postGuildInvite.mockResolvedValue(undefined);

    render(AddFriendPage);
    await waitFor(() =>
      expect(screen.getByText("5 members of your server aren't on Friends Calendar yet")).toBeInTheDocument()
    );

    await fireEvent.click(screen.getByRole('button', { name: 'Post invite' }));

    await waitFor(() => expect(postGuildInvite).toHaveBeenCalledOnce());
    expect(screen.getByText('Invite posted!')).toBeInTheDocument();
  });

  it('hides the missing-members prompt when there is nothing to report', async () => {
    getMissingMembersCount.mockResolvedValue(0);
    render(AddFriendPage);

    await waitFor(() => expect(screen.getByText('No pending requests.')).toBeInTheDocument());
    expect(screen.queryByText(/aren't on Friends Calendar yet/)).not.toBeInTheDocument();
  });

  it('navigates back to /friends', async () => {
    render(AddFriendPage);
    await waitFor(() => expect(screen.getByText('No pending requests.')).toBeInTheDocument());

    await fireEvent.click(screen.getByText('← Back to friends'));

    expect(goto).toHaveBeenCalledWith('/friends');
  });
});
