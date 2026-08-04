#!/usr/bin/env bash
# Render tests for the NisseFHIR chart's optional PodDisruptionBudget.
#
# These tests render the chart with `helm template` and assert on the output,
# covering: PDB disabled (default), enabled with minAvailable, enabled with
# maxUnavailable (percentage), single- and multi-replica rendering, and invalid
# values (both/neither of minAvailable/maxUnavailable set).
#
# Run from the chart directory:
#   ./tests/test_pdb_render.sh
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

# pdb_doc <rendered-manifest> -> the PodDisruptionBudget YAML document (empty if absent)
pdb_doc() {
    awk '/^# Source: .*poddisruptionbudget\.yaml$/{flag=1; next} /^---$/{if(flag)exit} flag' <<<"$1"
}

expect_render_fail() {
    local desc="$1"; shift
    if helm template "test" . "$@" >/dev/null 2>&1; then
        fail "$desc (helm template unexpectedly succeeded)"
    else
        note "$desc"
    fi
}

DEFAULT="$(render default)"
MIN_ONE="$(render min-one \
    --set podDisruptionBudget.enabled=true \
    --set podDisruptionBudget.minAvailable=1)"
MIN_TWO="$(render min-two \
    --set replicaCount=3 \
    --set podDisruptionBudget.enabled=true \
    --set podDisruptionBudget.minAvailable=2)"
MAX_PCT="$(render max-pct \
    --set replicaCount=4 \
    --set podDisruptionBudget.enabled=true \
    --set podDisruptionBudget.maxUnavailable=25%)"

MIN_ONE_PDB="$(pdb_doc "$MIN_ONE")"
MIN_TWO_PDB="$(pdb_doc "$MIN_TWO")"
MAX_PCT_PDB="$(pdb_doc "$MAX_PCT")"

# ---- disabled by default --------------------------------------------------
assert_not_contains "default: no PDB rendered" "$DEFAULT" 'kind: PodDisruptionBudget'

# ---- enabled with minAvailable (single replica) ----------------------------
assert_contains "min-one: PDB rendered"          "$MIN_ONE_PDB" 'kind: PodDisruptionBudget'
assert_contains "min-one: uses policy/v1"        "$MIN_ONE_PDB" 'apiVersion: policy/v1'
assert_contains "min-one: minAvailable set"      "$MIN_ONE_PDB" 'minAvailable: 1'
assert_contains "min-one: selector name"         "$MIN_ONE_PDB" 'app.kubernetes.io/name: nissefhir'
assert_contains "min-one: selector instance"     "$MIN_ONE_PDB" 'app.kubernetes.io/instance: test'
assert_not_contains "min-one: no maxUnavailable" "$MIN_ONE_PDB" 'maxUnavailable:'

# ---- enabled with minAvailable (multi replica) -----------------------------
assert_contains "min-two: PDB rendered"          "$MIN_TWO_PDB" 'kind: PodDisruptionBudget'
assert_contains "min-two: minAvailable set"      "$MIN_TWO_PDB" 'minAvailable: 2'
assert_not_contains "min-two: no maxUnavailable" "$MIN_TWO_PDB" 'maxUnavailable:'

# ---- enabled with maxUnavailable (percentage) ------------------------------
assert_contains "max-pct: PDB rendered"          "$MAX_PCT_PDB" 'kind: PodDisruptionBudget'
assert_contains "max-pct: maxUnavailable set"    "$MAX_PCT_PDB" 'maxUnavailable: 25%'
assert_not_contains "max-pct: no minAvailable"   "$MAX_PCT_PDB" 'minAvailable:'

# ---- zero-valued settings are still rendered -------------------------------
ZERO="$(render zero \
    --set podDisruptionBudget.enabled=true \
    --set podDisruptionBudget.maxUnavailable=0)"
ZERO_PDB="$(pdb_doc "$ZERO")"
assert_contains "zero: maxUnavailable: 0 rendered" "$ZERO_PDB" 'maxUnavailable: 0'
assert_not_contains "zero: no minAvailable"        "$ZERO_PDB" 'minAvailable:'

# ---- invalid values --------------------------------------------------------
expect_render_fail "invalid: both minAvailable and maxUnavailable" \
    --set podDisruptionBudget.enabled=true \
    --set podDisruptionBudget.minAvailable=1 \
    --set podDisruptionBudget.maxUnavailable=1
expect_render_fail "invalid: neither minAvailable nor maxUnavailable" \
    --set podDisruptionBudget.enabled=true
expect_render_fail "invalid: null minAvailable" \
    --set podDisruptionBudget.enabled=true \
    --set-json podDisruptionBudget.minAvailable=null

if [[ "$FAILURES" -gt 0 ]]; then
    printf '\n%d render test(s) failed\n' "$FAILURES" >&2
    exit 1
fi
printf '\nall render tests passed\n'
