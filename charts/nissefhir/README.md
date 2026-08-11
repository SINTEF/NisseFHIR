# NisseFHIR Helm Chart

Helm chart for deploying NisseFHIR with either a CloudNativePG-managed PostgreSQL cluster or an external PostgreSQL database.

## Install

Minimal install:

```bash
helm install my-release ./charts/nissefhir
```

Install from the published repository:

```bash
helm repo add nissefhir https://nissefhir.github.io/helm-charts
helm install my-release nissefhir/nissefhir
```

## JWT Secret Handling

For `config.jwtMode=static`, the chart supports two Secret sources:

1. An existing Kubernetes Secret.
2. A Secret created by the chart at install or upgrade time.

It also supports two delivery modes to the container:

1. `env`: inject the secret as `JWT_SECRET`.
2. `file`: mount the Secret and set `JWT_SECRET_FILE`.

The server supports both `JWT_SECRET` and `JWT_SECRET_FILE`. It also supports `DATABASE_URL_FILE` for environments that prefer mounted secrets.

### Recommended: Existing Secret

```bash
kubectl create secret generic my-release-jwt \
  --from-literal=jwt-secret="$(openssl rand -hex 32)"

helm install my-release ./charts/nissefhir \
  --set config.jwtMode=static \
  --set config.jwtSecret.existingSecret.name=my-release-jwt
```

### Chart-Managed Secret

If `config.jwtSecret.create=true` and no explicit value is provided, the chart generates a random secret. On upgrade, it reuses the existing Secret value when possible instead of rotating it implicitly.

This is the simplest way to get started. For production, an externally managed Secret is often still the cleaner choice.

Auto-generated secret:

```bash
helm install my-release ./charts/nissefhir \
  --set config.jwtMode=static \
  --set config.jwtSecret.create=true
```

Explicit secret value with `openssl`:

```bash
helm install my-release ./charts/nissefhir \
  --set config.jwtMode=static \
  --set config.jwtSecret.create=true \
  --set-string config.jwtSecret.value="$(openssl rand -hex 32)"
```

### File Delivery Mode

```bash
helm install my-release ./charts/nissefhir \
  --set config.jwtMode=static \
  --set config.jwtSecret.existingSecret.name=my-release-jwt \
  --set config.jwtSecret.delivery=file
```

By default, file delivery mounts the Secret at `/var/run/secrets/nissefhir` and points `JWT_SECRET_FILE` to the selected key inside that directory.

## Key Values

```yaml
config:
  jwtMode: static
  jwtSecret:
    create: false
    key: jwt-secret
    delivery: env
    mountPath: /var/run/secrets/nissefhir
    existingSecret:
      name: my-secret
      key: jwt-secret
```

Related database configuration:

```yaml
cnpg:
  enabled: true

  externalDatabase:
    url: postgres://user:password@host:5432/fhir
    existingSecret:
      name: my-db-secret
      key: database-url
```

## Geospatial search (`near`)

The FHIR `near` search parameter always works, on any PostgreSQL, without
configuration: when the optional `earthdistance` extension is not installed
the server transparently falls back to a pure-SQL haversine filter, so startup
never fails and `near` is always advertised in the CapabilityStatement.

Installing `earthdistance` (which pulls in `cube`) additionally gives `near`
a GiST-indexed path, which is preferable for large `Location` collections.
It is an optional enhancement, not a requirement. Pre-provision it so the
application role never needs elevated privileges:

- **CNPG-managed cluster** — run the `CREATE EXTENSION` statements during
  bootstrap by adding `postInitApplicationSQL` to the Cluster spec:

  ```yaml
  cnpg:
    bootstrap:
      initdb:
        postInitApplicationSQL:
          - CREATE EXTENSION IF NOT EXISTS cube;
          - CREATE EXTENSION IF NOT EXISTS earthdistance;
  ```

- **External database** — run once as a superuser / privileged role:

  ```sql
  CREATE EXTENSION IF NOT EXISTS cube;
  CREATE EXTENSION IF NOT EXISTS earthdistance;
  ```

The server detects the extension at startup and logs the selected mode
(`EarthDistance` or `Haversine`).

## Prometheus Metrics

NisseFHIR serves privacy-safe Prometheus metrics on a dedicated telemetry
listener that is separate from the public FHIR router and is never exposed
through the chart's ingress. It is unauthenticated for Prometheus
compatibility and must be protected by cluster networking.

```yaml
metrics:
  enabled: true
  port: 9090
  serviceMonitor:
    enabled: false
    additionalLabels: {}
    interval: 30s
    scrapeTimeout: 10s
```

- `metrics.enabled` maps to `METRICS_ENABLED`. When `false`, no telemetry
  listener is started and no metrics port or `ServiceMonitor` is rendered.
- `metrics.port` maps to `METRICS_BIND_ADDR` and the named `metrics` container
  and Service ports.
- `metrics.serviceMonitor.enabled` renders a
  `monitoring.coreos.com/v1` `ServiceMonitor` that scrapes the chart Service's
  named `metrics` port at `/metrics`. The `ServiceMonitor` CRD is owned by the
  Prometheus Operator and is **not** installed by this chart; enabling this
  option requires that CRD to already exist in your cluster.
- `additionalLabels` are merged into the `ServiceMonitor` labels so a
  Prometheus Operator instance can select it.

The chart rejects invalid ports, non-positive durations, and a
`scrapeTimeout` greater than `interval` at render time.

To verify metrics locally with Docker Compose:

```bash
curl --fail http://localhost:9090/metrics
```

## PodDisruptionBudget

NisseFHIR can render a `policy/v1` `PodDisruptionBudget` to protect against
voluntary disruptions (node drains, autoscaling) taking down every replica at
once. It is disabled by default so single-replica installs are unaffected.

```yaml
podDisruptionBudget:
  enabled: false
  # minAvailable: 1
  # maxUnavailable: "25%"
```

- `podDisruptionBudget.enabled` renders the PodDisruptionBudget.
- When enabled, set **exactly one** of `minAvailable` or `maxUnavailable`.
  Both accept an integer replica count or a percentage string such as `"50%"`.
  Setting both (or neither) is rejected at render time.
- The PDB selector matches the Deployment selector labels.
- The chart prints a note during install if a PDB is enabled with a single
  replica, since such a PDB can block voluntary disruptions until the replica
  is ready again.

## Notes

- `config.jwksUrl` maps to the server's `JWT_JWKS_URI` environment variable.
- `config.shutdownTimeoutSecs` maps to `SHUTDOWN_TIMEOUT_SECS` and bounds the graceful-shutdown drain window (default `10` seconds).
- `config.dbPoolMinConnections`, `config.dbPoolMaxConnections`, `config.dbPoolIdleTimeoutSecs`, and `config.dbPoolMaxLifetimeSecs` map to the `DB_POOL_*` environment variables and tune the PostgreSQL connection pool. Leave them unset to use the server defaults.
- `config.jwtSecret.delivery=file` only changes how the Secret reaches the container; the source Secret configuration stays the same.
- If `cnpg.enabled=true`, the chart uses the CloudNativePG application Secret for `DATABASE_URL`.
