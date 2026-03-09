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
}
