# Test Overview

## Current Coverage

Code coverage: **93.04% line**, **91.21% region** (via `cargo llvm-cov`).

### Unit Tests (183 tests in `src/`)

- `auth::tests` — JWT validation, scope parsing, resource allow-list case sensitivity, tenant claim precedence, missing-exp rejection, malformed bearer rejection, JWKS missing/unknown kid rejection, dev-mode self-verification.
- `capability::tests` — Capability statement shape: resource type, FHIR version, status, server mode, supported interactions (create/read/update/delete/patch/search-type), generic search parameters, Patient/Observation resource-specific search parameters, patchFormat, implementation URL, JSON-only format, conditionalCreate advertised, `application/fhir+json` format listed.
- `config::tests` — Environment parsing and validation for auth/search/http settings, including hardened database timeout controls (`DB_CONNECT_TIMEOUT_SECS`, `DB_ACQUIRE_TIMEOUT_SECS`, `DB_STATEMENT_TIMEOUT_MS`) and rejection of unsafe zero values.
- `error::tests` — OperationOutcome mapping for all error variants (400/401/403/404/409/412/413/500), issue severity and codes.
- `fhir::tests` — ID generation for create, resourceType mismatch rejection, unsupported path resource type rejection, search bundle cursor-link structure, resource-specific search parameter parsing, identifier filter parsing, schema validation OperationOutcome, malformed JSON OperationOutcome.
- `search_params::sql::tests` (54 tests) — Comprehensive SQL-generation coverage for all search parameter types: string filters (single/nested/deep/WhereFilter/noop), reference filters (single/nested with and without reference suffix/WhereFilter/noop), date filters (single/nested/WhereFilter), URI filters (single/noop), number filters, quantity filters (value-only/full system+code/code-only), exists filters (true/false/other), token filters (identifier with/without system, nested 2-segment/deep, single-field CodeableConcept/array-of-CodeableConcept, WhereFilter with/without suffix), special/position filters, `push_search_filters` integration with multiple params, composite noop, `safe_array_elements` helper, geospatial `near` parsing and SQL generation, and regression coverage ensuring CodeableConcept token searches use containment-based (`@>`) predicates.
- `validation::tests` — Schema validator plus secondary datatype checks: accepts minimal and named Patient, minimal Observation with any code string for status, integer boundary values, and fragment canonical values; rejects unknown resource types, additional properties, wrong types, invalid calendar dates/dateTimes, fractional or out-of-range integer fields, invalid positiveInt/unsignedInt values, overflowing integer64 strings, invalid `uri`/`url`/`canonical` values, `ContactPoint.value` without `system`, `Attachment.data` without `contentType`, `Quantity.code` without `system`, and `Period.start > end`. Validator caching and multi-type support.

### Integration Tests (165 tests in `tests/`)

- `auth.rs` — Missing/expired/wrong-secret/missing-exp tokens rejected, read-only/write-only scope enforcement on create/read/PUT, resource type restrictions on create/read, tenant isolation and same-ID-different-tenant coexistence, tenant claim precedence over sub.
- `bundle.rs` (16 tests) — Transaction Bundle: atomic multi-resource create, rollback on failure (patient not persisted when later entry fails), PUT update through transaction, DELETE through transaction, GET read within transaction, mixed operations (create + update + delete in one transaction), schema validation enforcement. Batch Bundle: independent multi-resource create, partial failure continues processing (failed entries reported inline with OperationOutcome, successful entries persist). Validation: rejects non-Bundle resourceType, rejects unsupported Bundle types (e.g. searchset), requires write scope, empty entries return empty response for both transaction and batch, missing request field rejected, response entries include ETag/Location/lastModified metadata.
- `conditional_create.rs` (5 tests) — `If-None-Exist` conditional create: no match creates resource (201), single match returns existing (200), multiple matches returns 412 Precondition Failed, empty header returns 400, create without header works normally.
- `content_type.rs` (6 tests) — `application/fhir+json` content type: FHIR create responses use `application/fhir+json`, read responses use `application/fhir+json`, search responses use `application/fhir+json`, metadata responses use `application/fhir+json`, healthz does NOT use `application/fhir+json`, server accepts `application/fhir+json` content-type on create.
- `crud.rs` (20 tests) — Create returns 201 with correct headers (ETag, Last-Modified, Location), ID generation, database persistence, initial version=1, read-after-create roundtrip, ETag/Last-Modified on read, 404 for nonexistent/wrong-type, update returns 200 with incremented version, mismatched ID/type rejection, non-upsert `PUT` semantics, optional `If-Match` compatibility, stale `If-Match` returns 412, multi-resource-type roundtrip, field preservation on update, healthz, metadata endpoint, double-create upsert.
- `delete.rs` (8 tests) — Delete returns 204, nonexistent returns 404, deleted resource no longer readable, count reduction after delete, write scope required, resource type restriction enforced, tenant isolation on delete, unauthenticated rejection when auth required.
- `history.rs` (7 tests) — `GET /fhir/{type}/{id}/_history` bundle shape and descending version order, self link and request URL metadata, delete tombstone (`410 Gone`) entries, read-scope enforcement, unauthenticated rejection, resource restriction enforcement, tenant isolation, and 404 for missing resources.
- `http_config.rs` (8 tests) — Docs route disabled by default, can be enabled, CORS allows only configured origin, CORS rejects unconfigured origin, dev token endpoint mints valid tokens, dev token defaults on empty body, dev token endpoint hidden in static mode, dev-minted tokens authenticate requests.
- `patch.rs` (11 tests) — PATCH add/replace/remove field operations, 404 for nonexistent resource, 400 for invalid patch ops, version increment on patch, rejection of resourceType change, write scope required, resource type restriction enforced, optional `If-Match` compatibility, stale `If-Match` returns 412, read-after-patch roundtrip, schema validation of patched result.
- `search.rs` — Searchset bundle shape and total, cursor pagination with `_count`/`_after_id` and next links, multi-page filtered traversal across a moderate generated dataset, tenant isolation, forbidden resource type, `_count` above limit rejected, legacy `_offset` rejection, Patient search by `name`/`birthdate`/`identifier`, Observation search by `code`/`status`/`subject`, filtered pagination links, unsupported parameter rejection, malformed identifier rejection.
- `search_extended.rs` (10 tests) — Condition search by `clinical-status`, `code` (SNOMED), `patient` (subject reference), `category`, `onset-date`, and no-match empty bundle. Encounter search by `status`, `subject`, `type` (SNOMED CodeableConcept), and no-match empty bundle.
- `validation.rs` (31 tests) — Acceptance of comprehensive Patient/Observation/Organization/Practitioner/Encounter/Condition/Procedure/DiagnosticReport examples. Rejection of extra properties, invalid types, unsupported resource types, invalid path-only resource types, missing resourceType, type mismatch, malformed/truncated/empty JSON, invalid calendar birth dates, invalid positiveInt extension values, invalid identifier URIs, `ContactPoint.value` without `system`, `Quantity.code` without `system`, `Period.start > end`, case-insensitive path matching, multiple validation errors, diagnostics content.

### External E2E Harness

- `scripts/e2e_examples.py` — Starts real infrastructure, downloads HL7 example data if needed, launches the server in native or Docker mode, then scans both the local `examples/` directory and `references/fhir-test-cases/r5/examples/` by parsing each file's top-level `resourceType`. It validates all resource-bearing JSON files in parallel over real HTTP, classifying each as accepted, schema-invalid, unsupported resource type, or payload-too-large for oversized payloads, and can finish with a CRUD smoke flow.

Current full-scan baseline:

- 2410 example files scanned.
- 2392 accepted by the current server and bundled schema.
- 17 rejected with valid `OperationOutcome` schema errors.
- 0 unsupported resource types in the current archive against the bundled FHIR schema.
- 1 transport-limited oversized bundle (`profiles-resources.json`, ~45 MB).

## What Is Missing

- Conditional delete (If-Match headers) and conditional-update-by-search.
- Search parameters beyond the current slice. Patient supports `name`, `birthdate`, `identifier`; Observation supports `code`, `status`, `subject`; Condition supports `clinical-status`, `code`, `patient`, `category`, `onset-date`; Encounter supports `status`, `subject`, `type`. Other resource-specific filters remain unimplemented.
- Performance regression tests as part of CI.
- Query-plan and index assertions for larger cursor-paginated datasets.
- ND-JSON bulk data export.
- Automated CI execution of the external E2E harness in both native and Docker modes.
- Broader FHIR datatype and invariant coverage beyond the current second-pass slice, especially decimal precision and range checks, string length limits, richer URI/canonical rules where the schema is inlined in more places, and additional complex datatype invariants such as `Timing`, `SampledData`, `Range`, and `Ratio`.

## Recommended Next Steps

1. Add remaining conditional interaction support (If-Match on delete, and conditional update/delete by search criteria).
2. Expand search support to more resource types and closer FHIR semantics where needed.
3. Wire the external Python E2E harness into CI once the pipeline is added.
4. Add ND-JSON bulk data export.
