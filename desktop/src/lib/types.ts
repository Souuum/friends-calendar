export interface User {
  id: string;
  discord_id: string;
  username: string;
  discriminator?: string;
  avatar?: string;
  email?: string;
  display_name?: string;
  timezone: string;
  default_visibility: Visibility;
  notify_event_invites: boolean;
  notify_rsvp_changes: boolean;
  notify_announcements: boolean;
  notify_weekly_digest: boolean;
  notify_event_reminders: boolean;
  /** Whether the bot may DM you (nudges only, today). */
  notify_discord_dm: boolean;
}

export interface UpdateProfileRequest {
  display_name?: string;
  timezone?: string;
  default_visibility?: Visibility;
  notify_event_invites?: boolean;
  notify_rsvp_changes?: boolean;
  notify_announcements?: boolean;
  notify_weekly_digest?: boolean;
  notify_event_reminders?: boolean;
  notify_discord_dm?: boolean;
}

export interface BotChannelConfig {
  guild_id: string;
  announcements_channel_id?: string;
  digest_enabled: boolean;
  last_digest_sent_at?: string;
}

export interface UpdateBotChannelConfigRequest {
  announcements_channel_id?: string;
  digest_enabled?: boolean;
}

export interface CalendarEvent {
  id: string;
  creator_id: string;
  title: string;
  description?: string;
  start_time: string;
  end_time: string;
  location?: string;
  visibility: Visibility;
  created_at: string;
  updated_at: string;
  discord_message_id?: string;
  discord_channel_id?: string;
  price?: string;
  link?: string;
}

export type Status = 'pending' | 'accepted' | 'declined' | 'maybe';

// Must stay lowercase and must match the backend's Visibility enum, which
// carries #[serde(rename_all = "lowercase")] for exactly this reason. This
// union used to be written out inline in four places; one of them drifted
// to the capitalized form and silently 422'd every request that sent it.
export type Visibility = 'private' | 'friends' | 'public';

export interface ParticipantInfo {
  user_id: string;
  username: string;
  avatar_url?: string;
  status: Status;
  responded_at?: string;
}

export interface EventWithParticipants extends CalendarEvent {
  participants: ParticipantInfo[];
  is_creator: boolean;
  my_status?: Status;
  /**
   * Whether you're actually on the guest list, as opposed to just being
   * able to see the event (it's public, or friends-visible and its creator
   * is a friend). `my_status` can't answer this - it's absent both for a
   * non-participant and for an invitee who hasn't replied.
   */
  is_participant: boolean;
  /**
   * Reminder offsets in minutes-before-start, ascending. Empty = no
   * reminders. Several are allowed: an event can remind a week out, a day
   * out and an hour out.
   */
  reminder_leads: number[];
}

export interface FriendInfo {
  user_id: string;
  username: string;
  avatar_url?: string;
  synced_at: string;
}

export interface SyncFriendsResult {
  synced: number;
  removed: number;
  friends: FriendInfo[];
}

export interface LinkedServerInfo {
  id: string;
  name: string;
  icon_url?: string;
  approximate_member_count?: number;
}

export type AnnouncementTag = 'event' | 'general';

/**
 * The result of turning an existing Discord announcement into an event.
 *
 * `rsvps_recorded` is the point of the feature: people had already reacted
 * ✅ to the message, and adopting recovers those answers instead of asking
 * for them again.
 */
/** A time the group could actually meet. */
export interface BestSlot {
  start: string;
  /** How many *friends* are free - the caller isn't counted. */
  free_count: number;
  free_friend_ids: string[];
}

/** A calendar imported so availability knows about the rest of your life. */
export interface ExternalCalendar {
  id: string;
  provider: string;
  label?: string;
  last_synced_at?: string;
  /** Why it last failed. A silently dead connection is worse than none. */
  last_error?: string;
}

/**
 * A block of time imported from a connected calendar.
 *
 * ⚠️ No title, and there is no way to add one from the client: the server
 * never reads `SUMMARY` out of the feed and `external_busy` has nowhere to
 * put it. These render as anonymous "Busy" bands - *that* you are committed,
 * never *why*.
 */
export interface ExternalBusy {
  starts_at: string;
  ends_at: string;
}

/** What a nudge reached. */
export interface NudgeReport {
  nudged: number;
  /** How many also got a Discord DM. */
  dms_sent: number;
  /** The event's Discord thread couldn't be posted to; the in-app half went. */
  discord_failed: boolean;
}

/** A channel the bot can post in, as the picker on /server renders it. */
export interface ChannelInfo {
  id: string;
  name: string;
  /** The Discord category it sits under, when the bot can see one. */
  category?: string;
}

export interface AdoptionResult {
  event: CalendarEvent;
  rsvps_recorded: number;
  /** The event was created, but the existing reactions could not be read. */
  backfill_failed: boolean;
}

export interface AnnouncementPostInfo {
  id: string;
  author_username: string;
  author_avatar_url?: string;
  title?: string;
  body: string;
  tag: AnnouncementTag;
  reaction_count: number;
  reply_count: number;
  pinned: boolean;
  posted_at: string;
  /** Deep link to the message's Discord thread; absent if no server is linked. */
  thread_url?: string;
}

export interface NotificationInfo {
  id: string;
  kind: string;
  actor_username?: string;
  actor_avatar_url?: string;
  event_id?: string;
  message: string;
  read: boolean;
  created_at: string;
  /**
   * How *you* have answered the event this is about, when it is about one.
   *
   * ⚠️ Without this the page cannot tell an answered invite from an open
   * one: it showed three untouched Going/Maybe/Can't buttons either way, so
   * answering looked like it did nothing and a reload brought them back.
   * `read` is not a substitute - seen and answered are different facts.
   */
  my_status?: Status;
}

export interface FriendRequestInfo {
  id: string;
  from_user_id: string;
  from_username: string;
  from_avatar_url?: string;
  created_at: string;
}

export interface DayAvailability {
  date: string;
  free_user_ids: string[];
}

export type ButtonType = 'button' | 'submit' | 'reset';

/**
 * 'list' is the mobile agenda: upcoming events grouped by day rather than
 * laid out on a grid. It's offered at every width - a chronological list is
 * useful on a desktop too - but it's the one that makes the calendar usable
 * at 402px, where a 7-column grid has ~55px per day.
 */
export type ViewType = 'month' | 'week' | 'day' | 'list';

/** A reply in an announcement's Discord thread. Fetched live, never cached. */
export interface ReplyInfo {
  author_username: string;
  author_avatar_url?: string;
  body: string;
  posted_at: string;
}

/** A Discord server the bot is in. */
export interface GuildInfo {
  id: string;
  discord_guild_id: string;
  /** Null until the bot's gateway has seen the server (it fills this in on connect). */
  name?: string;
  icon_url?: string;
}

export interface ServersResponse {
  guilds: GuildInfo[];
  invite_url: string;
}
