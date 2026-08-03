use std::collections::BTreeMap;

use serde_json::{Value, json};
use url::form_urlencoded::Serializer;

use crate::{
    SearchConfig,
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
    query: BTreeMap<String, String>,
    search: SearchConfig,
) -> Result<ParsedSearchParams, AppError> {
    let mut count = search.default_count;
    let mut after_id = None;
    let mut filters = Vec::new();
    let mut canonical_filters = Vec::new();

    // Look up the search parameters supported for this resource type
    let supported_params = search_params::search_params_for(resource_type);

    for (key, value) in query {
        match key.as_str() {
            "_count" => {
                count = parse_u32_query_param("_count", &value)?;
                if count > search.max_count {
                    return Err(AppError::BadRequest(format!(
                        "_count must be less than or equal to {}",
                        search.max_count
                    )));
                }
            }
            "_after_id" => {
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
                    // Validate token-type parameters with pipe syntax
                    if param.param_type == search_params::SearchParamType::Token
                        && param_code == "identifier"
                    {
                        // Special validation for identifier tokens
                        validate_identifier_value(&value)?;
                    }

                    filters.push(search_params::SearchFilter {
                        param,
                        value: value.clone(),
                    });
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
    if let Some((system, id_value)) = value.split_once('|')
        && (system.is_empty() || id_value.is_empty())
    {
        return Err(AppError::BadRequest(
            "identifier must be 'value' or 'system|value'".to_owned(),
        ));
    }
    Ok(())
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
    let query: BTreeMap<String, String> = url::form_urlencoded::parse(query_string.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if query.is_empty() {
        return Err(AppError::BadRequest(
            "If-None-Exist must contain at least one search parameter".to_owned(),
        ));
    }

    parse_search_params(resource_type, query, search)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::Utc;
    use serde_json::json;

    use super::{
        SearchPage, build_history_bundle, build_search_bundle, parse_search_params,
        validate_identifier_value,
    };
    use crate::{
        SearchConfig,
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
            BTreeMap::from([
                ("_count".to_owned(), "5".to_owned()),
                ("name".to_owned(), "peter".to_owned()),
                (
                    "identifier".to_owned(),
                    "urn:oid:1.2.36.146.595.217.0.1|12345".to_owned(),
                ),
            ]),
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
            BTreeMap::from([("_after_id".to_owned(), "patient-123".to_owned())]),
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
            BTreeMap::from([("status".to_owned(), "final".to_owned())]),
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
}
