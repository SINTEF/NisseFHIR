//! FHIR date search parameters: precision-aware prefix parsing and SQL range
//! comparison.
//!
//! FHIR dates may appear at several precisions:
//!
//! | Precision          | Example              | Implicit period           |
//! | ------------------ | -------------------- | -------------------------- |
//! | Year               | `1974`               | [1974-01-01, 1975-01-01)   |
//! | Year + month        | `1974-12`            | [1974-12-01, 1975-01-01)   |
//! | Date               | `1974-12-25`         | [1974-12-25, 1974-12-26)   |
//! | Date time (minute) | `1974-12-25T10:30Z`  | [instant, instant + 1m)    |
//! | Date time (second) | `1974-12-25T10:30:00Z` | [instant, instant + 1s)  |
//!
//! A search value (with optional prefix) is parsed into an inclusive
//! `start` / exclusive `end` pair expressed in UTC. The same
//! precision-expansion is applied to the resource's stored date value inside
//! the SQL `WHERE` clause (see [`push_date_predicate`]) so that comparisons
//! honour FHIR's range comparison semantics.
//!
//! ## Prefix semantics
//!
//! Given a search period `[s_start, s_end)` and a resource period
//! `[r_start, r_end)` (both half-open):
//!
//! | Prefix | Match condition                               | Meaning                         |
//! | ------ | --------------------------------------------- | ------------------------------- |
//! | `eq`   | `r_start >= s_start AND r_end <= s_end`       | search fully contains resource  |
//! | `ne`   | `r_start < s_start OR r_end > s_end`          | search does not contain resource|
//! | `gt`   | `r_end > s_end`                               | some resource range is above    |
//! | `ge`   | `r_end > s_start`                             | resource overlaps/is above      |
//! | `lt`   | `r_start < s_start`                           | some resource range is below    |
//! | `le`   | `r_start < s_end`                             | resource overlaps/is below      |
//! | `sa`   | `r_start >= s_end`                            | resource starts after search    |
//! | `eb`   | `r_end <= s_start`                            | resource ends before search     |
//! | `ap`   | overlap after widening search by 10% duration | approximately the same          |
//!
//! FHIR leaves the exact tolerance for `ap` to servers. This implementation
//! uses a deterministic 10% of the search value's precision period on each
//! side. Fractional seconds are accepted but intentionally compared at whole
//! second precision, which FHIR explicitly permits servers to do. DateTimes
//! without a timezone are interpreted as UTC for deterministic behavior.
//!
//! ## Invalid input
//!
//! Malformed dates, impossible calendar values (`2024-13-..`), unsupported
//! two-letter prefixes (`xx2020`), and negative or unparseable instants are
//! rejected with a descriptive error message. Callers translate these into a
//! `400` HTTP response via [`crate::error::AppError::BadRequest`].

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use sqlx::{Postgres, QueryBuilder};

/// The set of FHIR date comparator prefixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatePrefix {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Sa,
    Eb,
    Ap,
}

impl DatePrefix {
    /// Parse a two-character FHIR comparator prefix. Returns `None` when the
    /// leading two characters are not a registered prefix (in which case the
    /// input is treated as a prefix-less `eq` date).
    pub fn parse(input: &str) -> Option<Self> {
        let head = input.get(..2)?;
        Some(match head {
            "eq" => Self::Eq,
            "ne" => Self::Ne,
            "gt" => Self::Gt,
            "ge" => Self::Ge,
            "lt" => Self::Lt,
            "le" => Self::Le,
            "sa" => Self::Sa,
            "eb" => Self::Eb,
            "ap" => Self::Ap,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Gt => "gt",
            Self::Ge => "ge",
            Self::Lt => "lt",
            Self::Le => "le",
            Self::Sa => "sa",
            Self::Eb => "eb",
            Self::Ap => "ap",
        }
    }
}

/// A half-open `[start, end)` UTC period expanded from a FHIR date value.
#[derive(Debug, Clone, Copy)]
pub struct DateBounds {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl DateBounds {
    /// Compute the duration of the period in milliseconds.
    fn duration_ms(self) -> i64 {
        (self.end - self.start).num_milliseconds()
    }

    /// Widen the period symmetrically by `fraction` (e.g. `0.1` for 10%) of
    /// the period duration. Used to implement `ap`.
    fn widened(self, fraction: f64) -> Self {
        let tol_ms = (self.duration_ms() as f64 * fraction).round() as i64;
        let tolerance = chrono::Duration::milliseconds(tol_ms);
        Self {
            start: self.start - tolerance,
            end: self.end + tolerance,
        }
    }
}

/// Parse a FHIR date search value, peeling off an optional comparator prefix.
///
/// The returned [`DateBounds`] are always UTC and half-open. Returns an
/// `Err(message)` when the value cannot be parsed; callers should surface the
/// message as a `400` response.
pub fn parse_fhir_date_value(value: &str) -> Result<(DatePrefix, DateBounds), String> {
    if value.is_empty() {
        return Err("date search value must not be empty".to_owned());
    }

    let (prefix, body) = match DatePrefix::parse(value) {
        Some(prefix) => (prefix, &value[2..]),
        None => (DatePrefix::Eq, value),
    };

    Ok((prefix, parse_fhir_date(body)?))
}

/// Parse the date body (without prefix) into half-open UTC bounds.
pub fn parse_fhir_date(value: &str) -> Result<DateBounds, String> {
    if value.is_empty() {
        return Err("date search value must not be empty".to_owned());
    }

    // Datetime form: contains 'T' or 't' separator per ISO 8601 / FHIR.
    if let Some(idx) = value.find(['T', 't']) {
        return parse_fhir_datetime(&value[..idx], &value[idx + 1..])
            .map_err(|e| format!("invalid FHIR dateTime '{value}': {e}"));
    }

    // Pure date forms.
    let segments: Vec<&str> = value.split('-').collect();
    match segments.len() {
        1 => parse_year(segments[0]).map_err(|e| format!("invalid FHIR date '{value}': {e}")),
        2 => parse_year_month(segments[0], segments[1])
            .map_err(|e| format!("invalid FHIR date '{value}': {e}")),
        3 => parse_year_month_day(segments[0], segments[1], segments[2])
            .map_err(|e| format!("invalid FHIR date '{value}': {e}")),
        _ => Err(format!("invalid FHIR date '{value}'")),
    }
}

fn parse_year(year_str: &str) -> Result<DateBounds, String> {
    let year = parse_year_component(year_str)?;

    let start = instant_utc(year, 1, 1, 0, 0, 0)?;
    let end_year = year.checked_add(1).ok_or("year overflow")?;
    let end = instant_utc(end_year, 1, 1, 0, 0, 0)?;
    Ok(DateBounds { start, end })
}

fn parse_year_month(year_str: &str, month_str: &str) -> Result<DateBounds, String> {
    let year = parse_year_component(year_str)?;
    let month = parse_month_component(month_str)?;
    let start = instant_utc(year, month, 1, 0, 0, 0)?;
    let (end_year, end_month) = if month == 12 {
        (year.checked_add(1).ok_or("year overflow")?, 1)
    } else {
        (year, month + 1)
    };
    let end = instant_utc(end_year, end_month, 1, 0, 0, 0)?;
    Ok(DateBounds { start, end })
}

fn parse_year_month_day(
    year_str: &str,
    month_str: &str,
    day_str: &str,
) -> Result<DateBounds, String> {
    let year = parse_year_component(year_str)?;
    let month = parse_month_component(month_str)?;
    let day = parse_day_component(day_str)?;
    let naive = NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("invalid calendar date {year}-{month:02}-{day:02}"))?;
    let start = naive.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let end = (naive + chrono::Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    Ok(DateBounds { start, end })
}

fn parse_fhir_datetime(date_part: &str, time_part: &str) -> Result<DateBounds, String> {
    let date_segments: Vec<&str> = date_part.split('-').collect();
    if date_segments.len() != 3 {
        return Err("dateTime must use YYYY-MM-DD prefix".to_owned());
    }
    let year = parse_year_component(date_segments[0])?;
    let month = parse_month_component(date_segments[1])?;
    let day = parse_day_component(date_segments[2])?;

    // Split optional timezone designator off the time portion.
    let (tz_start, tz_suffix) = time_part
        .find(['Z', 'z', '+', '-'])
        .map(|idx| (&time_part[..idx], Some(&time_part[idx..])))
        .unwrap_or((time_part, None));

    // Reject empty time component ("YYYY-MM-DDT").
    if tz_start.is_empty() {
        return Err("dateTime time component is missing".to_owned());
    }

    // FHIR permits minute or second precision once a time is present. An hour
    // must always be followed by minutes; seconds default to zero.
    let time_segments: Vec<&str> = tz_start.split(':').collect();
    if !(2..=3).contains(&time_segments.len()) {
        return Err("dateTime must use HH:MM or HH:MM:SS time precision".to_owned());
    }
    let hour = parse_two_digit(time_segments.first().copied().unwrap_or(""), 0, 23, "hour")?;
    let minute = parse_two_digit(time_segments[1], 0, 59, "minute")?;
    let second_raw = time_segments.get(2).copied().unwrap_or("");
    let (second_str, fraction) = second_raw
        .split_once('.')
        .map_or((second_raw, None), |(second, fraction)| {
            (second, Some(fraction))
        });
    if let Some(fraction) = fraction
        && (fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err("fractional seconds must contain digits".to_owned());
    }
    let second = if time_segments.len() == 2 {
        0
    } else if second_str.is_empty() {
        return Err("second must be two digits".to_owned());
    } else {
        parse_two_digit(second_str, 0, 60, "second")?
    };

    // Optional fractional seconds — accepted but truncated (precision beyond
    // seconds is not honoured for date comparisons).
    let naive = NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("invalid calendar date {year}-{month:02}-{day:02}"))?
        .and_hms_opt(hour, minute, second)
        .ok_or_else(|| "invalid time of day".to_owned())?;

    let start = match tz_suffix {
        None => naive.and_utc(),
        Some("Z") | Some("z") => DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc),
        Some(offset) => {
            let fixed = parse_fixed_offset(offset)?;
            let with_offset = fixed
                .from_local_datetime(&naive)
                .single()
                .ok_or_else(|| "invalid local datetime/offset combination".to_owned())?;
            with_offset.with_timezone(&Utc)
        }
    };

    let end = start
        + if time_segments.len() == 2 {
            chrono::Duration::minutes(1)
        } else {
            chrono::Duration::seconds(1)
        };
    Ok(DateBounds { start, end })
}

fn parse_fixed_offset(offset: &str) -> Result<chrono::FixedOffset, String> {
    let rest = &offset[1..];
    if rest.len() != 5 || rest.as_bytes().get(2).copied() != Some(b':') {
        return Err(format!(
            "invalid timezone offset '{offset}' (expected ±HH:MM)"
        ));
    }
    let sign = match &offset[..1] {
        "+" => 1,
        "-" => -1,
        _ => return Err("invalid timezone offset sign".to_owned()),
    };
    let hours: i32 = rest[..2]
        .parse()
        .map_err(|_| "invalid timezone offset hours".to_owned())?;
    let minutes: i32 = rest[3..]
        .parse()
        .map_err(|_| "invalid timezone offset minutes".to_owned())?;
    if hours > 14 || minutes > 59 || (hours == 14 && minutes != 0) {
        return Err("timezone offset out of range".to_owned());
    }
    chrono::FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60))
        .ok_or_else(|| "timezone offset out of range".to_owned())
}

fn instant_utc(
    year: i32,
    month: u32,
    day: u32,
    h: u32,
    m: u32,
    s: u32,
) -> Result<DateTime<Utc>, String> {
    let naive = NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("invalid calendar date {year}-{month:02}-{day:02}"))?
        .and_hms_opt(h, m, s)
        .ok_or_else(|| "invalid time of day".to_owned())?;
    Ok(naive.and_utc())
}

fn parse_year_component(s: &str) -> Result<i32, String> {
    if s.len() != 4 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err("year must be four digits".to_owned());
    }
    let year = s.parse().map_err(|_| "year is out of range".to_owned())?;
    if year == 0 {
        return Err("year must be greater than zero".to_owned());
    }
    Ok(year)
}

fn parse_month_component(s: &str) -> Result<u32, String> {
    parse_two_digit(s, 1, 12, "month")
}

fn parse_day_component(s: &str) -> Result<u32, String> {
    parse_two_digit(s, 1, 31, "day")
}

fn parse_two_digit(s: &str, min: u32, max: u32, label: &str) -> Result<u32, String> {
    if s.len() != 2 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{label} must be two digits"));
    }
    let value: u32 = s.parse().map_err(|_| format!("{label} is out of range"))?;
    if !(min..=max).contains(&value) {
        return Err(format!("{label} is out of range"));
    }
    Ok(value)
}

// ---------------------------------------------------------------------------
// SQL generation
// ---------------------------------------------------------------------------

/// Append a precision-aware date comparison predicate for the given resource
/// date expression.
///
/// `r_json_expr` must be a SQL fragment yielding a JSONB primitive date value
/// or a FHIR Period object. Bounds come from the parsed search value (already
/// in UTC). Every client-supplied value is bound as a parameter.
pub fn push_date_predicate(
    query: &mut QueryBuilder<Postgres>,
    r_json_expr: &str,
    prefix: DatePrefix,
    bounds: DateBounds,
) {
    let r_start = resource_start_sql(r_json_expr);
    let r_end = resource_end_sql(r_json_expr);

    query.push(" AND (");
    match prefix {
        DatePrefix::Eq => {
            // The parameter range fully contains the resource range.
            query.push(&r_start);
            query.push(" >= ");
            query.push_bind(bounds.start);
            query.push(" AND ");
            query.push(&r_end);
            query.push(" <= ");
            query.push_bind(bounds.end);
        }
        DatePrefix::Ne => {
            query.push(&r_start);
            query.push(" < ");
            query.push_bind(bounds.start);
            query.push(" OR ");
            query.push(&r_end);
            query.push(" > ");
            query.push_bind(bounds.end);
        }
        DatePrefix::Gt => {
            query.push(&r_end);
            query.push(" > ");
            query.push_bind(bounds.end);
        }
        DatePrefix::Ge => {
            query.push(&r_end);
            query.push(" > ");
            query.push_bind(bounds.start);
        }
        DatePrefix::Lt => {
            query.push(&r_start);
            query.push(" < ");
            query.push_bind(bounds.start);
        }
        DatePrefix::Le => {
            query.push(&r_start);
            query.push(" < ");
            query.push_bind(bounds.end);
        }
        DatePrefix::Sa => {
            query.push(&r_start);
            query.push(" >= ");
            query.push_bind(bounds.end);
        }
        DatePrefix::Eb => {
            query.push(&r_end);
            query.push(" <= ");
            query.push_bind(bounds.start);
        }
        DatePrefix::Ap => {
            let widened = bounds.widened(0.1);
            query.push(&r_start);
            query.push(" < ");
            query.push_bind(widened.end);
            query.push(" AND ");
            query.push_bind(widened.start);
            query.push(" < ");
            query.push(&r_end);
        }
    }
    query.push(")");
}

/// Return SQL for the lower bound of a primitive FHIR date stored as text.
fn primitive_start_sql(expr: &str) -> String {
    format!(
        "(CASE \
           WHEN {expr} IS NULL THEN NULL \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}-\\d{{2}}[Tt]\\d{{2}}:\\d{{2}}([Zz]|[+-]\\d{{2}}:\\d{{2}})?$' \
             AND {expr} ~ '([Zz]|[+-]\\d{{2}}:\\d{{2}})$' THEN date_trunc('minute', ({expr})::timestamptz) \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}-\\d{{2}}[Tt]\\d{{2}}:\\d{{2}}$' THEN date_trunc('minute', ({expr})::timestamp) AT TIME ZONE 'UTC' \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}-\\d{{2}}[Tt]\\d{{2}}' \
             AND {expr} ~ '([Zz]|[+-]\\d{{2}}:\\d{{2}})$' THEN date_trunc('second', ({expr})::timestamptz) \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}-\\d{{2}}[Tt]\\d{{2}}' THEN date_trunc('second', ({expr})::timestamp) AT TIME ZONE 'UTC' \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}-\\d{{2}}$' THEN ({expr} || 'T00:00:00Z')::timestamptz \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}$' THEN ({expr} || '-01T00:00:00Z')::timestamptz \
           WHEN {expr} ~ '^\\d{{4}}$' THEN ({expr} || '-01-01T00:00:00Z')::timestamptz \
           ELSE NULL \
         END)"
    )
}

/// Return SQL for the exclusive upper bound of a primitive FHIR date.
fn primitive_end_sql(expr: &str) -> String {
    format!(
        "(CASE \
           WHEN {expr} IS NULL THEN NULL \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}-\\d{{2}}[Tt]\\d{{2}}:\\d{{2}}([Zz]|[+-]\\d{{2}}:\\d{{2}})?$' \
             AND {expr} ~ '([Zz]|[+-]\\d{{2}}:\\d{{2}})$' THEN date_trunc('minute', ({expr})::timestamptz) + INTERVAL '1 minute' \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}-\\d{{2}}[Tt]\\d{{2}}:\\d{{2}}$' THEN (date_trunc('minute', ({expr})::timestamp) + INTERVAL '1 minute') AT TIME ZONE 'UTC' \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}-\\d{{2}}[Tt]\\d{{2}}' \
             AND {expr} ~ '([Zz]|[+-]\\d{{2}}:\\d{{2}})$' THEN date_trunc('second', ({expr})::timestamptz) + INTERVAL '1 second' \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}-\\d{{2}}[Tt]\\d{{2}}' THEN (date_trunc('second', ({expr})::timestamp) + INTERVAL '1 second') AT TIME ZONE 'UTC' \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}-\\d{{2}}$' THEN (({expr} || 'T00:00:00')::timestamp + INTERVAL '1 day') AT TIME ZONE 'UTC' \
           WHEN {expr} ~ '^\\d{{4}}-\\d{{2}}$' THEN (({expr} || '-01T00:00:00')::timestamp + INTERVAL '1 month') AT TIME ZONE 'UTC' \
           WHEN {expr} ~ '^\\d{{4}}$' THEN (({expr} || '-01-01T00:00:00')::timestamp + INTERVAL '1 year') AT TIME ZONE 'UTC' \
           ELSE NULL \
         END)"
    )
}

fn timing_start_sql(expr: &str) -> String {
    let event_start = primitive_start_sql("(timing_event #>> '{}')");
    let first_event = format!(
        "(SELECT min({event_start}) FROM jsonb_array_elements(\
         CASE WHEN jsonb_typeof({expr}->'event') = 'array' THEN {expr}->'event' ELSE '[]'::jsonb END\
         ) AS timing_event)"
    );
    let bounds = format!("{expr}->'repeat'->'boundsPeriod'");
    let bounds_start = primitive_start_sql(&format!("({bounds}->>'start')"));
    format!(
        "(CASE \
           WHEN jsonb_typeof({bounds}) = 'object' AND {bounds}->>'start' IS NULL \
             THEN '-infinity'::timestamptz \
           ELSE LEAST({first_event}, {bounds_start}) \
         END)"
    )
}

fn timing_end_sql(expr: &str) -> String {
    let event_end = primitive_end_sql("(timing_event #>> '{}')");
    let last_event = format!(
        "(SELECT max({event_end}) FROM jsonb_array_elements(\
         CASE WHEN jsonb_typeof({expr}->'event') = 'array' THEN {expr}->'event' ELSE '[]'::jsonb END\
         ) AS timing_event)"
    );
    let bounds = format!("{expr}->'repeat'->'boundsPeriod'");
    let bounds_end = primitive_end_sql(&format!("({bounds}->>'end')"));
    format!(
        "(CASE \
           WHEN jsonb_typeof({bounds}) = 'object' AND {bounds}->>'end' IS NULL \
             THEN 'infinity'::timestamptz \
           ELSE GREATEST({last_event}, {bounds_end}) \
         END)"
    )
}

/// Return SQL for the inclusive lower bound of a primitive date, Period, or Timing.
fn resource_start_sql(expr: &str) -> String {
    let primitive = primitive_start_sql(&format!("({expr} #>> '{{}}')"));
    let period_start = primitive_start_sql(&format!("({expr}->>'start')"));
    let timing_start = timing_start_sql(expr);
    format!(
        "(CASE \
           WHEN {expr} IS NULL OR jsonb_typeof({expr}) = 'null' THEN NULL \
           WHEN jsonb_typeof({expr}) = 'string' THEN {primitive} \
           WHEN jsonb_typeof({expr}) = 'object' AND ({expr}->>'start' IS NOT NULL OR {expr}->>'end' IS NOT NULL) \
             THEN CASE WHEN {expr}->>'start' IS NULL THEN '-infinity'::timestamptz ELSE {period_start} END \
           WHEN jsonb_typeof({expr}) = 'object' \
             AND (jsonb_typeof({expr}->'event') = 'array' OR jsonb_typeof({expr}->'repeat'->'boundsPeriod') = 'object') \
             THEN {timing_start} \
           ELSE NULL \
         END)"
    )
}

/// Return SQL for the exclusive upper bound of a primitive date, Period, or Timing.
fn resource_end_sql(expr: &str) -> String {
    let primitive = primitive_end_sql(&format!("({expr} #>> '{{}}')"));
    let period_end = primitive_end_sql(&format!("({expr}->>'end')"));
    let timing_end = timing_end_sql(expr);
    format!(
        "(CASE \
           WHEN {expr} IS NULL OR jsonb_typeof({expr}) = 'null' THEN NULL \
           WHEN jsonb_typeof({expr}) = 'string' THEN {primitive} \
           WHEN jsonb_typeof({expr}) = 'object' AND ({expr}->>'start' IS NOT NULL OR {expr}->>'end' IS NOT NULL) \
             THEN CASE WHEN {expr}->>'end' IS NULL THEN 'infinity'::timestamptz ELSE {period_end} END \
           WHEN jsonb_typeof({expr}) = 'object' \
             AND (jsonb_typeof({expr}->'event') = 'array' OR jsonb_typeof({expr}->'repeat'->'boundsPeriod') = 'object') \
             THEN {timing_end} \
           ELSE NULL \
         END)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    // ---------------------------------------------------------------------
    // Section 1: prefix parsing
    // ---------------------------------------------------------------------

    #[test]
    fn prefix_parses_known_codes() {
        for (code, expected) in [
            ("eq", DatePrefix::Eq),
            ("ne", DatePrefix::Ne),
            ("gt", DatePrefix::Gt),
            ("ge", DatePrefix::Ge),
            ("lt", DatePrefix::Lt),
            ("le", DatePrefix::Le),
            ("sa", DatePrefix::Sa),
            ("eb", DatePrefix::Eb),
            ("ap", DatePrefix::Ap),
        ] {
            assert_eq!(DatePrefix::parse(code), Some(expected));
            assert_eq!(expected.as_str(), code);
        }
    }

    #[test]
    fn prefix_rejects_unknown_two_letter_codes() {
        assert_eq!(DatePrefix::parse("xx"), None);
        assert_eq!(DatePrefix::parse("zz"), None);
        assert_eq!(DatePrefix::parse(""), None);
        assert_eq!(DatePrefix::parse("e"), None);
    }

    #[test]
    fn prefix_is_case_sensitive() {
        for bad in ["EQ", "Eq", "GT", "Gt", "Ap", "AP"] {
            assert_eq!(DatePrefix::parse(bad), None, "rejected '{bad}'");
        }
    }

    // ---------------------------------------------------------------------
    // Section 2: precision expansion (year / year-month / date / dateTime)
    // ---------------------------------------------------------------------

    #[test]
    fn year_precision_expands_to_full_year_period() {
        let (prefix, bounds) = parse_fhir_date_value("1974").unwrap();
        assert_eq!(prefix, DatePrefix::Eq);
        assert_eq!(bounds.start, dt("1974-01-01T00:00:00Z"));
        assert_eq!(bounds.end, dt("1975-01-01T00:00:00Z"));
        assert_eq!(bounds.duration_ms(), 365 * 24 * 3600 * 1000);
    }

    #[test]
    fn leap_year_period_is_366_days() {
        let (_, bounds) = parse_fhir_date_value("2000").unwrap();
        assert_eq!(bounds.duration_ms(), 366 * 24 * 3600 * 1000);
    }

    #[test]
    fn year_month_precision_handles_december_rollover() {
        let (_, bounds) = parse_fhir_date_value("1974-12").unwrap();
        assert_eq!(bounds.start, dt("1974-12-01T00:00:00Z"));
        assert_eq!(bounds.end, dt("1975-01-01T00:00:00Z"));
    }

    #[test]
    fn year_month_precision_january_is_one_month_duration() {
        let (_, bounds) = parse_fhir_date_value("2024-01").unwrap();
        assert_eq!(bounds.start, dt("2024-01-01T00:00:00Z"));
        assert_eq!(bounds.end, dt("2024-02-01T00:00:00Z"));
        assert_eq!(bounds.duration_ms(), 31 * 24 * 3600 * 1000);
    }

    #[test]
    fn year_month_precision_february_leap_year_is_29_days() {
        let (_, bounds) = parse_fhir_date_value("2024-02").unwrap();
        assert_eq!(bounds.duration_ms(), 29 * 24 * 3600 * 1000);
    }

    #[test]
    fn year_month_precision_february_non_leap_year_is_28_days() {
        let (_, bounds) = parse_fhir_date_value("2023-02").unwrap();
        assert_eq!(bounds.duration_ms(), 28 * 24 * 3600 * 1000);
    }

    #[test]
    fn date_precision_uses_one_day_period() {
        let (_, bounds) = parse_fhir_date_value("1974-12-25").unwrap();
        assert_eq!(bounds.start, dt("1974-12-25T00:00:00Z"));
        assert_eq!(bounds.end, dt("1974-12-26T00:00:00Z"));
        assert_eq!(bounds.duration_ms(), 24 * 3600 * 1000);
    }

    #[test]
    fn date_precision_handles_year_boundary() {
        let (_, bounds) = parse_fhir_date_value("1999-12-31").unwrap();
        assert_eq!(bounds.start, dt("1999-12-31T00:00:00Z"));
        assert_eq!(bounds.end, dt("2000-01-01T00:00:00Z"));
    }

    #[test]
    fn datetime_precision_uses_one_second_period() {
        let (_, bounds) = parse_fhir_date_value("1974-12-25T10:30:00Z").unwrap();
        assert_eq!(bounds.start, dt("1974-12-25T10:30:00Z"));
        assert_eq!(bounds.end, dt("1974-12-25T10:30:01Z"));
        assert_eq!(bounds.duration_ms(), 1000);
    }

    // ---------------------------------------------------------------------
    // Section 3: timezones
    // ---------------------------------------------------------------------

    #[test]
    fn datetime_with_positive_offset_normalises_to_utc() {
        let (_, bounds) = parse_fhir_date_value("1974-12-25T10:30:00+05:00").unwrap();
        assert_eq!(bounds.start, dt("1974-12-25T05:30:00Z"));
        assert_eq!(bounds.end, dt("1974-12-25T05:30:01Z"));
    }

    #[test]
    fn datetime_with_negative_offset_normalises_to_utc() {
        let (_, bounds) = parse_fhir_date_value("1974-12-25T10:30:00-08:00").unwrap();
        assert_eq!(bounds.start, dt("1974-12-25T18:30:00Z"));
        assert_eq!(bounds.end, dt("1974-12-25T18:30:01Z"));
    }

    #[test]
    fn datetime_without_timezone_assumed_utc() {
        let (_, bounds) = parse_fhir_date_value("1974-12-25T10:30:00").unwrap();
        assert_eq!(bounds.start, dt("1974-12-25T10:30:00Z"));
    }

    #[test]
    fn datetime_with_lowercase_zulu_accepted() {
        let (_, bounds) = parse_fhir_date_value("1974-12-25t10:30:00z").unwrap();
        assert_eq!(bounds.start, dt("1974-12-25T10:30:00Z"));
    }

    #[test]
    fn datetime_with_fractional_seconds_truncated_to_second() {
        let (_, bounds) = parse_fhir_date_value("1974-12-25T10:30:00.500Z").unwrap();
        assert_eq!(bounds.start, dt("1974-12-25T10:30:00Z"));
        assert_eq!(bounds.duration_ms(), 1000);
    }

    #[test]
    fn datetime_hour_only_rejected() {
        assert!(parse_fhir_date_value("2024-01-01T10Z").is_err());
    }

    #[test]
    fn datetime_hour_minute_only_with_zulu_accepted() {
        let (_, bounds) = parse_fhir_date_value("2024-01-01T10:30Z").unwrap();
        assert_eq!(bounds.start, dt("2024-01-01T10:30:00Z"));
        assert_eq!(bounds.duration_ms(), 60_000);
    }

    #[test]
    fn maximum_valid_timezone_offset_accepted() {
        let (_, bounds) = parse_fhir_date_value("2024-01-01T00:00:00+14:00").unwrap();
        assert_eq!(bounds.start, dt("2023-12-31T10:00:00Z"));
    }

    #[test]
    fn maximum_negative_timezone_offset_accepted() {
        let (_, bounds) = parse_fhir_date_value("2024-01-01T00:00:00-14:00").unwrap();
        assert_eq!(bounds.start, dt("2024-01-01T14:00:00Z"));
    }

    #[test]
    fn offset_with_missing_minutes_rejected() {
        assert!(parse_fhir_date_value("2024-01-01T00:00:00+05").is_err());
        assert!(parse_fhir_date_value("2024-01-01T00:00:00-05").is_err());
        assert!(parse_fhir_date_value("2024-01-01T00:00:00+14:01").is_err());
    }

    #[test]
    fn malformed_fractional_seconds_rejected() {
        assert!(parse_fhir_date_value("2024-01-01T00:00:00.Z").is_err());
        assert!(parse_fhir_date_value("2024-01-01T00:00:00.xyzZ").is_err());
        assert!(parse_fhir_date_value("2024-01-01T00:00:00:10Z").is_err());
        assert!(parse_fhir_date_value("2024-01-01T00:00:Z").is_err());
        assert!(parse_fhir_date_value("2024-01-01T00:00:.5Z").is_err());
    }

    // ---------------------------------------------------------------------
    // Section 4: prefix stripping
    // ---------------------------------------------------------------------

    #[test]
    fn prefix_is_stripped_before_parsing_for_each_comparator() {
        for code in ["eq", "ne", "gt", "ge", "lt", "le", "sa", "eb", "ap"] {
            let expected_prefix = DatePrefix::parse(code).unwrap();
            let (parsed_prefix, bounds) =
                parse_fhir_date_value(&format!("{code}2020-03-01")).unwrap();
            assert_eq!(parsed_prefix, expected_prefix);
            assert_eq!(bounds.start, dt("2020-03-01T00:00:00Z"));
            assert_eq!(bounds.end, dt("2020-03-02T00:00:00Z"));
        }
    }

    // ---------------------------------------------------------------------
    // Section 5: rejection of invalid input
    // ---------------------------------------------------------------------

    #[test]
    fn empty_input_rejected() {
        assert!(parse_fhir_date_value("").is_err());
        assert!(parse_fhir_date("").is_err());
    }

    #[test]
    fn invalid_month_rejected() {
        assert!(parse_fhir_date_value("2024-13").is_err());
        assert!(parse_fhir_date_value("2024-00").is_err());
    }

    #[test]
    fn invalid_day_rejected() {
        assert!(parse_fhir_date_value("2024-02-31").is_err());
        assert!(parse_fhir_date_value("2024-00-15").is_err());
        assert!(parse_fhir_date_value("2024-01-00").is_err());
        assert!(parse_fhir_date_value("2024-01-32").is_err());
    }

    #[test]
    fn invalid_day_for_non_leap_february_rejected() {
        assert!(parse_fhir_date_value("2023-02-29").is_err());
        assert!(parse_fhir_date_value("2024-02-29").is_ok());
    }

    #[test]
    fn invalid_timezone_offset_rejected() {
        assert!(parse_fhir_date_value("2024-01-01T00:00:00+99:00").is_err());
        assert!(parse_fhir_date_value("2024-01-01T00:00:00+5:00").is_err());
        assert!(parse_fhir_date_value("2024-01-01T00:00:00+05:60").is_err());
        assert!(parse_fhir_date_value("2024-01-01T00:00:00+15:00").is_err());
    }

    #[test]
    fn invalid_hour_minute_second_rejected() {
        assert!(parse_fhir_date_value("2024-01-01T24:00:00Z").is_err());
        assert!(parse_fhir_date_value("2024-01-01T23:60:00Z").is_err());
        assert!(parse_fhir_date_value("2024-01-01T23:59:61Z").is_err());
    }

    #[test]
    fn two_digit_year_rejected() {
        assert!(parse_fhir_date_value("74").is_err());
        assert!(parse_fhir_date_value("99").is_err());
    }

    #[test]
    fn one_digit_year_rejected() {
        assert!(parse_fhir_date_value("1").is_err());
    }

    #[test]
    fn five_digit_year_rejected() {
        // FHIR allows four-digit year only (no expanded-year signaling).
        assert!(parse_fhir_date_value("10000").is_err());
    }

    #[test]
    fn non_numeric_date_segments_rejected() {
        assert!(parse_fhir_date_value("abcd").is_err());
        assert!(parse_fhir_date_value("20a4").is_err());
        assert!(parse_fhir_date_value("2024-ab").is_err());
        assert!(parse_fhir_date_value("2024-01-cd").is_err());
    }

    #[test]
    fn unsupported_two_letter_body_rejected_as_invalid_date() {
        // 'xx' is not a valid comparator → the whole string is treated as a
        // date body, which is then not a valid FHIR date.
        assert!(parse_fhir_date_value("xx2020-01-01").is_err());
    }

    #[test]
    fn truncated_datetime_rejected() {
        assert!(parse_fhir_date_value("2024-01-01T").is_err());
        assert!(parse_fhir_date_value("2024-01-01TZ").is_err());
    }

    #[test]
    fn datetime_missing_time_after_separator_rejected() {
        assert!(parse_fhir_date_value("2024-01-01T:30:00Z").is_err());
        assert!(parse_fhir_date_value("2024-01-01T10::00Z").is_err());
    }

    // ---------------------------------------------------------------------
    // Section 6: ap-prefix widening
    // ---------------------------------------------------------------------

    #[test]
    fn ap_prefix_widens_bounds_by_ten_percent() {
        let year = parse_fhir_date_value("ap2000").unwrap().1;
        let widened = year.widened(0.1);
        let original = year.end - year.start;
        let expanded = widened.end - widened.start;
        assert_eq!(
            expanded.num_seconds(),
            (original.num_seconds() as f64 * 1.2) as i64
        );
    }

    #[test]
    fn ap_prefix_widens_one_day_period_by_roughly_five_hours() {
        let day = parse_fhir_date_value("ap2024-01-01").unwrap().1;
        let widened = day.widened(0.1);
        let original_duration_ms = day.duration_ms();
        let expanded_duration_ms = widened.duration_ms();
        assert_eq!(
            expanded_duration_ms,
            (original_duration_ms as f64 * 1.2) as i64
        );
    }

    // ---------------------------------------------------------------------
    // Section 7: SQL generation helpers
    // ---------------------------------------------------------------------

    #[test]
    fn resource_start_sql_for_year_coerces_to_canonical_instant() {
        let sql = resource_start_sql("resource->'birthDate'");
        assert!(sql.contains("'-01-01T00:00:00Z')::timestamptz"));
        assert!(sql.contains("::timestamptz"));
        assert!(sql.contains("'^\\d{4}$'"));
    }

    #[test]
    fn resource_start_sql_for_year_month_uses_first_day_prefix() {
        let sql = resource_start_sql("resource->'start'");
        assert!(sql.contains("'-01T00:00:00Z')::timestamptz"));
        assert!(sql.contains("'^\\d{4}-\\d{2}$'"));
    }

    #[test]
    fn resource_start_sql_for_date_uses_midnight_prefix() {
        let sql = resource_start_sql("resource->'date'");
        assert!(sql.contains("'T00:00:00Z')::timestamptz"));
        assert!(sql.contains("'^\\d{4}-\\d{2}-\\d{2}$'"));
    }

    #[test]
    fn resource_start_sql_for_datetime_uses_value_directly() {
        let sql = resource_start_sql("resource->'effective'");
        // The regex branch for full datetimes must include a `[Tt]` capture so
        // lowercase 't' is honoured by PostgreSQL case-sensitive matching.
        assert!(sql.contains("[Tt]\\d{2}"));
    }

    #[test]
    fn resource_end_sql_contains_all_precision_intervals() {
        let sql = resource_end_sql("resource->'birthDate'");
        assert!(sql.contains("+ INTERVAL '1 year'"));
        assert!(sql.contains("+ INTERVAL '1 month'"));
        assert!(sql.contains("+ INTERVAL '1 day'"));
        assert!(sql.contains("+ INTERVAL '1 minute'"));
        assert!(sql.contains("+ INTERVAL '1 second'"));
    }

    #[test]
    fn resource_end_sql_for_datetime_adds_one_second() {
        let sql = resource_end_sql("resource->'effectiveDateTime'");
        assert!(sql.contains("+ INTERVAL '1 second'"));
    }

    #[test]
    fn resource_start_and_end_sql_yield_null_on_null_input() {
        let sql = resource_start_sql("resource->'missing'");
        assert!(sql.contains("IS NULL THEN NULL"));
        let sql_end = resource_end_sql("resource->'missing'");
        assert!(sql_end.contains("IS NULL THEN NULL"));
    }

    // ---------------------------------------------------------------------
    // Section 8: push_date_predicate — SQL shapes per comparator
    // ---------------------------------------------------------------------

    fn sql_for(prefix: DatePrefix, value: &str) -> String {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        let (_, bounds) = parse_fhir_date_value(value).unwrap();
        push_date_predicate(&mut query, "resource->'d'", prefix, bounds);
        query.into_sql().as_str().to_owned()
    }

    /// Count Postgres bind placeholders (`$N`) in the generated SQL. We use a
    /// dedicated helper because the SQL contains literal `$` characters in
    /// regex anchors (`\d{4}$`) that would otherwise inflate a naive count.
    fn bind_placeholder_count(sql: &str) -> usize {
        // Match `$` followed by one or more digits.
        let byte_idx = sql.as_bytes();
        let mut count = 0;
        let mut i = 0;
        while i + 1 < byte_idx.len() {
            if byte_idx[i] == b'$' && byte_idx[i + 1].is_ascii_digit() {
                count += 1;
                i += 2;
                while i < byte_idx.len() && byte_idx[i].is_ascii_digit() {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        count
    }

    #[test]
    fn predicate_for_eq_requires_resource_containment() {
        let sql = sql_for(DatePrefix::Eq, "2024");
        assert!(
            sql.contains(" >= "),
            "expected lower containment bound, got {sql}"
        );
        assert!(
            sql.contains(" <= "),
            "expected upper containment bound, got {sql}"
        );
        assert!(sql.contains("::timestamptz"));
        let placeholder_count = bind_placeholder_count(&sql);
        assert!(
            placeholder_count >= 2,
            "expected at least 2 bind placeholders, got {sql}"
        );
    }

    #[test]
    fn predicate_for_ne_negates_resource_containment() {
        let sql = sql_for(DatePrefix::Ne, "2024");
        assert!(sql.contains(" OR "), "expected outside-range OR, got {sql}");
        assert!(sql.contains(" < "));
        assert!(sql.contains(" > "));
    }

    #[test]
    fn predicate_for_gt_compares_resource_end_against_search_end() {
        let sql = sql_for(DatePrefix::Gt, "2024");
        assert!(sql.contains(" > "), "expected > comparison, got {sql}");
        let placeholder_count = bind_placeholder_count(&sql);
        assert_eq!(
            placeholder_count, 1,
            "expected exactly 1 placeholder, got {sql}"
        );
    }

    #[test]
    fn predicate_for_sa_is_stricter_than_gt() {
        let gt = sql_for(DatePrefix::Gt, "2024-01-01");
        let sa = sql_for(DatePrefix::Sa, "2024-01-01");
        assert_ne!(gt, sa, "gt and sa have distinct FHIR range semantics");
        assert!(sa.contains(" >= "));
    }

    #[test]
    fn predicate_for_ge_compares_resource_end_against_search_start() {
        let sql = sql_for(DatePrefix::Ge, "2024");
        assert!(sql.contains(" > "), "expected > comparison, got {sql}");
        let placeholder_count = bind_placeholder_count(&sql);
        assert_eq!(placeholder_count, 1);
    }

    #[test]
    fn predicate_for_lt_compares_resource_start_against_search_start() {
        let sql = sql_for(DatePrefix::Lt, "2024");
        assert!(sql.contains(" < "), "expected < comparison, got {sql}");
        let placeholder_count = bind_placeholder_count(&sql);
        assert_eq!(placeholder_count, 1);
    }

    #[test]
    fn predicate_for_eb_is_stricter_than_lt() {
        let lt = sql_for(DatePrefix::Lt, "2024-01-01");
        let eb = sql_for(DatePrefix::Eb, "2024-01-01");
        assert_ne!(lt, eb, "lt and eb have distinct FHIR range semantics");
        assert!(eb.contains(" <= "));
    }

    #[test]
    fn predicate_for_le_compares_resource_start_against_search_end() {
        let sql = sql_for(DatePrefix::Le, "2024");
        assert!(sql.contains(" < "), "expected < comparison, got {sql}");
        let placeholder_count = bind_placeholder_count(&sql);
        assert_eq!(placeholder_count, 1);
    }

    #[test]
    fn predicate_for_ap_widens_search_period_with_two_bounds() {
        let sql = sql_for(DatePrefix::Ap, "2024");
        assert!(sql.contains(" < "), "expected widened overlap, got {sql}");
        let placeholder_count = bind_placeholder_count(&sql);
        assert_eq!(placeholder_count, 2);
    }

    #[test]
    fn predicate_never_interpolates_user_value_as_string_literal() {
        for value in ["1974", "2024-01-01", "2024-01-01T10:30:00Z", "1974-12"] {
            let sql = sql_for(DatePrefix::Eq, value);
            assert!(
                !sql.contains(&format!("'{value}'")),
                "user value '{value}' leaked into SQL: {sql}"
            );
        }
    }

    #[test]
    fn predicate_binds_chrono_datetime_as_parameters() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 WHERE TRUE");
        let (_, bounds) = parse_fhir_date_value("2024").unwrap();
        push_date_predicate(&mut query, "resource->'d'", DatePrefix::Le, bounds);
        let sql = query.into_sql().as_str().to_owned();
        assert!(sql.contains("$1"), "expected bind placeholder, got {sql}");
        assert!(sql.contains("< $1"));
    }

    // ---------------------------------------------------------------------
    // Section 9: numeric matrix — every (prefix, precision) combination
    // ---------------------------------------------------------------------

    #[test]
    fn all_prefix_precision_combinations_parse_successfully() {
        let prefixes = [
            DatePrefix::Eq,
            DatePrefix::Ne,
            DatePrefix::Gt,
            DatePrefix::Ge,
            DatePrefix::Lt,
            DatePrefix::Le,
            DatePrefix::Sa,
            DatePrefix::Eb,
            DatePrefix::Ap,
        ];
        let bodies = [
            (
                "1974",
                dt("1974-01-01T00:00:00Z"),
                dt("1975-01-01T00:00:00Z"),
            ),
            (
                "1974-12",
                dt("1974-12-01T00:00:00Z"),
                dt("1975-01-01T00:00:00Z"),
            ),
            (
                "1974-12-25",
                dt("1974-12-25T00:00:00Z"),
                dt("1974-12-26T00:00:00Z"),
            ),
            (
                "1974-12-25T10:30:00Z",
                dt("1974-12-25T10:30:00Z"),
                dt("1974-12-25T10:30:01Z"),
            ),
        ];

        for prefix in prefixes {
            for (body, expected_start, expected_end) in bodies {
                let input = format!("{}{body}", prefix.as_str());
                let (parsed_prefix, bounds) = parse_fhir_date_value(&input)
                    .unwrap_or_else(|e| panic!("'{}' should parse: {e}", input));
                assert_eq!(parsed_prefix, prefix, "prefix mismatch for '{input}'");
                assert_eq!(bounds.start, expected_start, "start mismatch for '{input}'");
                assert_eq!(bounds.end, expected_end, "end mismatch for '{input}'");
            }
        }
    }

    // ---------------------------------------------------------------------
    // Section 10: regression class — injection & bad unicode
    // ---------------------------------------------------------------------

    #[test]
    fn sql_generation_is_safe_against_injection_in_user_value() {
        for malicious in [
            "2024-01-01`; DROP TABLE x; --",
            "2024-Wéb",
            "2024\n01-01",
            "2024-01-01T10:30:00Z; DROP TABLE resources;",
        ] {
            assert!(
                parse_fhir_date_value(malicious).is_err(),
                "expected rejection of malicious input {malicious:?}"
            );
        }
    }

    #[test]
    fn year_zero_is_rejected() {
        assert!(parse_fhir_date_value("0000").is_err());
        assert!(parse_fhir_date_value("0000-01-01").is_err());
    }
}
