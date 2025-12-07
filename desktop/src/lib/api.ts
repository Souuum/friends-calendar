import type { User, CalendarEvent, EventWithParticipants, ParticipantInfo } from './types';

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

  // Events
  async createEvent(data: {
    title: string;
    description?: string;
    start_time: string;
    end_time: string;
    location?: string;
    visibility?: 'Private' | 'Friends' | 'Public';
    participant_ids?: string[];
  }): Promise<CalendarEvent> {
    return this.fetch<CalendarEvent>('/api/events', {
      method: 'POST',
      body: JSON.stringify(data)
    });
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
      visibility: 'private' | 'friends' | 'public';
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
}

export const api = new ApiClient();
