import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import AnnouncementCard from './AnnouncementCard.svelte';
import type { EventWithParticipants } from '$lib/types';

const baseEvent: EventWithParticipants = {
  id: 'e1',
  creator_id: 'me-id',
  title: 'Board Game Night',
  description: 'Bring snacks',
  start_time: '2026-03-01T19:00:00Z',
  end_time: '2026-03-01T22:00:00Z',
  location: "Alice's place",
  visibility: 'friends',
  created_at: '2026-02-01T00:00:00Z',
  updated_at: '2026-02-01T00:00:00Z',
  discord_message_id: 'msg1',
  price: '5€',
  link: 'https://example.com/event',
  is_creator: true,
  my_status: 'accepted',
  participants: [
    { user_id: 'me-id', username: 'me', status: 'accepted' },
    { user_id: 'bob-id', username: 'bob', status: 'declined' },
    { user_id: 'zoe-id', username: 'zoe', status: 'maybe' }
  ]
};

describe('AnnouncementCard', () => {
  it('shows the event title, location, price and link', () => {
    render(AnnouncementCard, { event: baseEvent });

    expect(screen.getByText('Board Game Night')).toBeInTheDocument();
    expect(screen.getByText("Alice's place")).toBeInTheDocument();
    expect(screen.getByText('5€')).toBeInTheDocument();
    expect(screen.getByText('https://example.com/event')).toBeInTheDocument();
  });

  it("shows whether the current user accepted, and everyone else's response", () => {
    render(AnnouncementCard, { event: baseEvent });

    expect(screen.getByText('✓ You accepted')).toBeInTheDocument();
    expect(screen.getByText('bob')).toBeInTheDocument();
    expect(screen.getByText('zoe')).toBeInTheDocument();
    expect(screen.getByText('3 participants')).toBeInTheDocument();
  });

  it('reflects a declined or unanswered status', () => {
    const { rerender } = render(AnnouncementCard, {
      event: { ...baseEvent, my_status: 'declined' }
    });
    expect(screen.getByText('✗ You declined')).toBeInTheDocument();

    rerender({ event: { ...baseEvent, my_status: undefined } });
    expect(screen.getByText('No response yet')).toBeInTheDocument();
  });

  it('omits optional fields that are absent', () => {
    const minimal: EventWithParticipants = {
      ...baseEvent,
      description: undefined,
      location: undefined,
      price: undefined,
      link: undefined
    };

    render(AnnouncementCard, { event: minimal });

    expect(screen.queryByText("Alice's place")).not.toBeInTheDocument();
    expect(screen.queryByText('5€')).not.toBeInTheDocument();
    expect(screen.queryByText('https://example.com/event')).not.toBeInTheDocument();
  });
});
