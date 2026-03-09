use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveTime, TimeZone, Utc};
use fluent_uri::{Uri, UriRef};
use jsonschema::Validator;
use serde_json::Value;

use crate::error::{AppError, OperationIssue};

const FHIR_SCHEMA_JSON: &str = include_str!("../../fhir.schema.json");
const DATE_PATTERN: &str = "^([0-9]([0-9]([0-9][1-9]|[1-9]0)|[1-9]00)|[1-9]000)(-(0[1-9]|1[0-2])(-(0[1-9]|[1-2][0-9]|3[0-1]))?)?$";
const DATETIME_PATTERN: &str = "^([0-9]([0-9]([0-9][1-9]|[1-9]0)|[1-9]00)|[1-9]000)(-(0[1-9]|1[0-2])(-(0[1-9]|[1-2][0-9]|3[0-1])(T([01][0-9]|2[0-3]):[0-5][0-9]:([0-5][0-9]|60)(\\.[0-9]{1,9})?(Z|(\\+|-)((0[0-9]|1[0-3]):[0-5][0-9]|14:00)?)?)?)?)?$";
const INSTANT_PATTERN: &str = "^([0-9]([0-9]([0-9][1-9]|[1-9]0)|[1-9]00)|[1-9]000)-(0[1-9]|1[0-2])-(0[1-9]|[1-2][0-9]|3[0-1])T([01][0-9]|2[0-3]):[0-5][0-9]:([0-5][0-9]|60)(\\.[0-9]{1,9})?(Z|(\\+|-)((0[0-9]|1[0-3]):[0-5][0-9]|14:00))$";
const INTEGER_PATTERN: &str = "^[0]|[-+]?[1-9][0-9]*$";
const POSITIVE_INT_PATTERN: &str = "^[1-9][0-9]*$";
const UNSIGNED_INT_PATTERN: &str = "^[0]|([1-9][0-9]*)$";

#[derive(Clone, Copy)]
enum PrimitiveConstraint {
    Date,
    DateTime,
    Instant,
    Integer,
    Integer64,
    PositiveInt,
    Uri,
    Url,
    Canonical,
    UnsignedInt,
}

#[derive(Clone, Copy)]
enum ObjectConstraint {
    Attachment,
    ContactPoint,
    Quantity,
    Period,
}

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
        let mut issues: Vec<_> = validator
            .iter_errors(resource)
            .map(|error| {
                let path = error.instance_path().to_string();
                let diagnostics = if path.is_empty() {
                    error.to_string()
                } else {
                    format!("{error} at instance path '{path}'")
                };

                OperationIssue::error("invalid", diagnostics)
            })
            .collect();

        if let Some(schema) = self.resolve_ref(&format!("#/definitions/{resource_type}")) {
            self.collect_datatype_issues(schema, resource, "", None, &mut issues);
        }

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

    fn resolve_ref<'a>(&'a self, ref_path: &str) -> Option<&'a Value> {
        ref_path
            .strip_prefix('#')
            .and_then(|pointer| self.root_schema.pointer(pointer))
    }

    fn collect_datatype_issues(
        &self,
        schema: &Value,
        instance: &Value,
        path: &str,
        property_name: Option<&str>,
        issues: &mut Vec<OperationIssue>,
    ) {
        let schema_ref = schema.get("$ref").and_then(Value::as_str);
        let resolved = schema
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|ref_path| self.resolve_ref(ref_path))
            .unwrap_or(schema);

        if let Some(issue) = primitive_constraint(schema_ref, resolved, property_name)
            .and_then(|constraint| validate_primitive_constraint(constraint, instance, path))
        {
            issues.push(issue);
        }

        if let Some(object_constraint) = object_constraint(schema_ref)
            && let Some(issue) = validate_object_constraint(object_constraint, instance, path)
        {
            issues.push(issue);
        }

        if let Some(all_of) = resolved.get("allOf").and_then(Value::as_array) {
            for child_schema in all_of {
                self.collect_datatype_issues(child_schema, instance, path, property_name, issues);
            }
        }

        if let Some(properties) = resolved.get("properties").and_then(Value::as_object)
            && let Some(object) = instance.as_object()
        {
            for (key, child_value) in object {
                if let Some(child_schema) = properties.get(key) {
                    let child_path = join_instance_path(path, key);
                    self.collect_datatype_issues(
                        child_schema,
                        child_value,
                        &child_path,
                        Some(key.as_str()),
                        issues,
                    );
                }
            }
        }

        if let Some(items_schema) = resolved.get("items")
            && let Some(items) = instance.as_array()
        {
            for (index, item) in items.iter().enumerate() {
                let child_path = join_instance_path(path, &index.to_string());
                self.collect_datatype_issues(items_schema, item, &child_path, None, issues);
            }
        }
    }
}

fn primitive_constraint(
    schema_ref: Option<&str>,
    schema: &Value,
    property_name: Option<&str>,
) -> Option<PrimitiveConstraint> {
    if let Some(ref_path) = schema_ref {
        return match ref_path {
            "#/definitions/date" => Some(PrimitiveConstraint::Date),
            "#/definitions/dateTime" => Some(PrimitiveConstraint::DateTime),
            "#/definitions/instant" => Some(PrimitiveConstraint::Instant),
            "#/definitions/integer" => Some(PrimitiveConstraint::Integer),
            "#/definitions/integer64" => Some(PrimitiveConstraint::Integer64),
            "#/definitions/positiveInt" => Some(PrimitiveConstraint::PositiveInt),
            "#/definitions/uri" => Some(PrimitiveConstraint::Uri),
            "#/definitions/url" => Some(PrimitiveConstraint::Url),
            "#/definitions/canonical" => Some(PrimitiveConstraint::Canonical),
            "#/definitions/unsignedInt" => Some(PrimitiveConstraint::UnsignedInt),
            _ => None,
        };
    }

    match (
        schema.get("type").and_then(Value::as_str),
        schema.get("pattern").and_then(Value::as_str),
        property_name,
    ) {
        (Some("string"), Some(DATE_PATTERN), _) => Some(PrimitiveConstraint::Date),
        (Some("string"), Some(DATETIME_PATTERN), _) => Some(PrimitiveConstraint::DateTime),
        (Some("string"), Some(INSTANT_PATTERN), _) => Some(PrimitiveConstraint::Instant),
        (Some("number"), Some(INTEGER_PATTERN), _) => Some(PrimitiveConstraint::Integer),
        (Some("string"), Some(INTEGER_PATTERN), _) => Some(PrimitiveConstraint::Integer64),
        (Some("number"), Some(POSITIVE_INT_PATTERN), _) => Some(PrimitiveConstraint::PositiveInt),
        (Some("number"), Some(UNSIGNED_INT_PATTERN), _) => Some(PrimitiveConstraint::UnsignedInt),
        (Some("string"), Some("^\\S*$"), Some("valueCanonical")) => {
            Some(PrimitiveConstraint::Canonical)
        }
        _ => None,
    }
}

fn object_constraint(schema_ref: Option<&str>) -> Option<ObjectConstraint> {
    match schema_ref {
        Some("#/definitions/Attachment") => Some(ObjectConstraint::Attachment),
        Some("#/definitions/ContactPoint") => Some(ObjectConstraint::ContactPoint),
        Some("#/definitions/Quantity") => Some(ObjectConstraint::Quantity),
        Some("#/definitions/Period") => Some(ObjectConstraint::Period),
        _ => None,
    }
}

fn validate_primitive_constraint(
    constraint: PrimitiveConstraint,
    instance: &Value,
    path: &str,
) -> Option<OperationIssue> {
    match constraint {
        PrimitiveConstraint::Date => validate_date_value(instance, path),
        PrimitiveConstraint::DateTime => validate_datetime_value(instance, path),
        PrimitiveConstraint::Instant => validate_instant_value(instance, path),
        PrimitiveConstraint::Integer => validate_integer_value(instance, path),
        PrimitiveConstraint::Integer64 => validate_integer64_value(instance, path),
        PrimitiveConstraint::PositiveInt => validate_positive_int_value(instance, path),
        PrimitiveConstraint::Uri => validate_uri_value(instance, path),
        PrimitiveConstraint::Url => validate_url_value(instance, path),
        PrimitiveConstraint::Canonical => validate_canonical_value(instance, path),
        PrimitiveConstraint::UnsignedInt => validate_unsigned_int_value(instance, path),
    }
}

fn validate_object_constraint(
    constraint: ObjectConstraint,
    instance: &Value,
    path: &str,
) -> Option<OperationIssue> {
    match constraint {
        ObjectConstraint::Attachment => validate_attachment_object(instance, path),
        ObjectConstraint::ContactPoint => validate_contact_point_object(instance, path),
        ObjectConstraint::Quantity => validate_quantity_object(instance, path),
        ObjectConstraint::Period => validate_period_object(instance, path),
    }
}

fn validate_date_value(instance: &Value, path: &str) -> Option<OperationIssue> {
    let value = instance.as_str()?;
    if is_valid_fhir_date(value) {
        None
    } else {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "FHIR date at instance path '{}' must be a valid calendar date",
                display_path(path)
            ),
        ))
    }
}

fn validate_datetime_value(instance: &Value, path: &str) -> Option<OperationIssue> {
    let value = instance.as_str()?;
    if is_valid_fhir_datetime(value) {
        None
    } else {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "FHIR dateTime at instance path '{}' must contain a valid calendar date",
                display_path(path)
            ),
        ))
    }
}

fn validate_instant_value(instance: &Value, path: &str) -> Option<OperationIssue> {
    let value = instance.as_str()?;
    if is_valid_fhir_instant(value) {
        None
    } else {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "FHIR instant at instance path '{}' must contain a valid calendar date",
                display_path(path)
            ),
        ))
    }
}

fn validate_integer_value(instance: &Value, path: &str) -> Option<OperationIssue> {
    let number = instance.as_number()?;
    let valid = number
        .as_i64()
        .map(|value| i32::try_from(value).is_ok())
        .or_else(|| number.as_u64().map(|value| i32::try_from(value).is_ok()))
        .unwrap_or(false);

    if valid {
        None
    } else {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "FHIR integer at instance path '{}' must be a whole number in the 32-bit signed range",
                display_path(path)
            ),
        ))
    }
}

fn validate_integer64_value(instance: &Value, path: &str) -> Option<OperationIssue> {
    let value = instance.as_str()?;
    if value.parse::<i64>().is_ok() {
        None
    } else {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "FHIR integer64 at instance path '{}' must be a whole number in the 64-bit signed range",
                display_path(path)
            ),
        ))
    }
}

fn validate_positive_int_value(instance: &Value, path: &str) -> Option<OperationIssue> {
    let number = instance.as_number()?;
    let valid = number
        .as_u64()
        .map(|value| value >= 1 && i32::try_from(value).is_ok())
        .unwrap_or(false);

    if valid {
        None
    } else {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "FHIR positiveInt at instance path '{}' must be a whole number between 1 and 2147483647",
                display_path(path)
            ),
        ))
    }
}

fn validate_uri_value(instance: &Value, path: &str) -> Option<OperationIssue> {
    let value = instance.as_str()?;
    if UriRef::parse(value).is_ok() {
        None
    } else {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "FHIR uri at instance path '{}' must be a valid URI reference",
                display_path(path)
            ),
        ))
    }
}

fn validate_url_value(instance: &Value, path: &str) -> Option<OperationIssue> {
    let value = instance.as_str()?;
    if Uri::parse(value).is_ok() {
        None
    } else {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "FHIR url at instance path '{}' must be an absolute URL",
                display_path(path)
            ),
        ))
    }
}

fn validate_canonical_value(instance: &Value, path: &str) -> Option<OperationIssue> {
    let value = instance.as_str()?;
    let (base, _) = value.split_once('|').unwrap_or((value, ""));
    let valid = if base.starts_with('#') {
        UriRef::parse(base).is_ok()
    } else {
        UriRef::parse(base)
            .map(|uri| uri.has_scheme())
            .unwrap_or(false)
    };

    if valid {
        None
    } else {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "FHIR canonical at instance path '{}' must be an absolute URI or fragment reference",
                display_path(path)
            ),
        ))
    }
}

fn validate_unsigned_int_value(instance: &Value, path: &str) -> Option<OperationIssue> {
    let number = instance.as_number()?;
    let valid = number
        .as_u64()
        .map(|value| i32::try_from(value).is_ok())
        .unwrap_or(false);

    if valid {
        None
    } else {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "FHIR unsignedInt at instance path '{}' must be a whole number between 0 and 2147483647",
                display_path(path)
            ),
        ))
    }
}

fn validate_attachment_object(instance: &Value, path: &str) -> Option<OperationIssue> {
    let object = instance.as_object()?;
    if object.contains_key("data") && !object.contains_key("contentType") {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "Attachment at instance path '{}' must include contentType when data is present",
                display_path(path)
            ),
        ))
    } else {
        None
    }
}

fn validate_contact_point_object(instance: &Value, path: &str) -> Option<OperationIssue> {
    let object = instance.as_object()?;
    if object.contains_key("value") && !object.contains_key("system") {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "ContactPoint at instance path '{}' must include system when value is present",
                display_path(path)
            ),
        ))
    } else {
        None
    }
}

fn validate_quantity_object(instance: &Value, path: &str) -> Option<OperationIssue> {
    let object = instance.as_object()?;
    if object.contains_key("code") && !object.contains_key("system") {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "Quantity at instance path '{}' must include system when code is present",
                display_path(path)
            ),
        ))
    } else {
        None
    }
}

fn validate_period_object(instance: &Value, path: &str) -> Option<OperationIssue> {
    let object = instance.as_object()?;
    let start = object.get("start").and_then(Value::as_str)?;
    let end = object.get("end").and_then(Value::as_str)?;

    let start_low = period_low_boundary(start)?;
    let end_high = period_high_boundary(end)?;

    if start_low <= end_high {
        None
    } else {
        Some(OperationIssue::error(
            "invalid",
            format!(
                "Period at instance path '{}' must have start less than or equal to end",
                display_path(path)
            ),
        ))
    }
}

fn is_valid_fhir_date(value: &str) -> bool {
    match value.split('-').collect::<Vec<_>>().as_slice() {
        [year] => parse_year(year).is_some(),
        [year, month] => parse_year(year).is_some() && parse_month(month).is_some(),
        [year, month, day] => {
            let year = parse_year(year);
            let month = parse_month(month);
            let day = parse_day(day);

            match (year, month, day) {
                (Some(year), Some(month), Some(day)) => {
                    NaiveDate::from_ymd_opt(year, month, day).is_some()
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn is_valid_fhir_datetime(value: &str) -> bool {
    value
        .split_once('T')
        .map(|(date, _)| is_valid_fhir_date(date))
        .unwrap_or_else(|| is_valid_fhir_date(value))
}

fn is_valid_fhir_instant(value: &str) -> bool {
    value
        .split_once('T')
        .map(|(date, _)| is_valid_fhir_date(date) && date.split('-').count() == 3)
        .unwrap_or(false)
}

fn parse_year(value: &str) -> Option<i32> {
    value.parse::<i32>().ok().filter(|year| *year > 0)
}

fn parse_month(value: &str) -> Option<u32> {
    value
        .parse::<u32>()
        .ok()
        .filter(|month| (1..=12).contains(month))
}

fn parse_day(value: &str) -> Option<u32> {
    value
        .parse::<u32>()
        .ok()
        .filter(|day| (1..=31).contains(day))
}

fn period_low_boundary(value: &str) -> Option<DateTime<Utc>> {
    parse_period_boundary(value, true)
}

fn period_high_boundary(value: &str) -> Option<DateTime<Utc>> {
    parse_period_boundary(value, false)
}

fn parse_period_boundary(value: &str, low: bool) -> Option<DateTime<Utc>> {
    if value.contains('T') {
        let parsed = DateTime::parse_from_rfc3339(value).ok()?;
        return Some(parsed.with_timezone(&Utc));
    }

    let parts: Vec<_> = value.split('-').collect();
    let date = match parts.as_slice() {
        [year] => {
            let year = parse_year(year)?;
            if low {
                NaiveDate::from_ymd_opt(year, 1, 1)?
            } else {
                NaiveDate::from_ymd_opt(year, 12, 31)?
            }
        }
        [year, month] => {
            let year = parse_year(year)?;
            let month = parse_month(month)?;
            if low {
                NaiveDate::from_ymd_opt(year, month, 1)?
            } else {
                let day = last_day_of_month(year, month)?;
                NaiveDate::from_ymd_opt(year, month, day)?
            }
        }
        [year, month, day] => {
            let year = parse_year(year)?;
            let month = parse_month(month)?;
            let day = parse_day(day)?;
            NaiveDate::from_ymd_opt(year, month, day)?
        }
        _ => return None,
    };

    let time = if low {
        NaiveTime::from_hms_opt(0, 0, 0)?
    } else {
        NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999)?
    };

    Some(
        FixedOffset::east_opt(0)?
            .from_utc_datetime(&date.and_time(time))
            .with_timezone(&Utc),
    )
}

fn last_day_of_month(year: i32, month: u32) -> Option<u32> {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };

    let first_of_next = NaiveDate::from_ymd_opt(next_year, next_month, 1)?;
    Some(first_of_next.pred_opt()?.day())
}

fn join_instance_path(base: &str, segment: &str) -> String {
    if base.is_empty() {
        format!("/{segment}")
    } else {
        format!("{base}/{segment}")
    }
}

fn display_path(path: &str) -> &str {
    if path.is_empty() { "/" } else { path }
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
            .validate_resource(
                "Patient",
                &json!({"resourceType": "Patient", "bogus": true}),
            )
            .expect_err("schema validation must fail");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn accepts_minimal_patient() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        validator
            .validate_resource("Patient", &json!({"resourceType": "Patient"}))
            .expect("minimal Patient should be valid");
    }

    #[test]
    fn accepts_patient_with_name() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "name": [{"family": "Smith", "given": ["John"]}]
                }),
            )
            .expect("Patient with name should be valid");
    }

    #[test]
    fn accepts_minimal_observation() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        validator
            .validate_resource(
                "Observation",
                &json!({
                    "resourceType": "Observation",
                    "status": "final",
                    "code": {"coding": [{"system": "http://loinc.org", "code": "1234-5"}]}
                }),
            )
            .expect("minimal Observation should be valid");
    }

    #[test]
    fn accepts_observation_with_any_code_string_for_status() {
        // The FHIR JSON Schema defines status as a `code` type (string with pattern),
        // not as an enum. Value-set validation is a separate concern beyond JSON Schema.
        let validator = FhirSchemaValidator::new().expect("validator should load");
        validator
            .validate_resource(
                "Observation",
                &json!({
                    "resourceType": "Observation",
                    "status": "some-code-value",
                    "code": {"coding": [{"system": "http://loinc.org", "code": "1234-5"}]}
                }),
            )
            .expect("any valid code string should pass JSON Schema validation");
    }

    #[test]
    fn rejects_patient_with_wrong_type_for_active() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({"resourceType": "Patient", "active": "yes"}),
            )
            .expect_err("wrong type for active should fail");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_patient_with_invalid_calendar_birth_date() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "birthDate": "2024-02-30"
                }),
            )
            .expect_err("invalid calendar dates must fail");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_fractional_integer_fields() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "multipleBirthInteger": 1.5
                }),
            )
            .expect_err("FHIR integer fields must reject fractions");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_out_of_range_integer_fields() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "multipleBirthInteger": 3_000_000_000_i64
                }),
            )
            .expect_err("FHIR integer fields must stay within 32-bit range");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_non_positive_positive_int_fields() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "extension": [{
                        "url": "http://example.org/fhir/StructureDefinition/test-positive-int",
                        "valuePositiveInt": 0
                    }]
                }),
            )
            .expect_err("positiveInt fields must be greater than zero");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_negative_unsigned_int_fields() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "extension": [{
                        "url": "http://example.org/fhir/StructureDefinition/test-unsigned-int",
                        "valueUnsignedInt": -1
                    }]
                }),
            )
            .expect_err("unsignedInt fields must be non-negative");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_out_of_range_integer64_fields() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "photo": [{
                        "contentType": "image/png",
                        "size": "9223372036854775808"
                    }]
                }),
            )
            .expect_err("integer64 fields must stay within 64-bit range");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_invalid_calendar_datetime_in_extensions() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "extension": [{
                        "url": "http://example.org/fhir/StructureDefinition/test-datetime",
                        "valueDateTime": "2024-02-30T10:15:30Z"
                    }]
                }),
            )
            .expect_err("invalid calendar dateTimes must fail");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn accepts_boundary_integer_values() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "multipleBirthInteger": 2_147_483_647,
                    "extension": [
                        {
                            "url": "http://example.org/fhir/StructureDefinition/test-positive-bound",
                            "valuePositiveInt": 1
                        },
                        {
                            "url": "http://example.org/fhir/StructureDefinition/test-unsigned-bound",
                            "valueUnsignedInt": 0
                        }
                    ],
                    "photo": [{
                        "contentType": "image/png",
                        "size": "9223372036854775807"
                    }]
                }),
            )
            .expect("FHIR boundary integer values should be valid");
    }

    #[test]
    fn rejects_invalid_uri_fields() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "identifier": [{
                        "system": "http://[::1",
                        "value": "12345"
                    }]
                }),
            )
            .expect_err("FHIR uri fields must contain valid URI references");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_non_url_attachment_urls() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "photo": [{
                        "contentType": "image/png",
                        "url": "Patient/example"
                    }]
                }),
            )
            .expect_err("FHIR url fields must be absolute URLs");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_relative_canonical_values() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "extension": [{
                        "url": "http://example.org/fhir/StructureDefinition/test-canonical",
                        "valueCanonical": "Patient/example"
                    }]
                }),
            )
            .expect_err("FHIR canonical values must be absolute URIs or fragment references");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn accepts_fragment_canonical_values() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "extension": [{
                        "url": "http://example.org/fhir/StructureDefinition/test-canonical",
                        "valueCanonical": "#contained"
                    }]
                }),
            )
            .expect("fragment canonical values should be valid");
    }

    #[test]
    fn rejects_contact_point_value_without_system() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "telecom": [{
                        "value": "555-0100"
                    }]
                }),
            )
            .expect_err("ContactPoint.value requires ContactPoint.system");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_attachment_data_without_content_type() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Patient",
                &json!({
                    "resourceType": "Patient",
                    "photo": [{
                        "data": "SGVsbG8="
                    }]
                }),
            )
            .expect_err("Attachment.data requires Attachment.contentType");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_quantity_code_without_system() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Observation",
                &json!({
                    "resourceType": "Observation",
                    "status": "final",
                    "code": {
                        "coding": [{
                            "system": "http://loinc.org",
                            "code": "15074-8"
                        }]
                    },
                    "valueQuantity": {
                        "value": 6.3,
                        "code": "mmol/L"
                    }
                }),
            )
            .expect_err("Quantity.code requires Quantity.system");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn rejects_periods_with_start_after_end() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let err = validator
            .validate_resource(
                "Observation",
                &json!({
                    "resourceType": "Observation",
                    "status": "final",
                    "code": {
                        "coding": [{
                            "system": "http://loinc.org",
                            "code": "15074-8"
                        }]
                    },
                    "effectivePeriod": {
                        "start": "2024-03-02T10:00:00Z",
                        "end": "2024-03-01T10:00:00Z"
                    }
                }),
            )
            .expect_err("Period.start must not be after Period.end");

        assert!(err.to_string().contains("validation failed"));
    }

    #[test]
    fn validator_caches_compiled_schemas() {
        let validator = FhirSchemaValidator::new().expect("validator should load");

        // First call compiles, second call uses cache
        validator
            .validate_resource("Patient", &json!({"resourceType": "Patient"}))
            .expect("first call should work");
        validator
            .validate_resource("Patient", &json!({"resourceType": "Patient"}))
            .expect("cached call should work too");

        // Different type also works
        validator
            .validate_resource(
                "Observation",
                &json!({
                    "resourceType": "Observation",
                    "status": "final",
                    "code": {"coding": [{"system": "http://loinc.org", "code": "1"}]}
                }),
            )
            .expect("Observation should work");
    }

    #[test]
    fn multiple_resource_types_are_supported() {
        let validator = FhirSchemaValidator::new().expect("validator should load");
        let types = [
            "Patient",
            "Observation",
            "Organization",
            "Practitioner",
            "Encounter",
            "Condition",
            "Procedure",
            "DiagnosticReport",
            "AllergyIntolerance",
            "MedicationRequest",
            "Bundle",
        ];

        for rt in types {
            let result = validator.validate_resource(rt, &json!({"resourceType": rt}));
            // Some types may require mandatory fields and fail,
            // but they should NOT fail with "unsupported FHIR resource type"
            if let Err(e) = result {
                assert!(
                    !e.to_string().contains("unsupported"),
                    "{rt} should be recognized as a valid resource type"
                );
            }
        }
    }
}
