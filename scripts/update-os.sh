#!/usr/bin/env bash
# Download the published teddy OS image for this machine, verified.
#
# The other half of scripts/publish-os.sh. That one puts images on
# teddysearch.com; this one brings them back, checks them against the published
# checksums, and stages the result. It is the shell face of the bridge's
# `update.check` / `update.download` — same code path a guest uses, so a
# failure here is a failure there.
#
# What it deliberately does NOT do is apply anything. These images are boot
# media: replacing what a machine boots from is something a person does on
# purpose, not a side effect of asking whether an update exists.
#
#   ./scripts/update-os.sh                 this machine's architecture
#   ./scripts/update-os.sh --arch x86_64   the other one
#   ./scripts/update-os.sh --check         report only, download nothing
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ARCH="$(uname -m)"
CHECK_ONLY=0
while [ $# -gt 0 ]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --check) CHECK_ONLY=1; shift ;;
    -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
    *) echo "error: unknown argument $1" >&2; exit 2 ;;
  esac
done

ADDR="${OS_MCP_BRIDGE_ADDR:-127.0.0.1:7420}"
HOST="${ADDR%:*}"
PORT="${ADDR##*:}"

./scripts/ensure-bridge.sh >/dev/null

# `nc -w` is an idle timeout, not a total one, so a long download does not trip
# it — but a stalled one does, which is the behaviour worth having.
call() { printf '%s\n' "$1" | nc -w 300 "$HOST" "$PORT"; }

# A bridge is long-lived and shared: ensure-bridge reuses whatever is already
# listening, which after a rebuild is routinely an OLDER binary — that has
# already cost this repo an afternoon ("a bridge kept serving a build from an
# hour earlier"). An old one answers `not_found` here, and without this guard
# that surfaces as a confusing failure of the *download*.
#
# It is not killed automatically. The port is shared with QEMU/UTM sessions
# that may be mid-boot, and taking their bridge away to check for an update is
# a worse outcome than this message.
if [ "$(call 'CALL update.status' | head -1)" = "ERR update.status not_found" ]; then
  echo "error: the bridge on $ADDR predates this feature — it has no update tools." >&2
  echo "  Restart it so it picks up the current build:" >&2
  echo "    kill \$(lsof -tnP -iTCP:$PORT -sTCP:LISTEN) && ./scripts/ensure-bridge.sh" >&2
  echo "  (or 'make bridge-install' if it is the login agent)" >&2
  exit 1
fi

echo "== published"
check="$(call "CALL update.check running=$(cat VERSION 2>/dev/null || echo unknown)")"
printf '%s\n' "$check" | sed 's/^/  /'

case "$check" in
  ERR*)
    # Distinguish "nothing published" from "the updater is broken" — the token
    # says which, and they send you to different places.
    echo >&2
    echo "Nothing usable is published at the update URL." >&2
    echo "  manifest_is_not_json / sums_is_not_checksums — the /tsearch/ catch-all is" >&2
    echo "  answering, which means no release is there yet. Run 'make publish-os'." >&2
    exit 1
    ;;
esac

[ "$CHECK_ONLY" = 1 ] && exit 0

echo
echo "== downloading for $ARCH"
out="$(call "CALL update.download arch=$ARCH wait=1")"
printf '%s\n' "$out" | sed 's/^/  /'

case "$out" in
  ERR*)
    echo >&2
    case "$out" in
      *checksum_mismatch*)
        echo "The download does not match its published checksum, and has been deleted." >&2
        echo "Do not retry blindly: check ./scripts/check-published.sh first — a CDN" >&2
        echo "serving a stale image looks exactly like a tampered one." >&2 ;;
      *unknown_arch*)
        echo "No image is published for '$ARCH'. Published: x86_64, arm64." >&2 ;;
      *)
        echo "Download failed. The token above says which half broke." >&2 ;;
    esac
    exit 1
    ;;
esac

path="$(printf '%s\n' "$out" | awk -F'path=' '/^ROW path=/{print $2}')"
echo
echo "Staged, checksum verified: $path"
echo
echo "Nothing has been applied — this is boot media, not an installed package."
echo "To use it:"
echo "    ISO=$path ./scripts/make-usb.sh"
echo "    (or point a VM's CD drive at that file)"
