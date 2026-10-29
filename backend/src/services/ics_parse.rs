//! Reading an iCalendar document down to **busy intervals and nothing else**.
//!
//! ⚠️ This is the privacy boundary of the whole import feature. A work
//! calendar contains "Interview with…" and "Oncology appointment"; a social
//! app has no business storing either. Availability only ever needs
//! `(start, end)`, so `SUMMARY`, `DESCRIPTION`, `LOCATION` and `ATTENDEE`
//! are never even parsed - not read and discarded, *not parsed*. There is a
//! test asserting the output type carries no room for them. (`UID` is read,
//! but only to pair a moved occurrence with its series; it is an opaque id
//! and never leaves this module.)
//!
//! ## What it understands
//!
//! - Line unfolding (RFC 5545 §3.1) - a folded `DTSTART` is otherwise
//!   invisible.
//! - Parameter values in double quotes, which may contain `:` and `;`.
//! - `DTSTART`/`DTEND` as UTC (`…Z`), as a zoned local time with `TZID`, as
//!   a floating time, or as an all-day `VALUE=DATE`.
//! - `TZID` as an IANA name, a **Windows** zone id (what Outlook and Teams
//!   write - see `services::windows_zones`), an Outlook display name such as
//!   `(UTC+01:00) Amsterdam, Berlin, …`, a Mozilla-style path, or any name
//!   described by a `VTIMEZONE` block in the feed. `X-WR-TIMEZONE` covers
//!   floating times. An unresolvable zone is logged, then read as UTC.
//! - `DURATION` where there is no `DTEND`, which is what Google writes for
//!   many events.
//! - `RRULE` for `DAILY`/`WEEKLY`/`MONTHLY`/`YEARLY` with `INTERVAL`,
//!   `COUNT`, `UNTIL`, `WKST`, `BYDAY` (with ordinals for monthly rules, e.g.
//!   `2TU`) and `BYMONTHDAY`. Occurrences are generated in **DTSTART's own
//!   zone** and converted afterwards, so a 09:00 Paris meeting stays at 09:00
//!   across a DST change.
//! - `EXDATE`, and `RECURRENCE-ID` overrides: a moved occurrence blocks its
//!   new slot instead of its old one, and a cancelled one blocks neither.
//!
//! ## Bounded, on purpose
//!
//! `RRULE:FREQ=DAILY` with no `COUNT` or `UNTIL` is infinite. Expansion jumps
//! arithmetically to the requested window rather than walking from
//! `DTSTART`, so a stand-up created years ago costs the same as one created
//! last week; only a `COUNT` rule is walked from the start, because `COUNT`
//! is counted from there. A hard cap on periods walked, per rule and per
//! feed, is a second belt, and a feed producing more than
//! `MAX_INTERVALS_PER_FEED` blocks is refused rather than truncated.
//!
//! ⚠️ The feed is any URL somebody pasted. Every piece of arithmetic on its
//! values is checked, and an invalid value skips that property or event -
//! it never panics, and (in release builds, where overflow wraps) never
//! silently becomes a different, plausible value.
//!
//! ## What it does not
//!
//! `RDATE`, `BYSETPOS`, `BYMONTH`/`BYWEEKNO`/`BYYEARDAY` on the event itself,
//! sub-daily frequencies (`HOURLY` and below - only `DTSTART` is used), and
//! `RECURRENCE-ID;RANGE=THISANDFUTURE` (treated as a single occurrence). Each
//! would leave a handful of occurrences slightly wrong rather than dropping
//! a calendar.

use std::collections::HashMap;
use std::rc::Rc;

use anyhow::{Result, anyhow, bail};
use chrono::{
    DateTime, Datelike, Duration, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc, Weekday,
};
use chrono_tz::{TZ_VARIANTS, Tz};

use crate::services::windows_zones;

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
///
/// ⚠️ A parameter value containing `:` or `;` must be quoted (RFC 5545
/// §3.1), and Outlook does exactly that: `TZID="(UTC+01:00) Amsterdam, …"`.
/// Splitting on the first colon regardless of quotes cut the property inside
/// the zone name, the value stopped parsing, and the event was dropped.
fn split_property(line: &str) -> Option<(String, Params, String)> {
    let mut in_quotes = false;
    let mut cuts = Vec::new();
    let mut colon = None;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ';' if !in_quotes => cuts.push(i),
            ':' if !in_quotes => {
                colon = Some(i);
                break;
            }
            _ => {}
        }
    }
    let colon = colon?;
    let left = &line[..colon];
    let value = &line[colon + 1..];

    let mut pieces = Vec::with_capacity(cuts.len() + 1);
    let mut begin = 0;
    for cut in cuts {
        pieces.push(&left[begin..cut]);
        begin = cut + 1;
    }
    pieces.push(&left[begin..]);

    let name = pieces[0].to_ascii_uppercase();
    let params = pieces[1..]
        .iter()
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((k.to_ascii_uppercase(), v.trim_matches('"').to_string()))
        })
        .collect();
    Some((name, params, value.to_string()))
}

// --- Time zones ------------------------------------------------------------

/// Where a wall-clock time lives.
#[derive(Debug, Clone)]
enum Zone {
    Utc,
    Named(Tz),
    Fixed(FixedOffset),
    /// Described only by a `VTIMEZONE` block in the feed itself.
    Custom(Rc<VTimezone>),
}

impl Zone {
    fn to_utc(&self, local: NaiveDateTime) -> Option<DateTime<Utc>> {
        match self {
            Zone::Utc => Some(Utc.from_utc_datetime(&local)),
            Zone::Named(tz) => local_to_utc(tz, local),
            Zone::Fixed(offset) => local_to_utc(offset, local),
            Zone::Custom(vtz) => local
                .checked_sub_signed(Duration::seconds(vtz.offset_at(local) as i64))
                .map(|utc| Utc.from_utc_datetime(&utc)),
        }
    }
}

/// The earlier reading of an ambiguous (clocks-back) time. A time inside the
/// clocks-forward gap does not exist; RFC 5545 §3.3.5 reads it with the
/// offset from before the gap, which lands an hour later on the wall clock.
fn local_to_utc<Z: TimeZone>(zone: &Z, local: NaiveDateTime) -> Option<DateTime<Utc>> {
    zone.from_local_datetime(&local)
        .earliest()
        .or_else(|| {
            zone.from_local_datetime(&local.checked_add_signed(Duration::hours(1))?)
                .earliest()
        })
        .map(|dt| dt.with_timezone(&Utc))
}

/// `(BYMONTH, BYDAY ordinal, BYDAY weekday)` of a yearly zone transition.
type ObservanceRule = (u32, i32, Weekday);

/// One `STANDARD`/`DAYLIGHT` block of a `VTIMEZONE`.
#[derive(Debug)]
struct Observance {
    start: NaiveDateTime,
    /// Seconds east of UTC once this observance is in effect.
    offset_to: i32,
    /// `FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU` - the only shape real exports use.
    rule: Option<ObservanceRule>,
}

#[derive(Debug, Default)]
struct VTimezone {
    observances: Vec<Observance>,
}

impl VTimezone {
    /// Offset in seconds east of UTC at a wall-clock time: whichever
    /// observance most recently began.
    fn offset_at(&self, local: NaiveDateTime) -> i32 {
        let mut best: Option<(NaiveDateTime, i32)> = None;
        for obs in &self.observances {
            let onsets: Vec<NaiveDateTime> = match obs.rule {
                Some((month, ordinal, weekday)) => {
                    [local.year().checked_sub(1), Some(local.year())]
                        .into_iter()
                        .flatten()
                        .filter_map(|y| nth_weekday(y, month, weekday, ordinal))
                        .map(|d| d.and_time(obs.start.time()))
                        .filter(|onset| *onset >= obs.start)
                        .collect()
                }
                None => vec![obs.start],
            };
            for onset in onsets {
                if onset <= local && best.is_none_or(|(b, _)| onset > b) {
                    best = Some((onset, obs.offset_to));
                }
            }
        }
        best.map(|(_, o)| o)
            .or_else(|| self.observances.first().map(|o| o.offset_to))
            .unwrap_or(0)
    }
}

/// `+0100`, `-0800`, `+053000`.
fn parse_offset(value: &str) -> Option<i32> {
    let (sign, digits) = match value.as_bytes().first()? {
        b'+' => (1, &value[1..]),
        b'-' => (-1, &value[1..]),
        _ => return None,
    };
    if digits.len() < 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let h: i32 = digits[0..2].parse().ok()?;
    let m: i32 = digits[2..4].parse().ok()?;
    let s: i32 = digits.get(4..6).and_then(|s| s.parse().ok()).unwrap_or(0);
    Some(sign * (h * 3600 + m * 60 + s))
}

/// `(UTC+01:00) Amsterdam, …` -> +1h. `(UTC) Dublin, …` -> 0.
fn display_name_offset(name: &str) -> Option<FixedOffset> {
    let inner = name.strip_prefix('(')?.split(')').next()?;
    let rest = inner
        .strip_prefix("UTC")
        .or_else(|| inner.strip_prefix("GMT"))?;
    if rest.is_empty() {
        return FixedOffset::east_opt(0);
    }
    let (sign, rest) = match rest.as_bytes().first()? {
        b'+' => (1, &rest[1..]),
        b'-' => (-1, &rest[1..]),
        _ => return None,
    };
    let (h, m) = rest.split_once(':').unwrap_or((rest, "0"));
    // ⚠️ Feed-supplied, so bounded before any arithmetic: `(UTC+999999:00)`
    // overflowed the multiply (a panic in debug, a wrapped offset in
    // release). Real offsets stay within ±14h.
    let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    if h > 14 || m > 59 {
        return None;
    }
    FixedOffset::east_opt(sign * (h * 3600 + m * 60) as i32)
}

/// The IANA zone a `TZID` means, by every naming scheme real exports use.
fn resolve_named(name: &str) -> Option<Tz> {
    let name = name.trim();
    // Every scheme below is tried per `/` or per city, so a hostile name
    // costs time quadratic in its length. No real zone name is this long.
    if name.len() > 256 {
        return None;
    }
    if let Ok(tz) = name.parse::<Tz>() {
        return Some(tz);
    }
    if let Some(tz) = windows_zones::to_iana(name).and_then(|iana| iana.parse().ok()) {
        return Some(tz);
    }
    // Mozilla-style `/mozilla.org/20050126_1/Europe/Paris`.
    for (i, _) in name.match_indices('/') {
        if let Ok(tz) = name[i + 1..].parse::<Tz>() {
            return Some(tz);
        }
    }
    // Outlook's display name: the cities listed share the zone's rules by
    // definition, so the first one IANA knows by name settles it.
    if let Some((_, cities)) = name.strip_prefix('(').and_then(|n| n.split_once(')')) {
        for city in cities.split(',') {
            let wanted = format!("/{}", city.trim().replace(' ', "_"));
            if wanted.len() < 3 {
                continue;
            }
            if let Some(tz) = TZ_VARIANTS.iter().find(|tz| tz.name().ends_with(&wanted)) {
                return Some(*tz);
            }
        }
    }
    None
}

/// Every time zone a feed can name, resolved once per `TZID`.
struct TzContext {
    vtimezones: HashMap<String, Rc<VTimezone>>,
    /// `X-WR-TIMEZONE`: the calendar's own zone, for floating times.
    default: Option<Zone>,
    cache: HashMap<String, Zone>,
}

impl TzContext {
    fn from_lines(lines: &[String]) -> Self {
        let mut vtimezones = HashMap::new();
        let mut default = None;

        let mut in_vtz = false;
        let mut tzid: Option<String> = None;
        let mut observances = Vec::new();
        // (start, offset_to, rule) of the observance being read.
        let mut current: Option<(Option<NaiveDateTime>, Option<i32>, Option<ObservanceRule>)> =
            None;

        for line in lines {
            let trimmed = line.trim_end();
            let upper = trimmed.to_ascii_uppercase();
            match upper.as_str() {
                "BEGIN:VTIMEZONE" => {
                    in_vtz = true;
                    tzid = None;
                    observances = Vec::new();
                    continue;
                }
                "END:VTIMEZONE" => {
                    if let Some(id) = tzid.take() {
                        vtimezones.insert(
                            id,
                            Rc::new(VTimezone {
                                observances: std::mem::take(&mut observances),
                            }),
                        );
                    }
                    in_vtz = false;
                    continue;
                }
                "BEGIN:STANDARD" | "BEGIN:DAYLIGHT" if in_vtz => {
                    current = Some((None, None, None));
                    continue;
                }
                "END:STANDARD" | "END:DAYLIGHT" if in_vtz => {
                    if let Some((Some(start), Some(offset_to), rule)) = current.take() {
                        observances.push(Observance {
                            start,
                            offset_to,
                            rule,
                        });
                    }
                    continue;
                }
                _ => {}
            }

            let Some((name, _, value)) = split_property(trimmed) else {
                continue;
            };
            if in_vtz {
                match (&mut current, name.as_str()) {
                    (None, "TZID") => tzid = Some(value),
                    (Some(obs), "DTSTART") => obs.0 = parse_naive(&value),
                    (Some(obs), "TZOFFSETTO") => obs.1 = parse_offset(&value),
                    (Some(obs), "RRULE") => obs.2 = parse_observance_rule(&value),
                    _ => {}
                }
            } else if name == "X-WR-TIMEZONE" {
                default = resolve_named(&value).map(Zone::Named);
            }
        }

        TzContext {
            vtimezones,
            default,
            cache: HashMap::new(),
        }
    }

    fn floating(&self) -> Zone {
        self.default.clone().unwrap_or(Zone::Utc)
    }

    fn zone_for(&mut self, tzid: &str) -> Zone {
        if let Some(zone) = self.cache.get(tzid) {
            return zone.clone();
        }
        let zone = if let Some(tz) = resolve_named(tzid) {
            Zone::Named(tz)
        } else if let Some(vtz) = self
            .vtimezones
            .get(tzid)
            .filter(|v| !v.observances.is_empty())
        {
            Zone::Custom(vtz.clone())
        } else if let Some(offset) = display_name_offset(tzid) {
            Zone::Fixed(offset)
        } else if let Some(default) = &self.default {
            default.clone()
        } else {
            // ⚠️ Not silent: a wrong zone puts every meeting in this feed
            // hours out, and this line is the only trace of why.
            tracing::warn!("Unrecognised TZID {tzid:?} in an imported calendar; reading it as UTC");
            Zone::Utc
        };
        self.cache.insert(tzid.to_string(), zone.clone());
        zone
    }
}

fn parse_observance_rule(value: &str) -> Option<ObservanceRule> {
    let mut month = None;
    let mut day = None;
    for part in value.split(';') {
        let (k, v) = part.split_once('=')?;
        match k.to_ascii_uppercase().as_str() {
            "BYMONTH" => month = v.parse().ok().filter(|m| (1..=12).contains(m)),
            "BYDAY" => day = parse_byday(v),
            _ => {}
        }
    }
    let (ordinal, weekday) = day?;
    Some((month?, ordinal.unwrap_or(1), weekday))
}

// --- Times -----------------------------------------------------------------

/// A parsed date-time: the wall clock, the zone it is read in, and whether it
/// is all-day.
#[derive(Debug, Clone)]
struct IcsTime {
    local: NaiveDateTime,
    zone: Zone,
    all_day: bool,
}

impl IcsTime {
    fn utc(&self) -> Option<DateTime<Utc>> {
        self.zone.to_utc(self.local)
    }

    /// Whether this (an `EXDATE` or `RECURRENCE-ID`) names the occurrence.
    /// A date-only value names the whole day.
    fn names(&self, local: NaiveDateTime, utc: DateTime<Utc>) -> bool {
        if self.all_day {
            self.local.date() == local.date()
        } else {
            self.utc() == Some(utc)
        }
    }
}

/// Years a feed may name. RFC 5545 dates are four digits; anything outside
/// is either garbage or hostile, and keeping every date well inside chrono's
/// range means no later step of a date's arithmetic can overflow.
const YEARS: std::ops::RangeInclusive<i32> = 1..=9999;

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y%m%d")
        .ok()
        .filter(|d| YEARS.contains(&d.year()))
}

fn parse_naive(value: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
        .ok()
        .filter(|d| YEARS.contains(&d.year()))
}

fn parse_time(params: &[(String, String)], value: &str, ctx: &mut TzContext) -> Option<IcsTime> {
    let is_date = params
        .iter()
        .any(|(k, v)| k == "VALUE" && v.eq_ignore_ascii_case("DATE"));

    if is_date || value.len() == 8 {
        let date = parse_date(value)?;
        return Some(IcsTime {
            local: date.and_hms_opt(0, 0, 0)?,
            zone: Zone::Utc,
            all_day: true,
        });
    }

    if let Some(stripped) = value.strip_suffix('Z') {
        return Some(IcsTime {
            local: parse_naive(stripped)?,
            zone: Zone::Utc,
            all_day: false,
        });
    }

    let local = parse_naive(value)?;

    // ⚠️ A local time with a TZID. Resolving it in the wrong zone puts the
    // block up to 12 hours out, which in this feature means confidently
    // suggesting a time somebody is in a meeting.
    let zone = match params.iter().find(|(k, _)| k == "TZID") {
        Some((_, tzid)) => ctx.zone_for(tzid),
        // No TZID: a "floating" time, local to whoever reads it. The
        // calendar's own X-WR-TIMEZONE is the best reading, else UTC.
        None => ctx.floating(),
    };

    Some(IcsTime {
        local,
        zone,
        all_day: false,
    })
}

/// `PT1H30M`, `P1D` and friends.
///
/// ⚠️ Every step is checked: `P99999999999999W` is out of `Duration`'s range
/// and the unchecked constructors panic on it. An unrepresentable duration
/// is no duration, and the event falls back to having none.
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
            total = total.checked_add(&match c {
                'W' => Duration::try_weeks(n)?,
                'D' => Duration::try_days(n)?,
                _ => return None,
            })?;
        }
    }
    if let Some(time_part) = time_part {
        for c in time_part.chars() {
            if c.is_ascii_digit() {
                number.push(c);
            } else {
                let n: i64 = number.parse().ok()?;
                number.clear();
                total = total.checked_add(&match c {
                    'H' => Duration::try_hours(n)?,
                    'M' => Duration::try_minutes(n)?,
                    'S' => Duration::try_seconds(n)?,
                    _ => return None,
                })?;
            }
        }
    }
    Some(total)
}

// --- Recurrence ------------------------------------------------------------

fn weekday_from_code(code: &str) -> Option<Weekday> {
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

/// One `BYDAY` entry: `TU`, `2TU`, `-1SU`.
fn parse_byday(entry: &str) -> Option<(Option<i32>, Weekday)> {
    let entry = entry.trim();
    // ⚠️ `get`, not `split_at`: the cut is in bytes, and a feed's `BYDAY=éa`
    // put it inside a character, which panicked.
    let cut = entry.len().checked_sub(2)?;
    let (ordinal, code) = (entry.get(..cut)?, entry.get(cut..)?);
    let weekday = weekday_from_code(code)?;
    let ordinal = match ordinal {
        "" => None,
        n => Some(n.trim_start_matches('+').parse().ok()?),
    };
    Some((ordinal, weekday))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
    /// HOURLY and below: only DTSTART is used.
    Other,
}

struct Rrule {
    freq: Freq,
    interval: i64,
    count: Option<u32>,
    /// Kept raw: a floating UNTIL is read in DTSTART's zone.
    until: Option<String>,
    byday: Vec<(Option<i32>, Weekday)>,
    bymonthday: Vec<i32>,
    wkst: Weekday,
}

fn parse_rrule(value: &str) -> Option<Rrule> {
    let mut freq = None;
    let mut interval = 1i64;
    let mut count = None;
    let mut until = None;
    let mut byday = Vec::new();
    let mut bymonthday = Vec::new();
    let mut wkst = Weekday::Mon;

    for part in value.split(';') {
        let (k, v) = part.split_once('=')?;
        match k.to_ascii_uppercase().as_str() {
            "FREQ" => {
                freq = Some(match v.to_ascii_uppercase().as_str() {
                    "DAILY" => Freq::Daily,
                    "WEEKLY" => Freq::Weekly,
                    "MONTHLY" => Freq::Monthly,
                    "YEARLY" => Freq::Yearly,
                    _ => Freq::Other,
                })
            }
            "INTERVAL" => interval = v.parse().unwrap_or(1),
            "COUNT" => count = v.parse().ok(),
            "UNTIL" => until = Some(v.to_string()),
            "BYDAY" => byday = v.split(',').filter_map(parse_byday).collect(),
            "BYMONTHDAY" => bymonthday = v.split(',').filter_map(|d| d.parse().ok()).collect(),
            "WKST" => wkst = weekday_from_code(v).unwrap_or(Weekday::Mon),
            _ => {}
        }
    }

    Some(Rrule {
        freq: freq?,
        interval: interval.max(1),
        count,
        until,
        byday,
        bymonthday,
        wkst,
    })
}

fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    let (ny, nm) = match month {
        12 => (year.checked_add(1)?, 1),
        1..=11 => (year, month + 1),
        _ => return None,
    };
    NaiveDate::from_ymd_opt(ny, nm, 1)?.pred_opt()
}

/// The `n`th `weekday` of a month; negative counts from the end (`-1SU` is
/// the last Sunday).
///
/// ⚠️ No month has more than five of any weekday, so anything beyond ±5 is
/// rejected up front - `-100000000MO` used to reach unchecked date
/// arithmetic, and `i32::MIN` overflowed negating it.
fn nth_weekday(year: i32, month: u32, weekday: Weekday, n: i32) -> Option<NaiveDate> {
    if !(-5..=5).contains(&n) {
        return None;
    }
    if n > 0 {
        NaiveDate::from_weekday_of_month_opt(year, month, weekday, u8::try_from(n).ok()?)
    } else if n < 0 {
        let last = last_day_of_month(year, month)?;
        let back = (last.weekday().num_days_from_monday() + 7 - weekday.num_days_from_monday()) % 7;
        let date = last.checked_sub_signed(Duration::days(back as i64 + 7 * (-(n as i64) - 1)))?;
        (date.month() == month).then_some(date)
    } else {
        None
    }
}

fn days_after_wkst(day: Weekday, wkst: Weekday) -> i64 {
    ((day.num_days_from_monday() + 7 - wkst.num_days_from_monday()) % 7) as i64
}

fn matches_monthday(rule: &Rrule, date: NaiveDate) -> bool {
    let Some(last) = last_day_of_month(date.year(), date.month()) else {
        return false;
    };
    rule.bymonthday.iter().any(|&n| {
        let day = if n > 0 { n } else { last.day() as i32 + 1 + n };
        day == date.day() as i32
    })
}

/// The occurrence dates of a monthly (or yearly-by-weekday) rule in one month.
///
/// ⚠️ An occurrence that doesn't exist - the 31st of a 30-day month - is
/// skipped, not clamped (RFC 5545 §3.3.10).
fn dates_in_month(rule: &Rrule, year: i32, month: u32, default_day: u32) -> Vec<NaiveDate> {
    let Some(last) = last_day_of_month(year, month) else {
        return Vec::new();
    };
    let mut dates: Vec<NaiveDate> = if !rule.byday.is_empty() {
        rule.byday
            .iter()
            .flat_map(|&(ordinal, weekday)| match ordinal {
                Some(n) => nth_weekday(year, month, weekday, n).into_iter().collect(),
                None => (1..=5)
                    .filter_map(|n| nth_weekday(year, month, weekday, n))
                    .collect::<Vec<_>>(),
            })
            .filter(|d| rule.bymonthday.is_empty() || matches_monthday(rule, *d))
            .collect()
    } else if !rule.bymonthday.is_empty() {
        (1..=last.day())
            .filter_map(|d| NaiveDate::from_ymd_opt(year, month, d))
            .filter(|d| matches_monthday(rule, *d))
            .collect()
    } else {
        NaiveDate::from_ymd_opt(year, month, default_day)
            .into_iter()
            .collect()
    };
    dates.sort();
    dates.dedup();
    dates
}

fn add_months_to_first(year: i32, month: u32, months: i64) -> Option<(i32, u32)> {
    let total = (year as i64 * 12 + (month as i64 - 1)).checked_add(months)?;
    Some((
        i32::try_from(total.div_euclid(12)).ok()?,
        (total.rem_euclid(12) + 1) as u32,
    ))
}

/// Period `k` of a rule: the date it begins on, and the occurrence dates in it.
fn period(
    rule: &Rrule,
    d0: NaiveDate,
    week0: NaiveDate,
    k: i64,
) -> Option<(NaiveDate, Vec<NaiveDate>)> {
    let step = k.checked_mul(rule.interval)?;
    match rule.freq {
        Freq::Daily => {
            let date = d0.checked_add_signed(Duration::try_days(step)?)?;
            let keep = (rule.byday.is_empty()
                || rule.byday.iter().any(|(_, w)| *w == date.weekday()))
                && (rule.bymonthday.is_empty() || matches_monthday(rule, date));
            Some((date, if keep { vec![date] } else { Vec::new() }))
        }
        Freq::Weekly => {
            // ⚠️ Week by week, honouring INTERVAL: a fortnightly 1:1 is every
            // *other* week, whichever days it lists.
            let start = week0.checked_add_signed(Duration::try_weeks(step)?)?;
            let offset = |w: Weekday| {
                start.checked_add_signed(Duration::days(days_after_wkst(w, rule.wkst)))
            };
            let mut dates: Vec<NaiveDate> = if rule.byday.is_empty() {
                offset(d0.weekday()).into_iter().collect()
            } else {
                rule.byday.iter().filter_map(|(_, w)| offset(*w)).collect()
            };
            dates.sort();
            dates.dedup();
            Some((start, dates))
        }
        Freq::Monthly => {
            let (y, m) = add_months_to_first(d0.year(), d0.month(), step)?;
            Some((
                NaiveDate::from_ymd_opt(y, m, 1)?,
                dates_in_month(rule, y, m, d0.day()),
            ))
        }
        Freq::Yearly => {
            let y = d0.year().checked_add(i32::try_from(step).ok()?)?;
            Some((
                NaiveDate::from_ymd_opt(y, 1, 1)?,
                dates_in_month(rule, y, d0.month(), d0.day()),
            ))
        }
        Freq::Other => None,
    }
}

/// How many whole periods lie between DTSTART and `target`, less one for
/// safety - where an endless rule can start walking without missing anything.
fn periods_before(rule: &Rrule, d0: NaiveDate, week0: NaiveDate, target: NaiveDate) -> i64 {
    let elapsed = match rule.freq {
        Freq::Daily => (target - d0).num_days(),
        Freq::Weekly => (target - week0).num_days() / 7,
        Freq::Monthly => {
            (target.year() as i64 - d0.year() as i64) * 12 + target.month() as i64
                - d0.month() as i64
        }
        Freq::Yearly => target.year() as i64 - d0.year() as i64,
        Freq::Other => 0,
    };
    (elapsed / rule.interval - 1).max(0)
}

/// `UNTIL` as an inclusive instant. A date-only UNTIL includes that whole day.
fn until_instant(raw: &str, zone: &Zone) -> Option<DateTime<Utc>> {
    if let Some(stripped) = raw.strip_suffix('Z') {
        return Some(Utc.from_utc_datetime(&parse_naive(stripped)?));
    }
    if raw.len() == 8 {
        return zone.to_utc(parse_date(raw)?.and_hms_opt(23, 59, 59)?);
    }
    zone.to_utc(parse_naive(raw)?)
}

/// Expands a recurring event's starts **within `[from, to)`**, as
/// `(wall clock, UTC)` pairs.
///
/// ⚠️ Generated on DTSTART's wall clock in DTSTART's zone, then converted:
/// stepping the UTC instant instead moves every occurrence by an hour once
/// DST changes.
///
/// ⚠️ Bounded: an endless rule jumps straight to the window, so its age
/// doesn't matter (walking from DTSTART with a step cap meant a stand-up
/// older than ~2.7 years produced nothing at all). A `COUNT` rule has to be
/// walked from the start, since that's where it counts from - it ends by
/// itself. `MAX_PERIODS` is a last belt for a pathological rule.
///
/// `budget` is shared by every rule in the feed and counts periods walked.
/// The per-rule cap alone still let a feed of tens of thousands of
/// `COUNT` rules starting in year 1 walk billions of periods; running out
/// of budget fails the whole feed with [`TooComplex`] rather than quietly
/// importing part of it.
fn expand(
    rule: &Rrule,
    start: &IcsTime,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    budget: &mut u64,
) -> Result<Vec<(NaiveDateTime, DateTime<Utc>)>, TooComplex> {
    const MAX_PERIODS: i64 = 100_000;

    if rule.freq == Freq::Other {
        return Ok(start
            .utc()
            .filter(|utc| *utc >= from && *utc < to)
            .map(|utc| vec![(start.local, utc)])
            .unwrap_or_default());
    }

    let d0 = start.local.date();
    let time = start.local.time();
    let Some(week0) =
        d0.checked_sub_signed(Duration::days(days_after_wkst(d0.weekday(), rule.wkst)))
    else {
        return Ok(Vec::new());
    };
    let until = rule
        .until
        .as_deref()
        .and_then(|u| until_instant(u, &start.zone));
    // Zone offsets never exceed a day, so two days of slack either side
    // covers any wall-clock/UTC skew.
    let (Some(window_start), Some(window_end)) = (
        from.checked_sub_signed(Duration::days(2)),
        to.checked_add_signed(Duration::days(2)),
    ) else {
        return Ok(Vec::new());
    };
    let window_end = window_end.naive_utc().date();
    let first_k = match rule.count {
        Some(_) => 0,
        None => periods_before(rule, d0, week0, window_start.naive_utc().date()),
    };

    let mut out = Vec::new();
    let mut emitted = 0u32;
    for k in first_k..first_k.saturating_add(MAX_PERIODS) {
        *budget = budget.checked_sub(1).ok_or(TooComplex)?;
        let Some((period_start, dates)) = period(rule, d0, week0, k) else {
            break;
        };
        if period_start > window_end {
            break;
        }
        for date in dates {
            let local = date.and_time(time);
            if local < start.local {
                continue;
            }
            if rule.count.is_some_and(|count| emitted >= count) {
                return Ok(out);
            }
            emitted += 1;
            let Some(utc) = start.zone.to_utc(local) else {
                continue;
            };
            if until.is_some_and(|until| utc > until) || utc >= to {
                return Ok(out);
            }
            if utc >= from {
                out.push((local, utc));
            }
        }
    }
    Ok(out)
}

/// A feed whose rules would take more work to expand than any real calendar
/// needs. See [`expand`].
#[derive(Debug)]
struct TooComplex;

/// Recurrence periods one feed may walk in total. A busy real calendar walks
/// a few thousand; this leaves three orders of magnitude of headroom.
const MAX_PERIODS_PER_FEED: u64 = 5_000_000;

/// Busy intervals one feed may produce inside the sync window.
///
/// ⚠️ A cap, not a truncation: a feed over it fails with a readable error and
/// keeps whatever was cached before, because importing an arbitrary prefix
/// would make the user look free at times they aren't. A packed work calendar
/// is a few thousand blocks over the seven-month window.
pub const MAX_INTERVALS_PER_FEED: usize = 20_000;

// --- Events ----------------------------------------------------------------

/// What one VEVENT says about *when*, and nothing about *what*.
#[derive(Default)]
struct RawEvent {
    uid: Option<String>,
    start: Option<IcsTime>,
    end: Option<IcsTime>,
    duration: Option<Duration>,
    rrule: Option<Rrule>,
    exdates: Vec<IcsTime>,
    recurrence_id: Option<IcsTime>,
    // A cancelled event still appears in many exports; it should not block.
    cancelled: bool,
    // TRANSP:TRANSPARENT is the calendar's own way of saying "this does not
    // make me busy" - respect it rather than overruling it.
    transparent: bool,
}

fn collect_events(lines: &[String], ctx: &mut TzContext) -> Vec<RawEvent> {
    let mut events = Vec::new();
    let mut current: Option<RawEvent> = None;
    // Depth of sub-components (VALARM) inside the event: their DURATION and
    // TRIGGER describe the alarm, not the event.
    let mut nested = 0usize;

    for line in lines {
        let trimmed = line.trim_end();
        if trimmed.eq_ignore_ascii_case("BEGIN:VEVENT") {
            current = Some(RawEvent::default());
            nested = 0;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("END:VEVENT") {
            if let Some(event) = current.take() {
                events.push(event);
            }
            continue;
        }
        let Some(event) = current.as_mut() else {
            continue;
        };

        let upper = trimmed.to_ascii_uppercase();
        if upper.starts_with("BEGIN:") {
            nested += 1;
            continue;
        }
        if upper.starts_with("END:") {
            nested = nested.saturating_sub(1);
            continue;
        }
        if nested > 0 {
            continue;
        }

        let Some((name, params, value)) = split_property(trimmed) else {
            continue;
        };
        match name.as_str() {
            "UID" => event.uid = Some(value),
            "DTSTART" => event.start = parse_time(&params, &value, ctx),
            "DTEND" => event.end = parse_time(&params, &value, ctx),
            "DURATION" => event.duration = parse_duration(&value),
            "RRULE" => event.rrule = parse_rrule(&value),
            "RECURRENCE-ID" => event.recurrence_id = parse_time(&params, &value, ctx),
            "EXDATE" => {
                for one in value.split(',') {
                    if let Some(t) = parse_time(&params, one, ctx) {
                        event.exdates.push(t);
                    }
                }
            }
            "STATUS" => event.cancelled = value.eq_ignore_ascii_case("CANCELLED"),
            "TRANSP" => event.transparent = value.eq_ignore_ascii_case("TRANSPARENT"),
            _ => {}
        }
    }

    events
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
    let mut ctx = TzContext::from_lines(&lines);
    let events = collect_events(&lines, &mut ctx);

    // A moved or cancelled occurrence is a second VEVENT with the series' UID
    // and a RECURRENCE-ID naming the slot it replaces. Whatever it says, the
    // series no longer owns that slot.
    let mut replaced: HashMap<&str, Vec<&IcsTime>> = HashMap::new();
    for event in &events {
        if let (Some(uid), Some(slot)) = (&event.uid, &event.recurrence_id) {
            replaced.entry(uid.as_str()).or_default().push(slot);
        }
    }

    let mut out = Vec::new();
    let mut budget = MAX_PERIODS_PER_FEED;
    for event in &events {
        if event.cancelled || event.transparent {
            continue;
        }
        let Some(start) = &event.start else { continue };
        if start.all_day && !include_all_day {
            continue;
        }
        let Some(start_utc) = start.utc() else {
            continue;
        };

        // No DTEND and no DURATION: RFC 5545 says an all-day event lasts a
        // day and a timed one is instantaneous. An instant blocks nothing, so
        // it is skipped rather than stored as a zero-width interval nothing
        // can overlap.
        let length = match (&event.end, event.duration) {
            (Some(end), _) => match end.utc() {
                Some(end_utc) => end_utc.signed_duration_since(start_utc),
                None => continue,
            },
            (None, Some(d)) => d,
            (None, None) if start.all_day => Duration::days(1),
            (None, None) => continue,
        };
        if length <= Duration::zero() {
            continue;
        }

        let (occurrences, overrides) = match (&event.rrule, &event.recurrence_id) {
            (Some(rule), None) => {
                // An event this long overlaps every window anyway; one too
                // long to subtract is not a real event.
                let Some(reach) = from.checked_sub_signed(length) else {
                    continue;
                };
                let occurrences = expand(rule, start, reach, to, &mut budget).map_err(|_| {
                    anyhow!("This calendar's repeating events are too complex to import")
                })?;
                (
                    occurrences,
                    event.uid.as_deref().and_then(|uid| replaced.get(uid)),
                )
            }
            _ => (vec![(start.local, start_utc)], None),
        };

        for (local, occurrence) in occurrences {
            if event.exdates.iter().any(|x| x.names(local, occurrence)) {
                continue;
            }
            if overrides.is_some_and(|slots| slots.iter().any(|x| x.names(local, occurrence))) {
                continue;
            }
            let Some(end) = occurrence.checked_add_signed(length) else {
                continue;
            };
            let interval = BusyInterval {
                start: occurrence,
                end,
            };
            // Half-open on both sides, matching services::availability.
            if interval.start < to && from < interval.end {
                if out.len() >= MAX_INTERVALS_PER_FEED {
                    bail!(
                        "This calendar has more than {MAX_INTERVALS_PER_FEED} busy blocks in the next few months, too many to import"
                    );
                }
                out.push(interval);
            }
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

    // --- Repros: real provider feeds (fix-external-busy-display) ---------
    //
    // "External events from Google or Teams are not shown." Each test below
    // feeds the parser what a real provider writes, rather than the minimal
    // UTC events above, and asserts where the meeting actually is.

    fn parse_window(input: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<BusyInterval> {
        busy_intervals(input, from, to, false).unwrap()
    }

    /// What Outlook.com / Microsoft 365 publishes ("Publish calendar" ->
    /// ICS link), which is also where Teams meetings live: Windows zone
    /// names in TZID and a VTIMEZONE block describing them.
    const OUTLOOK_FEED: &str = "BEGIN:VCALENDAR\r
METHOD:PUBLISH\r
PRODID:Microsoft Exchange Server 2010\r
VERSION:2.0\r
X-WR-CALNAME:Calendar\r
BEGIN:VTIMEZONE\r
TZID:Romance Standard Time\r
BEGIN:STANDARD\r
DTSTART:16010101T030000\r
TZOFFSETFROM:+0200\r
TZOFFSETTO:+0100\r
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=10\r
END:STANDARD\r
BEGIN:DAYLIGHT\r
DTSTART:16010101T020000\r
TZOFFSETFROM:+0100\r
TZOFFSETTO:+0200\r
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=3\r
END:DAYLIGHT\r
END:VTIMEZONE\r
BEGIN:VTIMEZONE\r
TZID:Pacific Standard Time\r
BEGIN:STANDARD\r
DTSTART:16010101T020000\r
TZOFFSETFROM:-0700\r
TZOFFSETTO:-0800\r
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=1SU;BYMONTH=11\r
END:STANDARD\r
BEGIN:DAYLIGHT\r
DTSTART:16010101T020000\r
TZOFFSETFROM:-0800\r
TZOFFSETTO:-0700\r
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=2SU;BYMONTH=3\r
END:DAYLIGHT\r
END:VTIMEZONE\r
BEGIN:VEVENT\r
UID:040000008200E00074C5B7101A82E00800000000A0B1C2D3E4F5\r
SUMMARY:Weekly sync (Microsoft Teams Meeting)\r
DTSTART;TZID=Romance Standard Time:20261102T090000\r
DTEND;TZID=Romance Standard Time:20261102T093000\r
CLASS:PUBLIC\r
PRIORITY:5\r
DTSTAMP:20261028T101500Z\r
TRANSP:OPAQUE\r
STATUS:CONFIRMED\r
SEQUENCE:0\r
LOCATION:Microsoft Teams Meeting\r
X-MICROSOFT-CDO-BUSYSTATUS:BUSY\r
X-MICROSOFT-CDO-INTENDEDSTATUS:BUSY\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:040000008200E00074C5B7101A82E00800000000F0E1D2C3B4A5\r
SUMMARY:Call with the Seattle office\r
DTSTART;TZID=Pacific Standard Time:20261103T090000\r
DTEND;TZID=Pacific Standard Time:20261103T100000\r
TRANSP:OPAQUE\r
STATUS:CONFIRMED\r
X-MICROSOFT-CDO-BUSYSTATUS:BUSY\r
END:VEVENT\r
END:VCALENDAR\r
";

    // ⚠️ `"Romance Standard Time".parse::<Tz>()` fails, so the TZID is
    // silently dropped and 09:00 Paris is read as 09:00 UTC - an hour late.
    // For the Pacific meeting it is eight hours out: a 09:00 call lands at
    // 02:00 the next morning in Paris instead of 18:00 - on the wrong day.
    #[test]
    fn outlook_windows_timezone_names_are_resolved() {
        let out = parse_window(OUTLOOK_FEED, at(2026, 11, 1, 0), at(2026, 11, 8, 0));

        assert_eq!(out.len(), 2, "both Outlook meetings are busy: {out:?}");
        // Paris is UTC+1 in November.
        assert_eq!(out[0].start, at(2026, 11, 2, 8), "Romance Standard Time");
        // Los Angeles is UTC-8 after the first Sunday of November.
        assert_eq!(out[1].start, at(2026, 11, 3, 17), "Pacific Standard Time");
    }

    // ⚠️ Outlook desktop / Exchange on-premises exports name the zone by its
    // display string, which contains a colon and so is quoted (RFC 5545
    // §3.1: a param value containing ':' MUST be quoted). The property is
    // split on the *first* colon, which is inside the quotes, so DTSTART's
    // value becomes `00) Amsterdam, ...":20261102T090000`, fails to parse,
    // and the event is dropped outright.
    #[test]
    fn a_quoted_tzid_containing_a_colon_does_not_drop_the_event() {
        let feed = cal("BEGIN:VEVENT\n\
DTSTART;TZID=\"(UTC+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna\":20261102T090000\n\
DTEND;TZID=\"(UTC+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna\":20261102T100000\n\
END:VEVENT");

        let out = parse_window(&feed, at(2026, 11, 1, 0), at(2026, 11, 8, 0));

        assert_eq!(out.len(), 1, "the meeting was dropped: {out:?}");
    }

    // ⚠️ `expand` walks from DTSTART with a hard cap of 1000 steps, and a
    // weekly rule with BYDAY steps one *day* at a time. Both Google and
    // Outlook write BYDAY on weekly rules, so a stand-up that has existed
    // since 2 Jan 2023 runs out of steps on 29 Sep 2025 and contributes
    // nothing to any later window - the longest-standing meetings, which are
    // most of a work calendar, vanish completely.
    #[test]
    fn a_recurring_meeting_that_started_years_ago_still_blocks_this_month() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20230102T090000Z\nDTEND:20230102T091500Z\nRRULE:FREQ=WEEKLY;WKST=MO;BYDAY=MO,TU,WE,TH,FR\nEND:VEVENT",
        );

        assert_eq!(out.len(), 22, "every weekday of March 2026: {out:?}");
    }

    #[test]
    fn a_daily_rule_that_started_years_ago_still_blocks_this_month() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20230102T090000Z\nDTEND:20230102T091500Z\nRRULE:FREQ=DAILY\nEND:VEVENT",
        );

        assert_eq!(out.len(), 31, "every day of March 2026");
    }

    // With BYDAY, the WEEKLY branch steps a day at a time and never looks at
    // INTERVAL, so a fortnightly 1:1 shows every week.
    #[test]
    fn a_fortnightly_rule_with_byday_skips_alternate_weeks() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260303T090000Z\nDTEND:20260303T100000Z\nRRULE:FREQ=WEEKLY;INTERVAL=2;BYDAY=TU\nEND:VEVENT",
        );

        let starts: Vec<_> = out.iter().map(|i| i.start).collect();
        assert_eq!(
            starts,
            vec![at(2026, 3, 3, 9), at(2026, 3, 17, 9), at(2026, 3, 31, 9)]
        );
    }

    // ⚠️ Occurrences are generated by adding whole days to the *UTC* start,
    // so a 09:00 Paris meeting stays at 08:00 UTC after the clocks change on
    // 29 March and is drawn at 10:00 local from then on. Google writes
    // `TZID=Europe/Paris` on every recurring event, so this shifts every
    // recurring meeting by an hour for the half of the year after DST flips.
    #[test]
    fn a_zoned_recurring_meeting_keeps_its_local_time_across_dst() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART;TZID=Europe/Paris:20260302T090000\nDTEND;TZID=Europe/Paris:20260302T100000\nRRULE:FREQ=WEEKLY;BYDAY=MO\nEND:VEVENT",
        );

        assert_eq!(out.len(), 5);
        assert_eq!(out[3].start, at(2026, 3, 23, 8), "before DST: UTC+1");
        assert_eq!(out[4].start, at(2026, 3, 30, 7), "after DST: UTC+2");
    }

    // Google and Outlook both express "this one occurrence moved" as a second
    // VEVENT with the same UID and a RECURRENCE-ID naming the slot it
    // replaces. Unhandled, the master rule still blocks the old slot, so the
    // moved meeting shows twice.
    #[test]
    fn a_moved_occurrence_no_longer_blocks_its_original_slot() {
        let out = parse(
            "BEGIN:VEVENT\nUID:standup@google.com\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nRRULE:FREQ=WEEKLY;BYDAY=MO\nEND:VEVENT\n\
BEGIN:VEVENT\nUID:standup@google.com\nRECURRENCE-ID:20260309T090000Z\nDTSTART:20260309T140000Z\nDTEND:20260309T150000Z\nEND:VEVENT",
        );

        let on_the_ninth: Vec<_> = out
            .iter()
            .filter(|i| i.start.date_naive() == at(2026, 3, 9, 0).date_naive())
            .map(|i| i.start)
            .collect();
        assert_eq!(on_the_ninth, vec![at(2026, 3, 9, 14)]);
    }

    // --- Beyond the repros: the rest of what real feeds write -----------

    // Outlook's display-name form resolves to a real zone, not just "kept":
    // Amsterdam is UTC+1 in November.
    #[test]
    fn a_quoted_display_name_tzid_resolves_to_its_city() {
        let feed = cal("BEGIN:VEVENT\n\
DTSTART;TZID=\"(UTC+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna\":20261102T090000\n\
DTEND;TZID=\"(UTC+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna\":20261102T100000\n\
END:VEVENT");

        let out = parse_window(&feed, at(2026, 11, 1, 0), at(2026, 11, 8, 0));

        assert_eq!(out[0].start, at(2026, 11, 2, 8));
    }

    // A zone no table knows ("Customized Time Zone" is what Outlook writes
    // for an edited zone) is read from the feed's own VTIMEZONE, DST and all.
    #[test]
    fn an_unknown_tzid_is_read_from_its_vtimezone() {
        let feed = "BEGIN:VCALENDAR\r
BEGIN:VTIMEZONE\r
TZID:Customized Time Zone\r
BEGIN:STANDARD\r
DTSTART:16010101T030000\r
TZOFFSETFROM:+0200\r
TZOFFSETTO:+0100\r
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=10\r
END:STANDARD\r
BEGIN:DAYLIGHT\r
DTSTART:16010101T020000\r
TZOFFSETFROM:+0100\r
TZOFFSETTO:+0200\r
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=3\r
END:DAYLIGHT\r
END:VTIMEZONE\r
BEGIN:VEVENT\r
DTSTART;TZID=Customized Time Zone:20260302T090000\r
DTEND;TZID=Customized Time Zone:20260302T100000\r
RRULE:FREQ=WEEKLY;BYDAY=MO\r
END:VEVENT\r
END:VCALENDAR\r
";

        let out = parse_window(feed, at(2026, 3, 1, 0), at(2026, 4, 1, 0));

        assert_eq!(out.len(), 5);
        assert_eq!(out[3].start, at(2026, 3, 23, 8), "winter: UTC+1");
        assert_eq!(out[4].start, at(2026, 3, 30, 7), "summer: UTC+2");
    }

    // Google writes X-WR-TIMEZONE; a floating time belongs to it.
    #[test]
    fn a_floating_time_uses_the_calendars_own_zone() {
        let feed = "BEGIN:VCALENDAR\r\nX-WR-TIMEZONE:Europe/Paris\r\nBEGIN:VEVENT\r\nDTSTART:20260302T090000\r\nDTEND:20260302T100000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

        let out = parse_window(feed, at(2026, 3, 1, 0), at(2026, 4, 1, 0));

        assert_eq!(out[0].start, at(2026, 3, 2, 8));
    }

    // A cancelled occurrence (an override with STATUS:CANCELLED) frees the
    // slot and blocks nothing in its place.
    #[test]
    fn a_cancelled_occurrence_frees_its_slot() {
        let out = parse(
            "BEGIN:VEVENT\nUID:standup@google.com\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nRRULE:FREQ=WEEKLY;BYDAY=MO\nEND:VEVENT\n\
BEGIN:VEVENT\nUID:standup@google.com\nRECURRENCE-ID:20260309T090000Z\nDTSTART:20260309T090000Z\nDTEND:20260309T100000Z\nSTATUS:CANCELLED\nEND:VEVENT",
        );

        assert_eq!(out.len(), 4);
        assert!(!out.iter().any(|i| i.start == at(2026, 3, 9, 9)));
    }

    // An override only touches its own series.
    #[test]
    fn an_override_does_not_touch_another_series() {
        let out = parse(
            "BEGIN:VEVENT\nUID:a\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nRRULE:FREQ=WEEKLY;BYDAY=MO\nEND:VEVENT\n\
BEGIN:VEVENT\nUID:b\nRECURRENCE-ID:20260309T090000Z\nDTSTART:20260309T140000Z\nDTEND:20260309T150000Z\nEND:VEVENT",
        );

        assert!(out.iter().any(|i| i.start == at(2026, 3, 9, 9)));
        assert!(out.iter().any(|i| i.start == at(2026, 3, 9, 14)));
    }

    // EXDATE in the series' own zone has to match after DST, which it only
    // does if occurrences are generated on the wall clock.
    #[test]
    fn a_zoned_exdate_after_dst_still_matches() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART;TZID=Europe/Paris:20260302T090000\nDTEND;TZID=Europe/Paris:20260302T100000\nRRULE:FREQ=WEEKLY;BYDAY=MO\nEXDATE;TZID=Europe/Paris:20260330T090000\nEND:VEVENT",
        );

        assert_eq!(out.len(), 4);
        assert!(
            !out.iter()
                .any(|i| i.start.date_naive() == at(2026, 3, 30, 0).date_naive())
        );
    }

    // ⚠️ COUNT counts from DTSTART, so skipping ahead must not reset it: ten
    // days from 25 Feb leaves six in March.
    #[test]
    fn count_is_counted_from_dtstart_not_from_the_window() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260225T090000Z\nDTEND:20260225T093000Z\nRRULE:FREQ=DAILY;COUNT=10\nEND:VEVENT",
        );
        assert_eq!(out.len(), 6);

        let long_finished = parse(
            "BEGIN:VEVENT\nDTSTART:20230102T090000Z\nDTEND:20230102T093000Z\nRRULE:FREQ=WEEKLY;BYDAY=MO;COUNT=5\nEND:VEVENT",
        );
        assert!(long_finished.is_empty());
    }

    // Outlook's "second Tuesday of every month" and "last Friday".
    #[test]
    fn monthly_byday_ordinals_pick_the_right_week() {
        let out = busy_intervals(
            &cal("BEGIN:VEVENT\nDTSTART:20260113T090000Z\nDTEND:20260113T100000Z\nRRULE:FREQ=MONTHLY;BYDAY=2TU\nEND:VEVENT\n\
BEGIN:VEVENT\nDTSTART:20260130T170000Z\nDTEND:20260130T180000Z\nRRULE:FREQ=MONTHLY;BYDAY=-1FR\nEND:VEVENT"),
            at(2026, 3, 1, 0),
            at(2026, 5, 1, 0),
            false,
        )
        .unwrap();

        let mut starts: Vec<_> = out.iter().map(|i| i.start).collect();
        starts.sort();
        assert_eq!(
            starts,
            vec![
                at(2026, 3, 10, 9),
                at(2026, 3, 27, 17),
                at(2026, 4, 14, 9),
                at(2026, 4, 24, 17)
            ]
        );
    }

    // RFC 5545: an occurrence that doesn't exist is skipped, not moved.
    #[test]
    fn a_monthly_rule_on_the_31st_skips_short_months() {
        let out = busy_intervals(
            &cal(
                "BEGIN:VEVENT\nDTSTART:20260131T090000Z\nDTEND:20260131T100000Z\nRRULE:FREQ=MONTHLY\nEND:VEVENT",
            ),
            at(2026, 2, 1, 0),
            at(2026, 6, 1, 0),
            false,
        )
        .unwrap();

        let starts: Vec<_> = out.iter().map(|i| i.start).collect();
        assert_eq!(starts, vec![at(2026, 3, 31, 9), at(2026, 5, 31, 9)]);
    }

    // A VALARM's DURATION is the alarm's repeat spacing, not the event's.
    #[test]
    fn an_alarm_does_not_change_the_events_length() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDURATION:PT1H\nBEGIN:VALARM\nTRIGGER:-PT15M\nDURATION:PT5M\nREPEAT:2\nACTION:DISPLAY\nEND:VALARM\nEND:VEVENT",
        );

        assert_eq!(out[0].end, at(2026, 3, 2, 10));
    }

    // ⚠️ The bound that matters: an endless rule from decades ago is as cheap
    // as one from last week, and still lands on the right days.
    #[test]
    fn an_ancient_endless_rule_stays_bounded_and_correct() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:19700105T090000Z\nDTEND:19700105T091500Z\nRRULE:FREQ=WEEKLY;BYDAY=MO\nEND:VEVENT",
        );

        let starts: Vec<_> = out.iter().map(|i| i.start).collect();
        assert_eq!(
            starts,
            vec![
                at(2026, 3, 2, 9),
                at(2026, 3, 9, 9),
                at(2026, 3, 16, 9),
                at(2026, 3, 23, 9),
                at(2026, 3, 30, 9)
            ]
        );
    }

    // --- Verifier: recurrence edge cases ---------------------------------

    fn starts(out: &[BusyInterval]) -> Vec<DateTime<Utc>> {
        let mut s: Vec<_> = out.iter().map(|i| i.start).collect();
        s.sort();
        s
    }

    // COUNT with several BYDAYs: twenty occurrences from Mon 2 Feb on
    // Mon/Wed/Fri are twelve in February and the first eight in March.
    #[test]
    fn count_with_byday_is_counted_per_occurrence_not_per_week() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260202T090000Z\nDTEND:20260202T100000Z\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR;COUNT=20\nEND:VEVENT",
        );
        let s = starts(&out);
        assert_eq!(s.len(), 8, "{s:?}");
        assert_eq!(*s.last().unwrap(), at(2026, 3, 18, 9));
    }

    // A BYDAY day earlier in DTSTART's own week is not an occurrence and must
    // not use up the COUNT.
    #[test]
    fn byday_before_dtstart_in_its_first_week_does_not_consume_count() {
        // Wed 25 Feb; BYDAY=MO,WE; COUNT=4 -> Feb 25, Mar 2, Mar 4, Mar 9.
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260225T090000Z\nDTEND:20260225T100000Z\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE;COUNT=4\nEND:VEVENT",
        );
        assert_eq!(
            starts(&out),
            vec![at(2026, 3, 2, 9), at(2026, 3, 4, 9), at(2026, 3, 9, 9)]
        );
    }

    // A date-only UNTIL (common in hand-made and older feeds) includes its
    // whole day.
    #[test]
    fn a_date_only_until_includes_its_last_day() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nRRULE:FREQ=DAILY;UNTIL=20260305\nEND:VEVENT",
        );
        assert_eq!(
            starts(&out),
            vec![
                at(2026, 3, 2, 9),
                at(2026, 3, 3, 9),
                at(2026, 3, 4, 9),
                at(2026, 3, 5, 9)
            ]
        );
    }

    // What Google writes: a zoned DTSTART with UNTIL in UTC. Inclusive.
    #[test]
    fn a_utc_until_on_a_zoned_series_is_inclusive_and_dst_correct() {
        // 23 March 09:00 Paris is 08:00Z (before DST).
        let inclusive = parse(
            "BEGIN:VEVENT\nDTSTART;TZID=Europe/Paris:20260302T090000\nDTEND;TZID=Europe/Paris:20260302T100000\nRRULE:FREQ=WEEKLY;BYDAY=MO;UNTIL=20260323T080000Z\nEND:VEVENT",
        );
        assert_eq!(inclusive.len(), 4, "{inclusive:?}");
        let exclusive = parse(
            "BEGIN:VEVENT\nDTSTART;TZID=Europe/Paris:20260302T090000\nDTEND;TZID=Europe/Paris:20260302T100000\nRRULE:FREQ=WEEKLY;BYDAY=MO;UNTIL=20260323T075959Z\nEND:VEVENT",
        );
        assert_eq!(exclusive.len(), 3, "{exclusive:?}");
    }

    // EXDATE in UTC on a zoned series, after DST: 30 March 09:00 Paris is
    // 07:00Z, not 08:00Z.
    #[test]
    fn a_utc_exdate_on_a_zoned_series_after_dst_matches() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART;TZID=Europe/Paris:20260302T090000\nDTEND;TZID=Europe/Paris:20260302T100000\nRRULE:FREQ=WEEKLY;BYDAY=MO\nEXDATE:20260330T070000Z\nEND:VEVENT",
        );
        assert_eq!(out.len(), 4, "{out:?}");
        assert!(!out.iter().any(|i| i.start == at(2026, 3, 30, 7)));
    }

    // Skip-ahead must stay aligned to INTERVAL from DTSTART, not from the
    // window.
    #[test]
    fn skip_ahead_keeps_weekly_and_monthly_intervals_aligned() {
        // Every third Monday from 6 Jan 2025: 60 weeks to 2 Mar 2026.
        let weekly = parse(
            "BEGIN:VEVENT\nDTSTART:20250106T090000Z\nDTEND:20250106T100000Z\nRRULE:FREQ=WEEKLY;INTERVAL=3;BYDAY=MO\nEND:VEVENT",
        );
        assert_eq!(starts(&weekly), vec![at(2026, 3, 2, 9), at(2026, 3, 23, 9)]);

        // Every fifth month from 15 Jan 2020: 75 months later is April 2026.
        let monthly = busy_intervals(
            &cal(
                "BEGIN:VEVENT\nDTSTART:20200115T090000Z\nDTEND:20200115T100000Z\nRRULE:FREQ=MONTHLY;INTERVAL=5\nEND:VEVENT",
            ),
            at(2026, 3, 1, 0),
            at(2026, 5, 1, 0),
            false,
        )
        .unwrap();
        assert_eq!(starts(&monthly), vec![at(2026, 4, 15, 9)]);
    }

    // The bound, timed: endless rules from 1970 at every frequency, over the
    // production window (6 weeks back, 6 months ahead).
    #[test]
    fn endless_rules_from_1970_are_fast_over_the_production_window() {
        let feed = cal(
            "BEGIN:VEVENT\nDTSTART:19700101T090000Z\nDTEND:19700101T091500Z\nRRULE:FREQ=DAILY\nEND:VEVENT\n\
BEGIN:VEVENT\nDTSTART:19700105T090000Z\nDTEND:19700105T091500Z\nRRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR\nEND:VEVENT\n\
BEGIN:VEVENT\nDTSTART:19700113T090000Z\nDTEND:19700113T091500Z\nRRULE:FREQ=MONTHLY;BYDAY=2TU\nEND:VEVENT\n\
BEGIN:VEVENT\nDTSTART:19700301T090000Z\nDTEND:19700301T091500Z\nRRULE:FREQ=YEARLY\nEND:VEVENT\n\
BEGIN:VEVENT\nDTSTART;TZID=Europe/Paris:19700101T090000\nDTEND;TZID=Europe/Paris:19700101T091500\nRRULE:FREQ=DAILY\nEND:VEVENT",
        );
        let from = at(2026, 3, 1, 0) - Duration::days(42);
        let to = at(2026, 3, 1, 0) + Duration::days(183);
        let started = std::time::Instant::now();
        let out = busy_intervals(&feed, from, to, false).unwrap();
        let took = started.elapsed();
        // 225 days: 225 + ~161 weekdays + ~7 + 1 + 225.
        assert!(out.len() > 600 && out.len() < 650, "{}", out.len());
        assert!(took < std::time::Duration::from_millis(500), "{took:?}");
        // Paris daily keeps 09:00 local across the March DST change.
        assert!(out.iter().any(|i| i.start == at(2026, 3, 28, 8)));
        assert!(out.iter().any(|i| i.start == at(2026, 3, 30, 7)));
    }

    // An override whose RECURRENCE-ID is written in a different zone than the
    // master still names the same instant. 9 March 09:00 Paris (08:00Z) is
    // 04:00 in New York, which is already on DST that day.
    #[test]
    fn an_override_with_a_different_tzid_still_replaces_its_slot() {
        let out = parse(
            "BEGIN:VEVENT\nUID:x\nDTSTART;TZID=Europe/Paris:20260302T090000\nDTEND;TZID=Europe/Paris:20260302T100000\nRRULE:FREQ=WEEKLY;BYDAY=MO\nEND:VEVENT\n\
BEGIN:VEVENT\nUID:x\nRECURRENCE-ID;TZID=America/New_York:20260309T040000\nDTSTART:20260309T140000Z\nDTEND:20260309T150000Z\nEND:VEVENT",
        );
        let s = starts(&out);
        assert!(!s.contains(&at(2026, 3, 9, 8)), "{s:?}");
        assert!(s.contains(&at(2026, 3, 9, 14)), "{s:?}");
        assert_eq!(s.len(), 5);
    }

    // An all-day series with an all-day override (a day off moved by one).
    #[test]
    fn an_all_day_override_moves_its_day() {
        let out = busy_intervals(
            &cal(
                "BEGIN:VEVENT\nUID:off\nDTSTART;VALUE=DATE:20260302\nDTEND;VALUE=DATE:20260303\nRRULE:FREQ=WEEKLY;BYDAY=MO\nEND:VEVENT\n\
BEGIN:VEVENT\nUID:off\nRECURRENCE-ID;VALUE=DATE:20260309\nDTSTART;VALUE=DATE:20260310\nDTEND;VALUE=DATE:20260311\nEND:VEVENT",
            ),
            at(2026, 3, 1, 0),
            at(2026, 4, 1, 0),
            true,
        )
        .unwrap();
        let s = starts(&out);
        assert!(!s.contains(&at(2026, 3, 9, 0)), "{s:?}");
        assert!(s.contains(&at(2026, 3, 10, 0)), "{s:?}");
        assert_eq!(s.len(), 5);
    }

    // `outlook_windows_timezone_names_are_resolved` stays green with the
    // Windows table switched off, because its feed carries VTIMEZONE blocks
    // and those resolve the zone on their own. This one has none, so only
    // the table can place it - and it pins that the parser consults it.
    #[test]
    fn a_windows_tzid_without_a_vtimezone_resolves_through_the_table() {
        let feed = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART;TZID=Romance Standard Time:20260706T090000\r\nDTEND;TZID=Romance Standard Time:20260706T100000\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;TZID=Pacific Standard Time:20260707T090000\r\nDTEND;TZID=Pacific Standard Time:20260707T100000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

        let out = parse_window(feed, at(2026, 7, 1, 0), at(2026, 7, 31, 0));

        // Summer: Paris UTC+2, Los Angeles UTC-7.
        assert_eq!(starts(&out), vec![at(2026, 7, 6, 7), at(2026, 7, 7, 16)]);
    }

    // --- Verifier: hostile feed data must not panic ----------------------
    //
    // The feed is whatever URL a user pasted. `sync_all` runs every user's
    // feed inside the one spawned hourly task, so a panic here stops
    // *everyone's* sync until the process restarts, and the connect handler
    // calls `sync_one` inline.

    fn does_not_panic(body: &str) {
        let feed = cal(body);
        let result = std::panic::catch_unwind(|| {
            busy_intervals(&feed, at(2026, 3, 1, 0), at(2026, 4, 1, 0), true)
        });
        assert!(result.is_ok(), "the parser panicked on: {body}");
    }

    // `parse_byday` uses `split_at(len - 2)` on bytes.
    #[test]
    fn a_non_ascii_byday_does_not_panic() {
        does_not_panic(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nRRULE:FREQ=WEEKLY;BYDAY=éa\nEND:VEVENT",
        );
    }

    // `nth_weekday` computes `last - 7 * (-n - 1)` days unchecked.
    #[test]
    fn a_huge_negative_byday_ordinal_does_not_panic() {
        does_not_panic(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nRRULE:FREQ=MONTHLY;BYDAY=-100000000MO\nEND:VEVENT",
        );
    }

    #[test]
    fn a_huge_negative_ordinal_in_a_vtimezone_does_not_panic() {
        does_not_panic(
            "BEGIN:VTIMEZONE\nTZID:Odd\nBEGIN:STANDARD\nDTSTART:16010101T030000\nTZOFFSETTO:+0100\nRRULE:FREQ=YEARLY;BYDAY=-2147483648SU;BYMONTH=10\nEND:STANDARD\nEND:VTIMEZONE\n\
BEGIN:VEVENT\nDTSTART;TZID=Odd:20260302T090000\nDTEND;TZID=Odd:20260302T100000\nEND:VEVENT",
        );
    }

    // `Duration::weeks`/`days` panic out of range, and `from - length`
    // overflows chrono's date range for a merely large one.
    #[test]
    fn a_huge_duration_does_not_panic() {
        does_not_panic(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDURATION:P99999999999999W\nEND:VEVENT",
        );
        does_not_panic("BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDURATION:P100000000D\nEND:VEVENT");
    }

    // `display_name_offset` multiplies the hours in i32 unchecked.
    #[test]
    fn a_huge_display_name_offset_does_not_panic() {
        does_not_panic(
            "BEGIN:VEVENT\nDTSTART;TZID=\"(UTC+999999:00) Nowhere\":20260302T090000\nDTEND;TZID=\"(UTC+999999:00) Nowhere\":20260302T100000\nEND:VEVENT",
        );
    }

    // --- Builder: further hostile-feed cases found auditing the parser ----

    // ⚠️ Not just "no panic": in a release build 1193046 * 3600 wraps to
    // -1696 seconds, which `FixedOffset` accepts, so the unchecked multiply
    // read this as UTC-00:28:16 instead of rejecting it. An unrecognised
    // zone reads as UTC.
    #[test]
    fn a_display_name_offset_that_would_wrap_is_rejected_not_wrapped() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART;TZID=\"(UTC+1193046:00) Nowhere\":20260302T090000\nDTEND;TZID=\"(UTC+1193046:00) Nowhere\":20260302T100000\nEND:VEVENT",
        );

        assert_eq!(out[0].start, at(2026, 3, 2, 9));
    }

    #[test]
    fn a_real_display_name_offset_still_applies() {
        let out = parse(
            "BEGIN:VEVENT\nDTSTART;TZID=\"(UTC+05:30) Nowhere\":20260302T090000\nDTEND;TZID=\"(UTC+05:30) Nowhere\":20260302T100000\nEND:VEVENT",
        );

        assert_eq!(
            out[0].start,
            Utc.with_ymd_and_hms(2026, 3, 2, 3, 30, 0).unwrap()
        );
    }

    // `last_day_of_month` did `month + 1` on a feed-supplied BYMONTH.
    #[test]
    fn an_out_of_range_bymonth_in_a_vtimezone_does_not_panic() {
        does_not_panic(
            "BEGIN:VTIMEZONE\nTZID:Odd\nBEGIN:STANDARD\nDTSTART:16010101T030000\nTZOFFSETTO:+0100\nRRULE:FREQ=YEARLY;BYDAY=-1SU;BYMONTH=4294967295\nEND:STANDARD\nEND:VTIMEZONE\n\
BEGIN:VEVENT\nDTSTART;TZID=Odd:20260302T090000\nDTEND;TZID=Odd:20260302T100000\nEND:VEVENT",
        );
    }

    // `add_months_to_first` added the step unchecked.
    #[test]
    fn a_huge_monthly_interval_does_not_panic() {
        does_not_panic(
            "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nRRULE:FREQ=MONTHLY;INTERVAL=9223372036854775807\nEND:VEVENT",
        );
    }

    // Dates at the edge of chrono's range made the zone and window
    // arithmetic overflow; four-digit years keep everything well inside it.
    #[test]
    fn a_year_outside_four_digits_is_ignored_not_a_panic() {
        does_not_panic(
            "BEGIN:VEVENT\nDTSTART;TZID=Europe/Paris:+2621430101T090000\nDTEND;TZID=Europe/Paris:+2621430101T100000\nRRULE:FREQ=WEEKLY\nEND:VEVENT",
        );
        does_not_panic(
            "BEGIN:VEVENT\nDTSTART:00000101T000000Z\nDURATION:P1D\nRRULE:FREQ=DAILY\nEND:VEVENT",
        );
    }

    // ⚠️ A cap, not a truncation: importing an arbitrary prefix would make
    // somebody look free when they aren't.
    #[test]
    fn a_feed_with_too_many_busy_blocks_is_refused_with_a_reason() {
        let events: String = (0..=MAX_INTERVALS_PER_FEED)
            .map(|_| "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nEND:VEVENT\n")
            .collect();

        let err =
            busy_intervals(&cal(&events), at(2026, 3, 1, 0), at(2026, 4, 1, 0), false).unwrap_err();

        assert!(err.to_string().contains("too many to import"), "{err}");
    }

    #[test]
    fn a_feed_exactly_at_the_cap_still_imports() {
        let events: String = (0..MAX_INTERVALS_PER_FEED)
            .map(|_| "BEGIN:VEVENT\nDTSTART:20260302T090000Z\nDTEND:20260302T100000Z\nEND:VEVENT\n")
            .collect();

        let out =
            busy_intervals(&cal(&events), at(2026, 3, 1, 0), at(2026, 4, 1, 0), false).unwrap();

        assert_eq!(out.len(), MAX_INTERVALS_PER_FEED);
    }

    // The per-rule period cap alone let many `COUNT` rules from year 1 walk
    // 100k periods each. The feed-wide budget stops that with an error.
    #[test]
    fn a_feed_whose_rules_take_too_long_to_expand_is_refused() {
        let events: String = (0..60)
            .map(|_| "BEGIN:VEVENT\nDTSTART:00010101T090000Z\nDTEND:00010101T100000Z\nRRULE:FREQ=DAILY;INTERVAL=1;COUNT=4000000000\nEND:VEVENT\n")
            .collect();

        let err =
            busy_intervals(&cal(&events), at(2026, 3, 1, 0), at(2026, 4, 1, 0), false).unwrap_err();

        assert!(err.to_string().contains("too complex"), "{err}");
    }

    // A real Google public feed, fetched live. All-day by nature, so it is
    // parsed with `include_all_day` on. Run with `--ignored`.
    #[test]
    #[ignore = "needs network"]
    fn a_real_google_public_feed_parses() {
        let url = "https://calendar.google.com/calendar/ical/en.french%23holiday%40group.v.calendar.google.com/public/basic.ics";
        let body = std::process::Command::new("curl")
            .args(["-sSfL", "-m", "30", url])
            .output()
            .expect("curl");
        assert!(body.status.success(), "fetch failed");
        let body = String::from_utf8(body.stdout).unwrap();
        let from = Utc::now() - Duration::days(42);
        let to = Utc::now() + Duration::days(365);
        // Google marks every holiday TRANSP:TRANSPARENT, so as served the
        // feed blocks nothing, all-day or not - which is right.
        let as_served = busy_intervals(&body, from, to, true).unwrap();
        assert!(as_served.is_empty(), "{as_served:?}");
        // Made opaque, the same events must parse as all-day days.
        let opaque = body.replace("TRANSP:TRANSPARENT", "TRANSP:OPAQUE");
        let all = busy_intervals(&opaque, from, to, true).unwrap();
        let timed = busy_intervals(&opaque, from, to, false).unwrap();
        eprintln!(
            "{} bytes, {} all-day intervals in the window, {} timed",
            body.len(),
            all.len(),
            timed.len()
        );
        assert!(!all.is_empty());
        assert!(all.iter().all(|i| i.end - i.start == Duration::days(1)));
        assert!(timed.is_empty(), "holidays are all-day");
    }
}

// --- Verifier: seeded fuzzing of `busy_intervals` ---------------------------
//
// The feed is any URL a user pasted. These generate structured-but-hostile
// calendars (and plain garbage), mutate them at the character level, and
// assert the parser neither panics nor runs away. No dependencies: a seeded
// xorshift, so any failure is reproducible from the seed and case it prints.
//
// `ICS_FUZZ_CASES` raises the case count (the default keeps `cargo test`
// quick) and `ICS_FUZZ_SEED` (hex) changes the seed. Run it in release too,
// where overflow wraps instead of panicking.
#[cfg(test)]
mod fuzz {
    use super::*;
    use std::time::{Duration as StdDuration, Instant};

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
        fn chance(&mut self, percent: u64) -> bool {
            self.next() % 100 < percent
        }
        fn pick<'a>(&mut self, pool: &[&'a str]) -> &'a str {
            pool[self.below(pool.len())]
        }
    }

    const DATES: &[&str] = &[
        "20260302T090000Z",
        "20260302T090000",
        "20260302",
        "20260315T183000",
        "20251026T023000",
        "20260329T023000",
        "20200101T000000Z",
        "99991231T235959Z",
        "99991231T235959",
        "99991231",
        "00010101T000000",
        "00010101T000000Z",
        "00010101",
        "+2621430101T090000",
        "-2621430101T090000",
        "",
        "2026",
        "20261302T250000",
        "20260229",
        "20240229T120000Z",
        "1",
        "-1",
        "999999999999999999999999",
        "20260302T090000ZZ",
        "éééééééé",
        "2026030é",
        "20260302T09000é",
    ];

    const TZIDS: &[&str] = &[
        "Europe/Paris",
        "America/New_York",
        "\"(UTC+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna\"",
        "\"(UTC-05:00) Bogotá, Lima, Quito\"",
        "\"(UTC-05:00) Bogot\u{fffd}, Lima, Quito\"",
        "Custom",
        "Other",
        "/mozilla.org/20050126_1/Europe/Paris",
        "\"(UTC+999999:00) Nowhere\"",
        "\"(UTC-14:59) x\"",
        "\"(UTC+1193046:00) x\"",
        "\"(UTC-2147483648:00) x\"",
        "\"(UTC+\"",
        "\"(UTC\"",
        "\"(\"",
        "\"()\"",
        "\"(UTC+é) é\"",
        "\"(UTC) , , ,é,\"",
        "W. Europe Standard Time",
        "Canada Central Standard Time",
        "Mountain Standard Time",
        "Pacific Standard Time",
        "",
        "\"",
        "é",
        "//////",
        "/é/é/",
        "Etc/GMT+12",
        "Etc/GMT-14",
        "UTC",
    ];

    const DURATIONS: &[&str] = &[
        "P1D",
        "PT1H30M",
        "PT45M",
        "P99999999999999W",
        "PT9223372036854775807S",
        "P9223372036854775807D",
        "PT9223372036854775807H",
        "P1W9223372036854775807D",
        "-PT1H",
        "P",
        "PT",
        "P1Y",
        "P1DT",
        "PT-1H",
        "P1W2D",
        "PT0S",
        "Pé",
        "PT1é",
        "P10675199DT2H48M5S",
    ];

    const OFFSETS: &[&str] = &[
        "+0100",
        "-0800",
        "+053000",
        "+9999",
        "-99999999",
        "+",
        "",
        "+12",
        "+01:00",
        "+0000",
        "-0000",
        "+2359",
        "+995999",
        "é0100",
        "+é100",
        "+01é0",
    ];

    const FREQS: &[&str] = &[
        "DAILY", "WEEKLY", "MONTHLY", "YEARLY", "HOURLY", "SECONDLY", "", "daily", "é",
    ];
    const INTERVALS: &[&str] = &[
        "0",
        "1",
        "2",
        "3",
        "-5",
        "1000000",
        "9223372036854775807",
        "-9223372036854775808",
        "abc",
        "",
    ];
    const COUNTS: &[&str] = &["0", "1", "5", "100", "4294967295", "-1", "99999999999", "é"];
    const BYDAYS: &[&str] = &[
        "MO",
        "TU,TH",
        "2TU",
        "-1SU",
        "+5FR",
        "6MO",
        "-6MO",
        "-100000000MO",
        "-2147483648MO",
        "2147483647MO",
        "99999999999MO",
        "éa",
        "é",
        "aé",
        "",
        "XX",
        "1",
        "MO,MO,MO",
        ",,,",
        "+-1MO",
        "--1MO",
        "MO,TU,WE,TH,FR,SA,SU",
        "1MO,2MO,3MO,4MO,5MO,-1MO,-2MO,-3MO,-4MO,-5MO",
    ];
    const MONTHDAYS: &[&str] = &[
        "1",
        "31",
        "-1",
        "-31",
        "0",
        "32",
        "-32",
        "2147483647",
        "-2147483648",
        "1,15,-1",
        "é",
        "29,30,31",
    ];
    const MONTHS: &[&str] = &["0", "1", "3", "10", "12", "13", "4294967295", "-1", "é", ""];
    const WKSTS: &[&str] = &["SU", "MO", "XX", "", "é"];
    const WEIRD: &[&str] = &[
        "é",
        "\u{10FFFF}",
        "\u{0}",
        "\r",
        "\n",
        "\n ",
        "\r\n\t",
        "\t",
        ":",
        ";",
        "\"",
        "=",
        ",",
        "Z",
        "T",
        "-",
        "+",
        "9999999999999999999999",
        "\u{fffd}",
        "日本",
        "🎉",
        "BEGIN:VEVENT\n",
        "END:VEVENT\n",
        "BEGIN:VALARM\n",
        "END:VTIMEZONE\n",
    ];

    fn rrule(r: &mut Rng) -> String {
        let mut parts = vec![format!("FREQ={}", r.pick(FREQS))];
        for _ in 0..r.below(6) {
            parts.push(match r.below(8) {
                0 => format!("INTERVAL={}", r.pick(INTERVALS)),
                1 => format!("COUNT={}", r.pick(COUNTS)),
                2 => format!("UNTIL={}", r.pick(DATES)),
                3 => format!("BYDAY={}", r.pick(BYDAYS)),
                4 => format!("BYMONTHDAY={}", r.pick(MONTHDAYS)),
                5 => format!("BYMONTH={}", r.pick(MONTHS)),
                6 => format!("WKST={}", r.pick(WKSTS)),
                _ => r
                    .pick(&["BYSETPOS=-1", "X", "=", "FREQ=", ";;", "é=é"])
                    .to_string(),
            });
        }
        if r.chance(10) {
            parts.reverse();
        }
        parts.join(";")
    }

    fn time_prop(r: &mut Rng, name: &str) -> String {
        match r.below(4) {
            0 => format!("{name}:{}", r.pick(DATES)),
            1 => format!("{name};VALUE=DATE:{}", r.pick(DATES)),
            _ => format!("{name};TZID={}:{}", r.pick(TZIDS), r.pick(DATES)),
        }
    }

    fn vtimezone(r: &mut Rng) -> String {
        let mut s = format!(
            "BEGIN:VTIMEZONE\nTZID:{}\n",
            r.pick(&[
                "Custom",
                "Other",
                "Europe/Paris",
                "",
                "é",
                "(UTC-05:00) Bogotá, Lima, Quito"
            ])
        );
        for _ in 0..r.below(4) {
            let kind = r.pick(&["STANDARD", "DAYLIGHT"]);
            s.push_str(&format!("BEGIN:{kind}\n"));
            if r.chance(85) {
                s.push_str(&format!("DTSTART:{}\n", r.pick(DATES)));
            }
            if r.chance(85) {
                s.push_str(&format!("TZOFFSETTO:{}\n", r.pick(OFFSETS)));
            }
            s.push_str(&format!("TZOFFSETFROM:{}\n", r.pick(OFFSETS)));
            if r.chance(70) {
                s.push_str(&format!(
                    "RRULE:FREQ=YEARLY;BYMONTH={};BYDAY={}\n",
                    r.pick(MONTHS),
                    r.pick(BYDAYS)
                ));
            }
            s.push_str(&format!("END:{kind}\n"));
        }
        s.push_str("END:VTIMEZONE\n");
        s
    }

    fn vevent(r: &mut Rng) -> String {
        let mut s = String::from("BEGIN:VEVENT\n");
        if r.chance(60) {
            s.push_str(&format!("UID:{}\n", r.pick(&["a", "b", "", "é"])));
        }
        if r.chance(95) {
            s.push_str(&time_prop(r, "DTSTART"));
            s.push('\n');
        }
        match r.below(3) {
            0 => {
                s.push_str(&time_prop(r, "DTEND"));
                s.push('\n');
            }
            1 => s.push_str(&format!("DURATION:{}\n", r.pick(DURATIONS))),
            _ => {}
        }
        if r.chance(70) {
            s.push_str(&format!("RRULE:{}\n", rrule(r)));
        }
        for _ in 0..r.below(3) {
            let list: Vec<&str> = (0..1 + r.below(3)).map(|_| r.pick(DATES)).collect();
            if r.chance(50) {
                s.push_str(&format!("EXDATE:{}\n", list.join(",")));
            } else {
                s.push_str(&format!(
                    "EXDATE;TZID={}:{}\n",
                    r.pick(TZIDS),
                    list.join(",")
                ));
            }
        }
        if r.chance(15) {
            s.push_str(&time_prop(r, "RECURRENCE-ID"));
            s.push('\n');
        }
        if r.chance(10) {
            s.push_str("STATUS:CANCELLED\n");
        }
        if r.chance(10) {
            s.push_str("TRANSP:TRANSPARENT\n");
        }
        if r.chance(15) {
            s.push_str("BEGIN:VALARM\nTRIGGER:-PT15M\nDURATION:PT5M\nEND:VALARM\n");
        }
        s.push_str("END:VEVENT\n");
        s
    }

    fn calendar(r: &mut Rng) -> String {
        let mut s = String::from("BEGIN:VCALENDAR\n");
        if r.chance(30) {
            s.push_str(&format!("X-WR-TIMEZONE:{}\n", r.pick(TZIDS)));
        }
        for _ in 0..r.below(3) {
            s.push_str(&vtimezone(r));
        }
        for _ in 0..1 + r.below(5) {
            s.push_str(&vevent(r));
        }
        s.push_str("END:VCALENDAR\n");
        if r.chance(50) {
            s = s.replace('\n', "\r\n");
        }
        s
    }

    fn char_cut(s: &str, at: usize) -> usize {
        let mut at = at.min(s.len());
        while !s.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    fn mutate(r: &mut Rng, mut s: String) -> String {
        for _ in 0..r.below(6) {
            if s.is_empty() {
                break;
            }
            let a = char_cut(&s, r.below(s.len() + 1));
            let b = char_cut(&s, a + r.below(40)).max(a);
            match r.below(6) {
                0 => s.replace_range(a..b, ""),
                1 => s.insert_str(a, r.pick(WEIRD)),
                2 => s.truncate(a),
                3 => {
                    let piece = s[a..b].to_string();
                    s.insert_str(b, &piece);
                }
                // Fold a line at an arbitrary point.
                4 => s.insert_str(a, r.pick(&["\r\n ", "\n\t", "\n "])),
                _ => {
                    let n = r.pick(&["0", "9", "99999999999999999999", "-1", "+"]);
                    s.replace_range(a..b, n);
                }
            }
        }
        s
    }

    fn garbage(r: &mut Rng) -> String {
        let bytes: Vec<u8> = (0..r.below(400)).map(|_| r.next() as u8).collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn window(r: &mut Rng) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc.with_ymd_and_hms(2026, 3, 20, 12, 0, 0).unwrap();
        match r.below(5) {
            0 => (
                Utc.with_ymd_and_hms(1, 1, 1, 0, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(9999, 12, 31, 0, 0, 0).unwrap(),
            ),
            1 => (
                Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap(),
            ),
            // What `external_calendar::sync_one` actually asks for.
            _ => (now - Duration::days(42), now + Duration::days(183)),
        }
    }

    fn run(seed: u64, cases: u64, per_case_limit: StdDuration) {
        let mut slowest = (StdDuration::ZERO, 0u64);
        let mut errors = 0u64;
        let mut produced = 0usize;
        for case in 0..cases {
            let mut r = Rng((seed ^ case.wrapping_mul(0x9E37_79B9_7F4A_7C15)) | 1);
            let input = match r.below(10) {
                0 => garbage(&mut r),
                1..=3 => calendar(&mut r),
                _ => {
                    let c = calendar(&mut r);
                    mutate(&mut r, c)
                }
            };
            let (from, to) = window(&mut r);
            let include_all_day = r.chance(50);
            let started = Instant::now();
            let result =
                std::panic::catch_unwind(|| busy_intervals(&input, from, to, include_all_day));
            let took = started.elapsed();
            match result {
                Err(_) => panic!(
                    "busy_intervals panicked (seed {seed:#x}, case {case}, window {from}..{to}, all_day {include_all_day}) on:\n{input:?}"
                ),
                Ok(Ok(out)) => produced += out.len(),
                Ok(Err(_)) => errors += 1,
            }
            if took > slowest.0 {
                slowest = (took, case);
            }
            assert!(
                took < per_case_limit,
                "busy_intervals took {took:?} (seed {seed:#x}, case {case}) on:\n{input:?}"
            );
        }
        eprintln!(
            "fuzzed {cases} feeds from seed {seed:#x}: {produced} intervals, {errors} refused; slowest {:?} (case {})",
            slowest.0, slowest.1
        );
    }

    fn cases() -> u64 {
        std::env::var("ICS_FUZZ_CASES")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(3_000)
    }

    fn seed() -> u64 {
        std::env::var("ICS_FUZZ_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0x05EE_D1C5)
    }

    // --- Headroom: the caps must not refuse a real heavy work calendar ----

    /// 300 recurring series (20 daily-on-weekdays stand-ups, 130 weekly,
    /// 100 fortnightly, 50 monthly, half of them started years ago and a
    /// third counted with COUNT from their start) plus 3000 one-off
    /// meetings, in Paris time with a VTIMEZONE, over the real sync window.
    fn heavy_work_calendar(now: DateTime<Utc>) -> String {
        let mut s = String::from(
            "BEGIN:VCALENDAR\r\nX-WR-TIMEZONE:Europe/Paris\r\n\
BEGIN:VTIMEZONE\r\nTZID:Europe/Paris\r\n\
BEGIN:DAYLIGHT\r\nDTSTART:19810329T020000\r\nTZOFFSETTO:+0200\r\nRRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\nEND:DAYLIGHT\r\n\
BEGIN:STANDARD\r\nDTSTART:19961027T030000\r\nTZOFFSETTO:+0100\r\nRRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\nEND:STANDARD\r\n\
END:VTIMEZONE\r\n",
        );
        let days = ["MO", "TU", "WE", "TH", "FR"];
        for i in 0..300u32 {
            let started = if i % 2 == 0 { 2016 + i % 9 } else { 2026 };
            let rule = match i {
                0..20 => "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR".to_string(),
                20..150 => format!("FREQ=WEEKLY;BYDAY={}", days[i as usize % 5]),
                150..250 => format!("FREQ=WEEKLY;INTERVAL=2;BYDAY={}", days[i as usize % 5]),
                _ => format!("FREQ=MONTHLY;BYDAY={}{}", 1 + i % 4, days[i as usize % 5]),
            };
            let rule = if i % 3 == 0 {
                format!("{rule};COUNT=2000")
            } else {
                rule
            };
            let hour = 8 + i % 10;
            s.push_str(&format!(
                "BEGIN:VEVENT\r\nUID:series-{i}\r\nDTSTART;TZID=Europe/Paris:{started}0105T{hour:02}{:02}00\r\nDTEND;TZID=Europe/Paris:{started}0105T{hour:02}{:02}00\r\nRRULE:{rule}\r\nEXDATE;TZID=Europe/Paris:20260406T{hour:02}0000\r\nEND:VEVENT\r\n",
                (i % 2) * 30,
                (i % 2) * 30 + 25,
            ));
        }
        let first = (now - Duration::days(42)).date_naive();
        for j in 0..3000i64 {
            let day = first + Duration::days(j % 225);
            let hour = 8 + j % 10;
            s.push_str(&format!(
                "BEGIN:VEVENT\r\nUID:one-{j}\r\nDTSTART;TZID=Europe/Paris:{}T{hour:02}1500\r\nDURATION:PT45M\r\nEND:VEVENT\r\n",
                day.format("%Y%m%d"),
            ));
        }
        s.push_str("END:VCALENDAR\r\n");
        s
    }

    #[test]
    fn a_heavy_real_work_calendar_fits_well_inside_every_cap() {
        let now = Utc.with_ymd_and_hms(2026, 3, 20, 12, 0, 0).unwrap();
        let feed = heavy_work_calendar(now);
        let started = Instant::now();

        let out = busy_intervals(
            &feed,
            now - Duration::days(42),
            now + Duration::days(183),
            false,
        )
        .unwrap();

        eprintln!(
            "heavy calendar: {} bytes, {} intervals, parsed in {:?}",
            feed.len(),
            out.len(),
            started.elapsed()
        );
        assert!(
            feed.len() < 5 * 1024 * 1024 / 4,
            "body well under the size cap"
        );
        assert!(
            out.len() * 3 / 2 < MAX_INTERVALS_PER_FEED,
            "{} intervals leaves under 1.5x headroom",
            out.len()
        );
    }

    // --- Work that no cap counts --------------------------------------------
    //
    // Every series occurrence is checked against every RECURRENCE-ID sharing
    // its UID, and every EXDATE on it, before it is counted towards
    // `MAX_INTERVALS_PER_FEED`. An overridden occurrence is never pushed, so
    // the interval cap never fires; the period budget counts periods, not
    // comparisons. Only the 5 MB body bounds it.
    fn override_storm(series: usize, overrides: usize, cancel_every_slot: bool) -> String {
        let mut s = String::from("BEGIN:VCALENDAR\r\n");
        for _ in 0..series {
            s.push_str("BEGIN:VEVENT\r\nUID:x\r\nDTSTART:20260201T090000Z\r\nDTEND:20260201T100000Z\r\nRRULE:FREQ=DAILY\r\nEND:VEVENT\r\n");
        }
        let first = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        for k in 0..overrides {
            // Distinct slots, none of which the series has, so every
            // occurrence scans the whole list.
            let d = first + Duration::days((k % 300) as i64);
            s.push_str(&format!(
                "BEGIN:VEVENT\r\nUID:x\r\nRECURRENCE-ID:{}T{:02}{:02}00Z\r\nEND:VEVENT\r\n",
                d.format("%Y%m%d"),
                10 + (k / 300) % 12,
                (k / 3600) % 60
            ));
        }
        if cancel_every_slot {
            // Last, so each occurrence scans the whole list before it finds
            // its own. Every occurrence is then skipped - none is pushed, so
            // the interval cap cannot fire however many series there are.
            for day in 0..300 {
                let d = first + Duration::days(day);
                s.push_str(&format!(
                    "BEGIN:VEVENT\r\nUID:x\r\nRECURRENCE-ID:{}T090000Z\r\nEND:VEVENT\r\n",
                    d.format("%Y%m%d")
                ));
            }
        }
        s.push_str("END:VCALENDAR\r\n");
        s
    }

    // ⚠️ Repro. 400 series sharing a UID with 38,000 RECURRENCE-IDs, every
    // occurrence cancelled by the last few: 2.5 MB, half the size cap, and
    // no cap fires because nothing is pushed. The cost is
    // occurrences x overrides; measured in release at 10.6 s for 200 series
    // and 108 s for 2,000, linear, so 20,000 series (4.7 MB, still under
    // the size cap and under the 5M period budget) holds a sync for ~18
    // minutes. `sync_all` awaits feeds one at a time, so that is everybody's
    // sync, every hour, and the connect request hangs for as long.
    #[test]
    fn many_overrides_on_a_shared_uid_cost_linear_not_quadratic_time() {
        let now = Utc.with_ymd_and_hms(2026, 3, 20, 12, 0, 0).unwrap();
        let feed = override_storm(400, 38_000, true);
        assert!(feed.len() < 5 * 1024 * 1024 / 2);
        let started = Instant::now();

        let out = busy_intervals(
            &feed,
            now - Duration::days(42),
            now + Duration::days(183),
            false,
        );

        let took = started.elapsed();
        assert!(out.is_ok_and(|o| o.is_empty()));
        assert!(
            took < StdDuration::from_secs(5),
            "a 2.5 MB feed held the parser for {took:?}"
        );
    }

    #[test]
    #[ignore = "timing probe; run with --ignored --nocapture"]
    fn override_storm_cost() {
        let now = Utc.with_ymd_and_hms(2026, 3, 20, 12, 0, 0).unwrap();
        let scale: usize = std::env::var("STORM_SERIES")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(200);
        for (series, overrides, cancel) in [
            (1, 1000, false),
            (10, 10_000, false),
            (80, 60_000, false),
            (scale, 38_000, true),
        ] {
            let feed = override_storm(series, overrides, cancel);
            let started = Instant::now();
            let out = busy_intervals(
                &feed,
                now - Duration::days(42),
                now + Duration::days(183),
                false,
            );
            eprintln!(
                "{series} series x {overrides} overrides: {} bytes, {:?} -> {:?}",
                feed.len(),
                started.elapsed(),
                out.map(|o| o.len()).map_err(|e| e.to_string())
            );
        }
    }

    #[test]
    fn hostile_generated_feeds_never_panic_or_run_away() {
        // Generous: an unoptimised build on a shared container. The point is
        // "terminates", not a benchmark.
        run(seed(), cases(), StdDuration::from_secs(10));
    }
}
