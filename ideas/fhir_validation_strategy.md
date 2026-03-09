# Idea - FHIR JSON Validation Strategy

- Implemented baseline: `jsonschema` crate with cached per-resource validators built from the bundled draft-06 schema.
- Implemented baseline: FHIR `OperationOutcome` responses for malformed JSON and schema validation failures.
- Implemented next slice: a schema-driven secondary validation layer for primitive datatype semantics the JSON Schema does not fully enforce, currently covering calendar-valid `date`/`dateTime`/`instant`, integer-family constraints for `integer`, `integer64`, `positiveInt`, and `unsignedInt`, plus RFC-aware `uri`/`url`/`canonical` parsing.
- Implemented next slice: targeted complex datatype invariants from the FHIR spec for `Period`, `Quantity`, `Attachment`, and `ContactPoint`.
- Next idea: extend the secondary validation layer to additional primitive semantics where the schema is intentionally permissive or technically insufficient, especially `decimal` and string length limits.
- Next idea: add the next batch of complex datatype invariants from the FHIR spec, starting with `Timing`, `SampledData`, `Range`, and `Ratio`.
- Next idea: translate common schema error paths into more human-readable FHIR element diagnostics.
