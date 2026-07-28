#!/usr/bin/env bash
# Prove the product layer runs on a Linux guest — DISABLED, standalone build.
#
# This used to boot Alpine under HVF, fetch linux/agent-shell from the host,
# and run it against a REAL host bridge at 10.0.2.2:7420 to prove the
# capability model was portable off this kernel. Both halves of that proof
# are gone now, not just the binary:
#
#   * host/bridge/ was deleted (chore: convert to fully standalone OS).
#   * the kernel itself no longer probes a bridge on COM2 — every guest query
#     path unconditionally returns BridgeStatus::Offline in a standalone
#     build (see kernel/src/mcp.rs, and CLAUDE.md: "No bridge probing on
#     COM2"). Re-adding a host bridge alone would not revive this: nothing in
#     the shipped kernel would ever call it.
#
# So there's no "run it locally instead" version of this test — the thing it
# proved (a live guest talking to a live host bridge) is the exact capability
# the standalone refactor removed on purpose. If bridge-mode search ever
# comes back, restore this script from git history alongside host/bridge/.
set -euo pipefail

cat >&2 <<'EOF'
substrate-proof.sh is disabled: it proved a Linux guest could reach a live
host bridge, and both the bridge and the kernel's bridge-probing path were
removed by the standalone refactor. There is nothing left for it to prove
against — see kernel/src/mcp.rs (BridgeStatus::Offline, unconditional) and
CLAUDE.md ("No bridge probing on COM2").

Nothing to run standalone: this was specifically a test of network-mediated
functionality that no longer exists in this build.
EOF
exit 1
