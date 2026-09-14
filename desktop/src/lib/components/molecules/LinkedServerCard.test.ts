import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import LinkedServerCard from './LinkedServerCard.svelte';
import type { LinkedServerInfo } from '$lib/types';

const server: LinkedServerInfo = {
  id: 'g1',
  name: 'Friends Server',
  icon_url: 'https://cdn.discordapp.com/icons/g1/abc123.png',
  approximate_member_count: 6
};

describe('LinkedServerCard', () => {
  it('renders the server name and member count', () => {
    render(LinkedServerCard, { server });

    expect(screen.getByText('Friends Server')).toBeInTheDocument();
    expect(screen.getByText('6 members')).toBeInTheDocument();
  });

  it('omits the member count when unknown', () => {
    render(LinkedServerCard, { server: { ...server, approximate_member_count: undefined } });

    expect(screen.getByText('Friends Server')).toBeInTheDocument();
    expect(screen.queryByText(/members/)).not.toBeInTheDocument();
  });
});
