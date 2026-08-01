#!/usr/bin/env bash
# Write the release manifest machines check for updates (Linux live only).
#
#   { "version", "built", "commit",
#     "live_amd64_sha256", "live_arm64_sha256",
#     "software_commit", "software_sha256", "software_url" }
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

LIVE_DIR="${LIVE_DIR:-dist}"
newest_live() { ls -1t "$LIVE_DIR"/teddyos-*-"$1"-*.iso 2>/dev/null | head -1; }
LIVE_X86_SRC="${LIVE_X86_SRC:-$(newest_live amd64)}"
LIVE_ARM_SRC="${LIVE_ARM_SRC:-$(newest_live arm64)}"
OUT="${MANIFEST_OUT:-manifest.json}"

VERSION="$(cat VERSION 2>/dev/null || echo 0.0.0)"
COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
  COMMIT="$COMMIT-dirty"
fi
BUILT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

sha_or_empty() {
  if [ -n "${1:-}" ] && [ -f "$1" ]; then
    shasum -a 256 "$1" | cut -d' ' -f1
  else
    echo ""
  fi
}

X86_SHA="$(sha_or_empty "$LIVE_X86_SRC")"
ARM_SHA="$(sha_or_empty "$LIVE_ARM_SRC")"

PAYLOAD="${PAYLOAD_FILE:-dist/teddyos-update.tar.gz}"
SW_SHA=""; SW_COMMIT=""; SW_URL=""
if [ -f "$PAYLOAD" ]; then
  SW_SHA="$(shasum -a 256 "$PAYLOAD" | cut -d' ' -f1)"
  SW_COMMIT="$(tar -xzOf "$PAYLOAD" ./PAYLOAD 2>/dev/null || tar -xzOf "$PAYLOAD" PAYLOAD 2>/dev/null || true)"
  SW_COMMIT="$(printf '%s' "$SW_COMMIT" | sed -n 's/^commit=//p')"
  SW_URL="${PUBLISH_URL:-https://teddysearch.com/tsearch/os}/teddyos-update.tar.gz"
fi

cat > "$OUT" <<JSON
{
  "version": "$VERSION",
  "built": "$BUILT",
  "commit": "$COMMIT",
  "live_amd64_sha256": "$X86_SHA",
  "live_arm64_sha256": "$ARM_SHA",
  "software_commit": "$SW_COMMIT",
  "software_sha256": "$SW_SHA",
  "software_url": "$SW_URL"
}
JSON

echo "wrote $OUT"
cat "$OUT"

if command -v python3 >/dev/null; then
  python3 - "$OUT" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))
for k in ("version", "commit", "software_url"):
    if k not in m:
        sys.exit(f"manifest missing {k}")
print("manifest shape ok")
PY
fi
