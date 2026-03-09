# Core Roadmap

- Add remaining conditional interactions: `If-None-Exist` on create, `If-Match` on delete, and conditional update/delete by search criteria.
- Expand search support to more resource types and closer FHIR semantics where that materially improves interoperability.
- Add ND-JSON bulk data export.
- Run the external E2E harness in CI for both native and Docker modes.
- Extend secondary validation with more primitive and complex datatype invariants that the JSON Schema does not fully enforce.
