use serde_json::{Value, json};

pub fn capability_statement(base_url: &str) -> Value {
    json!({
        "resourceType": "CapabilityStatement",
        "status": "active",
        "kind": "instance",
        "fhirVersion": "6.0.0-ballot3",
        "format": ["json"],
        "rest": [{
            "mode": "server",
            "resource": [
                {
                    "type": "*",
                    "interaction": [
                        {"code": "create"},
                        {"code": "read"},
                        {"code": "update"}
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
    }

    #[test]
    fn capability_includes_implementation_url() {
        let value = capability_statement("https://fhir.example.com/api");
        assert_eq!(value["implementation"]["url"], "https://fhir.example.com/api");
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
