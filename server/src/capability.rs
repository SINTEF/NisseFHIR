use serde_json::{Value, json};

pub fn capability_statement(base_url: &str) -> Value {
    let generic_interactions = json!([
        {"code": "create"},
        {"code": "read"},
        {"code": "update"},
        {"code": "patch"},
        {"code": "delete"},
        {"code": "search-type"}
    ]);

    json!({
        "resourceType": "CapabilityStatement",
        "status": "active",
        "kind": "instance",
        "fhirVersion": "6.0.0-ballot3",
        "format": ["json"],
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
            "resource": [
                {
                    "type": "*",
                    "interaction": generic_interactions,
                    "searchParam": [
                        {
                            "name": "_count",
                            "type": "number",
                            "documentation": "Limits the number of resources returned per page."
                        },
                        {
                            "name": "_after_id",
                            "type": "string",
                            "documentation": "Returns resources that sort after the supplied resource id cursor."
                        }
                    ]
                },
                {
                    "type": "Patient",
                    "interaction": generic_interactions,
                    "searchParam": [
                        {
                            "name": "_count",
                            "type": "number",
                            "documentation": "Limits the number of resources returned per page."
                        },
                        {
                            "name": "_after_id",
                            "type": "string",
                            "documentation": "Returns resources that sort after the supplied resource id cursor."
                        },
                        {
                            "name": "name",
                            "type": "string",
                            "documentation": "Matches a patient's family or given names using case-insensitive partial matching."
                        },
                        {
                            "name": "birthdate",
                            "type": "date",
                            "documentation": "Matches the patient's exact birthDate value."
                        },
                        {
                            "name": "identifier",
                            "type": "token",
                            "documentation": "Matches a patient identifier by exact value or exact system|value pair."
                        }
                    ]
                },
                {
                    "type": "Observation",
                    "interaction": generic_interactions,
                    "searchParam": [
                        {
                            "name": "_count",
                            "type": "number",
                            "documentation": "Limits the number of resources returned per page."
                        },
                        {
                            "name": "_after_id",
                            "type": "string",
                            "documentation": "Returns resources that sort after the supplied resource id cursor."
                        },
                        {
                            "name": "code",
                            "type": "token",
                            "documentation": "Matches an observation code.coding.code value exactly."
                        },
                        {
                            "name": "status",
                            "type": "token",
                            "documentation": "Matches the observation status exactly."
                        },
                        {
                            "name": "subject",
                            "type": "reference",
                            "documentation": "Matches the exact subject reference, for example Patient/example."
                        }
                    ]
                }
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
    fn capability_only_supports_json() {
        let value = capability_statement("http://localhost:8080/fhir");
        let formats = value["format"].as_array().unwrap();
        assert_eq!(formats.len(), 1);
        assert_eq!(formats[0], "json");
    }
}
