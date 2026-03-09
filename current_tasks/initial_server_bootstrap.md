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
- Added first-pass resource-specific search parameters: Patient (`name`, `birthdate`, `identifier`) and Observation (`code`, `status`, `subject`)
- Comprehensive test suites: 47 unit tests + 96 integration tests (143 total)
- Capability statement with security metadata (JWT, CORS, scopes), patch interaction, patchFormat
- Dockerfile for containerized deployment
- Python E2E harness that boots native or Docker deployments, prefers local PostgreSQL for native runs, scans both the HL7 `examples/` directory and `fhir-test-cases/r5/examples/` in parallel by inferred `resourceType`, classifies accepted/invalid/unsupported/payload-too-large outcomes, and runs real CRUD/search checks over HTTP
- All dependencies up to date (verified with cargo-outdated)
- Clippy clean with no warnings
- Performance verified: ~1ms reads, ~4ms creates, ~2ms searches
- OpenAPI documentation via utoipa + utoipa-scalar, served at `/docs`
- Security headers via tower-helmet
- Request body size limit (50 MB) with HTTP 413 + OperationOutcome for oversized payloads
- JSON Patch support (RFC 6902) via `PATCH /fhir/:type/:id` with 11 integration tests

## Remaining (near-term)

1. Add history endpoint for reading previous versions of a resource.
2. Add conditional interactions (If-Match on update/delete, If-None-Exist on create).
3. Add transaction/batch Bundle processing.
4. Expand search support to additional resource types and closer FHIR semantics where needed.
5. Add ND-JSON bulk data export support.
6. Add CI pipeline with disposable PostgreSQL and run the Python E2E harness in both native and Docker modes.
