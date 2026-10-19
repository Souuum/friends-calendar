import type {
  AnnouncementPostInfo,
  BestSlot,
  EventWithParticipants,
  ExternalBusy,
  FriendInfo,
  GuildInfo,
  LinkedServerInfo,
  ParticipantInfo,
  User
} from '$lib/types';
import mimo from './assets/mimo.png';

/**
 * Shared story fixtures. Times are relative to *now* so the calendar views
 * always have something on screen in the month/week/day they open on.
 */

/** Today at `hour`:`minute`, offset by `dayOffset` days. */
export function at(dayOffset: number, hour: number, minute = 0): string {
  const d = new Date();
  d.setDate(d.getDate() + dayOffset);
  d.setHours(hour, minute, 0, 0);
  return d.toISOString();
}

const AGO_1H = new Date(Date.now() - 3600_000).toISOString();

export const ME: User = {
  id: 'me-id',
  discord_id: '80351110224678912',
  username: 'hugo',
  display_name: 'Hugo',
  timezone: 'Europe/Paris',
  default_visibility: 'friends',
  notify_event_invites: true,
  notify_rsvp_changes: true,
  notify_announcements: false,
  notify_weekly_digest: true,
  notify_event_reminders: true,
  notify_discord_dm: true
};

export const PARTICIPANTS: ParticipantInfo[] = [
  { user_id: 'u1', username: 'alexandre', avatar_url: mimo, status: 'accepted' },
  { user_id: 'u2', username: 'brigitte', avatar_url: mimo, status: 'maybe' },
  { user_id: 'u3', username: 'camille', status: 'pending' },
  { user_id: 'u4', username: 'damien', status: 'declined' }
];

export const FRIENDS: FriendInfo[] = PARTICIPANTS.map((p) => ({
  user_id: p.user_id,
  username: p.username,
  avatar_url: p.avatar_url,
  synced_at: AGO_1H
}));

function event(overrides: Partial<EventWithParticipants>): EventWithParticipants {
  return {
    id: 'e0',
    creator_id: 'me-id',
    title: 'Event',
    start_time: at(0, 20),
    end_time: at(0, 22),
    visibility: 'friends',
    created_at: AGO_1H,
    updated_at: AGO_1H,
    participants: PARTICIPANTS,
    is_creator: true,
    my_status: 'accepted',
    is_participant: true,
    reminder_leads: [1440, 60],
    ...overrides
  };
}

export const EVENT_HOSTED = event({
  id: 'e1',
  title: 'Soirée jeux de société',
  description: 'On commence vers 20h. Apportez vos jeux préférés.',
  location: 'Chez Hugo',
  price: '5 €',
  link: 'https://example.com/soiree'
});

export const EVENT_INVITED = event({
  id: 'e2',
  creator_id: 'u1',
  title: 'Escalade au gymnase',
  start_time: at(1, 18),
  end_time: at(1, 20),
  location: 'Block Out',
  is_creator: false,
  my_status: 'pending',
  participants: PARTICIPANTS.slice(0, 3)
});

export const EVENT_MAYBE = event({
  id: 'e3',
  creator_id: 'u2',
  title: 'Brunch',
  start_time: at(2, 11),
  end_time: at(2, 13),
  is_creator: false,
  my_status: 'maybe'
});

export const EVENT_DECLINED = event({
  id: 'e4',
  creator_id: 'u3',
  title: 'Cinéma',
  start_time: at(3, 21),
  end_time: at(3, 23),
  is_creator: false,
  my_status: 'declined'
});

/** Visible because it's public, not because you were invited. */
export const EVENT_DISCOVERED = event({
  id: 'e5',
  creator_id: 'u4',
  title: 'Concert au Bikini',
  start_time: at(5, 20),
  end_time: at(5, 23),
  visibility: 'public',
  is_creator: false,
  my_status: undefined,
  is_participant: false
});

export const EVENT_LONG_TITLE = event({
  id: 'e6',
  title: 'Week-end au lac avec toute la bande — covoiturage depuis la gare, prévoir les tentes',
  start_time: at(0, 9),
  end_time: at(0, 12),
  location: 'Lac de Saint-Ferréol, quelque part assez loin pour que ça déborde'
});

export const EVENTS: EventWithParticipants[] = [
  EVENT_HOSTED,
  EVENT_LONG_TITLE,
  EVENT_INVITED,
  EVENT_MAYBE,
  EVENT_DECLINED,
  EVENT_DISCOVERED
];

export const BUSY: ExternalBusy[] = [
  { starts_at: at(0, 14), ends_at: at(0, 16) },
  { starts_at: at(1, 9), ends_at: at(1, 17) }
];

export const BEST_SLOT: BestSlot = {
  start: at(4, 20),
  free_count: 3,
  free_friend_ids: ['u1', 'u2', 'u3']
};

export const LINKED_SERVER: LinkedServerInfo = {
  id: '111111111111111111',
  name: 'The Hangout',
  icon_url: mimo,
  approximate_member_count: 42
};

export const GUILDS: GuildInfo[] = [
  { id: 'g1', discord_guild_id: '111111111111111111', name: 'The Hangout', icon_url: mimo },
  // Registered by id but never seen by the gateway, so no name yet.
  { id: 'g2', discord_guild_id: '222222222222222222' }
];

export const INVITE_URL =
  'https://discord.com/oauth2/authorize?client_id=abc&scope=bot&permissions=309237902400';

export const ANNOUNCEMENT_BOT: AnnouncementPostInfo = {
  id: 'a1',
  author_username: 'friends-calendar',
  author_avatar_url: mimo,
  title: '@everyone',
  body:
    "## Proposition d'activité :\n" +
    '> Date : **<t:1795806000:F>**\n' +
    '> Activité : **EsdeeKid**\n' +
    "> Lieu : **L'Olympia**\n" +
    '> Prix : **59€ fosse**\n' +
    '> Lien : [Billetterie](https://example.com/tickets)\n\n' +
    '**Réagissez avec ✅ pour participer !**',
  tag: 'event',
  reaction_count: 12,
  reply_count: 4,
  pinned: true,
  posted_at: AGO_1H,
  thread_url: 'https://discord.com/channels/111111111111111111/333333333333333333'
};

export const ANNOUNCEMENT_HANDWRITTEN: AnnouncementPostInfo = {
  id: 'a2',
  author_username: 'julioo',
  body: 'Concert samedi soir au Bikini, 20h. Qui vient ?',
  tag: 'general',
  reaction_count: 5,
  reply_count: 0,
  pinned: false,
  posted_at: AGO_1H
};

/** The `eventsForDay` prop the grid views take, over a fixed list. */
export function eventsForDayOf(events: EventWithParticipants[]) {
  return (day: Date) =>
    events.filter((e) => new Date(e.start_time).toDateString() === day.toDateString());
}
