#!/usr/bin/env bash
# Refresh what search knows — DISABLED, standalone build.
#
# This used to re-index workspace files and re-sync the portal corpus by
# calling a host bridge (`CALL workspace.index`, `CALL tsearch.sync`) and
# writing the result to ~/Library/Application Support/os/knowledge/teddy.json
# for a guest to query later. Both halves are gone:
#
#   * host/bridge/ was deleted (chore: convert to fully standalone OS), so
#     there is no process to send those CALLs to.
#   * even if it were re-synced, the kernel's guest query path returns
#     BridgeStatus::Offline unconditionally in a standalone build (see
#     kernel/src/mcp.rs, and CLAUDE.md: "No bridge probing on COM2") — nothing
#     in the shipped kernel ever reads that cache file anymore.
#
# What search a standalone guest gets is a small, hand-curated snapshot baked
# into the kernel at compile time from search/corpus.json (see
# kernel/src/search.rs / kernel/build.rs). Refreshing *that* means editing
# search/corpus.json and rebuilding — there is no live index to keep warm on
# a schedule, so there's nothing for a daily timer to do here anymore.
#
# Kept as a no-op (exit 0, not an error) rather than removed outright, since
# `make refresh` and the com.os.refresh LaunchAgent both still call it — a
# LaunchAgent firing daily should not show up as a failure in .refresh.log.
set -uo pipefail

log() { printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"; }

log "refresh-index.sh is disabled in this standalone build — no host bridge"
log "  to sync, and no consumer for the result even if there were one."
log "  Search now comes from a compile-time corpus (search/corpus.json)."
log "  If bridge-mode search comes back, restore this script's sync logic"
log "  from git history alongside host/bridge/, or run 'make refresh-uninstall'"
log "  to stop this LaunchAgent from firing on a schedule that no longer does"
log "  anything."
log "done"
