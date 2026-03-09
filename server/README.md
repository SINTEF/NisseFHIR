# fhir_server

Initial lightweight FHIR 6.0 server implementation in Rust.

## Implemented in this milestone

- JSON-only API
- PostgreSQL-backed storage (`JSONB`)
- Multi-tenant context from JWT (`tenant`/`sub` claim)
- Mandatory bearer-token authentication with configurable JWT verification
- FHIR JSON Schema validation backed by the bundled `fhir.schema.json`
- FHIR `OperationOutcome` error responses for auth and request validation failures
- Endpoints:
  - `GET /healthz`
  - `GET /metadata`
  - `POST /fhir/:resource_type`
  - `GET /fhir/:resource_type/:id`
  - `PUT /fhir/:resource_type/:id`
  - `DELETE /fhir/:resource_type/:id`
  - `GET /fhir/:resource_type` with `_count` and `_offset`

## Environment

### Core

- `DATABASE_URL` (required)
- `BIND_ADDR` (default: `0.0.0.0:8080`)
- `FHIR_BASE_URL` (default: `http://localhost:8080/fhir`)
- `CORS_ALLOWED_ORIGINS` (optional comma-separated origin allowlist; unset means no cross-origin access)
- `SERVE_DOCS` (default: `false`; set to `true` to expose `/docs`)

### JWT Authentication

The server supports three JWT modes selected via `JWT_MODE`:

#### `JWT_MODE=static` (default)

Verify tokens against a single static key — either HMAC or an asymmetric PEM.

| Variable | Description |
|---|---|
| `JWT_ALGORITHM` | `HS256` (default), `HS384`, `HS512`, `RS256`, `RS384`, `RS512`, `ES256`, `ES384` |
| `JWT_SECRET` | Required for HMAC algorithms; minimum 32 chars |
| `JWT_PUBLIC_KEY_PEM` or `JWT_PUBLIC_KEY_PATH` | Required for RSA/ECDSA algorithms |
| `JWT_ISSUER` | Optional — validate the `iss` claim |
| `JWT_AUDIENCE` | Optional — validate the `aud` claim |

For production, prefer asymmetric verification (`RS256`/`ES256`) with a mounted public key.

#### `JWT_MODE=jwks`

Fetch signing keys from an OpenID Connect / OAuth2 JWKS endpoint. Keys are loaded at startup (server fails fast if unreachable) and refreshed in the background.

| Variable | Description |
|---|---|
| `JWT_JWKS_URI` | Required — URL to the provider's `/.well-known/jwks.json` |
| `JWT_JWKS_REFRESH_SECS` | Background refresh interval (default: `300`) |
| `JWT_ISSUER` | Optional — validate the `iss` claim |
| `JWT_AUDIENCE` | Optional — validate the `aud` claim |

The server decodes each token's `kid` header to find the matching key in the JWKS set. Tokens without a `kid` are rejected.

#### `JWT_MODE=dev`

Development-only mode. A cryptographically random HMAC secret is generated on each startup and a token-minting endpoint is enabled at `POST /dev/token`.

```bash
# Mint a development token:
curl -s http://localhost:8080/dev/token \
  -H 'Content-Type: application/json' \
  -d '{"tenant":"my-tenant","scope":"read write","expires_in":3600}'
```

No secrets need to be configured. The endpoint accepts optional fields `tenant`, `scope`, `resource_types`, and `expires_in` (seconds, max 86400).

**Do not use dev mode in production** — the secret does not persist across restarts and `POST /dev/token` is unauthenticated.

## Run

```bash
cd server
cargo run
```

## End-To-End Checks

From the repository root, run the Python E2E harness against the native server, the Docker image, or both:

```bash
python3 scripts/e2e_examples.py --mode both
```

Useful variants:

```bash
python3 scripts/e2e_examples.py --mode native --native-db auto --dataset all-plus-smoke --workers 12
python3 scripts/e2e_examples.py --mode docker --dataset all-plus-smoke --workers 12
```

What it does:

- Ensures the HL7 `examples-json.zip` dataset is present, downloading and extracting it automatically when needed.
- In native mode, prefers a local PostgreSQL instance when available and falls back to disposable PostgreSQL via `docker compose -f docker-compose.e2e.yml`.
- Runs the server natively with `cargo run --release` or as the built container.
- Scans the whole `examples/` directory, infers each example type from its top-level `resourceType`, and uses parallel HTTP requests to keep the run time reasonable.
- Treats schema-valid examples as accepted resources, schema-invalid official examples as expected `OperationOutcome` rejections, and oversized payload transport failures as an explicit current limitation.
- Uses a small CRUD smoke flow on a known-good subset when `--dataset smoke` or `--dataset all-plus-smoke` is selected.

## Notes

- Capability statement advertises create/read/update/delete/search-type interactions.
- Create and update requests validate both the FHIR envelope (`resourceType`, `id`) and the resource-specific JSON Schema definition.
- Invalid JSON, schema failures, auth failures, and missing resources return FHIR-shaped `OperationOutcome` bodies.
- TLS is expected to be terminated by a reverse proxy or ingress layer; the server itself only listens on HTTP.
