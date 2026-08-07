#!/usr/bin/env bash
# Render-time validation tests for the NisseFHIR chart's values contract.
#
# These tests render the chart with `helm template` and assert that
# contradictory or incomplete value combinations are rejected (via
# values.schema.json or templates/validate.yaml) and that the README's
# documented install examples render successfully.
#
# Run from the chart directory:
#   ./tests/test_values_validation.sh
set -euo pipefail

CHART_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$CHART_DIR"

FAILURES=0

note()  { printf 'ok   - %s\n' "$*"; }
fail()  { printf 'FAIL - %s\n' "$*"; FAILURES=$((FAILURES + 1)); }

assert_contains() {
    local desc="$1" haystack="$2" needle="$3"
    if grep -qF -- "$needle" <<<"$haystack"; then
        note "$desc"
    else
        fail "$desc (missing: $needle)"
    fi
}

assert_not_contains() {
    local desc="$1" haystack="$2" needle="$3"
    if grep -qF -- "$needle" <<<"$haystack"; then
        fail "$desc (unexpected: $needle)"
    else
        note "$desc"
    fi
}

render() {
    helm template "test" . "${@:2}"
}

expect_render_ok() {
    local desc="$1"; shift
    if helm template "test" . "$@" >/dev/null 2>&1; then
        note "$desc"
    else
        fail "$desc (helm template unexpectedly failed)"
    fi
}

expect_render_fail() {
    local desc="$1"; shift
    if helm template "test" . "$@" >/dev/null 2>&1; then
        fail "$desc (helm template unexpectedly succeeded)"
    else
        note "$desc"
    fi
}

# ---- defaults --------------------------------------------------------------
expect_render_ok "default values render"

# ---- unsupported enum values are rejected ----------------------------------
expect_render_fail "invalid: jwtMode=none" \
    --set config.jwtMode=none
expect_render_fail "invalid: jwtSecret.delivery=volume" \
    --set config.jwtSecret.delivery=volume
expect_render_fail "invalid: relative jwtSecret.mountPath" \
    --set config.jwtSecret.delivery=file \
    --set config.jwtSecret.mountPath=secrets
# mountPath is unused with env delivery, so a relative value is tolerated there.
expect_render_ok "env delivery tolerates unused relative mountPath" \
    --set config.jwtSecret.delivery=env \
    --set config.jwtSecret.mountPath=secrets

# ---- conditional auth requirements -----------------------------------------
expect_render_fail "invalid: jwtMode=jwks without jwksUrl" \
    --set config.jwtMode=jwks
expect_render_fail "invalid: static without create or existingSecret" \
    --set config.jwtSecret.create=false

# ---- conditional database requirements -------------------------------------
expect_render_fail "invalid: cnpg disabled without external database" \
    --set cnpg.enabled=false

# ---- README examples render -------------------------------------------------
# An explicit existingSecret.name takes precedence over create=true, so the
# documented commands must reference the existing Secret and must not render
# the chart-managed one.
EXISTING_SECRET="$(render existing-secret \
    --set config.jwtMode=static \
    --set config.jwtSecret.existingSecret.name=my-release-jwt)"
assert_contains "README: existing secret referenced" \
    "$EXISTING_SECRET" 'name: my-release-jwt'
assert_not_contains "README: existing secret does not render generated Secret" \
    "$EXISTING_SECRET" 'name: test-nissefhir-jwt'

expect_render_ok "README: chart-managed secret" \
    --set config.jwtMode=static \
    --set config.jwtSecret.create=true
expect_render_ok "README: explicit secret value" \
    --set config.jwtMode=static \
    --set config.jwtSecret.create=true \
    --set-string config.jwtSecret.value=0123456789abcdef
expect_render_ok "README: jwks mode" \
    --set config.jwtMode=jwks \
    --set config.jwksUrl=https://issuer.example.com/.well-known/jwks.json
expect_render_ok "README: external database via url" \
    --set cnpg.enabled=false \
    --set cnpg.externalDatabase.url=postgres://user:password@host:5432/fhir
expect_render_ok "README: external database via existingSecret" \
    --set cnpg.enabled=false \
    --set cnpg.externalDatabase.existingSecret.name=my-db-secret
expect_render_ok "external database via extraEnv DATABASE_URL" \
    --set cnpg.enabled=false \
    --set extraEnv[0].name=DATABASE_URL \
    --set extraEnv[0].value=postgres://user:password@host:5432/fhir
expect_render_ok "external database via extraEnv DATABASE_URL_FILE" \
    --set cnpg.enabled=false \
    --set extraEnv[0].name=DATABASE_URL_FILE \
    --set extraEnv[0].value=/run/secrets/database-url
expect_render_ok "external database via extraEnv valueFrom" \
    --set cnpg.enabled=false \
    --set extraEnv[0].name=DATABASE_URL \
    --set extraEnv[0].valueFrom.secretKeyRef.name=my-db-secret \
    --set extraEnv[0].valueFrom.secretKeyRef.key=uri
expect_render_fail "invalid: extraEnv DATABASE_URL without value or valueFrom" \
    --set cnpg.enabled=false \
    --set extraEnv[0].name=DATABASE_URL
expect_render_ok "null existingSecret falls back to chart-managed secret" \
    --set config.jwtSecret.create=true \
    --set-json config.jwtSecret.existingSecret=null

# ---- file delivery wires mount path into the deployment --------------------
FILE_DELIVERY="$(render file-delivery \
    --set config.jwtMode=static \
    --set config.jwtSecret.existingSecret.name=my-release-jwt \
    --set config.jwtSecret.delivery=file)"
assert_contains "file delivery: JWT_SECRET_FILE set" \
    "$FILE_DELIVERY" 'name: JWT_SECRET_FILE'
assert_contains "file delivery: existing secret mounted" \
    "$FILE_DELIVERY" 'secretName: my-release-jwt'
assert_contains "file delivery: default mount path used" \
    "$FILE_DELIVERY" 'value: "/var/run/secrets/nissefhir/jwt-secret"'
assert_contains "file delivery: secret volume mounted" \
    "$FILE_DELIVERY" 'mountPath: "/var/run/secrets/nissefhir"'

CUSTOM_PATH="$(render custom-path \
    --set config.jwtSecret.delivery=file \
    --set config.jwtSecret.mountPath=/run/jwt)"
assert_contains "file delivery: custom mount path used" \
    "$CUSTOM_PATH" 'value: "/run/jwt/jwt-secret"'

EXTERNAL_DB="$(render external-db \
    --set cnpg.enabled=false \
    --set cnpg.externalDatabase.url=postgres://user:password@host:5432/fhir)"
assert_contains "external db: DATABASE_URL set" \
    "$EXTERNAL_DB" 'value: "postgres://user:password@host:5432/fhir"'

if [[ "$FAILURES" -gt 0 ]]; then
    printf '\n%d render test(s) failed\n' "$FAILURES" >&2
    exit 1
fi
printf '\nall render tests passed\n'
