#!/usr/bin/env bash
# Publish both ISOs to teddysearch.com/tsearch/os/.
#
# Why this exists: the download page offers two images that are not
# interchangeable, and nothing republishes them when the OS changes. Done by
# hand it is a dozen steps in a fixed order, and the failure that matters -
# shipping an image that does not boot - is silent until a stranger tries it.
#
# This writes to a public web root, so the guards here are load-bearing:
#   * refuses a dirty tree, so what is published is reproducible from a commit
#   * builds BOTH images from that commit, release profile, never debug
#   * BOOTS both before uploading anything - the whole point of the target
#   * uploads beside the live file and moves into place, so a visitor can
#     never download a half-written ISO
#   * re-verifies the checksums on the server, then again over HTTPS, rather
#     than trusting rsync's exit code
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

HOST="${PUBLISH_HOST:-root@159.203.75.186}"
DIR="${PUBLISH_DIR:-/var/www/tsearch/os}"
URL="${PUBLISH_URL:-https://teddysearch.com/tsearch/os}"
ALLOW_DIRTY="${ALLOW_DIRTY:-0}"
DRY_RUN="${DRY_RUN:-0}"
SKIP_BOOT_TEST="${SKIP_BOOT_TEST:-0}"

X86="${IMAGE_NAME:-os}.iso"
ARM="${ARM64_IMAGE_NAME:-os-arm64}.iso"

die() { echo "error: $*" >&2; exit 1; }
step() { printf '\n=== %s\n' "$*"; }

command -v rsync >/dev/null 2>&1 || die "rsync not found"
command -v shasum >/dev/null 2>&1 || die "shasum not found"

# 1. Reproducibility. An artifact nobody can rebuild is an artifact nobody can
#    debug when a stranger reports it broken.
COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
if [ "$ALLOW_DIRTY" != "1" ]; then
  if [ -n "$(git status --porcelain 2>/dev/null | grep -v '^?? ')" ]; then
    git status --short | grep -v '^?? ' >&2
    die "working tree has uncommitted changes — commit them, or ALLOW_DIRTY=1 to publish anyway"
  fi
fi

step "building release images at $COMMIT"
make iso KERNEL_PROFILE=release
make arm64-iso
[ -f "$X86" ] || die "$X86 missing after build"
[ -f "$ARM" ] || die "$ARM missing after build"

# 2. Boot them. This is the step that makes the target worth having: every
#    other failure here is loud, and this one is not.
if [ "$SKIP_BOOT_TEST" = "1" ]; then
  echo "WARNING: skipping the boot test — publishing an untested image" >&2
else
  step "booting both images before anything is published"
  ./scripts/smoke-qemu.sh
  ./scripts/smoke-arm64.sh
fi

step "checksums"
shasum -a 256 "$X86" "$ARM" | sed 's#  .*/#  #' > SHA256SUMS
cat SHA256SUMS
cat > BUILD-INFO.txt <<EOF
teddy OS images
commit:  $COMMIT
built:   $(date -u '+%Y-%m-%dT%H:%M:%SZ')
profile: release (both architectures)

$X86        x86-64 — Intel/AMD machines
$ARM  ARM64 — Apple Silicon and other ARM64 machines

Verify:  shasum -a 256 -c SHA256SUMS
EOF

if [ "$DRY_RUN" = "1" ]; then
  step "DRY RUN — would publish to $HOST:$DIR"
  ls -la "$X86" "$ARM" SHA256SUMS BUILD-INFO.txt
  exit 0
fi

# 3. Upload beside the live files, then move into place. rsync writes into the
#    destination name as it goes; without this a download that starts
#    mid-transfer gets a truncated ISO that fails to boot for no visible reason.
step "uploading to $HOST:$DIR"
ssh "$HOST" "mkdir -p '$DIR'"
rsync -az "$X86" "$HOST:$DIR/.$X86.incoming"
rsync -az "$ARM" "$HOST:$DIR/.$ARM.incoming"
rsync -az SHA256SUMS BUILD-INFO.txt "$HOST:$DIR/"
ssh "$HOST" "cd '$DIR' && mv -f '.$X86.incoming' '$X86' && mv -f '.$ARM.incoming' '$ARM' && chmod 644 '$X86' '$ARM' SHA256SUMS BUILD-INFO.txt"

step "verifying on the server"
ssh "$HOST" "cd '$DIR' && sha256sum -c SHA256SUMS" ||
  die "checksums do not match on the server — the upload is corrupt, the old images may be gone"

# 4. And over HTTPS, because "the file is on the server" and "a visitor can
#    fetch it" are different claims — nginx, permissions and caching all sit
#    between them.
step "verifying over HTTPS from $URL"
for f in "$X86" "$ARM" SHA256SUMS; do
  code="$(curl -s -o /dev/null -w '%{http_code}' "$URL/$f")"
  [ "$code" = "200" ] || die "$URL/$f returned HTTP $code"
  printf '  %-16s %s\n' "$f" "HTTP $code"
done

tmp="$(mktemp -t os-publish-verify)"
curl -s -o "$tmp" "$URL/$ARM"
got="$(shasum -a 256 "$tmp" | awk '{print $1}')"
want="$(awk -v f="$ARM" '$2 == f {print $1}' SHA256SUMS)"
rm -f "$tmp"
[ "$got" = "$want" ] || die "the ARM64 image served over HTTPS does not match its published checksum"

step "published $COMMIT"
echo "  $URL/$X86"
echo "  $URL/$ARM"
echo "  page: ${URL%/os}/os.html"
