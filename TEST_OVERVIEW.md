# Test Overview

## Current Coverage

### Unit Tests (66 tests in `server/src/`)

- `auth::tests` — JWT validation, scope parsing, resource allow-list case sensitivity, tenant claim precedence, missing-exp rejection, malformed bearer rejection, JWKS missing/unknown kid rejection, dev-mode self-verification.
- `capability::tests` — Capability statement shape: resource type, FHIR version, status, server mode, supported interactions (create/read/update/delete/patch/search-type), generic search parameters, Patient/Observation resource-specific search parameters, patchFormat, implementation URL, JSON-only format.
- `error::tests` — OperationOutcome mapping for all error variants (400/401/403/404/413/500), issue severity and codes.
- `fhir::tests` — ID generation for create, resourceType mismatch rejection, search bundle link/pagination structure, resource-specific search parameter parsing, identifier filter parsing, schema validation OperationOutcome, malformed JSON OperationOutcome.
- `validation::tests` — Schema validator plus secondary datatype checks: accepts minimal and named Patient, minimal Observation with any code string for status, integer boundary values, and fragment canonical values; rejects unknown resource types, additional properties, wrong types, invalid calendar dates/dateTimes, fractional or out-of-range integer fields, invalid positiveInt/unsignedInt values, overflowing integer64 strings, invalid `uri`/`url`/`canonical` values, `ContactPoint.value` without `system`, `Attachment.data` without `contentType`, `Quantity.code` without `system`, and `Period.start > end`. Validator caching and multi-type support.

### Integration Tests (108 tests in `server/tests/`)

- `auth.rs` — Missing/expired/wrong-secret/missing-exp tokens rejected, read-only/write-only scope enforcement on create/read/PUT, resource type restrictions on create/read, tenant isolation and same-ID-different-tenant coexistence, tenant claim precedence over sub.
- `crud.rs` (20 tests) — Create returns 201 with correct headers (ETag, Last-Modified, Location), ID generation, database persistence, initial version=1, read-after-create roundtrip, ETag/Last-Modified on read, 404 for nonexistent/wrong-type, update returns 200 with incremented version, mismatched ID/type rejection, multi-resource-type roundtrip, field preservation on update, healthz, metadata endpoint, double-create upsert.
- `delete.rs` (8 tests) — Delete returns 204, nonexistent returns 404, deleted resource no longer readable, count reduction after delete, write scope required, resource type restriction enforced, tenant isolation on delete, unauthenticated rejection when auth required.
- `http_config.rs` (8 tests) — Docs route disabled by default, can be enabled, CORS allows only configured origin, CORS rejects unconfigured origin, dev token endpoint mints valid tokens, dev token defaults on empty body, dev token endpoint hidden in static mode, dev-minted tokens authenticate requests.
- `patch.rs` (11 tests) — PATCH add/replace/remove field operations, 404 for nonexistent resource, 400 for invalid patch ops, version increment on patch, rejection of resourceType change, write scope required, resource type restriction enforced, read-after-patch roundtrip, schema validation of patched result.
- `search.rs` — Searchset bundle shape and total, pagination with _count/_offset and next links, tenant isolation, forbidden resource type, _count above limit rejected, Patient search by `name`/`birthdate`/`identifier`, Observation search by `code`/`status`/`subject`, filtered pagination links, unsupported parameter rejection, malformed identifier rejection.
- `validation.rs` (30 tests) — Acceptance of comprehensive Patient/Observation/Organization/Practitioner/Encounter/Condition/Procedure/DiagnosticReport examples. Rejection of extra properties, invalid types, unsupported resource types, missing resourceType, type mismatch, malformed/truncated/empty JSON, invalid calendar birth dates, invalid positiveInt extension values, invalid identifier URIs, `ContactPoint.value` without `system`, `Quantity.code` without `system`, `Period.start > end`, case-insensitive path matching, multiple validation errors, diagnostics content.

### External E2E Harness

- `scripts/e2e_examples.py` — Starts real infrastructure, downloads HL7 example data if needed, launches the server in native or Docker mode, then scans both the local `examples/` directory and `fhir-test-cases/r5/examples/` by parsing each file's top-level `resourceType`. It validates all resource-bearing JSON files in parallel over real HTTP, classifying each as accepted, schema-invalid, unsupported resource type, or payload-too-large for oversized payloads, and can finish with a CRUD smoke flow.

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
- Search parameters beyond the current first slice. Patient now supports `name`, `birthdate`, and `identifier`; Observation now supports `code`, `status`, and `subject`. Other resource-specific filters remain unimplemented.
- Performance regression tests as part of CI.
- ND-JSON bulk data export.
- Automated CI execution of the external E2E harness in both native and Docker modes.
- Broader FHIR datatype and invariant coverage beyond the current second-pass slice, especially decimal precision and range checks, string length limits, richer URI/canonical rules where the schema is inlined in more places, and additional complex datatype invariants such as `Timing`, `SampledData`, `Range`, and `Ratio`.

## Recommended Next Steps

1. Add history endpoint (`GET /fhir/{type}/{id}/_history`) with version tracking.
2. Add conditional interaction support (If-Match on update/delete).
3. Add transaction/batch Bundle endpoint.
4. Expand search support to more resource types and closer FHIR semantics where needed.
5. Wire the external Python E2E harness into CI once the pipeline is added.
