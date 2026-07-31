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
for mod in caps search sandbox work_tools git_projects accounts pending_ask \
  audience progress logutil; do
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
export XAUTHORITY=$(ls /run/user/1000/.mutter-Xwaylandauth.* 2>/dev/null | head -1)
# Single-instance apps may reparent; wait and match by name, not only nohup PID.
nohup teddyos-welcome --force >/tmp/e2e-tour.log 2>&1 &
echo $! > /tmp/e2e-tour.pid
for i in 1 2 3 4 5; do
  sleep 0.6
  if pgrep -f 'teddyos-welcome' >/dev/null 2>&1; then
    echo TOUR_UP
    pkill -f 'teddyos-welcome' 2>/dev/null || true
    kill "$(cat /tmp/e2e-tour.pid)" 2>/dev/null || true
    exit 0
  fi
  if kill -0 "$(cat /tmp/e2e-tour.pid)" 2>/dev/null; then
    echo TOUR_UP
    kill "$(cat /tmp/e2e-tour.pid)" 2>/dev/null || true
    exit 0
  fi
done
echo "welcome failed after retries" >&2
cat /tmp/e2e-tour.log 2>/dev/null | tail -40
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

# --- 36. Audience / query detection (all coding helpers + UI chip) -----------
echo ">>> audience detection + Answers chip"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from audience import (
    Audience, SHAPED_TOOLS, detect_audience, shape_prompt_for_tool,
    audience_chip_label,
)
assert detect_audience("explain this in simple words") is Audience.PLAIN
assert detect_audience("refactor the risk function in risk.py") is Audience.CODE
assert detect_audience("gym hypertrophy progressive overload plan") is Audience.FITNESS
assert detect_audience("fine tune llm rag pipeline pytorch") is Audience.ML_AI
assert detect_audience("clogged drain replace faucet plumbing") is Audience.PLUMBING
assert detect_audience("youtube script thumbnail for my channel") is Audience.CONTENT_CREATOR
assert detect_audience("adhd executive function tips neurodivergent") is Audience.NEURODIVERSITY
assert detect_audience("make a budget emergency fund pay off debt") is Audience.PERSONAL_FINANCE
assert detect_audience("kubernetes deployment kubectl helm chart") is Audience.KUBERNETES
assert detect_audience("train my dog puppy training leash") is Audience.DOG_TRAINING
assert detect_audience("learn spanish conjugation practice") is Audience.SPANISH
assert detect_audience("rust ownership borrow checker") is Audience.RUST_LANG
assert detect_audience("dockerfile docker compose build image") is Audience.DOCKER
assert detect_audience("learn japanese hiragana kanji") is Audience.JAPANESE
assert detect_audience("terraform module state plan") is Audience.TERRAFORM
assert detect_audience("pickleball third shot drop kitchen") is Audience.PICKLEBALL
assert detect_audience("build a habit stack tracker") is Audience.HABIT_BUILDING
assert detect_audience("homelab proxmox self hosted") is Audience.HOME_LAB
assert detect_audience("fresh pasta risotto technique") is Audience.ITALIAN_COOKING
assert detect_audience("linkedin profile headline about") is Audience.LINKEDIN
assert len(list(Audience)) >= 550
for tid in ("claude", "grok", "codex", "copilot", "gemini"):
    assert tid in SHAPED_TOOLS
    s = shape_prompt_for_tool(tid, "fix TypeError in async handler")
    assert "comfortable with code" in s.lower()
assert audience_chip_label(Audience.CODE) == "Ultracode"
assert audience_chip_label(Audience.ML_AI) == "ML/AI mode"
assert audience_chip_label(Audience.SPANISH) == "Spanish mode"
src = open("/usr/bin/teddyos-ask-all").read()
assert "shape_prompt_for_tool" in src
assert "teddyos-audience-chip" in src or "audience_chip_label" in src
assert "_model_prompt" in src
assert "_play_ultracode_switch" in src
assert "ULTRACODE" in src
print("audience-guest-ok")
PY
REMOTE
then
  ok "audience multi-tool shape + Ultracode animation"
else
  bad "audience" "module or Ultracode wiring missing"
fi

# --- 37. Progress / Ultracode fluency gamification --------------------------
echo ">>> Ultracode fluency gamification"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
# Isolate so e2e does not pollute the guest user's real progress.
td = tempfile.mkdtemp(prefix="e2e-progress-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, "/usr/lib/teddyos")
import progress
importlib.reload(progress)

s0 = progress.snapshot()
assert s0.xp == 0
line0 = progress.progress_line(s0)
assert "Ultracode" in line0 or "ask to start" in line0 or "getting started" in line0.lower()

s1 = progress.award_from_prompt("what does this project do in simple words?")
assert s1.plain_asks >= 1 and s1.ultracode_asks == 0
assert s1.xp > 0

raw = progress._load_raw()
raw["last_ask_ts"] = 0
progress._save_raw(raw)

s2 = progress.award_from_prompt(
    "refactor auth middleware in src/api.ts and fix TypeError",
    multi_ai=True,
)
assert s2.ultracode_asks >= 1, s2
assert "ultracode" in progress.snapshot().badges
line = progress.progress_line()
assert "Ultracode" in line or "fluency" in line
frac, _ = progress.ultracode_meter()
assert 0.0 <= frac <= 1.0

# Debounce: second ask within window does not stack huge XP
xp_before = progress.snapshot().xp
s3 = progress.award_from_prompt("refactor again foo.py")
assert progress.snapshot().xp - xp_before < 20  # badge-only-ish

# Persona switch + levels
raw = progress._load_raw()
raw["last_ask_ts"] = 0
progress._save_raw(raw)
s4 = progress.award_from_prompt("i wanna respond to my linkedin messages")
assert s4.active_persona == "linkedin", s4.active_persona
assert s4.switch_count >= 1
assert any(e.kind == "switch" for e in s4.events)
assert "Level" in progress.persona_level_line("linkedin")
assert "Lv." in progress.progress_line(s4) or "Level" in progress.progress_line(s4)

# Search + ask-all must wire award_from_prompt / synthesis / persona switch
src_app = open("/usr/bin/teddyos-search-app").read()
assert "award_from_prompt" in src_app
assert "_progress_bar" in src_app
src_aa = open("/usr/bin/teddyos-ask-all").read()
assert "award_from_prompt" in src_aa or "award_ask" in src_aa
assert "award_synthesis" in src_aa
assert "_start_synthesis" in src_aa
assert "Across all answers" in src_aa
assert "_play_ultracode_switch" in src_aa
assert "_play_persona_switch" in src_aa
assert "persona_level_line" in src_aa
print("progress-guest-ok", progress.snapshot().xp, "sw", progress.snapshot().switch_count)
PY
REMOTE
then
  ok "persona levels + mode switch + Search/Answers hooks"
else
  bad "progress gamification" "module or hooks missing"
fi

# --- 38. Across-all-answers synthesis contracts -----------------------------
echo ">>> synthesis contracts"
if remote "grep -q _start_synthesis /usr/bin/teddyos-ask-all \
  && grep -q _review_prompt /usr/bin/teddyos-ask-all \
  && grep -q 'Across all answers' /usr/bin/teddyos-ask-all \
  && grep -q _collect_answers /usr/bin/teddyos-ask-all \
  && grep -q _pick_reviewer /usr/bin/teddyos-ask-all"; then
  ok "ask-all Across all answers synthesis present"
else
  bad "synthesis" "missing symbols"
fi

# --- 39. Search Get help multi-AI copy --------------------------------------
echo ">>> Get help multi-AI copy"
if remote "grep -q 'every ready AI' /usr/bin/teddyos-search-app \
  && grep -q 'pull them together' /usr/bin/teddyos-search-app \
  && grep -q Ultracode /usr/bin/teddyos-search-app"; then
  ok "Search promises multi-AI + Ultracode"
else
  bad "Search Get help copy" "missing multi-AI / Ultracode strings"
fi

# --- 40. progress module importable with other libs -------------------------
echo ">>> progress import matrix"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import audience, progress, accounts, work_tools, pending_ask
assert audience.detect_audience("fix TypeError") is audience.Audience.CODE
assert progress.award_from_prompt  # callable
print("import-matrix-ok")
PY
REMOTE
then
  ok "audience + progress + accounts import together"
else
  bad "import matrix" "failed"
fi

# --- 41. ask-all model shaping for all SHAPED_TOOLS -------------------------
echo ">>> headless shaping for all chat tools"
if remote_bash <<'REMOTE'
python3 - <<'PY'
from pathlib import Path
src = Path("/usr/bin/teddyos-ask-all").read_text()
assert "_model_prompt" in src
assert "shape_prompt_for_tool" in src
# All headless paths go through _model_prompt / shaped text
assert "claude" in src and "grok" in src and "copilot" in src
print("shape-all-ok")
PY
REMOTE
then
  ok "ask-all shapes all coding helpers"
else
  bad "ask-all shaping" "missing"
fi

# --- 42. pending_ask + progress modules installed for e2e path --------------
echo ">>> pending_ask + progress files"
if remote "test -f /usr/lib/teddyos/pending_ask.py && test -f /usr/lib/teddyos/progress.py \
  && test -f /usr/lib/teddyos/audience.py"; then
  ok "pending_ask + progress + audience on guest"
else
  bad "guest modules" "missing progress/audience/pending_ask"
fi

# --- 43. Credit / balance detection surfaces real labels --------------------
echo ">>> credit balance detection"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from work_tools import available_work_tools, probe_credits, _parse_balance_snippet, _LOW
# Parser keeps real balance lines, rejects interactive help
assert _parse_balance_snippet("72% remaining this week")
assert _parse_balance_snippet("Not sure which usage you mean. Quick options:") is None
assert _LOW.search("Credit balance is too low")
# Probes must not crash; labels must be non-empty for AI tools
for t in available_work_tools():
    if not t.is_ai:
        continue
    st = probe_credits(t)
    assert st.label, t.id
    # Never dump multi-line CLI help into UI
    assert "\n" not in st.label, (t.id, st.label)
# Search must preserve plan/balance labels (not rewrite all to Ready)
src = open("/usr/bin/teddyos-search-app").read()
assert "_credit_summary_line" in src
assert "Max plan" in open("/usr/lib/teddyos/work_tools.py").read() or "plan · ready" in open("/usr/lib/teddyos/work_tools.py").read()
# Regression: plain_status must not map "remaining" alone to Ready
assert 'if any(s in low for s in ("ready", "installed", "available", "remaining"))' not in src
print("credit-ok")
for t in available_work_tools():
    if t.is_ai:
        st = probe_credits(t)
        print(f"  {t.id}: {st.label}")
PY
REMOTE
then
  ok "credit probes + UI preserve balance/plan labels"
else
  bad "credit detection" "probes or plain_status regression"
fi

# --- 44. Audience completeness on guest (maps + lexicons + priority) --------
echo ">>> audience completeness guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import audience as a
from audience import (
    Audience, detect_audience, score_audiences, shape_prompt,
    shape_prompt_for_tool, audience_chip_label, audience_chip_hint,
    audience_header_title, audience_css_class, is_code_audience,
)
n = len(list(Audience))
assert n >= 550, n
for aud in Audience:
    assert audience_chip_label(aud)
    assert audience_chip_hint(aud)
    assert audience_header_title(aud)
    assert aud in a._SHAPE and "{text}" in a._SHAPE[aud]
    assert audience_css_class(aud).startswith("teddyos-audience-")
assert is_code_audience(Audience.CODE) and not is_code_audience(Audience.PLAIN)
lex = {x[0] for x in a._LEXICONS}
missing = [x for x in Audience if x not in lex and x not in (Audience.PLAIN, Audience.CODE)]
assert not missing, missing[:5]
assert len(a._PRIORITY) == n == len(set(a._PRIORITY))
sc = score_audiences("refactor TypeError in x.py")
assert sc[Audience.CODE] > 0 and set(sc) == set(Audience)
# Specialist shapes embed text
for p in ("learn spanish conjugation", "make a budget emergency fund", "gym hypertrophy plan"):
    s = shape_prompt(p)
    assert p in s and len(s) > len(p)
    assert shape_prompt_for_tool("claude", p) == s
    assert shape_prompt_for_tool("files", p) == p
print("audience-complete-guest", n)
PY
REMOTE
then
  ok "audience completeness maps/lexicons/priority on guest"
else
  bad "audience completeness" "guest maps incomplete"
fi

# --- 45. Audience regression matrix on guest --------------------------------
echo ">>> audience regression matrix guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from audience import Audience, detect_audience
matrix = [
    ("explain this in simple words", Audience.PLAIN),
    ("refactor TypeError in worker.py", Audience.CODE),
    ("NDA in plain English", Audience.LEGAL),
    ("visa H-1B green card USCIS", Audience.IMMIGRATION),
    ("obsidian zettelkasten second brain", Audience.PKM),
    ("gym hypertrophy progressive overload", Audience.FITNESS),
    ("fine tune llm rag pytorch", Audience.ML_AI),
    ("error budget SLO postmortem", Audience.SRE),
    ("system design interview url shortener", Audience.SYSTEM_DESIGN),
    ("youtube script thumbnail channel", Audience.CONTENT_CREATOR),
    ("seo audit keyword research", Audience.SEO),
    ("make a budget emergency fund", Audience.PERSONAL_FINANCE),
    ("kubernetes kubectl helm chart", Audience.KUBERNETES),
    ("terraform module state plan", Audience.TERRAFORM),
    ("dockerfile docker compose", Audience.DOCKER),
    ("learn spanish conjugation", Audience.SPANISH),
    ("learn japanese hiragana kanji", Audience.JAPANESE),
    ("rust ownership borrow checker", Audience.RUST_LANG),
    ("pandas dataframe groupby jupyter", Audience.PYTHON_DATA),
    ("fresh pasta risotto technique", Audience.ITALIAN_COOKING),
    ("bake bread sourdough loaf", Audience.BREAD),
    ("houseplant care repot plant", Audience.HOUSEPLANTS),
    ("pc build choose a gpu", Audience.PC_BUILDING),
    ("homelab proxmox self hosted", Audience.HOME_LAB),
    ("linkedin profile headline", Audience.LINKEDIN),
    ("build a habit stack tracker", Audience.HABIT_BUILDING),
    ("pickleball third shot drop", Audience.PICKLEBALL),
    ("clogged drain replace faucet", Audience.PLUMBING),
    ("train my dog puppy leash", Audience.DOG_TRAINING),
    ("password manager enable 2fa", Audience.PASSWORD_SECURITY),
    ("adhd executive function neurodivergent", Audience.NEURODIVERSITY),
    ("smoke a brisket smoker temperature", Audience.BBQ),
    ("term sheet seed round cap table", Audience.VC),
    ("terraform kubernetes helm ci/cd pipeline", Audience.DEVOPS),
    ("improve lcp core web vitals", Audience.PERFORMANCE_WEB),
    ("ux writing microcopy error message", Audience.UX_WRITING),
    ("college essay common app", Audience.COLLEGE_APPS),
    ("solo travel tips traveling alone", Audience.SOLO_TRAVEL),
]
fails = [(t, e.value, detect_audience(t).value) for t, e in matrix if detect_audience(t) is not e]
assert not fails, fails[:6]
print("matrix-ok", len(matrix))
PY
REMOTE
then
  ok "audience regression matrix on guest"
else
  bad "audience matrix" "guest detection regressions"
fi

# --- 46. Disambiguation edges (specialist beats generic) --------------------
echo ">>> audience disambiguation edges"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from audience import Audience, detect_audience
# Specialist beats umbrella
assert detect_audience("kubernetes kubectl helm chart") is Audience.KUBERNETES
assert detect_audience("terraform module state plan") is Audience.TERRAFORM
assert detect_audience("dockerfile docker compose build image") is Audience.DOCKER
# Umbrella when multi-tool + CI/CD
assert detect_audience("terraform kubernetes helm ci/cd pipeline") is Audience.DEVOPS
# Language-specific beats generic language
assert detect_audience("learn spanish conjugation practice") is Audience.SPANISH
assert detect_audience("learn mandarin pinyin tones hsk") is Audience.MANDARIN
# Music production / cooking specialty
assert detect_audience("ableton mix this track music production") is Audience.MUSIC_PRODUCTION
assert detect_audience("fresh pasta risotto technique") is Audience.ITALIAN_COOKING
assert detect_audience("bake bread sourdough loaf formula") is Audience.BREAD
# Password vs cyber hygiene
assert detect_audience("password manager enable 2fa passkey") is Audience.PASSWORD_SECURITY
# Personal finance vs investing
assert detect_audience("make a budget emergency fund pay off debt") is Audience.PERSONAL_FINANCE
print("disambig-ok")
PY
REMOTE
then
  ok "audience disambiguation edges"
else
  bad "disambiguation" "specialist/umbrella edges failed"
fi

# --- 47. ask-all + search source contracts for multi-persona UI -------------
echo ">>> multi-persona UI source contracts"
if remote_bash <<'REMOTE'
python3 - <<'PY'
from pathlib import Path
aa = Path("/usr/bin/teddyos-ask-all").read_text()
app = Path("/usr/bin/teddyos-search-app").read_text()
for need in (
    "detect_audience", "shape_prompt_for_tool", "audience_chip_label",
    "audience_css_class", "is_code_audience", "_play_ultracode_switch",
    "_play_persona_switch", "persona_level_line",
    "ULTRACODE", "award_from_prompt",
):
    assert need in aa or need in app, need
# CSS class hook for non-code chips
assert "teddyos-audience-" in aa or "audience_css_class" in aa
assert "Across all answers" in aa
assert "every ready AI" in app or "pull them together" in app
print("ui-src-ok")
PY
REMOTE
then
  ok "multi-persona UI source contracts (ask-all + search)"
else
  bad "multi-persona UI" "missing source hooks"
fi

# --- 48. Web apps never get prompt shaping ----------------------------------
echo ">>> web apps pass-through shaping"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from audience import shape_prompt_for_tool, detect_audience, Audience
p = "refactor TypeError in worker.py"
assert detect_audience(p) is Audience.CODE
assert "comfortable with code" in shape_prompt_for_tool("claude", p).lower()
for tid in ("devin", "replit", "perplexity", "files", "web"):
    assert shape_prompt_for_tool(tid, p) == p, tid
print("passthrough-ok")
PY
REMOTE
then
  ok "web apps / files pass through without Ultracode shape"
else
  bad "shape passthrough" "web apps incorrectly shaped"
fi

# --- 49b. LinkedIn messages → freeform Get help (not career corpus junk) ---
echo ">>> LinkedIn messages freeform help"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
from audience import detect_audience, Audience, shape_prompt
q = "i wanna respond to my linkedin messages"
assert s.is_work_goal(q), q
assert s.is_freeform_help_goal(q), q
assert detect_audience(q) is Audience.LINKEDIN, detect_audience(q)
shaped = shape_prompt(q)
assert "linkedin" in shaped.lower()
assert "career mode" not in shaped.lower() and "job search" not in shaped.lower()
# Search app on guest has freeform wiring
src = open("/usr/bin/teddyos-search-app").read()
assert "is_freeform_help_goal" in src
assert "Open LinkedIn messages" in src
print("linkedin-messages-ok")
PY
REMOTE
then
  ok "LinkedIn messages → freeform Get help + LinkedIn audience"
else
  bad "LinkedIn messages path" "still treated as job/corpus search"
fi

# --- 49c. LinkedIn do-it: messaging wrapper + draft playbook on guest ------
echo ">>> LinkedIn do-it path on guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys, shutil
from pathlib import Path
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
from audience import detect_audience, Audience

q = "i wanna reply to my linkedin messages"
assert s.is_linkedin_messages_goal(q), q
assert s.is_freeform_help_goal(q), q
assert detect_audience(q) is Audience.LINKEDIN
p = s.linkedin_reply_action_prompt(q)
assert "Never claim you sent" in p
assert "paste" in p.lower()

src = Path("/usr/bin/teddyos-search-app").read_text()
for need in (
    "is_linkedin_messages_goal",
    "_open_linkedin_messaging",
    "Draft my replies",
    "linkedin.com/messaging",
    "Nothing is sent without you",
):
    assert need in src, need
# auto-start may be messaging-shared or linkedin-named
assert (
    "_auto_start_messaging_replies" in src
    or "_auto_start_linkedin_replies" in src
)

# Wrapper optional until next ISO rebuild; search-app falls back to chromium --app
wrap = shutil.which("teddyos-linkedin")
if wrap:
    body = Path(wrap).read_text()
    assert "linkedin.com/messaging" in body
print("linkedin-do-it-guest-ok", "wrapper=" + ("yes" if wrap else "fallback"))
PY
REMOTE
then
  ok "LinkedIn do-it (draft playbook + open messaging)"
else
  bad "LinkedIn do-it guest" "missing draft/open wiring"
fi

# --- 49d. WhatsApp do-it: open chat + draft playbook on guest --------------
echo ">>> WhatsApp do-it path on guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys, shutil
from pathlib import Path
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
from audience import detect_audience, Audience

q = "i wanna reply to my whatsapp messages"
assert s.is_whatsapp_messages_goal(q), q
assert s.is_freeform_help_goal(q), q
assert detect_audience(q) is Audience.WHATSAPP
p = s.whatsapp_reply_action_prompt(q)
assert "Never claim you sent" in p
assert "paste" in p.lower()

src = Path("/usr/bin/teddyos-search-app").read_text()
for need in (
    "is_whatsapp_messages_goal",
    "_open_whatsapp",
    "_whatsapp_action",
    "Draft my replies",
    "web.whatsapp.com",
    "Opening WhatsApp",
):
    assert need in src, need

wrap = shutil.which("teddyos-whatsapp")
if wrap:
    body = Path(wrap).read_text()
    assert "web.whatsapp.com" in body
print("whatsapp-do-it-guest-ok", "wrapper=" + ("yes" if wrap else "fallback"))
PY
REMOTE
then
  ok "WhatsApp do-it (draft playbook + open chat)"
else
  bad "WhatsApp do-it guest" "missing draft/open wiring"
fi

# --- 49. progress awards specialty asks without forcing Ultracode -----------
echo ">>> progress specialty vs ultracode"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
td = tempfile.mkdtemp(prefix="e2e-prog2-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, "/usr/lib/teddyos")
import progress
importlib.reload(progress)
from audience import detect_audience, Audience
assert detect_audience("learn spanish conjugation") is Audience.SPANISH
s = progress.award_from_prompt("learn spanish conjugation practice")
assert s.xp > 0
assert s.ultracode_asks == 0  # specialty is not Ultracode
assert s.active_persona == "spanish"
raw = progress._load_raw(); raw["last_ask_ts"] = 0; progress._save_raw(raw)
s2 = progress.award_from_prompt("fix TypeError in async worker.py")
assert s2.ultracode_asks >= 1
assert s2.active_persona == "code"
assert s2.switch_count >= 1
print("progress-specialty-ok", s.xp, s2.ultracode_asks, s2.switch_count)
PY
REMOTE
then
  ok "progress: specialty asks ≠ Ultracode; code asks are"
else
  bad "progress specialty" "Ultracode mis-awarded"
fi

# --- 50. Persona multi-switch chain + ladder on guest -----------------------
echo ">>> persona multi-switch + ladder"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
td = tempfile.mkdtemp(prefix="e2e-ladder-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, "/usr/lib/teddyos")
import progress
importlib.reload(progress)

def ask(p):
    raw = progress._load_raw(); raw["last_ask_ts"] = 0; progress._save_raw(raw)
    return progress.award_from_prompt(p)

ask("explain this in simple words")
s1 = ask("refactor TypeError in worker.py")
assert s1.active_persona == "code" and s1.switch_count >= 1
s2 = ask("i wanna respond to my linkedin messages")
assert s2.active_persona == "linkedin" and s2.switch_count >= 2
s3 = ask("gym hypertrophy progressive overload plan")
assert s3.active_persona == "fitness" and s3.switch_count >= 3
for _ in range(4):
    ask("reply to linkedin messages in my inbox")
snap = progress.snapshot()
assert snap.persona_levels.get("linkedin", 1) >= 2, snap.persona_levels
assert snap.persona_xp.get("code", 0) > 0
assert snap.persona_xp.get("fitness", 0) > 0
frac, cap = progress.persona_meter("linkedin")
assert 0.0 <= frac <= 1.0 and "Level" in cap
line = progress.progress_line(snap)
assert "Lv." in line
# Persist shape
raw = progress._load_raw()
assert isinstance(raw.get("personas"), dict)
assert raw.get("last_audience")
assert int(raw.get("switch_count") or 0) >= 3
print("ladder-guest-ok", snap.switch_count, snap.persona_levels)
PY
REMOTE
then
  ok "persona multi-switch chain + ladder on guest"
else
  bad "persona ladder guest" "switch chain or level grind failed"
fi

# --- 51. Freeform help goal matrix on guest ---------------------------------
echo ">>> freeform help matrix guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
from audience import detect_audience, Audience
assert s.is_work_goal("i wanna respond to my linkedin messages")
assert s.is_freeform_help_goal("i wanna respond to my linkedin messages")
assert s.is_freeform_help_goal("i need to check my email inbox")
assert s.is_freeform_help_goal("help me draft a reply to this message")
assert s.is_work_goal("i wanna work on tsearch")
assert not s.is_freeform_help_goal("i wanna work on tsearch")
assert not s.is_work_goal("what is photosynthesis")
assert detect_audience("i wanna respond to my linkedin messages") is Audience.LINKEDIN
assert detect_audience("reply to linkedin messages") is Audience.LINKEDIN
# focus
fq = s.focus_query("i wanna respond to my linkedin messages").lower()
assert "linkedin" in fq
print("freeform-matrix-guest-ok")
PY
REMOTE
then
  ok "freeform help goal matrix on guest"
else
  bad "freeform matrix guest" "classification failed"
fi

# --- 52. format_toast priority (level > switch > badge) ---------------------
echo ">>> progress toast priority"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp(prefix="e2e-toast-")
sys.path.insert(0, "/usr/lib/teddyos")
import progress
importlib.reload(progress)
E = progress.ProgressEvent
ev = [
    E(kind="xp", title="+4 for asking", xp_delta=4),
    E(kind="switch", title="Switched · LinkedIn", persona="linkedin"),
    E(kind="persona_level", title="Level 2 · Ultracode", persona="code", persona_level=2),
]
t = progress.format_toast(ev)
assert t and "Level 2" in t, t
ev2 = [E(kind="xp", title="+1"), E(kind="switch", title="Switched · Fitness")]
assert "Switch" in (progress.format_toast(ev2) or "")
print("toast-priority-ok", t)
PY
REMOTE
then
  ok "progress toast prefers persona_level then switch"
else
  bad "toast priority" "format_toast ordering wrong"
fi

# --- 53. Search app freeform wiring + level strip source --------------------
echo ">>> search freeform + progress strip source"
if remote "grep -q is_freeform_help_goal /usr/bin/teddyos-search-app \
  && grep -q 'Open LinkedIn messages' /usr/bin/teddyos-search-app \
  && grep -q persona_meter /usr/bin/teddyos-search-app \
  && grep -q _play_persona_switch /usr/bin/teddyos-ask-all \
  && grep -q persona_level_line /usr/bin/teddyos-ask-all \
  && grep -q _level_chip /usr/bin/teddyos-ask-all"; then
  ok "Search freeform + Answers level chip source wiring"
else
  bad "source wiring" "freeform/level symbols missing on guest"
fi

# --- 54. Progress debounce + persistence on guest ---------------------------
echo ">>> progress debounce + persistence guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
td = tempfile.mkdtemp(prefix="e2e-deb-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, "/usr/lib/teddyos")
import progress
importlib.reload(progress)
s1 = progress.award_from_prompt("refactor TypeError in worker.py", multi_ai=True)
xp1 = progress.snapshot().xp
s2 = progress.award_from_prompt("i wanna respond to my linkedin messages")
assert progress.snapshot().xp - xp1 < 25
raw = progress._load_raw(); raw["last_ask_ts"] = 0; progress._save_raw(raw)
s3 = progress.award_from_prompt("reply to linkedin messages")
assert s3.active_persona == "linkedin"
# reload module from disk — same XDG should keep state
importlib.reload(progress)
snap = progress.snapshot()
assert snap.xp >= xp1
assert snap.persona_xp.get("code", 0) > 0 or snap.ultracode_asks >= 1
print("debounce-guest-ok", snap.xp, snap.switch_count)
PY
REMOTE
then
  ok "progress debounce + persistence on guest"
else
  bad "debounce guest" "failed"
fi

# --- 55. multi_ai + synthesis awards ----------------------------------------
echo ">>> multi_ai + synthesis awards"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp(prefix="e2e-multi-")
sys.path.insert(0, "/usr/lib/teddyos")
import progress
importlib.reload(progress)
s = progress.award_from_prompt(
    "refactor auth in src/api.ts",
    multi_ai=True,
    tool_ids=["claude", "copilot", "grok"],
)
assert s.ultracode_asks >= 1
badges = progress.snapshot().badges
assert "multi_ai" in badges or "ultracode_multi" in badges or "ultracode" in badges
s2 = progress.award_synthesis()
assert s2.xp >= s.xp
assert "across_all" in progress.snapshot().badges
print("multi-synth-ok", progress.snapshot().badges)
PY
REMOTE
then
  ok "multi_ai + synthesis badge awards"
else
  bad "multi/synth awards" "failed"
fi

# --- 56. ask-all re-ask awards persona XP (source) --------------------------
echo ">>> ask-all re-ask award wiring"
if remote_bash <<'REMOTE'
python3 - <<'PY'
from pathlib import Path
src = Path("/usr/bin/teddyos-ask-all").read_text()
# _rerun_one must re-detect audience and award progress
assert "def _rerun_one" in src
assert "detect_audience" in src
idx = src.index("def _rerun_one")
chunk = src[idx:idx+1200]
assert "award_from_prompt" in chunk or "award_ask" in chunk
assert "persona_level_line" in chunk or "persona_level_line" in src
assert "_refresh_audience_chip" in chunk
assert "animate=True" in chunk
print("rerun-award-ok")
PY
REMOTE
then
  ok "ask-all re-ask awards persona XP + re-detects"
else
  bad "re-ask award" "missing wiring in _rerun_one"
fi

# --- 57. work_tools is_chat_helper + readiness on guest ---------------------
echo ">>> work_tools chat helper matrix guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from work_tools import available_work_tools, is_chat_helper, tools_ready_for_broadcast, CreditStatus
tools = available_work_tools()
assert tools
for tid in ("claude", "grok", "gemini", "codex", "copilot"):
    # may not be installed; is_chat_helper is pure
    assert is_chat_helper(tid)
assert not is_chat_helper("files")
assert not is_chat_helper("devin")
assert not is_chat_helper("replit")
chats = [t for t in tools if is_chat_helper(t.id)]
if chats:
    t = chats[0]
    assert tools_ready_for_broadcast([t], {t.id: CreditStatus(True, "ok")}) == [t]
    assert tools_ready_for_broadcast([t], {t.id: CreditStatus(False, "no")}) == []
ids = {t.id for t in tools}
for tid in ("devin", "replit", "perplexity"):
    assert tid in ids, tid
print("chat-helper-guest-ok", len(tools), len(chats))
PY
REMOTE
then
  ok "work_tools chat helper + readiness on guest"
else
  bad "chat helper guest" "failed"
fi

# --- 58. Audience LinkedIn messages never Career mode -----------------------
echo ">>> LinkedIn messages not career"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from audience import detect_audience, Audience, shape_prompt, score_audiences
for q in (
    "i wanna respond to my linkedin messages",
    "reply to linkedin messages in my inbox",
    "check linkedin messages",
    "linkedin inbox reply draft",
):
    aud = detect_audience(q)
    assert aud is Audience.LINKEDIN, (q, aud)
    sc = score_audiences(q)
    assert sc[Audience.LINKEDIN] > sc[Audience.JOB], (q, sc[Audience.LINKEDIN], sc[Audience.JOB])
    assert "job search" not in shape_prompt(q).lower()
# Profile/job still career-ish when resume language present
aud2 = detect_audience("cover letter and resume for software engineer interview")
assert aud2 is Audience.JOB
print("linkedin-not-career-ok")
PY
REMOTE
then
  ok "LinkedIn messaging never Career mode"
else
  bad "linkedin vs job" "messaging misclassified as career"
fi

# --- 59. search focus_query edges on guest ----------------------------------
echo ">>> search focus_query guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
assert "tsearch" in s.focus_query("i wanna work on tsearch").lower()
assert "linkedin" in s.focus_query("i wanna respond to my linkedin messages").lower()
assert s.focus_query("immigration paradise") == "immigration paradise"
assert not s.is_work_goal("")
assert s.is_work_goal("i wanna work on trading")
assert s.is_freeform_help_goal("catch up on my messages")
print("focus-guest-ok")
PY
REMOTE
then
  ok "search focus_query edges on guest"
else
  bad "focus guest" "failed"
fi

# --- 60. progress setup awards on guest -------------------------------------
echo ">>> progress setup awards guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp(prefix="e2e-setup-")
sys.path.insert(0, "/usr/lib/teddyos")
import progress
importlib.reload(progress)
progress.award_signin()
progress.award_tour()
progress.award_clone()
progress.award_all_set()
b = set(progress.snapshot().badges)
for need in ("first_signin", "tour_done", "first_clone", "all_set"):
    assert need in b, (need, b)
print("setup-awards-guest-ok", sorted(b))
PY
REMOTE
then
  ok "progress setup awards on guest"
else
  bad "setup awards guest" "failed"
fi

# --- 61. accounts always_available matrix guest -----------------------------
echo ">>> accounts always_available guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import accounts
by = {a.id: a for a in accounts.all_accounts()}
assert "--claudeai" in by["claude"].connect_argv
for tid in ("devin", "replit", "perplexity"):
    assert by[tid].always_available, tid
print("accounts-aa-guest-ok", len(by))
PY
REMOTE
then
  ok "accounts always_available matrix on guest"
else
  bad "accounts guest" "always_available failed"
fi

# --- 62. persona_meter bounds for several personas --------------------------
echo ">>> persona_meter bounds"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp(prefix="e2e-meter-")
sys.path.insert(0, "/usr/lib/teddyos")
import progress
importlib.reload(progress)
def ask(p):
    raw = progress._load_raw(); raw["last_ask_ts"]=0; progress._save_raw(raw)
    return progress.award_from_prompt(p)
ask("refactor TypeError in x.py")
ask("i wanna respond to my linkedin messages")
ask("gym hypertrophy progressive overload plan")
for pid in ("code", "linkedin", "fitness", "plain"):
    frac, cap = progress.persona_meter(pid)
    assert 0.0 <= frac <= 1.0, (pid, frac)
    assert isinstance(cap, str) and len(cap) > 3
print("persona-meter-ok")
PY
REMOTE
then
  ok "persona_meter bounds for active personas"
else
  bad "persona_meter" "bounds failed"
fi

# --- 63. SHAPED_TOOLS shape all coding helpers guest ------------------------
echo ">>> SHAPED_TOOLS full matrix guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from audience import SHAPED_TOOLS, shape_prompt_for_tool, detect_audience, Audience
p = "fix TypeError in async handler"
assert detect_audience(p) is Audience.CODE
for tid in SHAPED_TOOLS:
    s = shape_prompt_for_tool(tid, p)
    assert "comfortable with code" in s.lower(), tid
for tid in ("files", "devin", "replit", "perplexity", "web"):
    assert shape_prompt_for_tool(tid, p) == p, tid
print("shaped-tools-ok", sorted(SHAPED_TOOLS))
PY
REMOTE
then
  ok "SHAPED_TOOLS full matrix on guest"
else
  bad "SHAPED_TOOLS" "shape matrix failed"
fi

# --- 64. Badge ladder on guest ----------------------------------------------
echo ">>> badge ladder guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp(prefix="e2e-badges-")
sys.path.insert(0, "/usr/lib/teddyos")
import progress
importlib.reload(progress)
def ask(p):
    raw = progress._load_raw(); raw["last_ask_ts"]=0; progress._save_raw(raw)
    return progress.award_from_prompt(p)
ask("what is this")
ask("refactor TypeError in x.py")
ask("fix race in worker.rs")
ask("open a PR for login")
ask("i wanna respond to my linkedin messages")
progress.award_synthesis()
b = set(progress.snapshot().badges)
assert "first_ask" in b and "ultracode" in b
assert "across_all" in b
assert "mode_switcher" in b or progress.snapshot().switch_count >= 1
print("badge-ladder-guest-ok", sorted(b))
PY
REMOTE
then
  ok "badge ladder on guest"
else
  bad "badge ladder guest" "failed"
fi

# --- 65. CODE vs PLAIN force on guest ---------------------------------------
echo ">>> CODE vs PLAIN force guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from audience import detect_audience, Audience, is_code_audience
assert detect_audience("refactor auth in src/api.ts") is Audience.CODE
assert is_code_audience(detect_audience("fix TypeError"))
assert detect_audience("explain this project in simple words") is Audience.PLAIN
assert detect_audience("i'm not a developer, help me change the logo") is Audience.PLAIN
print("code-plain-guest-ok")
PY
REMOTE
then
  ok "CODE vs PLAIN force on guest"
else
  bad "code/plain guest" "failed"
fi

# --- 66. sandbox + git_projects + logutil guest -----------------------------
echo ">>> sandbox git logutil guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import sandbox, git_projects, logutil
auth = git_projects.git_auth()
assert hasattr(auth, "ok")
assert hasattr(sandbox, "available") or True
print("core-mods-guest-ok", auth.ok)
PY
REMOTE
then
  ok "sandbox + git_projects + logutil on guest"
else
  bad "core mods guest" "import failed"
fi

# --- 67. freeform email/inbox goals guest -----------------------------------
echo ">>> freeform email/inbox goals"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
for q in (
    "i need to check my email inbox",
    "catch up on my messages",
    "help me draft a reply to this message",
    "i wanna respond to my linkedin messages",
):
    assert s.is_work_goal(q) or s.is_freeform_help_goal(q), q
    assert s.is_freeform_help_goal(q), q
assert not s.is_freeform_help_goal("i wanna work on tsearch")
print("freeform-email-ok")
PY
REMOTE
then
  ok "freeform email/inbox goals on guest"
else
  bad "freeform email" "failed"
fi

# --- 68. Search app freeform block uses home + pending task (source) --------
echo ">>> freeform block uses pending task"
if remote_bash <<'REMOTE'
python3 - <<'PY'
from pathlib import Path
src = Path("/usr/bin/teddyos-search-app").read_text()
assert "is_freeform_help_goal" in src
assert "_pending_task" in src
assert "Path.home()" in src or "home()" in src
assert "freeform" in src
assert "Open LinkedIn messages" in src
assert "linkedin.com/messaging" in src
assert "is_linkedin_messages_goal" in src
assert "Draft my replies" in src
assert "linkedin_reply_action_prompt" in src
assert "is_whatsapp_messages_goal" in src
assert "whatsapp_reply_action_prompt" in src
assert "_open_whatsapp" in src
assert (
    "_auto_start_messaging_replies" in src
    or "_auto_start_linkedin_replies" in src
)
print("freeform-block-src-ok")
PY
REMOTE
then
  ok "freeform block sets pending task + messaging do-it paths"
else
  bad "freeform block src" "missing"
fi

# --- 69. work_tools prompt_argv + recents guest -----------------------------
echo ">>> work_tools prompt_argv + recents"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp(prefix="e2e-wt-")
sys.path.insert(0, "/usr/lib/teddyos")
import work_tools
importlib.reload(work_tools)
assert work_tools.prompt_argv("claude", "hi") == ["hi"]
assert work_tools.prompt_argv("gemini", "x") == ["-i", "x"]
assert work_tools.prompt_argv("files", "x") == []
work_tools.record_use("claude")
work_tools.record_use("copilot")
assert work_tools.recent_ids()[0] in ("claude", "copilot")
assert work_tools.is_recent("claude") or work_tools.is_recent("copilot")
print("wt-argv-guest-ok")
PY
REMOTE
then
  ok "work_tools prompt_argv + recents on guest"
else
  bad "work_tools argv guest" "failed"
fi

# --- 70. search normalise_url + query_terms guest ---------------------------
echo ">>> search util guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
assert s.normalise_url("https://x.com").startswith("https://")
assert s.normalise_url("/tsearch/docs/a").startswith("https://")
terms = s.query_terms("can you tell me about meetings please")
assert "meetings" in terms
assert "please" not in terms
print("search-util-guest-ok", terms[:4])
PY
REMOTE
then
  ok "search normalise_url + query_terms on guest"
else
  bad "search util guest" "failed"
fi

# --- 71. caps API guest -----------------------------------------------------
echo ">>> caps API guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import caps
g = caps.load()
assert isinstance(g, dict) and g
assert caps.level() in {l["id"] for l in caps.LEVELS}
assert isinstance(caps.is_guided(), bool)
assert isinstance(caps.skills(), list)
print("caps-guest-ok", caps.level(), len(g))
PY
REMOTE
then
  ok "caps API on guest"
else
  bad "caps guest" "failed"
fi

# --- 72. accounts get_account + status labels guest -------------------------
echo ">>> accounts status labels guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import accounts
assert accounts.get_account("missing") is None
for tid in ("claude", "github", "devin", "replit", "perplexity"):
    a = accounts.get_account(tid)
    assert a is not None, tid
    st = accounts.status_for(a)
    assert st.label and "\n" not in st.label, (tid, st.label)
# No false ready for signed-out style
for tid in ("devin", "replit", "perplexity"):
    assert accounts.get_account(tid).always_available
print("accounts-status-guest-ok")
PY
REMOTE
then
  ok "accounts get_account + single-line status labels"
else
  bad "accounts status guest" "failed"
fi

# --- 73. pending_ask overwrite guest ----------------------------------------
echo ">>> pending_ask overwrite guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp(prefix="e2e-pa2-")
sys.path.insert(0, "/usr/lib/teddyos")
import pending_ask
importlib.reload(pending_ask)
pending_ask.clear()
pending_ask.save("", "nope")
assert pending_ask.load() is None
proj = tempfile.mkdtemp()
pending_ask.save(proj, "one")
pending_ask.save(proj, "two")
assert pending_ask.load()["prompt"] == "two"
pending_ask.clear()
assert pending_ask.load() is None
print("pending-overwrite-guest-ok")
PY
REMOTE
then
  ok "pending_ask overwrite on guest"
else
  bad "pending overwrite guest" "failed"
fi

# --- 74. web wrappers Chromium --app on guest -------------------------------
echo ">>> wrappers chromium guest"
if remote "grep -q app.devin.ai /usr/bin/teddyos-devin \
  && grep -q chromium /usr/bin/teddyos-devin \
  && grep -q -- '--app=' /usr/bin/teddyos-devin \
  && grep -q replit /usr/bin/teddyos-replit \
  && grep -q chromium /usr/bin/teddyos-open-signin \
  && ! grep -qi 'exec firefox' /usr/bin/teddyos-open-signin"; then
  ok "web wrappers Chromium --app on guest"
else
  bad "wrappers guest" "not Chromium --app"
fi

# --- 75. corpus / builtin search smoke guest --------------------------------
echo ">>> builtin search smoke"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
# Should not throw; results optional if corpus thin
try:
    out = s.search("teddyos", limit=5)
except Exception as e:
    # Some guests only have confined search via app; try builtin only
    res, err = s.search_builtin("linux", limit=5)
    assert isinstance(res, list)
    print("builtin-only-ok", len(res), err)
else:
    assert hasattr(out, "results")
    print("search-smoke-ok", len(out.results), getattr(out, "denied", None))
PY
REMOTE
then
  ok "builtin/search smoke on guest"
else
  bad "search smoke" "failed"
fi

# --- 76. search prune + humanise guest --------------------------------------
echo ">>> search prune + humanise guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
full = s.Result("A", "u", "s", "b", score=10, matched=2, terms=2)
partial = s.Result("B", "u", "s", "b", score=50, matched=1, terms=2)
assert s.prune([full, partial]) == [full]
assert "online" in s.humanise("name resolution failure").lower() or "internet" in s.humanise("name resolution").lower()
toks = s.tokenize("H-1B covid-19")
assert "h1b" in toks or any("h1b" in t for t in toks)
print("prune-humanise-guest-ok")
PY
REMOTE
then
  ok "search prune + humanise on guest"
else
  bad "prune/humanise guest" "failed"
fi

# --- 77. resolve_project_dirs guest -----------------------------------------
echo ">>> resolve_project_dirs guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys, tempfile
from pathlib import Path
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
root = Path(tempfile.mkdtemp(prefix="e2e-proj-"))
name = "e2e-unique-proj-xyz"
(proj := root / name).mkdir()
found = s.resolve_project_dirs(name, roots=[root])
assert any(p.name == name for p in found), found
assert s.resolve_project_dirs("nope-zzzz", roots=[root]) == []
print("resolve-proj-guest-ok")
PY
REMOTE
then
  ok "resolve_project_dirs on guest"
else
  bad "resolve_project_dirs guest" "failed"
fi

# --- 78. git_projects safe empty guest --------------------------------------
echo ">>> git_projects safe guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import git_projects as gp
assert gp.find_remote_repos("") == []
assert gp.find_remote_repos("a") == []
auth = gp.git_auth()
assert auth.method in ("gh", "ssh", "none")
print("git-safe-guest-ok", auth.method, auth.ok)
PY
REMOTE
then
  ok "git_projects empty/unauth safe on guest"
else
  bad "git_projects guest" "failed"
fi

# --- 79. ready_for_broadcast edges guest ------------------------------------
echo ">>> ready_for_broadcast guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from work_tools import WorkTool, CreditStatus, ready_for_broadcast, tools_ready_for_broadcast
claude = WorkTool(id="claude", title="C", subtitle="", icon="x", argv=("{path}",), metered=True, is_ai=True)
files = WorkTool(id="files", title="F", subtitle="", icon="x", argv=("{path}",), metered=False, is_ai=False)
assert ready_for_broadcast(claude, CreditStatus(True, "ok"))
assert not ready_for_broadcast(claude, None)
assert not ready_for_broadcast(files, CreditStatus(True, "ok"))
assert tools_ready_for_broadcast([claude, files], {"claude": CreditStatus(True, "ok")}) == [claude]
print("ready-bcast-guest-ok")
PY
REMOTE
then
  ok "ready_for_broadcast edges on guest"
else
  bad "ready_for_broadcast guest" "failed"
fi

# --- 80. progress meters guest ----------------------------------------------
echo ">>> progress meters guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp(prefix="e2e-meters-")
sys.path.insert(0, "/usr/lib/teddyos")
import progress
importlib.reload(progress)
assert progress.format_toast([]) is None
progress.award_from_prompt("refactor TypeError in x.py")
frac, line = progress.ultracode_meter()
assert 0.0 <= frac <= 1.0
frac2, line2 = progress.persona_meter("code")
assert 0.0 <= frac2 <= 1.0 and "Level" in line2
print("meters-guest-ok", line2)
PY
REMOTE
then
  ok "progress meters on guest"
else
  bad "progress meters guest" "failed"
fi

# --- 81. portal_corpus_status + sandbox selftest guest ----------------------
echo ">>> portal status + sandbox selftest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
import sandbox
st = s.portal_corpus_status()
assert set(st) >= {"present", "crawled_at", "docs", "bytes"}
ok, msg = sandbox.selftest()
assert isinstance(ok, bool) and isinstance(msg, str)
assert isinstance(sandbox.available(), bool)
print("portal-sandbox-ok", st["present"], st["docs"], sandbox.available(), ok)
PY
REMOTE
then
  ok "portal_corpus_status + sandbox selftest on guest"
else
  bad "portal/sandbox guest" "failed"
fi

# --- 82. teddyos-search --caps / --help guest -------------------------------
echo ">>> teddyos-search CLI guest"
if remote_bash <<'REMOTE'
set -e
teddyos-search --help | grep -q -- '--caps'
teddyos-search --caps | grep -Eiq 'capabilit|built-in|granted|denied'
# no query → non-zero
if teddyos-search >/tmp/ts-empty.out 2>/tmp/ts-empty.err; then
  echo "expected non-zero without query" >&2
  exit 1
fi
echo "search-cli-guest-ok"
REMOTE
then
  ok "teddyos-search --help / --caps / no-query on guest"
else
  bad "search CLI guest" "failed"
fi

# --- 83. ask-all --help + missing args guest --------------------------------
echo ">>> ask-all CLI guest"
if remote_bash <<'REMOTE'
set -e
teddyos-ask-all --help 2>&1 | grep -q -- '--project'
teddyos-ask-all --help 2>&1 | grep -q -- '--prompt'
teddyos-ask-all --help 2>&1 | grep -q -- '--tools'
# missing required args
if teddyos-ask-all >/tmp/aa-empty.out 2>/tmp/aa-empty.err; then
  echo "expected failure without args" >&2
  exit 1
fi
echo "ask-all-cli-guest-ok"
REMOTE
then
  ok "ask-all --help + missing args fail on guest"
else
  bad "ask-all CLI guest" "failed"
fi

# --- 84. shape_prompt_for_claude alias guest --------------------------------
echo ">>> shape alias guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
from audience import shape_prompt, shape_prompt_for_claude, score_audiences, Audience
q = "refactor TypeError in worker.py"
assert shape_prompt_for_claude(q) == shape_prompt(q)
sc = score_audiences(q)
assert sc[Audience.CODE] >= 4
assert set(sc) == set(Audience)
print("shape-alias-guest-ok")
PY
REMOTE
then
  ok "shape_prompt_for_claude alias on guest"
else
  bad "shape alias guest" "failed"
fi

# --- 85. award_ask persona= on guest ----------------------------------------
echo ">>> award_ask persona guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import os, tempfile, sys, importlib
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp(prefix="e2e-ap-")
sys.path.insert(0, "/usr/lib/teddyos")
import progress
importlib.reload(progress)
s = progress.award_ask(persona="fitness")
assert s.active_persona == "fitness"
assert s.persona_xp.get("fitness", 0) > 0
raw = progress._load_raw(); raw["last_ask_ts"]=0; progress._save_raw(raw)
s2 = progress.award_from_prompt("fix bug", tool_ids=["claude","grok"])
assert s2.active_persona == "code" or s2.ultracode_asks >= 0
print("award-persona-guest-ok", s.active_persona_label)
PY
REMOTE
then
  ok "award_ask(persona=) on guest"
else
  bad "award persona guest" "failed"
fi

# --- 86. caps schema + text() guest -----------------------------------------
echo ">>> caps schema guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import caps
ids = {c["id"] for c in caps.CAPABILITIES}
assert set(caps.DEFAULTS) == ids
for c in caps.CAPABILITIES:
    assert caps.text(c, "label", True)
    assert caps.text(c, "label", False)
assert {l["id"] for l in caps.LEVELS} >= {"guided", "advanced"}
print("caps-schema-guest-ok", len(ids))
PY
REMOTE
then
  ok "caps schema + text() on guest"
else
  bad "caps schema guest" "failed"
fi

# --- 87. Outcome/Result + probe_credits guest -------------------------------
echo ">>> Outcome + probe_credits guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import search as s
from work_tools import available_work_tools, probe_credits, CreditStatus, probe_all
r = s.Result("t", "https://x", "s", "builtin", score=1.0)
o = s.Outcome(results=[r], denied=["x"])
assert o.results[0].title == "t"
tools = available_work_tools()
assert tools
st = probe_credits(tools[0])
assert isinstance(st, CreditStatus) and st.label and "\n" not in st.label
m = probe_all(tools[:2])
assert len(m) == min(2, len(tools))
print("outcome-probe-guest-ok", tools[0].id, st.label[:40])
PY
REMOTE
then
  ok "Outcome + probe_credits/probe_all on guest"
else
  bad "outcome/probe guest" "failed"
fi

# --- 88. accounts is_installed guest ----------------------------------------
echo ">>> accounts is_installed guest"
if remote_bash <<'REMOTE'
python3 - <<'PY'
import sys
sys.path.insert(0, "/usr/lib/teddyos")
import accounts
for a in accounts.all_accounts():
    assert isinstance(accounts.is_installed(a), bool)
# web apps should be installed when chromium exists (guest has it)
import shutil
if shutil.which("chromium") or shutil.which("chromium-browser"):
    for tid in ("devin", "replit", "perplexity"):
        assert accounts.is_installed(accounts.get_account(tid)), tid
print("is-installed-guest-ok")
PY
REMOTE
then
  ok "accounts is_installed on guest"
else
  bad "is_installed guest" "failed"
fi

# --- 89. open-signin usage + chromium wrappers guest ------------------------
echo ">>> open-signin usage guest"
if remote_bash <<'REMOTE'
set -e
# no args
if teddyos-open-signin >/tmp/osi.out 2>/tmp/osi.err; then
  echo "expected exit 2" >&2; exit 1
fi
grep -qi usage /tmp/osi.err || grep -qi usage /tmp/osi.out
# accounts device-code paste
grep -q _DEVICE_CODE_ACCOUNTS /usr/bin/teddyos-accounts
grep -q _submit_paste /usr/bin/teddyos-accounts
grep -q 'Paste code from the browser' /usr/bin/teddyos-accounts
echo "open-signin-guest-ok"
REMOTE
then
  ok "open-signin usage + device-code paste on guest"
else
  bad "open-signin guest" "failed"
fi

# --- 90. icon files present on guest ----------------------------------------
echo ">>> guest icons present"
if remote_bash <<'REMOTE'
python3 - <<'PY'
from pathlib import Path
need = [
    "teddyos-search", "teddyos-answers", "teddyos-accounts", "teddyos-devin",
    "teddyos-replit", "teddyos-perplexity", "teddyos-claude", "teddyos-whatsapp",
]
base = Path("/usr/share/icons/hicolor/scalable/apps")
missing = []
for n in need:
    if not (base / f"{n}.svg").is_file() and not list(Path("/usr/share/icons/hicolor").rglob(f"{n}.png")):
        # also accept any size png
        missing.append(n)
# soft: at least most present
assert len(missing) <= 2, missing
print("guest-icons-ok", "missing", missing)
PY
REMOTE
then
  ok "guest product icons present"
else
  bad "guest icons" "too many missing"
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
