# Architecture

Lightweight FHIR 6.0 server implementation in Rust.

## Implemented in this milestone

- JSON-only API
- PostgreSQL-backed storage (`JSONB`)
- Multi-tenant context from JWT (`tenant`/`sub` claim)
- Mandatory bearer-token authentication with configurable JWT verification
- FHIR JSON Schema validation backed by the bundled `fhir.schema.json`
- FHIR `OperationOutcome` error responses for auth and request validation failures
- Endpoints:
  - `GET /healthz`
  - `GET /fhir/metadata` (standard `[base]/metadata` endpoint)
  - `GET /metadata` (temporary legacy alias)
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
- `SEARCH_DEFAULT_COUNT` (default: `128`)
- `SEARCH_MAX_COUNT` (default: `2048`)
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

Fetch signing keys from an OpenID Connect / OAuth2 JWKS endpoint. Keys are loaded at startup (server fails fast if unreachable or if the set contains no usable key — fails closed) and refreshed in the background. A failed refresh or a set with no usable key (empty, malformed, or only unsupported algorithms) never replaces the current working set, so a transient bad response cannot wipe out verification keys.

| Variable | Description |
| --- | --- |
| `JWT_JWKS_URI` | Required — URL to the provider's `/.well-known/jwks.json` |
| `JWT_JWKS_REFRESH_SECS` | Background refresh interval (default: `300`); must be at least `60` |
| `JWT_ISSUER` | Optional — validate the `iss` claim |
| `JWT_AUDIENCE` | Optional — validate the `aud` claim |

The server decodes each token's `kid` header to find the matching key in the JWKS set. Tokens without a `kid` are rejected.

## Run

```bash

export JWT_SECRET="$(openssl rand -hex 32)"
JWT_MODE=static \
JWT_ALGORITHM=HS256 \
SERVE_DOCS=true \
DATABASE_URL=postgres://postgres:postgres@localhost/postgres \
cargo run
```

## Authentication And Docs

Most FHIR endpoints require an `Authorization: Bearer <token>` header. Only `GET /healthz`, `GET /fhir/metadata`, and its temporary legacy alias `GET /metadata` are intentionally public.

The CapabilityStatement uses
`https://sintef.github.io/NisseFHIR/CapabilityStatement/nissefhir` as its stable
canonical identifier. This identifier is deployment-independent; the
statement's `implementation.url` contains the configured `FHIR_BASE_URL`, and
the statement is retrieved from `[FHIR_BASE_URL]/metadata`.

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
python3 scripts/generate_static_jwt.py \\
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

### Scope claim (authorization)

Authorization is fail-closed on the `scope` claim:

- Missing, empty, or whitespace-only `scope` grants no read or write permission.
- A scope containing only unrecognized values grants no read or write permission.
- Matching is case-insensitive and whitespace-delimited; recognized names are `read` and `write`.
- `read` authorizes read interactions (search, read, history); `write` authorizes write interactions (create, update, patch, delete).
- Bundle (batch/transaction) entries are authorized individually by their HTTP interaction. A read-only token can submit a Bundle containing only authorized `GET` entries; write entries require `write`. In a transaction, one forbidden entry rejects the whole transaction. In a batch, it produces an inline `403 Forbidden` entry while other entries continue.
- `resource_types` additionally restricts which FHIR resource types may be accessed; it never relaxes scope requirements.

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
- In native mode, prefers a local PostgreSQL instance when available and falls back to disposable PostgreSQL via `docker compose -f compose.e2e.yaml`.
- Runs the server natively with `cargo run --release` or as the built container.
- Scans the whole `examples/` directory, infers each example type from its top-level `resourceType`, and uses parallel HTTP requests to keep the run time reasonable.
- Treats schema-valid examples as accepted resources, schema-invalid official examples as expected `OperationOutcome` rejections, and oversized payload transport failures as an explicit current limitation.
- Uses a small CRUD smoke flow on a known-good subset when `--dataset smoke` or `--dataset all-plus-smoke` is selected.

## Notes

- Capability statement advertises create/read/history-instance/update/patch/delete/search-type interactions.
- Collection search uses cursor pagination ordered by resource id by default. Clients request the first page with `_count` and follow the returned `next` link using `_after_id`. Clients can request a different order with `_sort`; see [Sorted search (`_sort`)](#sorted-search-_sort) below.
- HTTP response compression is enabled through `tower-http` content negotiation. Clients can request compressed FHIR responses with standard `Accept-Encoding` values such as `gzip`, `br`, or `deflate`.
- Create and update requests validate both the FHIR envelope (`resourceType`, `id`) and the resource-specific JSON Schema definition.
- Invalid JSON, schema failures, auth failures, and missing resources return FHIR-shaped `OperationOutcome` bodies.
- TLS is expected to be terminated by a reverse proxy or ingress layer; the server itself only listens on HTTP.

## Conditional create (`If-None-Exist`) atomicity

`POST /fhir/{type}` with an `If-None-Exist` header implements the FHIR
conditional create interaction. The match-and-create decision is serialized
inside one PostgreSQL transaction per (tenant, resource type, canonical
condition) so that two concurrent identical conditional creates cannot both
observe zero matches and create duplicate logical resources.

### Locking mechanism

- The `If-None-Exist` query is parsed with the same search parameter parser
  used by `GET /fhir/{type}`. The decoded filters are canonicalized by sorting
  on the parameter code, so equivalent conditions with reordered parameters
  produce the same canonical key (see `conditional_create_lock_key`).
- The canonical key is hashed to an `i64` and acquired with
  `pg_advisory_xact_lock(bigint)` inside a PostgreSQL transaction. The lock
  is transaction-scoped: it is released automatically on commit or rollback.
- The transaction then runs the parametrized search (`LIMIT 2`) under the
  lock and either returns the existing match, returns `412 Precondition
  Failed` for multiple matches, or inserts the new resource via the same
  `create_in_tx` insert-only path used by unconditional POST.

### Collision behavior

- Identical `(tenant, type, condition)` triples always hash to the same
  lock key, so concurrent duplicates serialize: one request creates the
  resource and the other observes it and returns `200 OK` with the
  existing resource.
- Unrelated tenants, resource types, or conditions hash to different
  lock keys and are **not** serialized against each other.
- A hash collision between two unrelated keys only causes extra
  serialization — it cannot produce incorrect results, because the search
  and insert still run inside the transaction and rely on the database's own
  consistency guarantees. True semantic collisions are impossible because
  identical inputs hash identically.

## Version-aware writes (`If-Match`)

`PUT`, `PATCH`, and `DELETE` support the FHIR optimistic-concurrency
interaction via the `If-Match` header, carrying a concrete version ETag such
as `W/"3"`. A wildcard (`*`) is rejected.

- `PUT` with a matching version updates the resource and records the next
  history version; a stale version returns `412 Precondition Failed` and does
  not modify anything.
- `PATCH` behaves like `PUT`: the patch is applied and committed only when the
  version predicate matches, otherwise `412`.
- `DELETE` with a matching version removes the resource and records the next
  history version as a tombstone; a stale version returns `412` without
  deleting anything.

### Behavior without `If-Match`

When no `If-Match` header is supplied, the interaction is **unconditional**:

- `PUT` performs an upsert (creates the resource if it is missing, otherwise
  updates the current version).
- `PATCH` and `DELETE` operate on the current version regardless of how many
  times it has been rewritten since the client last read it. This means a
  stale client can overwrite or remove a resource updated by another writer.
  Clients that need to avoid clobbering concurrent writes should always send
  the `If-Match` ETag they observed on their most recent read.

### Atomicity

The version predicate and the write are executed inside a single PostgreSQL
transaction, so the check-and-write decision is atomic. For `DELETE`, the
predicate is expressed directly in the `DELETE` statement's `WHERE` clause
(`version_id = expected`), and a subsequent existence probe distinguishes a
version mismatch from a missing resource. The same semantics apply to
standalone endpoints and to Bundle `transaction`/`batch` entries, so both
paths behave consistently.

## Sorted search (`_sort`)

`GET /fhir/{type}` accepts an optional `_sort` result parameter: a
comma-separated list of sort keys, each optionally prefixed with `-` for
descending order, applied in the order given (e.g.
`_sort=status,-_lastUpdated`). Omitting `_sort` keeps today's default order —
ascending by `id` — unchanged.

### Sortable keys

Every resource accepts `_id` and `_lastUpdated`. A deliberately small,
resource-specific allow-list adds singular scalar search parameters where they
are useful for a catalogue, workflow, or clinical timeline. The current list
is the executable `sortable_search_param_codes_for` registry: Patient
(`birthdate`, `death-date`, `gender`, `active`), Organization (`active`,
`name`), Observation (`status`, `value-string`), Questionnaire (`date`,
`name`, `status`, `title`), QuestionnaireResponse (`authored`, `status`),
Task (`authored-on`, `modified`, `status`), Composition (`date`, `status`,
`title`), CommunicationRequest (`authored`, `status`), MedicationRequest
(`authoredon`, `status`), Condition (`recorded-date`), Immunization (`date`,
`status`), DocumentReference (`date`, `status`), and ServiceRequest
(`authored`, `status`).

Complex, repeated, reference, quantity, and questionnaire-answer parameters
are intentionally not sortable: they have no single obvious value to order
by. A key outside the allow-list — unknown, unindexed, or unsupported for the
resource type — returns a privacy-safe `400` rather than being silently
ignored or substituted. A repeated `_sort` parameter, or more sort keys than
the server allows in one request, is also rejected with `400`. The
CapabilityStatement advertises the accepted keys for each resource type as a
repeating `sortParameter` extension (there is no standard
CapabilityStatement field for this) on `rest.resource`, verified by a test
that cross-checks advertised and accepted keys.

### Ordering and pagination

Every sort order is total and deterministic: the `id` column is appended as a
final tiebreak whenever it is not already one of the requested keys, so
resources with equal sort values still have a stable relative order across
pages.

Sorting and paging are one design because the pagination cursor's meaning
depends on the sort order the page was generated under. `_after_id` always
carries an opaque, versioned cursor encoding the sort column values of the
last row on the page (not just its id), together with a fingerprint of the
effective sort and filters it was minted under. When `_sort` is absent, the
effective sort is `_id` ascending. Replaying a cursor against a different
sort or filter set is rejected with `400` — a cursor is never silently
reinterpreted.

`_sort` is preserved unchanged in the generated `self` and `next` Bundle
links, so following `next` repeatedly never requires the client to re-supply
it. A composite B-tree index on `(tenant_id, last_updated, id)` supports
`_sort=_lastUpdated` keyset pagination the same way the existing
`(tenant_id, id)` index already supports the default and `_sort=_id` orders,
so a sorted page is always served by an index scan.

### Out of scope

`_sort` is only implemented for `GET /fhir/{type}`. AuditEvent search
(`GET /fhir/AuditEvent`) and instance history
(`GET /fhir/{type}/{id}/_history`) do not accept it — both already fail
closed on any parameter they do not recognize, so passing `_sort` to either
returns a clear `400` rather than being accepted and silently ignored.
