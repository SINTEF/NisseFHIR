# fhir_server

Initial lightweight FHIR 6.0 server implementation in Rust.

## Implemented in this milestone

- JSON-only API
- PostgreSQL-backed storage (`JSONB`)
- Multi-tenant context from JWT (`tenant`/`sub` claim)
- Optional unauthenticated mode for local development
- Endpoints:
  - `GET /healthz`
  - `GET /metadata`
  - `POST /fhir/:resource_type`
  - `GET /fhir/:resource_type/:id`
  - `PUT /fhir/:resource_type/:id`

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

## Notes

- Capability statement currently advertises only create/read/update interactions.
- JSON schema validation is not wired yet; current validation checks basic FHIR envelope consistency (`resourceType`, `id`).
