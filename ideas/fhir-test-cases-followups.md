# Idea - fhir-test-cases Follow-ups

- Add a validator smoke suite driven by selected `fhir-test-cases/validator/` fixtures with expected pass/fail outcomes from `manifest.json`.
- Broaden PATCH conformance by importing the upstream `fhir-test-cases/r5/patch/json-patch-tests.json` vectors instead of relying only on handwritten integration tests.
- Revisit FHIRPath-backed search extraction only if direct PostgreSQL JSON traversal becomes a limiting factor.
- Keep SQL-on-FHIR view support as a separate future feature, not part of the core server roadmap.
