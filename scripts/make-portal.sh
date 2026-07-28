#!/usr/bin/env bash
# One machine per tSearch portal, with each user's data kept apart.
#
# STANDALONE NOTE: this used to start a per-portal `os-mcp-bridge` process so
# several users' guests could each reach their own corpus/credentials over
# COM2. That half is gone — host/bridge/ was deleted and the kernel's guest
# query path unconditionally returns Offline in a standalone build (see
# kernel/src/mcp.rs, CLAUDE.md: "No bridge probing on COM2"). Starting a
# per-portal bridge process would have nothing on the other end to talk to
# it, so `./scripts/make-portal.sh <name>` now refuses with an explanation
# instead of pretending to work.
#
# `--list` still works: it only reads portals.json and reports what has
# already been cached on disk from a previous (bridge-mode) sync. No network,
# no bridge, so there's nothing standalone-incompatible about it.
#
#   ./scripts/make-portal.sh --list        # what is configured (still works)
#   ./scripts/make-portal.sh theo          # start a bridge for that portal (disabled)
#   ./scripts/make-portal.sh theo --sync   # ...and fetch its corpus (disabled)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

REGISTRY="$ROOT/portals.json"
PROFILES="${OS_PORTAL_PROFILES:-$HOME/Library/Application Support/os-portals}"

[[ -f "$REGISTRY" ]] || { echo "error: $REGISTRY missing" >&2; exit 1; }

list_portals() {
  python3 - "$REGISTRY" "$PROFILES" <<'PY'
import json, os, sys
reg, profiles = sys.argv[1], sys.argv[2]
print(f"{'portal':<12} {'auth':<7} {'corpus cached':<14} corpus")
for p in json.load(open(reg))["portals"]:
    home = os.path.join(profiles, p["name"], "home")
    cache = os.path.join(home, "Library/Application Support/os/knowledge/teddy.json")
    if os.path.exists(cache):
        mb = os.path.getsize(cache) / 1024 / 1024
        cached = f"{mb:.0f} MB"
    else:
        cached = "-"
    print(f"{p['name']:<12} {p['auth']:<7} {cached:<14} {p['corpus']}")
PY
}

if [[ "${1:-}" == "--list" || -z "${1:-}" ]]; then
  list_portals
  echo
  echo "starting a portal bridge is disabled in this standalone build — see"
  echo "the note at the top of this script for why."
  exit 0
fi

NAME="$1"; shift

cat >&2 <<EOF
error: cannot start a bridge for portal '$NAME' — this build is standalone.

host/bridge/ (the os-mcp-bridge package this launched) was removed, and the
kernel's guest query path returns Offline unconditionally in a standalone
build (kernel/src/mcp.rs; see also CLAUDE.md: "No bridge probing on COM2").
Building and launching a per-portal bridge process would have no consumer —
no guest in this build ever asks a bridge for anything.

'./scripts/make-portal.sh --list' still works; it only reads cached state
from disk and touches no network.

If bridge-mode search comes back, restore this script's bridge-launch path
from git history alongside host/bridge/.
EOF
exit 1
