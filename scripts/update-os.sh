#!/usr/bin/env bash
# Download the published teddy OS image for this machine, verified.
#
# The other half of scripts/publish-os.sh. That one puts images on
# teddysearch.com; this one brings them back, checks them against the
# published checksums, and stages the result.
#
# Runs directly over curl now. The host bridge that used to relay this (CALL
# update.check / update.download) was removed when the OS went fully
# standalone — see CLAUDE.md ("no bridge probing"). The guarantees are the
# same, just with no daemon in the middle:
#   * refuses a stale/cached fetch — reads the commit out of BUILD-INFO.txt
#     and downloads with ?v=<commit>, because a CDN serves the previous
#     release for hours after a publish
#   * verifies against SHA256SUMS before doing anything else with the bytes;
#     a mismatch is deleted, never staged
#   * never applies anything — these are boot media, staged only
#
#   ./scripts/update-os.sh                 this machine's architecture
#   ./scripts/update-os.sh --arch x86_64   the other one
#   ./scripts/update-os.sh --check         report only, download nothing
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

case "$(uname -m)" in
  arm64|aarch64) ARCH=arm64 ;;
  x86_64|amd64)  ARCH=x86_64 ;;
  *)             ARCH="$(uname -m)" ;;
esac
CHECK_ONLY=0
while [ $# -gt 0 ]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --check) CHECK_ONLY=1; shift ;;
    -h|--help) sed -n '2,21p' "$0"; exit 0 ;;
    *) echo "error: unknown argument $1" >&2; exit 2 ;;
  esac
done

URL="${OS_UPDATE_URL:-https://teddysearch.com/tsearch/os}"
STAGE="${OS_UPDATE_STAGE:-$HOME/Library/Application Support/os/updates}"

die() { echo "error: $*" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || die "curl not found"
command -v shasum >/dev/null 2>&1 || die "shasum not found"

case "$ARCH" in
  x86_64) IMG=os.iso ;;
  arm64)  IMG=os-arm64.iso ;;
  *) die "unknown architecture '$ARCH' — published: x86_64, arm64" ;;
esac

# /tsearch/ has a catch-all: a missing path answers HTTP 200 with the
# homepage, not 404. Every fetch below is judged by its body, never its
# status code — a fetch that fails outright (curl -f) is treated the same
# as a fetch that succeeds with the wrong body.
fetch() { curl -fsS --max-time 20 "$1" 2>/dev/null; }

echo "== published"
build_info="$(fetch "$URL/BUILD-INFO.txt" || true)"
commit="$(printf '%s\n' "$build_info" | sed -n 's/^commit:[[:space:]]*//p' | head -1)"
if [ -z "$commit" ] || [ "$commit" = unknown ]; then
  echo "Nothing usable is published at $URL." >&2
  echo "  BUILD-INFO.txt is missing or unreadable — the /tsearch/ catch-all is" >&2
  echo "  probably answering, which means no release is there yet. Run 'make publish-os'." >&2
  exit 1
fi

running="$(cat VERSION 2>/dev/null || echo unknown)"
manifest="$(fetch "$URL/manifest.json" || true)"
version=""
if printf '%s' "$manifest" | grep -q '"version"'; then
  version="$(printf '%s\n' "$manifest" | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
fi

if [ -n "$version" ]; then
  echo "  latest:  $version (commit $commit)"
  if [ "$running" = "$version" ]; then
    echo "  state:   current"
  else
    echo "  state:   behind (running=$running, latest=$version)"
  fi
else
  # manifest.json is optional (publish-os.sh writes it, but an older publish
  # may not have). A commit is not comparable to a version, so this reports
  # undetermined rather than guessing "behind".
  echo "  latest:  commit $commit (no manifest published — version undetermined)"
  echo "  state:   undetermined"
fi

[ "$CHECK_ONLY" = 1 ] && exit 0

echo
echo "== downloading for $ARCH ($IMG)"
sums="$(fetch "$URL/SHA256SUMS")" || die "could not fetch SHA256SUMS from $URL"
want="$(printf '%s\n' "$sums" | awk -v f="$IMG" '$2 == f {print $1}')"
[ -n "$want" ] || die "no image is published for '$ARCH' ($IMG not listed in SHA256SUMS)"

mkdir -p "$STAGE"
tmp="$(mktemp "$STAGE/.${IMG}.XXXXXX")"
cleanup() { [ -f "$tmp" ] && rm -f "$tmp"; }
trap cleanup EXIT

# Versioned URL, not the bare one: a CDN fronts teddysearch.com and serves the
# previous release for hours after a publish. Fetched unversioned, a good
# release would arrive as a checksum mismatch, which reads as tampering.
if ! curl -fsS --max-time 1800 -o "$tmp" "$URL/$IMG?v=$commit"; then
  die "download failed"
fi

got="$(shasum -a 256 "$tmp" | awk '{print $1}')"
if [ "$got" != "$want" ]; then
  echo "error: the download does not match its published checksum, and has been deleted." >&2
  echo "  Do not retry blindly: check ./scripts/check-published.sh first — a CDN" >&2
  echo "  serving a stale image looks exactly like a tampered one." >&2
  exit 1
fi

dest="$STAGE/$IMG"
mv -f "$tmp" "$dest"
trap - EXIT
echo
echo "Staged, checksum verified: $dest"
echo
echo "Nothing has been applied — this is boot media, not an installed package."
echo "To use it:"
echo "    ISO=$dest ./scripts/make-usb.sh"
echo "    (or point a VM's CD drive at that file)"
