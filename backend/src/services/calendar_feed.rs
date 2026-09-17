//! The subscribable `.ics` feed.
//!
//! One endpoint of RFC 5545 that Google Calendar, Apple Calendar and Outlook
//! all understand - so a single implementation covers every provider, where
//! *importing* needs a separate integration per vendor.
//!
//! ## Why the token, and why it isn't the JWT
//!
//! ⚠️ A calendar app is handed a URL and GETs it unattended, forever. It
//! cannot send an `Authorization` header, so this is the only route in the
//! app that authenticates on the **URL itself**. That makes the token a
//! bearer credential in a string people paste around - into Google, into a
//! phone, sometimes into a chat.
//!
//! The JWT is the wrong thing to reuse twice over: it expires, which would
//! silently break the feed a week later, and it is a *login* credential. A
//! calendar subscription is not a session.
//!
//! ## Why the formatting is fussy
//!
//! A malformed feed fails **silently** - the calendar app subscribes and
//! shows nothing. Every rule in `render` is there because breaking it
//! produces exactly that.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rand::RngCore;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::EventWithParticipants;

/// 32 bytes, hex-encoded. Long enough that guessing is not a strategy, and
/// hex needs no URL escaping wherever it gets pasted.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Mints a token if the user has none, or replaces it if `rotate`.
///
/// Rotation is the whole revocation story: a leaked link has to be killable
/// without deleting the account.
pub async fn ensure_token(db: &PgPool, user_id: Uuid, rotate: bool) -> Result<String> {
    if !rotate {
        let existing: Option<Option<String>> =
            sqlx::query_scalar("SELECT calendar_feed_token FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(db)
                .await?;
        if let Some(Some(token)) = existing {
            return Ok(token);
        }
    }

    let token = generate_token();
    sqlx::query("UPDATE users SET calendar_feed_token = $1 WHERE id = $2")
        .bind(&token)
        .bind(user_id)
        .execute(db)
        .await?;

    Ok(token)
}

/// The user a feed token belongs to, or nothing.
pub async fn user_for_token(db: &PgPool, token: &str) -> Result<Option<Uuid>> {
    let id = sqlx::query_scalar("SELECT id FROM users WHERE calendar_feed_token = $1")
        .bind(token)
        .fetch_optional(db)
        .await?;

    Ok(id)
}

/// Escapes a TEXT value per RFC 5545 §3.3.11.
///
/// ⚠️ Order matters: backslash first, or the escapes we add get escaped
/// again. An event called "Drinks, then food" breaks the parse without this.
fn escape_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace("\r\n", "\\n")
        .replace(['\n', '\r'], "\\n")
}

/// Folds a content line to 75 **octets**, continuations starting with one
/// space (RFC 5545 §3.1).
///
/// ⚠️ Octets, not characters - splitting mid-UTF-8 produces a line no parser
/// accepts, and event titles here are routinely French. Long descriptions
/// are the common case, and an unfolded long line is the most frequent
/// reason a feed is rejected outright.
fn fold(line: &str) -> String {
    const LIMIT: usize = 75;
    if line.len() <= LIMIT {
        return line.to_string();
    }

    let mut out = String::new();
    let mut remaining = line;
    let mut limit = LIMIT;

    loop {
        if remaining.len() <= limit {
            out.push_str(remaining);
            break;
        }
        // Back off to a char boundary so a multibyte character is never cut.
        let mut split = limit;
        while split > 0 && !remaining.is_char_boundary(split) {
            split -= 1;
        }
        out.push_str(&remaining[..split]);
        out.push_str("\r\n ");
        remaining = &remaining[split..];
        // Continuation lines carry a leading space, so one less octet fits.
        limit = LIMIT - 1;
    }

    out
}

fn format_utc(at: &DateTime<Utc>) -> String {
    at.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Renders the whole calendar.
///
/// ⚠️ **CRLF everywhere.** Bare `\n` is the other silent-failure classic.
pub fn render(events: &[EventWithParticipants], app_url: &str) -> String {
    let mut lines: Vec<String> = vec![
        "BEGIN:VCALENDAR".into(),
        "VERSION:2.0".into(),
        "PRODID:-//Friends Calendar//EN".into(),
        "CALSCALE:GREGORIAN".into(),
        "METHOD:PUBLISH".into(),
        // Without this the subscription is called "Untitled" in the
        // subscriber's sidebar.
        "X-WR-CALNAME:Friends Calendar".into(),
    ];

    for wrapper in events {
        let event = &wrapper.event;
        let mut description = event.description.clone().unwrap_or_default();
        if let Some(link) = &event.link {
            if !description.is_empty() {
                description.push('\n');
            }
            description.push_str(link);
        }

        lines.push("BEGIN:VEVENT".into());
        // ⚠️ Stable and globally unique. An unstable UID makes every refresh
        // look like a delete-and-recreate, which on a phone is a
        // notification storm.
        lines.push(format!("UID:{}@friends-calendar", event.id));
        lines.push(format!("DTSTAMP:{}", format_utc(&event.updated_at)));
        lines.push(format!("DTSTART:{}", format_utc(&event.start_time)));
        lines.push(format!("DTEND:{}", format_utc(&event.end_time)));
        lines.push(format!("SUMMARY:{}", escape_text(&event.title)));
        if !description.is_empty() {
            lines.push(format!("DESCRIPTION:{}", escape_text(&description)));
        }
        if let Some(location) = &event.location {
            lines.push(format!("LOCATION:{}", escape_text(location)));
        }
        lines.push(format!("URL:{app_url}/"));
        lines.push("END:VEVENT".into());
    }

    lines.push("END:VCALENDAR".into());

    let mut out = String::new();
    for line in lines {
        out.push_str(&fold(&line));
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CalendarEvent, Visibility};
    use chrono::TimeZone;

    fn an_event(title: &str) -> EventWithParticipants {
        EventWithParticipants {
            event: CalendarEvent {
                id: Uuid::nil(),
                creator_id: Uuid::nil(),
                title: title.to_string(),
                description: None,
                start_time: Utc.with_ymd_and_hms(2026, 3, 1, 19, 0, 0).unwrap(),
                end_time: Utc.with_ymd_and_hms(2026, 3, 1, 22, 0, 0).unwrap(),
                location: None,
                visibility: Visibility::Friends,
                created_at: Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap(),
                updated_at: Utc.with_ymd_and_hms(2026, 2, 2, 0, 0, 0).unwrap(),
                price: None,
                link: None,
            },
            participants: vec![],
            is_creator: true,
            my_status: None,
            is_participant: true,
            reminder_leads: vec![],
        }
    }

    // --- escaping: a malformed feed fails *silently* ----------------------

    #[test]
    fn commas_semicolons_and_backslashes_are_escaped() {
        assert_eq!(escape_text("Drinks, then food"), "Drinks\\, then food");
        assert_eq!(escape_text("a;b"), "a\\;b");
        // Backslash first, or the escapes we add get escaped again.
        assert_eq!(escape_text("a\\b"), "a\\\\b");
    }

    #[test]
    fn newlines_become_literal_backslash_n() {
        assert_eq!(escape_text("one\ntwo"), "one\\ntwo");
        assert_eq!(escape_text("one\r\ntwo"), "one\\ntwo");
    }

    // --- folding: the most common reason a feed is rejected ---------------

    #[test]
    fn a_short_line_is_left_alone() {
        assert_eq!(fold("SUMMARY:short"), "SUMMARY:short");
    }

    #[test]
    fn a_long_line_is_folded_at_75_octets_with_a_leading_space() {
        let line = format!("DESCRIPTION:{}", "x".repeat(200));
        let folded = fold(&line);

        let parts: Vec<&str> = folded.split("\r\n").collect();
        assert!(parts.len() > 1, "a 212-octet line must fold");
        assert_eq!(parts[0].len(), 75);
        for continuation in &parts[1..] {
            assert!(continuation.starts_with(' '), "continuations need a space");
            assert!(continuation.len() <= 75);
        }
        // Nothing is lost.
        assert_eq!(folded.replace("\r\n ", ""), line);
    }

    // ⚠️ Octets, not chars. Splitting mid-UTF-8 yields a line no parser
    // accepts, and titles here are routinely French.
    #[test]
    fn folding_never_splits_a_multibyte_character() {
        let line = format!("SUMMARY:{}", "é".repeat(80));

        let folded = fold(&line);

        // The real assertion: it is still valid UTF-8 and nothing was lost.
        assert_eq!(folded.replace("\r\n ", ""), line);
        for part in folded.split("\r\n") {
            assert!(std::str::from_utf8(part.as_bytes()).is_ok());
        }
    }

    // --- the document ------------------------------------------------------

    #[test]
    fn every_line_ends_crlf() {
        let out = render(&[an_event("Raclette")], "https://app.example");

        assert!(out.starts_with("BEGIN:VCALENDAR\r\n"));
        assert!(out.ends_with("END:VCALENDAR\r\n"));
        // A bare \n anywhere is the other silent-failure classic.
        assert!(!out.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn times_are_utc_with_a_trailing_z() {
        let out = render(&[an_event("Raclette")], "https://app.example");

        assert!(out.contains("DTSTART:20260301T190000Z\r\n"));
        assert!(out.contains("DTEND:20260301T220000Z\r\n"));
    }

    // An unstable UID makes every refresh look like a delete-and-recreate,
    // which on a phone is a notification storm.
    #[test]
    fn the_uid_is_stable_across_renders() {
        let event = an_event("Raclette");
        let first = render(std::slice::from_ref(&event), "https://app.example");
        let second = render(&[event], "https://app.example");

        assert_eq!(first, second);
        assert!(first.contains(&format!("UID:{}@friends-calendar", Uuid::nil())));
    }

    #[test]
    fn an_empty_calendar_is_still_a_valid_document() {
        let out = render(&[], "https://app.example");

        assert!(out.contains("BEGIN:VCALENDAR"));
        assert!(out.contains("END:VCALENDAR"));
        assert!(!out.contains("BEGIN:VEVENT"));
    }

    #[test]
    fn the_link_is_appended_to_the_description() {
        let mut event = an_event("Concert");
        event.event.description = Some("Doors at 19h".into());
        event.event.link = Some("https://tickets.example/x".into());

        let out = render(&[event], "https://app.example");

        assert!(out.contains("DESCRIPTION:Doors at 19h\\nhttps://tickets.example/x"));
    }

    #[test]
    fn a_token_is_long_and_url_safe() {
        let token = generate_token();

        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        // Two draws must not collide.
        assert_ne!(token, generate_token());
    }
}
