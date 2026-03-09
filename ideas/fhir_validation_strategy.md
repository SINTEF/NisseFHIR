# Idea - FHIR JSON Validation Strategy

- Implemented baseline: `jsonschema` crate with cached per-resource validators built from the bundled draft-06 schema.
- Implemented baseline: FHIR `OperationOutcome` responses for malformed JSON and schema validation failures.
- Implemented next slice: a schema-driven secondary validation layer for primitive datatype semantics the JSON Schema does not fully enforce, currently covering calendar-valid `date`/`dateTime`/`instant` and integer-family constraints for `integer`, `integer64`, `positiveInt`, and `unsignedInt`.
- Next idea: extend the secondary validation layer to additional primitive semantics where the schema is intentionally permissive or technically insufficient, especially `decimal`, `uri`/`url`/`canonical`, and string length limits.
- Next idea: add targeted complex datatype invariants from the FHIR spec, starting with `Period`, `Quantity`, `Attachment`, `ContactPoint`, and `Timing`.
- Next idea: translate common schema error paths into more human-readable FHIR element diagnostics.
