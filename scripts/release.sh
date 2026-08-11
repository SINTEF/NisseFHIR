#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>  (e.g. 0.1.1)"
  exit 1
fi

NEW="$1"
DATE=$(date +%Y-%m-%d)

# Detect current version from Cargo.toml
OLD=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
echo "Bumping $OLD → $NEW (date: $DATE)"
echo ""

# ── Pre-flight checks ──────────────────────────────────────────────
echo "=== Running pre-flight checks ==="

echo "→ cargo fmt --check"
cargo fmt --all -- --check

echo "→ cargo clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings

echo "→ cargo test (lib + unit)"
cargo test --lib

echo "→ pre-commit"
pre-commit run --all-files

echo ""
echo "=== All checks passed ==="
echo ""

# ── Version bump ────────────────────────────────────────────────────

# 1. Cargo.toml
sed -i "0,/^version = \"$OLD\"/s//version = \"$NEW\"/" Cargo.toml

# 2. Cargo.lock (cargo update syncs the lock file)
cargo update --workspace

# 3. Dockerfile OCI label
sed -i "s/org.opencontainers.image.version=\"$OLD\"/org.opencontainers.image.version=\"$NEW\"/" Dockerfile

# 4. compose.yaml image tag
sed -i "s|nissefhir:$OLD|nissefhir:$NEW|" compose.yaml

# 5. Helm Chart.yaml (version + appVersion)
sed -i "s/^version: $OLD/version: $NEW/" charts/nissefhir/Chart.yaml
sed -i "s/^appVersion: \"$OLD\"/appVersion: \"$NEW\"/" charts/nissefhir/Chart.yaml

# 6. CapabilityStatement dates in src/capability.rs
sed -i "s/\"date\": \"[0-9]\{4\}-[0-9]\{2\}-[0-9]\{2\}\"/\"date\": \"$DATE\"/" src/capability.rs
sed -i "s/\"releaseDate\": \"[0-9]\{4\}-[0-9]\{2\}-[0-9]\{2\}\"/\"releaseDate\": \"$DATE\"/" src/capability.rs

# 7. CHANGELOG.md – insert new section after header
ENTRY="## [$NEW] - $DATE\n\n### Changed\n\n- (fill in changes)\n"
sed -i "/^## \[$OLD\]/i\\$ENTRY" CHANGELOG.md
# Update link references
sed -i "s|\[$OLD\]: \(.*\)/releases/tag/$OLD|[$NEW]: \1/releases/tag/$NEW\n[$OLD]: \1/releases/tag/$OLD|" CHANGELOG.md

echo "Files updated. Review with:  git diff"
echo ""
echo "When ready, run:"
echo "  git add -A && git commit -m 'release: $NEW'"
echo "  git tag -a $NEW -m 'Release $NEW'"
echo "  git push origin main && git push origin $NEW"
