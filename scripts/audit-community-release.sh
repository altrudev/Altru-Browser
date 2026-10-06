#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() { printf 'BLOCK: %s\n' "$*" >&2; exit 1; }
pass() { printf 'PASS: %s\n' "$*"; }

required=(
  README.md
  ARCHITECTURE.md
  ROADMAP.md
  DEVELOPMENT.md
  GOVERNANCE.md
  CODE_OF_CONDUCT.md
  SUPPORT.md
  CHANGELOG.md
  CONTRIBUTING.md
  SECURITY.md
  COMMUNITY.md
  LICENSE-MIT
  LICENSE-APACHE
  NOTICE
  THIRD_PARTY_NOTICES.md
  TRADEMARKS.md
  assets/BRAND-LICENSE.md
  assets/README.md
  docs/BRAND.md
  docs/COMMUNITY-RELEASE-GATE.md
  docs/OPEN-SOURCE-BOUNDARY.md
  docs/NATIVE-ENGINE-ROADMAP.md
  docs/PLATFORM-MATRIX.md
  docs/PROJECT-STATUS.md
  docs/CONFORMANCE.md
  docs/PRIVACY.md
  docs/THREAT-MODEL.md
  docs/RELEASE-PROCESS.md
)
for file in "${required[@]}"; do
  [[ -f "$file" ]] || fail "missing required public file: $file"
done
pass "required community files present"

grep -Fq 'license = "MIT OR Apache-2.0"' Cargo.toml   || fail "Cargo.toml license metadata is not publication-ready"
pass "Cargo license metadata matches dual-license policy"

if git ls-files | grep -Eq '(^|/)(target|evidence/runtime)/'; then
  fail "generated build/runtime artifacts are tracked"
fi
pass "generated build/runtime artifacts are excluded"

secret_re='-----BEGIN (RSA|OPENSSH|EC|DSA) PRIVATE KEY-----|gh[pousr]_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}'
if git grep -nEI "$secret_re" -- . ':!scripts/audit-community-release.sh' >/tmp/altru-release-secrets.txt 2>/dev/null; then
  cat /tmp/altru-release-secrets.txt >&2
  fail "current tree contains a high-confidence secret pattern"
fi
pass "current tree high-confidence secret scan"

history_hits="$(mktemp)"
while read -r rev; do
  git grep -nEI "$secret_re" "$rev" -- . ':!scripts/audit-community-release.sh' >>"$history_hits" 2>/dev/null || true
done < <(git rev-list --all)
if [[ -s "$history_hits" ]]; then
  cat "$history_hits" >&2
  rm -f "$history_hits"
  fail "repository history contains a high-confidence secret pattern"
fi
rm -f "$history_hits"
pass "repository history high-confidence secret scan"

private_re='Frequency-(Core|Dev|Conduit)|purpose-bound credentials|customer evidence|credential broker secret'
if git grep -nEI "$private_re" -- . 2>/dev/null \
  | grep -Ev '^(scripts/audit-community-release\.sh|docs/OPEN-SOURCE-BOUNDARY\.md|README\.md|CONTRIBUTING\.md|SECURITY\.md|docs/COMMUNITY-RELEASE-GATE\.md):' \
  >/tmp/altru-release-private.txt; then
  cat /tmp/altru-release-private.txt >&2
  fail "possible private Frequency implementation reference in public source"
fi
pass "private-assurance boundary scan"

cargo fmt --check
pass "rustfmt"

cargo clippy --all-targets -- -D warnings
pass "clippy all targets"

cargo test
pass "debug tests"

cargo test --release
pass "release tests"

cargo test --features taffy-layout
pass "feature-gated Taffy candidate tests"

scripts/verify-native-platforms.sh
pass "native platform verification"

printf 'ALLOW-CANDIDATE: release audit passed for %s\n' "$(git rev-parse HEAD)"
