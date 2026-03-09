# Next Steps: Leverage fhir-test-cases

These are concrete tasks derived from analyzing the `fhir-test-cases` submodule.

## 1. Run R5 Examples Through E2E Harness

**Priority:** High
**Effort:** Small

Point `scripts/e2e_examples.py` at `fhir-test-cases/r5/examples/` (in addition to `examples/`). Run the harness and see which R5 JSON examples pass our schema validation and CRUD cycle. Document results — failures are either R5→R6 schema differences (expected) or bugs in our validation (fix).

## 2. Validator Smoke Test with Known-Good and Known-Bad Files

**Priority:** High
**Effort:** Medium

Cherry-pick ~30 JSON files from `fhir-test-cases/validator/` with known expected outcomes from `manifest.json`:
- Files expected to have 0 errors → should pass our validation
- Files expected to have errors → should fail our validation

Write a simple test or script that checks our `jsonschema` validator against these files. This gives confidence that validation is working correctly beyond just our own examples.

## 3. Implement PATCH Endpoint with Test Vectors

**Priority:** Medium
**Effort:** Medium

Implement `PATCH /fhir/{type}/{id}` supporting RFC 6902 JSON Patch. Use `fhir-test-cases/r5/patch/json-patch-tests.json` (16 test cases) as the compliance test suite.

Steps:
1. Add `json-patch` crate (or implement RFC 6902 manually — it's simple)
2. Add PATCH route in Axum
3. Integration tests using the 16 test vectors
4. Support `Content-Type: application/json-patch+json`

## 4. FHIRPath Evaluation for Search Parameters

**Priority:** Medium (blocked by search parameters task)
**Effort:** Large

When building resource-specific search parameters (Patient?name=Smith), we need FHIRPath evaluation. Tests in `fhir-test-cases/r5/fhirpath/tests-fhir-r5.xml` can validate the engine.

Options:
- Build a minimal FHIRPath evaluator for common search params
- Use PostgreSQL JSON operators directly (simpler, less spec-compliant)
- Find a Rust FHIRPath crate (unlikely to exist)

## 5. SQL-on-FHIR View Layer (Future)

**Priority:** Low (future feature)
**Effort:** Large

The `sql-on-fhir/` directory contains 21 test files for the SQL-on-FHIR v2 spec. This would let users define views that project JSONB resources into flat SQL tables. Great for analytics and reporting.

Implementation would involve:
- Parse ViewDefinition JSON
- Translate FHIRPath selectors to PostgreSQL JSONB paths
- Create/manage PostgreSQL views dynamically
- Compliance test against all 21 test files
