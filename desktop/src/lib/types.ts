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
}

export interface BotChannelConfig {
  guild_id: string;
  events_channel_id?: string;
  announcements_channel_id?: string;
  reminders_channel_id?: string;
  digest_enabled: boolean;
  last_digest_sent_at?: string;
}

export interface UpdateBotChannelConfigRequest {
  events_channel_id?: string;
  announcements_channel_id?: string;
  reminders_channel_id?: string;
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
  /** Minutes before start_time that the reminder fires. 0 = no reminder. */
  reminder_lead_minutes: number;
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
