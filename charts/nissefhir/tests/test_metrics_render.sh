#!/usr/bin/env bash
# Render tests for the NisseFHIR chart's optional Prometheus metrics surface.
#
# These tests render the chart with `helm template` and assert on the output,
# covering: metrics disabled, enabled without a ServiceMonitor, enabled with a
# ServiceMonitor (including additional labels), and invalid values.
#
# Run from the chart directory:
#   ./tests/test_metrics_render.sh
set -euo pipefail

CHART_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$CHART_DIR"

FAILURES=0

note()  { printf 'ok   - %s\n' "$*"; }
fail()  { printf 'FAIL - %s\n' "$*"; FAILURES=$((FAILURES + 1)); }

# assert_contains <description> <haystack> <needle>
assert_contains() {
    local desc="$1" haystack="$2" needle="$3"
    if grep -qF -- "$needle" <<<"$haystack"; then
        note "$desc"
    else
        fail "$desc (missing: $needle)"
    fi
}

# assert_not_contains <description> <haystack> <needle>
assert_not_contains() {
    local desc="$1" haystack="$2" needle="$3"
    if grep -qF -- "$needle" <<<"$haystack"; then
        fail "$desc (unexpected: $needle)"
    else
        note "$desc"
    fi
}

# render <name> <args...> -> outputs rendered manifest to stdout
render() {
    helm template "test" . "${@:2}"
}

# expect_render_fail <description> <args...> -- expects helm template to error
expect_render_fail() {
    local desc="$1"; shift
    if helm template "test" . "$@" >/dev/null 2>&1; then
        fail "$desc (helm template unexpectedly succeeded)"
    else
        note "$desc"
    fi
}

DEFAULT="$(render default)"
DISABLED="$(render disabled --set metrics.enabled=false)"
SM="$(render sm --set metrics.serviceMonitor.enabled=true \
        --set metrics.serviceMonitor.additionalLabels.team=ops \
        --set metrics.serviceMonitor.interval=30s \
        --set metrics.serviceMonitor.scrapeTimeout=10s)"
SM_MIXED_UNITS="$(render sm-mixed-units \
        --set metrics.serviceMonitor.enabled=true \
        --set metrics.serviceMonitor.interval=1m \
        --set metrics.serviceMonitor.scrapeTimeout=30s)"

# ---- metrics enabled by default -------------------------------------------
assert_contains "default: METRICS_ENABLED=true"        "$DEFAULT" 'name: METRICS_ENABLED'
assert_contains "default: METRICS_BIND_ADDR"           "$DEFAULT" 'name: METRICS_BIND_ADDR'
assert_contains "default: bind addr uses configured port" "$DEFAULT" 'value: "0.0.0.0:9090"'
assert_contains "default: metrics container port"      "$DEFAULT" 'name: metrics'
assert_contains "default: metrics service port"        "$DEFAULT" 'targetPort: metrics'
assert_not_contains "default: no ServiceMonitor rendered" "$DEFAULT" 'kind: ServiceMonitor'

# ---- metrics disabled -----------------------------------------------------
assert_contains "disabled: METRICS_ENABLED=false"      "$DISABLED" 'name: METRICS_ENABLED'
assert_contains "disabled: sets false value"           "$DISABLED" 'value: "false"'
assert_not_contains "disabled: no METRICS_BIND_ADDR"   "$DISABLED" 'METRICS_BIND_ADDR'
assert_not_contains "disabled: no metrics container port" "$DISABLED" 'containerPort: 9090'
assert_not_contains "disabled: no metrics service port"   "$DISABLED" 'targetPort: metrics'
assert_not_contains "disabled: no ServiceMonitor rendered" "$DISABLED" 'kind: ServiceMonitor'

# ---- metrics enabled with ServiceMonitor ----------------------------------
assert_contains "sm: ServiceMonitor rendered"          "$SM" 'kind: ServiceMonitor'
assert_contains "sm: selects metrics port"             "$SM" 'port: metrics'
assert_contains "sm: scrapes /metrics"                 "$SM" 'path: /metrics'
assert_contains "sm: configured interval"              "$SM" 'interval: 30s'
assert_contains "sm: configured scrape timeout"        "$SM" 'scrapeTimeout: 10s'
assert_contains "sm: additional label merged"          "$SM" 'team: ops'
assert_contains "sm: keeps metrics port"               "$SM" 'targetPort: metrics'
assert_contains "sm: accepts valid mixed duration units" "$SM_MIXED_UNITS" 'interval: 1m'
assert_contains "sm: preserves mixed-unit scrape timeout" "$SM_MIXED_UNITS" 'scrapeTimeout: 30s'

# ---- invalid values -------------------------------------------------------
expect_render_fail "invalid: port out of range" \
    --set metrics.port=99999
expect_render_fail "invalid: scrapeTimeout > interval" \
    --set metrics.serviceMonitor.enabled=true \
    --set metrics.serviceMonitor.interval=10s \
    --set metrics.serviceMonitor.scrapeTimeout=30s
expect_render_fail "invalid: mixed-unit scrapeTimeout > interval" \
    --set metrics.serviceMonitor.enabled=true \
    --set metrics.serviceMonitor.interval=30s \
    --set metrics.serviceMonitor.scrapeTimeout=1m
expect_render_fail "invalid: non-positive interval" \
    --set metrics.serviceMonitor.enabled=true \
    --set metrics.serviceMonitor.interval=0s
expect_render_fail "invalid: non-positive scrapeTimeout" \
    --set metrics.serviceMonitor.enabled=true \
    --set metrics.serviceMonitor.scrapeTimeout=0s
expect_render_fail "invalid: malformed interval" \
    --set metrics.serviceMonitor.enabled=true \
    --set metrics.serviceMonitor.interval=30

if [[ "$FAILURES" -gt 0 ]]; then
    printf '\n%d render test(s) failed\n' "$FAILURES" >&2
    exit 1
fi
printf '\nall render tests passed\n'
