use serde_json::{Value, json};

use crate::search_params::{RESOURCE_TYPES, SearchParamType, search_params_for};

pub fn capability_statement(base_url: &str) -> Value {
    let generic_interactions = json!([
        {"code": "create"},
        {"code": "read"},
        {"code": "history-instance"},
        {"code": "update"},
        {"code": "patch"},
        {"code": "delete"},
        {"code": "search-type"}
    ]);

    let pagination_params = vec![
        json!({
            "name": "_count",
            "type": "number",
            "documentation": "Limits the number of resources returned per page."
        }),
        json!({
            "name": "_after_id",
            "type": "string",
            "documentation": "Returns resources that sort after the supplied resource id cursor."
        }),
    ];

    // Build resource entries dynamically from the search parameter registry
    let mut resource_entries: Vec<Value> = Vec::with_capacity(RESOURCE_TYPES.len() + 1);

    // Wildcard entry with shared pagination params
    resource_entries.push(json!({
        "type": "*",
        "interaction": generic_interactions,
        "conditionalCreate": true,
        "updateCreate": true,
        "searchParam": pagination_params,
    }));

    for &rt in RESOURCE_TYPES {
        let params = search_params_for(rt);
        if params.is_empty() {
            // Still list the resource type even if it has no search params
            resource_entries.push(json!({
                "type": rt,
                "interaction": generic_interactions,
                "conditionalCreate": true,
                "updateCreate": true,
            }));
            continue;
        }

        let mut search_params: Vec<Value> = pagination_params.clone();
        for sp in params {
            let type_str = match sp.param_type {
                SearchParamType::String => "string",
                SearchParamType::Token => "token",
                SearchParamType::Reference => "reference",
                SearchParamType::Date => "date",
                SearchParamType::Quantity => "quantity",
                SearchParamType::Number => "number",
                SearchParamType::Uri => "uri",
                SearchParamType::Composite => "composite",
                SearchParamType::Special => "special",
            };
            search_params.push(json!({
                "name": sp.code,
                "type": type_str,
            }));
        }

        resource_entries.push(json!({
            "type": rt,
            "interaction": generic_interactions,
            "conditionalCreate": true,
            "updateCreate": true,
            "searchParam": search_params,
        }));
    }

    let version = env!("CARGO_PKG_VERSION");

    json!({
        "resourceType": "CapabilityStatement",
        "url": format!("{base_url}/metadata"),
        "version": version,
        "name": "NisseFHIR",
        "title": "NisseFHIR – Lightweight FHIR R6 Server",
        "status": "active",
        "date": "2026-03-10",
        "publisher": "SINTEF / Invest4Health",
        "description": "A lightweight, stateless FHIR R6 server written in Rust. Supports JSON-only with PostgreSQL JSONB storage, JWT-based multi-tenant authentication, and comprehensive FHIR CRUD with search.",
        "kind": "instance",
        "software": {
            "name": "NisseFHIR",
            "version": version,
            "releaseDate": "2026-03-10"
        },
        "fhirVersion": "6.0.0-ballot3",
        "format": ["json", "application/fhir+json"],
        "patchFormat": ["application/json-patch+json"],
        "rest": [{
            "mode": "server",
            "security": {
                "cors": true,
                "service": [
                    {
                        "coding": [
                            {
                                "system": "http://terminology.hl7.org/CodeSystem/restful-security-service",
                                "code": "SMART-on-FHIR",
                                "display": "SMART on FHIR"
                            }
                        ],
                        "text": "JWT Bearer Token authentication. Tokens must include tenant or sub, scope (read/write), and optionally resource_types claims."
                    }
                ],
                "description": "This server supports JWT Bearer Token authentication with static keys or a JWKS provider. Tokens encode tenant identity, read/write scopes, and optional resource type restrictions."
            },
            "resource": resource_entries,
            "interaction": [
                {"code": "transaction"},
                {"code": "batch"}
            ]
        }],
        "implementation": {
            "description": "Lightweight Rust FHIR server",
            "url": base_url
        }
    })
}

#[cfg(test)]
mod tests {
    use super::capability_statement;

    #[test]
    fn capability_has_fhir_resource_type() {
        let value = capability_statement("http://localhost:8080/fhir");
        assert_eq!(value["resourceType"], "CapabilityStatement");
        assert_eq!(value["format"][0], "json");
    }

    #[test]
    fn capability_has_correct_fhir_version() {
        let value = capability_statement("http://localhost:8080/fhir");
        assert_eq!(value["fhirVersion"], "6.0.0-ballot3");
    }

    #[test]
    fn capability_has_active_status() {
        let value = capability_statement("http://localhost:8080/fhir");
        assert_eq!(value["status"], "active");
        assert_eq!(value["kind"], "instance");
    }

    #[test]
    fn capability_has_rest_server_mode() {
        let value = capability_statement("http://localhost:8080/fhir");
        assert_eq!(value["rest"][0]["mode"], "server");
    }

    #[test]
    fn capability_lists_supported_interactions() {
        let value = capability_statement("http://localhost:8080/fhir");
        let interactions = &value["rest"][0]["resource"][0]["interaction"];
        let codes: Vec<&str> = interactions
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["code"].as_str().unwrap())
            .collect();
        assert!(codes.contains(&"create"));
        assert!(codes.contains(&"read"));
        assert!(codes.contains(&"history-instance"));
        assert!(codes.contains(&"update"));
        assert!(codes.contains(&"patch"));
        assert!(codes.contains(&"delete"));
        assert!(codes.contains(&"search-type"));
    }

    #[test]
    fn capability_lists_search_parameters() {
        let value = capability_statement("http://localhost:8080/fhir");
        let search_params = value["rest"][0]["resource"][0]["searchParam"]
            .as_array()
            .unwrap();
        let names: Vec<&str> = search_params
            .iter()
            .map(|param| param["name"].as_str().unwrap())
            .collect();

        assert!(names.contains(&"_count"));
        assert!(names.contains(&"_after_id"));
    }

    #[test]
    fn capability_lists_patient_and_observation_search_parameters() {
        let value = capability_statement("http://localhost:8080/fhir");
        let resources = value["rest"][0]["resource"].as_array().unwrap();

        let patient = resources
            .iter()
            .find(|resource| resource["type"] == "Patient")
            .unwrap();
        let observation = resources
            .iter()
            .find(|resource| resource["type"] == "Observation")
            .unwrap();

        let patient_params: Vec<&str> = patient["searchParam"]
            .as_array()
            .unwrap()
            .iter()
            .map(|param| param["name"].as_str().unwrap())
            .collect();
        let observation_params: Vec<&str> = observation["searchParam"]
            .as_array()
            .unwrap()
            .iter()
            .map(|param| param["name"].as_str().unwrap())
            .collect();

        assert!(patient_params.contains(&"name"));
        assert!(patient_params.contains(&"birthdate"));
        assert!(patient_params.contains(&"identifier"));
        assert!(observation_params.contains(&"code"));
        assert!(observation_params.contains(&"status"));
        assert!(observation_params.contains(&"subject"));
    }

    #[test]
    fn capability_includes_implementation_url() {
        let value = capability_statement("https://fhir.example.com/api");
        assert_eq!(
            value["implementation"]["url"],
            "https://fhir.example.com/api"
        );
        assert_eq!(
            value["implementation"]["description"],
            "Lightweight Rust FHIR server"
        );
    }

    #[test]
    fn capability_includes_software_version() {
        let value = capability_statement("http://localhost:8080/fhir");
        assert_eq!(value["software"]["name"], "NisseFHIR");
        assert!(!value["software"]["version"].as_str().unwrap().is_empty());
        assert_eq!(value["name"], "NisseFHIR");
        assert!(value["version"].as_str().unwrap().starts_with("0."));
    }

    #[test]
    fn capability_only_supports_json_and_fhir_json() {
        let value = capability_statement("http://localhost:8080/fhir");
        let formats = value["format"].as_array().unwrap();
        assert_eq!(formats.len(), 2);
        assert_eq!(formats[0], "json");
        assert_eq!(formats[1], "application/fhir+json");
    }

    #[test]
    fn capability_advertises_conditional_create() {
        let value = capability_statement("http://localhost:8080/fhir");
        let resources = value["rest"][0]["resource"].as_array().unwrap();
        // Wildcard entry
        assert_eq!(resources[0]["conditionalCreate"], true);
        // A specific resource type
        let patient = resources.iter().find(|r| r["type"] == "Patient").unwrap();
        assert_eq!(patient["conditionalCreate"], true);
    }

    #[test]
    fn capability_advertises_update_create() {
        let value = capability_statement("http://localhost:8080/fhir");
        let resources = value["rest"][0]["resource"].as_array().unwrap();
        assert_eq!(resources[0]["updateCreate"], true);
        let patient = resources.iter().find(|r| r["type"] == "Patient").unwrap();
        assert_eq!(patient["updateCreate"], true);
    }
}
