# FHIR Server Security Audit Status Review

**Original audit date:** 2026-03-09
**Status review date:** 2026-03-10
**Target:** `fhir_server` v0.1.0 — Rust/Axum FHIR R6 server

This file is a cleanup of the original audit report against the current codebase. It keeps findings that are still relevant, removes findings that are now fixed, and reclassifies items that are better treated as deployment concerns or explicit product choices rather than server vulnerabilities.

---

## Executive Summary

The original report is now materially out of date.

Several previously reported issues have already been fixed in the codebase:

- `JWT_SECRET` is required for HMAC mode and must be at least 32 characters.
- Development-looking JWT secrets now trigger a startup warning.
- CORS is no longer permissive by default and can be restricted to configured origins.
- OpenAPI docs are disabled by default and only exposed when `SERVE_DOCS=true`.
- The request body limit is now 10 MB, not 50 MB.
- FHIR responses are served as `application/fhir+json`.
- Instance history exists via `GET /fhir/{resourceType}/{id}/_history`.
- `If-Match` / ETag conditional update support exists for `PUT` and `PATCH`.
- The unauthenticated bypass flag reported in the original audit is no longer present.

At this point, the main server-side gap that still looks materially relevant is the lack of an audit trail for data access and mutations.

Some items from the original report are better treated as deployment responsibilities or accepted product choices for this server:

- TLS termination
- Rate limiting
- public `/metadata`
- optional docs exposure when explicitly enabled
- client-side HTML escaping responsibilities
- local test credentials in `compose.e2e.yaml`

---

## Still Relevant

### 1. No audit trail for resource access and mutation

- **Status:** Still relevant
- **Scope:** All CRUD handlers in `src/fhir.rs`
- **Why it still matters:** The server does not create `AuditEvent` resources and does not appear to persist a structured audit log for reads, creates, updates, patches, deletes, or bundle operations.
- **Impact:** Operational forensics, breach investigation, and regulated-environment compliance remain weak. For healthcare workloads, this is usually the biggest remaining gap.
- **Recommendation:**
  1. Decide whether the product wants FHIR-native `AuditEvent`, an internal audit table, or structured append-only logs.
  2. At minimum, record tenant, subject, operation, resource type, resource id, outcome, and timestamp.
  3. Cover both direct REST operations and Bundle processing.

---

## Behavior Notes, Not Security Vulnerabilities

### 1. Bundle `PUT` still behaves like upsert

- **Status:** Still true in bundle processing
- **Scope:** `process_single_entry` in `src/fhir.rs` still routes Bundle `PUT` through `exec_upsert`
- **Assessment:** This is no longer true for the normal REST `PUT /fhir/{type}/{id}` handler, which now updates existing resources only and returns `404` when the resource does not exist. The remaining inconsistency is inside Bundle handling.
- **Why this is not a security issue by itself:** It is primarily a semantics and consistency concern, not an authz bypass.
- **Recommendation:** Align Bundle `PUT` with standalone `PUT` only if strict REST/FHIR consistency is important for your product.

### 2. Stored HTML or script content in FHIR strings

- **Status:** Accepted behavior
- **Assessment:** The server stores and returns resource JSON as data. With `application/fhir+json`, this is not a server-side XSS issue by itself. Any XSS risk appears when downstream clients render arbitrary FHIR string content as HTML without escaping.
- **Recommendation:** Document this for API consumers if you expect browser-based portals to render user-controlled fields.

---

## Deployment and Platform Responsibilities

These were reported in the original audit, but they are better treated as deployment architecture choices than application-layer vulnerabilities in this repository.

### 1. TLS / HTTPS

- This server currently serves plain HTTP.
- That is acceptable if TLS is terminated at an ingress, reverse proxy, or load balancer.
- The real requirement is: do not expose this service over untrusted networks without TLS in front of it.

### 2. Rate limiting

- The application does not implement rate limiting middleware.
- If your architecture already expects this at the gateway, ingress, proxy, or API-management layer, that is a reasonable boundary.
- If this server is ever exposed directly, revisit that decision.

### 3. Local test credentials in `compose.e2e.yaml`

- The compose file still contains local test credentials.
- For an e2e-only local setup, this is acceptable.
- This should not be copied unchanged into shared or production deployments.

---

## Closed Since The Original Audit

The following original findings are no longer current:

1. **Hardcoded default JWT secret:** fixed. `JWT_SECRET` is now required for HMAC verification.
2. **JWT secret strength not validated:** fixed. Minimum length validation exists.
3. **CORS allows any origin:** fixed. CORS uses configured origins rather than `CorsLayer::permissive()`.
4. **`ALLOW_UNAUTHENTICATED` kill-switch:** obsolete. This flag is no longer present in the current auth path.
5. **No optimistic concurrency control:** fixed. `If-Match` is parsed and enforced for conditional update flows.
6. **No resource version history:** fixed. History is stored and exposed via `_history`.
7. **API docs exposed without authentication by default:** fixed in practice. Docs are disabled unless `SERVE_DOCS=true`.
8. **50 MB body limit:** obsolete. The limit is now 10 MB.
9. **Missing FHIR content type:** fixed. FHIR JSON responses are rewritten to `application/fhir+json`.

---

## Accepted Public Surface

These items were called out in the original report, but they are acceptable for this server as currently positioned:

1. **`GET /metadata` without authentication:** acceptable and aligned with common FHIR server behavior.
2. **Documentation exposure when explicitly enabled:** acceptable for development and self-hosted setups; keep `SERVE_DOCS=false` by default.
3. **10 MB request body limit:** acceptable if you intend to support larger resources or want operational headroom.

---

## Current Recommendation

If this repository stays a lightweight FHIR server intended to sit behind normal infrastructure, the actionable server-side security work is now much smaller than the original report suggested.

Priority order:

1. Implement audit logging if you care about healthcare-grade traceability.
2. Decide whether Bundle `PUT` should match standalone `PUT` semantics.
3. Document the deployment boundary clearly: TLS and rate limiting are expected outside the app.

---

## Verification Notes

This cleanup is based on the current implementation, including:

- required and validated JWT HMAC secrets in `src/config.rs`
- non-permissive CORS setup and 10 MB body limit in `src/lib.rs`
- `_history`, `If-Match`, and FHIR content type handling in `src/fhir.rs`
- version history persistence in `src/store.rs`
- docs gating and CORS tests in `tests/http_config.rs`
- history and conditional update tests in `tests/history.rs`, `tests/crud.rs`, and `tests/patch.rs`
