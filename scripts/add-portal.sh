#!/usr/bin/env bash
# Register a tSearch portal, one command per user.
#
# Why this is a command rather than "edit portals.json": the list of users is
# not discoverable from this machine. teddysearch.com serves a catch-all under
# /tsearch/ — every missing path returns HTTP 200 with the homepage — so
# probing cannot tell a real portal from a typo, and enumerating would mean
# guessing usernames against a live server. Only the operator knows who they
# are, so the operator names them and this does the rest.
#
#   ./scripts/add-portal.sh alice
#   ./scripts/add-portal.sh public-demo --auth none
#   ./scripts/add-portal.sh alice bob carol        # several at once
#
# Then: ./scripts/make-portal.sh alice --sync
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
REGISTRY="$ROOT/portals.json"

AUTH="basic"
NAMES=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --auth) AUTH="${2:-basic}"; shift 2 ;;
    -*) echo "error: unknown flag $1" >&2; exit 1 ;;
    *) NAMES+=("$1"); shift ;;
  esac
done

if [[ ${#NAMES[@]} -eq 0 ]]; then
  echo "usage: add-portal.sh <portal> [<portal>...] [--auth none|basic]" >&2
  exit 1
fi

for NAME in "${NAMES[@]}"; do
  # A portal name becomes both a URL path and a directory name. Keep it boring
  # rather than discovering later what a space does to either.
  if [[ ! "$NAME" =~ ^[a-z0-9][a-z0-9._-]*$ ]]; then
    echo "error: '$NAME' — portal names must be [a-z0-9._-] and start alphanumeric" >&2
    exit 1
  fi

  REGISTRY="$REGISTRY" NAME="$NAME" AUTH="$AUTH" python3 <<'PY'
import json, os, sys
reg, name, auth = os.environ["REGISTRY"], os.environ["NAME"], os.environ["AUTH"]
doc = json.load(open(reg))
if any(p["name"] == name for p in doc["portals"]):
    print(f"  {name}: already registered, leaving it alone")
    sys.exit(0)
doc["portals"].append({
    "name": name,
    "corpus": f"https://teddysearch.com/{name}/corpus.json",
    "auth": auth,
    "note": "corpus URL assumed from the portal name",
})
with open(reg, "w") as f:
    json.dump(doc, f, indent=2)
    f.write("\n")
print(f"  {name}: added ({auth})")
PY

  # Confirm the portal is really there. A 401 is proof — auth-protected paths
  # bypass the catch-all. A 200 proves nothing at all, and saying so matters
  # more than looking confident.
  code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 12 \
    "https://teddysearch.com/$NAME/corpus.json" 2>/dev/null || echo 000)
  case "$code" in
    401|403) echo "           verified: HTTP $code — protected, so the portal exists" ;;
    200)     echo "           HTTP 200, but the /tsearch/ catch-all returns that for missing paths too — unverified" ;;
    000)     echo "           could not reach the server — unverified" ;;
    *)       echo "           HTTP $code — check the name" ;;
  esac
done

echo
"$ROOT/scripts/make-portal.sh" --list
