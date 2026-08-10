use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, LazyLock, RwLock};

use serde_json::Value;
use url::Url;

use crate::search_params::registry::JsonPath;
use crate::search_params::{SearchParamType, search_params_for};

type AttachmentPaths = Vec<Vec<String>>;
type AttachmentPathCache = BTreeMap<String, Arc<AttachmentPaths>>;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExtractedReference {
    pub search_param_code: Option<&'static str>,
    pub json_path: String,
    pub target_type: String,
    pub target_id: String,
    pub target_version_id: Option<i64>,
}

pub fn extract_references(
    resource_type: &str,
    resource: &Value,
    local_base_url: Option<&Url>,
) -> Vec<ExtractedReference> {
    let mut found = BTreeSet::new();
    // The Patient search parameter is global and has more than one branch for
    // several resource types.  It is kept separately from the executable
    // search registry so compartment membership never inherits a search
    // generator's "first branch only" limitation.
    for path in patient_compartment_paths(resource_type) {
        walk_path_owned(resource, path, "$".to_owned(), &mut |value, path| {
            collect(value, path, Some("patient"), local_base_url, &mut found)
        });
    }
    for param in search_params_for(resource_type)
        .iter()
        .filter(|param| param.param_type == SearchParamType::Reference)
    {
        match &param.path {
            JsonPath::Field(path) => {
                walk_path(resource, path, "$".to_owned(), &mut |value, path| {
                    collect(value, path, Some(param.code), local_base_url, &mut found)
                })
            }
            JsonPath::FieldAlternatives(paths) => {
                for path in *paths {
                    walk_path(resource, path, "$".to_owned(), &mut |value, path| {
                        collect(value, path, Some(param.code), local_base_url, &mut found)
                    });
                }
            }
            JsonPath::WhereFilter { base, suffix, .. } => {
                let mut path = base.to_vec();
                path.extend_from_slice(suffix);
                walk_path(resource, &path, "$".to_owned(), &mut |value, path| {
                    collect(value, path, Some(param.code), local_base_url, &mut found)
                });
            }
            _ => {}
        }
    }
    // Attachment.url is not a Reference. Derive its paths from the checked-in
    // R6 schema so an unrelated URL-valued element (for example
    // Extension.url) can never be treated as a Binary reference.
    for path in attachment_paths(resource_type).iter() {
        walk_path_owned(resource, path, "$".to_owned(), &mut |value, path| {
            collect_attachment_url(value, path, local_base_url, &mut found)
        });
    }
    // Extension is recursively nestable, so a finite schema path table cannot
    // enumerate every valid `extension.extension...valueAttachment` path.
    // Walk only the schema-defined Extension.valueAttachment shape in the
    // actual resource; arbitrary URL-valued objects remain excluded.
    walk_extension_attachment_urls(resource, "$".to_owned(), local_base_url, &mut found);
    // The R6 schema permits Reference values that do not have a corresponding
    // search parameter. Schema validation rejects arbitrary resource fields,
    // so this walker complements (rather than replaces) the generated paths.
    walk_schema_reference_values(resource, "$".to_owned(), local_base_url, &mut found);
    let mut deduplicated = BTreeMap::new();
    for reference in found {
        let key = (
            reference.json_path.clone(),
            reference.target_type.clone(),
            reference.target_id.clone(),
            reference.target_version_id,
        );
        let existing = deduplicated.entry(key).or_insert_with(|| reference.clone());
        if reference.search_param_code == Some("patient")
            || (existing.search_param_code.is_none() && reference.search_param_code.is_some())
        {
            *existing = reference;
        }
    }
    deduplicated.into_values().collect()
}

/// Extract all direct Patient-compartment branches from the checked-in R6
/// SearchParameter artifact. This is initialized once and makes all branches
/// available even while the general search registry remains deliberately
/// conservative about executable SQL paths.
fn patient_compartment_paths(resource_type: &str) -> &'static [Vec<String>] {
    static PATHS: LazyLock<BTreeMap<String, Vec<Vec<String>>>> = LazyLock::new(|| {
        let registry: serde_json::Map<String, Value> =
            serde_json::from_str(include_str!("../search_registry.json"))
                .expect("checked-in search registry must be valid JSON");
        let expression = registry
            .values()
            .find_map(|parameters| {
                parameters.as_array()?.iter().find_map(|parameter| {
                    (parameter.get("code").and_then(Value::as_str) == Some("patient"))
                        .then(|| parameter.get("expression").and_then(Value::as_str))
                        .flatten()
                })
            })
            .expect("R6 registry must include the global patient parameter");
        let mut paths: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
        for branch in expression.split('|').map(str::trim) {
            let Some((resource_type, path)) = branch.split_once('.') else {
                continue;
            };
            let path = path.split(".where(").next().unwrap_or(path);
            let mut segments: Vec<String> = path.split('.').map(str::to_owned).collect();
            if segments.last().is_some_and(|last| last != "reference") {
                segments.push("reference".to_owned());
            }
            paths
                .entry(resource_type.to_owned())
                .or_default()
                .push(segments);
        }
        paths
    });
    PATHS.get(resource_type).map(Vec::as_slice).unwrap_or(&[])
}

fn walk_path_owned(
    value: &Value,
    segments: &[String],
    path: String,
    visit: &mut impl FnMut(&Value, String),
) {
    if let Some(items) = value.as_array() {
        for (index, item) in items.iter().enumerate() {
            walk_path_owned(item, segments, format!("{path}[{index}]"), visit);
        }
        return;
    }
    if segments.is_empty() {
        visit(value, path);
        return;
    }
    if let Some(next) = value.get(&segments[0]) {
        walk_path_owned(
            next,
            &segments[1..],
            format!("{path}.{}", segments[0]),
            visit,
        );
    }
}

fn walk_schema_reference_values(
    value: &Value,
    path: String,
    local_base_url: Option<&Url>,
    found: &mut BTreeSet<ExtractedReference>,
) {
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                walk_schema_reference_values(
                    item,
                    format!("{path}[{index}]"),
                    local_base_url,
                    found,
                );
            }
        }
        Value::Object(object) => {
            if let Some(reference) = object.get("reference").and_then(Value::as_str) {
                collect_local(
                    reference,
                    format!("{path}.reference"),
                    None,
                    local_base_url,
                    found,
                );
            }
            for (key, child) in object {
                if key != "reference" {
                    walk_schema_reference_values(
                        child,
                        format!("{path}.{key}"),
                        local_base_url,
                        found,
                    );
                }
            }
        }
        _ => {}
    }
}

fn collect_attachment_url(
    value: &Value,
    path: String,
    local_base_url: Option<&Url>,
    found: &mut BTreeSet<ExtractedReference>,
) {
    let Some(url) = value.get("url").and_then(Value::as_str) else {
        return;
    };
    let Some((target_type, target_id, target_version_id)) =
        parse_local_reference(url, local_base_url)
    else {
        return;
    };
    if target_type == "Binary" {
        found.insert(ExtractedReference {
            search_param_code: None,
            json_path: format!("{path}.url"),
            target_type,
            target_id,
            target_version_id,
        });
    }
}

/// Find every property path whose schema is the FHIR Attachment datatype.
/// This is derived from the same R6 JSON schema used for resource validation,
/// rather than inferring Attachment from an object that happens to have a
/// `url` property.
fn attachment_paths(resource_type: &str) -> Arc<AttachmentPaths> {
    static SCHEMA: LazyLock<Value> = LazyLock::new(|| {
        serde_json::from_str(include_str!("../../fhir.schema.json"))
            .expect("checked-in FHIR schema must be valid JSON")
    });
    static PATHS: LazyLock<RwLock<AttachmentPathCache>> =
        LazyLock::new(|| RwLock::new(BTreeMap::new()));
    if let Some(paths) = PATHS
        .read()
        .expect("attachment path cache lock must not be poisoned")
        .get(resource_type)
        .cloned()
    {
        return paths;
    }
    let definitions = SCHEMA
        .get("definitions")
        .and_then(Value::as_object)
        .expect("FHIR schema must have definitions");
    let Some(resource_schema) = definitions.get(resource_type) else {
        return Arc::new(Vec::new());
    };
    let mut paths = Vec::new();
    collect_attachment_paths(
        definitions,
        resource_schema,
        &mut Vec::new(),
        &mut BTreeSet::new(),
        &mut paths,
    );
    let paths = Arc::new(paths);
    PATHS
        .write()
        .expect("attachment path cache lock must not be poisoned")
        .entry(resource_type.to_owned())
        .or_insert_with(|| Arc::clone(&paths))
        .clone()
}

fn walk_extension_attachment_urls(
    value: &Value,
    path: String,
    local_base_url: Option<&Url>,
    found: &mut BTreeSet<ExtractedReference>,
) {
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                walk_extension_attachment_urls(
                    item,
                    format!("{path}[{index}]"),
                    local_base_url,
                    found,
                );
            }
        }
        Value::Object(object) => {
            for extension_key in ["extension", "modifierExtension"] {
                if let Some(extensions) = object.get(extension_key).and_then(Value::as_array) {
                    for (index, extension) in extensions.iter().enumerate() {
                        let extension_path = format!("{path}.{extension_key}[{index}]");
                        if let Some(attachment) = extension.get("valueAttachment") {
                            collect_attachment_url(
                                attachment,
                                format!("{extension_path}.valueAttachment"),
                                local_base_url,
                                found,
                            );
                        }
                        // An Extension itself may contain nested extensions.
                        walk_extension_attachment_urls(
                            extension,
                            extension_path,
                            local_base_url,
                            found,
                        );
                    }
                }
            }
            for (key, child) in object {
                if key != "extension" && key != "modifierExtension" {
                    walk_extension_attachment_urls(
                        child,
                        format!("{path}.{key}"),
                        local_base_url,
                        found,
                    );
                }
            }
        }
        _ => {}
    }
}

fn collect_attachment_paths(
    definitions: &serde_json::Map<String, Value>,
    schema: &Value,
    path: &mut Vec<String>,
    active_refs: &mut BTreeSet<String>,
    paths: &mut Vec<Vec<String>>,
) {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str)
        && let Some(name) = reference.strip_prefix("#/definitions/")
    {
        if name == "Attachment" {
            paths.push(path.clone());
            return;
        }
        if !active_refs.insert(name.to_owned()) {
            return;
        }
        if let Some(resolved) = definitions.get(name) {
            collect_attachment_paths(definitions, resolved, path, active_refs, paths);
        }
        active_refs.remove(name);
        return;
    }
    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        for child in all_of {
            collect_attachment_paths(definitions, child, path, active_refs, paths);
        }
    }
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (name, child) in properties {
            path.push(name.clone());
            collect_attachment_paths(definitions, child, path, active_refs, paths);
            path.pop();
        }
    }
    if let Some(items) = schema.get("items") {
        collect_attachment_paths(definitions, items, path, active_refs, paths);
    }
}

fn walk_path(
    value: &Value,
    segments: &[&str],
    path: String,
    visit: &mut impl FnMut(&Value, String),
) {
    if let Some(items) = value.as_array() {
        for (index, item) in items.iter().enumerate() {
            walk_path(item, segments, format!("{path}[{index}]"), visit);
        }
        return;
    }
    if segments.is_empty() {
        visit(value, path);
        return;
    }
    if let Some(next) = value.get(segments[0]) {
        walk_path(
            next,
            &segments[1..],
            format!("{path}.{}", segments[0]),
            visit,
        );
    }
}

fn collect(
    value: &Value,
    path: String,
    search_param_code: Option<&'static str>,
    local_base_url: Option<&Url>,
    found: &mut BTreeSet<ExtractedReference>,
) {
    if let Some(items) = value.as_array() {
        for (index, item) in items.iter().enumerate() {
            collect(
                item,
                format!("{path}[{index}]"),
                search_param_code,
                local_base_url,
                found,
            );
        }
        return;
    }
    let (reference, reference_path) = match value {
        Value::String(reference) => (reference.as_str(), path),
        Value::Object(object) => match object.get("reference").and_then(Value::as_str) {
            Some(reference) => (reference, format!("{path}.reference")),
            None => return,
        },
        _ => return,
    };
    collect_local(
        reference,
        reference_path,
        search_param_code,
        local_base_url,
        found,
    );
}

fn collect_local(
    reference: &str,
    json_path: String,
    search_param_code: Option<&'static str>,
    local_base_url: Option<&Url>,
    found: &mut BTreeSet<ExtractedReference>,
) {
    if let Some((target_type, target_id, target_version_id)) =
        parse_local_reference(reference, local_base_url)
    {
        found.insert(ExtractedReference {
            search_param_code,
            json_path,
            target_type,
            target_id,
            target_version_id,
        });
    }
}

pub fn parse_local_reference(
    reference: &str,
    local_base_url: Option<&Url>,
) -> Option<(String, String, Option<i64>)> {
    if reference.starts_with('#') || reference.starts_with("urn:") {
        return None;
    }
    let relative = if let Ok(absolute) = Url::parse(reference) {
        let base = local_base_url?;
        if absolute.scheme() != base.scheme()
            || absolute.host_str() != base.host_str()
            || absolute.port_or_known_default() != base.port_or_known_default()
        {
            return None;
        }
        let base_path = base.path().trim_end_matches('/');
        absolute
            .path()
            .strip_prefix(base_path)?
            .trim_start_matches('/')
            .to_owned()
    } else {
        reference.trim_start_matches('/').to_owned()
    };
    let pieces: Vec<_> = relative.split('/').collect();
    let (resource_type, id, version) = match pieces.as_slice() {
        [resource_type, id] => (*resource_type, *id, None),
        [resource_type, id, "_history", version] => {
            (*resource_type, *id, Some(version.parse::<i64>().ok()?))
        }
        _ => return None,
    };
    if !crate::search_params::is_valid_resource_type(resource_type)
        || id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
    {
        return None;
    }
    Some((resource_type.to_owned(), id.to_owned(), version))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use url::Url;

    use super::{extract_references, parse_local_reference};

    #[test]
    fn parses_relative_versioned_and_same_base_absolute_references() {
        let base = Url::parse("https://example.test/fhir").unwrap();
        assert_eq!(
            parse_local_reference("Patient/p1", Some(&base)).unwrap().1,
            "p1"
        );
        assert_eq!(
            parse_local_reference("Observation/o1/_history/7", Some(&base))
                .unwrap()
                .2,
            Some(7)
        );
        assert!(
            parse_local_reference("https://example.test/fhir/Patient/p1", Some(&base)).is_some()
        );
        assert!(parse_local_reference("https://other.test/fhir/Patient/p1", Some(&base)).is_none());
        assert!(parse_local_reference("#contained", Some(&base)).is_none());
    }

    #[test]
    fn walks_only_generated_reference_paths_and_marks_patient_membership() {
        let resource = json!({
            "resourceType":"Observation",
            "subject":{"reference":"Patient/p1"},
            "performer":[{"reference":"Practitioner/pr1"}],
            "note":[{"text":"Patient/not-a-reference"}]
        });
        let references = extract_references("Observation", &resource, None);
        assert!(
            references
                .iter()
                .any(|r| r.search_param_code == Some("patient") && r.target_id == "p1")
        );
        assert!(
            references
                .iter()
                .any(|r| r.search_param_code == Some("performer") && r.target_id == "pr1")
        );
        assert_eq!(references.len(), 2);
    }

    #[test]
    fn indexes_all_patient_branches_and_binary_attachment_urls() {
        let resource = json!({
            "resourceType":"DocumentReference",
            "status":"current",
            "subject":{"reference":"Patient/p-subject"},
            "content":[{"attachment":{"url":"Binary/b1"}}],
            "extension":[{
                "url":"Binary/confidential",
                "extension":[{
                    "url":"https://example.test/nested-attachment",
                    "valueAttachment":{"url":"Binary/nested"}
                }]
            }]
        });
        let references = extract_references("DocumentReference", &resource, None);
        assert!(
            references
                .iter()
                .any(|r| r.search_param_code == Some("patient") && r.target_id == "p-subject")
        );
        assert!(
            references
                .iter()
                .any(|r| r.target_type == "Binary" && r.target_id == "b1")
        );
        assert!(references.iter().all(|r| r.target_id != "confidential"));
        assert!(references.iter().any(|r| r.target_id == "nested"));
    }
}
