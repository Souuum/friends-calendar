import type { Page } from '@playwright/test';

/**
 * Deterministic API stand-in for the layout tests.
 *
 * Content here is deliberately *long* - long event titles, long usernames,
 * long announcement bodies, a two-digit unread badge. An empty page never
 * overflows, so fixtures full of "Test Event" would make every overflow
 * assertion pass while proving nothing. These are sized like the worst
 * realistic case, which is what the assertions are actually for.
 */

const IN_2H = new Date(Date.now() + 2 * 3600_000).toISOString();
const IN_4H = new Date(Date.now() + 4 * 3600_000).toISOString();
const IN_2D = new Date(Date.now() + 48 * 3600_000).toISOString();
const IN_2D_LATER = new Date(Date.now() + 50 * 3600_000).toISOString();
const AGO_1H = new Date(Date.now() - 3600_000).toISOString();

const ME = {
  id: 'me-id',
  discord_id: 'me-discord',
  username: 'hugo_with_a_fairly_long_handle',
  display_name: 'Hugo (a long display name that could wrap)',
  timezone: 'Europe/Paris',
  default_visibility: 'friends',
  notify_event_invites: true,
  notify_rsvp_changes: true,
  notify_announcements: false,
  notify_weekly_digest: true,
  notify_event_reminders: true
};

const PARTICIPANTS = [
  { user_id: 'u1', username: 'alexandre_the_longest_name', status: 'accepted' },
  { user_id: 'u2', username: 'brigitte_also_quite_long', status: 'maybe' },
  { user_id: 'u3', username: 'camille', status: 'pending' }
];

const EVENTS = [
  {
    id: 'e1',
    creator_id: 'me-id',
    title: 'Soirée jeux de société chez Hugo — apportez vos boissons préférées',
    description:
      'On commence vers 20h. Il y a de la place pour huit personnes, et une longue description comme celle-ci existe pour vérifier que rien ne déborde du conteneur sur un écran étroit.',
    start_time: IN_2H,
    end_time: IN_4H,
    location: 'Chez Hugo, 12 rue de la République, quelque part assez loin',
    visibility: 'friends',
    created_at: AGO_1H,
    updated_at: AGO_1H,
    price: '15 €',
    link: 'https://example.com/a/rather/long/link/that/should/not/overflow',
    participants: PARTICIPANTS,
    is_creator: true,
    my_status: 'accepted',
    is_participant: true,
    reminder_leads: [10080, 1440, 60]
  },
  {
    id: 'e2',
    creator_id: 'u1',
    title: 'Escalade au gymnase',
    start_time: IN_2D,
    end_time: IN_2D_LATER,
    visibility: 'public',
    created_at: AGO_1H,
    updated_at: AGO_1H,
    participants: PARTICIPANTS.slice(0, 2),
    is_creator: false,
    my_status: 'pending',
    is_participant: true,
    reminder_leads: []
  }
];

const FRIENDS = [
  { user_id: 'u1', username: 'alexandre_the_longest_name', synced_at: AGO_1H },
  { user_id: 'u2', username: 'brigitte_also_quite_long', synced_at: AGO_1H },
  { user_id: 'u3', username: 'camille', synced_at: AGO_1H }
];

const ANNOUNCEMENTS = [
  {
    id: 'a1',
    author_username: 'alexandre_the_longest_name',
    title: 'Règles du serveur mises à jour pour la rentrée',
    body: 'Un message assez long, parce que les cartes doivent tenir à 402px de large sans provoquer de défilement horizontal, et un texte court ne le prouverait pas.',
    tag: 'general',
    reaction_count: 12,
    reply_count: 4,
    pinned: true,
    posted_at: AGO_1H
  },
  {
    id: 'a2',
    author_username: 'brigitte_also_quite_long',
    body: 'Court.',
    tag: 'event',
    reaction_count: 0,
    reply_count: 0,
    pinned: false,
    posted_at: AGO_1H
  }
];

const REPLIES = [
  {
    author_username: 'alexandre_the_longest_name',
    body: 'Une réponse assez longue pour occuper plusieurs lignes sur un écran de 402 pixels de large.',
    posted_at: AGO_1H
  },
  { author_username: 'camille', body: 'ok', posted_at: AGO_1H }
];

const NOTIFICATIONS = [
  {
    id: 'n1',
    kind: 'event_invite',
    actor_username: 'alexandre_the_longest_name',
    event_id: 'e2',
    message: 'alexandre_the_longest_name vous a invité à « Escalade au gymnase »',
    read: false,
    created_at: AGO_1H
  },
  {
    id: 'n2',
    kind: 'rsvp_change',
    actor_username: 'brigitte_also_quite_long',
    message: 'brigitte_also_quite_long a répondu « peut-être » à votre événement',
    read: true,
    created_at: AGO_1H
  }
];

const GUILDS = {
  guilds: [
    {
      id: 'g1',
      discord_guild_id: '111111111111111111',
      name: 'The Hangout — a server with a long name'
    },
    // Registered by id but never seen by the gateway, so no name yet.
    { id: 'g2', discord_guild_id: '222222222222222222' }
  ],
  invite_url:
    'https://discord.com/oauth2/authorize?client_id=abc&scope=bot&permissions=309237902400'
};

/** Longest path first, so `/api/notifications/unread-count` wins over `/api/notifications`. */
const ROUTES: Array<[string, unknown]> = [
  ['/api/notifications/unread-count', { count: 12 }],
  ['/api/availability/friends-now', ['u1', 'u2']],
  ['/api/availability/week', []],
  ['/api/friend-requests/missing-members', { missing: [] }],
  ['/api/friend-requests', []],
  ['/api/announcements/a1/replies', REPLIES],
  ['/api/announcements', ANNOUNCEMENTS],
  ['/api/notifications', NOTIFICATIONS],
  [
    '/api/discord/server',
    {
      id: '111111111111111111',
      name: 'The Hangout — a server with a long name',
      approximate_member_count: 42
    }
  ],
  ['/api/discord/config', { guild_id: '111111111111111111', digest_enabled: true }],
  ['/api/guilds', GUILDS],
  ['/api/friends', FRIENDS],
  ['/api/events', EVENTS],
  ['/api/auth/me', ME]
];

/**
 * Intercepts every backend call and serves fixtures, so the pages render
 * the same way on every run with no backend and no database involved.
 */
export async function mockApi(page: Page): Promise<void> {
  // The pages read the token before fetching; without it the root route
  // renders LoginScreen instead of the calendar.
  await page.addInitScript(() => {
    window.localStorage.setItem('jwt_token', 'layout-test-token');
  });

  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    const hit = ROUTES.find(([prefix]) => path.startsWith(prefix));

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      // An unmatched endpoint returns [] rather than failing the request:
      // a 500 would push pages into an error state and the layout under
      // test would never render at all.
      body: JSON.stringify(hit ? hit[1] : [])
    });
  });
}

/** Routes worth checking at every width. Dynamic ones use fixture ids. */
export const ROUTES_UNDER_TEST = [
  '/',
  '/friends',
  '/friends/u1',
  '/friends/add',
  '/announcements',
  '/announcements/a1',
  '/notifications',
  '/settings',
  '/server',
  '/servers'
];
