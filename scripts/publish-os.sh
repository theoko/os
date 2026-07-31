#!/usr/bin/env bash
# Publish Linux live images + software update payload to teddysearch.com/tsearch/os/.
#
# Freestanding kernel ISOs are gone from this tree. This only ships:
#   teddyos-amd64.iso / teddyos-arm64.iso  (stable names from dist/ builds)
#   teddyos-update.tar.gz
#   SHA256SUMS, BUILD-INFO.txt, manifest.json
#
# Guards:
#   * refuses a dirty tree unless ALLOW_DIRTY=1
#   * requires at least one live ISO in dist/ (build with make linux-iso)
#   * atomic upload (incoming then rename)
#   * re-verifies checksums on the server and over HTTPS
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

HOST="${PUBLISH_HOST:-root@159.203.75.186}"
DIR="${PUBLISH_DIR:-/var/www/tsearch/os}"
URL="${PUBLISH_URL:-https://teddysearch.com/tsearch/os}"
ALLOW_DIRTY="${ALLOW_DIRTY:-0}"
DRY_RUN="${DRY_RUN:-0}"

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

COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
if [ "$ALLOW_DIRTY" != "1" ]; then
  if [ -n "$(git status --porcelain 2>/dev/null | grep -v '^?? ')" ]; then
    git status --short | grep -v '^?? ' >&2
    die "working tree has uncommitted changes — commit them, or ALLOW_DIRTY=1"
  fi
fi

have_any=0
[ -n "$LIVE_X86_SRC" ] && [ -f "$LIVE_X86_SRC" ] && have_any=1
[ -n "$LIVE_ARM_SRC" ] && [ -f "$LIVE_ARM_SRC" ] && have_any=1
[ "$have_any" = 1 ] || die "no live ISO in $LIVE_DIR — run: make linux-iso"

step "software update payload"
./scripts/make-update-payload.sh

step "checksums"
: > SHA256SUMS
PUBLISH_FILES=""
for pair in "$LIVE_X86_SRC:$LIVE_X86" "$LIVE_ARM_SRC:$LIVE_ARM"; do
  src="${pair%%:*}"; dst="${pair##*:}"
  if [ -n "$src" ] && [ -f "$src" ]; then
    shasum -a 256 "$src" | sed "s#^\\([0-9a-f]*\\)  .*#\\1  $dst#" >> SHA256SUMS
    PUBLISH_FILES="$PUBLISH_FILES $dst"
    echo "  live image: $src -> $dst"
  else
    echo "  no live $dst found — publishing without it" >&2
  fi
done
PAYLOAD="dist/teddyos-update.tar.gz"
if [ -f "$PAYLOAD" ]; then
  shasum -a 256 "$PAYLOAD" | sed "s#^\\([0-9a-f]*\\)  .*#\\1  teddyos-update.tar.gz#" >> SHA256SUMS
  PUBLISH_FILES="$PUBLISH_FILES teddyos-update.tar.gz"
fi
cat SHA256SUMS

cat > BUILD-INFO.txt <<EOF
teddyOS Linux live images
commit:  $COMMIT
built:   $(date -u '+%Y-%m-%dT%H:%M:%SZ')
version: $(cat VERSION 2>/dev/null || echo unknown)

$LIVE_X86   amd64 live (Intel/AMD)
$LIVE_ARM   arm64 live (Apple Silicon and other aarch64)

Verify:  shasum -a 256 -c SHA256SUMS
EOF

step "release manifest"
./scripts/make-manifest.sh

if [ "$DRY_RUN" = "1" ]; then
  step "DRY RUN — would publish to $HOST:$DIR"
  ls -la SHA256SUMS BUILD-INFO.txt manifest.json $PAYLOAD 2>/dev/null || true
  exit 0
fi

step "uploading to $HOST:$DIR"
ssh "$HOST" "mkdir -p '$DIR'"

need_kb=0
for f in "$LIVE_X86_SRC" "$LIVE_ARM_SRC" "$PAYLOAD"; do
  [ -n "$f" ] && [ -f "$f" ] && need_kb=$((need_kb + $(du -k "$f" | cut -f1)))
done
free_kb="$(ssh "$HOST" "df -Pk '$DIR' | tail -1 | awk '{print \$4}'")"
echo "  need $((need_kb / 1024)) MB, free $((free_kb / 1024)) MB"
[ "$free_kb" -gt "$((need_kb + 1048576))" ] ||
  die "not enough room on $HOST: need $((need_kb / 1024)) MB plus 1 GB headroom"

moves=""
[ -n "$LIVE_X86_SRC" ] && [ -f "$LIVE_X86_SRC" ] && {
  rsync -az --info=progress2 "$LIVE_X86_SRC" "$HOST:$DIR/.$LIVE_X86.incoming"
  moves="${moves:+$moves && }mv -f '.$LIVE_X86.incoming' '$LIVE_X86'"
}
[ -n "$LIVE_ARM_SRC" ] && [ -f "$LIVE_ARM_SRC" ] && {
  rsync -az --info=progress2 "$LIVE_ARM_SRC" "$HOST:$DIR/.$LIVE_ARM.incoming"
  moves="${moves:+$moves && }mv -f '.$LIVE_ARM.incoming' '$LIVE_ARM'"
}
if [ -f "$PAYLOAD" ]; then
  rsync -az "$PAYLOAD" "$HOST:$DIR/.teddyos-update.tar.gz.incoming"
  moves="${moves:+$moves && }mv -f '.teddyos-update.tar.gz.incoming' 'teddyos-update.tar.gz'"
fi
rsync -az SHA256SUMS BUILD-INFO.txt manifest.json "$HOST:$DIR/"
ssh "$HOST" "cd '$DIR' && $moves && chmod 644 $PUBLISH_FILES SHA256SUMS BUILD-INFO.txt manifest.json"

step "verifying on the server"
ssh "$HOST" "cd '$DIR' && sha256sum -c SHA256SUMS" ||
  die "checksums do not match on the server"

step "verifying over HTTPS from $URL"
for f in $PUBLISH_FILES SHA256SUMS; do
  code="$(curl -s -o /dev/null -w '%{http_code}' "$URL/$f")"
  [ "$code" = "200" ] || die "$URL/$f returned HTTP $code"
  printf '  %-24s %s\n' "$f" "HTTP $code"
done

tmp="$(mktemp -t os-publish-manifest)"
curl -s -o "$tmp" "$URL/manifest.json"
if ! grep -q '"version"' "$tmp"; then
  head -c 120 "$tmp" >&2; echo >&2
  rm -f "$tmp"
  die "manifest.json did not publish — catch-all may be answering"
fi
rm -f "$tmp"

step "published $COMMIT"
echo "  $URL/$LIVE_X86"
echo "  $URL/$LIVE_ARM"
echo "  page: ${URL%/os}/os.html"
