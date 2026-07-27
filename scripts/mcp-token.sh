#!/usr/bin/env bash
# Print the shared secret the guest kernel presents to the host MCP bridge,
# generating one on first use and reusing it afterwards.
#
# This is the single source of truth for "the token" — every script that
# needs to hand the same value to both the ISO (`cmdline: mcp_token=...`)
# and the bridge process (`OS_MCP_BRIDGE_TOKEN=...`) calls this and gets the
# same answer, so a guest built at one moment and a bridge started at another
# always agree without a manual copy/paste step. This is the whole of "set
# up MCP during initial setup": there is no separate pairing screen, no code
# to type in — building the ISO and starting the bridge both just ask this
# script for the token they need.
#
# Nothing is printed but the token itself, so callers can do
# TOKEN="$(scripts/mcp-token.sh)" directly.
set -euo pipefail

DIR="${OS_MCP_TOKEN_DIR:-$HOME/.os-mcp}"
FILE="$DIR/token"

mkdir -p "$DIR"
chmod 700 "$DIR" 2>/dev/null || true

if [[ -s "$FILE" ]]; then
  # Reuse: a fresh token on every `make iso`/`make virtualbox-arm64` would
  # mean the previous session's still-running bridge (ensure-bridge.sh keeps
  # bridges alive across rebuilds on purpose) suddenly no longer matches the
  # newly minted guest, failing auth for no reason a user caused.
  cat "$FILE"
  exit 0
fi

if command -v openssl >/dev/null 2>&1; then
  TOKEN="$(openssl rand -hex 16)"
else
  # openssl ships with macOS and every dev box this project targets, but
  # fall back rather than hard-failing a first run on a stripped-down host.
  TOKEN="$(od -An -tx1 -N16 /dev/urandom | tr -d ' \n')"
fi

# Write mode 0600 from the start (umask-independent) rather than chmod after
# the fact, so the secret is never briefly world-readable between create and
# chmod.
umask 177
printf '%s' "$TOKEN" >"$FILE"
chmod 600 "$FILE" 2>/dev/null || true

printf '%s' "$TOKEN"
