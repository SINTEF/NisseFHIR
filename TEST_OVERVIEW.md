# Test Overview

## Current Coverage

### Unit Tests (42 tests in `server/src/`)

- `auth::tests` — JWT validation, scope parsing, resource allow-list case sensitivity, tenant claim precedence, unauthenticated mode fallback, malformed bearer rejection.
- `capability::tests` — Capability statement shape: resource type, FHIR version, status, server mode, supported interactions (create/read/update/delete/search-type), search parameters, implementation URL, JSON-only format.
- `error::tests` — OperationOutcome mapping for all error variants (400/401/403/404/500), issue severity and codes.
- `fhir::tests` — ID generation for create, resourceType mismatch rejection, search bundle link/pagination structure, schema validation OperationOutcome, malformed JSON OperationOutcome.
- `validation::tests` — Schema validator: accepts minimal and named Patient, minimal Observation with any code string for status, rejects unknown resource types, additional properties, wrong types. Validator caching and multi-type support.

### Integration Tests (78 tests in `server/tests/`)

- `auth.rs` (20 tests) — Unauthenticated mode allows/denies correctly, expired/wrong-secret/missing tokens rejected, read-only/write-only scope enforcement on create/read/PUT, resource type restrictions on create/read, tenant isolation and same-ID-different-tenant coexistence, tenant claim precedence over sub.
- `crud.rs` (20 tests) — Create returns 201 with correct headers (ETag, Last-Modified, Location), ID generation, database persistence, initial version=1, read-after-create roundtrip, ETag/Last-Modified on read, 404 for nonexistent/wrong-type, update returns 200 with incremented version, mismatched ID/type rejection, multi-resource-type roundtrip, field preservation on update, healthz, metadata endpoint, double-create upsert.
- `delete.rs` (8 tests) — Delete returns 204, nonexistent returns 404, deleted resource no longer readable, count reduction after delete, write scope required, resource type restriction enforced, tenant isolation on delete, unauthenticated rejection when auth required.
- `search.rs` (6 tests) — Searchset bundle shape and total, pagination with _count/_offset and next links, tenant isolation, forbidden resource type, unauthenticated mode uses public tenant, _count above limit rejected.
- `validation.rs` (24 tests) — Acceptance of comprehensive Patient/Observation/Organization/Practitioner/Encounter/Condition/Procedure/DiagnosticReport examples. Rejection of extra properties, invalid types, unsupported resource types, missing resourceType, type mismatch, malformed/truncated/empty JSON, case-insensitive path matching, multiple validation errors, diagnostics content.

### External E2E Harness

- `scripts/e2e_examples.py` — Starts real infrastructure, downloads HL7 example data if needed, launches the server in native or Docker mode, then scans the full `examples/` directory by parsing each file’s top-level `resourceType`. It validates all resource-bearing JSON files in parallel over real HTTP, classifying each as accepted, schema-invalid, unsupported resource type, or transport-limited for oversized payloads, and can finish with a CRUD smoke flow.

Current full-scan baseline:

- 2410 example files scanned.
- 2392 accepted by the current server and bundled schema.
- 17 rejected with valid `OperationOutcome` schema errors.
- 0 unsupported resource types in the current archive against the bundled FHIR schema.
- 1 transport-limited oversized bundle (`profiles-resources.json`, ~45 MB).

## What Is Missing

- History interaction (read previous versions of a resource).
- Conditional create/update/delete (If-None-Exist, If-Match headers).
- Transaction/batch Bundle processing.
- Search parameters beyond `_count` and `_offset` (e.g., resource-specific filtering like `Patient?name=Smith`).
- Performance regression tests as part of CI.
- ND-JSON bulk data export.
- Automated CI execution of the external E2E harness in both native and Docker modes.

## Recommended Next Steps

1. Add resource-specific search parameters starting with Patient (name, birthdate, identifier) and Observation (code, status, subject).
2. Add history endpoint (`GET /fhir/{type}/{id}/_history`) with version tracking.
3. Add conditional interaction support (If-Match on update/delete).
4. Add transaction/batch Bundle endpoint.
5. Wire the external Python E2E harness into CI once the pipeline is added.
