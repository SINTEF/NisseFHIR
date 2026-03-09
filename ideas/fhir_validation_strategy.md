# Idea - FHIR JSON Validation Strategy

- Implemented baseline: `jsonschema` crate with cached per-resource validators built from the bundled draft-06 schema.
- Implemented baseline: FHIR `OperationOutcome` responses for malformed JSON and schema validation failures.
- Next idea: add a secondary validation layer for FHIR invariants and profile constraints that are not expressible in JSON Schema alone.
- Next idea: translate common schema error paths into more human-readable FHIR element diagnostics.
