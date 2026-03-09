use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use jsonschema::Validator;
use serde_json::Value;

use crate::error::{AppError, OperationIssue};

const FHIR_SCHEMA_JSON: &str = include_str!("../../fhir.schema.json");

pub struct FhirSchemaValidator {
    root_schema: Value,
    validators: RwLock<HashMap<String, Arc<Validator>>>,
}

impl FhirSchemaValidator {
    pub fn new() -> Result<Self, AppError> {
        let root_schema = serde_json::from_str(FHIR_SCHEMA_JSON).map_err(|error| {
            AppError::Internal(format!("failed to parse bundled FHIR schema: {error}"))
        })?;

        Ok(Self {
            root_schema,
            validators: RwLock::new(HashMap::new()),
        })
    }

    pub fn validate_resource(&self, resource_type: &str, resource: &Value) -> Result<(), AppError> {
        let validator = self.validator_for(resource_type)?;
        let issues: Vec<_> = validator
            .iter_errors(resource)
            .map(|error| {
                let path = error.instance_path.to_string();
                let diagnostics = if path.is_empty() {
                    error.to_string()
                } else {
                    format!("{error} at instance path '{path}'")
                };

                OperationIssue::error("invalid", diagnostics)
            })
            .collect();

        if issues.is_empty() {
            Ok(())
        } else {
            Err(AppError::Validation(issues))
        }
    }

    fn validator_for(&self, resource_type: &str) -> Result<Arc<Validator>, AppError> {
        if let Some(existing) = self
            .validators
            .read()
            .map_err(|_| AppError::Internal("schema validator cache lock poisoned".to_owned()))?
            .get(resource_type)
            .cloned()
        {
            return Ok(existing);
        }

        let compiled = Arc::new(self.compile_validator(resource_type)?);
        let mut validators = self
            .validators
            .write()
            .map_err(|_| AppError::Internal("schema validator cache lock poisoned".to_owned()))?;

        Ok(validators
            .entry(resource_type.to_owned())
            .or_insert_with(|| Arc::clone(&compiled))
            .clone())
    }

    fn compile_validator(&self, resource_type: &str) -> Result<Validator, AppError> {
        let has_definition = self
            .root_schema
            .get("definitions")
            .and_then(Value::as_object)
            .map(|definitions| definitions.contains_key(resource_type))
            .unwrap_or(false);

        if !has_definition {
            return Err(AppError::BadRequest(format!(
                "unsupported FHIR resource type '{resource_type}'"
            )));
        }

        let mut schema = self.root_schema.clone();
        let object = schema.as_object_mut().ok_or_else(|| {
            AppError::Internal("bundled FHIR schema root must be a JSON object".to_owned())
        })?;

        object.insert(
            "$ref".to_owned(),
            Value::String(format!("#/definitions/{resource_type}")),
        );

        jsonschema::validator_for(&schema).map_err(|error| {
            AppError::Internal(format!(
                "failed to compile FHIR schema validator for {resource_type}: {error}"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::FhirSchemaValidator;

    #[test]
    fn rejects_unknown_resource_type() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource("MadeUpResource", &json!({"resourceType": "MadeUpResource"}))
            .expect_err("unknown type must fail");

        assert!(err.to_string().contains("unsupported FHIR resource type"));
    }

    #[test]
    fn rejects_additional_properties() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource("Patient", &json!({"resourceType": "Patient", "bogus": true}))
            .expect_err("schema validation must fail");

        assert!(err.to_string().contains("validation failed"));
    }
}