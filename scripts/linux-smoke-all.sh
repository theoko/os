#!/usr/bin/env bash
# Run all teddyOS Linux smoke / e2e / GUI flow suites and print a combined card.
#
#   ./scripts/linux-smoke-all.sh
#   TEDDYOS_VM_HOST=192.168.64.2 ./scripts/linux-smoke-all.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="/opt/homebrew/opt/rustup/bin:${HOME}/.cargo/bin:/opt/homebrew/bin:${PATH}"

OUT="${SMOKE_ALL_OUT:-/tmp/teddyos-smoke-all}"
mkdir -p "$OUT"
set +e

echo "############################################################"
echo "# 1) Host unit tests (make test-host)"
echo "############################################################"
make test-host 2>&1 | tee "$OUT/01-test-host.log"
HOST=$?

echo
echo "############################################################"
echo "# 2) Host Linux contract unit (AST + Claude/auth/catalog)"
echo "############################################################"
./scripts/linux-contract-unit.sh 2>&1 | tee "$OUT/02-contract-unit.log"
AST=$?

echo
echo "############################################################"
echo "# 3) Linux desktop contract e2e"
echo "############################################################"
./scripts/linux-desktop-e2e.sh 2>&1 | tee "$OUT/03-desktop-e2e.log"
DESK=$?

echo
echo "############################################################"
echo "# 4) Linux GUI flow e2e"
echo "############################################################"
./scripts/linux-gui-flow-e2e.sh 2>&1 | tee "$OUT/04-gui-flow.log"
GUI=$?

echo
echo "############################################################"
echo "# 5) QEMU ISO smoke (optional)"
echo "############################################################"
if [[ -f os.iso ]]; then
  make smoke 2>&1 | tee "$OUT/05-qemu-smoke.log"
  QEMU=$?
else
  echo "SKIP: no os.iso (run make iso first)" | tee "$OUT/05-qemu-smoke.log"
  QEMU=0
  QEMU_SKIP=1
fi

set -e
echo
echo "############################################################"
echo "# COMBINED SCORECARD"
echo "############################################################"
printf '  %-28s %s\n' "make test-host" "$([[ $HOST -eq 0 ]] && echo PASS || echo FAIL)"
printf '  %-28s %s\n' "linux-contract-unit" "$([[ $AST -eq 0 ]] && echo PASS || echo FAIL)"
printf '  %-28s %s\n' "linux-desktop-e2e" "$([[ $DESK -eq 0 ]] && echo PASS || echo FAIL)"
printf '  %-28s %s\n' "linux-gui-flow-e2e" "$([[ $GUI -eq 0 ]] && echo PASS || echo FAIL)"
if [[ "${QEMU_SKIP:-0}" -eq 1 ]]; then
  printf '  %-28s %s\n' "make smoke (QEMU)" "SKIP (no os.iso)"
else
  printf '  %-28s %s\n' "make smoke (QEMU)" "$([[ $QEMU -eq 0 ]] && echo PASS || echo FAIL)"
fi
echo
echo "logs: $OUT"
if [[ $HOST -ne 0 || $AST -ne 0 || $DESK -ne 0 || $GUI -ne 0 || $QEMU -ne 0 ]]; then
  echo "RESULT: FAIL"
  exit 1
fi
echo "RESULT: PASS"
exit 0
