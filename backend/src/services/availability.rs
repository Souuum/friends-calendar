use anyhow::Result;
use chrono::Duration;
use chrono::{DateTime, Datelike, DurationRound, Timelike, Utc};
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

    // Plus anything imported from a real calendar, if they've connected one.
    //
    // ⚠️ This union is the *entire* integration point for external
    // calendars. Every consumer below takes plain (user_id, start, end)
    // tuples and neither knows nor cares where a row came from - which is
    // why importing a work calendar improves best-overlap, the week strip
    // and "free tonight" without any of them changing. See
    // services::external_calendar.
    let external = sqlx::query_as::<_, (Uuid, DateTime<Utc>, DateTime<Utc>)>(
        r#"
        SELECT c.user_id, b.starts_at, b.ends_at
        FROM external_busy b
        JOIN external_calendars c ON c.id = b.calendar_id
        WHERE c.user_id = ANY($1)
          AND b.starts_at < $3
          AND b.ends_at > $2
        "#,
    )
    .bind(user_ids)
    .bind(window_start)
    .bind(window_end)
    .fetch_all(db)
    .await?;

    let mut rows = rows;
    rows.extend(external);

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

// --- Slot ranking -------------------------------------------------------
//
// ⚠️ Everything above is **day granularity on purpose** (see
// `compute_free_users_per_day`'s doc): it backs the mockup's weekly strip,
// one coloured cell per day, and it is correct as it stands. It cannot
// answer "Fri **20:00**" - somebody with a 09:00 dentist appointment is
// "busy Friday" and would be excluded from every Friday-evening suggestion,
// which is exactly the population this feature exists to find.
//
// So this is a second computation *beside* it, not a replacement.

/// A candidate time, and who is free for the whole of it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Slot {
    pub start: DateTime<Utc>,
    pub free_user_ids: Vec<Uuid>,
}

/// Local hours a suggestion may start at.
///
/// Not "every 30 minutes across the week": that is 336 candidates of mostly
/// nonsense (03:00 Tuesday), and a suggestion nobody would act on is noise.
/// Evenings every day, plus weekend afternoons - the shape of when this kind
/// of group actually meets.
fn candidate_local_hours(weekday: chrono::Weekday) -> &'static [u32] {
    use chrono::Weekday::{Sat, Sun};
    match weekday {
        Sat | Sun => &[12, 14, 16, 18, 19, 20, 21],
        _ => &[18, 19, 20, 21],
    }
}

/// Free for the **whole** slot, not merely at its start - a slot that
/// begins in a gap and runs into an event is not a time you can meet.
fn free_for_interval(
    busy: &[(Uuid, DateTime<Utc>, DateTime<Utc>)],
    user_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> bool {
    // Half-open on both sides, matching `compute_free_users_at` above: an
    // event that ends exactly when the slot starts does not block it.
    !busy.iter().any(|(busy_user, busy_start, busy_end)| {
        *busy_user == user_id && *busy_start < end && start < *busy_end
    })
}

/// Pure: candidate slots in `[from, to)` ranked by how many of `user_ids`
/// are free for all of each one.
///
/// `tz_offset_minutes` is the **requester's** offset from UTC, so "evening"
/// means their evening. ⚠️ It is a fixed offset rather than a timezone, so a
/// window spanning a daylight-saving change is out by an hour on the far
/// side of it. Within the week this is asked about that is a rounding error,
/// and the alternative is a timezone database for one hour a year.
///
/// Ties break toward the **soonest** slot, and the order is total, so the
/// suggestion doesn't shuffle between page loads for no reason.
pub fn rank_slots(
    busy: &[(Uuid, DateTime<Utc>, DateTime<Utc>)],
    user_ids: &[Uuid],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    slot_minutes: i64,
    tz_offset_minutes: i64,
) -> Vec<Slot> {
    if user_ids.is_empty() || slot_minutes <= 0 || to <= from {
        return Vec::new();
    }

    let offset = Duration::minutes(tz_offset_minutes);
    let duration = Duration::minutes(slot_minutes);

    // Walk hour by hour and keep the candidates. At most ~168 steps for a
    // week, which costs nothing and avoids fiddly local-midnight arithmetic
    // around the ends of the window.
    let mut slots: Vec<Slot> = Vec::new();
    let mut cursor = from
        .with_timezone(&Utc)
        .duration_trunc(Duration::hours(1))
        .unwrap_or(from);
    if cursor < from {
        cursor += Duration::hours(1);
    }

    while cursor < to {
        let local = cursor + offset;
        let hour = local.time().hour();
        let weekday = local.weekday();

        if candidate_local_hours(weekday).contains(&hour) {
            let end = cursor + duration;
            let free: Vec<Uuid> = user_ids
                .iter()
                .copied()
                .filter(|id| free_for_interval(busy, *id, cursor, end))
                .collect();
            slots.push(Slot {
                start: cursor,
                free_user_ids: free,
            });
        }

        cursor += Duration::hours(1);
    }

    slots.sort_by(|a, b| {
        b.free_user_ids
            .len()
            .cmp(&a.free_user_ids.len())
            .then(a.start.cmp(&b.start))
    });

    slots
}

/// The best times for `user_ids` to meet in `[from, to)`.
///
/// Returns several rather than one: the calendar bar shows the top slot but
/// the create form wants alternatives, and two endpoints for one computation
/// would drift.
pub async fn best_slots(
    db: &PgPool,
    user_ids: &[Uuid],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    slot_minutes: i64,
    tz_offset_minutes: i64,
    limit: usize,
) -> Result<Vec<Slot>> {
    if user_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Reuses the one query the day path already has - no second query shape
    // for the same facts.
    let busy = fetch_busy_intervals(db, user_ids, from, to).await?;
    let mut ranked = rank_slots(&busy, user_ids, from, to, slot_minutes, tz_offset_minutes);
    ranked.truncate(limit);

    Ok(ranked)
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
    // --- rank_slots: a pure interval problem, which is where the bugs are ---

    /// 2026-03-02 is a Monday. UTC throughout unless a test says otherwise.
    fn mon(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, 2, hour, 0, 0).unwrap()
    }
    fn sat(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, 7, hour, 0, 0).unwrap()
    }

    fn rank(
        busy: &[(Uuid, DateTime<Utc>, DateTime<Utc>)],
        users: &[Uuid],
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Vec<Slot> {
        rank_slots(busy, users, from, to, 120, 0)
    }

    #[test]
    fn someone_with_nothing_on_is_free_in_every_candidate_slot() {
        let user = Uuid::new_v4();

        let slots = rank(&[], &[user], mon(0), mon(23));

        // Monday: 18, 19, 20, 21.
        assert_eq!(slots.len(), 4);
        assert!(slots.iter().all(|s| s.free_user_ids == vec![user]));
    }

    // The whole point of hour granularity: a morning appointment must not
    // rule someone out of the evening.
    #[test]
    fn a_morning_event_does_not_block_that_evening() {
        let user = Uuid::new_v4();
        let busy = vec![(user, mon(9), mon(10))];

        let slots = rank(&busy, &[user], mon(0), mon(23));

        assert!(slots.iter().all(|s| s.free_user_ids == vec![user]));
    }

    // Free *at the start* is not enough - a slot that runs into an event is
    // not a time you can meet.
    #[test]
    fn a_slot_that_runs_into_an_event_is_not_free() {
        let user = Uuid::new_v4();
        // Busy 20:00-22:00. The 19:00 slot (19-21) overlaps it even though
        // 19:00 itself is free.
        let busy = vec![(user, mon(20), mon(22))];

        let slots = rank(&busy, &[user], mon(0), mon(23));

        let at = |h: u32| {
            slots
                .iter()
                .find(|s| s.start == mon(h))
                .map(|s| s.free_user_ids.len())
        };
        assert_eq!(at(18), Some(1)); // 18-20, ends exactly as the event starts
        assert_eq!(at(19), Some(0)); // 19-21 overlaps
        assert_eq!(at(20), Some(0)); // 20-22 is the event
    }

    // Half-open intervals, matching compute_free_users_at above.
    #[test]
    fn an_event_ending_exactly_when_a_slot_starts_does_not_block_it() {
        let user = Uuid::new_v4();
        let busy = vec![(user, mon(16), mon(18))];

        let slots = rank(&busy, &[user], mon(0), mon(23));

        assert_eq!(
            slots
                .iter()
                .find(|s| s.start == mon(18))
                .unwrap()
                .free_user_ids,
            vec![user]
        );
    }

    #[test]
    fn an_all_day_event_blocks_every_slot_that_day() {
        let user = Uuid::new_v4();
        let busy = vec![(user, mon(0), mon(23))];

        let slots = rank(&busy, &[user], mon(0), mon(23));

        assert!(slots.iter().all(|s| s.free_user_ids.is_empty()));
    }

    #[test]
    fn slots_are_ranked_by_how_many_are_free() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        // `b` is busy 20:00-22:00. With two-hour slots that leaves 18:00
        // (18-20, ending exactly as the event starts) free for both, and
        // 19:00 / 20:00 / 21:00 free for `a` alone.
        let busy = vec![(b, mon(20), mon(22))];

        let slots = rank(&busy, &[a, b], mon(0), mon(23));

        assert_eq!(slots[0].start, mon(18));
        assert_eq!(slots[0].free_user_ids.len(), 2);
        // And the rest scored lower, rather than merely being later.
        assert!(slots[1..].iter().all(|s| s.free_user_ids.len() == 1));
    }

    // Several slots will tie. Without a deterministic tiebreak the
    // suggestion shuffles between page loads for no reason.
    #[test]
    fn ties_resolve_to_the_soonest_slot() {
        let user = Uuid::new_v4();

        let slots = rank(&[], &[user], mon(0), mon(23));

        assert_eq!(slots[0].start, mon(18));
        let starts: Vec<_> = slots.iter().map(|s| s.start).collect();
        let mut sorted = starts.clone();
        sorted.sort();
        assert_eq!(
            starts, sorted,
            "equal-scoring slots should be chronological"
        );
    }

    // A suggestion at 03:00 is noise; the candidate set is deliberately the
    // shape of when this kind of group meets.
    #[test]
    fn small_hours_are_never_suggested() {
        let user = Uuid::new_v4();

        let slots = rank(&[], &[user], mon(0), mon(23));

        assert!(slots.iter().all(|s| s.start.hour() >= 18));
    }

    #[test]
    fn weekends_also_offer_afternoons() {
        let user = Uuid::new_v4();

        let slots = rank(&[], &[user], sat(0), sat(23));

        assert!(slots.iter().any(|s| s.start.hour() == 12));
        assert!(slots.iter().any(|s| s.start.hour() == 20));
    }

    // "Evening" means the requester's evening. At UTC+2, their 20:00 is
    // 18:00 UTC.
    #[test]
    fn candidate_hours_follow_the_requesters_offset() {
        let user = Uuid::new_v4();

        let slots = rank_slots(&[], &[user], mon(0), mon(23), 120, 120);

        let hours: Vec<u32> = slots.iter().map(|s| s.start.hour()).collect();
        assert_eq!(hours, vec![16, 17, 18, 19]);
    }

    #[test]
    fn an_empty_user_list_yields_nothing_rather_than_panicking() {
        assert!(rank(&[], &[], mon(0), mon(23)).is_empty());
    }

    #[test]
    fn a_backwards_or_zero_window_yields_nothing() {
        let user = Uuid::new_v4();
        assert!(rank(&[], &[user], mon(23), mon(0)).is_empty());
        assert!(rank_slots(&[], &[user], mon(0), mon(23), 0, 0).is_empty());
    }

    // Same rule the day path already has a test for; the two are read as a
    // pair, so the slot-level twin has to exist.
    #[sqlx::test]
    async fn declined_and_pending_do_not_block_a_slot(db: PgPool) {
        use crate::models::ParticipationStatus;
        let creator = seed_user(&db, "creator-d", "creator").await;
        let declined = seed_user(&db, "declined-d", "declined").await;
        let pending = seed_user(&db, "pending-d", "pending").await;

        // A six-hour event covering the whole evening, that neither of them
        // agreed to.
        let start = Utc::now() + Duration::days(1);
        seed_event(
            &db,
            creator,
            declined,
            ParticipationStatus::Declined,
            start,
            start + Duration::hours(24),
        )
        .await;
        seed_event(
            &db,
            creator,
            pending,
            ParticipationStatus::Pending,
            start,
            start + Duration::hours(24),
        )
        .await;

        let slots = best_slots(
            &db,
            &[declined, pending],
            Utc::now(),
            Utc::now() + Duration::days(7),
            120,
            0,
            5,
        )
        .await
        .unwrap();

        assert!(
            slots.iter().any(|s| s.free_user_ids.len() == 2),
            "declined/pending should not make anyone busy"
        );
    }
    // --- external calendars -----------------------------------------------
    //
    // The point of the whole import feature: a work meeting has to block a
    // slot exactly the way one of our own events does, and it does so by
    // being one more row out of fetch_busy_intervals - the ranking never
    // learns that external calendars exist.

    async fn connect_and_block(db: &PgPool, user: Uuid, start: DateTime<Utc>, end: DateTime<Utc>) {
        let calendar: Uuid = sqlx::query_scalar(
            "INSERT INTO external_calendars (id, user_id, provider, credential)
             VALUES (gen_random_uuid(), $1, 'ics', $2) RETURNING id",
        )
        .bind(user)
        .bind(format!("https://example.com/{user}.ics"))
        .fetch_one(db)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO external_busy (id, calendar_id, starts_at, ends_at)
             VALUES (gen_random_uuid(), $1, $2, $3)",
        )
        .bind(calendar)
        .bind(start)
        .bind(end)
        .execute(db)
        .await
        .unwrap();
    }

    #[sqlx::test]
    async fn an_imported_meeting_makes_someone_busy(db: PgPool) {
        let user = seed_user(&db, "busy-d", "busy").await;
        let now = Utc::now();

        // Free before it exists...
        let before = free_users_now(&db, &[user]).await.unwrap();
        assert_eq!(before, vec![user]);

        connect_and_block(
            &db,
            user,
            now - Duration::minutes(30),
            now + Duration::hours(1),
        )
        .await;

        let after = free_users_now(&db, &[user]).await.unwrap();
        assert!(after.is_empty(), "an imported block should make them busy");
    }

    #[sqlx::test]
    async fn best_slots_avoids_imported_meetings(db: PgPool) {
        let user = seed_user(&db, "busy-d", "busy").await;
        // Tomorrow 18:00-22:00 UTC, covering every evening candidate slot.
        let day = (Utc::now() + Duration::days(1))
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let day_start = Utc.from_utc_datetime(&day);
        connect_and_block(
            &db,
            user,
            day_start + Duration::hours(18),
            day_start + Duration::hours(23),
        )
        .await;

        let slots = best_slots(
            &db,
            &[user],
            day_start,
            day_start + Duration::days(1),
            120,
            0,
            10,
        )
        .await
        .unwrap();

        assert!(
            slots.iter().all(|s| s.free_user_ids.is_empty()),
            "every evening slot that day is taken by the imported block"
        );
    }

    // A disconnected calendar must stop counting immediately, not at the
    // next sync.
    #[sqlx::test]
    async fn removing_the_calendar_frees_them_again(db: PgPool) {
        let user = seed_user(&db, "busy-d", "busy").await;
        let now = Utc::now();
        connect_and_block(
            &db,
            user,
            now - Duration::minutes(30),
            now + Duration::hours(1),
        )
        .await;
        assert!(free_users_now(&db, &[user]).await.unwrap().is_empty());

        sqlx::query("DELETE FROM external_calendars WHERE user_id = $1")
            .bind(user)
            .execute(&db)
            .await
            .unwrap();

        assert_eq!(free_users_now(&db, &[user]).await.unwrap(), vec![user]);
    }
}
