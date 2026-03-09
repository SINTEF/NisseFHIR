# Test Overview

## Current Coverage

- `server/tests/validation.rs`: schema validation acceptance and rejection for representative FHIR resources.
- `server/tests/auth.rs`: JWT validation, scope enforcement, resource allow-list checks, unauthenticated mode, and tenant isolation.
- `server/tests/crud.rs`: create, read, update semantics, headers, roundtrips, versioning, and resource coexistence.
- `server/tests/search.rs`: collection search baseline, pagination shape, tenant scoping, unauthenticated mode, and forbidden resource-type access.
- `server/src/*.rs` unit tests: capability statement shape, auth claim parsing, error mapping, payload normalization, and search bundle formatting.

## What Is Missing

- End-to-end tests with disposable PostgreSQL infrastructure instead of relying on a preconfigured local test database.
- Search parameter coverage beyond `_count` and `_offset`, including resource-specific filtering and edge cases around unsupported parameters.
- Delete, history, conditional interactions, and transaction/batch behavior.
- Capability statement assertions for security/auth metadata and more exact conformance details.
- Performance-focused tests or lightweight benchmarks for validation and collection reads.

## Recommended Next Steps

1. Add disposable-Postgres integration setup so CI does not depend on an externally prepared database.
2. Extend search tests with one or two real FHIR parameters per resource type, starting with simple exact-match fields.
3. Add audit/logging assertions once request IDs and tenant-aware logging are implemented.