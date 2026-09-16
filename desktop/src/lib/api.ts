import type {
  User,
  Visibility,
  CalendarEvent,
  EventWithParticipants,
  ParticipantInfo,
  FriendInfo,
  SyncFriendsResult,
  LinkedServerInfo,
  NotificationInfo,
  FriendRequestInfo,
  DayAvailability,
  UpdateProfileRequest,
  BotChannelConfig,
  UpdateBotChannelConfigRequest,
  AnnouncementPostInfo,
  ReplyInfo
} from './types';

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:8080';

class ApiClient {
  private token: string | null = null;

  setToken(token: string) {
    this.token = token;
    localStorage.setItem('jwt_token', token);
  }

  getToken(): string | null {
    if (!this.token) {
      this.token = localStorage.getItem('jwt_token');
    }
    return this.token;
  }

  clearToken() {
    this.token = null;
    localStorage.removeItem('jwt_token');
  }

  private async fetch<T>(endpoint: string, options: RequestInit = {}): Promise<T> {
    const token = this.getToken();
    const headers: HeadersInit = new Headers(options.headers);
    headers.set('Content-Type', 'application/json');

    if (token) {
      headers.set('Authorization', `Bearer ${token}`);
    }

    const response = await fetch(`${API_URL}${endpoint}`, {
      ...options,
      headers
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({ error: 'Unknown error' }));
      throw new Error(error.error || `HTTP ${response.status}`);
    }

    if (response.status === 204) {
      return null as T;
    }

    return response.json();
  }

  // Auth
  async getCurrentUser(): Promise<User> {
    return this.fetch<User>('/api/auth/me');
  }

  async updateProfile(data: UpdateProfileRequest): Promise<User> {
    return this.fetch<User>('/api/auth/me', {
      method: 'PATCH',
      body: JSON.stringify(data)
    });
  }

  async deleteAccount(confirmUsername: string): Promise<void> {
    return this.fetch<void>('/api/auth/me', {
      method: 'DELETE',
      body: JSON.stringify({ confirm_username: confirmUsername })
    });
  }

  // Events
  async createEvent(data: {
    title: string;
    description?: string;
    start_time: string;
    end_time: string;
    location?: string;
    visibility?: Visibility;
    participant_ids?: string[];
    price?: string;
    link?: string;
    reminder_lead_minutes?: number;
  }): Promise<CalendarEvent> {
    return this.fetch<CalendarEvent>('/api/events', {
      method: 'POST',
      body: JSON.stringify(data)
    });
  }

  /**
   * Renders the Discord announcement for an event that doesn't exist yet.
   * Server-side on purpose: it reuses the same formatter the real
   * announcement uses, so the preview can't drift from what gets posted.
   */
  async previewAnnouncement(data: {
    title: string;
    description?: string;
    start_time: string;
    end_time: string;
    location?: string;
    price?: string;
    link?: string;
  }): Promise<string> {
    const { message } = await this.fetch<{ message: string }>(
      '/api/events/announcement-preview',
      { method: 'POST', body: JSON.stringify(data) }
    );
    return message;
  }

  async getEvents(params?: {
    start_date?: string;
    end_date?: string;
    include_declined?: boolean;
  }): Promise<EventWithParticipants[]> {
    const query = new URLSearchParams();
    if (params?.start_date) query.append('start_date', params.start_date);
    if (params?.end_date) query.append('end_date', params.end_date);
    if (params?.include_declined !== undefined) {
      query.append('include_declined', String(params.include_declined));
    }

    const queryString = query.toString();
    return this.fetch<EventWithParticipants[]>(
      `/api/events${queryString ? `?${queryString}` : ''}`
    );
  }

  async getEvent(id: string): Promise<EventWithParticipants> {
    return this.fetch<EventWithParticipants>(`/api/events/${id}`);
  }

  async updateEvent(
    id: string,
    data: Partial<{
      title: string;
      description: string;
      start_time: string;
      end_time: string;
      location: string;
      visibility: Visibility;
      price: string;
      link: string;
      reminder_lead_minutes: number;
    }>
  ): Promise<CalendarEvent> {
    return this.fetch<CalendarEvent>(`/api/events/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data)
    });
  }

  async deleteEvent(id: string): Promise<void> {
    return this.fetch<void>(`/api/events/${id}`, {
      method: 'DELETE'
    });
  }

  async updateParticipation(
    eventId: string,
    status: 'accepted' | 'declined' | 'maybe'
  ): Promise<void> {
    return this.fetch<void>(`/api/events/${eventId}/participation`, {
      method: 'PUT',
      body: JSON.stringify({ status })
    });
  }

  // Friends
  // Currently-synced friends: other app users sharing the Discord server
  // this app's bot is in. Discord doesn't expose real Friends/relationships
  // to bots or OAuth2 apps, so this is a shared-server proxy — see
  // backend/src/services/friends.rs.
  async getFriends(): Promise<FriendInfo[]> {
    return this.fetch<FriendInfo[]>('/api/friends');
  }

  async syncFriends(): Promise<SyncFriendsResult> {
    return this.fetch<SyncFriendsResult>('/api/friends/sync', {
      method: 'POST'
    });
  }

  // Discord server this app is linked to (single server for now)
  async getLinkedServer(): Promise<LinkedServerInfo> {
    return this.fetch<LinkedServerInfo>('/api/discord/server');
  }

  async getDiscordConfig(): Promise<BotChannelConfig> {
    return this.fetch<BotChannelConfig>('/api/discord/config');
  }

  async updateDiscordConfig(data: UpdateBotChannelConfigRequest): Promise<BotChannelConfig> {
    return this.fetch<BotChannelConfig>('/api/discord/config', {
      method: 'PUT',
      body: JSON.stringify(data)
    });
  }

  // Announcements: a mirror of the linked channel's Discord messages
  // (replaces the old event-RSVP-tracking view).
  async getAnnouncements(): Promise<AnnouncementPostInfo[]> {
    return this.fetch<AnnouncementPostInfo[]>('/api/announcements');
  }

  async syncAnnouncements(): Promise<AnnouncementPostInfo[]> {
    return this.fetch<AnnouncementPostInfo[]>('/api/announcements/sync', {
      method: 'POST'
    });
  }

  /**
   * Replies live in the post's Discord thread, so both of these hit Discord
   * server-side rather than reading announcement_posts.reply_count - that
   * count is whatever the last sync saw.
   */
  async getAnnouncementReplies(id: string): Promise<ReplyInfo[]> {
    return this.fetch<ReplyInfo[]>(`/api/announcements/${id}/replies`);
  }

  /** Returns the refreshed thread, not just the sent reply. */
  async postAnnouncementReply(id: string, body: string): Promise<ReplyInfo[]> {
    return this.fetch<ReplyInfo[]>(`/api/announcements/${id}/reply`, {
      method: 'POST',
      body: JSON.stringify({ body })
    });
  }

  // Notifications
  async getNotifications(): Promise<NotificationInfo[]> {
    return this.fetch<NotificationInfo[]>('/api/notifications');
  }

  async getUnreadNotificationCount(): Promise<number> {
    const { count } = await this.fetch<{ count: number }>('/api/notifications/unread-count');
    return count;
  }

  async markNotificationRead(id: string): Promise<void> {
    return this.fetch<void>(`/api/notifications/${id}/read`, {
      method: 'POST'
    });
  }

  async markAllNotificationsRead(): Promise<void> {
    return this.fetch<void>('/api/notifications/read-all', {
      method: 'POST'
    });
  }

  // Friend requests
  async sendFriendRequest(username: string): Promise<{ status: 'sent' | 'auto_accepted' }> {
    return this.fetch('/api/friend-requests', {
      method: 'POST',
      body: JSON.stringify({ username })
    });
  }

  async listFriendRequests(): Promise<FriendRequestInfo[]> {
    return this.fetch<FriendRequestInfo[]>('/api/friend-requests');
  }

  async acceptFriendRequest(id: string): Promise<void> {
    return this.fetch<void>(`/api/friend-requests/${id}/accept`, { method: 'POST' });
  }

  async declineFriendRequest(id: string): Promise<void> {
    return this.fetch<void>(`/api/friend-requests/${id}/decline`, { method: 'POST' });
  }

  async getMissingMembersCount(): Promise<number> {
    const { count } = await this.fetch<{ count: number }>('/api/friend-requests/missing-members');
    return count;
  }

  async postGuildInvite(): Promise<void> {
    return this.fetch<void>('/api/friend-requests/post-invite', { method: 'POST' });
  }

  // Availability
  async getFreeFriendsNow(): Promise<string[]> {
    const { free_friend_ids } = await this.fetch<{ free_friend_ids: string[] }>('/api/availability/friends-now');
    return free_friend_ids;
  }

  async getWeekAvailability(withFriendId: string, weekStart: string): Promise<DayAvailability[]> {
    const query = new URLSearchParams({ with: withFriendId, week_start: weekStart });
    return this.fetch<DayAvailability[]>(`/api/availability/week?${query.toString()}`);
  }
}

export const api = new ApiClient();
