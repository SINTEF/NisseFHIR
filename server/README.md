# fhir_server

Initial lightweight FHIR 6.0 server implementation in Rust.

## Implemented in this milestone

- JSON-only API
- PostgreSQL-backed storage (`JSONB`)
- Multi-tenant context from JWT (`tenant`/`sub` claim)
- Optional unauthenticated mode for local development
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

- `DATABASE_URL` (required)
- `BIND_ADDR` (default: `0.0.0.0:8080`)
- `JWT_SECRET` (default: `dev-secret-change-me`)
- `ALLOW_UNAUTHENTICATED` (default: `false`)
- `FHIR_BASE_URL` (default: `http://localhost:8080/fhir`)

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
