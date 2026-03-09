# Initial Server Bootstrap - Remaining Tasks

## Completed

- Created Rust server crate (`server/`)
- Added Axum + Tower HTTP stack
- Added PostgreSQL storage schema and migration with LIST partitioning by resource type
- Added JWT tenant extraction with optional unauthenticated mode
- Implemented `healthz`, `metadata`, and CRUD (create/read/update/delete) resource routes
- Added JSON Schema validation against the bundled `fhir.schema.json` with per-resource-type validator caching
- Returned FHIR `OperationOutcome` bodies for all error responses
- Added baseline collection search (`GET /fhir/:resource_type`) returning paged FHIR `searchset` Bundles
- Comprehensive test suites: 42 unit tests + 78 integration tests (120 total, all passing)
- Capability statement with security metadata (JWT, CORS, scopes)
- Dockerfile for containerized deployment
- All dependencies up to date (verified with cargo-outdated)
- Clippy clean with no warnings
- Performance verified: ~1ms reads, ~4ms creates, ~2ms searches

## Remaining (near-term)

1. Add resource-specific search parameters (e.g., `Patient?name=Smith`, `Observation?code=...`).
2. Add history endpoint for reading previous versions of a resource.
3. Add conditional interactions (If-Match on update/delete, If-None-Exist on create).
4. Add transaction/batch Bundle processing.
5. Add ND-JSON bulk data export support.
6. Add CI pipeline with disposable PostgreSQL.
