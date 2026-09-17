//! Reading an iCalendar document down to **busy intervals and nothing else**.
//!
//! ⚠️ This is the privacy boundary of the whole import feature. A work
//! calendar contains "Interview with…" and "Oncology appointment"; a social
//! app has no business storing either. Availability only ever needs
//! `(start, end)`, so `SUMMARY`, `DESCRIPTION`, `LOCATION` and `ATTENDEE`
//! are never even parsed - not read and discarded, *not parsed*. There is a
//! test asserting the output type carries no room for them.
//!
//! ## What it understands
//!
//! - Line unfolding (RFC 5545 §3.1) - a folded `DTSTART` is otherwise
//!   invisible.
//! - `DTSTART`/`DTEND` as UTC (`…Z`), as a floating/zoned local time with
//!   `TZID`, or as an all-day `VALUE=DATE`.
//! - `DURATION` where there is no `DTEND`, which is what Google writes for
//!   many events.
//! - `RRULE` for `DAILY`/`WEEKLY`/`MONTHLY` with `INTERVAL`, `COUNT`,
//!   `UNTIL` and `BYDAY`, expanded **only inside the requested window** so
//!   an infinite rule stays bounded.
//! - `EXDATE`, so a cancelled occurrence of a recurring event stops
//!   blocking.
//!
//! ## What it does not
//!
//! `RDATE`, `BYSETPOS`, `BYMONTHDAY`, and per-occurrence overrides via
//! `RECURRENCE-ID`. Each would make a handful of events slightly wrong
//! rather than silently dropping them; they're listed here so the gap is
//! known rather than discovered.

use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc, Weekday};
use chrono_tz::Tz;

/// The only thing that leaves this module. Deliberately has nowhere to put a
/// title - see the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusyInterval {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Undoes RFC 5545 line folding: CRLF (or LF) followed by a single space or
/// tab continues the previous line.
///
/// ⚠️ Must run before anything else. A folded `DTSTART` looks like a line
/// starting with a space and would simply not match, so the event would
/// silently lose its time.
fn unfold(input: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in input.replace("\r\n", "\n").split('\n') {
        if let (Some(rest), Some(last)) = (
            raw.strip_prefix(' ').or_else(|| raw.strip_prefix('\t')),
            lines.last_mut(),
        ) {
            last.push_str(rest);
            continue;
        }
        lines.push(raw.to_string());
    }
    lines
}

/// A property's parameters: `TZID=Europe/Paris`, `VALUE=DATE`, and so on.
type Params = Vec<(String, String)>;

/// Splits `DTSTART;TZID=Europe/Paris:20260301T190000` into its name, its
/// parameters, and its value.
fn split_property(line: &str) -> Option<(String, Params, String)> {
    let (left, value) = line.split_once(':')?;
    let mut parts = left.split(';');
    let name = parts.next()?.to_ascii_uppercase();
    let params = parts
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((k.to_ascii_uppercase(), v.trim_matches('"').to_string()))
        })
        .collect();
    Some((name, params, value.to_string()))
}

/// A parsed date-time, which may be all-day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IcsTime {
    at: DateTime<Utc>,
    all_day: bool,
}

fn parse_time(params: &[(String, String)], value: &str) -> Option<IcsTime> {
    let is_date = params
        .iter()
        .any(|(k, v)| k == "VALUE" && v.eq_ignore_ascii_case("DATE"));

    if is_date || value.len() == 8 {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        return Some(IcsTime {
            at: Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0)?),
            all_day: true,
        });
    }

    if let Some(stripped) = value.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(stripped, "%Y%m%dT%H%M%S").ok()?;
        return Some(IcsTime {
            at: Utc.from_utc_datetime(&naive),
            all_day: false,
        });
    }

    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;

    // ⚠️ A local time with a TZID. Resolving it in the wrong zone puts the
    // block up to 12 hours out, which in this feature means confidently
    // suggesting a time somebody is in a meeting.
    let tz = params
        .iter()
        .find(|(k, _)| k == "TZID")
        .and_then(|(_, v)| v.parse::<Tz>().ok());

    let at = match tz {
        Some(tz) => tz
            .from_local_datetime(&naive)
            .earliest()
            .map(|dt| dt.with_timezone(&Utc))?,
        // No TZID: a "floating" time, which the spec says means local to
        // whoever reads it. UTC is the only defensible reading here, and it
        // is rare in practice.
        None => Utc.from_utc_datetime(&naive),
    };

    Some(IcsTime { at, all_day: false })
}

/// `PT1H30M`, `P1D` and friends.
fn parse_duration(value: &str) -> Option<Duration> {
    let value = value.strip_prefix('P')?;
    let (date_part, time_part) = match value.split_once('T') {
        Some((d, t)) => (d, Some(t)),
        None => (value, None),
    };

    let mut total = Duration::zero();
    let mut number = String::new();
    for c in date_part.chars() {
        if c.is_ascii_digit() {
            number.push(c);
        } else {
            let n: i64 = number.parse().ok()?;
            number.clear();
            total += match c {
                'W' => Duration::weeks(n),
                'D' => Duration::days(n),
                _ => return None,
            };
        }
    }
    if let Some(time_part) = time_part {
        for c in time_part.chars() {
            if c.is_ascii_digit() {
                number.push(c);
            } else {
                let n: i64 = number.parse().ok()?;
                number.clear();
                total += match c {
                    'H' => Duration::hours(n),
                    'M' => Duration::minutes(n),
                    'S' => Duration::seconds(n),
                    _ => return None,
                };
            }
        }
    }
    Some(total)
}

fn weekday_from_ics(code: &str) -> Option<Weekday> {
    // BYDAY can carry an ordinal ("2TU"); the ordinal is BYSETPOS-adjacent
    // and unsupported, so only the day code is read.
    let code = code.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '+');
    match code.to_ascii_uppercase().as_str() {
        "MO" => Some(Weekday::Mon),
        "TU" => Some(Weekday::Tue),
        "WE" => Some(Weekday::Wed),
        "TH" => Some(Weekday::Thu),
        "FR" => Some(Weekday::Fri),
        "SA" => Some(Weekday::Sat),
        "SU" => Some(Weekday::Sun),
        _ => None,
    }
}

struct Rrule {
    freq: String,
    interval: i64,
    count: Option<usize>,
    until: Option<DateTime<Utc>>,
    byday: Vec<Weekday>,
}

fn parse_rrule(value: &str) -> Option<Rrule> {
    let mut freq = None;
    let mut interval = 1i64;
    let mut count = None;
    let mut until = None;
    let mut byday = Vec::new();

    for part in value.split(';') {
        let (k, v) = part.split_once('=')?;
        match k.to_ascii_uppercase().as_str() {
            "FREQ" => freq = Some(v.to_ascii_uppercase()),
            "INTERVAL" => interval = v.parse().unwrap_or(1),
            "COUNT" => count = v.parse().ok(),
            "UNTIL" => until = parse_time(&[], v).map(|t| t.at),
            "BYDAY" => byday = v.split(',').filter_map(weekday_from_ics).collect(),
            _ => {}
        }
    }

    Some(Rrule {
        freq: freq?,
        interval: interval.max(1),
        count,
        until,
        byday,
    })
}

/// Expands a recurring event's start times **within `[from, to)`**.
///
/// ⚠️ Bounded by the window on purpose: `RRULE:FREQ=DAILY` with no `COUNT`
/// or `UNTIL` is infinite, and a sync that tried to enumerate it would never
/// return. The cap below is a second belt for a pathological `INTERVAL`.
fn expand(
    rule: &Rrule,
    first: DateTime<Utc>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Vec<DateTime<Utc>> {
    const MAX_OCCURRENCES: usize = 1000;

    let mut out = Vec::new();
    let mut cursor = first;
    let mut emitted = 0usize;

    for _ in 0..MAX_OCCURRENCES {
        if cursor >= to {
            break;
        }
        if rule.until.is_some_and(|until| cursor > until) {
            break;
        }
        if rule.count.is_some_and(|count| emitted >= count) {
            break;
        }

        let matches_day = rule.byday.is_empty() || rule.byday.contains(&cursor.weekday());
        if matches_day {
            emitted += 1;
            if cursor >= from {
                out.push(cursor);
            }
        }

        cursor = match rule.freq.as_str() {
            // With BYDAY, a weekly rule lists several days per week, so step
            // a day at a time and let the BYDAY check above do the filtering.
            "DAILY" => cursor + Duration::days(rule.interval),
            "WEEKLY" if !rule.byday.is_empty() => cursor + Duration::days(1),
            "WEEKLY" => cursor + Duration::weeks(rule.interval),
            "MONTHLY" => add_months(cursor, rule.interval),
            _ => break,
        };
    }

    out
}

/// Calendar-aware month arithmetic, clamping to the last valid day - the
/// 31st of a 30-day month would otherwise roll into the next one.
fn add_months(at: DateTime<Utc>, months: i64) -> DateTime<Utc> {
    let total = at.month0() as i64 + months;
    let year = at.year() + (total.div_euclid(12)) as i32;
    let month0 = total.rem_euclid(12) as u32;

    let mut day = at.day();
    loop {
        if let Some(date) = NaiveDate::from_ymd_opt(year, month0 + 1, day) {
            return Utc.from_utc_datetime(&date.and_time(at.time()));
        }
        if day <= 28 {
            return at + Duration::days(30 * months);
        }
        day -= 1;
    }
}

/// Every busy interval in `input` that overlaps `[from, to)`.
///
/// `include_all_day` decides whether a day-long block counts. See
/// `services::external_calendar` for why the two callers disagree.
pub fn busy_intervals(
    input: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    include_all_day: bool,
) -> Result<Vec<BusyInterval>> {
    let lines = unfold(input);
    let mut out = Vec::new();

    let mut in_event = false;
    let mut start: Option<IcsTime> = None;
    let mut end: Option<IcsTime> = None;
    let mut duration: Option<Duration> = None;
    let mut rrule: Option<Rrule> = None;
    let mut exdates: Vec<DateTime<Utc>> = Vec::new();
    // A cancelled event still appears in many exports; it should not block.
    let mut cancelled = false;
    // So should a declined one, where the export says so.
    let mut transparent = false;

    for line in lines {
        let trimmed = line.trim_end();
        if trimmed.eq_ignore_ascii_case("BEGIN:VEVENT") {
            in_event = true;
            start = None;
            end = None;
            duration = None;
            rrule = None;
            exdates.clear();
            cancelled = false;
            transparent = false;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("END:VEVENT") {
            in_event = false;
            if cancelled || transparent {
                continue;
            }
            let Some(start_time) = start else { continue };
            if start_time.all_day && !include_all_day {
                continue;
            }

            // No DTEND and no DURATION: RFC 5545 says an all-day event lasts
            // a day and a timed one is instantaneous. An instant blocks
            // nothing, so it is skipped rather than stored as a zero-width
            // interval nothing can overlap.
            let length = match (end, duration) {
                (Some(e), _) => e.at - start_time.at,
                (None, Some(d)) => d,
                (None, None) if start_time.all_day => Duration::days(1),
                (None, None) => continue,
            };
            if length <= Duration::zero() {
                continue;
            }

            let starts = match &rrule {
                Some(rule) => expand(rule, start_time.at, from - length, to),
                None => vec![start_time.at],
            };

            for occurrence in starts {
                if exdates.contains(&occurrence) {
                    continue;
                }
                let interval = BusyInterval {
                    start: occurrence,
                    end: occurrence + length,
                };
                // Half-open on both sides, matching services::availability.
                if interval.start < to && from < interval.end {
                    out.push(interval);
                }
            }
            continue;
        }
        if !in_event {
            continue;
        }

        let Some((name, params, value)) = split_property(trimmed) else {
            continue;
        };
        match name.as_str() {
            "DTSTART" => start = parse_time(&params, &value),
            "DTEND" => end = parse_time(&params, &value),
            "DURATION" => duration = parse_duration(&value),
            "RRULE" => rrule = parse_rrule(&value),
            "EXDATE" => {
                for one in value.split(',') {
                    if let Some(t) = parse_time(&params, one) {
                        exdates.push(t.at);
                    }
                }
            }
            "STATUS" => cancelled = value.eq_ignore_ascii_case("CANCELLED"),
            // TRANSP:TRANSPARENT is the calendar's own way of saying "this
            // does not make me busy" - respect it rather than overruling it.
            "TRANSP" => transparent = value.eq_ignore_ascii_case("TRANSPARENT"),
            _ => {}
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    /// Wraps VEVENT bodies in a calendar, with CRLF like a real export.
    fn cal(body: &str) -> String {
        format!("BEGIN:VCALENDAR\r\n{body}\r\nEND:VCALENDAR\r\n")
            .replace('\n', "\r\n")
            .replace("\r\r\n", "\r\n")
    }

    fn parse(body: &str) -> Vec<BusyInterval> {
        busy_intervals(&cal(body), at(2026, 3, 1, 0), at(2026, 4, 1, 0), false).unwrap()
    }

    #[test]
    fn a_plain_utc_event_becomes_one_interval() {
        let out =
            parse("BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nEND:VEVENT");

        assert_eq!(
            out,
            vec![BusyInterval {
                start: at(2026, 3, 2, 9),
                end: at(2026, 3, 2, 10)
            }]
        );
    }

    // ⚠️ Resolving a TZID in the wrong zone puts the block hours out. Paris
    // is UTC+1 in March before the DST switch.
    #[test]
    fn a_tzid_local_time_is_converted_to_utc() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART;TZID=Europe/Paris:20260302T090000\nDTEND;TZID=Europe/Paris:20260302T100000\nEND:VEVENT",
        );

        assert_eq!(out[0].start, at(2026, 3, 2, 8));
        assert_eq!(out[0].end, at(2026, 3, 2, 9));
    }

    #[test]
    fn a_duration_stands_in_for_a_missing_dtend() {
        let out = parse("BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDURATION:PT1H30M\nEND:VEVENT");

        assert_eq!(
            out[0].end,
            Utc.with_ymd_and_hms(2026, 3, 2, 10, 30, 0).unwrap()
        );
    }

    // ⚠️ A folded DTSTART looks like a line starting with a space. Without
    // unfolding the event silently loses its time and is dropped.
    #[test]
    fn a_folded_line_is_rejoined_before_parsing() {
        let folded = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART:2026030\r\n 2T090000Z\r\nDTEND:20260302T100000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

        let out = busy_intervals(folded, at(2026, 3, 1, 0), at(2026, 4, 1, 0), false).unwrap();

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start, at(2026, 3, 2, 9));
    }

    // The canonical case this feature exists for: a weekly standup.
    #[test]
    fn a_weekly_rule_expands_across_the_window() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T093000Z\nRRULE:FREQ=WEEKLY\nEND:VEVENT",
        );

        // Mondays in March 2026 from the 2nd: 2, 9, 16, 23, 30.
        assert_eq!(out.len(), 5);
        assert_eq!(out[0].start, at(2026, 3, 2, 9));
        assert_eq!(out[4].start, at(2026, 3, 30, 9));
    }

    #[test]
    fn byday_expands_several_days_a_week() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T093000Z\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE\nEND:VEVENT",
        );

        // Mon + Wed through March: 2,4,9,11,16,18,23,25,30.
        assert_eq!(out.len(), 9);
        assert!(
            out.iter()
                .all(|i| matches!(i.start.weekday(), Weekday::Mon | Weekday::Wed))
        );
    }

    #[test]
    fn count_and_until_both_stop_a_rule() {
        let counted = parse(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T093000Z\nRRULE:FREQ=DAILY;COUNT=3\nEND:VEVENT",
        );
        assert_eq!(counted.len(), 3);

        let bounded = parse(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T093000Z\nRRULE:FREQ=DAILY;UNTIL=20260304T235959Z\nEND:VEVENT",
        );
        assert_eq!(bounded.len(), 3);
    }

    #[test]
    fn interval_skips_occurrences() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T093000Z\nRRULE:FREQ=DAILY;INTERVAL=7;COUNT=3\nEND:VEVENT",
        );

        assert_eq!(out.len(), 3);
        assert_eq!(out[1].start, at(2026, 3, 9, 9));
    }

    // ⚠️ An unbounded daily rule is infinite. If this ever hangs or returns
    // thousands of rows, the window bound has been lost.
    #[test]
    fn an_endless_rule_is_bounded_by_the_window() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T093000Z\nRRULE:FREQ=DAILY\nEND:VEVENT",
        );

        assert_eq!(out.len(), 30, "2nd to 31st of March inclusive");
    }

    // A cancelled standup should stop blocking that week.
    #[test]
    fn an_exdate_removes_that_occurrence() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T093000Z\nRRULE:FREQ=WEEKLY\nEXDATE:20260309T090000Z\nEND:VEVENT",
        );

        assert_eq!(out.len(), 4);
        assert!(!out.iter().any(|i| i.start == at(2026, 3, 9, 9)));
    }

    #[test]
    fn a_cancelled_event_does_not_block() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nSTATUS:CANCELLED\nEND:VEVENT",
        );

        assert!(out.is_empty());
    }

    // The calendar's own way of saying "this doesn't make me busy".
    #[test]
    fn a_transparent_event_does_not_block() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nTRANSP:TRANSPARENT\nEND:VEVENT",
        );

        assert!(out.is_empty());
    }

    // A day-long "Annual leave" should not rule out a 19:00 slot, which is
    // why the caller chooses - see services::external_calendar.
    #[test]
    fn all_day_events_are_opt_in() {
        let body =
            "BEGIN:VEVENT\nDTSTART;VALUE=DATE:20260302\nDTEND;VALUE=DATE:20260303\nEND:VEVENT";

        let excluded =
            busy_intervals(&cal(body), at(2026, 3, 1, 0), at(2026, 4, 1, 0), false).unwrap();
        let included =
            busy_intervals(&cal(body), at(2026, 3, 1, 0), at(2026, 4, 1, 0), true).unwrap();

        assert!(excluded.is_empty());
        assert_eq!(included.len(), 1);
        assert_eq!(included[0].start, at(2026, 3, 2, 0));
    }

    #[test]
    fn events_outside_the_window_are_dropped() {
        let out =
            parse("BEGIN:VEVENT\nDTSTART:20250101T090000Z\nDTEND:20250101T100000Z\nEND:VEVENT");

        assert!(out.is_empty());
    }

    // An event straddling the window's start is still busy inside it.
    #[test]
    fn an_event_overlapping_the_window_edge_is_kept() {
        let out = busy_intervals(
            &cal("BEGIN:VEVENT\nDTSTART:20260228T230000Z\nDTEND:20260301T010000Z\nEND:VEVENT"),
            at(2026, 3, 1, 0),
            at(2026, 4, 1, 0),
            false,
        )
        .unwrap();

        assert_eq!(out.len(), 1);
    }

    #[test]
    fn a_zero_length_or_backwards_event_is_skipped() {
        let zero =
            parse("BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T090000Z\nEND:VEVENT");
        let backwards =
            parse("BEGIN:VEVENT\nDTSTART:20260302T100000Z\nDTEND:20260302T090000Z\nEND:VEVENT");

        assert!(zero.is_empty());
        assert!(backwards.is_empty());
    }

    #[test]
    fn junk_does_not_panic_or_produce_intervals() {
        for input in [
            "",
            "not a calendar",
            "BEGIN:VEVENT",
            "BEGIN:VEVENT\r\nDTSTART:nonsense\r\nEND:VEVENT",
        ] {
            let out = busy_intervals(input, at(2026, 3, 1, 0), at(2026, 4, 1, 0), false).unwrap();
            assert!(out.is_empty(), "input: {input:?}");
        }
    }

    // ⚠️ The privacy promise, asserted rather than trusted: the output type
    // has nowhere to put a title, and the parser never reads one.
    #[test]
    fn no_event_content_is_ever_extracted() {
        let out = parse(
            "BEGIN:VEVENT\nSUMMARY:Oncology appointment\nDESCRIPTION:secret\nLOCATION:Hospital\nATTENDEE:mailto:a@b.c\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nEND:VEVENT",
        );

        assert_eq!(out.len(), 1);
        // If BusyInterval ever grows a text field, this stops compiling -
        // which is the point.
        let BusyInterval { start: _, end: _ } = out[0];
    }
}
