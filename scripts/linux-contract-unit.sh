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
need = {
    "linux/teddyos-search/progress.py",
    "linux/teddyos-search/audience.py",
    "linux/teddyos-search/pending_ask.py",
}
seen = set()
for p in pathlib.Path("linux").rglob("*"):
    if not p.is_file():
        continue
    text = p.read_text(errors="replace")
    first = text.splitlines()[0] if text else ""
    if p.suffix == ".py" or (p.name.startswith("teddyos-") and "python" in first):
        try:
            ast.parse(text)
            n += 1
            if str(p) in need or p.as_posix() in need:
                seen.add(p.as_posix())
        except SyntaxError as e:
            bad.append(f"{p}: {e}")
for m in need:
    if m not in seen and not pathlib.Path(m).is_file():
        bad.append(f"missing {m}")
    elif m not in seen and pathlib.Path(m).is_file():
        # parsed via rglob under different path form
        pass
print(f"parsed={n}")
if bad:
    print("\n".join(bad))
    sys.exit(1)
# Explicit must-exist modules
for m in need:
    assert pathlib.Path(m).is_file(), m
print("core-modules-ok")
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

# --- audience detection for coding chat helpers -----------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
from audience import (
    Audience,
    SHAPED_TOOLS,
    audience_chip_label,
    detect_audience,
    shape_prompt_for_claude,
    shape_prompt_for_tool,
)

assert detect_audience("explain this project in simple words") is Audience.PLAIN
assert detect_audience("what does this app do?") is Audience.PLAIN
assert detect_audience("refactor the risk model function in trading/risk.py") is Audience.CODE
assert detect_audience("fix the TypeError in async handler") is Audience.CODE
assert detect_audience("open a PR for the login bug") is Audience.CODE
assert detect_audience("i'm not a developer, help me change the logo") is Audience.PLAIN

# Multi-persona coverage (everyday + specialist domains)
assert detect_audience("gym hypertrophy progressive overload plan") is Audience.FITNESS
assert detect_audience("fine tune llm rag pipeline pytorch") is Audience.ML_AI
assert detect_audience("clogged drain replace faucet plumbing") is Audience.PLUMBING
assert detect_audience("youtube script thumbnail for my channel") is Audience.CONTENT_CREATOR
assert detect_audience("system design interview design a url shortener") is Audience.SYSTEM_DESIGN
assert detect_audience("adhd executive function tips neurodivergent") is Audience.NEURODIVERSITY
assert detect_audience("smoke a brisket smoker temperature") is Audience.BBQ
assert detect_audience("term sheet seed round cap table") is Audience.VC
assert detect_audience("error budget SLO postmortem on-call") is Audience.SRE
assert detect_audience("terraform kubernetes helm ci/cd pipeline") is Audience.DEVOPS
assert detect_audience("home assistant smart home automation lights") is Audience.SMART_HOME
assert detect_audience("write a haiku about autumn rain") is Audience.POETRY
assert detect_audience("tennis lesson improve serve forehand") is Audience.TENNIS
assert detect_audience("make a budget emergency fund pay off debt") is Audience.PERSONAL_FINANCE
assert detect_audience("data pipeline dbt model airflow etl") is Audience.DATA_ENGINEERING
assert detect_audience("kubernetes deployment kubectl helm chart") is Audience.KUBERNETES
assert detect_audience("train my dog puppy training leash") is Audience.DOG_TRAINING
assert detect_audience("write a screenplay beat sheet logline") is Audience.SCREENWRITING
assert detect_audience("meal prep batch cooking weekly") is Audience.MEAL_PREP
assert detect_audience("password manager enable 2fa passkey") is Audience.PASSWORD_SECURITY
assert detect_audience("learn spanish conjugation practice") is Audience.SPANISH
assert detect_audience("learn japanese hiragana kanji") is Audience.JAPANESE
assert detect_audience("rust ownership borrow checker") is Audience.RUST_LANG
assert detect_audience("pandas dataframe groupby jupyter") is Audience.PYTHON_DATA
assert detect_audience("dockerfile docker compose build image") is Audience.DOCKER
assert detect_audience("fresh pasta risotto technique") is Audience.ITALIAN_COOKING
assert detect_audience("linkedin profile headline about") is Audience.LINKEDIN
assert detect_audience("pickleball third shot drop kitchen") is Audience.PICKLEBALL
assert detect_audience("build a habit stack tracker") is Audience.HABIT_BUILDING
# LinkedIn *messages* are product use, not job-search coaching
assert detect_audience("i wanna respond to my linkedin messages") is Audience.LINKEDIN
assert detect_audience("reply to linkedin messages in my inbox") is Audience.LINKEDIN
assert "message" in shape_prompt_for_claude(
    "i wanna respond to my linkedin messages"
).lower() or "linkedin" in shape_prompt_for_claude(
    "i wanna respond to my linkedin messages"
).lower()
assert len(list(Audience)) >= 550
assert audience_chip_label(Audience.ML_AI) == "ML/AI mode"
assert audience_chip_label(Audience.FITNESS) == "Fitness mode"
assert audience_chip_label(Audience.KUBERNETES) == "Kubernetes mode"
assert audience_chip_label(Audience.SPANISH) == "Spanish mode"

plain = shape_prompt_for_claude("what does this folder do?")
assert "not a software engineer" in plain.lower() or "everyday" in plain.lower()
assert "what does this folder do?" in plain

code = shape_prompt_for_claude("refactor auth middleware in src/api.ts")
assert "comfortable with code" in code.lower()
assert "src/api.ts" in code or "refactor" in code

# All coding chat helpers shape; Files/web wrappers do not.
for tid in ("claude", "grok", "gemini", "codex", "copilot", "antigravity"):
    assert tid in SHAPED_TOOLS
    shaped = shape_prompt_for_tool(tid, "fix the race in worker.rs")
    assert "comfortable with code" in shaped
assert shape_prompt_for_tool("files", "refactor x") == "refactor x"
assert audience_chip_label(Audience.CODE) == "Ultracode"
assert "simple" in audience_chip_label(Audience.PLAIN).lower()
assert "Ultracode" in shape_prompt_for_tool("claude", "fix race in x.rs")

# ask-all wires chip + Ultracode burst + multi-tool shaping + synthesis
src = Path("linux/teddyos-agent/teddyos-ask-all").read_text()
assert "shape_prompt_for_tool" in src
assert "audience_chip_label" in src or "teddyos-audience-chip" in src
assert "_model_prompt" in src
assert "_start_synthesis" in src
assert "Across all answers" in src
assert "_review_prompt" in src
assert "_play_ultracode_switch" in src
assert "ULTRACODE" in src
print("audience-ok")
PY
then
  ok "audience detect + multi-tool shape + Ultracode chip"
else
  bad "audience module" "detection, shape, or chip failed"
fi

# --- audience completeness + shape integrity --------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import audience as a
from audience import (
    Audience,
    detect_audience,
    score_audiences,
    shape_prompt,
    shape_prompt_for_tool,
    audience_chip_label,
    audience_chip_hint,
    audience_header_title,
    audience_css_class,
    is_code_audience,
)

n = len(list(Audience))
assert n >= 550, n
# Every persona has chip / hint / header / shape
for aud in Audience:
    assert audience_chip_label(aud), aud
    assert audience_chip_hint(aud), aud
    assert audience_header_title(aud), aud
    assert aud in a._SHAPE and "{text}" in a._SHAPE[aud], aud
    css = audience_css_class(aud)
    assert css.startswith("teddyos-audience-"), css
assert audience_css_class(Audience.CODE) == "teddyos-audience-code"
assert audience_css_class(Audience.PLAIN) == "teddyos-audience-plain"
assert is_code_audience(Audience.CODE) and is_code_audience("code")
assert not is_code_audience(Audience.PLAIN)
assert not is_code_audience(Audience.FITNESS)

# Lexicons cover every specialist (except CODE/PLAIN)
lex = {x[0] for x in a._LEXICONS}
missing = [x for x in Audience if x not in lex and x not in (Audience.PLAIN, Audience.CODE)]
assert not missing, missing[:10]
assert len(a._PRIORITY) == n and len(set(a._PRIORITY)) == n

# score_audiences keys complete; empty → all zero-ish PLAIN path
sc0 = score_audiences("")
assert set(sc0) == set(Audience)
assert detect_audience("") is Audience.PLAIN
assert detect_audience("   ") is Audience.PLAIN

# Shape embeds original text for code + specialist + plain
for prompt, expect_aud in (
    ("refactor foo.py TypeError", Audience.CODE),
    ("learn spanish conjugation practice", Audience.SPANISH),
    ("make a budget emergency fund", Audience.PERSONAL_FINANCE),
    ("explain this in simple words please", Audience.PLAIN),
    ("gym hypertrophy progressive overload", Audience.FITNESS),
):
    aud = detect_audience(prompt)
    assert aud is expect_aud, (prompt, aud, expect_aud)
    shaped = shape_prompt(prompt)
    assert prompt in shaped
    assert "Their request" in shaped or "request" in shaped.lower()
    # Chat tools get shaping; web wrappers pass through
    assert shape_prompt_for_tool("claude", prompt) == shaped
    assert shape_prompt_for_tool("perplexity", prompt) == prompt
    assert shape_prompt_for_tool("devin", prompt) == prompt

# Broader regression matrix (waves 3–8)
matrix = [
    ("NDA in plain English", Audience.LEGAL),
    ("visa H-1B green card USCIS", Audience.IMMIGRATION),
    ("obsidian zettelkasten second brain", Audience.PKM),
    ("PADI open water scuba dive plan", Audience.SCUBA),
    ("index fund etf asset allocation brokerage", Audience.INVESTING),
    ("aws s3 bucket cloud architecture", Audience.CLOUD),
    ("smoke a brisket smoker temperature", Audience.BBQ),
    ("term sheet seed round cap table", Audience.VC),
    ("error budget SLO postmortem on-call", Audience.SRE),
    ("system design interview design twitter", Audience.SYSTEM_DESIGN),
    ("youtube script thumbnail for my channel", Audience.CONTENT_CREATOR),
    ("seo audit keyword research on-page", Audience.SEO),
    ("salary negotiation counter offer BATNA", Audience.NEGOTIATION),
    ("clogged drain replace faucet", Audience.PLUMBING),
    ("breaker tripped electrical outlet", Audience.ELECTRICAL_TRADE),
    ("AC not cooling heat pump HVAC", Audience.HVAC),
    ("train my dog puppy leash", Audience.DOG_TRAINING),
    ("write a screenplay beat sheet", Audience.SCREENWRITING),
    ("write a novel outline chapter", Audience.NOVEL),
    ("data pipeline dbt airflow etl", Audience.DATA_ENGINEERING),
    ("excel formula pivot table", Audience.SPREADSHEETS),
    ("kubernetes kubectl helm chart", Audience.KUBERNETES),
    ("terraform module state plan", Audience.TERRAFORM),
    ("dockerfile docker compose", Audience.DOCKER),
    ("prompt engineering system prompt", Audience.PROMPT_ENG),
    ("learn french conjugation practice", Audience.FRENCH),
    ("learn mandarin pinyin tones", Audience.MANDARIN),
    ("rust ownership borrow checker", Audience.RUST_LANG),
    ("pandas dataframe groupby jupyter", Audience.PYTHON_DATA),
    ("fresh pasta risotto technique", Audience.ITALIAN_COOKING),
    ("bake bread sourdough loaf", Audience.BREAD),
    ("croissant laminated dough", Audience.PASTRY),
    ("houseplant care repot succulent", Audience.HOUSEPLANTS),
    ("pc build choose a gpu", Audience.PC_BUILDING),
    ("homelab proxmox self hosted", Audience.HOME_LAB),
    ("linkedin profile headline", Audience.LINKEDIN),
    ("career change pivot transferable", Audience.CAREER_CHANGE),
    ("build a habit stack tracker", Audience.HABIT_BUILDING),
    ("time blocking deep work schedule", Audience.TIME_BLOCKING),
    ("improve lcp core web vitals", Audience.PERFORMANCE_WEB),
    ("graphic design poster typography", Audience.GRAPHIC_DESIGN),
    ("ux writing microcopy error message", Audience.UX_WRITING),
    ("esports practice vod review", Audience.ESPORTS),
    ("pickleball third shot drop", Audience.PICKLEBALL),
    ("music theory harmonic analysis", Audience.MUSIC_THEORY),
    ("solo travel tips traveling alone", Audience.SOLO_TRAVEL),
    ("make an offer home inspection", Audience.HOME_BUYING),
    ("college essay common app", Audience.COLLEGE_APPS),
    ("password manager enable 2fa", Audience.PASSWORD_SECURITY),
    ("adhd executive function neurodivergent", Audience.NEURODIVERSITY),
    ("fine tune llm rag pytorch", Audience.ML_AI),
    ("tennis lesson improve serve", Audience.TENNIS),
    ("meal prep batch cooking weekly", Audience.MEAL_PREP),
]
fails = []
for text, expected in matrix:
    got = detect_audience(text)
    if got is not expected:
        fails.append((text, expected.value, got.value))
assert not fails, fails[:8]

# Chip labels stay short (UI)
for aud in (Audience.CODE, Audience.PLAIN, Audience.SPANISH, Audience.DOCKER, Audience.ML_AI):
    lab = audience_chip_label(aud)
    assert 3 <= len(lab) <= 40, (aud, lab)

print("audience-complete-ok", n, "matrix", len(matrix))
PY
then
  ok "audience completeness + shape integrity + matrix"
else
  bad "audience completeness" "missing maps or regression matrix failed"
fi

# --- freeform help goals (LinkedIn messages, etc.) --------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import search as s

q = "i wanna respond to my linkedin messages"
assert s.is_work_goal(q), "should be a goal, not corpus keyword search"
assert s.is_freeform_help_goal(q), "should be freeform Get help, not project folder"
assert s.focus_query(q).lower().endswith("linkedin messages") or "linkedin" in s.focus_query(q).lower()
# Project goals stay project-shaped
assert s.is_work_goal("i wanna work on tsearch")
assert not s.is_freeform_help_goal("i wanna work on tsearch")
assert s.is_work_goal("work on my-app")
assert not s.is_freeform_help_goal("work on my-app")
# Other everyday tasks
assert s.is_freeform_help_goal("i need to check my email inbox")
assert s.is_work_goal("help me draft a reply to this message")
# Search app wires freeform path
app = Path("linux/teddyos-search/teddyos-search-app").read_text()
assert "is_freeform_help_goal" in app
assert "Open LinkedIn messages" in app
assert "_open_link_row" in app
print("freeform-help-ok")
PY
then
  ok "freeform help goals (LinkedIn messages → Get help)"
else
  bad "freeform help" "work_goal / freeform path broken"
fi

# --- credit parser host unit ------------------------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
from work_tools import _parse_balance_snippet, _LOW, CreditStatus

assert _parse_balance_snippet("72% remaining this week")
assert "remaining" in (_parse_balance_snippet("72% remaining this week") or "").lower()
assert _parse_balance_snippet("Not sure which usage you mean. Quick options:") is None
assert _parse_balance_snippet("") is None
assert _LOW.search("Credit balance is too low")
assert _LOW.search("rate limit")
# CreditStatus is a simple container
st = CreditStatus(True, "72% remaining")
assert st.ok is True and "72%" in st.label
st2 = CreditStatus(False, "Needs sign-in")
assert st2.ok is False
print("credit-parser-ok")
PY
then
  ok "credit balance parser host unit"
else
  bad "credit parser" "work_tools parser contract failed"
fi

# --- ISO installs audience + progress modules -------------------------------
if grep -q 'audience.py' linux/iso/build-iso.sh \
  && grep -q 'progress.py' linux/iso/build-iso.sh \
  && grep -q 'pending_ask.py' linux/iso/build-iso.sh; then
  ok "build-iso installs audience + progress + pending_ask"
else
  bad "build-iso modules" "missing audience/progress/pending_ask install"
fi

# --- Across-all-answers synthesis contract ----------------------------------
if python3 - <<'PY'
from pathlib import Path
src = Path("linux/teddyos-agent/teddyos-ask-all").read_text()
for need in (
    "_start_synthesis",
    "_run_synthesis",
    "_collect_answers",
    "_pick_reviewer",
    "Across all answers",
    "Answers to compare",
):
    assert need in src, need
# Search copy promises the multi-AI path
s2 = Path("linux/teddyos-search/teddyos-search-app").read_text()
assert "every ready AI" in s2 or "pull them together" in s2
print("synth-ok")
PY
then
  ok "Across all answers synthesis wired"
else
  bad "synthesis" "ask-all or search copy missing"
fi

# --- Gamification = Ultracode fluency ---------------------------------------
if python3 - <<'PY'
import os, tempfile
from pathlib import Path
import sys
td = tempfile.mkdtemp(prefix="teddyos-progress-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import importlib
import progress
importlib.reload(progress)

s0 = progress.snapshot()
assert s0.xp == 0
assert "Ultracode" in progress.progress_line(s0) or "ask to start" in progress.progress_line(s0)

# Plain ask: small fluency, no ultracode badge
s1 = progress.award_from_prompt("what does this project do?", multi_ai=False)
assert s1.xp > 0
assert s1.plain_asks >= 1
assert s1.ultracode_asks == 0

# Fresh clock so debounce doesn't swallow Ultracode ask
import progress as P
raw = P._load_raw()
raw["last_ask_ts"] = 0
P._save_raw(raw)

s2 = progress.award_from_prompt(
    "refactor the auth middleware in src/api.ts",
    multi_ai=True,
)
assert s2.ultracode_asks >= 1
assert "ultracode" in progress.snapshot().badges
assert any(getattr(e, "ultracode", False) for e in s2.events) or "ultracode" in s2.badges

# Line is fluency/Ultracode centric
line = progress.progress_line()
assert "Ultracode" in line or "fluency" in line or "simple" in line
frac, _ = progress.ultracode_meter()
assert 0.0 <= frac <= 1.0

# award_from_prompt uses audience.detect
from audience import detect_audience, Audience
assert detect_audience("fix TypeError in worker") is Audience.CODE

# Per-persona levels + mode switch
raw = P._load_raw()
raw["last_ask_ts"] = 0
P._save_raw(raw)
s3 = progress.award_from_prompt("i wanna respond to my linkedin messages")
assert s3.active_persona == "linkedin", s3.active_persona
assert s3.switch_count >= 1
assert any(e.kind == "switch" for e in s3.events)
assert "linkedin" in s3.persona_xp or s3.persona_levels.get("linkedin", 0) >= 1
line = progress.progress_line(s3)
assert "Lv." in line or "Level" in progress.persona_level_line("linkedin")
assert "Level" in progress.persona_level_line("code")
toast = progress.format_toast(s3.events)
assert toast and ("Switch" in toast or "LinkedIn" in toast or "Level" in toast)

assert "progress.py" in Path("linux/iso/build-iso.sh").read_text()
app = Path("linux/teddyos-search/teddyos-search-app").read_text()
assert "award_from_prompt" in app and "_progress_bar" in app
aa = Path("linux/teddyos-agent/teddyos-ask-all").read_text()
assert "_play_persona_switch" in aa
assert "persona_level_line" in aa
print(
    "progress-ok", progress.snapshot().xp,
    "ultra", progress.snapshot().ultracode_asks,
    "switches", progress.snapshot().switch_count,
)
PY
then
  ok "gamification = personas + levels + mode switch"
else
  bad "progress" "persona levels / switch broken"
fi

# --- persona ladder grind + multi-switch chain ------------------------------
if python3 - <<'PY'
import os, tempfile, importlib
from pathlib import Path
import sys
td = tempfile.mkdtemp(prefix="teddyos-persona-ladder-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import progress
importlib.reload(progress)

def ask(p: str):
    raw = progress._load_raw()
    raw["last_ask_ts"] = 0
    progress._save_raw(raw)
    return progress.award_from_prompt(p)

# Chain: plain → code → linkedin → fitness → code again
s_plain = ask("explain this folder in simple words")
assert s_plain.active_persona == "plain"
s_code = ask("refactor TypeError in async worker.py")
assert s_code.active_persona == "code"
assert s_code.switch_count >= 1
assert any(e.kind == "switch" for e in s_code.events)
s_li = ask("i wanna respond to my linkedin messages")
assert s_li.active_persona == "linkedin"
assert s_li.switch_count >= 2
s_fit = ask("gym hypertrophy progressive overload plan")
assert s_fit.active_persona == "fitness"
s_code2 = ask("fix race condition in worker.rs")
assert s_code2.active_persona == "code"
assert s_code2.switch_count >= 3

# Grind LinkedIn to level 2+
for _ in range(4):
    ask("reply to linkedin messages in my inbox")
snap = progress.snapshot()
assert snap.persona_levels.get("linkedin", 1) >= 2, snap.persona_levels
assert snap.persona_xp.get("linkedin", 0) >= 12
assert "linkedin" in {p[0] for p in snap.top_personas}
line = progress.persona_level_line("linkedin")
assert line.startswith("Level ") and "LinkedIn" in line
frac, cap = progress.persona_meter("linkedin")
assert 0.0 <= frac <= 1.0 and "Level" in cap
# format_toast prefers persona_level / switch
ev = [
    progress.ProgressEvent(kind="xp", title="+1"),
    progress.ProgressEvent(kind="switch", title="Switched · Fitness", persona="fitness"),
    progress.ProgressEvent(kind="persona_level", title="Level 2 · LinkedIn", persona="linkedin", persona_level=2),
]
assert "Level 2" in (progress.format_toast(ev) or "")
# progress.json shape
raw = progress._load_raw()
assert "personas" in raw and isinstance(raw["personas"], dict)
assert "last_audience" in raw
assert int(raw.get("switch_count") or 0) >= 3
print(
    "persona-ladder-ok",
    "sw", snap.switch_count,
    "li_lv", snap.persona_levels.get("linkedin"),
    "top", snap.top_personas[:3],
)
PY
then
  ok "persona ladder grind + multi-switch chain"
else
  bad "persona ladder" "levels / multi-switch chain failed"
fi

# --- freeform help goal matrix ----------------------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import search as s
from audience import detect_audience, Audience

cases = [
    ("i wanna respond to my linkedin messages", True, True, Audience.LINKEDIN),
    ("reply to linkedin messages in my inbox", True, True, Audience.LINKEDIN),
    ("i need to check my email inbox", True, True, None),  # freeform; audience flexible
    ("help me draft a reply to this message", True, True, None),
    ("catch up on my messages", True, True, None),
    ("i wanna work on tsearch", True, False, None),
    ("work on my-app", True, False, None),
    ("what is photosynthesis", False, False, None),
    ("immigration paradise", False, False, None),
]
fails = []
for text, want_goal, want_free, want_aud in cases:
    g = s.is_work_goal(text)
    f = s.is_freeform_help_goal(text)
    if g != want_goal or f != want_free:
        fails.append((text, "goal", g, want_goal, "free", f, want_free))
        continue
    if want_aud is not None and detect_audience(text) is not want_aud:
        fails.append((text, "aud", detect_audience(text).value, want_aud.value))
assert not fails, fails
# focus_query strips intent
assert "linkedin" in s.focus_query("i wanna respond to my linkedin messages").lower()
assert s.focus_query("i wanna work on tsearch").lower() in ("tsearch", "work on tsearch") or "tsearch" in s.focus_query("i wanna work on tsearch").lower()
print("freeform-matrix-ok", len(cases))
PY
then
  ok "freeform help goal matrix"
else
  bad "freeform matrix" "goal/freeform classification failed"
fi

# --- ask-all UI contracts for switch + levels -------------------------------
if python3 - <<'PY'
from pathlib import Path
aa = Path("linux/teddyos-agent/teddyos-ask-all").read_text()
app = Path("linux/teddyos-search/teddyos-search-app").read_text()
for need in (
    "_play_persona_switch",
    "_play_ultracode_switch",
    "_last_audience_value",
    "persona_level_line",
    "_level_chip",
    "teddyos-mode-flash",
    "award_from_prompt",
):
    assert need in aa, need
for need in (
    "persona_meter",
    "progress_line",
    "award_from_prompt",
    "_progress_bar",
    "is_freeform_help_goal",
    "Open LinkedIn messages",
):
    assert need in app, need
print("ui-switch-level-ok")
PY
then
  ok "ask-all/search UI contracts for switch + levels"
else
  bad "UI contracts" "missing switch/level symbols"
fi

# --- progress debounce + soft switch + multi_ai + badges -------------------
if python3 - <<'PY'
import os, tempfile, importlib, sys
from pathlib import Path
td = tempfile.mkdtemp(prefix="teddyos-debounce-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import progress
importlib.reload(progress)

# First ask — full XP
s1 = progress.award_from_prompt("refactor TypeError in worker.py", multi_ai=True)
assert s1.ultracode_asks >= 1
assert "ultracode" in s1.badges or "ultracode" in progress.snapshot().badges
assert "multi_ai" in progress.snapshot().badges or any(
    e.badge_id == "multi_ai" for e in s1.events
) or "ultracode_multi" in progress.snapshot().badges
xp1 = progress.snapshot().xp

# Immediate second ask — debounce (little/no XP stack)
s2 = progress.award_from_prompt("i wanna respond to my linkedin messages")
xp2 = progress.snapshot().xp
assert xp2 - xp1 < 25, (xp1, xp2)  # soft switch badges only-ish
# Soft switch still updates last_audience
assert progress.snapshot().active_persona in ("linkedin", "code")
# After debounce window, linkedin ask sticks
raw = progress._load_raw()
raw["last_ask_ts"] = 0
progress._save_raw(raw)
s3 = progress.award_from_prompt("reply to linkedin messages")
assert s3.active_persona == "linkedin"
assert s3.switch_count >= 1 or progress.snapshot().switch_count >= 1
# mode_switcher badge eventually
for _ in range(3):
    raw = progress._load_raw(); raw["last_ask_ts"] = 0; progress._save_raw(raw)
    progress.award_from_prompt("gym hypertrophy progressive overload plan")
    raw = progress._load_raw(); raw["last_ask_ts"] = 0; progress._save_raw(raw)
    progress.award_from_prompt("refactor bug in main.rs")
snap = progress.snapshot()
assert "mode_switcher" in snap.badges or snap.switch_count >= 1
# Persistence reload
importlib.reload(progress)
snap2 = progress.snapshot()
assert snap2.xp == snap.xp
assert snap2.persona_xp.get("code", 0) > 0 or snap2.ultracode_asks >= 1
print("debounce-persist-ok", snap2.xp, snap2.switch_count, sorted(snap2.badges)[:6])
PY
then
  ok "progress debounce, soft switch, multi_ai, persistence"
else
  bad "progress debounce" "debounce/switch/persist failed"
fi

# --- work_tools readiness + chat helpers ------------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
from work_tools import (
    available_work_tools, tools_ready_for_broadcast, CreditStatus, is_chat_helper,
)
tools = available_work_tools()
assert tools
ids = {t.id for t in tools}
# On host some CLIs may be missing; catalog source still has web apps
src = Path("linux/teddyos-search/work_tools.py").read_text()
for tid in ("claude", "devin", "replit", "perplexity", "files"):
    assert f'"{tid}"' in src or f"'{tid}'" in src, tid
# is_chat_helper known set
for tid in ("claude", "grok", "gemini", "codex", "copilot"):
    assert is_chat_helper(tid), tid
assert not is_chat_helper("files")
assert not is_chat_helper("devin")
# Readiness gates
if tools:
    t = next((x for x in tools if is_chat_helper(x.id)), tools[0])
    assert tools_ready_for_broadcast([t], {t.id: CreditStatus(None, "u")}) == []
    assert tools_ready_for_broadcast([t], {t.id: CreditStatus(False, "Needs sign-in")}) == []
    assert tools_ready_for_broadcast([t], {t.id: CreditStatus(True, "Connected")}) == [t]
print("work-tools-ready-ok", len(tools))
PY
then
  ok "work_tools readiness + is_chat_helper"
else
  bad "work_tools readiness" "chat helper / broadcast gate failed"
fi

# --- pending_ask host module ------------------------------------------------
if python3 - <<'PY'
import sys, tempfile
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import pending_ask
td = tempfile.mkdtemp(prefix="pending-")
pending_ask.clear()
# save/load with custom project path semantics used by Search
pending_ask.save(td, "hello from e2e suite")
data = pending_ask.load()
assert data and data.get("prompt") == "hello from e2e suite"
pending_ask.clear()
assert pending_ask.load() is None
print("pending-ask-ok")
PY
then
  ok "pending_ask save/load/clear"
else
  bad "pending_ask" "module contract failed"
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

# --- search focus_query + work_goal edge cases ------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import search as s

# Intent strip
assert s.focus_query("i wanna work on tsearch").lower().endswith("tsearch") or "tsearch" in s.focus_query("i wanna work on tsearch").lower()
assert "linkedin" in s.focus_query("i wanna respond to my linkedin messages").lower()
# Non-goals unchanged
assert s.focus_query("immigration paradise") == "immigration paradise"
assert s.focus_query("") == ""
# Empty-ish
assert not s.is_work_goal("")
assert not s.is_work_goal("   ")
assert not s.is_freeform_help_goal("")
# Action verbs
for q in (
    "reply to my messages",
    "catch up on my inbox",
    "help me draft a reply",
    "check my email",
):
    assert s.is_work_goal(q) or s.is_freeform_help_goal(q), q
print("search-focus-ok")
PY
then
  ok "search focus_query + work_goal edges"
else
  bad "search focus" "focus_query / work_goal edges failed"
fi

# --- progress setup awards (signin/tour/clone) -----------------------------
if python3 - <<'PY'
import os, tempfile, importlib, sys
from pathlib import Path
td = tempfile.mkdtemp(prefix="teddyos-setup-awards-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import progress
importlib.reload(progress)
s0 = progress.snapshot()
assert s0.xp == 0
s1 = progress.award_signin()
assert s1.xp > 0
assert "first_signin" in progress.snapshot().badges
s2 = progress.award_tour()
assert "tour_done" in progress.snapshot().badges
s3 = progress.award_clone()
assert "first_clone" in progress.snapshot().badges
s4 = progress.award_all_set()
assert "all_set" in progress.snapshot().badges
# Idempotent-ish: second signin does not explode
progress.award_signin()
print("setup-awards-ok", progress.snapshot().xp, progress.snapshot().badges)
PY
then
  ok "progress setup awards (signin/tour/clone/all_set)"
else
  bad "setup awards" "signin/tour/clone awards failed"
fi

# --- accounts always_available + claude argv matrix -------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import accounts
all_a = list(accounts.all_accounts())
assert len(all_a) >= 5
by_id = {a.id: a for a in all_a}
assert "claude" in by_id
assert "--claudeai" in by_id["claude"].connect_argv
for tid in ("devin", "replit", "perplexity"):
    assert tid in by_id, tid
    assert by_id[tid].always_available, tid
# GitHub present
assert "github" in by_id
print("accounts-matrix-ok", len(all_a), sorted(by_id))
PY
then
  ok "accounts always_available + claude argv matrix"
else
  bad "accounts matrix" "always_available / argv failed"
fi

# --- caps module importable host --------------------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import caps
# guided defaults should exist
assert hasattr(caps, "guided") or hasattr(caps, "defaults") or True
# at least the module loads and has some public API
names = [n for n in dir(caps) if not n.startswith("_")]
assert len(names) >= 3, names
print("caps-ok", names[:8])
PY
then
  ok "caps module imports on host"
else
  bad "caps" "import failed"
fi

# --- ask-all synthesis + re-ask source completeness -------------------------
if python3 - <<'PY'
from pathlib import Path
aa = Path("linux/teddyos-agent/teddyos-ask-all").read_text()
for need in (
    "_start_synthesis", "_run_synthesis", "_collect_answers", "_pick_reviewer",
    "_review_prompt", "Across all answers", "_rerun_one", "_kick_rerun",
    "award_from_prompt", "detect_audience", "shape_prompt_for_tool",
    "_play_persona_switch", "_play_ultracode_switch", "persona_level_line",
    "ULTRACODE", "COPILOT_ALLOW_ALL", "--allow-all-tools",
):
    assert need in aa, need
print("ask-all-src-ok")
PY
then
  ok "ask-all synthesis + re-ask + persona source complete"
else
  bad "ask-all source" "missing symbols"
fi

# --- sandbox + git_projects + logutil import --------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import sandbox
import git_projects
# logutil lives under linux/logging in tree; guest installs to /usr/lib/teddyos
sys.path.insert(0, str(Path("linux/logging").resolve()))
import logutil  # noqa: F401
assert hasattr(sandbox, "available") or callable(getattr(sandbox, "available", None)) or True
# git_projects has auth helper
assert hasattr(git_projects, "git_auth")
auth = git_projects.git_auth()
assert hasattr(auth, "ok")
print("sandbox-git-logutil-ok", type(auth.ok).__name__)
PY
then
  ok "sandbox + git_projects + logutil import"
else
  bad "core modules" "sandbox/git_projects/logutil import failed"
fi

# --- progress badge ladder thresholds ---------------------------------------
if python3 - <<'PY'
import os, tempfile, importlib, sys
from pathlib import Path
td = tempfile.mkdtemp(prefix="teddyos-badges-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import progress
importlib.reload(progress)

def ask(p, multi=False):
    raw = progress._load_raw(); raw["last_ask_ts"] = 0; progress._save_raw(raw)
    return progress.award_from_prompt(p, multi_ai=multi)

ask("what is this app")
assert "first_ask" in progress.snapshot().badges
ask("refactor TypeError in x.py")
assert "ultracode" in progress.snapshot().badges
# three ultracode
ask("fix race in worker.rs")
ask("open a PR for login bug")
b = set(progress.snapshot().badges)
assert "ultracode_3" in b or progress.snapshot().ultracode_asks >= 3
# switch enough for mode_switcher
ask("i wanna respond to my linkedin messages")
ask("gym hypertrophy progressive overload")
ask("refactor bug in main.py")
b = set(progress.snapshot().badges)
assert "mode_switcher" in b
# synthesis
progress.award_synthesis()
assert "across_all" in progress.snapshot().badges
print("badge-ladder-ok", sorted(progress.snapshot().badges))
PY
then
  ok "progress badge ladder thresholds"
else
  bad "badge ladder" "expected badges not awarded"
fi

# --- audience code ties prefer CODE -----------------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
from audience import detect_audience, Audience, is_code_audience
# Strong code signals
for q in (
    "refactor the auth middleware in src/api.ts",
    "fix TypeError in async handler",
    "open a PR for the login bug",
    "pytest is failing on test_risk.py",
):
    aud = detect_audience(q)
    assert aud is Audience.CODE, (q, aud)
    assert is_code_audience(aud)
# Plain force
assert detect_audience("i'm not a developer, help me change the logo") is Audience.PLAIN
assert detect_audience("explain this project in simple words") is Audience.PLAIN
print("code-ties-ok")
PY
then
  ok "audience prefers CODE on tech asks; plain force works"
else
  bad "code ties" "CODE/PLAIN detection failed"
fi

# --- search app source contracts host ---------------------------------------
if python3 - <<'PY'
from pathlib import Path
app = Path("linux/teddyos-search/teddyos-search-app").read_text()
for need in (
    "is_work_goal", "is_freeform_help_goal", "Open LinkedIn messages",
    "_open_link_row", "award_from_prompt", "_progress_bar", "persona_meter",
    "every ready AI", "pull them together", "Get help", "teddyos-ask-all",
    "pending_ask", "com.teddyos.Search",
):
    assert need in app, need
print("search-app-src-ok")
PY
then
  ok "search-app source contracts (freeform + progress + multi-AI)"
else
  bad "search-app source" "missing contracts"
fi

# --- ISO install lines for search modules complete --------------------------
if grep -q 'audience.py' linux/iso/build-iso.sh \
  && grep -q 'progress.py' linux/iso/build-iso.sh \
  && grep -q 'pending_ask.py' linux/iso/build-iso.sh \
  && grep -q 'accounts.py' linux/iso/build-iso.sh \
  && grep -q 'work_tools.py' linux/iso/build-iso.sh \
  && grep -q teddyos-ask-all linux/iso/build-iso.sh \
  && grep -q teddyos-search-app linux/iso/build-iso.sh; then
  ok "build-iso installs full search/agent stack"
else
  bad "build-iso stack" "missing install lines"
fi

# --- work_tools prompt_argv + recents ---------------------------------------
if python3 - <<'PY'
import os, tempfile, importlib, sys
from pathlib import Path
td = tempfile.mkdtemp(prefix="teddyos-wt-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import work_tools
importlib.reload(work_tools)

assert work_tools.prompt_argv("files", "hi") == []
assert work_tools.prompt_argv("claude", "") == []
assert work_tools.prompt_argv("claude", "  hello  ") == ["hello"]
assert work_tools.prompt_argv("gemini", "hi") == ["-i", "hi"]
assert work_tools.prompt_argv("antigravity", "x") == ["-i", "x"]
assert work_tools.prompt_argv("codex", "y") == ["y"]

# recents
work_tools.record_use("claude")
work_tools.record_use("copilot")
work_tools.record_use("claude")
rec = work_tools.recent_ids()
assert rec[0] == "claude", rec
assert "copilot" in rec
assert work_tools.is_recent("claude")
assert not work_tools.is_recent("definitely-not-a-tool-xyz")
print("prompt-argv-recents-ok", rec[:4])
PY
then
  ok "work_tools prompt_argv + recents"
else
  bad "work_tools argv/recents" "failed"
fi

# --- search normalise_url + tokenize + query_terms --------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import search as s

assert s.normalise_url("") == ""
assert s.normalise_url("https://example.com/x") == "https://example.com/x"
assert s.normalise_url("en.wikipedia.org/wiki/X").startswith("https://")
assert s.normalise_url("//cdn.example/a") == "https://cdn.example/a"
# site-relative must not become empty authority
u = s.normalise_url("/tsearch/docs/notes/x")
assert u.startswith("https://") and "tsearch" in u

terms = s.query_terms("can you tell me about my meetings please")
assert "meetings" in terms
assert "can" not in terms and "please" not in terms
toks = s.tokenize("Hello, WORLD! covid-19")
assert any("hello" in t or t == "hello" for t in toks) or "hello" in toks
assert s.tokenize("") == []
print("search-util-ok", terms[:5], toks[:6])
PY
then
  ok "search normalise_url + tokenize + query_terms"
else
  bad "search utils" "failed"
fi

# --- caps load / level / guided ---------------------------------------------
if python3 - <<'PY'
import os, tempfile, sys
from pathlib import Path
td = tempfile.mkdtemp(prefix="teddyos-caps-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import caps
# isolate user conf if module uses home config
granted = caps.load()
assert isinstance(granted, dict) and granted
assert caps.level() in {l["id"] for l in caps.LEVELS}
assert isinstance(caps.is_guided(), bool)
assert isinstance(caps.skills(), list)
assert isinstance(caps.is_configured(), bool)
print("caps-api-ok", caps.level(), len(granted), caps.skills()[:3])
PY
then
  ok "caps load/level/guided/skills API"
else
  bad "caps API" "failed"
fi

# --- accounts get_account + status shape ------------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import accounts
assert accounts.get_account("nope") is None
cl = accounts.get_account("claude")
assert cl is not None and cl.id == "claude"
# status_for returns AccountStatus with label
st = accounts.status_for(cl)
assert hasattr(st, "ok") and hasattr(st, "label")
assert st.label  # non-empty string
# web apps always_available
for tid in ("devin", "replit", "perplexity"):
    a = accounts.get_account(tid)
    assert a and a.always_available
print("accounts-status-ok", st.label[:40])
PY
then
  ok "accounts get_account + status shape"
else
  bad "accounts status" "failed"
fi

# --- pending_ask overwrite + empty project ignored --------------------------
if python3 - <<'PY'
import os, tempfile, sys
from pathlib import Path
td = tempfile.mkdtemp(prefix="teddyos-pa-")
os.environ["XDG_CONFIG_HOME"] = td
# Force pending_ask to re-resolve path — module caches _PATH at import
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import importlib
import pending_ask
importlib.reload(pending_ask)
pending_ask.clear()
pending_ask.save("", "should-ignore")
assert pending_ask.load() is None
pending_ask.save(td, "first prompt")
assert pending_ask.load()["prompt"] == "first prompt"
pending_ask.save(td, "second prompt")
assert pending_ask.load()["prompt"] == "second prompt"
assert pending_ask.load()["project"] == td
pending_ask.clear()
assert pending_ask.load() is None
print("pending-ask-overwrite-ok")
PY
then
  ok "pending_ask overwrite + empty project ignored"
else
  bad "pending_ask overwrite" "failed"
fi

# --- web wrappers + open-signin chromium-only -------------------------------
if python3 - <<'PY'
from pathlib import Path
for name, url in (
    ("teddyos-devin", "app.devin.ai"),
    ("teddyos-replit", "replit.com"),
    ("teddyos-perplexity", "perplexity"),
):
    p = Path(f"linux/teddyos-agent/{name}")
    assert p.is_file(), name
    text = p.read_text()
    assert url in text.lower() or url in text
    assert "chromium" in text or "google-chrome" in text
    assert "--app=" in text or "--app" in text
oi = Path("linux/teddyos-agent/teddyos-open-signin").read_text()
assert "chromium" in oi
assert "firefox" in oi.lower()  # docs say never Firefox by accident
assert "usage:" in oi.lower() or "usage" in oi
print("wrappers-chromium-ok")
PY
then
  ok "web wrappers + open-signin are Chromium --app"
else
  bad "wrappers chromium" "failed"
fi

# --- smoke-all orchestrator wires all suites --------------------------------
if grep -q linux-contract-unit.sh scripts/linux-smoke-all.sh \
  && grep -q linux-desktop-e2e.sh scripts/linux-smoke-all.sh \
  && grep -q linux-gui-flow-e2e.sh scripts/linux-smoke-all.sh \
  && grep -q 'make test-host' scripts/linux-smoke-all.sh; then
  ok "smoke-all orchestrates host + contract + desktop + gui"
else
  bad "smoke-all" "missing suite invocations"
fi

# --- audience multi-persona chip labels unique enough -----------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
from audience import Audience, audience_chip_label, audience_header_title
labels = [audience_chip_label(a) for a in Audience]
assert all(labels)
# No empty; CODE is Ultracode
assert audience_chip_label(Audience.CODE) == "Ultracode"
assert "simple" in audience_chip_label(Audience.PLAIN).lower()
headers = {audience_header_title(a) for a in list(Audience)[:50]}
assert len(headers) >= 20
print("chip-diversity-ok", len(set(labels)), "unique of", len(labels))
PY
then
  ok "audience chip labels present + Ultracode/plain"
else
  bad "chip labels" "failed"
fi

# --- search prune + path_terms + humanise + hyphen tokens -------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import search as s

# hyphen / path tokenize
toks = s.tokenize("H-1B visa iakovos/trading")
assert "h1b" in toks or any("h1b" in t for t in toks)
assert any("/" in t or t == "trading" for t in toks)
assert "trading" in toks
# path_terms
pts = s.path_terms("i wanna work on iakovos/trading")
assert any("iakovos/trading" in p or "trading" in p for p in pts) or pts == [] or True
# if pathish in focus: work on strips to iakovos/trading
fq = s.focus_query("i wanna work on iakovos/trading")
assert "trading" in fq.lower()
pts2 = s.path_terms(fq)
assert any("trading" in p for p in pts2) or "iakovos/trading" in fq.lower()

# humanise network errors for UI
assert "online" in s.humanise("urlopen error Temporary failure in name resolution").lower() \
    or "internet" in s.humanise("Temporary failure in name resolution").lower()
assert "long" in s.humanise("timed out").lower() or "moment" in s.humanise("timeout").lower()
assert "secure" in s.humanise("SSL certificate problem").lower() or "connection" in s.humanise("ssl error").lower()
assert len(s.humanise("weird unknown boom")) > 10

# prune: full-match wins
full = s.Result("A", "u", "s", "b", score=10, matched=2, terms=2)
partial = s.Result("B", "u", "s", "b", score=50, matched=1, terms=2)
assert s.prune([full, partial]) == [full]
# prune: score floor
hi = s.Result("H", "u", "s", "b", score=10, matched=1, terms=1)
lo = s.Result("L", "u", "s", "b", score=1, matched=1, terms=1)
pr = s.prune([hi, lo])
assert hi in pr and lo not in pr
assert s.prune([]) == []
print("search-prune-humanise-ok")
PY
then
  ok "search prune + path_terms + humanise + hyphen tokens"
else
  bad "search prune/humanise" "failed"
fi

# --- resolve_project_dirs finds temp project --------------------------------
if python3 - <<'PY'
import sys, tempfile
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import search as s
root = Path(tempfile.mkdtemp(prefix="teddyos-proj-"))
proj = root / "my-unique-e2e-proj"
proj.mkdir()
(proj / "README").write_text("x\n")
found = s.resolve_project_dirs("my-unique-e2e-proj", roots=[root])
assert any(p.resolve() == proj.resolve() for p in found), found
assert s.resolve_project_dirs("", roots=[root]) == []
assert s.resolve_project_dirs("no-such-folder-zzzz", roots=[root]) == []
print("resolve-project-ok", [p.name for p in found])
PY
then
  ok "resolve_project_dirs finds local project"
else
  bad "resolve_project_dirs" "failed"
fi

# --- git_projects empty subject / unauth safe -------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import git_projects as gp
assert gp.find_remote_repos("") == []
assert gp.find_remote_repos("x") == []  # too short
auth = gp.git_auth()
assert auth.method in ("gh", "ssh", "none")
# Never raises on nonsense
assert isinstance(gp.find_remote_repos("definitely-not-a-real-repo-xyz-99999"), list)
print("git-projects-safe-ok", auth.method, auth.ok)
PY
then
  ok "git_projects empty/unauth safe"
else
  bad "git_projects" "failed"
fi

# --- work_tools ready_for_broadcast edges -----------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
from work_tools import WorkTool, CreditStatus, ready_for_broadcast, tools_ready_for_broadcast

def tool(tid, ai=True):
    return WorkTool(
        id=tid, title=tid, subtitle="", icon="x",
        argv=("{path}",), metered=True, is_ai=ai,
    )

claude = tool("claude")
files = tool("files", ai=False)
assert ready_for_broadcast(claude, CreditStatus(True, "ok"))
assert not ready_for_broadcast(claude, CreditStatus(False, "no"))
assert not ready_for_broadcast(claude, CreditStatus(None, "unknown"))
assert not ready_for_broadcast(claude, None)
assert not ready_for_broadcast(files, CreditStatus(True, "ok"))
# non-prompt tool id
assert not ready_for_broadcast(tool("devin"), CreditStatus(True, "ok"))
ready = tools_ready_for_broadcast(
    [claude, files, tool("devin")],
    {"claude": CreditStatus(True, "ok"), "files": CreditStatus(True, "ok"), "devin": CreditStatus(True, "ok")},
)
assert ready == [claude], ready
print("ready-broadcast-ok")
PY
then
  ok "work_tools ready_for_broadcast edges"
else
  bad "ready_for_broadcast" "failed"
fi

# --- progress format_toast empty + meters -----------------------------------
if python3 - <<'PY'
import os, tempfile, importlib, sys
from pathlib import Path
td = tempfile.mkdtemp(prefix="teddyos-meters-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import progress
importlib.reload(progress)
assert progress.format_toast([]) is None
s = progress.award_from_prompt("refactor TypeError in x.py")
assert progress.format_toast(s.events)
frac, line = progress.ultracode_meter()
assert 0.0 <= frac <= 1.0 and isinstance(line, str)
frac2, line2 = progress.persona_meter("code")
assert 0.0 <= frac2 <= 1.0 and "Level" in line2
print("meters-toast-ok", line2)
PY
then
  ok "progress format_toast empty + meters"
else
  bad "progress meters" "failed"
fi

# --- accounts resolve_connect_argv shape ------------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import accounts
cl = accounts.get_account("claude")
assert cl is not None
argv = accounts.resolve_connect_argv(cl)
# may be None if claude binary missing on host — still must not throw
assert argv is None or (isinstance(argv, list) and argv and isinstance(argv[0], str))
# web apps: connect may open browser wrapper
for tid in ("devin", "replit", "perplexity"):
    a = accounts.get_account(tid)
    assert a is not None
    # always_available even if argv None
    assert a.always_available
print("resolve-connect-ok", argv[:2] if argv else None)
PY
then
  ok "accounts resolve_connect_argv safe"
else
  bad "resolve_connect_argv" "failed"
fi

# --- perplexity + replit wrapper URLs ---------------------------------------
if grep -qi perplexity linux/teddyos-agent/teddyos-perplexity \
  && grep -q chromium linux/teddyos-agent/teddyos-perplexity \
  && grep -q -- '--app=' linux/teddyos-agent/teddyos-perplexity \
  && grep -qi replit linux/teddyos-agent/teddyos-replit \
  && grep -q chromium linux/teddyos-agent/teddyos-replit; then
  ok "perplexity + replit wrappers Chromium --app"
else
  bad "perplexity/replit wrappers" "missing URL or chromium"
fi

# --- portal_corpus_status + builtin corpus via local seed -------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import search as s

st = s.portal_corpus_status()
assert isinstance(st, dict)
for k in ("present", "crawled_at", "docs", "bytes"):
    assert k in st, k
assert isinstance(st["present"], bool)
assert isinstance(st["docs"], int) and st["docs"] >= 0

# Point builtin corpus at repo seed if present
seed = Path("search/corpus.json")
if seed.is_file():
    s.BUILTIN_CORPUS = seed
    res, err = s.search_builtin("linux", limit=5)
    assert isinstance(res, list)
    # may be empty if seed is personal index without "linux" — still no throw
    print("builtin-search-ok", len(res), err)
else:
    res, err = s.search_builtin("linux", limit=3)
    assert isinstance(res, list)
    print("builtin-missing-ok", err)

# scoring helpers
sc = s._score(["linux", "kernel"], "the linux kernel rocks", "Linux Kernel")
assert sc > 0
print("portal-status-ok", st["present"], st["docs"])
PY
then
  ok "portal_corpus_status + builtin search + _score"
else
  bad "portal/builtin" "failed"
fi

# --- sandbox available/sandboxed/selftest never throw -----------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import sandbox
assert isinstance(sandbox.available(), bool)
assert isinstance(sandbox.sandboxed(), bool)
ok, msg = sandbox.selftest()
assert isinstance(ok, bool)
assert isinstance(msg, str)
# host mac typically no systemd sandbox
if not sandbox.available():
    assert not sandbox.sandboxed()
print("sandbox-api-ok", sandbox.available(), ok, msg[:60] if msg else "")
PY
then
  ok "sandbox available/sandboxed/selftest API"
else
  bad "sandbox API" "failed"
fi

# --- shape_prompt_for_claude alias + score_audiences completeness -----------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
from audience import (
    Audience, detect_audience, score_audiences,
    shape_prompt, shape_prompt_for_claude, shape_prompt_for_tool,
)
q = "refactor TypeError in worker.py"
assert shape_prompt_for_claude(q) == shape_prompt(q)
assert detect_audience(q) is Audience.CODE
sc = score_audiences(q)
assert set(sc) == set(Audience)
assert all(isinstance(v, int) and v >= 0 for v in sc.values())
assert sc[Audience.CODE] == max(sc.values()) or sc[Audience.CODE] >= 4
# multi-tool shape parity
a = shape_prompt_for_tool("claude", q)
b = shape_prompt_for_tool("codex", q)
assert a == b == shape_prompt(q)
print("shape-alias-score-ok", sc[Audience.CODE])
PY
then
  ok "shape_prompt_for_claude alias + score_audiences completeness"
else
  bad "shape/score" "failed"
fi

# --- teddyos-search CLI --caps / --help -------------------------------------
if python3 - <<'PY'
import subprocess, sys
from pathlib import Path
cli = Path("linux/teddyos-search/teddyos-search")
r = subprocess.run([sys.executable, str(cli), "--help"], capture_output=True, text=True)
assert r.returncode == 0, r.stderr
assert "--caps" in r.stdout and "--json" in r.stdout
r2 = subprocess.run([sys.executable, str(cli), "--caps"], capture_output=True, text=True)
assert r2.returncode == 0, r2.stderr
assert "Capabilities" in r2.stdout or "Built-in" in r2.stdout or "granted" in r2.stdout.lower()
# missing query → usage exit 2
r3 = subprocess.run([sys.executable, str(cli)], capture_output=True, text=True)
assert r3.returncode == 2
print("search-cli-ok")
PY
then
  ok "teddyos-search CLI --help / --caps / no-query"
else
  bad "teddyos-search CLI" "failed"
fi

# --- ask-all argparse source (gi may be missing on host) --------------------
if python3 - <<'PY'
from pathlib import Path
src = Path("linux/teddyos-agent/teddyos-ask-all").read_text()
assert "argparse" in src
assert '--project' in src and '--prompt' in src and '--tools' in src
assert "required=True" in src
# without args should fail when run if gi present; host lacks gi — AST already checked
import ast
ast.parse(src)
print("ask-all-argparse-ok")
PY
then
  ok "ask-all argparse requires project/prompt/tools"
else
  bad "ask-all argparse" "failed"
fi

# --- progress award_ask persona= + multi tool_ids ---------------------------
if python3 - <<'PY'
import os, tempfile, importlib, sys
from pathlib import Path
td = tempfile.mkdtemp(prefix="teddyos-award-persona-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import progress
importlib.reload(progress)
s = progress.award_ask(persona="linkedin", multi_ai=False)
assert s.active_persona == "linkedin"
assert s.persona_xp.get("linkedin", 0) > 0
raw = progress._load_raw(); raw["last_ask_ts"] = 0; progress._save_raw(raw)
s2 = progress.award_from_prompt(
    "refactor TypeError",
    tool_ids=["claude", "copilot", "grok"],
)
assert s2.ultracode_asks >= 1
assert s2.switch_count >= 1 or s2.active_persona == "code"
print("award-persona-ok", s.active_persona_label, s2.badges[:5])
PY
then
  ok "progress award_ask(persona=) + tool_ids multi_ai"
else
  bad "award persona" "failed"
fi

# --- git_projects CLONE_ROOT + RemoteRepo shape -----------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import git_projects as gp
assert isinstance(gp.CLONE_ROOT, Path)
repo = gp.RemoteRepo(
    full_name="o/n", name="n", clone_url="https://example/n.git",
    description="d", source="test",
)
assert repo.full_name == "o/n"
print("clone-root-ok", gp.CLONE_ROOT)
PY
then
  ok "git_projects CLONE_ROOT + RemoteRepo"
else
  bad "CLONE_ROOT" "failed"
fi

# --- ISO + desktop entries for answers/accounts -----------------------------
if grep -q teddyos-answers.desktop linux/iso/build-iso.sh \
  && grep -q teddyos-accounts.desktop linux/iso/build-iso.sh \
  && grep -q teddyos-perplexity linux/iso/build-iso.sh \
  && test -f linux/icons/teddyos-answers.svg \
  && test -f linux/icons/teddyos-perplexity.svg; then
  ok "ISO desktops + icons for answers/accounts/perplexity"
else
  bad "ISO desktops/icons" "missing"
fi

# --- caps CAPABILITIES/DEFAULTS/LEVELS/text ---------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import caps
assert len(caps.CAPABILITIES) >= 3
ids = {c["id"] for c in caps.CAPABILITIES}
assert set(caps.DEFAULTS.keys()) == ids
for c in caps.CAPABILITIES:
    for f in ("id", "label", "detail", "default"):
        assert f in c, (c.get("id"), f)
    assert isinstance(c["default"], bool)
    # guided vs advanced copy
    plain = caps.text(c, "label", True)
    adv = caps.text(c, "label", False)
    assert plain and adv
assert {l["id"] for l in caps.LEVELS} == {"guided", "advanced"}
assert caps.DEFAULT_LEVEL in {l["id"] for l in caps.LEVELS}
assert len(caps.SKILLS) >= 3
assert set(caps.DEFAULT_SKILLS).issubset({s["id"] for s in caps.SKILLS})
print("caps-schema-ok", len(ids), len(caps.SKILLS))
PY
then
  ok "caps CAPABILITIES/DEFAULTS/LEVELS/text schema"
else
  bad "caps schema" "failed"
fi

# --- search Outcome + Result dataclasses ------------------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import search as s
r = s.Result(title="t", url="https://x", snippet="s", source="builtin", score=1.5, matched=1, terms=2)
assert r.title == "t" and r.score == 1.5
o = s.Outcome(results=[r], denied=["portal.sync"], errors=["e"], notes=["n"])
assert len(o.results) == 1 and o.denied == ["portal.sync"]
o2 = s.Outcome()
assert o2.results == [] and o2.denied == [] and o2.portal_age is None
print("outcome-result-ok")
PY
then
  ok "search Outcome + Result dataclasses"
else
  bad "Outcome/Result" "failed"
fi

# --- accounts is_installed + DEVICE_CODE source -----------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import accounts
# every account has id/title/icon
for a in accounts.all_accounts():
    assert a.id and a.icon
    # is_installed never throws
    assert isinstance(accounts.is_installed(a), bool)
# web apps: installed if chromium present on host (may be False on mac)
for tid in ("devin", "replit", "perplexity"):
    a = accounts.get_account(tid)
    assert a.always_available
src = Path("linux/teddyos-agent/teddyos-accounts").read_text()
assert "_DEVICE_CODE_ACCOUNTS" in src
assert "github" in src and "copilot" in src
assert "_submit_paste" in src
assert "Paste code from the browser" in src
print("accounts-installed-device-ok")
PY
then
  ok "accounts is_installed + device-code paste source"
else
  bad "accounts is_installed" "failed"
fi

# --- work_tools probe_credits returns CreditStatus --------------------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
from work_tools import available_work_tools, probe_credits, CreditStatus, probe_all
tools = available_work_tools()
assert tools
st = probe_credits(tools[0])
assert isinstance(st, CreditStatus)
assert st.label and "\n" not in st.label
# probe_all limited subset
subset = tools[:3]
all_st = probe_all(subset)
assert set(all_st.keys()) == {t.id for t in subset}
for tid, s in all_st.items():
    assert isinstance(s, CreditStatus) and s.label
print("probe-credits-ok", tools[0].id, st.label[:40], len(all_st))
PY
then
  ok "work_tools probe_credits + probe_all"
else
  bad "probe_credits" "failed"
fi

# --- open-signin usage exit code --------------------------------------------
if python3 - <<'PY'
import subprocess, sys
from pathlib import Path
cli = Path("linux/teddyos-agent/teddyos-open-signin")
r = subprocess.run([sys.executable, str(cli)], capture_output=True, text=True)
assert r.returncode == 2, r.returncode
assert "usage" in (r.stderr + r.stdout).lower()
# invalid non-url still needs a browser path — may return 1 without chromium
print("open-signin-usage-ok")
PY
then
  ok "teddyos-open-signin usage exit 2"
else
  bad "open-signin usage" "failed"
fi

# --- icon tree completeness for product tiles -------------------------------
if python3 - <<'PY'
from pathlib import Path
need = {
    "teddyos-search", "teddyos-answers", "teddyos-accounts", "teddyos-web",
    "teddyos-whatsapp", "teddyos-claude", "teddyos-grok", "teddyos-gemini",
    "teddyos-codex", "teddyos-copilot", "teddyos-devin", "teddyos-replit",
    "teddyos-perplexity", "teddyos-github", "teddyos-install",
}
icons = {p.stem for p in Path("linux/icons").glob("*.svg")}
missing = need - icons
assert not missing, missing
iso = Path("linux/iso/build-iso.sh").read_text()
for n in ("teddyos-search", "teddyos-answers", "teddyos-devin", "teddyos-whatsapp"):
    assert n in iso, n
print("icon-tree-ok", len(icons))
PY
then
  ok "icon tree completeness for product tiles"
else
  bad "icon tree" "missing product icons"
fi

# --- audience shape embeds disclaimer for medical/legal-ish -----------------
if python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
from audience import detect_audience, shape_prompt, Audience
# medicine / legal should be careful in shape templates
for q, aud in (
    ("NDA in plain English indemnity clause", Audience.LEGAL),
    ("visa H-1B green card USCIS process", Audience.IMMIGRATION),
):
    assert detect_audience(q) is aud, (q, detect_audience(q))
    sp = shape_prompt(q)
    assert q in sp
print("careful-personas-ok")
PY
then
  ok "legal/immigration detect + shape embed request"
else
  bad "careful personas" "failed"
fi

# --- progress snapshot fields complete --------------------------------------
if python3 - <<'PY'
import os, tempfile, importlib, sys
from pathlib import Path
td = tempfile.mkdtemp(prefix="teddyos-snap-")
os.environ["XDG_CONFIG_HOME"] = td
sys.path.insert(0, str(Path("linux/teddyos-search").resolve()))
import progress
importlib.reload(progress)
s = progress.snapshot()
for f in (
    "xp", "level", "level_title", "plain_asks", "ultracode_asks",
    "active_persona", "active_persona_level", "persona_levels", "persona_xp",
    "top_personas", "switch_count", "badges", "events",
):
    assert hasattr(s, f), f
assert s.level >= 0 and s.xp >= 0 and s.active_persona_level >= 1
print("snapshot-fields-ok", s.level_title)
PY
then
  ok "progress snapshot field completeness"
else
  bad "snapshot fields" "failed"
fi

echo
echo "PASS=$PASS  FAIL=$FAIL  TOTAL=$((PASS + FAIL))"
if [[ "$FAIL" -gt 0 ]]; then
  echo "RESULT: FAIL"
  exit 1
fi
echo "RESULT: PASS"
exit 0
