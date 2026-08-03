use serde_json::{Value, json};
use url::form_urlencoded::Serializer;

use crate::{
    MAX_SEARCH_OR_VALUES_PER_OCCURRENCE, MAX_SEARCH_PARAMETER_OCCURRENCES, MAX_SEARCH_QUERY_BYTES,
    MAX_SEARCH_TOTAL_VALUES, SearchConfig,
    error::AppError,
    search_params,
    store::{HistoricalResource, StoredResource},
};

#[derive(Debug)]
pub(crate) struct ParsedSearchParams {
    pub(crate) count: u32,
    pub(crate) after_id: Option<String>,
    pub(crate) filters: Vec<search_params::SearchFilter>,
    pub(crate) canonical_filters: Vec<(String, String)>,
}

pub(crate) struct SearchPage<'a> {
    pub(crate) count: u32,
    pub(crate) after_id: Option<&'a str>,
    pub(crate) next_after_id: Option<&'a str>,
}

pub(crate) fn build_search_bundle(
    base_url: &str,
    resource_type: &str,
    page: SearchPage<'_>,
    total: i64,
    resources: Vec<StoredResource>,
    filters: &[(String, String)],
) -> Value {
    let base_url = base_url.trim_end_matches('/');
    let search_url = build_search_url(base_url, resource_type, page.count, page.after_id, filters);

    let mut links = vec![json!({
        "relation": "self",
        "url": search_url,
    })];

    if let Some(next_after_id) = page.next_after_id {
        links.push(json!({
            "relation": "next",
            "url": build_search_url(base_url, resource_type, page.count, Some(next_after_id), filters),
        }));
    }

    let entry = resources
        .into_iter()
        .map(|stored| {
            json!({
                "fullUrl": format!("{base_url}/{resource_type}/{}", stored.id),
                "resource": stored.resource,
                "search": {
                    "mode": "match"
                }
            })
        })
        .collect::<Vec<_>>();

    json!({
        "resourceType": "Bundle",
        "type": "searchset",
        "total": total,
        "link": links,
        "entry": entry,
    })
}

pub(crate) fn build_history_bundle(
    base_url: &str,
    resource_type: &str,
    id: &str,
    history: Vec<HistoricalResource>,
) -> Value {
    let base_url = base_url.trim_end_matches('/');
    let self_url = format!("{base_url}/{resource_type}/{id}/_history");
    let total = history.len() as i64;

    let entry = history
        .into_iter()
        .map(|version| {
            json!({
                "fullUrl": format!("{base_url}/{resource_type}/{}/_history/{}", version.id, version.version_id),
                "resource": version.resource,
                "request": {
                    "method": if version.deleted { "DELETE" } else { "PUT" },
                    "url": format!("{resource_type}/{}", version.id),
                },
                "response": {
                    "status": if version.deleted { "410 Gone" } else { "200 OK" },
                    "etag": format!("W/\"{}\"", version.version_id),
                    "lastModified": version.last_updated.to_rfc3339(),
                }
            })
        })
        .collect::<Vec<_>>();

    json!({
        "resourceType": "Bundle",
        "type": "history",
        "total": total,
        "link": [{
            "relation": "self",
            "url": self_url,
        }],
        "entry": entry,
    })
}

pub(crate) fn parse_search_params(
    resource_type: &str,
    query: Vec<(String, String)>,
    search: SearchConfig,
) -> Result<ParsedSearchParams, AppError> {
    if query.len() > MAX_SEARCH_PARAMETER_OCCURRENCES {
        return Err(AppError::BadRequest(format!(
            "search contains too many parameter occurrences; maximum is {MAX_SEARCH_PARAMETER_OCCURRENCES}"
        )));
    }
    let query_bytes = query
        .iter()
        .map(|(key, value)| key.len().saturating_add(value.len()))
        .sum::<usize>();
    if query_bytes > MAX_SEARCH_QUERY_BYTES {
        return Err(AppError::BadRequest(format!(
            "decoded search query is too large; maximum is {MAX_SEARCH_QUERY_BYTES} bytes"
        )));
    }

    let mut count = search.default_count;
    let mut after_id = None;
    let mut filters = Vec::new();
    let mut canonical_filters = Vec::new();
    let mut saw_count = false;
    let mut saw_after_id = false;
    let mut total_values = 0_usize;

    // Look up the search parameters supported for this resource type
    let supported_params = search_params::search_params_for(resource_type);

    for (key, value) in query {
        match key.as_str() {
            "_count" => {
                if saw_count {
                    return Err(AppError::BadRequest(
                        "_count must not be repeated".to_owned(),
                    ));
                }
                saw_count = true;
                count = parse_u32_query_param("_count", &value)?;
                if count > search.max_count {
                    return Err(AppError::BadRequest(format!(
                        "_count must be less than or equal to {}",
                        search.max_count
                    )));
                }
            }
            "_after_id" => {
                if saw_after_id {
                    return Err(AppError::BadRequest(
                        "_after_id must not be repeated".to_owned(),
                    ));
                }
                saw_after_id = true;
                if value.is_empty() {
                    return Err(AppError::BadRequest(
                        "_after_id must not be empty".to_owned(),
                    ));
                }
                after_id = Some(value);
            }
            "_offset" => {
                return Err(AppError::BadRequest(
                    "_offset is no longer supported; use _after_id cursor pagination".to_owned(),
                ));
            }
            param_code => {
                // Look up the parameter in the registry
                if let Some(param) = supported_params.iter().find(|p| p.code == param_code) {
                    let values = split_fhir_or_values(&value).map_err(|message| {
                        AppError::BadRequest(format!(
                            "invalid value for search parameter '{param_code}': {message}"
                        ))
                    })?;

                    if values.len() > MAX_SEARCH_OR_VALUES_PER_OCCURRENCE {
                        return Err(AppError::BadRequest(format!(
                            "search parameter '{param_code}' contains too many OR values; maximum is {MAX_SEARCH_OR_VALUES_PER_OCCURRENCE}"
                        )));
                    }
                    total_values = total_values.saturating_add(values.len());
                    if total_values > MAX_SEARCH_TOTAL_VALUES {
                        return Err(AppError::BadRequest(format!(
                            "search contains too many filter values; maximum is {MAX_SEARCH_TOTAL_VALUES}"
                        )));
                    }

                    if values.len() > 1
                        && !matches!(
                            param.param_type,
                            search_params::SearchParamType::Token
                                | search_params::SearchParamType::String
                                | search_params::SearchParamType::Date
                                | search_params::SearchParamType::Reference
                        )
                    {
                        return Err(AppError::BadRequest(format!(
                            "comma-separated OR values are not supported for search parameter '{param_code}'"
                        )));
                    }

                    search_params::sql::validate_search_filter(param, &values)?;

                    // Validate token-type parameters with pipe syntax
                    if param.param_type == search_params::SearchParamType::Token
                        && param_code == "identifier"
                    {
                        // Special validation for identifier tokens
                        for value in &values {
                            validate_identifier_value(value)?;
                        }
                    }

                    filters.push(search_params::SearchFilter { param, values });
                    canonical_filters.push((key, value));
                } else {
                    return Err(AppError::BadRequest(format!(
                        "unsupported search parameter '{param_code}' for resource type '{resource_type}'"
                    )));
                }
            }
        }
    }

    Ok(ParsedSearchParams {
        count,
        after_id,
        filters,
        canonical_filters,
    })
}

fn validate_identifier_value(value: &str) -> Result<(), AppError> {
    if value.is_empty() {
        return Err(AppError::BadRequest(
            "identifier must be 'value' or 'system|value'".to_owned(),
        ));
    }
    let components = split_fhir_delimiter(value, '|').map_err(AppError::BadRequest)?;
    if components.len() > 2
        || (components.len() == 2 && (components[0].is_empty() || components[1].is_empty()))
    {
        return Err(AppError::BadRequest(
            "identifier must be 'value' or 'system|value'".to_owned(),
        ));
    }
    Ok(())
}

/// Decode an `application/x-www-form-urlencoded` query exactly once while
/// retaining every occurrence and its original order.
pub(crate) fn parse_query_pairs(query_string: &str) -> Vec<(String, String)> {
    url::form_urlencoded::parse(query_string.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

/// Split on an unescaped FHIR delimiter and unescape the resulting values.
/// Escapes for other delimiters are retained for a later, contextual split.
pub(crate) fn split_fhir_delimiter(value: &str, delimiter: char) -> Result<Vec<String>, String> {
    let mut parts = vec![String::new()];
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let escaped = chars
                .next()
                .ok_or_else(|| "a trailing backslash is not a valid FHIR escape".to_owned())?;
            if !matches!(escaped, '$' | ',' | '|' | '\\') {
                return Err(format!("unsupported FHIR escape '\\{escaped}'"));
            }
            if escaped == delimiter {
                parts.last_mut().expect("initial part").push(escaped);
            } else {
                parts.last_mut().expect("initial part").push('\\');
                parts.last_mut().expect("initial part").push(escaped);
            }
        } else if ch == delimiter {
            parts.push(String::new());
        } else {
            parts.last_mut().expect("initial part").push(ch);
        }
    }

    Ok(parts)
}

pub(crate) fn unescape_fhir_value(value: &str) -> String {
    let mut unescaped = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(escaped) = chars.next() {
                unescaped.push(escaped);
            }
        } else {
            unescaped.push(ch);
        }
    }
    unescaped
}

fn split_fhir_or_values(value: &str) -> Result<Vec<String>, String> {
    let values = split_fhir_delimiter(value, ',')?;
    if values.iter().any(String::is_empty) {
        return Err("comma-separated values must not be empty".to_owned());
    }
    Ok(values)
}

fn parse_u32_query_param(name: &str, value: &str) -> Result<u32, AppError> {
    value
        .parse::<u32>()
        .map_err(|_| AppError::BadRequest(format!("{name} must be an unsigned integer")))
}

fn build_search_url(
    base_url: &str,
    resource_type: &str,
    count: u32,
    after_id: Option<&str>,
    filters: &[(String, String)],
) -> String {
    let mut serializer = Serializer::new(String::new());
    serializer.append_pair("_count", &count.to_string());

    if let Some(after_id) = after_id {
        serializer.append_pair("_after_id", after_id);
    }

    for (key, value) in filters {
        serializer.append_pair(key, value);
    }

    format!("{base_url}/{resource_type}?{}", serializer.finish())
}

/// Parse the If-None-Exist query string into search filters.
pub(crate) fn parse_if_none_exist_query(
    resource_type: &str,
    query_string: &str,
    search: SearchConfig,
) -> Result<ParsedSearchParams, AppError> {
    let query = parse_query_pairs(query_string);

    if query.is_empty() {
        return Err(AppError::BadRequest(
            "If-None-Exist must contain at least one search parameter".to_owned(),
        ));
    }

    parse_search_params(resource_type, query, search)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::{
        SearchPage, build_history_bundle, build_search_bundle, parse_query_pairs,
        parse_search_params, split_fhir_delimiter, validate_identifier_value,
    };
    use crate::{
        MAX_SEARCH_OR_VALUES_PER_OCCURRENCE, MAX_SEARCH_PARAMETER_OCCURRENCES,
        MAX_SEARCH_QUERY_BYTES, SearchConfig,
        error::AppError,
        store::{HistoricalResource, StoredResource},
    };

    #[test]
    fn search_bundle_contains_self_and_next_links() {
        let bundle = build_search_bundle(
            "http://localhost:8080/fhir",
            "Patient",
            SearchPage {
                count: 1,
                after_id: None,
                next_after_id: Some("example"),
            },
            2,
            vec![StoredResource {
                id: "example".to_owned(),
                version_id: 1,
                last_updated: Utc::now(),
                resource: json!({
                    "resourceType": "Patient",
                    "id": "example"
                }),
            }],
            &[],
        );

        assert_eq!(bundle["resourceType"], "Bundle");
        assert_eq!(bundle["type"], "searchset");
        assert_eq!(bundle["total"], 2);
        assert_eq!(bundle["link"][0]["relation"], "self");
        assert_eq!(bundle["link"][1]["relation"], "next");
        assert_eq!(
            bundle["link"][1]["url"],
            "http://localhost:8080/fhir/Patient?_count=1&_after_id=example"
        );
        assert_eq!(bundle["entry"][0]["resource"]["id"], "example");
    }

    #[test]
    fn search_bundle_links_preserve_repeated_filter_occurrences() {
        let filters = vec![
            ("given".to_owned(), "Alice,Marie".to_owned()),
            ("given".to_owned(), "Anne".to_owned()),
        ];
        let bundle = build_search_bundle(
            "http://localhost:8080/fhir",
            "Patient",
            SearchPage {
                count: 10,
                after_id: None,
                next_after_id: Some("patient-10"),
            },
            11,
            vec![],
            &filters,
        );

        assert_eq!(
            bundle["link"][1]["url"],
            "http://localhost:8080/fhir/Patient?_count=10&_after_id=patient-10&given=Alice%2CMarie&given=Anne"
        );
    }

    #[test]
    fn history_bundle_contains_versions_including_deletes() {
        let now = Utc::now();
        let bundle = build_history_bundle(
            "http://localhost:8080/fhir",
            "Patient",
            "example",
            vec![
                HistoricalResource {
                    id: "example".to_owned(),
                    version_id: 2,
                    last_updated: now,
                    deleted: true,
                    resource: json!({
                        "resourceType": "Patient",
                        "id": "example"
                    }),
                },
                HistoricalResource {
                    id: "example".to_owned(),
                    version_id: 1,
                    last_updated: now,
                    deleted: false,
                    resource: json!({
                        "resourceType": "Patient",
                        "id": "example"
                    }),
                },
            ],
        );

        assert_eq!(bundle["resourceType"], "Bundle");
        assert_eq!(bundle["type"], "history");
        assert_eq!(bundle["total"], 2);
        assert_eq!(bundle["link"][0]["relation"], "self");
        assert_eq!(bundle["entry"][0]["response"]["status"], "410 Gone");
        assert_eq!(bundle["entry"][0]["response"]["etag"], "W/\"2\"");
        assert_eq!(bundle["entry"][1]["response"]["status"], "200 OK");
    }

    #[test]
    fn parse_search_params_builds_patient_filters() {
        let params = parse_search_params(
            "Patient",
            vec![
                ("_count".to_owned(), "5".to_owned()),
                ("name".to_owned(), "peter".to_owned()),
                (
                    "identifier".to_owned(),
                    "urn:oid:1.2.36.146.595.217.0.1|12345".to_owned(),
                ),
            ],
            SearchConfig {
                default_count: 50,
                max_count: 500,
            },
        )
        .unwrap();

        assert_eq!(params.count, 5);
        assert_eq!(params.after_id, None);
        assert_eq!(params.filters.len(), 2);
        assert_eq!(params.canonical_filters.len(), 2);
    }

    #[test]
    fn parse_search_params_accepts_after_id_cursor() {
        let params = parse_search_params(
            "Patient",
            vec![("_after_id".to_owned(), "patient-123".to_owned())],
            SearchConfig {
                default_count: 50,
                max_count: 500,
            },
        )
        .unwrap();

        assert_eq!(params.count, 50);
        assert_eq!(params.after_id.as_deref(), Some("patient-123"));
    }

    #[test]
    fn parse_search_params_rejects_unknown_resource_search_parameter() {
        let error = parse_search_params(
            "Patient",
            vec![("status".to_owned(), "final".to_owned())],
            SearchConfig {
                default_count: 50,
                max_count: 500,
            },
        )
        .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn validate_identifier_value_accepts_value_or_system_value() {
        validate_identifier_value("12345").unwrap();
        validate_identifier_value("urn:test|12345").unwrap();
    }

    #[test]
    fn validate_identifier_value_rejects_empty() {
        validate_identifier_value("").unwrap_err();
        validate_identifier_value("|").unwrap_err();
        validate_identifier_value("|12345").unwrap_err();
        validate_identifier_value("urn:test|").unwrap_err();
    }

    #[test]
    fn query_pairs_preserve_repeated_parameters() {
        assert_eq!(
            parse_query_pairs("given=Alice&given=Marie"),
            vec![
                ("given".to_owned(), "Alice".to_owned()),
                ("given".to_owned(), "Marie".to_owned()),
            ]
        );
    }

    #[test]
    fn query_values_are_url_decoded_once_before_fhir_parsing() {
        let params = parse_search_params(
            "Patient",
            parse_query_pairs("given=%252C"),
            SearchConfig {
                default_count: 50,
                max_count: 500,
            },
        )
        .unwrap();

        assert_eq!(params.filters[0].values, ["%2C"]);
    }

    #[test]
    fn parse_search_params_preserves_repeated_and_splits_or_values() {
        let params = parse_search_params(
            "Patient",
            parse_query_pairs("given=Alice%2CMarie&given=Anne"),
            SearchConfig {
                default_count: 50,
                max_count: 500,
            },
        )
        .unwrap();

        assert_eq!(params.filters.len(), 2);
        assert_eq!(params.filters[0].values, ["Alice", "Marie"]);
        assert_eq!(params.filters[1].values, ["Anne"]);
        assert_eq!(params.canonical_filters.len(), 2);
    }

    #[test]
    fn fhir_delimiters_honor_escaping() {
        assert_eq!(
            split_fhir_delimiter(r"one\,two,three\|four", ',').unwrap(),
            ["one,two", r"three\|four"]
        );
        assert_eq!(
            split_fhir_delimiter(r"system\|name|code", '|').unwrap(),
            ["system|name", "code"]
        );
        assert_eq!(
            super::unescape_fhir_value(r"dollar\$comma,pipe|slash\\"),
            r"dollar$comma,pipe|slash\"
        );
    }

    #[test]
    fn token_string_date_and_reference_accept_and_or_terms() {
        for (resource_type, query) in [
            ("Patient", "identifier=a,b&identifier=c"),
            ("Patient", "given=Alice,Marie&given=Anne"),
            ("Patient", "birthdate=1980,1990&birthdate=2000"),
            (
                "Patient",
                "general-practitioner=Practitioner%2F1,Practitioner%2F2&general-practitioner=Practitioner%2F3",
            ),
        ] {
            let params = parse_search_params(
                resource_type,
                parse_query_pairs(query),
                SearchConfig {
                    default_count: 50,
                    max_count: 500,
                },
            )
            .unwrap_or_else(|error| panic!("{query} should parse: {error:?}"));

            assert_eq!(params.filters.len(), 2, "{query}");
            assert_eq!(params.filters[0].values.len(), 2, "{query}");
            assert_eq!(params.filters[1].values.len(), 1, "{query}");
        }
    }

    #[test]
    fn parse_search_params_rejects_repeated_control_parameters() {
        let error = parse_search_params(
            "Patient",
            parse_query_pairs("_count=1&_count=2"),
            SearchConfig {
                default_count: 50,
                max_count: 500,
            },
        )
        .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn parse_search_params_rejects_unsupported_or_combinations() {
        let error = parse_search_params(
            "Observation",
            parse_query_pairs("value-quantity=1,2"),
            SearchConfig {
                default_count: 50,
                max_count: 500,
            },
        )
        .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn parse_search_params_enforces_complexity_limits() {
        let config = SearchConfig {
            default_count: 50,
            max_count: 500,
        };

        let too_many_occurrences = (0..=MAX_SEARCH_PARAMETER_OCCURRENCES)
            .map(|_| ("given".to_owned(), "Alice".to_owned()))
            .collect();
        assert!(parse_search_params("Patient", too_many_occurrences, config).is_err());

        let too_many_or_values =
            std::iter::repeat_n("Alice", MAX_SEARCH_OR_VALUES_PER_OCCURRENCE + 1)
                .collect::<Vec<_>>()
                .join(",");
        assert!(
            parse_search_params(
                "Patient",
                vec![("given".to_owned(), too_many_or_values)],
                config,
            )
            .is_err()
        );

        let too_large = "A".repeat(MAX_SEARCH_QUERY_BYTES + 1);
        assert!(
            parse_search_params("Patient", vec![("given".to_owned(), too_large)], config,).is_err()
        );
    }

    #[test]
    fn parse_search_params_rejects_unimplemented_comparators() {
        let error = parse_search_params(
            "Patient",
            parse_query_pairs("birthdate=ge2000-01-01"),
            SearchConfig {
                default_count: 50,
                max_count: 500,
            },
        )
        .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }
}
