# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed

- Authorization is now fail-closed when a JWT has no `scope` claim. Previously a valid token lacking `scope` was implicitly granted `read write`, giving full data access by default. Missing, empty, or unrecognized scopes now grant no read or write permission.
- Read-only JWTs can now submit batch and transaction Bundles containing `GET` entries. Each Bundle entry is authorized by its interaction instead of requiring write access for the Bundle envelope.
- AuditEvent `date` search now honours the strict/inclusive boundary of `gt`, `ge`, `lt`, and `le` prefixes. Previously `gt` was treated as `ge` and `le` as `lt`, so an event recorded exactly at the supplied timestamp was wrongly included for `gt` and excluded for `le`. Date parsing now shares the generic FHIR date-search implementation (precision-aware intervals, timezone offsets, fractional seconds); repeated lower/upper bounds are intersected rather than overwritten, and contradictory or unsupported ranges fail closed with a `400` response.

### Added

- Helm chart: optional `podDisruptionBudget` (minAvailable or maxUnavailable) to protect against voluntary disruptions taking down all replicas at once.

## [0.1.4] - 2026-03-10

### Changed

- Increase request body size limit for large FHIR resources.

## [0.1.3] - 2026-03-10

### Changed

- Rename project from fhir-autopilot to NisseFHIR across all files, Helm chart, Docker images, and documentation.
- Helm chart moved from `charts/fhir-autopilot/` to `charts/nissefhir/`.
- Docker image renamed to `ghcr.io/sintef/nissefhir`.
- Improved README.

## [0.1.2] - 2026-03-10

### Fixed

- Fix Rust formatting issue in `fhir.rs` that failed CI checks.

### Changed

- Release script now runs pre-flight checks (cargo fmt, clippy, tests, pre-commit) before bumping versions.

## [0.1.1] - 2026-03-10

### Added

- Release script (`scripts/release.sh`) for automated version bumping.
- Conditional create support with `If-None-Exist` header returning 412 on multiple matches.

### Fixed

- Reject unsupported resource types in URL paths with proper OperationOutcome.

## [0.1.0] - 2026-03-10

### Added

- Full FHIR R6 (6.0.0-ballot3) REST server written in Rust.
- JSON-only API with `application/fhir+json` support (no XML/RDF).
- PostgreSQL JSONB storage with partitioned tables by resource type.
- Complete CRUD: create, read, update, patch (JSON Patch), delete.
- Conditional create (`If-None-Exist` header).
- Resource history (`_history`) with version tracking.
- Search with cursor-based pagination (`_count`, `_after_id`).
- Search parameters for 40+ resource types (string, token, reference, date, quantity, number, uri, composite, special).
- JWT-based multi-tenant authentication (static HS256/RS256 or JWKS).
- SMART-on-FHIR compatible security with scope-based access control.
- FHIR JSON Schema validation with OperationOutcome error responses.
- CapabilityStatement at `/metadata` with software version info.
- Swagger UI documentation (optional, via `SERVE_DOCS`).
- Multi-stage Docker build with distroless runtime image.
- Helm chart for Kubernetes deployment with CloudNativePG support.
- Comprehensive test suite: 182 unit tests, 164 integration tests, 2410 E2E example resources.
- Security headers via tower-helmet middleware.
- CORS support with configurable allowed origins.
- Configurable database timeouts and search limits.

[0.1.4]: https://github.com/SINTEF/NisseFHIR/releases/tag/0.1.4
[0.1.3]: https://github.com/SINTEF/NisseFHIR/releases/tag/0.1.3
[0.1.2]: https://github.com/SINTEF/NisseFHIR/releases/tag/0.1.2
[0.1.1]: https://github.com/SINTEF/NisseFHIR/releases/tag/0.1.1
[0.1.0]: https://github.com/SINTEF/NisseFHIR/releases/tag/0.1.0
