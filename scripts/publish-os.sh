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

# 1b. Whose documents are in the image. Search is compiled in, so whatever
#     search/corpus.json holds at build time ships inside the ISO — every
#     title, and 320 characters of every document. A corpus baked from the
#     workspace index is personal: family papers, resumes, trading notes.
#     This target uploads to a public URL, so that build must never be the
#     one a stranger downloads.
if grep -q '"personal": true' search/corpus.json 2>/dev/null; then
  die "search/corpus.json is baked from your workspace index — publishing it
       would put your own documents on $URL.

       Build a public corpus first, then put yours back:

         ./scripts/bake-corpus.py --seed-only && make publish-os
         ./scripts/bake-corpus.py && make arm64-iso"
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

# The machine-readable half. SHA256SUMS carries checksums but no version, so a
# machine reading only that can tell "different bytes" from nothing else — it
# cannot say whether it is behind. Without this file `CALL update.check` on
# every installed machine reports state=undetermined forever, because the URL
# it reads answers with the /tsearch/ catch-all page instead of a manifest.
step "release manifest"
./scripts/make-manifest.sh

if [ "$DRY_RUN" = "1" ]; then
  step "DRY RUN — would publish to $HOST:$DIR"
  ls -la "$X86" "$ARM" SHA256SUMS BUILD-INFO.txt manifest.json
  exit 0
fi

# 3. Upload beside the live files, then move into place. rsync writes into the
#    destination name as it goes; without this a download that starts
#    mid-transfer gets a truncated ISO that fails to boot for no visible reason.
step "uploading to $HOST:$DIR"
ssh "$HOST" "mkdir -p '$DIR'"
rsync -az "$X86" "$HOST:$DIR/.$X86.incoming"
rsync -az "$ARM" "$HOST:$DIR/.$ARM.incoming"
rsync -az SHA256SUMS BUILD-INFO.txt manifest.json "$HOST:$DIR/"
ssh "$HOST" "cd '$DIR' && mv -f '.$X86.incoming' '$X86' && mv -f '.$ARM.incoming' '$ARM' && chmod 644 '$X86' '$ARM' SHA256SUMS BUILD-INFO.txt manifest.json"

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

# manifest.json is checked by its BODY, not its status. /tsearch/ has a
# catch-all: a path that does not exist answers HTTP 200 with the homepage. A
# 200 here would therefore prove the manifest is published *and* prove it is
# missing, equally well — and the updater reading it would see 88 KB of HTML.
tmp="$(mktemp -t os-publish-manifest)"
curl -s -o "$tmp" "$URL/manifest.json"
if ! grep -q '"x86_sha256"' "$tmp"; then
  head -c 120 "$tmp" >&2; echo >&2
  rm -f "$tmp"
  die "manifest.json did not publish — the catch-all is answering, so update.check sees a web page"
fi
rm -f "$tmp"
printf '  %-16s %s\n' "manifest.json" "is a manifest"

# A CDN fronts this origin and caches images for hours, so the bare URL can
# serve the PREVIOUS release long after the upload succeeded — which then fails
# its own published checksum and reads to a visitor as a tampered download.
# `os.html` therefore links with ?v=<commit>, and this checks the same URL a
# visitor will actually fetch. The bare URL is checked too, and only warned
# about: it is expected to be stale until the edge expires it.
tmp="$(mktemp -t os-publish-verify)"
curl -s -o "$tmp" "$URL/$ARM?v=$COMMIT"
got="$(shasum -a 256 "$tmp" | awk '{print $1}')"
want="$(awk -v f="$ARM" '$2 == f {print $1}' SHA256SUMS)"
rm -f "$tmp"
[ "$got" = "$want" ] || die "the ARM64 image served over HTTPS does not match its published checksum"

tmp="$(mktemp -t os-publish-cache)"
curl -s -o "$tmp" "$URL/$ARM"
cached="$(shasum -a 256 "$tmp" | awk '{print $1}')"
rm -f "$tmp"
if [ "$cached" != "$want" ]; then
  echo "  note: the un-versioned URL is still serving a cached older image."
  echo "        Visitors follow the ?v= link from os.html, so this is cosmetic;"
  echo "        purge the CDN if you want the bare URL correct immediately."
fi

step "published $COMMIT"
echo "  $URL/$X86"
echo "  $URL/$ARM"
echo "  page: ${URL%/os}/os.html"
