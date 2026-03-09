# Initial Server Bootstrap - Remaining Tasks

## Completed

- Created Rust server crate (`server/`)
- Added Axum + Tower HTTP stack
- Added PostgreSQL storage schema and migration
- Added JWT tenant extraction with optional unauthenticated mode
- Implemented `healthz`, `metadata`, and basic create/read/update resource routes
- Added initial tests around auth, payload validation, capability statement, and health endpoint wiring

## Remaining (near-term)

1. Wire full JSON Schema validation against `fhir.schema.json`.
2. Add OperationOutcome error bodies for stricter FHIR conformance.
3. Add search (`GET /fhir/:resource_type`) and conditional updates.
4. Add E2E tests using disposable Postgres (docker-compose or test container).
5. Harden capability statement details per implemented behavior.
6. Add tenant-aware audit logging and request ids.
