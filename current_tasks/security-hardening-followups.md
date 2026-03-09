# Security Hardening Follow-ups

## Completed in this pass

- Removed `ALLOW_UNAUTHENTICATED` from the server configuration and auth flow.
- Removed the insecure default JWT secret; HMAC verification now requires an explicit secret with a minimum length of 32 characters.
- Added support for asymmetric JWT verification via `JWT_PUBLIC_KEY_PEM` or `JWT_PUBLIC_KEY_PATH` with `RS256`/`ES256` family algorithms.
- Replaced permissive CORS with an explicit allowlist via `CORS_ALLOWED_ORIGINS`.
- Hid `/docs` unless `SERVE_DOCS=true` is set.
- Lowered the request body limit from 50 MB to 10 MB.
- Updated e2e tooling and tests to match the hardened auth model.
- Added three JWT modes via `JWT_MODE`: `static` (single key), `jwks` (OIDC/OAuth2 key discovery with background refresh), and `dev` (random key per startup with token-minting endpoint).
- Added optional `JWT_ISSUER` and `JWT_AUDIENCE` claim validation for all modes.
- Dev mode (`JWT_MODE=dev`) generates a cryptographically random secret per startup and exposes `POST /dev/token` for minting test tokens — safe for local development, impossible to leak a shared secret.
- JWKS mode fetches keys at startup (fail-fast) then periodically refreshes them in the background, supporting key rotation without restarts.

## Remaining work

1. Enforce optimistic concurrency with `If-Match` on `PUT` and `PATCH`.
2. Split `PUT` from create semantics so update does not upsert missing resources.
3. Add database connection and statement timeouts to reduce DoS blast radius.
4. Decide whether `application/fhir+json` should replace `application/json` in responses.
5. Document deployment expectations around reverse-proxy TLS termination and trusted CORS origins in the root project docs.

## Notes

- Rate limiting remains intentionally out of scope for this service layer and should be enforced at ingress or API gateway level unless a concrete in-process requirement appears.
- HTTP access logging via `tower_http::trace` remains the current audit trail mechanism.