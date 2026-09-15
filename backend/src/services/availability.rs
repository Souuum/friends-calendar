use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Per-day free/busy for a set of users, one entry per day starting at
/// `week_start`. "Free" that day means zero accepted/maybe events overlap
/// it at all - day granularity, not hour-by-hour, matching what the
/// mockup's weekly strip actually shows (one colored cell per day, not a
/// full calendar grid). See .claude/skills/mockup-availability/SKILL.md.
#[derive(Debug, Clone, PartialEq)]
pub struct DayAvailability {
    pub date: DateTime<Utc>,
    pub free_user_ids: Vec<Uuid>,
}

/// A user is "busy" during an event if they're `accepted` or `maybe` on
/// it - `declined`/`pending` don't block. Fetches every such event
/// overlapping [window_start, window_end) for the given users in one
/// query; the actual free/busy computation is pure logic below (unit
/// tested directly, no database needed for that part).
async fn fetch_busy_intervals(
    db: &PgPool,
    user_ids: &[Uuid],
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> Result<Vec<(Uuid, DateTime<Utc>, DateTime<Utc>)>> {
    let rows = sqlx::query_as::<_, (Uuid, DateTime<Utc>, DateTime<Utc>)>(
        r#"
        SELECT ep.user_id, e.start_time, e.end_time
        FROM event_participants ep
        JOIN calendar_events e ON e.id = ep.event_id
        WHERE ep.user_id = ANY($1)
          AND ep.status IN ('accepted', 'maybe')
          AND e.start_time < $3
          AND e.end_time > $2
        "#,
    )
    .bind(user_ids)
    .bind(window_start)
    .bind(window_end)
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// Pure: which of `user_ids` have no busy interval covering the instant
/// `at`. No I/O - see .claude/skills/add-tests/SKILL.md's unit tier.
fn compute_free_users_at(
    busy: &[(Uuid, DateTime<Utc>, DateTime<Utc>)],
    user_ids: &[Uuid],
    at: DateTime<Utc>,
) -> Vec<Uuid> {
    user_ids
        .iter()
        .copied()
        .filter(|user_id| {
            !busy
                .iter()
                .any(|(busy_user, start, end)| busy_user == user_id && *start <= at && at < *end)
        })
        .collect()
}

/// Pure: per-day free/busy over a 7-day window starting at `week_start`
/// (should be a day boundary, but any instant works - each day's window
/// is computed from it). A user is free that day iff none of their busy
/// intervals overlap [day_start, day_end).
fn compute_free_users_per_day(
    busy: &[(Uuid, DateTime<Utc>, DateTime<Utc>)],
    user_ids: &[Uuid],
    week_start: DateTime<Utc>,
) -> Vec<DayAvailability> {
    (0..7)
        .map(|day_offset| {
            let day_start = week_start + Duration::days(day_offset);
            let day_end = day_start + Duration::days(1);

            let free_user_ids = user_ids
                .iter()
                .copied()
                .filter(|user_id| {
                    !busy.iter().any(|(busy_user, start, end)| {
                        busy_user == user_id && *start < day_end && *end > day_start
                    })
                })
                .collect();

            DayAvailability {
                date: day_start,
                free_user_ids,
            }
        })
        .collect()
}

/// Which of `candidate_ids` are free right now.
pub async fn free_users_now(db: &PgPool, candidate_ids: &[Uuid]) -> Result<Vec<Uuid>> {
    if candidate_ids.is_empty() {
        return Ok(Vec::new());
    }
    let now = Utc::now();
    let busy = fetch_busy_intervals(db, candidate_ids, now, now).await?;
    Ok(compute_free_users_at(&busy, candidate_ids, now))
}

/// Per-day free/busy for `user_ids` over the 7 days starting `week_start`.
pub async fn week_availability(
    db: &PgPool,
    user_ids: &[Uuid],
    week_start: DateTime<Utc>,
) -> Result<Vec<DayAvailability>> {
    if user_ids.is_empty() {
        return Ok((0..7)
            .map(|d| DayAvailability {
                date: week_start + Duration::days(d),
                free_user_ids: vec![],
            })
            .collect());
    }
    let window_end = week_start + Duration::days(7);
    let busy = fetch_busy_intervals(db, user_ids, week_start, window_end).await?;
    Ok(compute_free_users_per_day(&busy, user_ids, week_start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn dt(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn free_users_at_excludes_anyone_with_an_overlapping_event() {
        let alice = Uuid::new_v4();
        let bob = Uuid::new_v4();
        let busy = vec![(alice, dt(2026, 3, 1, 18), dt(2026, 3, 1, 20))];

        let free = compute_free_users_at(&busy, &[alice, bob], dt(2026, 3, 1, 19));
        assert_eq!(free, vec![bob]);
    }

    #[test]
    fn free_users_at_treats_interval_end_as_exclusive() {
        let alice = Uuid::new_v4();
        let busy = vec![(alice, dt(2026, 3, 1, 18), dt(2026, 3, 1, 20))];

        // Exactly at the event's end - it's over, alice is free.
        let free = compute_free_users_at(&busy, &[alice], dt(2026, 3, 1, 20));
        assert_eq!(free, vec![alice]);

        // One minute before the end - still busy.
        let free =
            compute_free_users_at(&busy, &[alice], dt(2026, 3, 1, 20) - Duration::minutes(1));
        assert!(free.is_empty());
    }

    #[test]
    fn free_users_at_with_no_events_is_everyone() {
        let alice = Uuid::new_v4();
        let bob = Uuid::new_v4();
        let free = compute_free_users_at(&[], &[alice, bob], dt(2026, 3, 1, 12));
        assert_eq!(free.len(), 2);
    }

    #[test]
    fn free_users_per_day_marks_a_day_busy_if_any_event_overlaps_it() {
        let alice = Uuid::new_v4();
        let week_start = dt(2026, 3, 2, 0); // a Monday, midnight
        let busy = vec![(alice, dt(2026, 3, 4, 10), dt(2026, 3, 4, 12))]; // Wednesday

        let days = compute_free_users_per_day(&busy, &[alice], week_start);
        assert_eq!(days.len(), 7);
        assert_eq!(days[0].free_user_ids, vec![alice]); // Monday: free
        assert_eq!(days[1].free_user_ids, vec![alice]); // Tuesday: free
        assert!(days[2].free_user_ids.is_empty()); // Wednesday: busy
        assert_eq!(days[3].free_user_ids, vec![alice]); // Thursday: free
    }

    #[test]
    fn free_users_per_day_handles_an_event_spanning_midnight() {
        let alice = Uuid::new_v4();
        let week_start = dt(2026, 3, 2, 0);
        // Monday 22:00 to Tuesday 02:00 - touches both days.
        let busy = vec![(alice, dt(2026, 3, 2, 22), dt(2026, 3, 3, 2))];

        let days = compute_free_users_per_day(&busy, &[alice], week_start);
        assert!(days[0].free_user_ids.is_empty(), "Monday should be busy");
        assert!(days[1].free_user_ids.is_empty(), "Tuesday should be busy");
        assert_eq!(
            days[2].free_user_ids,
            vec![alice],
            "Wednesday should be free"
        );
    }

    #[test]
    fn free_users_per_day_with_no_users_returns_seven_empty_days() {
        let days = compute_free_users_per_day(&[], &[], dt(2026, 3, 2, 0));
        assert_eq!(days.len(), 7);
        assert!(days.iter().all(|d| d.free_user_ids.is_empty()));
    }

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

    async fn seed_event(
        db: &PgPool,
        creator_id: Uuid,
        participant_id: Uuid,
        status: crate::models::ParticipationStatus,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) {
        let event_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO calendar_events (id, creator_id, title, start_time, end_time, visibility, created_at, updated_at)
            VALUES ($1, $2, 'test event', $3, $4, 'private', now(), now())
            "#,
        )
        .bind(event_id)
        .bind(creator_id)
        .bind(start)
        .bind(end)
        .execute(db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO event_participants (id, event_id, user_id, status, invited_at, responded_at)
            VALUES ($1, $2, $3, $4, now(), now())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(event_id)
        .bind(participant_id)
        .bind(status)
        .execute(db)
        .await
        .unwrap();
    }

    #[sqlx::test]
    async fn free_users_now_reflects_accepted_and_maybe_but_not_declined(db: PgPool) {
        let alice = seed_user(&db, "alice-discord", "alice").await;
        let bob = seed_user(&db, "bob-discord", "bob").await;
        let carol = seed_user(&db, "carol-discord", "carol").await;

        let now = Utc::now();
        seed_event(
            &db,
            alice,
            alice,
            crate::models::ParticipationStatus::Accepted,
            now - Duration::minutes(30),
            now + Duration::minutes(30),
        )
        .await;
        seed_event(
            &db,
            bob,
            bob,
            crate::models::ParticipationStatus::Maybe,
            now - Duration::minutes(30),
            now + Duration::minutes(30),
        )
        .await;
        seed_event(
            &db,
            carol,
            carol,
            crate::models::ParticipationStatus::Declined,
            now - Duration::minutes(30),
            now + Duration::minutes(30),
        )
        .await;

        let free = free_users_now(&db, &[alice, bob, carol]).await.unwrap();
        assert_eq!(free, vec![carol]);
    }

    #[sqlx::test]
    async fn week_availability_returns_seven_days_from_real_data(db: PgPool) {
        let alice = seed_user(&db, "alice-discord", "alice").await;
        let week_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        seed_event(
            &db,
            alice,
            alice,
            crate::models::ParticipationStatus::Accepted,
            week_start + Duration::days(2) + Duration::hours(10),
            week_start + Duration::days(2) + Duration::hours(12),
        )
        .await;

        let days = week_availability(&db, &[alice], week_start).await.unwrap();
        assert_eq!(days.len(), 7);
        assert!(days[2].free_user_ids.is_empty());
        assert_eq!(days[0].free_user_ids, vec![alice]);
    }
}
