pub mod user;
pub mod calendar_event;

pub use user::{User, DiscordUser};
pub use calendar_event::{
    CalendarEvent, CreateEventRequest, UpdateEventRequest, Visibility, 
    ListEventsQuery, EventParticipant, ParticipationStatus, EventWithParticipants,
    ParticipantInfo, InviteParticipantsRequest, UpdateParticipationRequest
};