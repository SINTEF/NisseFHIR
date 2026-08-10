//! SQL query generation for FHIR search parameters.
//!
//! Translates search parameter definitions from the registry into PostgreSQL
//! JSONB query fragments that can be appended to a `WHERE` clause.

use sqlx::{Postgres, QueryBuilder};

use crate::error::AppError;

use super::registry::{JsonPath, SearchParam, SearchParamType};

/// A parsed search filter ready to be appended to a SQL query.
#[derive(Debug, Clone)]
pub struct SearchFilter {
    /// The search parameter definition this filter applies.
    pub param: &'static SearchParam,
    /// Optional FHIR modifier accepted for this parameter occurrence.
    pub modifier: Option<SearchModifier>,
    /// Values in one parameter occurrence. Multiple values are FHIR OR terms.
    pub values: Vec<String>,
}

/// String search modifiers implemented by the generic search engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchModifier {
    Exact,
    Contains,
}

/// How geospatial `near` filtering is computed.
///
/// Detected once at startup against the actual database so the server never
/// depends on the `earthdistance` extension being installable: databases whose
/// operator cannot install it (e.g. managed offerings with a restricted
/// extension allow-list) transparently use the haversine fallback instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeoSearchMode {
    /// `earthdistance` extension is installed: indexed GiST proximity filter.
    EarthDistance,
    /// No extension available: pure-SQL haversine filter that needs nothing.
    Haversine,
}

/// Detect which geospatial mode the connected database supports.
///
/// Returns [`GeoSearchMode::EarthDistance`] when the `earthdistance` extension
/// is present and [`GeoSearchMode::Haversine`] otherwise, so `near` search
/// keeps working — and is always advertised — on every database.
pub async fn detect_geo_search_mode<'e, E>(executor: E) -> Result<GeoSearchMode, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let has_earthdistance: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_extension WHERE extname = 'earthdistance')",
    )
    .fetch_one(executor)
    .await?;
    Ok(if has_earthdistance {
        GeoSearchMode::EarthDistance
    } else {
        GeoSearchMode::Haversine
    })
}

/// Append SQL `WHERE` clause fragments for the given filters.
///
/// Each filter produces one or more `AND …` conditions that narrow the result
/// set. The function handles all supported search parameter types (string,
/// token, reference, date, uri, number, quantity) and JSON path variants.
pub fn push_search_filters(
    query: &mut QueryBuilder<Postgres>,
    filters: &[SearchFilter],
    geo_mode: GeoSearchMode,
) -> Result<(), AppError> {
    for filter in filters {
        validate_search_filter(filter.param, &filter.values)?;
        if filter.values.len() == 1 {
            push_search_filter_value(query, filter, &filter.values[0], geo_mode);
        } else {
            // Existing filter emitters append `AND <predicate>`. Seeding each
            // branch with TRUE lets us compose those predicates as an OR group
            // without duplicating their SQL generation and bind handling.
            query.push(" AND (FALSE");
            for value in &filter.values {
                query.push(" OR (TRUE");
                push_search_filter_value(query, filter, value, geo_mode);
                query.push(")");
            }
            query.push(")");
        }
    }
    Ok(())
}

/// Validate that a registry entry has an implemented SQL path and that its
/// values cannot take a no-op branch. This is used by both request parsing and
/// SQL construction so fail-closed behavior does not depend on one caller.
pub fn validate_search_filter(param: &SearchParam, values: &[String]) -> Result<(), AppError> {
    let executable_path = match (&param.param_type, &param.path) {
        (SearchParamType::Token, JsonPath::ResourceId) => true,
        (SearchParamType::String, JsonPath::Field(segments)) => !segments.is_empty(),
        (SearchParamType::String, JsonPath::WhereFilter { .. }) => true,
        (SearchParamType::Token, JsonPath::Field(segments)) => !segments.is_empty(),
        (SearchParamType::Token, JsonPath::WhereFilter { .. }) => true,
        (SearchParamType::Token, JsonPath::Exists(segments)) => !segments.is_empty(),
        (SearchParamType::Token, JsonPath::ExistsAlternatives(paths)) => {
            !paths.is_empty() && paths.iter().all(|segments| !segments.is_empty())
        }
        (SearchParamType::Reference, JsonPath::Field(segments)) => !segments.is_empty(),
        (SearchParamType::Reference, JsonPath::WhereFilter { .. }) => true,
        (SearchParamType::Date, JsonPath::Field(segments)) => !segments.is_empty(),
        (SearchParamType::Date, JsonPath::FieldAlternatives(paths)) => {
            !paths.is_empty() && paths.iter().all(|segments| !segments.is_empty())
        }
        (SearchParamType::Date, JsonPath::WhereFilter { .. }) => true,
        (SearchParamType::Uri, JsonPath::Field(segments)) => !segments.is_empty(),
        (SearchParamType::Number, JsonPath::Field(segments)) => !segments.is_empty(),
        (SearchParamType::Quantity, JsonPath::Field(segments)) => !segments.is_empty(),
        (SearchParamType::Special, JsonPath::Position(segments)) => !segments.is_empty(),
        _ => false,
    };

    if !executable_path {
        return Err(AppError::BadRequest(format!(
            "search parameter '{}' uses an unsupported {:?} registry path",
            param.code, param.param_type
        )));
    }

    if matches!(param.param_type, SearchParamType::Date) {
        for value in values {
            super::date::parse_fhir_date_value(value).map_err(AppError::BadRequest)?;
        }
    }

    if matches!(
        param.param_type,
        SearchParamType::Number | SearchParamType::Quantity
    ) {
        for value in values {
            if has_unsupported_comparator(value) {
                return Err(AppError::BadRequest(format!(
                    "search parameter '{}' uses an unsupported comparator",
                    param.code
                )));
            }
        }
    }

    if param.param_type == SearchParamType::Special
        && values.iter().any(|value| parse_near_value(value).is_none())
    {
        return Err(AppError::BadRequest(format!(
            "search parameter '{}' has an invalid near value",
            param.code
        )));
    }

    Ok(())
}

pub fn is_executable_search_param(param: &SearchParam) -> bool {
    // Pick a placeholder value that is structurally valid for the parameter
    // type so the validation path can reach the executable-path check without
    // short-circuiting on type-specific value validation.
    let placeholder = match param.param_type {
        SearchParamType::Date => "2000-01-01",
        _ => "0|0",
    };
    validate_search_filter(param, &[placeholder.to_owned()]).is_ok()
}

fn has_unsupported_comparator(value: &str) -> bool {
    matches!(
        value.get(..2),
        Some("eq" | "ne" | "gt" | "lt" | "ge" | "le" | "sa" | "eb" | "ap")
    )
}

fn push_search_filter_value(
    query: &mut QueryBuilder<Postgres>,
    filter: &SearchFilter,
    value: &str,
    geo_mode: GeoSearchMode,
) {
    match filter.param.param_type {
        SearchParamType::String => {
            push_string_filter(query, &filter.param.path, value, filter.modifier)
        }
        SearchParamType::Token => push_token_filter(query, &filter.param.path, value),
        SearchParamType::Reference => push_reference_filter(query, &filter.param.path, value),
        SearchParamType::Date => push_date_filter(query, &filter.param.path, value),
        SearchParamType::Uri => push_uri_filter(query, &filter.param.path, value),
        SearchParamType::Number => push_number_filter(query, &filter.param.path, value),
        SearchParamType::Quantity => push_quantity_filter(query, &filter.param.path, value),
        SearchParamType::Special => push_special_filter(query, &filter.param.path, value, geo_mode),
        // Composite is not yet supported as a search filter.
        SearchParamType::Composite => {}
    }
}

// ---------------------------------------------------------------------------
// String search: case-insensitive prefix match (FHIR default for string)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum StringMatch {
    Prefix,
    Exact,
    Contains,
}

impl StringMatch {
    fn from_modifier(modifier: Option<SearchModifier>) -> Self {
        match modifier {
            None => Self::Prefix,
            Some(SearchModifier::Exact) => Self::Exact,
            Some(SearchModifier::Contains) => Self::Contains,
        }
    }

    fn pattern(self, value: &str) -> String {
        match self {
            Self::Prefix => format!("{}%", value.to_lowercase()),
            Self::Contains => format!("%{}%", value.to_lowercase()),
            Self::Exact => value.to_owned(),
        }
    }

    const fn operator(self) -> &'static str {
        match self {
            Self::Exact => " = ",
            Self::Prefix | Self::Contains => " LIKE ",
        }
    }
}

fn push_string_filter(
    query: &mut QueryBuilder<Postgres>,
    path: &JsonPath,
    value: &str,
    modifier: Option<SearchModifier>,
) {
    let value = crate::search::unescape_fhir_value(value);
    let mode = StringMatch::from_modifier(modifier);
    let pattern = mode.pattern(&value);

    match path {
        JsonPath::Field(segments) => {
            // For a field that is an array of complex objects (like `name`),
            // we need to search within array elements. For simple fields
            // (like `address.city`), we can use a direct text extraction.
            //
            // Heuristic: if the path has a single segment and the field is
            // typically an array of complex types (HumanName, Address, etc.),
            // we use jsonb_array_elements. Otherwise we use ->> for the leaf.
            if segments.len() == 1 {
                // Single segment: could be an array of objects or a scalar.
                // Use a broad search that handles both cases.
                push_string_array_or_scalar(query, segments[0], &pattern, mode);
            } else {
                // Multi-segment path: navigate into nested objects.
                // The parent segments may be arrays, so we use jsonb extraction
                // and search within them.
                push_string_nested_field(query, segments, &pattern, mode);
            }
        }
        JsonPath::WhereFilter {
            base,
            filter_field,
            filter_value,
            suffix,
        } => {
            push_string_where_filter(
                query,
                base,
                filter_field,
                filter_value,
                suffix,
                &pattern,
                mode,
            );
        }
        JsonPath::ResourceId
        | JsonPath::FieldAlternatives(_)
        | JsonPath::Exists(_)
        | JsonPath::ExistsAlternatives(_)
        | JsonPath::Position(_) => {
            // Exists/Position don't apply to string search, skip.
        }
    }
}

fn push_string_array_or_scalar(
    query: &mut QueryBuilder<Postgres>,
    field: &str,
    pattern: &str,
    mode: StringMatch,
) {
    // Search scalar strings and the direct string members of complex array
    // elements (e.g. HumanName.family and HumanName.given).  Searching the
    // JSON serialization itself would make prefix semantics depend on JSON
    // key order instead of the FHIR string values.
    query.push(" AND (");
    if !matches!(mode, StringMatch::Exact) {
        query.push("lower(");
    }
    query.push("resource->>'");
    query.push(field);
    query.push("'");
    if !matches!(mode, StringMatch::Exact) {
        query.push(")");
    }
    query.push(mode.operator());
    query.push_bind(pattern.to_owned());
    let arr_expr = safe_array_elements(&format!("resource->'{field}'"));
    query.push(format!(
        " OR EXISTS (SELECT 1 FROM {arr_expr} AS elem WHERE "
    ));
    if !matches!(mode, StringMatch::Exact) {
        query.push("lower(");
    }
    query.push("elem #>> '{}'");
    if !matches!(mode, StringMatch::Exact) {
        query.push(")");
    }
    query.push(mode.operator());
    query.push_bind(pattern.to_owned());
    query.push(" OR EXISTS (SELECT 1 FROM jsonb_each_text(CASE WHEN jsonb_typeof(elem) = 'object' THEN elem ELSE '{}'::jsonb END) AS attr(key, value) WHERE ");
    if !matches!(mode, StringMatch::Exact) {
        query.push("lower(");
    }
    query.push("attr.value");
    if !matches!(mode, StringMatch::Exact) {
        query.push(")");
    }
    query.push(mode.operator());
    query.push_bind(pattern.to_owned());
    query.push(" OR EXISTS (SELECT 1 FROM jsonb_each(CASE WHEN jsonb_typeof(elem) = 'object' THEN elem ELSE '{}'::jsonb END) AS attr(key, value), jsonb_array_elements_text(CASE WHEN jsonb_typeof(attr.value) = 'array' THEN attr.value ELSE '[]'::jsonb END) AS item(value) WHERE ");
    if !matches!(mode, StringMatch::Exact) {
        query.push("lower(");
    }
    query.push("item.value");
    if !matches!(mode, StringMatch::Exact) {
        query.push(")");
    }
    query.push(mode.operator());
    query.push_bind(pattern.to_owned());
    query.push("))))");
}

fn push_string_nested_field(
    query: &mut QueryBuilder<Postgres>,
    segments: &[&str],
    pattern: &str,
    mode: StringMatch,
) {
    // For nested paths like ["address", "city"], we need to handle the case
    // where the parent "address" is an array.
    // Strategy: treat first segment as potential array, drill into sub-fields.
    if segments.len() == 2 {
        let parent = segments[0];
        let child = segments[1];
        // Parent might be an array of objects
        let arr = safe_array_elements(&format!("resource->'{parent}'"));
        query.push(format!(" AND EXISTS (SELECT 1 FROM {arr} AS elem WHERE "));
        if !matches!(mode, StringMatch::Exact) {
            query.push("lower(");
        }
        query.push("elem->>'");
        query.push(child);
        query.push("'");
        if !matches!(mode, StringMatch::Exact) {
            query.push(")");
        }
        query.push(mode.operator());
        query.push_bind(pattern.to_owned());

        // Also search in sub-arrays (e.g., name.given is an array of strings)
        query.push(" OR EXISTS (SELECT 1 FROM jsonb_array_elements_text(COALESCE(elem->'");
        query.push(child);
        query.push("', '[]'::jsonb)) AS subelem WHERE ");
        if !matches!(mode, StringMatch::Exact) {
            query.push("lower(");
        }
        query.push("subelem");
        if !matches!(mode, StringMatch::Exact) {
            query.push(")");
        }
        query.push(mode.operator());
        query.push_bind(pattern.to_owned());
        query.push("))");
    } else if segments.len() >= 3 {
        // Deep nesting: navigate step by step
        let parent = segments[0];
        let mid = segments[1];
        let child = segments[2];
        let arr = safe_array_elements(&format!("resource->'{parent}'"));
        query.push(format!(" AND EXISTS (SELECT 1 FROM {arr} AS elem WHERE "));
        if !matches!(mode, StringMatch::Exact) {
            query.push("lower(");
        }
        query.push("elem->'");
        query.push(mid);
        query.push("'->>'");
        query.push(child);
        query.push("'");
        if !matches!(mode, StringMatch::Exact) {
            query.push(")");
        }
        query.push(mode.operator());
        query.push_bind(pattern.to_owned());
        query.push(")");
    }
}

fn push_string_where_filter(
    query: &mut QueryBuilder<Postgres>,
    base: &[&str],
    filter_field: &str,
    filter_value: &str,
    suffix: &[&str],
    pattern: &str,
    mode: StringMatch,
) {
    let base_path = build_jsonb_path("resource", base);
    let arr = safe_array_elements(&base_path);
    query.push(format!(
        " AND EXISTS (SELECT 1 FROM {arr} AS elem WHERE elem->>'"
    ));
    query.push(filter_field);
    query.push("' = '");
    query.push(filter_value);
    query.push("'");
    if suffix.is_empty() {
        query.push(" AND ");
        if !matches!(mode, StringMatch::Exact) {
            query.push("lower(");
        }
        query.push("elem::text");
        if !matches!(mode, StringMatch::Exact) {
            query.push(")");
        }
    } else {
        let suffix_path = build_jsonb_text_path("elem", suffix);
        query.push(" AND ");
        if !matches!(mode, StringMatch::Exact) {
            query.push("lower(");
        }
        query.push(&suffix_path);
        if !matches!(mode, StringMatch::Exact) {
            query.push(")");
        }
    }
    query.push(mode.operator());
    query.push_bind(pattern.to_owned());
    query.push(")");
}

// ---------------------------------------------------------------------------
// Token search: exact match on code/system|code/boolean values
// ---------------------------------------------------------------------------

fn push_token_filter(query: &mut QueryBuilder<Postgres>, path: &JsonPath, value: &str) {
    // Token search supports:
    // - [code]: match the code/value in any system
    // - [system|code]: match both system and code
    // - [|code]: match code with no system
    let (system, code) = parse_token_value(value);

    match path {
        JsonPath::ResourceId => {
            query.push(" AND id = ");
            query.push_bind(code);
        }
        JsonPath::Field(segments) => {
            if segments.len() == 1 {
                let field = segments[0];
                push_token_single_field(query, field, system.as_deref(), &code);
            } else {
                push_token_nested_field(query, segments, system.as_deref(), &code);
            }
        }
        JsonPath::WhereFilter {
            base,
            filter_field,
            filter_value,
            suffix,
        } => {
            push_token_where_filter(
                query,
                base,
                filter_field,
                filter_value,
                suffix,
                system.as_deref(),
                &code,
            );
        }
        JsonPath::Exists(segments) => {
            // For exists-type tokens (e.g., deceased), check if the field
            // exists and is not false/null
            push_exists_filter(query, segments, &code);
        }
        JsonPath::ExistsAlternatives(paths) => {
            if code == "false" {
                // Both choice representations must satisfy the false
                // predicate. Otherwise, a missing boolean alternative would
                // incorrectly mask an entered deceasedDateTime.
                query.push(" AND (TRUE");
                for segments in *paths {
                    query.push(" AND (TRUE");
                    push_exists_filter(query, segments, &code);
                    query.push(")");
                }
            } else {
                query.push(" AND (FALSE");
                for segments in *paths {
                    query.push(" OR (TRUE");
                    push_exists_filter(query, segments, &code);
                    query.push(")");
                }
            }
            query.push(")");
        }
        JsonPath::FieldAlternatives(_) | JsonPath::Position(_) => {}
    }
}

fn push_token_single_field(
    query: &mut QueryBuilder<Postgres>,
    field: &str,
    system: Option<&str>,
    code: &str,
) {
    // Token fields can be:
    // 1. A simple scalar (status, active, gender) -> exact match on text value
    // 2. An array of Identifier/CodeableConcept objects -> containment check
    // 3. A CodeableConcept object -> check coding array

    // For scalar values (most common for status, gender, active, etc.)
    // Also handle array fields like identifier
    match field {
        "identifier" => push_token_identifier(query, field, system, code),
        _ => {
            // Try scalar match first, then coding match, then identifier match
            query.push(" AND (resource->>'");
            query.push(field);
            query.push("' = ");
            query.push_bind(code.to_owned());

            // Also check CodeableConcept.coding with JSON containment so a
            // GIN index on (resource->'<field>'->'coding') can be used.
            query.push(" OR resource->'");
            query.push(field);
            query.push("'->'coding' @> jsonb_build_array(jsonb_build_object('code', to_jsonb(");
            query.push_bind(code.to_owned());
            query.push("::text)");

            if let Some(sys) = system {
                query.push(", 'system', to_jsonb(");
                query.push_bind(sys.to_owned());
                query.push("::text)");
            }

            query.push("))");

            // Also check if it's an array of CodeableConcept
            let arr = safe_array_elements(&format!("resource->'{field}'"));
            query.push(" OR resource->'");
            query.push(field);
            query.push("' @> jsonb_build_array(jsonb_build_object('coding', jsonb_build_array(jsonb_build_object('code', to_jsonb(");
            query.push_bind(code.to_owned());
            query.push("::text)");

            if let Some(sys) = system {
                query.push(", 'system', to_jsonb(");
                query.push_bind(sys.to_owned());
                query.push("::text)");
            }

            query.push("))))");

            // Check array of identifiers
            query.push(format!(
                " OR EXISTS (SELECT 1 FROM {arr} AS elem WHERE elem->>'value' = "
            ));
            query.push_bind(code.to_owned());

            if let Some(sys) = system {
                query.push(" AND elem->>'system' = ");
                query.push_bind(sys.to_owned());
            }

            query.push("))");
        }
    }
}

fn push_token_identifier(
    query: &mut QueryBuilder<Postgres>,
    field: &str,
    system: Option<&str>,
    code: &str,
) {
    // Identifier is always an array of {system, value} objects.
    // NOTE: We use a bare `resource->'field'` (no COALESCE) so the GIN
    // jsonb_path_ops index on (resource->'identifier') can be used.
    // When the field is NULL, `NULL @> anything` evaluates to NULL (falsy
    // in a WHERE clause) — same behaviour as the old COALESCE version.
    query.push(" AND resource->'");
    query.push(field);
    query.push("' @> jsonb_build_array(jsonb_build_object('value', to_jsonb(");
    query.push_bind(code.to_owned());
    query.push("::text)");

    if let Some(sys) = system {
        query.push(", 'system', to_jsonb(");
        query.push_bind(sys.to_owned());
        query.push("::text)");
    }

    query.push("))");
}

fn push_token_nested_field(
    query: &mut QueryBuilder<Postgres>,
    segments: &[&str],
    system: Option<&str>,
    code: &str,
) {
    // For nested token fields like code.coding, communication.language
    if segments.len() == 2 {
        let parent = segments[0];
        let child = segments[1];

        let arr = safe_array_elements(&format!("resource->'{parent}'"));

        // Check if it's a coding-style pattern (parent->child->coding)
        // or a direct value pattern
        query.push(" AND (");

        // Direct value in nested object
        query.push(format!(
            "EXISTS (SELECT 1 FROM {arr} AS elem WHERE elem->>'"
        ));
        query.push(child);
        query.push("' = ");
        query.push_bind(code.to_owned());

        query.push(")");

        // Also check CodeableConcept (child has .coding array)
        query.push(format!(
            " OR EXISTS (SELECT 1 FROM {arr} AS elem, jsonb_array_elements(COALESCE(elem->'"
        ));
        query.push(child);
        query.push("'->'coding', '[]'::jsonb)) AS coding WHERE coding->>'code' = ");
        query.push_bind(code.to_owned());

        if let Some(sys) = system {
            query.push(" AND coding->>'system' = ");
            query.push_bind(sys.to_owned());
        }

        query.push(")");
        query.push(")");
    } else {
        // For deeper nesting, use the full path
        let jsonb_text = build_jsonb_text_path("resource", segments);
        query.push(" AND ");
        query.push(&jsonb_text);
        query.push(" = ");
        query.push_bind(code.to_owned());
    }
}

fn push_token_where_filter(
    query: &mut QueryBuilder<Postgres>,
    base: &[&str],
    filter_field: &str,
    filter_value: &str,
    suffix: &[&str],
    system: Option<&str>,
    code: &str,
) {
    let base_path = build_jsonb_path("resource", base);
    let arr = safe_array_elements(&base_path);
    query.push(format!(
        " AND EXISTS (SELECT 1 FROM {arr} AS elem WHERE elem->>'"
    ));
    query.push(filter_field);
    query.push("' = '");
    query.push(filter_value);
    query.push("'");

    if suffix.is_empty() {
        // Match the value field of the element
        query.push(" AND elem->>'value' = ");
        query.push_bind(code.to_owned());

        if let Some(sys) = system {
            query.push(" AND elem->>'system' = ");
            query.push_bind(sys.to_owned());
        }
    } else {
        let suffix_text = build_jsonb_text_path("elem", suffix);
        query.push(" AND ");
        query.push(&suffix_text);
        query.push(" = ");
        query.push_bind(code.to_owned());
    }

    query.push(")");
}

fn push_exists_filter(query: &mut QueryBuilder<Postgres>, segments: &[&str], value: &str) {
    let jsonb_path = build_jsonb_path("resource", segments);
    match value {
        "true" => {
            query.push(" AND ");
            query.push(&jsonb_path);
            query.push(" IS NOT NULL AND ");
            query.push(&jsonb_path);
            query.push(" != 'false'::jsonb AND ");
            query.push(&jsonb_path);
            query.push(" != 'null'::jsonb");
        }
        "false" => {
            query.push(" AND (");
            query.push(&jsonb_path);
            query.push(" IS NULL OR ");
            query.push(&jsonb_path);
            query.push(" = 'false'::jsonb OR ");
            query.push(&jsonb_path);
            query.push(" = 'null'::jsonb)");
        }
        _ => {
            // For other values, just check existence
            query.push(" AND ");
            query.push(&jsonb_path);
            query.push(" IS NOT NULL");
        }
    }
}

// ---------------------------------------------------------------------------
// Reference search: match reference strings like "Patient/123"
// ---------------------------------------------------------------------------

fn push_reference_filter(query: &mut QueryBuilder<Postgres>, path: &JsonPath, value: &str) {
    let value = crate::search::unescape_fhir_value(value);
    match path {
        JsonPath::Field(segments) => {
            if segments.len() == 1 {
                let field = segments[0];
                // Reference can be a single object {reference: "..."} or array
                query.push(" AND (resource->'");
                query.push(field);
                query.push("'->>'reference' = ");
                query.push_bind(value.clone());

                // Or it's an array of references
                let arr = safe_array_elements(&format!("resource->'{field}'"));
                query.push(format!(
                    " OR EXISTS (SELECT 1 FROM {arr} AS elem WHERE elem->>'reference' = "
                ));
                query.push_bind(value.clone());
                query.push("))");
            } else {
                // Nested reference: navigate to the field and check .reference
                // Last segment might be "reference" already or we need to add it
                let last = *segments.last().unwrap_or(&"");
                if last == "reference" {
                    let jsonb_text = build_jsonb_text_path("resource", segments);
                    query.push(" AND ");
                    query.push(&jsonb_text);
                    query.push(" = ");
                    query.push_bind(value.clone());
                } else {
                    // Navigate to the object and check .reference
                    let jsonb_path = build_jsonb_path("resource", segments);
                    query.push(" AND (");
                    query.push(&jsonb_path);
                    query.push("->>'reference' = ");
                    query.push_bind(value.clone());

                    // Also check array case
                    let arr = safe_array_elements(&jsonb_path);
                    query.push(format!(
                        " OR EXISTS (SELECT 1 FROM {arr} AS elem WHERE elem->>'reference' = "
                    ));
                    query.push_bind(value.clone());
                    query.push("))");
                }
            }
        }
        JsonPath::WhereFilter {
            base,
            filter_field,
            filter_value,
            suffix,
        } => {
            let base_path = build_jsonb_path("resource", base);
            let arr = safe_array_elements(&base_path);
            query.push(format!(
                " AND EXISTS (SELECT 1 FROM {arr} AS elem WHERE elem->>'"
            ));
            query.push(filter_field);
            query.push("' = '");
            query.push(filter_value);
            query.push("'");

            if suffix.is_empty() {
                query.push(" AND elem->>'reference' = ");
            } else {
                let suffix_path = build_jsonb_text_path("elem", suffix);
                query.push(" AND ");
                query.push(&suffix_path);
                query.push(" = ");
            }
            query.push_bind(value);
            query.push(")");
        }
        JsonPath::ResourceId
        | JsonPath::FieldAlternatives(_)
        | JsonPath::Exists(_)
        | JsonPath::ExistsAlternatives(_)
        | JsonPath::Position(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Date search: FHIR comparator prefixes with precision-aware ranges.
// See `super::date` for the supported prefix semantics.
// ---------------------------------------------------------------------------

fn push_date_filter(query: &mut QueryBuilder<Postgres>, path: &JsonPath, value: &str) {
    let value = crate::search::unescape_fhir_value(value);
    // Validation runs before SQL generation, so this branch indicates an
    // internal caller bug. Fail closed rather than silently dropping a filter.
    let Ok((prefix, bounds)) = super::date::parse_fhir_date_value(&value) else {
        query.push(" AND FALSE");
        return;
    };
    match path {
        JsonPath::Field(segments) => push_date_field_filter(query, segments, prefix, bounds),
        JsonPath::FieldAlternatives(paths) => {
            query.push(" AND (FALSE");
            for segments in *paths {
                query.push(" OR (TRUE");
                push_date_field_filter(query, segments, prefix, bounds);
                query.push(")");
            }
            query.push(")");
        }
        JsonPath::WhereFilter {
            base,
            filter_field,
            filter_value,
            suffix,
        } => {
            let base_path = build_jsonb_path("resource", base);
            let arr = safe_array_elements(&base_path);
            let target = if suffix.is_empty() {
                "elem".to_owned()
            } else {
                build_jsonb_path("elem", suffix)
            };
            let values = safe_array_elements(&target);
            query.push(format!(
                " AND EXISTS (SELECT 1 FROM {arr} AS elem, {values} AS date_value WHERE elem->>'"
            ));
            query.push(filter_field);
            query.push("' = '");
            query.push(filter_value);
            query.push("'");
            super::date::push_date_predicate(query, "date_value", prefix, bounds);
            query.push(")");
        }
        JsonPath::ResourceId
        | JsonPath::Exists(_)
        | JsonPath::ExistsAlternatives(_)
        | JsonPath::Position(_) => {}
    }
}

fn push_date_field_filter(
    query: &mut QueryBuilder<Postgres>,
    segments: &[&str],
    prefix: super::date::DatePrefix,
    bounds: super::date::DateBounds,
) {
    if segments.len() >= 2 {
        let parent = segments[0];
        let child_segments = &segments[1..];
        let arr = safe_array_elements(&format!("resource->'{parent}'"));
        let values = safe_array_elements(&build_jsonb_path("elem", child_segments));
        query.push(format!(
            " AND EXISTS (SELECT 1 FROM {arr} AS elem, {values} AS date_value WHERE TRUE"
        ));
        super::date::push_date_predicate(query, "date_value", prefix, bounds);
        query.push(")");
    } else {
        let values = safe_array_elements(&build_jsonb_path("resource", segments));
        query.push(format!(
            " AND EXISTS (SELECT 1 FROM {values} AS date_value WHERE TRUE"
        ));
        super::date::push_date_predicate(query, "date_value", prefix, bounds);
        query.push(")");
    }
}

// ---------------------------------------------------------------------------
// URI search: exact match
// ---------------------------------------------------------------------------

fn push_uri_filter(query: &mut QueryBuilder<Postgres>, path: &JsonPath, value: &str) {
    let value = crate::search::unescape_fhir_value(value);
    match path {
        JsonPath::Field(segments) => {
            let jsonb_text = build_jsonb_text_path("resource", segments);
            query.push(" AND ");
            query.push(&jsonb_text);
            query.push(" = ");
            query.push_bind(value);
        }
        JsonPath::ResourceId
        | JsonPath::FieldAlternatives(_)
        | JsonPath::WhereFilter { .. }
        | JsonPath::Exists(_)
        | JsonPath::ExistsAlternatives(_)
        | JsonPath::Position(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Number search: exact numeric match
// ---------------------------------------------------------------------------

fn push_number_filter(query: &mut QueryBuilder<Postgres>, path: &JsonPath, value: &str) {
    let value = crate::search::unescape_fhir_value(value);
    match path {
        JsonPath::Field(segments) => {
            let jsonb_path = build_jsonb_path("resource", segments);
            // Cast JSONB value to numeric for comparison
            query.push(" AND (");
            query.push(&jsonb_path);
            query.push(")::text::numeric = ");
            query.push_bind(value);
            query.push("::numeric");
        }
        JsonPath::ResourceId
        | JsonPath::FieldAlternatives(_)
        | JsonPath::WhereFilter { .. }
        | JsonPath::Exists(_)
        | JsonPath::ExistsAlternatives(_)
        | JsonPath::Position(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Quantity search: match value and optionally system|code
// ---------------------------------------------------------------------------

fn push_quantity_filter(query: &mut QueryBuilder<Postgres>, path: &JsonPath, value: &str) {
    // Quantity format: [number]|[system]|[code]
    let parts: Vec<&str> = value.splitn(3, '|').collect();
    let number = parts.first().copied().unwrap_or("");
    let system = parts.get(1).copied();
    let code = parts.get(2).copied();

    match path {
        JsonPath::Field(segments) => {
            let jsonb_path = build_jsonb_path("resource", segments);

            query.push(" AND (");
            if !number.is_empty() {
                query.push("(");
                query.push(&jsonb_path);
                query.push("->>'value')::numeric = ");
                query.push_bind(number.to_owned());
                query.push("::numeric");
            } else {
                query.push("TRUE");
            }

            if let Some(sys) = system
                && !sys.is_empty()
            {
                query.push(" AND ");
                query.push(&jsonb_path);
                query.push("->>'system' = ");
                query.push_bind(sys.to_owned());
            }

            if let Some(c) = code
                && !c.is_empty()
            {
                query.push(" AND (");
                query.push(&jsonb_path);
                query.push("->>'code' = ");
                query.push_bind(c.to_owned());
                query.push(" OR ");
                query.push(&jsonb_path);
                query.push("->>'unit' = ");
                query.push_bind(c.to_owned());
                query.push(")");
            }

            query.push(")");
        }
        JsonPath::ResourceId
        | JsonPath::FieldAlternatives(_)
        | JsonPath::WhereFilter { .. }
        | JsonPath::Exists(_)
        | JsonPath::ExistsAlternatives(_)
        | JsonPath::Position(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Special search: handles type-specific special parameters (e.g. near)
// ---------------------------------------------------------------------------

fn push_special_filter(
    query: &mut QueryBuilder<Postgres>,
    path: &JsonPath,
    value: &str,
    geo_mode: GeoSearchMode,
) {
    if let JsonPath::Position(segments) = path {
        push_near_filter(query, segments, value, geo_mode);
    }
}

/// Parse a FHIR `near` search value: `latitude|longitude|distance|units`.
///
/// Returns `(latitude, longitude, distance_meters)` or `None` if the value
/// is malformed. Distance defaults to 5 km when omitted. The unit is
/// converted to meters (supports `km` and `mi`; defaults to `km`).
fn parse_near_value(value: &str) -> Option<(f64, f64, f64)> {
    let parts: Vec<&str> = value.splitn(4, '|').collect();
    if parts.len() < 2 {
        return None;
    }
    let lat: f64 = parts[0].parse().ok()?;
    let lon: f64 = parts[1].parse().ok()?;

    // Validate ranges
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }

    let distance_raw: f64 = parts.get(2).and_then(|d| d.parse().ok()).unwrap_or(5.0);
    let unit = parts.get(3).copied().unwrap_or("km");

    let distance_meters = match unit {
        "mi" => distance_raw * 1609.344,
        _ => distance_raw * 1000.0, // default km
    };

    if distance_meters <= 0.0 {
        return None;
    }

    Some((lat, lon, distance_meters))
}

/// Append a geospatial proximity filter for the FHIR `near` parameter.
///
/// Selects the SQL based on [`GeoSearchMode`]:
///
/// * [`GeoSearchMode::EarthDistance`] uses the `earthdistance` extension,
///   which is backed by a GiST index:
///   ```sql
///   AND resource->'position' IS NOT NULL
///   AND earth_box(ll_to_earth($lat, $lon), $distance_meters)
///         @> ll_to_earth(
///               (resource->'position'->>'latitude')::float8,
///               (resource->'position'->>'longitude')::float8)
///   AND earth_distance(
///         ll_to_earth(
///           (resource->'position'->>'latitude')::float8,
///           (resource->'position'->>'longitude')::float8),
///         ll_to_earth($lat, $lon)
///       ) <= $distance_meters
///   ```
/// * [`GeoSearchMode::Haversine`] uses a pure-SQL haversine distance formula
///   that needs no extension and therefore works on any database:
///   ```sql
///   AND resource->'position' IS NOT NULL
///   AND 2 * 6371000.0 * asin(sqrt(
///         power(sin((radians((resource->'position'->>'latitude')::float8)
///                    - radians($lat)) / 2), 2)
///         + cos(radians((resource->'position'->>'latitude')::float8))
///           * cos(radians($lat))
///           * power(sin((radians((resource->'position'->>'longitude')::float8)
///                        - radians($lon)) / 2), 2)
///       )) <= $distance_meters
///   ```
fn push_near_filter(
    query: &mut QueryBuilder<Postgres>,
    segments: &[&str],
    value: &str,
    geo_mode: GeoSearchMode,
) {
    let Some((lat, lon, distance_meters)) = parse_near_value(value) else {
        return;
    };

    let pos_path = build_jsonb_path("resource", segments);

    // Guard: skip rows without a position
    query.push(" AND ");
    query.push(&pos_path);
    query.push(" IS NOT NULL");

    match geo_mode {
        GeoSearchMode::EarthDistance => {
            // Use an indexable bounding box first, then the exact great-circle
            // distance to discard the box's corner false positives.
            query.push(" AND earth_box(ll_to_earth(");
            query.push_bind(lat);
            query.push(", ");
            query.push_bind(lon);
            query.push("), ");
            query.push_bind(distance_meters);
            query.push(") @> ll_to_earth((");
            query.push(&pos_path);
            query.push("->>'latitude')::float8, (");
            query.push(&pos_path);
            query.push("->>'longitude')::float8)");

            // The bounding box is deliberately approximate; this predicate
            // supplies the exact circular distance check.
            query.push(" AND earth_distance(ll_to_earth((");
            query.push(&pos_path);
            query.push("->>'latitude')::float8, (");
            query.push(&pos_path);
            query.push("->>'longitude')::float8), ll_to_earth(");
            query.push_bind(lat);
            query.push(", ");
            query.push_bind(lon);
            query.push(")) <= ");
            query.push_bind(distance_meters);
        }
        GeoSearchMode::Haversine => {
            // Pure-SQL haversine distance; no extension required. Uses the mean
            // Earth radius (6371 km); the slight difference from
            // earthdistance's radius is immaterial for a proximity search.
            query.push(" AND 2 * 6371000.0 * asin(sqrt(");
            query.push("power(sin((radians((");
            query.push(&pos_path);
            query.push("->>'latitude')::float8) - radians(");
            query.push_bind(lat);
            query.push(")) / 2), 2)");
            query.push(" + cos(radians((");
            query.push(&pos_path);
            query.push("->>'latitude')::float8)) * cos(radians(");
            query.push_bind(lat);
            query.push("))");
            query.push(" * power(sin((radians((");
            query.push(&pos_path);
            query.push("->>'longitude')::float8) - radians(");
            query.push_bind(lon);
            query.push(")) / 2), 2)");
            query.push(")) <= ");
            query.push_bind(distance_meters);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a JSONB navigation expression: `root->'a'->'b'->'c'`
fn build_jsonb_path(root: &str, segments: &[&str]) -> String {
    let mut path = root.to_owned();
    for seg in segments {
        path.push_str("->'");
        path.push_str(seg);
        path.push('\'');
    }
    path
}

/// Build a safe `jsonb_array_elements(...)` expression that handles values
/// that may be a JSON array, a single object/scalar, or null.
///
/// - Array  → iterate directly
/// - Object/scalar → wrap in a single-element array
/// - Null   → empty array
fn safe_array_elements(expr: &str) -> String {
    format!(
        "jsonb_array_elements(CASE \
         WHEN jsonb_typeof({expr}) = 'array' THEN {expr} \
         WHEN {expr} IS NOT NULL THEN jsonb_build_array({expr}) \
         ELSE '[]'::jsonb END)"
    )
}

/// Build a JSONB text extraction expression: `root->'a'->'b'->>'c'`
/// The last segment uses ->> to extract text.
fn build_jsonb_text_path(root: &str, segments: &[&str]) -> String {
    if segments.is_empty() {
        return format!("{root}::text");
    }

    let mut path = root.to_owned();
    for (i, seg) in segments.iter().enumerate() {
        if i == segments.len() - 1 {
            path.push_str("->>'");
        } else {
            path.push_str("->'");
        }
        path.push_str(seg);
        path.push('\'');
    }
    path
}

/// Parse a token search value into (system, code).
///
/// Formats:
/// - `code` → (None, code)
/// - `system|code` → (Some(system), code)
/// - `|code` → (Some(""), code)  treated as "no system"
fn parse_token_value(value: &str) -> (Option<String>, String) {
    let parts = crate::search::split_fhir_delimiter(value, '|')
        .expect("search values are escape-validated before SQL generation");
    if parts.len() >= 2 {
        (
            Some(crate::search::unescape_fhir_value(&parts[0])),
            crate::search::unescape_fhir_value(&parts[1..].join("|")),
        )
    } else {
        (None, crate::search::unescape_fhir_value(&parts[0]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_token_value_code_only() {
        let (system, code) = parse_token_value("active");
        assert!(system.is_none());
        assert_eq!(code, "active");
    }

    #[test]
    fn parse_token_value_system_and_code() {
        let (system, code) = parse_token_value("http://example.org|ABC");
        assert_eq!(system.as_deref(), Some("http://example.org"));
        assert_eq!(code, "ABC");
    }

    #[test]
    fn parse_token_value_empty_system() {
        let (system, code) = parse_token_value("|ABC");
        assert_eq!(system.as_deref(), Some(""));
        assert_eq!(code, "ABC");
    }

    #[test]
    fn parse_token_value_honors_escaped_pipe() {
        let (system, code) = parse_token_value(r"http://example.org/a\|b|ABC");
        assert_eq!(system.as_deref(), Some("http://example.org/a|b"));
        assert_eq!(code, "ABC");
    }

    #[test]
    fn resource_id_filter_targets_the_indexed_id_column() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_token_filter(&mut query, &JsonPath::ResourceId, "patient-123");

        assert!(query.into_sql().as_str().contains("AND id = $1"));
    }

    #[test]
    fn build_jsonb_path_single() {
        assert_eq!(
            build_jsonb_path("resource", &["status"]),
            "resource->'status'"
        );
    }

    #[test]
    fn build_jsonb_path_nested() {
        assert_eq!(
            build_jsonb_path("resource", &["code", "coding"]),
            "resource->'code'->'coding'"
        );
    }

    #[test]
    fn build_jsonb_text_path_single() {
        assert_eq!(
            build_jsonb_text_path("resource", &["status"]),
            "resource->>'status'"
        );
    }

    #[test]
    fn build_jsonb_text_path_nested() {
        assert_eq!(
            build_jsonb_text_path("resource", &["subject", "reference"]),
            "resource->'subject'->>'reference'"
        );
    }

    // -----------------------------------------------------------------------
    // Near / geospatial parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_near_full_value() {
        let (lat, lon, dist) = parse_near_value("42.36|-71.06|10|km").unwrap();
        assert!((lat - 42.36).abs() < 1e-6);
        assert!((lon - (-71.06)).abs() < 1e-6);
        assert!((dist - 10_000.0).abs() < 1e-6);
    }

    #[test]
    fn parse_near_miles() {
        let (_, _, dist) = parse_near_value("42.36|-71.06|5|mi").unwrap();
        assert!((dist - 5.0 * 1609.344).abs() < 1e-3);
    }

    #[test]
    fn parse_near_default_distance() {
        // Omitting distance and unit defaults to 5 km
        let (lat, lon, dist) = parse_near_value("42.36|-71.06").unwrap();
        assert!((lat - 42.36).abs() < 1e-6);
        assert!((lon - (-71.06)).abs() < 1e-6);
        assert!((dist - 5_000.0).abs() < 1e-6);
    }

    #[test]
    fn parse_near_rejects_single_value() {
        assert!(parse_near_value("42.36").is_none());
    }

    #[test]
    fn parse_near_rejects_out_of_range_latitude() {
        assert!(parse_near_value("91.0|-71.06|10|km").is_none());
    }

    #[test]
    fn parse_near_rejects_out_of_range_longitude() {
        assert!(parse_near_value("42.36|181.0|10|km").is_none());
    }

    #[test]
    fn parse_near_rejects_zero_distance() {
        assert!(parse_near_value("42.36|-71.06|0|km").is_none());
    }

    #[test]
    fn parse_near_rejects_negative_distance() {
        assert!(parse_near_value("42.36|-71.06|-5|km").is_none());
    }

    #[test]
    fn parse_near_rejects_non_numeric() {
        assert!(parse_near_value("abc|-71.06|10|km").is_none());
    }

    #[test]
    fn near_filter_produces_earth_distance_sql() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_near_filter(
            &mut query,
            &["position"],
            "42.36|-71.06|10|km",
            GeoSearchMode::EarthDistance,
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("earth_distance"),
            "expected earth_distance in SQL, got: {sql}"
        );
        assert!(
            sql.contains("earth_box"),
            "expected an indexable earth_box prefilter, got: {sql}"
        );
        assert!(
            sql.contains("@> ll_to_earth"),
            "expected the GiST containment operator, got: {sql}"
        );
        assert!(
            sql.contains("ll_to_earth"),
            "expected ll_to_earth in SQL, got: {sql}"
        );
        assert!(
            sql.contains("resource->'position'"),
            "expected position path in SQL, got: {sql}"
        );
    }

    #[test]
    fn near_filter_produces_haversine_sql() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_near_filter(
            &mut query,
            &["position"],
            "42.36|-71.06|10|km",
            GeoSearchMode::Haversine,
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            !sql.contains("earth_distance"),
            "haversine SQL must not reference earth_distance, got: {sql}"
        );
        assert!(
            !sql.contains("ll_to_earth"),
            "haversine SQL must not reference ll_to_earth, got: {sql}"
        );
        assert!(
            sql.contains("6371000.0"),
            "expected mean Earth radius in haversine SQL, got: {sql}"
        );
        assert!(
            sql.contains("asin(sqrt("),
            "expected haversine asin(sqrt(...)) formula, got: {sql}"
        );
        assert!(
            sql.contains("cos(radians(("),
            "expected haversine cosine term, got: {sql}"
        );
        assert!(
            sql.contains("resource->'position'"),
            "expected position path in SQL, got: {sql}"
        );
    }

    #[test]
    fn near_filter_skips_invalid_value() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_near_filter(
            &mut query,
            &["position"],
            "invalid",
            GeoSearchMode::EarthDistance,
        );
        let sql = query.into_sql().as_str().to_owned();
        // Should not add any condition for invalid input
        assert_eq!(sql, "SELECT 1 FROM t WHERE 1=1");
    }

    #[test]
    fn token_single_field_uses_containment_for_codeableconcept() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_token_single_field(&mut query, "code", None, "1234-5");
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("resource->'code'->'coding' @> jsonb_build_array(jsonb_build_object('code', to_jsonb("),
            "expected containment-based coding match, got: {sql}"
        );
        assert!(
            !sql.contains("jsonb_array_elements(COALESCE(resource->'code'->'coding'"),
            "expected old jsonb_array_elements coding match to be removed, got: {sql}"
        );
    }

    #[test]
    fn token_single_field_array_codeableconcept_uses_containment() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_token_single_field(
            &mut query,
            "category",
            Some("http://loinc.org"),
            "laboratory",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("resource->'category' @> jsonb_build_array(jsonb_build_object('coding', jsonb_build_array(jsonb_build_object('code', to_jsonb("),
            "expected containment-based array CodeableConcept match, got: {sql}"
        );
        assert!(
            sql.contains("'system', to_jsonb("),
            "expected system constraint in containment query, got: {sql}"
        );
    }

    // -----------------------------------------------------------------------
    // String filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn string_filter_single_field_produces_like() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_string_filter(&mut query, &JsonPath::Field(&["name"]), "peter", None);
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("lower(resource->>'name') LIKE"),
            "expected LIKE on scalar, got: {sql}"
        );
        assert!(
            sql.contains("jsonb_array_elements"),
            "expected array fallback, got: {sql}"
        );
    }

    #[test]
    fn string_filter_nested_field_drills_into_parent() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_string_filter(
            &mut query,
            &JsonPath::Field(&["address", "city"]),
            "boston",
            None,
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("elem->>'city'"),
            "expected nested city extraction, got: {sql}"
        );
    }

    #[test]
    fn string_filter_deep_nested_field() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_string_filter(
            &mut query,
            &JsonPath::Field(&["contact", "name", "family"]),
            "smith",
            None,
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("elem->'name'->>'family'"),
            "expected deep nested path, got: {sql}"
        );
    }

    #[test]
    fn string_filter_where_filter_produces_discriminated_search() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_string_filter(
            &mut query,
            &JsonPath::WhereFilter {
                base: &["telecom"],
                filter_field: "system",
                filter_value: "email",
                suffix: &["value"],
            },
            "john",
            None,
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("elem->>'system' = 'email'"),
            "expected system filter, got: {sql}"
        );
        assert!(
            sql.contains("elem->>'value'"),
            "expected value extraction, got: {sql}"
        );
    }

    #[test]
    fn string_filter_exists_and_position_are_noop() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_string_filter(&mut query, &JsonPath::Exists(&["deceased"]), "test", None);
        let sql = query.into_sql().as_str().to_owned();
        assert_eq!(
            sql, "SELECT 1 FROM t WHERE 1=1",
            "Exists should be no-op for string"
        );

        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_string_filter(&mut query, &JsonPath::Position(&["position"]), "test", None);
        let sql = query.into_sql().as_str().to_owned();
        assert_eq!(
            sql, "SELECT 1 FROM t WHERE 1=1",
            "Position should be no-op for string"
        );
    }

    // -----------------------------------------------------------------------
    // Reference filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn reference_filter_single_field_checks_reference() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_reference_filter(&mut query, &JsonPath::Field(&["subject"]), "Patient/123");
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("resource->'subject'->>'reference'"),
            "expected subject.reference extraction, got: {sql}"
        );
    }

    #[test]
    fn reference_filter_nested_field_with_reference_suffix() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_reference_filter(
            &mut query,
            &JsonPath::Field(&["subject", "reference"]),
            "Patient/123",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("resource->'subject'->>'reference'"),
            "expected direct text extraction for reference suffix, got: {sql}"
        );
    }

    #[test]
    fn reference_filter_nested_field_without_reference_suffix() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_reference_filter(
            &mut query,
            &JsonPath::Field(&["encounter", "serviceProvider"]),
            "Organization/1",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("->>'reference'"),
            "expected reference extraction, got: {sql}"
        );
    }

    #[test]
    fn reference_filter_where_filter() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_reference_filter(
            &mut query,
            &JsonPath::WhereFilter {
                base: &["participant"],
                filter_field: "type",
                filter_value: "ATND",
                suffix: &[],
            },
            "Practitioner/1",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("elem->>'type' = 'ATND'"),
            "expected type filter, got: {sql}"
        );
        assert!(
            sql.contains("elem->>'reference'"),
            "expected reference extraction, got: {sql}"
        );
    }

    #[test]
    fn reference_filter_exists_and_position_are_noop() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_reference_filter(&mut query, &JsonPath::Exists(&["field"]), "Patient/1");
        let sql = query.into_sql().as_str().to_owned();
        assert_eq!(sql, "SELECT 1 FROM t WHERE 1=1");
    }

    // -----------------------------------------------------------------------
    // Date filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn date_filter_single_field_uses_timestamptz_cast() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_date_filter(&mut query, &JsonPath::Field(&["birthDate"]), "1974");
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("resource->'birthDate'"),
            "expected date field extraction, got: {sql}"
        );
        assert!(
            sql.contains("::timestamptz"),
            "expected timestamptz cast for precision-aware date comparison, got: {sql}"
        );
        assert!(
            !sql.contains("LIKE"),
            "expected LIKE to be replaced with bound comparison, got: {sql}"
        );
    }

    #[test]
    fn date_filter_nested_field_uses_array_expansion() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_date_filter(
            &mut query,
            &JsonPath::Field(&["actualPeriod", "start"]),
            "2024-01",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("jsonb_array_elements"),
            "expected array expansion for nested date field, got: {sql}"
        );
        assert!(
            !sql.contains("WHERE AND"),
            "nested date predicate must form valid SQL, got: {sql}"
        );
    }

    #[test]
    fn date_filter_where_filter() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_date_filter(
            &mut query,
            &JsonPath::WhereFilter {
                base: &["event"],
                filter_field: "type",
                filter_value: "start",
                suffix: &["dateTime"],
            },
            "2024",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("elem->>'type' = 'start'"),
            "expected type filter, got: {sql}"
        );
        assert!(
            sql.contains("::timestamptz"),
            "expected precision-aware cast, got: {sql}"
        );
    }

    #[test]
    fn date_filter_uses_ge_prefix() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_date_filter(&mut query, &JsonPath::Field(&["birthDate"]), "ge2000-01-01");
        let sql = query.into_sql().as_str().to_owned();
        assert!(sql.contains(" > "), "expected > comparison, got: {sql}");
    }

    // -----------------------------------------------------------------------
    // URI filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn uri_filter_single_field_exact_match() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_uri_filter(
            &mut query,
            &JsonPath::Field(&["url"]),
            "http://example.org/fhir/ValueSet/123",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("resource->>'url'"),
            "expected url extraction, got: {sql}"
        );
        assert!(
            !sql.contains("LIKE"),
            "URI should use exact match, not LIKE, got: {sql}"
        );
    }

    #[test]
    fn uri_filter_where_filter_is_noop() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_uri_filter(
            &mut query,
            &JsonPath::WhereFilter {
                base: &["telecom"],
                filter_field: "system",
                filter_value: "url",
                suffix: &["value"],
            },
            "http://example.org",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert_eq!(
            sql, "SELECT 1 FROM t WHERE 1=1",
            "WhereFilter not supported for URI"
        );
    }

    // -----------------------------------------------------------------------
    // Number filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn number_filter_casts_to_numeric() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_number_filter(&mut query, &JsonPath::Field(&["priority"]), "5");
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("::text::numeric"),
            "expected numeric cast, got: {sql}"
        );
    }

    // -----------------------------------------------------------------------
    // Quantity filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn quantity_filter_value_only() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_quantity_filter(&mut query, &JsonPath::Field(&["valueQuantity"]), "5.4");
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("->>'value')::numeric"),
            "expected value extraction, got: {sql}"
        );
        assert!(
            !sql.contains("->>'system'"),
            "should not check system when not provided, got: {sql}"
        );
    }

    #[test]
    fn quantity_filter_full_value_system_code() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_quantity_filter(
            &mut query,
            &JsonPath::Field(&["valueQuantity"]),
            "5.4|http://unitsofmeasure.org|mg",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("->>'system'"),
            "expected system check, got: {sql}"
        );
        assert!(sql.contains("->>'code'"), "expected code check, got: {sql}");
        assert!(
            sql.contains("->>'unit'"),
            "expected unit fallback check, got: {sql}"
        );
    }

    #[test]
    fn quantity_filter_code_only_no_number() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_quantity_filter(&mut query, &JsonPath::Field(&["valueQuantity"]), "||mg");
        let sql = query.into_sql().as_str().to_owned();
        // Number part is empty, so TRUE is used
        assert!(
            sql.contains("TRUE"),
            "expected TRUE placeholder for missing number, got: {sql}"
        );
        assert!(sql.contains("->>'code'"), "expected code check, got: {sql}");
    }

    // -----------------------------------------------------------------------
    // Exists filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn exists_filter_true_checks_not_null_and_not_false() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_exists_filter(&mut query, &["deceased"], "true");
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("IS NOT NULL"),
            "expected NOT NULL check, got: {sql}"
        );
        assert!(
            sql.contains("!= 'false'::jsonb"),
            "expected false exclusion, got: {sql}"
        );
    }

    #[test]
    fn exists_filter_false_checks_null_or_false() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_exists_filter(&mut query, &["deceased"], "false");
        let sql = query.into_sql().as_str().to_owned();
        assert!(sql.contains("IS NULL"), "expected NULL check, got: {sql}");
        assert!(
            sql.contains("= 'false'::jsonb"),
            "expected false inclusion, got: {sql}"
        );
    }

    #[test]
    fn exists_filter_other_value_checks_not_null() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_exists_filter(&mut query, &["field"], "something");
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("IS NOT NULL"),
            "expected NOT NULL for other values, got: {sql}"
        );
        assert!(
            !sql.contains("'false'"),
            "should not check false for non-boolean, got: {sql}"
        );
    }

    // -----------------------------------------------------------------------
    // Token filter: identifier path
    // -----------------------------------------------------------------------

    #[test]
    fn token_identifier_uses_containment_operator() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_token_identifier(
            &mut query,
            "identifier",
            Some("http://example.org"),
            "12345",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains(
                "resource->'identifier' @> jsonb_build_array(jsonb_build_object('value', to_jsonb("
            ),
            "expected containment on identifier, got: {sql}"
        );
        assert!(
            sql.contains("'system', to_jsonb("),
            "expected system in containment, got: {sql}"
        );
    }

    #[test]
    fn token_identifier_without_system() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_token_identifier(&mut query, "identifier", None, "12345");
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("'value', to_jsonb("),
            "expected value in containment, got: {sql}"
        );
        assert!(
            !sql.contains("'system'"),
            "should not have system without system filter, got: {sql}"
        );
    }

    // -----------------------------------------------------------------------
    // Token filter: nested field
    // -----------------------------------------------------------------------

    #[test]
    fn token_nested_field_two_segments() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_token_nested_field(
            &mut query,
            &["code", "coding"],
            Some("http://loinc.org"),
            "15074-8",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("coding->>'code'"),
            "expected coding code extraction, got: {sql}"
        );
        assert!(
            sql.contains("coding->>'system'"),
            "expected coding system extraction, got: {sql}"
        );
    }

    #[test]
    fn token_nested_field_deep_nesting() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_token_nested_field(&mut query, &["a", "b", "c"], None, "val");
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("resource->'a'->'b'->>'c'"),
            "expected deep nested path, got: {sql}"
        );
    }

    // -----------------------------------------------------------------------
    // Token filter: where filter
    // -----------------------------------------------------------------------

    #[test]
    fn token_where_filter_with_suffix() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_token_where_filter(
            &mut query,
            &["telecom"],
            "system",
            "phone",
            &["value"],
            None,
            "555-1234",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("elem->>'system' = 'phone'"),
            "expected system filter, got: {sql}"
        );
        assert!(
            sql.contains("elem->>'value'"),
            "expected value extraction via suffix, got: {sql}"
        );
    }

    #[test]
    fn token_where_filter_without_suffix() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_token_where_filter(
            &mut query,
            &["telecom"],
            "system",
            "email",
            &[],
            Some("mailto"),
            "test@example.com",
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("elem->>'system' = 'email'"),
            "expected system filter, got: {sql}"
        );
        assert!(
            sql.contains("elem->>'value'"),
            "expected value extraction, got: {sql}"
        );
        assert!(
            sql.contains("elem->>'system'"),
            "expected system check in suffix, got: {sql}"
        );
    }

    // -----------------------------------------------------------------------
    // Special filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn special_filter_position_calls_near_filter() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_special_filter(
            &mut query,
            &JsonPath::Position(&["position"]),
            "42.36|-71.06|10|km",
            GeoSearchMode::EarthDistance,
        );
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("earth_distance"),
            "expected near filter, got: {sql}"
        );
    }

    #[test]
    fn special_filter_non_position_is_noop() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_special_filter(
            &mut query,
            &JsonPath::Field(&["status"]),
            "test",
            GeoSearchMode::EarthDistance,
        );
        let sql = query.into_sql().as_str().to_owned();
        assert_eq!(
            sql, "SELECT 1 FROM t WHERE 1=1",
            "non-Position path should be no-op"
        );
    }

    // -----------------------------------------------------------------------
    // push_search_filters integration test
    // -----------------------------------------------------------------------

    #[test]
    fn push_search_filters_applies_multiple_filters() {
        use super::super::registry::SearchParam;

        static PARAMS: [SearchParam; 2] = [
            SearchParam {
                code: "status",
                param_type: SearchParamType::Token,
                path: JsonPath::Field(&["status"]),
            },
            SearchParam {
                code: "subject",
                param_type: SearchParamType::Reference,
                path: JsonPath::Field(&["subject"]),
            },
        ];

        let filters = vec![
            SearchFilter {
                param: &PARAMS[0],
                modifier: None,
                values: vec!["active".to_owned()],
            },
            SearchFilter {
                param: &PARAMS[1],
                modifier: None,
                values: vec!["Patient/123".to_owned()],
            },
        ];

        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_search_filters(&mut query, &filters, GeoSearchMode::EarthDistance).unwrap();
        let sql = query.into_sql().as_str().to_owned();
        assert!(
            sql.contains("resource->>'status'"),
            "expected status filter, got: {sql}"
        );
        assert!(
            sql.contains("resource->'subject'->>'reference'"),
            "expected subject filter, got: {sql}"
        );
    }

    #[test]
    fn push_search_filters_ors_values_within_one_occurrence() {
        use super::super::registry::SearchParam;

        static PARAM: SearchParam = SearchParam {
            code: "status",
            param_type: SearchParamType::Token,
            path: JsonPath::Field(&["status"]),
        };
        let filters = vec![SearchFilter {
            param: &PARAM,
            modifier: None,
            values: vec!["active".to_owned(), "draft".to_owned()],
        }];

        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        push_search_filters(&mut query, &filters, GeoSearchMode::EarthDistance).unwrap();
        let sql = query.into_sql().as_str().to_owned();

        assert!(sql.contains("AND (FALSE OR (TRUE AND"), "{sql}");
        assert!(sql.matches("resource->>'status'").count() >= 2, "{sql}");
    }

    #[test]
    fn push_search_filters_rejects_composite_instead_of_ignoring_it() {
        use super::super::registry::SearchParam;

        static PARAMS: [SearchParam; 1] = [SearchParam {
            code: "combo",
            param_type: SearchParamType::Composite,
            path: JsonPath::Field(&["field"]),
        }];

        let filters = vec![SearchFilter {
            param: &PARAMS[0],
            modifier: None,
            values: vec!["value".to_owned()],
        }];

        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 FROM t WHERE 1=1");
        let error =
            push_search_filters(&mut query, &filters, GeoSearchMode::EarthDistance).unwrap_err();
        let sql = query.into_sql().as_str().to_owned();
        assert!(matches!(error, AppError::BadRequest(_)));
        assert_eq!(
            sql, "SELECT 1 FROM t WHERE 1=1",
            "unsupported filters must be rejected before changing the query"
        );
    }

    #[test]
    fn push_search_filters_rejects_unsupported_registry_path() {
        static PARAM: SearchParam = SearchParam {
            code: "unsafe-string-path",
            param_type: SearchParamType::String,
            path: JsonPath::Exists(&["field"]),
        };
        let filters = vec![SearchFilter {
            param: &PARAM,
            modifier: None,
            values: vec!["value".to_owned()],
        }];
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 WHERE TRUE");

        assert!(push_search_filters(&mut query, &filters, GeoSearchMode::EarthDistance).is_err());
        assert_eq!(query.into_sql().as_str(), "SELECT 1 WHERE TRUE");
    }

    #[test]
    fn injection_canaries_are_always_bind_parameters() {
        static PARAMS: [SearchParam; 3] = [
            SearchParam {
                code: "name",
                param_type: SearchParamType::String,
                path: JsonPath::Field(&["name"]),
            },
            SearchParam {
                code: "status",
                param_type: SearchParamType::Token,
                path: JsonPath::Field(&["status"]),
            },
            SearchParam {
                code: "subject",
                param_type: SearchParamType::Reference,
                path: JsonPath::Field(&["subject"]),
            },
        ];
        let payload = "x' OR TRUE; DROP TABLE fhir_resources; --";
        let filters = PARAMS
            .iter()
            .map(|param| SearchFilter {
                param,
                modifier: None,
                values: vec![payload.to_owned()],
            })
            .collect::<Vec<_>>();
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 WHERE TRUE");

        push_search_filters(&mut query, &filters, GeoSearchMode::EarthDistance).unwrap();
        let sql = query.into_sql().as_str().to_owned();

        assert!(
            !sql.contains(payload),
            "client payload leaked into SQL: {sql}"
        );
        assert!(sql.contains("$1"), "expected bind placeholders: {sql}");
        assert!(!sql.contains("DROP TABLE"), "{sql}");
    }

    #[test]
    fn date_filter_rejects_injection_payload_via_date_validation() {
        // The precision parser rejects malformed dates during validation,
        // before any SQL text is generated. SQL injection payloads cannot
        // reach the SQL builder for date parameters.
        static PARAM: SearchParam = SearchParam {
            code: "date",
            param_type: SearchParamType::Date,
            path: JsonPath::Field(&["date"]),
        };
        let payload = "x' OR TRUE; DROP TABLE fhir_resources; --";
        let filters = vec![SearchFilter {
            param: &PARAM,
            modifier: None,
            values: vec![payload.to_owned()],
        }];
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 WHERE TRUE");

        let result = push_search_filters(&mut query, &filters, GeoSearchMode::EarthDistance);
        assert!(
            result.is_err(),
            "expected date validation to reject the injection payload"
        );
        // The query must not contain the payload, nor emit any DROP TABLE.
        let sql = query.into_sql().as_str().to_owned();
        assert!(!sql.contains(payload), "{sql}");
        assert!(!sql.contains("DROP TABLE"), "{sql}");
    }

    #[test]
    fn date_filter_binds_bounds_as_parameters_not_interpolated() {
        // Even for a valid date search value, the generated bounds must
        // appear as bound parameters — never as inline SQL literals.
        static PARAM: SearchParam = SearchParam {
            code: "date",
            param_type: SearchParamType::Date,
            path: JsonPath::Field(&["date"]),
        };
        let filters = vec![SearchFilter {
            param: &PARAM,
            modifier: None,
            values: vec!["ge2020-01-01".to_owned()],
        }];
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT 1 WHERE TRUE");
        push_search_filters(&mut query, &filters, GeoSearchMode::EarthDistance).unwrap();
        let sql = query.into_sql().as_str().to_owned();

        // The literal date should not appear directly as a SQL timestamp
        // string; only Postgres bind placeholders (`$n`) should reference it.
        assert!(
            !sql.contains("'2020-01-01'"),
            "expected the date to be bound rather than interpolated: {sql}"
        );
        assert!(sql.contains("$"), "expected at least one bind placeholder");
        assert!(
            sql.contains("timestamptz"),
            "expected precision-aware timestamptz cast: {sql}"
        );
    }

    // -----------------------------------------------------------------------
    // Helper function tests
    // -----------------------------------------------------------------------

    #[test]
    fn build_jsonb_text_path_empty_segments() {
        assert_eq!(build_jsonb_text_path("resource", &[]), "resource::text");
    }

    #[test]
    fn safe_array_elements_produces_case_when() {
        let result = safe_array_elements("resource->'name'");
        assert!(
            result.contains("jsonb_typeof(resource->'name')"),
            "expected type check"
        );
        assert!(result.contains("'array'"), "expected array check");
        assert!(
            result.contains("jsonb_build_array"),
            "expected wrap fallback"
        );
        assert!(
            result.contains("'[]'::jsonb"),
            "expected empty array fallback"
        );
    }
}
