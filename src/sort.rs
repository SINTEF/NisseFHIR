//! FHIR `_sort` parsing, SQL generation, and opaque keyset cursors.
//!
//! Sorting is deliberately opt-in. The generated search registry describes
//! filtering, whereas this module accepts only the singular scalar parameters
//! approved by `sortable_search_param_codes_for`. This keeps repeated and
//! complex FHIR elements from acquiring accidental ordering semantics.

use std::hash::{Hash, Hasher};

use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::{Postgres, QueryBuilder};

use crate::{
    error::AppError,
    search_params::{
        SearchParam, SearchParamType, search_params_for, sortable_search_param_codes_for,
    },
    store::StoredResource,
};

pub const DEFAULT_SORT: &str = "_id";
pub const MAX_SORT_KEYS: usize = 4;
const CURSOR_VERSION: &str = "v2";
const CURSOR_FIELD_SEP: char = '\u{1f}';
const CURSOR_NULL: &str = "~";

#[derive(Debug, Clone, Copy)]
pub enum SortColumn {
    Id,
    LastUpdated,
    SearchParam(&'static SearchParam),
}

impl SortColumn {
    fn code(self) -> &'static str {
        match self {
            Self::Id => "_id",
            Self::LastUpdated => "_lastUpdated",
            Self::SearchParam(param) => param.code,
        }
    }

    fn is_case_insensitive(self) -> bool {
        matches!(
            self,
            Self::SearchParam(SearchParam {
                param_type: SearchParamType::String,
                ..
            })
        )
    }

    fn push_expression(self, query: &mut QueryBuilder<Postgres>) {
        match self {
            Self::Id => {
                query.push("id");
            }
            Self::LastUpdated => {
                query.push("last_updated");
            }
            Self::SearchParam(param) => {
                let crate::search_params::registry::JsonPath::Field(path) = &param.path else {
                    unreachable!("only singular Field sort definitions are registered")
                };
                debug_assert_eq!(path.len(), 1);
                if self.is_case_insensitive() {
                    query.push("lower(");
                }
                query.push("resource->>").push_bind(path[0]);
                if self.is_case_insensitive() {
                    query.push(")");
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy)]
pub struct SortKey {
    pub column: SortColumn,
    pub direction: SortDirection,
}

pub fn sortable_keys_for(resource_type: &str) -> Vec<&'static str> {
    let mut keys = vec!["_id", "_lastUpdated"];
    keys.extend_from_slice(sortable_search_param_codes_for(resource_type));
    keys
}

pub fn default_sort() -> Vec<SortKey> {
    vec![SortKey {
        column: SortColumn::Id,
        direction: SortDirection::Ascending,
    }]
}

/// Parse a resource-specific `_sort` value.  Only explicitly registered
/// scalar search parameters are accepted in addition to the two storage keys.
pub fn parse_sort_param(resource_type: &str, value: &str) -> Result<Vec<SortKey>, AppError> {
    if value.is_empty() {
        return Err(AppError::BadRequest("_sort must not be empty".to_owned()));
    }
    let allowed = sortable_search_param_codes_for(resource_type);
    let mut keys = Vec::new();
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
        let column = match code {
            "_id" => SortColumn::Id,
            "_lastUpdated" => SortColumn::LastUpdated,
            _ if allowed.contains(&code) => {
                let param = search_params_for(resource_type)
                    .iter()
                    .find(|param| param.code == code)
                    .expect("sortable search parameter must exist in the filtering registry");
                match (&param.param_type, &param.path) {
                    (
                        SearchParamType::String | SearchParamType::Token | SearchParamType::Date,
                        crate::search_params::registry::JsonPath::Field(path),
                    ) if path.len() == 1 => SortColumn::SearchParam(param),
                    _ => unreachable!("only singular scalar sort definitions are registered"),
                }
            }
            _ => {
                return Err(AppError::BadRequest(format!(
                    "unsupported, unknown, or unindexed sort key '{code}'"
                )));
            }
        };
        if keys.iter().any(|key: &SortKey| key.column.code() == code) {
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

pub fn effective_sort(requested: &[SortKey]) -> Vec<SortKey> {
    let mut effective = requested.to_vec();
    if !effective
        .iter()
        .any(|key| matches!(key.column, SortColumn::Id))
    {
        effective.push(SortKey {
            column: SortColumn::Id,
            direction: SortDirection::Ascending,
        });
    }
    effective
}

/// All nullable JSON values sort after present values, in either direction.
pub fn push_order_by(query: &mut QueryBuilder<Postgres>, sort: &[SortKey]) {
    query.push(" ORDER BY ");
    for (i, key) in sort.iter().enumerate() {
        if i > 0 {
            query.push(", ");
        }
        if matches!(key.column, SortColumn::SearchParam(_)) {
            key.column.push_expression(query);
            query.push(" IS NULL ASC, ");
        }
        key.column.push_expression(query);
        query.push(match key.direction {
            SortDirection::Ascending => " ASC",
            SortDirection::Descending => " DESC",
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortCursorValue {
    Id(String),
    LastUpdated(DateTime<Utc>),
    Text(Option<String>),
}

fn push_bind_value(query: &mut QueryBuilder<Postgres>, value: &SortCursorValue) {
    match value {
        SortCursorValue::Id(id) => {
            query.push_bind(id.clone());
        }
        SortCursorValue::LastUpdated(dt) => {
            query.push_bind(*dt);
        }
        SortCursorValue::Text(text) => {
            query.push_bind(text.clone());
        }
    }
}

fn push_equal(query: &mut QueryBuilder<Postgres>, key: SortKey, value: &SortCursorValue) {
    key.column.push_expression(query);
    if matches!(key.column, SortColumn::SearchParam(_)) {
        query.push(" IS NOT DISTINCT FROM ");
    } else {
        query.push(" = ");
    }
    push_bind_value(query, value);
}

fn push_after(query: &mut QueryBuilder<Postgres>, key: SortKey, value: &SortCursorValue) {
    if matches!(key.column, SortColumn::SearchParam(_))
        && matches!(value, SortCursorValue::Text(None))
    {
        query.push("FALSE");
        return;
    }
    query.push("(");
    key.column.push_expression(query);
    query.push(match key.direction {
        SortDirection::Ascending => " > ",
        SortDirection::Descending => " < ",
    });
    push_bind_value(query, value);
    if matches!(key.column, SortColumn::SearchParam(_)) {
        query.push(" OR ");
        key.column.push_expression(query);
        query.push(" IS NULL");
    }
    query.push(")");
}

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
            push_equal(query, *key, value);
        }
        query.push(" AND ");
        push_after(query, sort[i], &values[i]);
        query.push(")");
    }
    query.push(")");
}

pub fn cursor_values_for(sort: &[SortKey], resource: &StoredResource) -> Vec<SortCursorValue> {
    sort.iter()
        .map(|key| match key.column {
            SortColumn::Id => SortCursorValue::Id(resource.id.clone()),
            SortColumn::LastUpdated => SortCursorValue::LastUpdated(resource.last_updated),
            SortColumn::SearchParam(param) => {
                let crate::search_params::registry::JsonPath::Field(path) = &param.path else {
                    unreachable!()
                };
                let value = resource
                    .resource
                    .get(path[0])
                    .and_then(|value| value.as_str())
                    .map(|value| {
                        if key.column.is_case_insensitive() {
                            value.to_lowercase()
                        } else {
                            value.to_owned()
                        }
                    });
                SortCursorValue::Text(value)
            }
        })
        .collect()
}

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

pub fn encode_cursor(
    sort_raw: &str,
    canonical_filters: &[(String, String)],
    values: &[SortCursorValue],
) -> String {
    let mut fields = vec![
        CURSOR_VERSION.to_owned(),
        format!("{:x}", cursor_fingerprint(sort_raw, canonical_filters)),
    ];
    for value in values {
        fields.push(match value {
            SortCursorValue::Id(id) => id.clone(),
            SortCursorValue::LastUpdated(dt) => dt.to_rfc3339_opts(SecondsFormat::Nanos, true),
            SortCursorValue::Text(Some(text)) => text.clone(),
            SortCursorValue::Text(None) => CURSOR_NULL.to_owned(),
        });
    }
    fields.join(&CURSOR_FIELD_SEP.to_string())
}

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
    if u64::from_str_radix(fields[1], 16).map_err(|_| invalid())?
        != cursor_fingerprint(sort_raw, canonical_filters)
    {
        return Err(invalid());
    }
    effective
        .iter()
        .zip(&fields[2..])
        .map(|(key, field)| match key.column {
            SortColumn::Id => {
                crate::fhir::validate_fhir_id(field).map_err(|_| invalid())?;
                Ok(SortCursorValue::Id((*field).to_owned()))
            }
            SortColumn::LastUpdated => DateTime::parse_from_rfc3339(field)
                .map(|dt| SortCursorValue::LastUpdated(dt.with_timezone(&Utc)))
                .map_err(|_| invalid()),
            SortColumn::SearchParam(_) => Ok(SortCursorValue::Text(
                (*field != CURSOR_NULL).then(|| (*field).to_owned()),
            )),
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn render_sort(sort: &[SortKey]) -> String {
    sort.iter()
        .map(|key| {
            format!(
                "{}{}",
                if key.direction == SortDirection::Descending {
                    "-"
                } else {
                    ""
                },
                key.column.code()
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patient_accepts_its_explicit_scalar_sort_keys() {
        assert_eq!(
            sortable_keys_for("Patient"),
            [
                "_id",
                "_lastUpdated",
                "birthdate",
                "death-date",
                "gender",
                "active"
            ]
        );
        assert_eq!(
            render_sort(&parse_sort_param("Patient", "birthdate,-gender").unwrap()),
            "birthdate,-gender"
        );
    }

    #[test]
    fn questionnaire_and_workflow_sort_keys_are_explicit_and_resource_scoped() {
        assert_eq!(
            sortable_keys_for("Questionnaire"),
            ["_id", "_lastUpdated", "date", "name", "status", "title"]
        );
        assert_eq!(
            sortable_keys_for("QuestionnaireResponse"),
            ["_id", "_lastUpdated", "authored", "status"]
        );
        assert_eq!(
            render_sort(&parse_sort_param("QuestionnaireResponse", "-authored,status").unwrap()),
            "-authored,status"
        );
        assert!(parse_sort_param("QuestionnaireResponse", "answer-date").is_err());
        assert!(parse_sort_param("Questionnaire", "authored").is_err());
    }
    #[test]
    fn resource_specific_sort_keys_fail_closed() {
        assert!(parse_sort_param("Organization", "birthdate").is_err());
        assert!(parse_sort_param("Patient", "name").is_err());
    }
    #[test]
    fn effective_sort_appends_id_tiebreak() {
        assert_eq!(
            render_sort(&effective_sort(
                &parse_sort_param("Patient", "birthdate").unwrap()
            )),
            "birthdate,_id"
        );
    }
    #[test]
    fn nullable_sort_cursor_round_trips() {
        let sort = effective_sort(&parse_sort_param("Patient", "birthdate").unwrap());
        let values = vec![
            SortCursorValue::Text(None),
            SortCursorValue::Id("patient-1".to_owned()),
        ];
        let encoded = encode_cursor("birthdate", &[], &values);
        assert_eq!(
            decode_cursor(&encoded, &sort, "birthdate", &[]).unwrap(),
            values
        );
    }
    #[test]
    fn keyset_places_null_after_present_values() {
        let sort = effective_sort(&parse_sort_param("Patient", "birthdate").unwrap());
        let values = vec![
            SortCursorValue::Text(Some("2000-01-01".to_owned())),
            SortCursorValue::Id("patient-1".to_owned()),
        ];
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_keyset_predicate(&mut query, &sort, &values);
        assert!(
            query
                .into_sql()
                .as_str()
                .contains("resource->>$1 > $2 OR resource->>$3 IS NULL")
        );
    }
}
