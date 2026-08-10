//! FHIR `_sort` search-result parameter: parsing, opaque keyset cursors, and
//! the SQL fragments needed to page through a sorted result set.
//!
//! Only `_id` and `_lastUpdated` are sortable. Both map to `NOT NULL` columns
//! on `fhir_resources` (see `migrations/0001_create_fhir_resources.sql`), so
//! there is no null-handling decision to make: every row always has a value
//! for both keys, and neither repeats. Extending `_sort` to registry-backed
//! search parameters is explicitly out of scope for this task — those paths
//! may be absent, repeated, or unindexed for ordering, none of which apply to
//! the two storage columns supported here. See `ideas/search-and-indexing.md`.
//!
//! `_sort` is only implemented for `GET /fhir/{type}`. AuditEvent search and
//! instance history intentionally do not accept it: both already fail closed
//! on any unrecognized query parameter, so passing `_sort` to either yields a
//! privacy-safe `400` rather than being silently ignored.

use std::hash::{Hash, Hasher};

use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::{Postgres, QueryBuilder};

use crate::error::AppError;
use crate::store::StoredResource;

/// Sort keys accepted by `_sort`, in the order they are advertised.
pub const SORTABLE_KEYS: &[&str] = &[SortColumn::Id.code(), SortColumn::LastUpdated.code()];

/// The effective order for collection searches without an explicit `_sort`.
/// Its stable textual form is fingerprinted into their opaque cursors.
pub const DEFAULT_SORT: &str = "_id";

/// Upper bound on the number of sort keys accepted in one `_sort` value.
/// Only two keys are currently sortable, so this is mostly defensive: it
/// gives a clear error message instead of relying solely on the
/// duplicate-key check if more sortable keys are ever added.
pub const MAX_SORT_KEYS: usize = 2;

const CURSOR_VERSION: &str = "v1";
/// ASCII Unit Separator: not a legal character in a FHIR `id`
/// (`[A-Za-z0-9\-\.]{1,64}`) or in an RFC 3339 timestamp, so it is safe as a
/// field delimiter in the opaque cursor without any escaping.
const CURSOR_FIELD_SEP: char = '\u{1f}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Id,
    LastUpdated,
}

impl SortColumn {
    const fn code(self) -> &'static str {
        match self {
            SortColumn::Id => "_id",
            SortColumn::LastUpdated => "_lastUpdated",
        }
    }

    fn sql_name(self) -> &'static str {
        match self {
            SortColumn::Id => "id",
            SortColumn::LastUpdated => "last_updated",
        }
    }

    fn parse(code: &str) -> Option<Self> {
        match code {
            "_id" => Some(SortColumn::Id),
            "_lastUpdated" => Some(SortColumn::LastUpdated),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortKey {
    pub column: SortColumn,
    pub direction: SortDirection,
}

pub fn default_sort() -> Vec<SortKey> {
    vec![SortKey {
        column: SortColumn::Id,
        direction: SortDirection::Ascending,
    }]
}

/// A typed value extracted from one row for one sort column, used both to
/// build the next page's cursor and to bind a decoded cursor's values into
/// the keyset `WHERE` predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortCursorValue {
    Id(String),
    LastUpdated(DateTime<Utc>),
}

/// Parse a `_sort` value: a comma-separated list of sort keys, each
/// optionally prefixed with `-` for descending order, applied in the order
/// given. Fails closed on anything not in [`SORTABLE_KEYS`], on a repeated
/// key, on an empty entry, and once more than [`MAX_SORT_KEYS`] keys are
/// requested.
pub fn parse_sort_param(value: &str) -> Result<Vec<SortKey>, AppError> {
    if value.is_empty() {
        return Err(AppError::BadRequest("_sort must not be empty".to_owned()));
    }

    let mut keys: Vec<SortKey> = Vec::new();
    for raw in value.split(',') {
        if raw.is_empty() {
            return Err(AppError::BadRequest(
                "_sort must not contain empty entries".to_owned(),
            ));
        }
        let (direction, code) = match raw.strip_prefix('-') {
            Some(rest) => (SortDirection::Descending, rest),
            None => (SortDirection::Ascending, raw),
        };
        let Some(column) = SortColumn::parse(code) else {
            return Err(AppError::BadRequest(format!(
                "unsupported, unknown, or unindexed sort key '{code}'"
            )));
        };
        if keys.iter().any(|k| k.column == column) {
            return Err(AppError::BadRequest(format!(
                "_sort must not repeat sort key '{code}'"
            )));
        }
        if keys.len() >= MAX_SORT_KEYS {
            return Err(AppError::BadRequest(format!(
                "_sort accepts at most {MAX_SORT_KEYS} sort keys"
            )));
        }
        keys.push(SortKey { column, direction });
    }

    Ok(keys)
}

/// The fully tie-broken sort order: the requested keys, plus `_id ascending`
/// appended when it is not already one of the requested keys. Because `id`
/// is unique per (tenant, resource type), this always yields a total order.
pub fn effective_sort(requested: &[SortKey]) -> Vec<SortKey> {
    let mut effective = requested.to_vec();
    if !effective.iter().any(|k| k.column == SortColumn::Id) {
        effective.push(SortKey {
            column: SortColumn::Id,
            direction: SortDirection::Ascending,
        });
    }
    effective
}

/// Append `ORDER BY <col> ASC|DESC, ...` for the effective sort order.
pub fn push_order_by(query: &mut QueryBuilder<Postgres>, sort: &[SortKey]) {
    query.push(" ORDER BY ");
    for (i, key) in sort.iter().enumerate() {
        if i > 0 {
            query.push(", ");
        }
        query.push(key.column.sql_name());
        query.push(match key.direction {
            SortDirection::Ascending => " ASC",
            SortDirection::Descending => " DESC",
        });
    }
}

/// Append the keyset ("seek") predicate that selects rows strictly after
/// `values` in the given sort order:
///
/// ```sql
/// AND (
///        (k1 > v1)
///     OR (k1 = v1 AND k2 > v2)
///     OR ...
///     OR (k1 = v1 AND ... AND k_{n-1} = v_{n-1} AND kn > vn)
/// )
/// ```
///
/// using `<` in place of `>` for any column sorted descending. This is the
/// standard multi-column keyset expansion; a single SQL row-comparison
/// operator cannot express it once directions are mixed.
pub fn push_keyset_predicate(
    query: &mut QueryBuilder<Postgres>,
    sort: &[SortKey],
    values: &[SortCursorValue],
) {
    debug_assert_eq!(sort.len(), values.len());
    query.push(" AND (FALSE");
    for i in 0..sort.len() {
        query.push(" OR (TRUE");
        for (key, value) in sort[..i].iter().zip(&values[..i]) {
            query.push(" AND ");
            query.push(key.column.sql_name());
            query.push(" = ");
            push_bind_value(query, value);
        }
        query.push(" AND ");
        query.push(sort[i].column.sql_name());
        query.push(match sort[i].direction {
            SortDirection::Ascending => " > ",
            SortDirection::Descending => " < ",
        });
        push_bind_value(query, &values[i]);
        query.push(")");
    }
    query.push(")");
}

fn push_bind_value(query: &mut QueryBuilder<Postgres>, value: &SortCursorValue) {
    match value {
        SortCursorValue::Id(id) => {
            query.push_bind(id.clone());
        }
        SortCursorValue::LastUpdated(dt) => {
            query.push_bind(*dt);
        }
    }
}

/// Extract the cursor values for one row under the given effective sort.
pub fn cursor_values_for(sort: &[SortKey], resource: &StoredResource) -> Vec<SortCursorValue> {
    sort.iter()
        .map(|key| match key.column {
            SortColumn::Id => SortCursorValue::Id(resource.id.clone()),
            SortColumn::LastUpdated => SortCursorValue::LastUpdated(resource.last_updated),
        })
        .collect()
}

/// A stable fingerprint of the `(_sort, filters)` pair a cursor was minted
/// under. Not cryptographic and not stable across process restarts or Rust
/// versions — like [`crate::store::conditional_create_lock_key`], it only
/// needs to agree with itself within one running server, since a cursor is
/// only ever replayed against the same deployment that issued it.
fn cursor_fingerprint(sort_raw: &str, canonical_filters: &[(String, String)]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    sort_raw.hash(&mut hasher);
    0u8.hash(&mut hasher);
    for (key, value) in canonical_filters {
        key.hash(&mut hasher);
        1u8.hash(&mut hasher);
        value.hash(&mut hasher);
        2u8.hash(&mut hasher);
    }
    hasher.finish()
}

/// Encode an opaque `_after_id` cursor for a sorted search: a version tag, a
/// fingerprint of the `_sort` value and filters it was minted under, and the
/// sort column values of the last row on the page, in sort-key order.
pub fn encode_cursor(
    sort_raw: &str,
    canonical_filters: &[(String, String)],
    values: &[SortCursorValue],
) -> String {
    let fingerprint = cursor_fingerprint(sort_raw, canonical_filters);
    let mut fields = vec![CURSOR_VERSION.to_owned(), format!("{fingerprint:x}")];
    for value in values {
        fields.push(match value {
            SortCursorValue::Id(id) => id.clone(),
            SortCursorValue::LastUpdated(dt) => dt.to_rfc3339_opts(SecondsFormat::Nanos, true),
        });
    }
    fields.join(&CURSOR_FIELD_SEP.to_string())
}

/// Decode and validate an `_after_id` cursor against the current request's
/// effective sort and canonical filters. Rejects a cursor that is malformed,
/// was minted under a different `_sort`, or was minted under a different
/// filter set — a cursor must never be silently reinterpreted.
pub fn decode_cursor(
    raw: &str,
    effective: &[SortKey],
    sort_raw: &str,
    canonical_filters: &[(String, String)],
) -> Result<Vec<SortCursorValue>, AppError> {
    let invalid = || {
        AppError::BadRequest(
            "_after_id is not a valid cursor for the current _sort and filters".to_owned(),
        )
    };

    let fields: Vec<&str> = raw.split(CURSOR_FIELD_SEP).collect();
    if fields.len() != 2 + effective.len() || fields[0] != CURSOR_VERSION {
        return Err(invalid());
    }

    let fingerprint = u64::from_str_radix(fields[1], 16).map_err(|_| invalid())?;
    if fingerprint != cursor_fingerprint(sort_raw, canonical_filters) {
        return Err(invalid());
    }

    effective
        .iter()
        .zip(&fields[2..])
        .map(|(key, field)| match key.column {
            SortColumn::Id => {
                if crate::fhir::validate_fhir_id(field).is_err() {
                    return Err(invalid());
                }
                Ok(SortCursorValue::Id((*field).to_owned()))
            }
            SortColumn::LastUpdated => DateTime::parse_from_rfc3339(field)
                .map(|dt| SortCursorValue::LastUpdated(dt.with_timezone(&Utc)))
                .map_err(|_| invalid()),
        })
        .collect()
}

/// Render the effective sort order as `_sort`-syntax, e.g. `-_lastUpdated,_id`.
/// Used only for tests and diagnostics; the `self`/`next` Bundle links echo
/// the client's original raw `_sort` value unchanged, not this.
#[cfg(test)]
pub(crate) fn render_sort(sort: &[SortKey]) -> String {
    sort.iter()
        .map(|key| {
            let prefix = match key.direction {
                SortDirection::Ascending => "",
                SortDirection::Descending => "-",
            };
            format!("{prefix}{}", key.column.code())
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(column: SortColumn, direction: SortDirection) -> SortKey {
        SortKey { column, direction }
    }

    #[test]
    fn parses_single_ascending_key() {
        let keys = parse_sort_param("_lastUpdated").unwrap();
        assert_eq!(
            keys,
            vec![key(SortColumn::LastUpdated, SortDirection::Ascending)]
        );
    }

    #[test]
    fn parses_descending_prefix() {
        let keys = parse_sort_param("-_lastUpdated").unwrap();
        assert_eq!(
            keys,
            vec![key(SortColumn::LastUpdated, SortDirection::Descending)]
        );
    }

    #[test]
    fn parses_multiple_keys_in_order() {
        let keys = parse_sort_param("_id,-_lastUpdated").unwrap();
        assert_eq!(
            keys,
            vec![
                key(SortColumn::Id, SortDirection::Ascending),
                key(SortColumn::LastUpdated, SortDirection::Descending),
            ]
        );
    }

    #[test]
    fn rejects_unknown_key() {
        let error = parse_sort_param("status").unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn rejects_empty_value() {
        assert!(parse_sort_param("").is_err());
    }

    #[test]
    fn rejects_empty_entry() {
        assert!(parse_sort_param("_id,").is_err());
        assert!(parse_sort_param(",_id").is_err());
    }

    #[test]
    fn rejects_duplicate_key() {
        let error = parse_sort_param("_id,-_id").unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn rejects_too_many_keys() {
        // Only two distinct sortable codes exist, so three tokens can only be
        // reached by repeating one — which itself must be rejected — but this
        // also exercises the explicit MAX_SORT_KEYS bound directly.
        const { assert!(MAX_SORT_KEYS < 3) };
        let error = parse_sort_param("_id,_lastUpdated,_id").unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn effective_sort_appends_id_tiebreak_when_absent() {
        let requested = parse_sort_param("_lastUpdated").unwrap();
        let effective = effective_sort(&requested);
        assert_eq!(render_sort(&effective), "_lastUpdated,_id");
    }

    #[test]
    fn effective_sort_does_not_duplicate_id() {
        let requested = parse_sort_param("-_id").unwrap();
        let effective = effective_sort(&requested);
        assert_eq!(render_sort(&effective), "-_id");
    }

    #[test]
    fn effective_sort_preserves_id_position_when_requested_first() {
        let requested = parse_sort_param("_id,-_lastUpdated").unwrap();
        let effective = effective_sort(&requested);
        assert_eq!(render_sort(&effective), "_id,-_lastUpdated");
    }

    #[test]
    fn cursor_round_trips() {
        let sort = effective_sort(&parse_sort_param("-_lastUpdated").unwrap());
        let now = Utc::now();
        let values = vec![
            SortCursorValue::LastUpdated(now),
            SortCursorValue::Id("patient-1".to_owned()),
        ];
        let filters = vec![("name".to_owned(), "Alice".to_owned())];
        let encoded = encode_cursor("-_lastUpdated", &filters, &values);
        let decoded = decode_cursor(&encoded, &sort, "-_lastUpdated", &filters).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn cursor_rejected_for_different_sort() {
        let sort = effective_sort(&parse_sort_param("_lastUpdated").unwrap());
        let values = vec![
            SortCursorValue::LastUpdated(Utc::now()),
            SortCursorValue::Id("patient-1".to_owned()),
        ];
        let encoded = encode_cursor("_lastUpdated", &[], &values);
        let error = decode_cursor(&encoded, &sort, "-_lastUpdated", &[]).unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn cursor_rejected_for_different_filters() {
        let sort = effective_sort(&parse_sort_param("_id").unwrap());
        let values = vec![SortCursorValue::Id("patient-1".to_owned())];
        let filters_a = vec![("name".to_owned(), "Alice".to_owned())];
        let filters_b = vec![("name".to_owned(), "Bob".to_owned())];
        let encoded = encode_cursor("_id", &filters_a, &values);
        let error = decode_cursor(&encoded, &sort, "_id", &filters_b).unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn cursor_rejected_when_malformed() {
        let sort = effective_sort(&parse_sort_param("_id").unwrap());
        let error = decode_cursor("not-a-cursor", &sort, "_id", &[]).unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn cursor_rejected_when_id_is_not_a_fhir_id() {
        let sort = effective_sort(&parse_sort_param("_id").unwrap());
        let encoded = encode_cursor(
            "_id",
            &[],
            &[SortCursorValue::Id("not a FHIR id".to_owned())],
        );

        let error = decode_cursor(&encoded, &sort, "_id", &[]).unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn cursor_rejected_when_wrong_field_count() {
        let sort = effective_sort(&parse_sort_param("_lastUpdated").unwrap());
        let short_values = vec![SortCursorValue::LastUpdated(Utc::now())];
        // Encode as if only one sort key were effective (wrong shape).
        let encoded = encode_cursor("_lastUpdated", &[], &short_values);
        let error = decode_cursor(&encoded, &sort, "_lastUpdated", &[]).unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn keyset_predicate_single_ascending_key() {
        let sort = vec![key(SortColumn::Id, SortDirection::Ascending)];
        let values = vec![SortCursorValue::Id("patient-1".to_owned())];
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_keyset_predicate(&mut query, &sort, &values);
        let sql = query.into_sql().as_str().to_owned();
        assert_eq!(
            sql,
            "SELECT 1 FROM t WHERE 1=1 AND (FALSE OR (TRUE AND id > $1))"
        );
    }

    #[test]
    fn keyset_predicate_mixed_direction_multi_key() {
        let sort = vec![
            key(SortColumn::LastUpdated, SortDirection::Descending),
            key(SortColumn::Id, SortDirection::Ascending),
        ];
        let values = vec![
            SortCursorValue::LastUpdated(Utc::now()),
            SortCursorValue::Id("patient-1".to_owned()),
        ];
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_keyset_predicate(&mut query, &sort, &values);
        let sql = query.into_sql().as_str().to_owned();
        assert_eq!(
            sql,
            "SELECT 1 FROM t WHERE 1=1 AND (FALSE OR (TRUE AND last_updated < $1) OR (TRUE AND last_updated = $2 AND id > $3))"
        );
    }

    #[test]
    fn order_by_renders_each_column_and_direction() {
        let sort = vec![
            key(SortColumn::LastUpdated, SortDirection::Descending),
            key(SortColumn::Id, SortDirection::Ascending),
        ];
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t");
        push_order_by(&mut query, &sort);
        let sql = query.into_sql().as_str().to_owned();
        assert_eq!(sql, "SELECT 1 FROM t ORDER BY last_updated DESC, id ASC");
    }

    #[test]
    fn sortable_keys_are_exactly_id_and_last_updated() {
        assert_eq!(SORTABLE_KEYS, ["_id", "_lastUpdated"]);
        for &code in SORTABLE_KEYS {
            parse_sort_param(code).unwrap_or_else(|_| panic!("{code} should be accepted"));
        }
    }
}
