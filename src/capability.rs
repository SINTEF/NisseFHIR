use serde_json::{Value, json};

use crate::search_params::{RESOURCE_TYPES, SearchParamType, search_params_for};

/// Stable canonical identifier for the NisseFHIR CapabilityStatement.
///
/// This identifies the capability definition. It is deliberately independent
/// of the deployment-specific URL from which `/fhir/metadata` is retrieved.
pub const CAPABILITY_STATEMENT_CANONICAL_URL: &str =
    "https://sintef.github.io/NisseFHIR/CapabilityStatement/nissefhir";

/// FHIR's CapabilityStatement.rest.resource backbone element has no
/// first-class way to advertise which parameters `_sort` accepts, so this is
/// carried as a repeating extension — one `valueCode` per accepted sort key —
/// rather than as an ad hoc top-level field the JSON Schema would reject.
const SORT_PARAMETER_EXTENSION_URL: &str =
    "https://sintef.github.io/NisseFHIR/StructureDefinition/sort-parameter";

/// Build the `sortParameter` extension array for a resource that accepts
/// `_sort` with the given keys. Returns an empty `Vec` (omit the field
/// entirely) for a resource that does not support `_sort` at all.
fn sort_parameter_extensions(keys: &[&str]) -> Vec<Value> {
    keys.iter()
        .map(|key| json!({"url": SORT_PARAMETER_EXTENSION_URL, "valueCode": key}))
        .collect()
}

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
        json!({
            "name": "_sort",
            "type": "string",
            "documentation": "Comma-separated list of sort keys, each optionally prefixed with '-' for descending order, applied in the order given. See this resource's sortParameter for the keys accepted; an unsupported, unknown, or unindexed key is rejected with a 400."
        }),
        json!({
            "name": "_id",
            "type": "token",
            "documentation": "Matches a resource by its logical id."
        }),
    ];

    // Build concrete resource entries dynamically from the schema-derived
    // resource list and the executable search parameter registry.
    let mut resource_entries: Vec<Value> = Vec::with_capacity(RESOURCE_TYPES.len());

    for &rt in RESOURCE_TYPES {
        // AuditEvent has a registry entry (for `PARAMS_AUDITEVENT`) but is
        // served exclusively by the dedicated, read-only
        // `search_audit_events` / `read_audit_event` handlers, not the
        // generic resource CRUD path this loop describes. Its accurate
        // entry is appended separately below; building one here too would
        // produce two conflicting "AuditEvent" entries in `rest.resource`.
        if rt == "AuditEvent" {
            continue;
        }

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
            let mut parameter = json!({
                "name": sp.code,
                "type": type_str,
            });
            if sp.param_type == SearchParamType::String {
                parameter["documentation"] = json!(
                    "Supports the :exact and :contains modifiers in addition to default prefix matching."
                );
            }
            search_params.push(parameter);
        }

        let mut resource = json!({
            "type": rt,
            "interaction": generic_interactions,
            "conditionalCreate": !executable_params.is_empty(),
            "updateCreate": true,
            "searchParam": search_params,
            "extension": sort_parameter_extensions(&crate::sort::sortable_keys_for(rt)),
        });
        if rt == "Patient" {
            resource["operation"] = json!([{
                "name": "everything",
                "definition": "http://hl7.org/fhir/OperationDefinition/Patient-everything",
                "documentation": "Instance-level GET and POST are supported with mandatory bounded paging and signed 15-minute keyset cursors. Type-level invocation fails closed until authorization supplies an explicit Patient set. The nominated Patient is returned even when _type omits Patient. start/end requires _type narrowed to Encounter, Observation, Procedure, Condition, MedicationRequest, DocumentReference, Immunization, Patient, or a documented supporting context type."
            }]);
        } else if rt == "Group" {
            resource["operation"] = json!([{
                "name": "everything",
                "definition": "http://hl7.org/fhir/OperationDefinition/Group-everything",
                "documentation": "Instance-level GET and POST are supported for enumerated/actual person Groups of at most 100 local Patient members."
            }]);
        }
        resource_entries.push(resource);
    }
    // AuditEvent search does not support `_sort` (task 040): it is strictly
    // ordered by id (see `search_audit_events`), so no sort-parameter
    // extension is emitted here.
    resource_entries.push(json!({
        "type": "AuditEvent",
        "interaction": [{"code":"read"}, {"code":"search-type"}],
        "updateCreate": false,
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
    fn capability_advertises_instance_everything_operations() {
        let value = capability_statement("http://localhost:8080/fhir");
        let resources = value["rest"][0]["resource"].as_array().unwrap();
        for (resource_type, canonical) in [
            (
                "Patient",
                "http://hl7.org/fhir/OperationDefinition/Patient-everything",
            ),
            (
                "Group",
                "http://hl7.org/fhir/OperationDefinition/Group-everything",
            ),
        ] {
            let resource = resources
                .iter()
                .find(|entry| entry["type"] == resource_type)
                .unwrap();
            assert_eq!(resource["operation"][0]["name"], "everything");
            assert_eq!(resource["operation"][0]["definition"], canonical);
        }
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
        assert!(names.contains(&"_id"));
    }

    /// Extract the `sortParameter` extension's `valueCode`s from one resource
    /// entry (see `sort_parameter_extensions`), preserving order.
    fn sort_parameter_values(resource: &serde_json::Value) -> Vec<&str> {
        resource["extension"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|ext| ext["url"] == super::SORT_PARAMETER_EXTENSION_URL)
            .map(|ext| ext["valueCode"].as_str().unwrap())
            .collect()
    }

    /// Cross-checks the CapabilityStatement's advertised sort-parameter
    /// extension against what `_sort` parsing actually accepts, so the two
    /// can never drift: every generic resource type must advertise its exact
    /// resource-specific sortable key set, each advertised key must parse, and
    /// AuditEvent — which does not implement `_sort` (see
    /// `search_audit_events`) — must advertise none.
    #[test]
    fn capability_sort_parameters_match_accepted_sort_keys() {
        let value = capability_statement("http://localhost:8080/fhir");
        let resources = value["rest"][0]["resource"].as_array().unwrap();
        assert!(!resources.is_empty());

        // AuditEvent is pushed last, after the generic per-resource-type loop.
        let (audit_event, generic) = resources.split_last().unwrap();
        assert_eq!(audit_event["type"], "AuditEvent");
        assert!(
            sort_parameter_values(audit_event).is_empty(),
            "AuditEvent must not advertise any sort keys, it does not support _sort"
        );

        for resource in generic {
            let advertised = sort_parameter_values(resource);
            let resource_type = resource["type"].as_str().unwrap();
            assert_eq!(
                advertised,
                crate::sort::sortable_keys_for(resource_type),
                "resource {resource_type} advertises a sort key set that does not match its accepted sort keys",
            );
            for &key in &advertised {
                crate::sort::parse_sort_param(resource_type, key)
                    .unwrap_or_else(|_| panic!("advertised sort key '{key}' must be accepted"));
            }
        }

        assert!(
            crate::sort::parse_sort_param("Patient", "status").is_err(),
            "a key outside the advertised set must not be accepted"
        );
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
        assert!(patient_params.contains(&"_id"));
        assert!(!patient_params.contains(&"phonetic"));
        assert!(observation_params.contains(&"code"));
        assert!(observation_params.contains(&"status"));
        assert!(observation_params.contains(&"subject"));
    }

    #[test]
    fn capability_advertises_only_executable_registry_entries() {
        let value = capability_statement("http://localhost:8080/fhir");
        let resources = value["rest"][0]["resource"].as_array().unwrap();

        // One entry per schema-derived resource type, including AuditEvent
        // (whose accurate, hand-written entry replaces the generic one the
        // per-resource-type loop would otherwise produce for it).
        assert_eq!(resources.len(), crate::search_params::RESOURCE_TYPES.len());
        assert!(resources.iter().all(|resource| resource["type"] != "*"));

        // AuditEvent's searchParam list is hand-written, not derived from
        // the registry, so it is intentionally excluded from this
        // registry-vs-advertised comparison.
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
