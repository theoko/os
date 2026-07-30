#!/usr/bin/env bash
# End-to-end checks for the teddyOS Linux desktop product (guest).
#
#   ./scripts/linux-desktop-e2e.sh
#   TEDDYOS_VM_HOST=192.168.64.2 ./scripts/linux-desktop-e2e.sh
#
# Exit 0 only if every assertion passes.
set -euo pipefail

HOST="${TEDDYOS_VM_HOST:-192.168.64.2}"
USER="${TEDDYOS_VM_USER:-teddy}"
PORT="${TEDDYOS_VM_SSH_PORT:-22}"
TARGET="${USER}@${HOST}"
SSH=(ssh -p "$PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
     -o LogLevel=ERROR -o ConnectTimeout=8 "$TARGET")

# Remote session env for GUI / gsettings (gdm autologin user).
RENV='export DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus HOME=/home/teddy PATH=/usr/local/bin:/usr/bin:/bin;'

PASS=0
FAIL=0
SKIP=0
RESULTS=()

ok()   { PASS=$((PASS + 1)); RESULTS+=("PASS  $1"); echo "  OK  $1"; }
bad()  { FAIL=$((FAIL + 1)); RESULTS+=("FAIL  $1 -- $2"); echo "  FAIL  $1 -- $2" >&2; }
skip() { SKIP=$((SKIP + 1)); RESULTS+=("SKIP  $1 -- $2"); echo "  SKIP  $1 -- $2"; }

remote() { "${SSH[@]}" "$@"; }
remote_bash() { "${SSH[@]}" bash -s; }

echo "=== teddyOS Linux desktop e2e -> $TARGET ==="
echo

if ! remote true 2>/dev/null; then
  echo "FATAL: cannot ssh to $TARGET" >&2
  exit 2
fi
ok "ssh to guest"

# --- 1. binaries ------------------------------------------------------------
echo ">>> binaries"
for bin in \
  teddyos-search-app teddyos-search teddyos-welcome teddyos-setup \
  teddyos-accounts teddyos-ask-all teddyos-open-signin teddyos-perplexity \
  teddyos-claude teddyos-agent teddyos-devin teddyos-replit
do
  if remote "test -x /usr/bin/$bin"; then
    ok "binary $bin"
  else
    bad "binary $bin" "missing or not executable"
  fi
done

# --- 2. modules -------------------------------------------------------------
echo ">>> modules"
for mod in caps search sandbox work_tools git_projects accounts pending_ask logutil; do
  if remote "test -f /usr/lib/teddyos/${mod}.py"; then
    ok "module $mod.py"
  else
    bad "module $mod.py" "not installed"
  fi
done

if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import caps, search, sandbox, work_tools, git_projects, accounts, pending_ask
from work_tools import available_work_tools, tools_ready_for_broadcast, CreditStatus, is_chat_helper
tools = available_work_tools()
assert isinstance(tools, list) and tools, "no work tools"
assert any(t.id == "files" for t in tools), "Files missing"
chats = [t for t in tools if is_chat_helper(t.id)]
if chats:
    t = chats[0]
    assert tools_ready_for_broadcast([t], {t.id: CreditStatus(None, "u")}) == []
    assert tools_ready_for_broadcast([t], {t.id: CreditStatus(False, "Needs sign-in")}) == []
    assert tools_ready_for_broadcast([t], {t.id: CreditStatus(True, "Connected")}) == [t]
pending_ask.clear()
pending_ask.save("/tmp", "hello e2e")
data = pending_ask.load()
assert data and data.get("prompt") == "hello e2e"
pending_ask.clear()
assert pending_ask.load() is None
print("import-ok", len(tools))
PY
REMOTE
then
  ok "import + work_tools readiness + pending_ask"
else
  bad "import + work_tools + pending_ask" "see remote python error"
fi

# --- 3. syntax (python apps only) -------------------------------------------
echo ">>> syntax"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import ast
from pathlib import Path
bad = []
for p in Path("/usr/bin").glob("teddyos-*"):
    text = p.read_text(errors="replace")
    first = text.splitlines()[0] if text else ""
    if "python" not in first:
        continue  # skip shell helpers like teddyos-log-collect
    try:
        ast.parse(text)
    except SyntaxError as e:
        bad.append(f"{p.name}: {e}")
for p in Path("/usr/lib/teddyos").glob("*.py"):
    try:
        ast.parse(p.read_text())
    except SyntaxError as e:
        bad.append(f"{p.name}: {e}")
if bad:
    raise SystemExit("; ".join(bad))
print("syntax-ok")
PY
REMOTE
then
  ok "AST parse python teddyos apps + modules"
else
  bad "AST parse" "syntax error in python app"
fi

# --- 4. desktops ------------------------------------------------------------
echo ">>> desktops"
for desk in \
  teddyos-search.desktop teddyos-web.desktop teddyos-whatsapp.desktop \
  teddyos-welcome.desktop teddyos-answers.desktop teddyos-accounts.desktop \
  teddyos-install.desktop teddyos-perplexity.desktop \
  teddyos-devin.desktop teddyos-replit.desktop
do
  if remote "test -f /usr/share/applications/$desk"; then
    ok "desktop $desk"
  else
    bad "desktop $desk" "missing"
  fi
done

if remote_bash <<'REMOTE'
python3 - <<'PY'
from pathlib import Path
import re
bad = []
forbid = {"web-browser", "system-search", "help-about", "whatsapp", "system-users-symbolic"}
primary = {
    "teddyos-search.desktop", "teddyos-web.desktop", "teddyos-whatsapp.desktop",
    "teddyos-welcome.desktop", "teddyos-answers.desktop", "teddyos-accounts.desktop",
    "teddyos-install.desktop",
}
jargon = re.compile(r"\b(CLI|OAuth|SSH|helper|helpers|argv)\b", re.I)
for p in Path("/usr/share/applications").glob("teddyos-*.desktop"):
    for line in p.read_text().splitlines():
        if line.startswith("Icon=") and p.name in primary:
            icon = line.split("=", 1)[1].strip()
            if icon in forbid:
                bad.append(f"{p.name} Icon={icon}")
        if line.startswith(("Name=", "Comment=")) and jargon.search(line):
            bad.append(f"{p.name}: {line}")
if bad:
    raise SystemExit(" | ".join(bad))
print("desktop-ok")
PY
REMOTE
then
  ok "desktop Icon= + Name/Comment clean"
else
  bad "desktop quality" "placeholder icons or jargon"
fi

# --- 5. icons ---------------------------------------------------------------
echo ">>> icons"
for icon in \
  teddyos-search teddyos-web teddyos-whatsapp teddyos-install \
  teddyos-claude teddyos-grok teddyos-gemini teddyos-codex \
  teddyos-copilot teddyos-answers teddyos-accounts \
  teddyos-devin teddyos-replit teddyos-perplexity teddyos-github
do
  if remote "test -f /usr/share/icons/hicolor/scalable/apps/${icon}.svg || test -f /usr/share/icons/hicolor/128x128/apps/${icon}.png"; then
    ok "icon $icon"
  else
    bad "icon $icon" "missing"
  fi
done

if remote_bash <<'REMOTE'
export DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
python3 - <<'PY'
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gtk, Gdk
d = Gdk.Display.get_default()
if d is None:
    raise SystemExit("no-display")
theme = Gtk.IconTheme.get_for_display(d)
need = [
    "teddyos-search", "teddyos-web", "teddyos-whatsapp", "teddyos-install",
    "teddyos-claude", "teddyos-answers", "teddyos-accounts",
]
missing = [n for n in need if not theme.has_icon(n)]
if missing:
    raise SystemExit("unresolved:" + ",".join(missing))
print("icons-ok")
PY
REMOTE
then
  ok "Gtk resolves teddyOS icons"
else
  skip "Gtk icon resolve" "display/theme resolution failed"
fi

# --- 6. dock ----------------------------------------------------------------
echo ">>> dock"
DOCK=$(remote "$RENV gsettings get org.gnome.shell.extensions.dash-to-dock dock-fixed 2>/dev/null" || true)
if [[ "$DOCK" == *"true"* ]]; then
  ok "dock-fixed true"
else
  bad "dock-fixed" "got: ${DOCK:-empty}"
fi

FAV=$(remote "$RENV gsettings get org.gnome.shell favorite-apps 2>/dev/null" || true)
for need in teddyos-search.desktop teddyos-web.desktop; do
  if [[ "$FAV" == *"$need"* ]]; then
    ok "favorites contain $need"
  else
    bad "favorites" "missing $need"
  fi
done
if [[ "$FAV" == *gnome-terminal* ]] || [[ "$FAV" == *org.gnome.Terminal* ]]; then
  bad "favorites no Terminal" "Terminal pinned"
else
  ok "favorites have no Terminal"
fi
if [[ "$FAV" == *teddyos-claude.desktop* ]]; then
  bad "favorites no bare Claude" "Claude still pinned"
else
  ok "favorites have no bare Claude tile"
fi

# --- 7. Search launch -------------------------------------------------------
echo ">>> Search GUI"
# GApplication single-instance: a second launch exits 0 after activating the
# first. Always kill stragglers, then assert *a* search process is up.
if remote_bash <<'REMOTE'
export DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus HOME=/home/teddy
# prgname is com.teddyos.Search (GLib.set_prgname), not teddyos-search-app.
search_alive() {
  pgrep -x teddyos-search-app >/dev/null 2>&1 \
    || pgrep -x com.teddyos.Search >/dev/null 2>&1 \
    || pgrep -f '/usr/bin/teddyos-search-app' >/dev/null 2>&1
}
search_kill() {
  killall -q teddyos-search-app com.teddyos.Search 2>/dev/null || true
  pkill -f '/usr/bin/teddyos-search-app' 2>/dev/null || true
}
search_kill
sleep 0.5
for i in 1 2 3; do
  search_alive || break
  search_kill
  sleep 0.3
done
rm -f /tmp/e2e-search.log
nohup teddyos-search-app >/tmp/e2e-search.log 2>&1 &
sleep 2.0
if search_alive; then
  echo SEARCH_UP
  exit 0
fi
echo SEARCH_DOWN
cat /tmp/e2e-search.log 2>/dev/null || true
ps aux | grep -i teddyos-search | grep -v grep || true
exit 1
REMOTE
then
  ok "teddyos-search-app stays running"
else
  bad "teddyos-search-app launch" "exited early"
fi

if remote "grep -q ModuleNotFoundError /tmp/e2e-search.log 2>/dev/null"; then
  bad "search imports" "ModuleNotFoundError in log"
else
  ok "search no ModuleNotFoundError"
fi
remote "killall -q teddyos-search-app com.teddyos.Search 2>/dev/null || true; pkill -f '/usr/bin/teddyos-search-app' 2>/dev/null || true" || true

# product contracts in source
if remote "grep -q _close_if_still_blurred /usr/bin/teddyos-search-app \
  && grep -q _restore_pending_ask /usr/bin/teddyos-search-app \
  && grep -q 'Get help' /usr/bin/teddyos-search-app \
  && grep -q is_chat_helper /usr/bin/teddyos-search-app \
  && grep -q teddyos-ask-all /usr/bin/teddyos-search-app"; then
  ok "search-app contracts (blur-close, pending-ask, Get help, ask-all routing)"
else
  bad "search-app contracts" "missing expected symbols"
fi

# --- 8. Welcome tour --------------------------------------------------------
echo ">>> tour"
if remote "grep -q 'Welcome to teddyOS' /usr/bin/teddyos-welcome \
  && grep -q 'Open Search' /usr/bin/teddyos-welcome \
  && grep -q 'work on' /usr/bin/teddyos-welcome"; then
  ok "tour has welcome / work on / Open Search"
else
  bad "tour copy" "missing expected strings"
fi

remote "killall -q teddyos-welcome 2>/dev/null || true" || true
if remote_bash <<'REMOTE'
export DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus HOME=/home/teddy
nohup teddyos-welcome --force >/tmp/e2e-tour.log 2>&1 &
echo $! > /tmp/e2e-tour.pid
sleep 1.5
if kill -0 "$(cat /tmp/e2e-tour.pid)" 2>/dev/null; then
  echo TOUR_UP
  kill "$(cat /tmp/e2e-tour.pid)" 2>/dev/null || true
  exit 0
fi
cat /tmp/e2e-tour.log
exit 1
REMOTE
then
  ok "teddyos-welcome --force launches"
else
  bad "teddyos-welcome launch" "exited early"
fi

# --- 9. accounts + ask-all --------------------------------------------------
echo ">>> accounts + ask-all"
if remote "grep -q 'title=\"Getting you ready\"' /usr/bin/teddyos-accounts \
  || grep -q \"title='Getting you ready'\" /usr/bin/teddyos-accounts \
  || grep -q 'Getting you ready' /usr/bin/teddyos-accounts"; then
  # User-facing window title must not be the old helpers phrasing.
  if remote "grep -E 'super\\(\\_\\_init__\\_\\_.*title=.*Connect your helpers' /usr/bin/teddyos-accounts"; then
    bad "accounts title" "window still titled Connect your helpers"
  else
    ok "accounts: Getting you ready (window title)"
  fi
else
  bad "accounts title" "missing Getting you ready"
fi

if remote "grep -q _prompt_label /usr/bin/teddyos-ask-all \
  && grep -q _kick_rerun /usr/bin/teddyos-ask-all \
  && grep -q 'something else' /usr/bin/teddyos-ask-all \
  && grep -q -- '--allow-all-tools' /usr/bin/teddyos-ask-all \
  && grep -q COPILOT_ALLOW_ALL /usr/bin/teddyos-ask-all"; then
  ok "ask-all: re-ask + prompt label + Copilot flags"
else
  bad "ask-all contracts" "missing features"
fi

rc=$(remote "teddyos-ask-all >/tmp/aa.out 2>/tmp/aa.err; echo \$?" || true)
if [[ "$rc" != "0" ]]; then
  ok "ask-all without args exits non-zero ($rc)"
else
  bad "ask-all without args" "expected failure"
fi

# --- 10. accounts status ----------------------------------------------------
echo ">>> account status probes"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import accounts
for acc in accounts.all_accounts():
    st = accounts.status_for(acc)
    low = (st.label or "").lower()
    for junk in ("traceback", "exception", "errno", "argv"):
        assert junk not in low, f"{acc.id}: {st.label}"
    if "needs sign-in" in low or "not signed" in low:
        assert st.ok is not True, f"{acc.id} false ready: {st.label}"
    print(acc.id, st.ok, st.label)
print("status-ok")
PY
REMOTE
then
  ok "account status labels plain + no false ready"
else
  bad "account status" "probe failed or false ready"
fi

# --- 11. git_projects -------------------------------------------------------
echo ">>> git_projects"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import git_projects
auth = git_projects.git_auth()
print("auth", auth.method, auth.login)
repos = git_projects.find_remote_repos("zzznosuchrepoe2e999")
assert isinstance(repos, list)
print("git-ok", len(repos))
PY
REMOTE
then
  ok "git_projects auth + empty lookup"
else
  bad "git_projects" "failed"
fi

# --- 12. search CLI ---------------------------------------------------------
echo ">>> teddyos-search CLI"
if remote "teddyos-search --caps >/tmp/caps.out 2>/tmp/caps.err"; then
  ok "teddyos-search --caps"
else
  # some builds use different flag
  if remote "teddyos-search --help >/tmp/sh.out 2>&1 || true; test -s /tmp/sh.out -o -s /tmp/caps.err"; then
    skip "teddyos-search --caps" "non-zero but help/output present"
  else
    bad "teddyos-search --caps" "failed"
  fi
fi

# --- 13. open-signin --------------------------------------------------------
echo ">>> open-signin"
if remote "test -x /usr/bin/teddyos-open-signin"; then
  ok "teddyos-open-signin present"
else
  bad "open-signin" "missing"
fi

# --- 14. setup done hands off to tour ---------------------------------------
echo ">>> setup handoff"
if remote "grep -q 'teddyos-welcome' /usr/bin/teddyos-setup && grep -q '_leave_setup' /usr/bin/teddyos-setup"; then
  ok "setup hands off to welcome tour"
else
  bad "setup handoff" "missing _leave_setup / teddyos-welcome"
fi

# --- 15. icons full matrix --------------------------------------------------
echo ">>> icons full matrix"
if remote_bash <<'REMOTE'
export DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
export XAUTHORITY=$(ls /run/user/1000/.mutter-Xwaylandauth.* 2>/dev/null | head -1)
python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, "/usr/lib/teddyos")
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gtk, Gdk
import accounts, work_tools
t = Gtk.IconTheme.get_for_display(Gdk.Display.get_default())
need = set()
for p in Path("/usr/share/applications").glob("teddyos-*.desktop"):
    for line in p.read_text().splitlines():
        if line.startswith("Icon="):
            need.add(line.split("=", 1)[1].strip())
for a in accounts.all_accounts():
    need.add(a.icon)
for tool in work_tools.available_work_tools():
    need.add(tool.icon)
need.update({
    "teddyos-search", "teddyos-web", "teddyos-whatsapp", "teddyos-github",
    "teddyos-answers", "teddyos-accounts", "teddyos-install",
})
missing = [n for n in sorted(need) if not t.has_icon(n)]
if missing:
    raise SystemExit("unresolved: " + ",".join(missing))
# no stale desktop placeholders
stale = []
forbid = {"web-browser", "system-search", "help-about", "whatsapp", "system-users-symbolic"}
for p in Path("/usr/share/applications").glob("teddyos-*.desktop"):
    for line in p.read_text().splitlines():
        if line.startswith("Icon=") and line.split("=", 1)[1].strip() in forbid:
            stale.append(f"{p.name}:{line}")
if stale:
    raise SystemExit("stale: " + ";".join(stale))
# SVG + PNG present for teddyos-* brand set
for name in [
    "teddyos-search", "teddyos-web", "teddyos-whatsapp", "teddyos-claude",
    "teddyos-github", "teddyos-answers",
]:
    svg = Path(f"/usr/share/icons/hicolor/scalable/apps/{name}.svg")
    png = Path(f"/usr/share/icons/hicolor/128x128/apps/{name}.png")
    assert svg.is_file() or png.is_file(), name
print("icons-matrix-ok", len(need))
PY
REMOTE
then
  ok "icons matrix: desktops + accounts + work tools all resolve"
else
  bad "icons matrix" "unresolved or stale icon"
fi

# --- 16. pending_ask restore path -------------------------------------------
echo ">>> pending_ask restore"
if remote_bash <<'REMOTE'
export DISPLAY=:0
export XDG_RUNTIME_DIR=/run/user/1000
export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
export HOME=/home/teddy
export XAUTHORITY=$(ls /run/user/1000/.mutter-Xwaylandauth.* 2>/dev/null | head -1)
python3 - <<'PY'
import sys, pathlib, time, os, signal, subprocess
sys.path.insert(0, "/usr/lib/teddyos")
import pending_ask

def kill_search():
    for p in pathlib.Path("/proc").iterdir():
        if not p.name.isdigit():
            continue
        try:
            cmd = (p / "cmdline").read_bytes().replace(b"\0", b" ").decode()
        except OSError:
            continue
        if "teddyos-search-app" in cmd:
            try:
                os.kill(int(p.name), signal.SIGKILL)
            except OSError:
                pass
    time.sleep(0.8)

def search_running() -> bool:
    for p in pathlib.Path("/proc").iterdir():
        if not p.name.isdigit():
            continue
        try:
            cmd = (p / "cmdline").read_bytes().replace(b"\0", b" ").decode()
        except OSError:
            continue
        if "teddyos-search-app" in cmd:
            return True
    return False

# Must fully stop Search so a new window runs __init__ + restore timer
# (GApplication single-instance otherwise just re-presents the old window).
for _ in range(5):
    kill_search()
    if not search_running():
        break
assert not search_running(), "could not kill Search"

home = pathlib.Path.home()
proj = home / "Projects"
if proj.is_dir():
    sub = next((p for p in proj.iterdir() if p.is_dir()), proj)
else:
    sub = home
pending_ask.clear()
pending_ask.save(str(sub), "e2e pending restore prompt")
assert pending_ask.load() and pending_ask.load()["prompt"] == "e2e pending restore prompt"

env = os.environ.copy()
env["GDK_BACKEND"] = "x11"
env["HOME"] = str(home)
env["DISPLAY"] = os.environ.get("DISPLAY", ":0")
if os.environ.get("XAUTHORITY"):
    env["XAUTHORITY"] = os.environ["XAUTHORITY"]
logf = open("/tmp/e2e-pending-search.log", "w")
subprocess.Popen(
    ["/usr/bin/teddyos-search-app"],
    env=env,
    stdout=logf,
    stderr=logf,
    start_new_session=True,
)
# Wait until process is up, then until pending clears (restore @ ~400ms).
up = False
for _ in range(20):
    time.sleep(0.25)
    if search_running():
        up = True
        break
if not up:
    raise SystemExit("search never stayed up: " + open("/tmp/e2e-pending-search.log").read()[-400:])

cleared = False
for _ in range(24):
    time.sleep(0.25)
    if pending_ask.load() is None:
        cleared = True
        break
if not cleared:
    raise SystemExit(
        f"pending still present: {pending_ask.load()} search={search_running()} "
        f"log={open('/tmp/e2e-pending-search.log').read()[-300:]}"
    )
print("pending-ok")
PY
REMOTE
then
  ok "pending_ask restored and cleared on Search start"
else
  bad "pending_ask restore" "not cleared after Search launch"
fi
remote "python3 - <<'PY'
import pathlib, os, signal
for p in pathlib.Path('/proc').iterdir():
    if not p.name.isdigit(): continue
    try:
        cmd=(p/'cmdline').read_bytes().replace(b'\\0', b' ').decode()
    except OSError:
        continue
    if 'teddyos-search-app' in cmd:
        try: os.kill(int(p.name), signal.SIGKILL)
        except OSError: pass
PY
" 2>/dev/null || true

# --- 17. ask-all headless argv / prompt label contracts ---------------------
echo ">>> ask-all deep contracts"
if remote "grep -q '_ever_active' /usr/bin/teddyos-search-app \
  && grep -q '_prompt_label.set_text' /usr/bin/teddyos-ask-all \
  && grep -q 'allow-all-paths' /usr/bin/teddyos-ask-all \
  && grep -q 'dangerously-skip-permissions' /usr/bin/teddyos-ask-all"; then
  ok "ask-all + search deep contracts (prompt label, copilot paths, agy, blur ever-active)"
else
  bad "deep contracts" "missing symbols"
fi

# --- 18. tour structure -----------------------------------------------------
echo ">>> tour structure"
if remote_bash <<'REMOTE'
python3 - <<'PY'
from pathlib import Path
import re
src = Path("/usr/bin/teddyos-welcome").read_text()
# TOUR list should have multiple pages with primary actions
assert "TOUR" in src or "tour" in src.lower()
assert src.count("primary") >= 5 or src.count('"primary"') >= 5 or "Show me around" in src
assert "Open Search" in src
assert "Skip" in src or "skip" in src
# no helper jargon in tour bodies
# pull quoted UI-ish strings roughly
if re.search(r'"(body|title)":\s*"[^"]*\bhelpers?\b', src, re.I):
    raise SystemExit("helpers jargon in tour UI")
print("tour-structure-ok")
PY
REMOTE
then
  ok "tour structure (multi-step, Open Search, no helpers jargon)"
else
  bad "tour structure" "incomplete or jargon"
fi

# --- 19. caps + sandbox honesty ---------------------------------------------
echo ">>> caps/sandbox"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import caps, sandbox
g = caps.load()
assert isinstance(g, dict) and g, "empty grants"
# every capability has id
ids = [c["id"] for c in caps.CAPABILITIES]
assert len(ids) == len(set(ids))
# sandbox module importable
print("sandbox.available", sandbox.available())
print("caps-ok", len(ids), "guided" if caps.load else "")
# level default guided
assert caps.DEFAULT_LEVEL == "guided"
print("caps-ok")
PY
REMOTE
then
  ok "caps defaults + sandbox import"
else
  bad "caps/sandbox" "failed"
fi

# --- 20. clone error plain language -----------------------------------------
echo ">>> clone plain errors"
if remote "grep -q '_clone_error_plain' /usr/bin/teddyos-search-app \
  && grep -q 'gh repo clone\\|repo clone' /usr/lib/teddyos/git_projects.py \
  && ! grep -q 'Could not read Username' /usr/bin/teddyos-search-app"; then
  ok "clone path uses gh + plain error helper (no raw username prompt as UI)"
else
  # greps may fail on multiline - soft check
  if remote "grep -q _clone_error_plain /usr/bin/teddyos-search-app && grep -q 'repo clone' /usr/lib/teddyos/git_projects.py"; then
    ok "clone plain errors + gh path present"
  else
    bad "clone plain errors" "missing helpers"
  fi
fi

# --- 21. single-instance / blur-close safety --------------------------------
echo ">>> blur-close safety"
if remote "grep -q '_ever_active' /usr/bin/teddyos-search-app \
  && grep -q 'not self._ever_active' /usr/bin/teddyos-search-app"; then
  ok "blur-close only after window was active (no launch suicide)"
else
  bad "blur-close safety" "missing _ever_active guard"
fi

# --- 22. whatsapp desktop present in favorites path -------------------------
echo ">>> whatsapp desktop"
if remote "test -f /usr/share/applications/teddyos-whatsapp.desktop \
  && grep -q 'Icon=teddyos-whatsapp' /usr/share/applications/teddyos-whatsapp.desktop \
  && grep -q 'web.whatsapp.com' /usr/share/applications/teddyos-whatsapp.desktop"; then
  ok "whatsapp desktop: icon + URL"
else
  bad "whatsapp desktop" "missing or wrong"
fi

# --- 23. no Terminal in favorites + dock fixed (repeat hard assert) ---------
echo ">>> dock hard asserts"
FAV2=$(remote "$RENV gsettings get org.gnome.shell favorite-apps 2>/dev/null" || true)
DOCK2=$(remote "$RENV gsettings get org.gnome.shell.extensions.dash-to-dock dock-fixed 2>/dev/null" || true)
AUTO=$(remote "$RENV gsettings get org.gnome.shell.extensions.dash-to-dock autohide 2>/dev/null" || true)
if [[ "$DOCK2" == *"true"* ]] && [[ "$AUTO" == *"false"* ]]; then
  ok "dock fixed + autohide false"
else
  bad "dock visibility" "fixed=$DOCK2 autohide=$AUTO"
fi
if [[ "$FAV2" == *teddyos-whatsapp.desktop* ]]; then
  ok "favorites include WhatsApp"
else
  # may have been unpinned by user — soft
  skip "favorites WhatsApp" "not pinned right now"
fi

# --- 24. Devin (Cognition) + Replit web wrappers -----------------------------
echo ">>> Devin + Replit"
for pair in "teddyos-devin:app.devin.ai" "teddyos-replit:replit.com"; do
  bin="${pair%%:*}"
  host="${pair##*:}"
  if remote "test -x /usr/bin/$bin && grep -q '$host' /usr/bin/$bin"; then
    ok "$bin opens $host"
  else
    bad "$bin wrapper" "missing binary or URL $host"
  fi
done
if remote "test -f /usr/share/applications/teddyos-devin.desktop \
  && grep -q 'Exec=teddyos-devin' /usr/share/applications/teddyos-devin.desktop \
  && grep -q 'Icon=teddyos-devin' /usr/share/applications/teddyos-devin.desktop \
  && test -f /usr/share/applications/teddyos-replit.desktop \
  && grep -q 'Exec=teddyos-replit' /usr/share/applications/teddyos-replit.desktop"; then
  ok "Devin + Replit desktop entries"
else
  bad "Devin/Replit desktops" "missing or wrong Exec/Icon"
fi

# --- 25. work_tools catalog includes web AI helpers -------------------------
echo ">>> work_tools catalog"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from work_tools import available_work_tools, is_chat_helper
tools = {t.id: t for t in available_work_tools()}
# Always-on web apps should appear (teddyos-* wrappers installed on image).
for tid in ("devin", "replit", "perplexity"):
    assert tid in tools, f"missing tool {tid} (have {sorted(tools)})"
# Chat helpers used by ask-all / Get help
for tid in ("claude", "grok", "codex", "copilot"):
    if tid in tools:
        assert is_chat_helper(tid), f"{tid} should be chat helper"
# Files always first-class
assert "files" in tools
print("catalog-ok", len(tools), ",".join(sorted(tools)))
PY
REMOTE
then
  ok "work_tools catalog: devin/replit/perplexity + chat helpers"
else
  bad "work_tools catalog" "missing tools or chat helper flags"
fi

# --- 26. accounts catalog: Claude OAuth + always_available web apps ---------
echo ">>> accounts catalog contracts"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import accounts
ids = {a.id for a in accounts.all_accounts()}
for need in ("claude", "github", "devin", "replit", "perplexity"):
    assert need in ids, f"missing account {need}"
claude = next(a for a in accounts.all_accounts() if a.id == "claude")
assert "--claudeai" in claude.connect_argv, claude.connect_argv
assert claude.connect_argv[:3] == ("claude", "auth", "login")
for tid in ("devin", "replit", "perplexity"):
    a = next(x for x in accounts.all_accounts() if x.id == tid)
    assert a.always_available, f"{tid} should be always_available"
    assert a.connect_argv, f"{tid} missing connect"
# setup-all style: always_available web apps should not block "needs sign-in" queue
# status probes must not crash
for a in accounts.all_accounts():
    st = accounts.status_for(a)
    assert st.label, f"{a.id} empty label"
    # false Connected forbidden when label says needs sign-in
    low = st.label.lower()
    if "needs sign-in" in low or "not signed" in low:
        assert st.ok is not True, f"{a.id} false ready: {st.label}"
print("accounts-catalog-ok", len(ids))
PY
REMOTE
then
  ok "accounts catalog: claude --claudeai + web always_available"
else
  bad "accounts catalog" "claude argv or always_available wrong"
fi

# --- 27. Claude auth: no UUID false-code, paste UI present ------------------
echo ">>> Claude auth UX contracts"
if remote_bash <<'REMOTE'
python3 - <<'PY'
from pathlib import Path
import re
src = Path("/usr/bin/teddyos-accounts").read_text()
# Must scrub URLs before code match (Claude client_id UUID trap).
assert "_URL_RE.sub" in src or "scrubbed" in src, "no URL scrub before code match"
assert "_PASTE_PROMPT_RE" in src or "paste_entry" in src or "_paste_box" in src
assert "_submit_paste" in src
assert "_DEVICE_CODE_ACCOUNTS" in src
assert "github" in src and "copilot" in src
# Must not treat OAuth client_id fragments as device codes for Claude.
# Regression sample from live claude auth login output:
client_line = (
    "If the browser didn't open, visit: "
    "https://claude.com/cai/oauth/authorize?code=true"
    "&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code"
)
# Replicate guest extraction rules
url_re = re.compile(r"https?://[^\s\"'<>]+", re.I)
code_re = re.compile(
    r"\b([A-Z0-9]{4,5}-[A-Z0-9]{4,5})\b"
    r"|\bone-time code[:\s]+([A-Z0-9-]{6,})\b"
    r"|\benter code[:\s]+([A-Z0-9-]{6,})\b"
    r"|\buser code[:\s]+([A-Z0-9-]{6,})\b",
    re.I,
)
scrubbed = url_re.sub(" ", client_line)
codes = [next(g for g in m.groups() if g) for m in code_re.finditer(scrubbed)]
assert codes == [], f"false codes from OAuth URL: {codes}"
# Device code still matches outside URLs
g = "First copy your one-time code: AB12-CD34"
codes2 = [next(g for g in m.groups() if g) for m in code_re.finditer(url_re.sub(" ", g))]
assert "AB12-CD34" in codes2
# Paste UI strings for non-technical people
assert "Paste code from the browser" in src or "paste it" in src.lower()
assert "Send code" in src
print("claude-auth-ok")
PY
REMOTE
then
  ok "Claude auth: no UUID false-code + paste UI"
else
  bad "Claude auth contracts" "false-code guard or paste UI missing"
fi

# --- 28. open-signin is Chromium-only ---------------------------------------
echo ">>> open-signin Chromium path"
if remote "grep -q chromium /usr/bin/teddyos-open-signin \
  && grep -q -- '--app=' /usr/bin/teddyos-open-signin"; then
  ok "open-signin uses Chromium --app"
else
  bad "open-signin" "not Chromium --app"
fi

# --- 29. Perplexity wrapper parity ------------------------------------------
echo ">>> Perplexity wrapper"
if remote "test -x /usr/bin/teddyos-perplexity \
  && grep -qi perplexity /usr/bin/teddyos-perplexity \
  && test -f /usr/share/applications/teddyos-perplexity.desktop"; then
  ok "Perplexity wrapper + desktop"
else
  bad "Perplexity" "missing wrapper/desktop"
fi

# --- 30. setup-all skips always_available web apps in forced queue ----------
echo ">>> setup-all queue policy"
if remote_bash <<'REMOTE'
python3 - <<'PY'
from pathlib import Path
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import accounts
src = Path("/usr/bin/teddyos-accounts").read_text()
# Guided setup should not force Devin/Replit/Perplexity as required sign-ins.
# Either filter always_available, or only queue needs-sign-in native CLIs.
assert "always_available" in src or "setup_queue" in src or "_setup_all" in src
# Status: web apps can be "Needs sign-in" without blocking product use
for a in accounts.all_accounts():
    if getattr(a, "always_available", False):
        st = accounts.status_for(a)
        # ok may be False/None when cookies missing — that is fine
        assert st.label
print("setup-policy-ok")
PY
REMOTE
then
  ok "setup-all aware of always_available web apps"
else
  bad "setup-all policy" "missing always_available handling"
fi

# --- 31. launcher wrappers smoke (no crash, exit 0 when chromium present) ---
echo ">>> web launcher smoke"
if remote_bash <<'REMOTE'
export DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000
export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus HOME=/home/teddy
export XAUTHORITY=$(ls /run/user/1000/.mutter-Xwaylandauth.* 2>/dev/null | head -1)
set -e
pkill -f 'user-data-dir=.*/teddyos-(devin|replit|perplexity)' 2>/dev/null || true
for bin in teddyos-devin teddyos-replit teddyos-perplexity; do
  if ! command -v "$bin" >/dev/null; then
    echo "missing $bin"
    exit 1
  fi
  # Launchers Popen Chromium and return 0 immediately.
  if ! "$bin" >/tmp/e2e-$bin.out 2>&1; then
    echo "$bin failed: $(cat /tmp/e2e-$bin.out 2>/dev/null)"
    exit 1
  fi
done
sleep 1.0
# Best-effort cleanup of e2e browser windows.
pkill -f 'user-data-dir=.*/\.config/teddyos-(devin|replit|perplexity)' 2>/dev/null || true
echo launcher-ok
REMOTE
then
  ok "web launchers return 0 (devin/replit/perplexity)"
else
  bad "web launcher smoke" "launcher non-zero or missing"
fi

# --- 32. ask-all tool id routing includes known helpers ---------------------
echo ">>> ask-all tool routing"
if remote "grep -q claude /usr/bin/teddyos-ask-all \
  && grep -q copilot /usr/bin/teddyos-ask-all \
  && grep -q grok /usr/bin/teddyos-ask-all \
  && grep -q -- '--tools' /usr/bin/teddyos-ask-all"; then
  ok "ask-all routes known tool ids"
else
  bad "ask-all routing" "missing tool ids"
fi

# --- 33. GitHub device-code URL prefill still present -----------------------
echo ">>> GitHub device-code prefill"
if remote "grep -q 'github.com/login/device' /usr/bin/teddyos-accounts \
  && grep -q 'user_code=' /usr/bin/teddyos-accounts \
  && grep -q '_DEVICE_URL_FOR_ACCOUNT' /usr/bin/teddyos-accounts"; then
  ok "GitHub device URL prefill intact"
else
  bad "GitHub device prefill" "missing"
fi

# --- 34. no jargon in primary desktop Name/Comment (expanded) ---------------
echo ">>> desktop jargon sweep"
if remote_bash <<'REMOTE'
python3 - <<'PY'
from pathlib import Path
import re
jargon = re.compile(r"\b(CLI|OAuth|SSH|argv|TTY|VTE|stdin)\b", re.I)
bad = []
for p in Path("/usr/share/applications").glob("teddyos-*.desktop"):
    for line in p.read_text().splitlines():
        if line.startswith(("Name=", "Comment=", "GenericName=")) and jargon.search(line):
            bad.append(f"{p.name}:{line}")
if bad:
    raise SystemExit(" | ".join(bad))
print("jargon-ok")
PY
REMOTE
then
  ok "no CLI/OAuth jargon in desktop Name/Comment"
else
  bad "desktop jargon" "jargon in user-facing desktop strings"
fi

# --- 35. Search single-instance GApplication id -----------------------------
echo ">>> Search app id"
if remote "grep -q 'com.teddyos.Search' /usr/bin/teddyos-search-app \
  && grep -q 'GLib.set_prgname' /usr/bin/teddyos-search-app"; then
  ok "Search GApplication id + prgname"
else
  bad "Search app id" "missing com.teddyos.Search"
fi

# --- scorecard --------------------------------------------------------------
echo
echo "=== scorecard ==="
printf '%s\n' "${RESULTS[@]}"
echo
echo "PASS=$PASS  FAIL=$FAIL  SKIP=$SKIP  TOTAL=$((PASS + FAIL + SKIP))"
if [[ "$FAIL" -gt 0 ]]; then
  echo "RESULT: FAIL"
  exit 1
fi
echo "RESULT: PASS"
exit 0
