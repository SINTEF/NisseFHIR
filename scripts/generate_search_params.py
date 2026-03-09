#!/usr/bin/env python3
"""Generate Rust source code for FHIR search parameter registry.

Reads the official FHIR search-parameters.json bundle and produces:
- server/src/search_params/registry.rs  — static search parameter definitions
- server/src/search_params/resource_types.rs — complete list of FHIR resource types
"""

import json
import re
import os

BASE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SEARCH_PARAMS_JSON = os.path.join(BASE, "examples", "search-parameters.json")
SCHEMA_JSON = os.path.join(BASE, "fhir.schema.json")
OUT_DIR = os.path.join(BASE, "server", "src", "search_params")

os.makedirs(OUT_DIR, exist_ok=True)


def load_resource_types():
    with open(SCHEMA_JSON) as f:
        schema = json.load(f)
    return sorted(schema["discriminator"]["mapping"].keys())


def load_search_params():
    with open(SEARCH_PARAMS_JSON) as f:
        data = json.load(f)

    registry = {}
    for entry in data.get("entry", []):
        res = entry.get("resource", {})
        code = res.get("code") or res.get("name")
        sp_type = res.get("type")
        bases = res.get("base", [])
        expression = res.get("expression")

        if not code or not sp_type:
            continue

        for base in bases:
            if base not in registry:
                registry[base] = []
            registry[base].append(
                {"code": code, "type": sp_type, "expression": expression}
            )

    return registry


def extract_json_path_segments(resource_type, expression):
    """Convert FHIRPath expression to JSON path segments for PostgreSQL."""
    if not expression:
        return None, "unsupported"

    parts = [p.strip() for p in expression.split("|")]
    relevant = None
    for part in parts:
        if part.startswith(resource_type + ".") or part.startswith(
            "(" + resource_type + "."
        ):
            relevant = part
            break

    if not relevant:
        return None, "no_match"

    # Only strip balanced outer parentheses, not chars inside the expression
    while relevant.startswith("(") and relevant.endswith(")"):
        relevant = relevant[1:-1]
    if relevant.startswith(resource_type + "."):
        path = relevant[len(resource_type) + 1 :]
    else:
        return None, "parse_error"

    # Simple field: "status", "name", "identifier"
    if re.match(r"^[a-zA-Z]+$", path):
        return [path], "simple"

    # Nested field: "code.coding", "address.city"
    if re.match(r"^[a-zA-Z]+(\.[a-zA-Z]+)+$", path):
        return path.split("."), "nested"

    # .where(system='phone') -> filter within array
    m = re.match(
        r"^([a-zA-Z.]+)\.where\(([a-zA-Z]+)='([^']+)'\)(?:\.([a-zA-Z.]+))?$",
        path,
    )
    if m:
        base_path = m.group(1).split(".")
        filter_field = m.group(2)
        filter_value = m.group(3)
        suffix = m.group(4).split(".") if m.group(4) else []
        return {
            "base": base_path,
            "filter_field": filter_field,
            "filter_value": filter_value,
            "suffix": suffix,
        }, "where_filter"

    # .ofType(X) -> polymorphic field
    m = re.match(r"^([a-zA-Z.]+)\.ofType\(([a-zA-Z]+)\)?$", path)
    if m:
        base_path = m.group(1).split(".")
        type_name = m.group(2)
        # In FHIR JSON, polymorphic fields are stored as fieldTypeName
        # e.g., value.ofType(Quantity) -> valueQuantity
        # deceased.ofType(dateTime) -> deceasedDateTime
        if len(base_path) == 1:
            poly_field = base_path[0] + type_name[0].upper() + type_name[1:]
            return [poly_field], "ofType"
        else:
            last = base_path[-1]
            poly_field = last + type_name[0].upper() + type_name[1:]
            return base_path[:-1] + [poly_field], "ofType"

    # .exists() patterns
    if ".exists()" in path:
        base = path.split(".exists()")[0]
        return base.split("."), "exists"

    # subject.where(resolve() is Patient) -> just subject.reference
    if "where(resolve()" in path:
        base = path.split(".where(")[0]
        return base.split(".") + ["reference"], "resolve_filter"

    # relatedArtifact.where(type='X').resource
    m = re.match(
        r"^([a-zA-Z]+)\.where\(([a-zA-Z]+)='([^']+)'\)\.([a-zA-Z.]+)$", path
    )
    if m:
        return {
            "base": [m.group(1)],
            "filter_field": m.group(2),
            "filter_value": m.group(3),
            "suffix": m.group(4).split("."),
        }, "where_filter"

    # repeat() patterns - treat as array navigation
    m = re.match(r"^repeat\(([a-zA-Z]+)\)(?:\.([a-zA-Z.]+))?$", path)
    if m:
        base = m.group(1)
        suffix = m.group(2).split(".") if m.group(2) else []
        return [base] + suffix, "repeat"

    # 'subject as X' patterns
    m = re.match(r"^([a-zA-Z]+)\s+as\s+(\w+)$", path)
    if m:
        field = m.group(1)
        type_name = m.group(2)
        poly_field = field + type_name[0].upper() + type_name[1:]
        return [poly_field], "as_type"

    # entry[0].resource as X
    if "entry[0]" in path:
        return None, "unsupported"

    return None, "unsupported"


def sp_type_to_rust(sp_type):
    mapping = {
        "string": "String",
        "token": "Token",
        "reference": "Reference",
        "date": "Date",
        "quantity": "Quantity",
        "number": "Number",
        "uri": "Uri",
        "composite": "Composite",
        "special": "Special",
        "resource": "Resource",
    }
    return mapping.get(sp_type, "Special")


def generate_resource_types_rs(resource_types):
    lines = [
        "//! Complete list of FHIR R6 resource types.",
        "//!",
        "//! Auto-generated from fhir.schema.json — do not edit manually.",
        "",
        "/// All 127 FHIR R6 resource types.",
        "pub const RESOURCE_TYPES: &[&str] = &[",
    ]
    for rt in resource_types:
        lines.append(f'    "{rt}",')
    lines.append("];")
    lines.append("")
    lines.append("/// Check if a string is a valid FHIR resource type (case-sensitive).")
    lines.append("pub fn is_valid_resource_type(name: &str) -> bool {")
    lines.append("    RESOURCE_TYPES.binary_search(&name).is_ok()")
    lines.append("}")
    lines.append("")

    return "\n".join(lines)


def json_path_to_rust_expr(segments):
    """Convert JSON path segments to a Rust expression for JSONB access."""
    if isinstance(segments, list):
        parts = []
        for seg in segments:
            parts.append(f'"{seg}"')
        return f"&[{', '.join(parts)}]"
    return None


def generate_registry_rs(resource_types, registry):
    """Generate the search parameter registry Rust source."""
    lines = [
        "//! FHIR search parameter registry.",
        "//!",
        "//! Auto-generated from search-parameters.json — do not edit manually.",
        "//!",
        "//! Each search parameter maps a code (e.g. `name`, `status`, `identifier`)",
        "//! to a search type and a JSON path within the resource document.",
        "",
        "/// Search parameter type as defined by FHIR.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub enum SearchParamType {",
        "    String,",
        "    Token,",
        "    Reference,",
        "    Date,",
        "    Quantity,",
        "    Number,",
        "    Uri,",
        "    Composite,",
        "    Special,",
        "}",
        "",
        "/// How to extract the search value from the JSONB resource document.",
        "#[derive(Debug, Clone)]",
        "pub enum JsonPath {",
        "    /// Simple path segments: resource->'field' or resource->'field'->'subfield'",
        "    Field(&'static [&'static str]),",
        "    /// Array field with a filter: e.g. telecom.where(system='phone')",
        "    WhereFilter {",
        "        base: &'static [&'static str],",
        "        filter_field: &'static str,",
        "        filter_value: &'static str,",
        "        suffix: &'static [&'static str],",
        "    },",
        "    /// Existence check (e.g. deceased.exists())",
        "    Exists(&'static [&'static str]),",
        "}",
        "",
        "/// A single search parameter definition.",
        "#[derive(Debug, Clone)]",
        "pub struct SearchParam {",
        "    pub code: &'static str,",
        "    pub param_type: SearchParamType,",
        "    pub path: JsonPath,",
        "}",
        "",
        "/// Look up the search parameters defined for a given resource type.",
        "///",
        "/// Returns an empty slice for unknown resource types or those without",
        "/// specific search parameters.",
        "pub fn search_params_for(resource_type: &str) -> &'static [SearchParam] {",
        "    match resource_type {",
    ]

    # Separate out Resource-level (common) params
    common_params = registry.pop("Resource", [])
    domain_params = registry.pop("DomainResource", [])

    for rt in resource_types:
        rt_params = registry.get(rt, [])
        if not rt_params:
            continue

        var_name = f"PARAMS_{rt.upper()}"
        lines.append(f'        "{rt}" => &{var_name},')

    lines.append("        _ => &[],")
    lines.append("    }")
    lines.append("}")
    lines.append("")

    # Generate the static arrays for each resource type
    for rt in resource_types:
        rt_params = registry.get(rt, [])
        if not rt_params:
            continue

        var_name = f"PARAMS_{rt.upper()}"
        supported_params = []

        for p in rt_params:
            path_info, category = extract_json_path_segments(rt, p.get("expression"))

            if path_info is None:
                continue

            if p["type"] in ("composite", "special", "resource"):
                # Skip composite/special/resource types for now
                continue

            rust_type = sp_type_to_rust(p["type"])

            if category in ("simple", "nested", "ofType", "as_type", "repeat"):
                path_expr = json_path_to_rust_expr(path_info)
                if path_expr:
                    supported_params.append(
                        f'    SearchParam {{ code: "{p["code"]}", param_type: SearchParamType::{rust_type}, path: JsonPath::Field({path_expr}) }}'
                    )
            elif category == "where_filter" and isinstance(path_info, dict):
                base_expr = json_path_to_rust_expr(path_info["base"])
                suffix_expr = json_path_to_rust_expr(path_info["suffix"])
                if base_expr and suffix_expr:
                    supported_params.append(
                        f'    SearchParam {{ code: "{p["code"]}", param_type: SearchParamType::{rust_type}, path: JsonPath::WhereFilter {{ base: {base_expr}, filter_field: "{path_info["filter_field"]}", filter_value: "{path_info["filter_value"]}", suffix: {suffix_expr} }} }}'
                    )
            elif category == "exists":
                path_expr = json_path_to_rust_expr(path_info)
                if path_expr:
                    supported_params.append(
                        f'    SearchParam {{ code: "{p["code"]}", param_type: SearchParamType::{rust_type}, path: JsonPath::Exists({path_expr}) }}'
                    )
            elif category == "resolve_filter":
                path_expr = json_path_to_rust_expr(path_info)
                if path_expr:
                    supported_params.append(
                        f'    SearchParam {{ code: "{p["code"]}", param_type: SearchParamType::{rust_type}, path: JsonPath::Field({path_expr}) }}'
                    )

        if supported_params:
            lines.append(f"static {var_name}: [SearchParam; {len(supported_params)}] = [")
            for param_line in supported_params:
                lines.append(f"{param_line},")
            lines.append("];")
            lines.append("")

    return "\n".join(lines)


def generate_mod_rs():
    return """//! FHIR search parameter support.
//!
//! This module provides:
//! - A complete list of FHIR R6 resource types
//! - A registry of search parameters per resource type
//! - SQL query generation for search filters

pub mod registry;
pub mod resource_types;
pub mod sql;
"""


def main():
    resource_types = load_resource_types()
    registry = load_search_params()

    # Generate resource_types.rs
    rt_code = generate_resource_types_rs(resource_types)
    with open(os.path.join(OUT_DIR, "resource_types.rs"), "w") as f:
        f.write(rt_code)
    print(f"Generated resource_types.rs with {len(resource_types)} types")

    # Generate registry.rs
    reg_code = generate_registry_rs(resource_types, registry)
    with open(os.path.join(OUT_DIR, "registry.rs"), "w") as f:
        f.write(reg_code)

    # Count supported params
    supported = reg_code.count("SearchParam {")
    print(f"Generated registry.rs with ~{supported} search parameters")

    # Generate mod.rs
    mod_code = generate_mod_rs()
    with open(os.path.join(OUT_DIR, "mod.rs"), "w") as f:
        f.write(mod_code)
    print("Generated mod.rs")


if __name__ == "__main__":
    main()
