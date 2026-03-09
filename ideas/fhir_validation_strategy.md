# Idea - FHIR JSON Validation Strategy

- Use `jsonschema` crate with a startup-compiled validator.
- Cache resource-type schema pointers for faster per-request validation.
- Return FHIR `OperationOutcome` with path-specific diagnostics.
- Consider secondary validation layer for common profile constraints.
