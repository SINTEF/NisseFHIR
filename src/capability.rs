use serde_json::{Value, json};

use crate::search_params::{RESOURCE_TYPES, SearchParamType, search_params_for};

/// Stable canonical identifier for the NisseFHIR CapabilityStatement.
///
/// This identifies the capability definition. It is deliberately independent
/// of the deployment-specific URL from which `/fhir/metadata` is retrieved.
pub const CAPABILITY_STATEMENT_CANONICAL_URL: &str =
    "https://sintef.github.io/NisseFHIR/CapabilityStatement/nissefhir";

pub fn capability_statement(base_url: &str, cors_enabled: bool) -> Value {
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

    // Build concrete resource entries dynamically from the schema-derived
    // resource list and the executable search parameter registry.
    let mut resource_entries: Vec<Value> = Vec::with_capacity(RESOURCE_TYPES.len());

    for &rt in RESOURCE_TYPES {
        let mut search_params: Vec<Value> = pagination_params.clone();
        let executable_params = search_params_for(rt)
            .iter()
            .filter(|sp| crate::search_params::sql::is_executable_search_param(sp))
            .collect::<Vec<_>>();
        for sp in &executable_params {
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
            "conditionalCreate": !executable_params.is_empty(),
            "updateCreate": true,
            "searchParam": search_params,
        }));
    }
    resource_entries.push(json!({
        "type": "AuditEvent",
        "interaction": [{"code":"read"}, {"code":"search-type"}],
        "searchParam": [
            {"name":"_id","type":"token"}, {"name":"action","type":"token"},
            {"name":"code","type":"token"}, {"name":"outcome","type":"token"},
            {"name":"agent","type":"reference"}, {"name":"entity","type":"reference"},
            {"name":"_count","type":"number"}, {"name":"_after_id","type":"string"}
        ]
    }));

    let version = env!("CARGO_PKG_VERSION");

    json!({
        "resourceType": "CapabilityStatement",
        "url": CAPABILITY_STATEMENT_CANONICAL_URL,
        "version": version,
        "name": "NisseFHIR",
        "title": "NisseFHIR – Lightweight FHIR R6 Server",
        "status": "active",
        "publisher": "SINTEF / Invest4Health",
        "description": "A lightweight, stateless FHIR R6 server written in Rust. Supports JSON-only with PostgreSQL JSONB storage, JWT-based multi-tenant authentication, and comprehensive FHIR CRUD with search.",
        "kind": "instance",
        "software": {
            "name": "NisseFHIR",
            "version": version
        },
        "fhirVersion": "6.0.0-ballot3",
        "format": ["json", "application/fhir+json"],
        "patchFormat": ["application/json-patch+json"],
        "rest": [{
            "mode": "server",
            "security": {
                "cors": cors_enabled,
                "service": [
                    {
                        "text": "JWT Bearer token authentication"
                    }
                ],
                "description": "JWTs are verified with a configured static key or JWKS provider. A tenant claim selects the tenant; without one, sub is used. A subject claim is always required. The scope claim recognizes whitespace-separated read, write, and NisseFHIR-specific auditlog tokens; auditlog alone grants tenant-scoped access only to the read-only server AuditEvent API. missing, empty, or unrecognized scopes grant no permission. Bundle entries are authorized individually according to their HTTP interaction. An optional resource_types claim restricts clinical resource types. SMART discovery, launch contexts, and SMART clinical scopes are not implemented."
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
    use std::collections::BTreeSet;

    use super::{CAPABILITY_STATEMENT_CANONICAL_URL, capability_statement as build_statement};

    fn capability_statement(base_url: &str) -> serde_json::Value {
        build_statement(base_url, false)
    }

    #[test]
    fn capability_has_fhir_resource_type() {
        let value = capability_statement("http://localhost:8080/fhir");
        assert_eq!(value["resourceType"], "CapabilityStatement");
        assert_eq!(value["format"][0], "json");
    }

    #[test]
    fn capability_is_valid_fhir_json() {
        let validator = crate::validation::FhirSchemaValidator::new().unwrap();
        let value = capability_statement("http://localhost:8080/fhir");

        validator
            .validate_resource("CapabilityStatement", &value)
            .unwrap();
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
    fn capability_advertises_only_executable_registry_entries() {
        let value = capability_statement("http://localhost:8080/fhir");
        let resources = value["rest"][0]["resource"].as_array().unwrap();

        assert_eq!(
            resources.len(),
            crate::search_params::RESOURCE_TYPES.len() + 1
        );
        assert!(resources.iter().all(|resource| resource["type"] != "*"));

        for resource in resources
            .iter()
            .filter(|resource| resource["type"] != "AuditEvent")
        {
            let resource_type = resource["type"].as_str().unwrap();
            let advertised = resource["searchParam"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|param| param["name"].as_str())
                .filter(|name| !name.starts_with('_'))
                .collect::<BTreeSet<_>>();
            let executable = crate::search_params::search_params_for(resource_type)
                .iter()
                .filter(|param| crate::search_params::sql::is_executable_search_param(param))
                .map(|param| param.code)
                .collect::<BTreeSet<_>>();

            assert_eq!(
                advertised, executable,
                "registry mismatch for {resource_type}"
            );
            assert_eq!(resource["conditionalCreate"], !executable.is_empty());
        }
    }

    #[test]
    fn capability_uses_a_stable_canonical_url() {
        let first = capability_statement("https://one.example/fhir");
        let second = capability_statement("https://two.example/fhir");

        assert_eq!(first["url"], CAPABILITY_STATEMENT_CANONICAL_URL);
        assert_eq!(second["url"], CAPABILITY_STATEMENT_CANONICAL_URL);
        assert_ne!(first["url"], first["implementation"]["url"]);
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
        assert!(value.get("date").is_none());
        assert!(value["software"].get("releaseDate").is_none());
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
        let patient = resources.iter().find(|r| r["type"] == "Patient").unwrap();
        assert_eq!(patient["conditionalCreate"], true);
    }

    #[test]
    fn capability_advertises_update_create() {
        let value = capability_statement("http://localhost:8080/fhir");
        let resources = value["rest"][0]["resource"].as_array().unwrap();
        let patient = resources.iter().find(|r| r["type"] == "Patient").unwrap();
        assert_eq!(patient["updateCreate"], true);
    }

    #[test]
    fn capability_describes_jwt_without_claiming_smart() {
        let value = capability_statement("http://localhost:8080/fhir");
        let security = &value["rest"][0]["security"];
        let serialized = serde_json::to_string(security).unwrap();

        assert!(serialized.contains("JWT Bearer"));
        assert!(serialized.contains("missing, empty, or unrecognized scopes grant no permission"));
        assert!(!serialized.contains("defaults to both"));
        assert!(!serialized.contains("SMART-on-FHIR"));
        assert!(!serialized.contains("Smart-on-FHIR"));
    }

    #[test]
    fn capability_reports_instance_cors_configuration() {
        let disabled = build_statement("http://localhost:8080/fhir", false);
        let enabled = build_statement("http://localhost:8080/fhir", true);

        assert_eq!(disabled["rest"][0]["security"]["cors"], false);
        assert_eq!(enabled["rest"][0]["security"]["cors"], true);
    }
}
