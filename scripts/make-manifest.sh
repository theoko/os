#!/usr/bin/env bash
# Write the release manifest that machines check for updates.
#
# scripts/publish-os.sh already ships both ISOs with SHA256SUMS and boot-tests
# them first. What nothing produced was a document a *machine* can read to
# learn a newer build exists — SHA256SUMS carries checksums but no version, so
# a guest cannot tell "different bytes" from "newer release".
#
# Emits manifest.json beside the images:
#
#   { "version": "0.11.0", "built": "...", "commit": "...",
#     "x86_sha256": "...", "arm_sha256": "..." }
#
# Run it after building both ISOs; publish-os.sh can then upload it alongside
# them. Kept separate from publish-os.sh deliberately — that script is owned by
# another workstream and rsyncs to a live web root.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

X86="${IMAGE_NAME:-os}.iso"
ARM="${ARM64_IMAGE_NAME:-os-arm64}.iso"
OUT="${MANIFEST_OUT:-manifest.json}"

for f in "$X86" "$ARM"; do
  [[ -f "$f" ]] || { echo "error: $f missing — run 'make iso' and 'make arm64-iso' first" >&2; exit 1; }
done

VERSION="$(cat VERSION 2>/dev/null || echo 0.0.0)"
COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
# A manifest describing a dirty tree cannot be rebuilt from a commit, so say so
# in the field rather than publishing a commit that does not match the bytes.
if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
  COMMIT="$COMMIT-dirty"
fi
BUILT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
X86_SHA="$(shasum -a 256 "$X86" | cut -d' ' -f1)"
ARM_SHA="$(shasum -a 256 "$ARM" | cut -d' ' -f1)"

# The software payload, if one has been built. This is what lets an installed
# machine update without downloading an ISO: teddyos-update reads
# software_commit to decide whether it is behind, software_sha256 to verify
# what it fetched, and software_url to find it.
#
# Compared by COMMIT rather than version. Two builds of 0.13.0 are routinely
# different software — that is most of what a day's work produces — and a
# version-only comparison would call them identical and never update anything.
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
  "x86_sha256": "$X86_SHA",
  "arm_sha256": "$ARM_SHA",
  "software_commit": "$SW_COMMIT",
  "software_sha256": "$SW_SHA",
  "software_url": "$SW_URL"
}
JSON

echo "wrote $OUT"
cat "$OUT"

# The manifest is only useful if the checker accepts it. Catching a shape
# mismatch here beats discovering it from a machine that reports "no update"
# forever.
if command -v python3 >/dev/null; then
  python3 - "$OUT" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))
missing = [k for k in ("version", "x86_sha256", "arm_sha256") if not m.get(k)]
if missing:
    sys.exit(f"manifest is missing {missing} — update.check would refuse it")
print("manifest has every field update.check requires")
PY
fi

echo
echo "publish it beside the images, e.g.:"
echo "    scp $OUT root@159.203.75.186:/var/www/tsearch/os/manifest.json"
