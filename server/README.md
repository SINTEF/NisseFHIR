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
  - `GET /fhir/:resource_type/:id/_history`
  - `PUT /fhir/:resource_type/:id`
  - `DELETE /fhir/:resource_type/:id`
  - `GET /fhir/:resource_type` with `_count` and `_after_id`

## Environment

### Core

- `DATABASE_URL` (required)
- `DB_CONNECT_TIMEOUT_SECS` (default: `5`)
- `DB_ACQUIRE_TIMEOUT_SECS` (default: `5`)
- `DB_STATEMENT_TIMEOUT_MS` (default: `10000`)
- `BIND_ADDR` (default: `0.0.0.0:8080`)
- `FHIR_BASE_URL` (default: `http://localhost:8080/fhir`)
- `SEARCH_DEFAULT_COUNT` (default: `20`)
- `SEARCH_MAX_COUNT` (default: `100`)
- `CORS_ALLOWED_ORIGINS` (optional comma-separated origin allowlist; unset means no cross-origin access)
- `SERVE_DOCS` (default: `false`; set to `true` to expose `/docs` with vendored Swagger UI assets)

### JWT Authentication

The server supports two JWT modes selected via `JWT_MODE`:

#### `JWT_MODE=static` (default)

Verify tokens against a single static key — either HMAC or an asymmetric PEM.

| Variable | Description |
| --- | --- |
| `JWT_ALGORITHM` | `HS256` (default), `HS384`, `HS512`, `RS256`, `RS384`, `RS512`, `ES256`, `ES384` |
| `JWT_SECRET` | Required for HMAC algorithms; minimum 32 chars |
| `JWT_PUBLIC_KEY_PEM` or `JWT_PUBLIC_KEY_PATH` | Required for RSA/ECDSA algorithms |
| `JWT_ISSUER` | Optional — validate the `iss` claim |
| `JWT_AUDIENCE` | Optional — validate the `aud` claim |

For production, prefer asymmetric verification (`RS256`/`ES256`) with a mounted public key.

#### `JWT_MODE=jwks`

Fetch signing keys from an OpenID Connect / OAuth2 JWKS endpoint. Keys are loaded at startup (server fails fast if unreachable) and refreshed in the background.

| Variable | Description |
| --- | --- |
| `JWT_JWKS_URI` | Required — URL to the provider's `/.well-known/jwks.json` |
| `JWT_JWKS_REFRESH_SECS` | Background refresh interval (default: `300`) |
| `JWT_ISSUER` | Optional — validate the `iss` claim |
| `JWT_AUDIENCE` | Optional — validate the `aud` claim |

The server decodes each token's `kid` header to find the matching key in the JWKS set. Tokens without a `kid` are rejected.

## Run

```bash
cd server
export JWT_SECRET="$(openssl rand -hex 32)"
JWT_MODE=static \
JWT_ALGORITHM=HS256 \
SERVE_DOCS=true \
DATABASE_URL=postgres://postgres:postgres@localhost/postgres \
cargo run
```

## Authentication And Docs

Most FHIR endpoints require an `Authorization: Bearer <token>` header. Only `GET /healthz` and `GET /metadata` are intentionally public.

Tenant handling is explicit:

- the server requires either a `tenant` claim or a `sub` claim
- if both are present, `tenant` takes precedence over `sub`
- there is no default tenant on the server side

Issuer and audience validation are opt-in:

- set `JWT_ISSUER` to require a matching `iss` claim
- set `JWT_AUDIENCE` to require a matching `aud` claim
- if these variables are unset, those claims are not validated

### Getting a token for local development

For local testing with static HMAC mode, start the server as shown above and then generate a compatible JWT:

```bash
python3 ../scripts/generate_static_jwt.py \
  --tenant my-tenant \
  --scope 'read write'
```

The script reads `JWT_SECRET` from the environment by default. Use `--secret` only if you intentionally want to override it.

The script prints a JWT. Use it in requests like this:

```bash
TOKEN="paste-token-here"

curl -s http://localhost:8080/fhir/Patient \
  -H "Authorization: Bearer $TOKEN"
```

### Using Swagger UI with a token

When `SERVE_DOCS=true`, the docs are available at `http://localhost:8080/docs/`.

- The OpenAPI document now advertises HTTP bearer auth for the protected `/fhir/...` routes.
- Swagger UI shows an `Authorize` button where you can paste the JWT token.
- The UI is configured to persist the authorization between page reloads in the same browser profile.

Paste only the raw JWT value into the Swagger UI auth dialog unless the dialog explicitly asks for the full header value.

### Production and non-local tokens

Swagger UI cannot mint a token for you. You must obtain a valid JWT from the configured auth source:

- `JWT_MODE=static`: issue a token signed by your configured HMAC secret or matching private key.
- `JWT_MODE=jwks`: obtain a token from the external OpenID Connect or OAuth2 provider behind the configured JWKS endpoint.

Whichever mode you use, the token must satisfy this server's authorization checks:

- include either `tenant` or `sub`
- include `scope` with `read`, `write`, or both depending on the operation
- optionally include `resource_types` to restrict access to specific FHIR resource types

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

- Capability statement advertises create/read/history-instance/update/patch/delete/search-type interactions.
- Collection search uses cursor pagination ordered by resource id. Clients request the first page with `_count` and follow the returned `next` link using `_after_id`.
- Create and update requests validate both the FHIR envelope (`resourceType`, `id`) and the resource-specific JSON Schema definition.
- Invalid JSON, schema failures, auth failures, and missing resources return FHIR-shaped `OperationOutcome` bodies.
- TLS is expected to be terminated by a reverse proxy or ingress layer; the server itself only listens on HTTP.
