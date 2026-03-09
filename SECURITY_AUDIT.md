# FHIR Server Security Audit Report

**Date:** 2026-03-09
**Auditor:** White-hat penetration test (automated + manual code review)
**Target:** `fhir_server` v0.1.0 — Rust/Axum FHIR R6 server
**Scope:** Full source code review + live penetration testing against docker-compose.e2e.yml deployment
**TRL:** Low (prototype)

---

## Executive Summary

| Metric | Count |
|--------|-------|
| Total tests | 53 |
| Passed | 25 |
| Vulnerabilities found | 17 |
| Informational | 11 |
| **Critical** | **1** |
| **High** | **7** |
| **Medium** | **8** |
| **Low** | **1** |

The server has a **solid foundation**: parameterized SQL queries prevent injection, tenant isolation works correctly, JWT signature validation is sound, the error handling doesn't leak internals, security headers are present (via `tower-helmet`), and the Docker image runs as a non-root user. However, there are several significant issues that must be addressed before any deployment beyond localhost development.

---

## Vulnerabilities Found

### CRITICAL Severity

#### VULN-01: Hardcoded default JWT secret
- **ID:** AUTH-08
- **Location:** `server/src/config.rs:22`
- **Description:** `JWT_SECRET` defaults to `"dev-secret-change-me"` when the environment variable is unset. If the server is deployed without explicitly setting this variable, **any attacker who reads the open-source code can forge valid JWT tokens** for any tenant with any scope (read, write, all resource types).
- **Impact:** Complete authentication bypass — full read/write access to all data for all tenants.
- **Evidence:** Code review of `config.rs` line 22: `env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-me".to_owned())`
- **Recommendation:**
  1. **Remove the default** — make `JWT_SECRET` a required env var (panic on startup if missing).
  2. Add a startup check that the secret is at least 32 bytes of entropy.
  3. Log a warning if `JWT_SECRET` looks like a development value.

---

### HIGH Severity

#### VULN-02: CORS allows any origin (`CorsLayer::permissive()`)
- **ID:** DATA-04, DATA-05
- **Location:** `server/src/lib.rs:57`
- **Description:** `CorsLayer::permissive()` sets `Access-Control-Allow-Origin: *` and `Access-Control-Allow-Methods: *`. Any website can make cross-origin requests to this API, including `DELETE`, `PUT`, and `PATCH`.
- **Impact:** A malicious website visited by an authenticated user could:
  - Read their FHIR data (exfiltration)
  - Modify or delete resources
  - Perform CSRF-like attacks without any token theft needed (if cookies or auto-attached credentials are involved)
- **Evidence:** `curl -H "Origin: https://evil.com"` returns `access-control-allow-origin: *` and `access-control-expose-headers: *`.
- **Recommendation:**
  1. Replace `CorsLayer::permissive()` with an allowlist of trusted origins.
  2. Restrict allowed methods to those actually needed by known clients.
  3. Never allow `*` in production.

#### VULN-03: No rate limiting
- **ID:** DOS-05
- **Location:** `server/src/lib.rs` (middleware stack)
- **Description:** No rate-limiting middleware is configured. An attacker can send unlimited requests.
- **Impact:** Denial of service — an attacker can exhaust the database connection pool (only 10 connections), saturate CPU with schema validation, or perform credential-stuffing attacks against the JWT auth.
- **Recommendation:**
  1. Add `tower::limit::RateLimitLayer` or a per-IP rate limiter.
  2. Consider `governor` crate for more sophisticated rate limiting.
  3. Set connection timeouts and query timeouts on the database pool.

#### VULN-04: No audit trail
- **ID:** FHIR-07
- **Location:** All CRUD handlers in `server/src/fhir.rs`
- **Description:** No `AuditEvent` resources are created when resources are created, read, updated, or deleted. There is no logging of who accessed what data.
- **Impact:** Breach detection and incident response are impossible. Regulatory compliance (HIPAA, GDPR) for health data requires audit trails.
- **Recommendation:**
  1. Create FHIR `AuditEvent` resources for all state-changing operations.
  2. Log tenant ID, resource type, resource ID, operation type, and timestamp.
  3. Consider a dedicated audit log table or external audit service.

#### VULN-05: No TLS/HTTPS
- **ID:** CRYPTO-01
- **Location:** `server/src/main.rs`, `server/Dockerfile`
- **Description:** The server listens on plain HTTP. JWT tokens, patient data, and all FHIR resources are transmitted in cleartext.
- **Impact:** Network eavesdroppers can intercept JWT tokens (session hijacking) and read/modify health data in transit.
- **Evidence:** Server binds to `0.0.0.0:8080` with no TLS configuration. While `tower-helmet` adds an HSTS header, the server itself doesn't enforce HTTPS.
- **Recommendation:**
  1. Deploy behind a TLS-terminating reverse proxy (nginx, Traefik, cloud LB).
  2. Document that bare HTTP must never be exposed to untrusted networks.
  3. Consider adding native TLS support via `axum-server` with `rustls`.

#### VULN-06: `ALLOW_UNAUTHENTICATED` is a kill-switch for all auth
- **ID:** DEPLOY-05
- **Location:** `server/src/config.rs:24`, `server/src/auth.rs:79-85`
- **Description:** Setting `ALLOW_UNAUTHENTICATED=true` grants **full read+write** access to **all resource types** under the `"public"` tenant — with zero authentication.
- **Impact:** If accidentally enabled in production (misconfig, copy-paste from dev), the entire database is open to the internet.
- **Recommendation:**
  1. Remove this feature, or gate it behind a second confirmation flag (e.g., `ALLOW_UNAUTHENTICATED_I_KNOW_WHAT_IM_DOING=true`).
  2. Log a loud warning on startup when this is enabled.
  3. If kept, limit the public tenant to read-only access only.

#### VULN-07: Stored XSS possible in FHIR resource data
- **ID:** INJ-03
- **Location:** `server/src/fhir.rs` (create/update handlers)
- **Description:** The server stores and returns FHIR resources containing `<script>` tags and other HTML verbatim. While the JSON API itself is not vulnerable (JSON content-type prevents browser XSS execution), any frontend consuming this data that renders it as HTML without escaping would be vulnerable.
- **Impact:** If a FHIR client or portal renders resource fields (e.g., `Patient.name.family`) as HTML, stored XSS attacks are possible.
- **Evidence:** `POST /fhir/Patient` with `name.family = "<script>alert('xss')</script>"` returns the payload verbatim.
- **Recommendation:**
  1. This is partially by-design for FHIR (the spec allows arbitrary string content).
  2. Document this explicitly as a security consideration for API consumers.
  3. Consider adding `Content-Type: application/fhir+json` (not `application/json`) to signal FHIR semantics.
  4. Consider HTML-sanitizing string fields on write, or at minimum flag resources containing script tags.

---

### MEDIUM Severity

#### VULN-08: PUT creates resources (upsert semantics)
- **ID:** AUTHZ-06
- **Location:** `server/src/store.rs:64-80` (`INSERT ON CONFLICT DO UPDATE`)
- **Description:** `PUT /fhir/{type}/{id}` uses upsert — it creates a resource if it doesn't exist. This bypasses any future POST-specific validation, rate limiting, or auditing.
- **Impact:** An attacker with write access can create arbitrary resources via PUT, bypassing any create-specific controls.
- **Recommendation:** Split create (POST) and update (PUT) logic. PUT should return 404 if the resource doesn't exist, or require an existing version (`If-Match`).

#### VULN-09: No optimistic concurrency control (ETag/If-Match)
- **ID:** FHIR-03, RACE-01
- **Location:** `server/src/fhir.rs` (update_resource, patch_resource)
- **Description:** The server generates ETags in responses but never validates `If-Match` headers on updates. Concurrent updates silently overwrite each other (last-write-wins).
- **Impact:** Data corruption from concurrent edits. In clinical settings, this could mean lost medical data.
- **Evidence:** 5 concurrent PUT requests all returned 200; no conflict detection.
- **Recommendation:**
  1. Parse `If-Match` header on PUT/PATCH.
  2. Compare with current `version_id` before writing.
  3. Return `409 Conflict` if versions don't match.

#### VULN-10: No resource version history
- **ID:** FHIR-08
- **Location:** `server/src/store.rs` (upsert overwrites in-place)
- **Description:** Updates overwrite the previous version. Deleted resources are gone forever. No `_history` endpoint exists.
- **Impact:** No ability to recover from accidental modifications or deletions. Violates FHIR's recommended support for `vread` and `_history`.
- **Recommendation:** Store previous versions in a history table. Implement `GET /fhir/{type}/{id}/_history`.

#### VULN-11: JWT secret strength not validated
- **ID:** CRYPTO-02
- **Description:** No minimum length or entropy check for `JWT_SECRET`. The e2e secret `"e2e-secret"` is 10 characters — feasibly brute-forced offline.
- **Recommendation:** Enforce minimum 32 bytes. Consider migration to asymmetric signing (RS256/ES256).

#### VULN-12: API documentation exposed without authentication
- **ID:** AUTHZ-01
- **Location:** `server/src/lib.rs:53` (Scalar/OpenAPI docs route)
- **Description:** `GET /docs` returns the full OpenAPI specification and interactive Scalar docs without any authentication.
- **Impact:** Reveals full API structure, endpoint details, and request/response schemas to unauthenticated users.
- **Recommendation:** Move docs behind authentication, or only serve them when a `SERVE_DOCS=true` env var is set.

#### VULN-13: Database credentials in compose file
- **ID:** DEPLOY-02, DEPLOY-03
- **Location:** `docker-compose.e2e.yml`
- **Description:** `postgres/postgres` credentials and `DATABASE_URL` containing the password are hardcoded in the compose file.
- **Recommendation:** Use Docker secrets, `.env` files excluded from version control, or a secrets manager for any shared environment.

#### VULN-14: 50 MB body limit is excessive for FHIR
- **ID:** DOS-01
- **Location:** `server/src/lib.rs:20`
- **Description:** `MAX_BODY_SIZE` is 50 MB. Typical FHIR resources are < 100 KB. This allows unnecessarily large payloads.
- **Recommendation:** Lower to 5-10 MB unless you specifically need to support large Binary resources.

---

### LOW Severity

#### VULN-15: CapabilityStatement exposed without auth
- **ID:** AUTHZ-03
- **Location:** `server/src/fhir.rs` (get_metadata handler)
- **Description:** `GET /metadata` returns the FHIR CapabilityStatement without authentication, revealing server capabilities.
- **Note:** The FHIR spec allows this, and it's common practice. Flagged as low risk.

---

## What Passed (Things Done Right)

| Test | Notes |
|------|-------|
| **SQL injection resistance** | All queries use parameterized bindings via sqlx. Tested SQLi in resource_type, id — all safe. |
| **Tenant isolation** | Resources are correctly scoped by `tenant_id` from JWT. Cross-tenant reads/searches return 404/empty. |
| **JWT signature validation** | Wrong secret → 401. Algorithm none → 401. Missing token → 401. |
| **Token expiration enforcement** | Expired tokens are correctly rejected (`validate_exp = true`). |
| **Scope enforcement** | Read-only tokens can't write. Write-only tokens can't read. |
| **Resource type restriction** | Token-scoped resource type allow lists are enforced. |
| **PATCH safety** | Cannot change `resourceType` or `id` via JSON Patch. |
| **Schema validation** | Invalid FHIR resources rejected. Extra fields rejected. Wrong types rejected. |
| **Error message safety** | Database errors map to generic "internal server error". No stack traces leaked. |
| **Security headers** | `tower-helmet` provides X-Frame-Options, X-Content-Type-Options, CSP, HSTS, etc. |
| **Docker security** | Distroless runtime image with `:nonroot` tag. Minimal attack surface. |
| **Path traversal** | Axum's router rejects path traversal attempts. |
| **Dangerous HTTP methods** | TRACE, CONNECT return 405. |
| **Resource type case sensitivity** | `"patient"` in body vs `Patient` in path → 400. |
| **Deep nesting performance** | Schema validation of deeply nested resources completes in ~10ms. |
| **No Server header** | Technology stack not disclosed via HTTP headers. |

---

## Recommendations — Prioritized Action Plan

### P0 — Do Before Any Shared Deployment
1. **Make `JWT_SECRET` required** (no default). Validate minimum length ≥ 32 chars.
2. **Restrict CORS** to specific allowed origins. Remove `CorsLayer::permissive()`.
3. **Remove or gate `ALLOW_UNAUTHENTICATED`**. Add big warning if enabled.

### P1 — Do Before Beta / External Users
4. **Add rate limiting** (per-IP and per-tenant).
5. **Implement audit logging** (FHIR AuditEvent or structured log entries).
6. **Add `If-Match` / ETag concurrency control** for PUT and PATCH.
7. **Deploy behind TLS** (document reverse proxy requirement).
8. **Split PUT/POST semantics** — PUT should not create new resources.

### P2 — Production Readiness
9. **Add version history** (`_history` endpoint, history table).
10. **Lower body size limit** to 5-10 MB.
11. **Add query timeouts** to the database pool.
12. **Move to asymmetric JWT** (RS256/ES256) for better key management.
13. **Add database connection pool monitoring** and alerting.
14. **Set resource limits** in Docker (memory, CPU, pids).

### P3 — Compliance & Best Practices
15. **Document XSS risk** for API consumers.
16. **Add `Content-Type: application/fhir+json`** response header.
17. **Authenticate `/docs` and `/metadata`** endpoints.
18. **Add startup self-tests** (validate config, check DB connectivity before accepting traffic).
19. **Consider implementing SMART on FHIR** for standards-compliant authz.

---

## Test Script

The penetration test script is available at `scripts/pentest.py`. Run against a local deployment:

```bash
docker compose -f docker-compose.e2e.yml up -d --build
sleep 5
python3 scripts/pentest.py
docker compose -f docker-compose.e2e.yml down -v
```

---

## Methodology

1. **Static analysis:** Full source code review of all `.rs` files, SQL migrations, Dockerfile, and docker-compose
2. **Dependency review:** Cargo.toml dependency audit
3. **Live testing:** 53 automated tests against running server covering:
   - Authentication bypass (expired tokens, wrong secrets, algorithm confusion, no-exp tokens)
   - Authorization bypass (scope escalation, tenant isolation, resource type restrictions)
   - Injection attacks (SQL injection, stored XSS, JSON Patch manipulation)
   - Information disclosure (error messages, headers, CORS, API docs)
   - Denial of service (body limits, pagination abuse, deep nesting, rate limiting)
   - HTTP security (method restrictions, path traversal, URL encoding)
   - FHIR-specific (resource type validation, version control, audit events)
   - Cryptographic (TLS, key strength)
   - Race conditions (concurrent updates)
4. **Configuration review:** Docker, environment variables, deployment settings
