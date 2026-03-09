# Initial Server Bootstrap - Remaining Tasks

## Completed

- Created Rust server crate (`server/`)
- Added Axum + Tower HTTP stack
- Added PostgreSQL storage schema and migration
- Added JWT tenant extraction with optional unauthenticated mode
- Implemented `healthz`, `metadata`, and basic create/read/update resource routes
- Added initial tests around auth, payload validation, capability statement, and health endpoint wiring
- Wired full JSON Schema validation against the bundled `fhir.schema.json`
- Returned FHIR `OperationOutcome` bodies for malformed JSON and request/auth failures
- Added tests for schema validation failures and malformed JSON responses
- Added baseline collection search (`GET /fhir/:resource_type`) returning paged FHIR `searchset` Bundles
- Added integration tests for search pagination, tenant isolation, unauthenticated mode, and auth restrictions

## Remaining (near-term)

1. Add richer search behavior beyond paging, starting with selective filtering and stable FHIR pagination links.
2. Add E2E tests using disposable Postgres for create/read/update/search flows and tenant isolation.
3. Harden capability statement details so it advertises current auth behavior more precisely.
4. Add tenant-aware audit logging and request ids.
5. Decide whether not-found and internal failures should expose richer OperationOutcome diagnostics or remain intentionally generic.
