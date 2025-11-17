use crate::models::{
    User,
    CalendarEvent, CreateEventRequest, UpdateEventRequest, Visibility,
    EventParticipant, ParticipationStatus, EventWithParticipants, ParticipantInfo
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Result;

pub async fn create_event(
    db: &PgPool,
    creator_id: Uuid,
    req: CreateEventRequest,
) -> Result<CalendarEvent> {
    let event_id = Uuid::new_v4();
    
    let event = sqlx::query_as::<_, CalendarEvent>(
        r#"
        INSERT INTO calendar_events 
            (id, creator_id, title, description, start_time, end_time, location, visibility, price, link, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING *
        "#,
    )
    .bind(event_id)
    .bind(creator_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(&req.start_time)
    .bind(&req.end_time)
    .bind(&req.location)
    .bind(req.visibility.unwrap_or(Visibility::Private))
    .bind(&req.price)
    .bind(&req.link)
    .bind(Utc::now())
    .bind(Utc::now())
    .fetch_one(db)
    .await?;

    // Add creator as accepted participant
    sqlx::query(
        r#"
        INSERT INTO event_participants (id, event_id, user_id, status, invited_at, responded_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(event_id)
    .bind(creator_id)
    .bind(ParticipationStatus::Accepted)
    .bind(Utc::now())
    .bind(Some(Utc::now()))
    .execute(db)
    .await?;

    // Invite other participants if provided
    if let Some(participant_ids) = req.participant_ids {
        for user_id in participant_ids {
            if user_id != creator_id {
                sqlx::query(
                    r#"
                    INSERT INTO event_participants (id, event_id, user_id, status, invited_at)
                    VALUES ($1, $2, $3, $4, $5)
                    ON CONFLICT (event_id, user_id) DO NOTHING
                    "#,
                )
                .bind(Uuid::new_v4())
                .bind(event_id)
                .bind(user_id)
                .bind(ParticipationStatus::Pending)
                .bind(Utc::now())
                .execute(db)
                .await?;
            }
        }
    }

    Ok(event)
}

pub async fn get_event_with_participants(
    db: &PgPool,
    event_id: Uuid,
    requesting_user_id: Uuid,
) -> Result<Option<EventWithParticipants>> {
    // Get the event
    let event = sqlx::query_as::<_, CalendarEvent>(
        r#"
        SELECT e.* FROM calendar_events e
        LEFT JOIN event_participants ep ON e.id = ep.event_id
        WHERE e.id = $1 
        AND (
            e.creator_id = $2 
            OR ep.user_id = $2
            OR e.visibility = 'public'
        )
        LIMIT 1
        "#,
    )
    .bind(event_id)
    .bind(requesting_user_id)
    .fetch_optional(db)
    .await?;

    let Some(event) = event else {
        return Ok(None);
    };

    // Get participants with user info
    let participant_rows = sqlx::query_as::<_, (Uuid, String, String, Option<String>, ParticipationStatus, Option<DateTime<Utc>>)>(
        r#"
        SELECT u.id, u.discord_id, u.username, u.avatar, ep.status, ep.responded_at
        FROM event_participants ep
        JOIN users u ON ep.user_id = u.id
        WHERE ep.event_id = $1
        ORDER BY ep.invited_at ASC
        "#,
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;

    let participants: Vec<ParticipantInfo> = participant_rows
        .into_iter()
        .map(|(user_id, discord_id, username, avatar, status, responded_at)| ParticipantInfo {
            user_id,
            discord_id: discord_id.clone(),
            username,
            avatar_url: User::build_avatar_url(&discord_id, &avatar),
            status,
            responded_at,
        })
    .collect();

    // Get my participation status
    let my_status = participants
        .iter()
        .find(|p| p.user_id == requesting_user_id)
        .map(|p| p.status.clone());

    Ok(Some(EventWithParticipants {
        event: event.clone(),
        participants,
        is_creator: event.creator_id == requesting_user_id,
        my_status,
    }))
}

pub async fn list_user_events(
    db: &PgPool,
    user_id: Uuid,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    include_declined: bool,
) -> Result<Vec<EventWithParticipants>> {
    // Get all events where user is a participant
    let mut query = String::from(
        r#"
        SELECT DISTINCT e.* FROM calendar_events e
        JOIN event_participants ep ON e.id = ep.event_id
        WHERE ep.user_id = $1
        "#
    );
    
    if !include_declined {
        query.push_str(" AND ep.status != 'declined'");
    }
    
    let mut param_count = 1;
    
    if start_date.is_some() {
        param_count += 1;
        query.push_str(&format!(" AND e.start_time >= ${}", param_count));
    }
    
    if end_date.is_some() {
        param_count += 1;
        query.push_str(&format!(" AND e.end_time <= ${}", param_count));
    }
    
    query.push_str(" ORDER BY e.start_time ASC");

    let mut sql_query = sqlx::query_as::<_, CalendarEvent>(&query)
        .bind(user_id);
    
    if let Some(start) = start_date {
        sql_query = sql_query.bind(start);
    }
    
    if let Some(end) = end_date {
        sql_query = sql_query.bind(end);
    }

    let events = sql_query.fetch_all(db).await?;

    // For each event, get participants
    let mut events_with_participants = Vec::new();
    for event in events {
        if let Some(event_with_parts) = get_event_with_participants(db, event.id, user_id).await? {
            events_with_participants.push(event_with_parts);
        }
    }

    Ok(events_with_participants)
}

pub async fn update_event(
    db: &PgPool,
    event_id: Uuid,
    creator_id: Uuid,
    req: UpdateEventRequest,
) -> Result<Option<CalendarEvent>> {
    // Check if user is the creator
    let existing = sqlx::query_as::<_, CalendarEvent>(
        "SELECT * FROM calendar_events WHERE id = $1 AND creator_id = $2"
    )
    .bind(event_id)
    .bind(creator_id)
    .fetch_optional(db)
    .await?;

    let Some(mut event) = existing else {
        return Ok(None);
    };

    // Update fields if provided
    if let Some(title) = req.title {
        event.title = title;
    }
    if let Some(description) = req.description {
        event.description = Some(description);
    }
    if let Some(start_time) = req.start_time {
        event.start_time = start_time;
    }
    if let Some(end_time) = req.end_time {
        event.end_time = end_time;
    }
    if let Some(location) = req.location {
        event.location = Some(location);
    }
    if let Some(visibility) = req.visibility {
        event.visibility = visibility;
    }

    // Save updated event
    let updated = sqlx::query_as::<_, CalendarEvent>(
        r#"
        UPDATE calendar_events 
        SET title = $1, description = $2, start_time = $3, end_time = $4, 
            location = $5, visibility = $6, updated_at = $7
        WHERE id = $8 AND creator_id = $9
        RETURNING *
        "#,
    )
    .bind(&event.title)
    .bind(&event.description)
    .bind(&event.start_time)
    .bind(&event.end_time)
    .bind(&event.location)
    .bind(&event.visibility)
    .bind(Utc::now())
    .bind(event_id)
    .bind(creator_id)
    .fetch_one(db)
    .await?;

    Ok(Some(updated))
}

pub async fn delete_event(
    db: &PgPool,
    event_id: Uuid,
    creator_id: Uuid,
) -> Result<bool> {
    let result = sqlx::query(
        "DELETE FROM calendar_events WHERE id = $1 AND creator_id = $2"
    )
    .bind(event_id)
    .bind(creator_id)
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn invite_participants(
    db: &PgPool,
    event_id: Uuid,
    creator_id: Uuid,
    user_ids: Vec<Uuid>,
) -> Result<Vec<EventParticipant>> {
    // Verify user is the creator
    let event = sqlx::query_as::<_, CalendarEvent>(
        "SELECT * FROM calendar_events WHERE id = $1 AND creator_id = $2"
    )
    .bind(event_id)
    .bind(creator_id)
    .fetch_optional(db)
    .await?;

    if event.is_none() {
        return Ok(Vec::new());
    }

    let mut participants = Vec::new();
    for user_id in user_ids {
        let participant = sqlx::query_as::<_, EventParticipant>(
            r#"
            INSERT INTO event_participants (id, event_id, user_id, status, invited_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (event_id, user_id) DO NOTHING
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(event_id)
        .bind(user_id)
        .bind(ParticipationStatus::Pending)
        .bind(Utc::now())
        .fetch_optional(db)
        .await?;

        if let Some(p) = participant {
            participants.push(p);
        }
    }

    Ok(participants)
}

pub async fn update_participation_status(
    db: &PgPool,
    event_id: Uuid,
    user_id: Uuid,
    status: ParticipationStatus,
) -> Result<Option<EventParticipant>> {
    let participant = sqlx::query_as::<_, EventParticipant>(
        r#"
        UPDATE event_participants
        SET status = $1, responded_at = $2
        WHERE event_id = $3 AND user_id = $4
        RETURNING *
        "#,
    )
    .bind(status)
    .bind(Utc::now())
    .bind(event_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    Ok(participant)
}

pub async fn remove_participant(
    db: &PgPool,
    event_id: Uuid,
    creator_id: Uuid,
    user_id: Uuid,
) -> Result<bool> {
    // Verify user is the creator
    let is_creator = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM calendar_events WHERE id = $1 AND creator_id = $2)"
    )
    .bind(event_id)
    .bind(creator_id)
    .fetch_one(db)
    .await?;

    if !is_creator {
        return Ok(false);
    }

    // Don't allow removing the creator
    if user_id == creator_id {
        return Ok(false);
    }

    let result = sqlx::query(
        "DELETE FROM event_participants WHERE event_id = $1 AND user_id = $2"
    )
    .bind(event_id)
    .bind(user_id)
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}