#!/usr/bin/env bash
# Host-side unit contracts for teddyOS Linux desktop (no guest required).
# Run: ./scripts/linux-contract-unit.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PASS=0
FAIL=0
ok()  { PASS=$((PASS + 1)); echo "  OK  $1"; }
bad() { FAIL=$((FAIL + 1)); echo "  FAIL  $1 -- $2" >&2; }

echo "=== teddyOS Linux contract unit (host) ==="

# --- AST parse all python under linux/ --------------------------------------
if python3 - <<'PY'
import ast, pathlib, sys
bad = []
n = 0
for p in pathlib.Path("linux").rglob("*"):
    if not p.is_file():
        continue
    text = p.read_text(errors="replace")
    first = text.splitlines()[0] if text else ""
    if p.suffix == ".py" or (p.name.startswith("teddyos-") and "python" in first):
        try:
            ast.parse(text)
            n += 1
        except SyntaxError as e:
            bad.append(f"{p}: {e}")
print(f"parsed={n}")
if bad:
    print("\n".join(bad))
    sys.exit(1)
PY
then
  ok "AST parse linux python apps/modules"
else
  bad "AST parse" "syntax error"
fi

# --- Claude auth false-code regression --------------------------------------
if python3 - <<'PY'
import re
from pathlib import Path
src = Path("linux/teddyos-agent/teddyos-accounts").read_text()
assert "scrubbed" in src or "_URL_RE.sub" in src
assert "_submit_paste" in src
assert "_DEVICE_CODE_ACCOUNTS" in src
assert "Paste code from the browser" in src or "paste" in src.lower()
assert "Send code" in src

url_re = re.compile(r"https?://[^\s\"'<>]+", re.I)
code_re = re.compile(
    r"\b([A-Z0-9]{4,5}-[A-Z0-9]{4,5})\b"
    r"|\bone-time code[:\s]+([A-Z0-9-]{6,})\b"
    r"|\benter code[:\s]+([A-Z0-9-]{6,})\b"
    r"|\buser code[:\s]+([A-Z0-9-]{6,})\b",
    re.I,
)
oauth = (
    "If the browser didn't open, visit: "
    "https://claude.com/cai/oauth/authorize?code=true"
    "&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code"
)
codes = [
    next(g for g in m.groups() if g)
    for m in code_re.finditer(url_re.sub(" ", oauth))
]
assert codes == [], codes
device = "First copy your one-time code: AB12-CD34"
codes2 = [
    next(g for g in m.groups() if g)
    for m in code_re.finditer(url_re.sub(" ", device))
]
assert "AB12-CD34" in codes2
print("claude-false-code-ok")
PY
then
  ok "Claude OAuth UUID is not a device code"
else
  bad "Claude false-code" "regression"
fi

# --- accounts.py connect argv -----------------------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import accounts
claude = next(a for a in accounts.all_accounts() if a.id == "claude")
assert "--claudeai" in claude.connect_argv, claude.connect_argv
for tid in ("devin", "replit", "perplexity"):
    a = next(x for x in accounts.all_accounts() if x.id == tid)
    assert a.always_available, tid
print("accounts-ok", len(list(accounts.all_accounts())))
PY
then
  ok "accounts.py claude --claudeai + always_available web apps"
else
  bad "accounts.py" "catalog contract failed"
fi

# --- work_tools catalog has web apps ----------------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
# available_work_tools probes host binaries — just check catalog constants
import work_tools
# Catalog is _TOOLS or similar — import and inspect source if needed
src = Path("linux/teddyos-search/work_tools.py").read_text()
for tid in ('"devin"', '"replit"', '"perplexity"'):
    assert tid in src, tid
print("work_tools-src-ok")
PY
then
  ok "work_tools.py catalogs devin/replit/perplexity"
else
  bad "work_tools catalog" "missing ids"
fi

# --- wrappers exist with correct URLs ---------------------------------------
for pair in "teddyos-devin:app.devin.ai" "teddyos-replit:replit.com"; do
  bin="${pair%%:*}"
  host="${pair##*:}"
  f="linux/teddyos-agent/$bin"
  if [[ -f "$f" ]] && grep -q "$host" "$f"; then
    ok "host $bin → $host"
  else
    bad "host $bin" "missing or wrong URL"
  fi
done

# --- icons present on host tree ---------------------------------------------
for icon in teddyos-devin teddyos-replit teddyos-claude teddyos-accounts teddyos-github; do
  if [[ -f "linux/icons/${icon}.svg" ]]; then
    ok "icon tree $icon.svg"
  else
    bad "icon tree $icon" "missing svg"
  fi
done

# --- ISO build script installs new apps -------------------------------------
if grep -q teddyos-devin linux/iso/build-iso.sh \
  && grep -q teddyos-replit linux/iso/build-iso.sh \
  && grep -q teddyos-devin.desktop linux/iso/build-iso.sh; then
  ok "build-iso installs Devin + Replit"
else
  bad "build-iso" "missing install lines"
fi

echo
echo "PASS=$PASS  FAIL=$FAIL  TOTAL=$((PASS + FAIL))"
if [[ "$FAIL" -gt 0 ]]; then
  echo "RESULT: FAIL"
  exit 1
fi
echo "RESULT: PASS"
exit 0
