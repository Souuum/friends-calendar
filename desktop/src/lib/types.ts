export interface User {
  id: string;
  discord_id: string;
  username: string;
  discriminator?: string;
  avatar?: string;
  email?: string;
}

export interface CalendarEvent {
  id: string;
  creator_id: string;
  title: string;
  description?: string;
  start_time: string;
  end_time: string;
  location?: string;
  visibility: 'private' | 'friends' | 'public';
  created_at: string;
  updated_at: string;
  discord_message_id?: string;
  discord_channel_id?: string;
  price?: string;
  link?: string;
}

export type Status = 'pending' | 'accepted' | 'declined' | 'maybe';

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
}

export type ButtonType = 'button' | 'submit' | 'reset';

export type ViewType = 'month' | 'week' | 'day';
