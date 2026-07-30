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

## Notes

- `config.jwksUrl` maps to the server's `JWT_JWKS_URI` environment variable.
- `config.jwtSecret.delivery=file` only changes how the Secret reaches the container; the source Secret configuration stays the same.
- If `cnpg.enabled=true`, the chart uses the CloudNativePG application Secret for `DATABASE_URL`.
