#!/usr/bin/env bash
# Build the software update a running teddyOS can install without reinstalling.
#
# Until this existed, "update" meant "download a 2.6 GB ISO and install the
# machine again". That is fine for the boot media and absurd for the thing that
# actually changes: the teddyOS software itself is a few hundred kilobytes of
# Python, a shell extension, some icons and a dconf file. A search fix should
# not cost somebody their machine's state.
#
# The payload is built from the repository at a commit, so what a machine
# installs is exactly what was pushed. publish-os.sh uploads it beside the
# images and records its checksum in the manifest; teddyos-update on the guest
# reads that manifest, verifies, and applies.
#
#   ./scripts/make-update-payload.sh          # -> dist/teddyos-update.tar.gz
#   ./scripts/make-update-payload.sh --out X  # somewhere else
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

OUT="${1:-}"; [ "${1:-}" = "--out" ] && OUT="${2:-}"
OUT="${OUT:-dist/teddyos-update.tar.gz}"
STAGE="$(mktemp -d "${TMPDIR:-/tmp}/teddyos-payload.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT

VERSION="$(cat VERSION 2>/dev/null || echo 0.0.0)"
COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
# A payload built from a dirty tree cannot be reproduced from its commit, and
# the whole point of naming the commit is that somebody can rebuild what a
# stranger is running. Say so in the field rather than lie about it.
if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
  COMMIT="$COMMIT-dirty"
fi

echo ">>> teddyOS update payload  $VERSION ($COMMIT)"

# mkdir + cp rather than `install -D`: this script runs on the Mac, where
# install(1) is BSD and has no -D. Silently creating a file named after the
# flag is exactly the kind of failure that ships an empty payload.
put() {  # put <src> <dest-under-stage> [mode]
  local src="$1" dest="$STAGE/$2" mode="${3:-644}"
  [ -f "$src" ] || { echo "error: $src is missing — payload would ship a hole" >&2; exit 1; }
  mkdir -p "$(dirname "$dest")"
  cp "$src" "$dest"
  chmod "$mode" "$dest"
}

# The programs. Their import path is rewritten exactly as build-iso.sh does at
# install time, so an updated binary keeps finding /usr/lib/teddyos rather than
# the directory it happened to be built beside.
for b in teddyos-search teddyos-search-app; do
  put "linux/teddyos-search/$b" "usr/bin/$b" 755
done
put linux/teddyos-setup/teddyos-setup    usr/bin/teddyos-setup 755
put linux/teddyos-setup/teddyos-welcome  usr/bin/teddyos-welcome 755
put linux/teddyos-claude/teddyos-claude  usr/bin/teddyos-claude 755
put linux/teddyos-agent/teddyos-agent    usr/bin/teddyos-agent 755

sed -i.bak 's|sys.path.insert(0, str(Path(__file__).resolve().parent))|sys.path.insert(0, "/usr/lib/teddyos")|' \
  "$STAGE/usr/bin/teddyos-search"
sed -i.bak '\|resolve().parent))|d' "$STAGE/usr/bin/teddyos-search-app"
sed -i.bak '\|teddyos-search"))|d'  "$STAGE/usr/bin/teddyos-setup"
sed -i.bak '\|resolve().parent.parent|d' "$STAGE/usr/bin/teddyos-welcome"
rm -f "$STAGE"/usr/bin/*.bak

# The libraries they import (include newer modules used by Search / intent).
for m in caps search sandbox work_tools git_projects accounts audience intent \
         pending_ask progress phone_auth; do
  [ -f "linux/teddyos-search/$m.py" ] || continue
  put "linux/teddyos-search/$m.py" "usr/lib/teddyos/$m.py"
done
put linux/logging/logutil.py usr/lib/teddyos/logutil.py
put linux/logging/teddyos-log-collect usr/bin/teddyos-log-collect 755

# Offline built-in help seed (product install + ISO ship this; updates must too
# or a guest that only ever took payloads never gets /usr/share/teddyos/corpus.json).
if [ -f search/seed.json ]; then
  put search/seed.json usr/share/teddyos/corpus.json
elif [ -f search/corpus.json ]; then
  put search/corpus.json usr/share/teddyos/corpus.json
fi

# Agents beyond the original four.
for a in teddyos-ask-all teddyos-perplexity teddyos-devin teddyos-replit \
         teddyos-accounts teddyos-open-signin teddyos-linkedin teddyos-whatsapp \
         teddyos-gmail teddyos-display; do
  [ -f "linux/teddyos-agent/$a" ] || continue
  put "linux/teddyos-agent/$a" "usr/bin/$a" 755
done
put linux/teddyos-update/teddyos-update usr/bin/teddyos-update 755

# The shell extension and the icons.
put linux/shell-extension/teddyos@teddysearch.com/extension.js \
    "usr/share/gnome-shell/extensions/teddyos@teddysearch.com/extension.js"
put linux/shell-extension/teddyos@teddysearch.com/metadata.json \
    "usr/share/gnome-shell/extensions/teddyos@teddysearch.com/metadata.json"
for svg in linux/icons/*.svg; do
  [ -e "$svg" ] || continue
  case "$(basename "$svg")" in
    teddyos-logo.svg) continue ;;   # installer branding, handled below
  esac
  put "$svg" "usr/share/icons/hicolor/scalable/apps/$(basename "$svg")"
done

# Desktop defaults, rendered from the same generator the image uses so the two
# cannot disagree about what the dock contains.
mkdir -p "$STAGE/etc/dconf/db/local.d"
./linux/iso/render-dconf.sh > "$STAGE/etc/dconf/db/local.d/00-teddyos"

put linux/iso/chromium-policy.json  etc/chromium/policies/managed/teddyos.json
put linux/iso/calamares-branding.desc etc/calamares/branding/teddyos/branding.desc
put linux/iso/calamares-show.qml      etc/calamares/branding/teddyos/show.qml

# What the payload is, so the guest can decide whether it wants it and a human
# can tell what a machine is running without unpacking anything.
#
# `built` is the COMMIT date, not the moment this ran. A wall clock here makes
# every rebuild of the same commit a different payload, so republishing churns
# bytes and nothing downstream can use "same checksum" to mean "same software".
cat > "$STAGE/PAYLOAD" <<EOF
version=$VERSION
commit=$COMMIT
built=$(git log -1 --format=%cI 2>/dev/null || date -u +%Y-%m-%dT%H:%M:%SZ)
EOF

mkdir -p "$(dirname "$OUT")"
# Two payloads built from the same commit must be byte-identical, or "has this
# changed?" cannot be answered by comparing checksums — which is the entire
# mechanism the guest uses to decide whether to download.
#
# That needs three things: sorted entries, fixed ownership, and a fixed mtime.
# The files are copied fresh on every run, so their timestamps would otherwise
# differ every time and every build would look like a new release.
#
# No --mode: this is bsdtar on macOS and it rejects the flag. `2>/dev/null`
# used to hide that, and the result was an empty archive that passed a
# determinism check by being identically empty both times.
find "$STAGE" -exec touch -t 200001010000 {} +
# COPYFILE_DISABLE stops macOS tar writing an AppleDouble "._name" beside every
# file to carry extended attributes. Those are meaningless on the Debian guest
# and would be installed into /usr/bin as junk — the first test run put
# ._branding.desc and ._teddyos.json on the target.
( cd "$STAGE" && find . -type f ! -name '._*' | LC_ALL=C sort \
    | COPYFILE_DISABLE=1 tar -cf - --no-recursion -T - --uid 0 --gid 0 --numeric-owner ) \
  | gzip -n > "$OUT"

# Never ship a hole. An empty or near-empty payload installs successfully and
# leaves the machine unchanged, which is the worst outcome: the guest records a
# new version and stops offering the update that never arrived.
COUNT="$(tar -tzf "$OUT" | grep -c . || true)"
[ "$COUNT" -ge 15 ] || { echo "error: payload has only $COUNT files — refusing to publish a hole" >&2; exit 1; }

SUM="$(shasum -a 256 "$OUT" | cut -d' ' -f1)"
echo "    $(basename "$OUT")  $(( $(wc -c < "$OUT") / 1024 )) KB"
echo "    sha256 $SUM"
echo "    files  $(tar -tzf "$OUT" | grep -c . )"
