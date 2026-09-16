use crate::models::{
    CalendarEvent, CreateEventRequest, EventParticipant, EventWithParticipants, ParticipantInfo,
    ParticipationStatus, UpdateEventRequest, User, Visibility,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn create_event(
    db: &PgPool,
    creator_id: Uuid,
    req: CreateEventRequest,
) -> Result<CalendarEvent> {
    let event_id = Uuid::new_v4();

    // An explicit visibility in the request always wins; the stored
    // preference is a *default*, not an override. Only looked up when the
    // request omits the field, so the common path costs no extra query.
    let visibility = match req.visibility {
        Some(explicit) => explicit,
        None => sqlx::query_scalar::<_, Visibility>(
            "SELECT default_visibility FROM users WHERE id = $1",
        )
        .bind(creator_id)
        .fetch_optional(db)
        .await?
        .unwrap_or_default(),
    };

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
    .bind(req.start_time)
    .bind(req.end_time)
    .bind(&req.location)
    .bind(&visibility)
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
        // Only fetched if there's actually someone to notify - avoids the
        // extra query for the (very common) case of a solo event.
        let creator_username: Option<String> = if participant_ids.iter().any(|id| *id != creator_id)
        {
            sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
                .bind(creator_id)
                .fetch_optional(db)
                .await?
        } else {
            None
        };

        for user_id in participant_ids {
            if user_id != creator_id {
                let inserted = sqlx::query(
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

                if inserted.rows_affected() > 0
                    && let Some(creator_username) = &creator_username
                {
                    let message = format!("{creator_username} invited you to {}", event.title);
                    if let Err(e) = crate::services::notifications::create(
                        db,
                        user_id,
                        "event_invite",
                        Some(creator_id),
                        Some(event_id),
                        &message,
                    )
                    .await
                    {
                        tracing::warn!("Failed to create invite notification: {:?}", e);
                    }
                }
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
    // Must accept exactly what list_user_events accepts, including the
    // friends-visible case. list_user_events calls this per event to build
    // its participant lists, so anything rejected here is silently dropped
    // from the listing no matter what that query matched.
    let event = sqlx::query_as::<_, CalendarEvent>(
        r#"
        SELECT e.* FROM calendar_events e
        WHERE e.id = $1
        AND (
            e.creator_id = $2
            OR EXISTS (
                SELECT 1 FROM event_participants ep
                WHERE ep.event_id = e.id AND ep.user_id = $2
            )
            OR e.visibility = 'public'
            OR (
                e.visibility = 'friends'
                AND EXISTS (
                    SELECT 1 FROM friendships f
                    WHERE f.user_id = $2 AND f.friend_id = e.creator_id
                )
            )
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
    let participant_rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            Option<String>,
            ParticipationStatus,
            Option<DateTime<Utc>>,
        ),
    >(
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
        .map(
            |(user_id, discord_id, username, avatar, status, responded_at)| ParticipantInfo {
                user_id,
                discord_id: discord_id.clone(),
                username,
                avatar_url: User::build_avatar_url(&discord_id, &avatar),
                status,
                responded_at,
            },
        )
        .collect();

    // Get my participation status
    let my_status = participants
        .iter()
        .find(|p| p.user_id == requesting_user_id)
        .map(|p| p.status.clone());

    // Derived from the participant list rather than re-queried: if you have
    // a row there at all, you were invited, whatever you answered.
    let is_participant = participants.iter().any(|p| p.user_id == requesting_user_id);

    Ok(Some(EventWithParticipants {
        event: event.clone(),
        participants,
        is_creator: event.creator_id == requesting_user_id,
        my_status,
        is_participant,
    }))
}

pub async fn list_user_events(
    db: &PgPool,
    user_id: Uuid,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    include_declined: bool,
) -> Result<Vec<EventWithParticipants>> {
    // Three ways an event can reach you, per
    // .claude/skills/event-visibility-listing/SKILL.md:
    //   1. you're a participant (invited, however you answered)
    //   2. it's public
    //   3. it's friends-visible and its creator is a friend of yours
    //
    // `friends` deliberately means *any* row in `friendships`, covering both
    // sources - guild-synced (services::friends) and explicitly accepted
    // requests (services::friend_requests). That's a product decision, not
    // an accident: see the skill for the exposure it implies.
    //
    // EXISTS rather than the old JOIN + DISTINCT: an event can qualify by
    // more than one clause at once (invited *and* public), and DISTINCT
    // over `e.*` was the only thing stopping that from double-listing it.
    let mut query = String::from(
        r#"
        SELECT e.* FROM calendar_events e
        WHERE (
            EXISTS (
                SELECT 1 FROM event_participants ep
                WHERE ep.event_id = e.id AND ep.user_id = $1
            )
            OR e.visibility = 'public'
            OR (
                e.visibility = 'friends'
                AND EXISTS (
                    SELECT 1 FROM friendships f
                    WHERE f.user_id = $1 AND f.friend_id = e.creator_id
                )
            )
        )
        "#,
    );

    // Declining has to win over every route above, or an event you turned
    // down reappears through the public/friends clause - which is exactly
    // how this would regress. Scoped to the whole row, not to one JOINed
    // participant row.
    if !include_declined {
        query.push_str(
            r#"
            AND NOT EXISTS (
                SELECT 1 FROM event_participants ep
                WHERE ep.event_id = e.id AND ep.user_id = $1 AND ep.status = 'declined'
            )
            "#,
        );
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

    let mut sql_query = sqlx::query_as::<_, CalendarEvent>(&query).bind(user_id);

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
        "SELECT * FROM calendar_events WHERE id = $1 AND creator_id = $2",
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
    if let Some(price) = req.price {
        event.price = Some(price);
    }
    if let Some(link) = req.link {
        event.link = Some(link);
    }

    // Save updated event
    let updated = sqlx::query_as::<_, CalendarEvent>(
        r#"
        UPDATE calendar_events
        SET title = $1, description = $2, start_time = $3, end_time = $4,
            location = $5, visibility = $6, price = $7, link = $8, updated_at = $9
        WHERE id = $10 AND creator_id = $11
        RETURNING *
        "#,
    )
    .bind(&event.title)
    .bind(&event.description)
    .bind(event.start_time)
    .bind(event.end_time)
    .bind(&event.location)
    .bind(&event.visibility)
    .bind(&event.price)
    .bind(&event.link)
    .bind(Utc::now())
    .bind(event_id)
    .bind(creator_id)
    .fetch_one(db)
    .await?;

    Ok(Some(updated))
}

pub async fn delete_event(db: &PgPool, event_id: Uuid, creator_id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM calendar_events WHERE id = $1 AND creator_id = $2")
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
        "SELECT * FROM calendar_events WHERE id = $1 AND creator_id = $2",
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
    .bind(status.clone())
    .bind(Utc::now())
    .bind(event_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    // Notify the event's creator that someone responded - not fatal to the
    // request if this fails, the RSVP itself already succeeded above, and
    // skip entirely when the responder *is* the creator (their own RSVP on
    // their own event isn't news to them).
    if participant.is_some()
        && let Err(e) = notify_creator_of_rsvp(db, event_id, user_id, &status).await
    {
        tracing::warn!("Failed to create RSVP-change notification: {:?}", e);
    }

    Ok(participant)
}

async fn notify_creator_of_rsvp(
    db: &PgPool,
    event_id: Uuid,
    responder_id: Uuid,
    status: &ParticipationStatus,
) -> Result<()> {
    let event = sqlx::query_as::<_, CalendarEvent>("SELECT * FROM calendar_events WHERE id = $1")
        .bind(event_id)
        .fetch_optional(db)
        .await?;
    let Some(event) = event else { return Ok(()) };

    if event.creator_id == responder_id {
        return Ok(());
    }

    let username: Option<String> = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(responder_id)
        .fetch_optional(db)
        .await?;
    let Some(username) = username else {
        return Ok(());
    };

    let message = match status {
        ParticipationStatus::Accepted => {
            format!("{username} is going to your event {}", event.title)
        }
        ParticipationStatus::Declined => {
            format!("{username} can't make it to your event {}", event.title)
        }
        ParticipationStatus::Maybe => {
            format!("{username} might come to your event {}", event.title)
        }
        ParticipationStatus::Pending => format!(
            "{username} reset their response for your event {}",
            event.title
        ),
    };

    crate::services::notifications::create(
        db,
        event.creator_id,
        "rsvp_change",
        Some(responder_id),
        Some(event_id),
        &message,
    )
    .await
}

pub async fn remove_participant(
    db: &PgPool,
    event_id: Uuid,
    creator_id: Uuid,
    user_id: Uuid,
) -> Result<bool> {
    // Verify user is the creator
    let is_creator = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM calendar_events WHERE id = $1 AND creator_id = $2)",
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

    let result = sqlx::query("DELETE FROM event_participants WHERE event_id = $1 AND user_id = $2")
        .bind(event_id)
        .bind(user_id)
        .execute(db)
        .await?;

    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateEventRequest;
    use crate::services::notifications;

    async fn seed_user(db: &PgPool, discord_id: &str, username: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (id, discord_id, username, created_at, updated_at)
            VALUES ($1, $2, $3, now(), now())
            "#,
        )
        .bind(id)
        .bind(discord_id)
        .bind(username)
        .execute(db)
        .await
        .unwrap();
        id
    }

    // `default_visibility` on `users` was written by the settings page and
    // read by nobody - create_event hardcoded a Private fallback. These two
    // pin down the intended relationship: the preference supplies the
    // default, an explicit request value still overrides it.
    #[sqlx::test]
    async fn create_event_falls_back_to_the_creators_default_visibility(db: PgPool) {
        let creator = seed_user(&db, "creator", "creator").await;
        sqlx::query("UPDATE users SET default_visibility = 'public' WHERE id = $1")
            .bind(creator)
            .execute(&db)
            .await
            .unwrap();

        let mut req = minimal_request("Party", None);
        req.visibility = None;

        let event = create_event(&db, creator, req).await.unwrap();
        assert_eq!(event.visibility, Visibility::Public);
    }

    #[sqlx::test]
    async fn create_event_lets_an_explicit_visibility_beat_the_default(db: PgPool) {
        let creator = seed_user(&db, "creator", "creator").await;
        sqlx::query("UPDATE users SET default_visibility = 'public' WHERE id = $1")
            .bind(creator)
            .execute(&db)
            .await
            .unwrap();

        let mut req = minimal_request("Secret", None);
        req.visibility = Some(Visibility::Private);

        let event = create_event(&db, creator, req).await.unwrap();
        assert_eq!(event.visibility, Visibility::Private);
    }

    async fn befriend(db: &PgPool, a: Uuid, b: Uuid, source: &str) {
        // Symmetric, like both real writers: services::friends' guild sync
        // and services::friend_requests' accept path each insert both
        // directions.
        for (x, y) in [(a, b), (b, a)] {
            sqlx::query(
                r#"
                INSERT INTO friendships (id, user_id, friend_id, source, synced_at, created_at)
                VALUES ($1, $2, $3, $4, now(), now())
                ON CONFLICT (user_id, friend_id) DO NOTHING
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(x)
            .bind(y)
            .bind(source)
            .execute(db)
            .await
            .unwrap();
        }
    }

    async fn create_with_visibility(
        db: &PgPool,
        creator: Uuid,
        title: &str,
        v: Visibility,
    ) -> Uuid {
        let mut req = minimal_request(title, None);
        req.visibility = Some(v);
        create_event(db, creator, req).await.unwrap().id
    }

    // `visibility` had no effect on listing at all before 2026-09-16: the
    // query was participant-only, so a "public" event reached exactly the
    // people a "private" one would.
    #[sqlx::test]
    async fn listing_surfaces_public_events_to_non_participants(db: PgPool) {
        let creator = seed_user(&db, "creator", "creator").await;
        let stranger = seed_user(&db, "stranger", "stranger").await;

        create_with_visibility(&db, creator, "Public party", Visibility::Public).await;

        let listed = list_user_events(&db, stranger, None, None, false)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].event.title, "Public party");
        assert!(
            !listed[0].is_participant,
            "a discovered event must not look like one you were invited to"
        );
        assert!(listed[0].my_status.is_none());
    }

    #[sqlx::test]
    async fn listing_hides_private_events_from_non_participants(db: PgPool) {
        let creator = seed_user(&db, "creator", "creator").await;
        let stranger = seed_user(&db, "stranger", "stranger").await;
        befriend(&db, creator, stranger, "discord_guild").await;

        create_with_visibility(&db, creator, "Secret", Visibility::Private).await;

        let listed = list_user_events(&db, stranger, None, None, false)
            .await
            .unwrap();
        assert!(
            listed.is_empty(),
            "private must stay private even between friends"
        );
    }

    // The product decision recorded in the skill: `friends` covers BOTH
    // friendship sources, not just explicitly accepted requests.
    #[sqlx::test]
    async fn listing_surfaces_friends_events_to_friends_of_either_source(db: PgPool) {
        let creator = seed_user(&db, "creator", "creator").await;
        let guild_mate = seed_user(&db, "guildmate", "guildmate").await;
        let accepted = seed_user(&db, "accepted", "accepted").await;
        let stranger = seed_user(&db, "stranger", "stranger").await;

        befriend(&db, creator, guild_mate, "discord_guild").await;
        befriend(&db, creator, accepted, "friend_request").await;

        create_with_visibility(&db, creator, "Friends only", Visibility::Friends).await;

        for (who, label) in [(guild_mate, "guild-synced"), (accepted, "accepted request")] {
            let listed = list_user_events(&db, who, None, None, false).await.unwrap();
            assert_eq!(
                listed.len(),
                1,
                "{label} friend should see a friends-visible event"
            );
        }

        let listed = list_user_events(&db, stranger, None, None, false)
            .await
            .unwrap();
        assert!(
            listed.is_empty(),
            "a non-friend must not see a friends-visible event"
        );
    }

    // The easiest regression to ship by accident: the declined filter used
    // to live on the JOINed participant row, so an event you turned down
    // could reappear through the new public/friends clause.
    #[sqlx::test]
    async fn declining_still_hides_an_event_that_is_also_publicly_visible(db: PgPool) {
        let creator = seed_user(&db, "creator", "creator").await;
        let invitee = seed_user(&db, "invitee", "invitee").await;

        let mut req = minimal_request("Public but declined", Some(vec![invitee]));
        req.visibility = Some(Visibility::Public);
        let event = create_event(&db, creator, req).await.unwrap();

        update_participation_status(&db, event.id, invitee, ParticipationStatus::Declined)
            .await
            .unwrap();

        let listed = list_user_events(&db, invitee, None, None, false)
            .await
            .unwrap();
        assert!(
            listed.is_empty(),
            "declining must win over public visibility"
        );

        let listed = list_user_events(&db, invitee, None, None, true)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1, "include_declined should still return it");
    }

    #[sqlx::test]
    async fn listing_does_not_duplicate_an_event_that_matches_twice(db: PgPool) {
        let creator = seed_user(&db, "creator", "creator").await;
        let invitee = seed_user(&db, "invitee", "invitee").await;
        befriend(&db, creator, invitee, "discord_guild").await;

        // Invited AND public AND friend-of-creator - three routes, one event.
        let mut req = minimal_request("Everything at once", Some(vec![invitee]));
        req.visibility = Some(Visibility::Public);
        create_event(&db, creator, req).await.unwrap();

        let listed = list_user_events(&db, invitee, None, None, false)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].is_participant);
    }

    fn minimal_request(title: &str, participant_ids: Option<Vec<Uuid>>) -> CreateEventRequest {
        let now = Utc::now();
        CreateEventRequest {
            title: title.to_string(),
            description: None,
            start_time: now,
            end_time: now + chrono::Duration::hours(1),
            location: None,
            visibility: None,
            participant_ids,
            price: None,
            link: None,
        }
    }

    // These cover the notification *trigger wiring* itself (create_event /
    // update_participation_status calling services::notifications::create
    // at the right moments) - services::notifications's own module has the
    // create/list/mark-read unit coverage. See
    // .claude/skills/mockup-notifications/SKILL.md for why both matter:
    // testing services::notifications in isolation wouldn't catch a bug
    // where these call sites forgot to invoke it, or notified the wrong
    // person.

    #[sqlx::test]
    async fn create_event_notifies_invited_participants_but_not_the_creator(db: PgPool) {
        let creator = seed_user(&db, "creator-discord", "creator").await;
        let invitee = seed_user(&db, "invitee-discord", "invitee").await;

        create_event(
            &db,
            creator,
            minimal_request("Board games", Some(vec![invitee])),
        )
        .await
        .unwrap();

        let invitee_notifications = notifications::list(&db, invitee, 10).await.unwrap();
        assert_eq!(invitee_notifications.len(), 1);
        assert_eq!(invitee_notifications[0].kind, "event_invite");
        assert!(invitee_notifications[0].message.contains("Board games"));
        assert_eq!(
            invitee_notifications[0].actor_username.as_deref(),
            Some("creator")
        );

        // The creator doesn't get notified about their own event.
        assert!(
            notifications::list(&db, creator, 10)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test]
    async fn create_event_with_no_participants_notifies_nobody(db: PgPool) {
        let creator = seed_user(&db, "creator-discord", "creator").await;

        create_event(&db, creator, minimal_request("Solo errand", None))
            .await
            .unwrap();

        assert!(
            notifications::list(&db, creator, 10)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test]
    async fn rsvp_change_notifies_the_creator_but_not_when_they_answer_their_own_event(db: PgPool) {
        let creator = seed_user(&db, "creator-discord", "creator").await;
        let friend = seed_user(&db, "friend-discord", "friend").await;

        let event = create_event(
            &db,
            creator,
            minimal_request("Raclette night", Some(vec![friend])),
        )
        .await
        .unwrap();

        update_participation_status(&db, event.id, friend, ParticipationStatus::Accepted)
            .await
            .unwrap();

        let creator_notifications = notifications::list(&db, creator, 10).await.unwrap();
        assert_eq!(creator_notifications.len(), 1);
        assert_eq!(creator_notifications[0].kind, "rsvp_change");
        assert!(creator_notifications[0].message.contains("Raclette night"));
        assert!(creator_notifications[0].message.contains("friend"));

        // The creator responding to their own event's participant row
        // (they're auto-added as `accepted`) doesn't notify themselves.
        update_participation_status(&db, event.id, creator, ParticipationStatus::Maybe)
            .await
            .unwrap();
        assert_eq!(
            notifications::list(&db, creator, 10).await.unwrap().len(),
            1
        );
    }
}
