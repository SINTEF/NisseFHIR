# Idea - FHIR JSON Validation Strategy

- Extend the secondary validation layer to additional primitive semantics where the schema is intentionally permissive or technically insufficient, especially `decimal` and string length limits.
- Add the next batch of complex datatype invariants from the FHIR spec, starting with `Timing`, `SampledData`, `Range`, and `Ratio`.
- Translate common schema error paths into more human-readable FHIR element diagnostics.
