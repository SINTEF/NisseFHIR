# Prompts

Prompts as requested by the human.

---
## VsCode auto/autopilot - 5.3 Codex

Good morning. I would like you to start working as stated in #file:fhir-specs.md

Thank you very much.

---
## Vscode 5.4/autopilot - 5.4

Can you please continue the good work on this project folliwing #file:fhir-specs.md ?

The previous developer finished with the following text:
```
Implemented the first working milestone of the project and validated it end-to-end.

What I added:

Bootstrapped a Rust server crate with Axum + Tower HTTP + SQLx in Cargo.toml.
Implemented startup/config, DB connection, and migrations in main.rs.
Added JWT-based tenant extraction and simple scope/resource allow-list checks in auth.rs.
Added health and CapabilityStatement endpoints, plus basic FHIR create/read/update routes:
fhir.rs
capability.rs
Added PostgreSQL JSONB storage layer in store.rs.
Added initial migration for resource storage/indexing in 0001_create_fhir_resources.sql.
Added project docs and progress notes:
README.md
initial_server_bootstrap.md
fhir_validation_strategy.md
Validation status:

Test suite passes: 5 passed, 0 failed.
Main tested units include auth token parsing, capability output shape, payload validation, and router health endpoint behavior.
Important note:

Full FHIR JSON Schema validation is intentionally not wired yet in this first slice; it is tracked as the top remaining task in initial_server_bootstrap.md.
Natural next steps:

Implement strict schema validation using fhir.schema.json and return FHIR OperationOutcome-style errors.
Add search endpoint support (resource-type listing/filter baseline).
Add integration tests against disposable PostgreSQL for create/read/update flows.
Completed the initial FHIR server milestone: scaffolded a Rust Axum/SQLx service with PostgreSQL migrations, JWT tenant-aware auth, CapabilityStatement and health endpoints, basic FHIR create/read/update routes, project/task documentation, and a passing test suite (5/5).

Initial implementation is fully completed and validated: Rust FHIR server scaffold, auth, storage, core endpoints, migration, docs/task notes, and passing tests are all in place.
```