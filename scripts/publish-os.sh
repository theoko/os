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

# The live Debian images, which are a different product from the two above and
# were previously unpublishable: they are built inside the guest by
# linux/iso/build-iso.sh, land in dist/ under a build-stamped name, and nothing
# here knew they existed. So the thing a stranger is meant to download could
# not reach the server at all.
#
# Uploaded under a STABLE name rather than the stamped one. That was the
# decision this needed and it settles two others with it: the download link on
# os.html never changes, and because each upload replaces the last there is no
# retention policy to invent and no directory that grows without bound. The
# build id is not lost — it is inside the image at /etc/teddyos-build and on
# its boot splash, which is where someone debugging a stranger's report will
# actually look.
LIVE_DIR="${LIVE_DIR:-dist}"
newest_live() {  # $1 = arch
  ls -1t "$LIVE_DIR"/teddyos-*-"$1"-*.iso 2>/dev/null | head -1
}
LIVE_X86_SRC="${LIVE_X86_SRC:-$(newest_live amd64)}"
LIVE_ARM_SRC="${LIVE_ARM_SRC:-$(newest_live arm64)}"
LIVE_X86="teddyos-amd64.iso"
LIVE_ARM="teddyos-arm64.iso"

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
# The live images are checksummed under the name they will carry on the
# server, not the stamped name on disk, so `shasum -c SHA256SUMS` works for
# someone who downloaded them.
PUBLISH_FILES="$X86 $ARM"
: > SHA256SUMS
shasum -a 256 "$X86" "$ARM" | sed 's#  .*/#  #' >> SHA256SUMS
for pair in "$LIVE_X86_SRC:$LIVE_X86" "$LIVE_ARM_SRC:$LIVE_ARM"; do
  src="${pair%%:*}"; dst="${pair##*:}"
  if [ -n "$src" ] && [ -f "$src" ]; then
    shasum -a 256 "$src" | sed "s#^\\([0-9a-f]*\\)  .*#\\1  $dst#" >> SHA256SUMS
    PUBLISH_FILES="$PUBLISH_FILES $dst"
    echo "  live image: $src -> $dst"
  else
    echo "  no live $dst found in $LIVE_DIR — publishing without it" >&2
  fi
done
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
# The software payload, built before the manifest because the manifest records
# its checksum. This is the half that makes `git push` reach a running machine:
# publish takes what is committed, packages the teddyOS software, and the guest
# installs it without touching its boot media.
step "software update payload"
./scripts/make-update-payload.sh

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

# Room check before the transfer, not after. The live images are ~2.6 GB each
# and this host also serves teddysearch; filling its disk mid-rsync would take
# the site down to publish an OS, and the failure would look like a network
# problem rather than a full volume.
need_kb=0
for f in "$X86" "$ARM" "$LIVE_X86_SRC" "$LIVE_ARM_SRC"; do
  [ -n "$f" ] && [ -f "$f" ] && need_kb=$((need_kb + $(du -k "$f" | cut -f1)))
done
free_kb="$(ssh "$HOST" "df -Pk '$DIR' | tail -1 | awk '{print \$4}'")"
echo "  need $((need_kb / 1024)) MB, free $((free_kb / 1024)) MB"
[ "$free_kb" -gt "$((need_kb + 1048576))" ] ||
  die "not enough room on $HOST: need $((need_kb / 1024)) MB plus 1 GB headroom, have $((free_kb / 1024)) MB"

rsync -az "$X86" "$HOST:$DIR/.$X86.incoming"
rsync -az "$ARM" "$HOST:$DIR/.$ARM.incoming"
# --partial --inplace deliberately NOT used: a resumed partial would be moved
# into place as if whole. 2.6 GB over a home connection is exactly where an
# interrupted transfer is likely.
[ -n "$LIVE_X86_SRC" ] && [ -f "$LIVE_X86_SRC" ] &&
  rsync -az --info=progress2 "$LIVE_X86_SRC" "$HOST:$DIR/.$LIVE_X86.incoming"
[ -n "$LIVE_ARM_SRC" ] && [ -f "$LIVE_ARM_SRC" ] &&
  rsync -az --info=progress2 "$LIVE_ARM_SRC" "$HOST:$DIR/.$LIVE_ARM.incoming"
PAYLOAD="dist/teddyos-update.tar.gz"
if [ -f "$PAYLOAD" ]; then
  shasum -a 256 "$PAYLOAD" | sed "s#^\\([0-9a-f]*\\)  .*#\\1  teddyos-update.tar.gz#" >> SHA256SUMS
  rsync -az "$PAYLOAD" "$HOST:$DIR/.teddyos-update.tar.gz.incoming"
  PUBLISH_FILES="$PUBLISH_FILES teddyos-update.tar.gz"
  moves_extra=" && mv -f '.teddyos-update.tar.gz.incoming' 'teddyos-update.tar.gz'"
else
  moves_extra=""
fi
rsync -az SHA256SUMS BUILD-INFO.txt manifest.json "$HOST:$DIR/"

# One ssh doing every rename, so the set goes live together. A visitor who
# fetches SHA256SUMS between two separate moves gets checksums for an image
# that is still half-uploaded.
moves="mv -f '.$X86.incoming' '$X86' && mv -f '.$ARM.incoming' '$ARM'"
for dst in "$LIVE_X86" "$LIVE_ARM"; do
  case " $PUBLISH_FILES " in
    *" $dst "*) moves="$moves && mv -f '.$dst.incoming' '$dst'" ;;
  esac
done
ssh "$HOST" "cd '$DIR' && $moves$moves_extra && chmod 644 $PUBLISH_FILES SHA256SUMS BUILD-INFO.txt manifest.json"

step "verifying on the server"
ssh "$HOST" "cd '$DIR' && sha256sum -c SHA256SUMS" ||
  die "checksums do not match on the server — the upload is corrupt, the old images may be gone"

# 4. And over HTTPS, because "the file is on the server" and "a visitor can
#    fetch it" are different claims — nginx, permissions and caching all sit
#    between them.
step "verifying over HTTPS from $URL"
for f in $PUBLISH_FILES SHA256SUMS; do
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
