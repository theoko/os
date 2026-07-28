#!/usr/bin/env python3
"""Build `search/corpus.json` — everything the standalone guest can find.

The guest has no bridge and no network, so search can only ever answer from
what was compiled into the kernel. Until now that was `search/seed.json`
alone: sixteen documents about the OS itself. Every question about the owner's
own work came back "No offline hits for that query.", which reads as a broken
search and is really an empty shelf.

This merges the workspace index into that shelf:

    search/seed.json                       curated docs about the OS
    ~/Library/Application Support/os/       the workspace crawl (320 entries)
      knowledge/workspace.json
                    |
                    v
    search/corpus.json  ->  kernel/build.rs  ->  a static index in the kernel

Three things have to happen on the way, and none of them are cosmetic:

  * **ASCII.** The kernel's font atlas covers 0x20..=0x7E and nothing else
    (`font.rs`), and the tokenizer only ever emits ASCII alphanumeric runs
    (`search.rs::tokenize`). 140 of the 320 titles are not ASCII — mostly
    Greek, plus typographic dashes and the odd emoji. Passed through, a Greek
    title draws as holes and indexes as nothing: the document is baked in and
    unreachable. So Greek is transliterated (searchable by its Latin
    spelling), typographic punctuation is folded to its ASCII twin, and what
    is left over is dropped.

  * **Fit.** A result row copies into fixed slots and `copy_into` truncates in
    silence. A cut title still reads as a title, and two paths that agree for
    the first N bytes silently become one document. Titles are cut here, on
    purpose, at a word boundary and then de-duplicated; URLs are never cut —
    `build.rs` fails the build if one would be.

  * **Category.** `Doc.cat` is what the row shows instead of a URL, so it is
    the top-level directory: droplet, trading, family, content.

    ./scripts/bake-corpus.py            # rebuild search/corpus.json
    ./scripts/bake-corpus.py --check    # fail if it is out of date, change nothing

Rebuild the ISO afterwards — the index is compiled in, so an unrebuilt image
keeps answering from the old shelf.
"""

import argparse
import json
import pathlib
import re
import sys
import unicodedata

REPO = pathlib.Path(__file__).resolve().parent.parent
SEED = REPO / "search" / "seed.json"
CORPUS = REPO / "search" / "corpus.json"
WORKSPACE = (
    pathlib.Path.home()
    / "Library"
    / "Application Support"
    / "os"
    / "knowledge"
    / "workspace.json"
)

# Must match `Row::title` in kernel/src/searchui.rs. Titles are cut to fit;
# URLs are not (see URL_SLOT), because a cut path is a different document.
TITLE_SLOT = 56
URL_SLOT = 128

# How much of a snippet to index. The whole thing would let one long file
# dominate the vocabulary; the scorer already normalises by length, but the
# baked postings table grows with every distinct token and the ISO has to stay
# small enough to boot from a virtual CD.
SNIPPET_MAX = 320

# Generated files that say nothing about the work. Left in, they are not
# merely noise: the scorer divides term frequency by document length, so a
# four-word `top_level.txt` scores a term far higher than a real document that
# discusses it at length, and the junk floats to the top of the results.
SKIP_DIRS = {".egg-info", "node_modules", "__pycache__", ".venv", "venv", "dist", "build", ".git"}
SKIP_NAMES = {
    "SOURCES.txt", "top_level.txt", "dependency_links.txt", "entry_points.txt",
    "PKG-INFO", "requires.txt", "package-lock.json", "poetry.lock", "Cargo.lock",
}

# Fewest indexed tokens a workspace document may carry. Below this there is not
# enough text to rank honestly — see the length-normalisation note above.
MIN_TOKENS = 12


def is_noise(path: str) -> bool:
    parts = path.split("/")
    if parts[-1] in SKIP_NAMES:
        return True
    if re.fullmatch(r"requirements[-\w]*\.txt", parts[-1]):
        return True
    return any(d in SKIP_DIRS or d.endswith(".egg-info") for d in parts[:-1])


def token_count(*fields: str) -> int:
    """Tokens as `search.rs::tokenize` counts them — the title twice, as the
    scorer weights it."""
    return sum(len([t for t in re.findall(r"[A-Za-z0-9]+", f) if len(t) >= 2]) for f in fields)

# ELOT 743-ish. Not a standard-compliant romanisation — a searchable one.
# Accents are stripped first (NFD), so only bare letters need an entry.
GREEK = {
    "α": "a", "β": "v", "γ": "g", "δ": "d", "ε": "e", "ζ": "z", "η": "i",
    "θ": "th", "ι": "i", "κ": "k", "λ": "l", "μ": "m", "ν": "n", "ξ": "x",
    "ο": "o", "π": "p", "ρ": "r", "σ": "s", "ς": "s", "τ": "t", "υ": "y",
    "φ": "f", "χ": "ch", "ψ": "ps", "ω": "o",
}

# Typographic characters that carry meaning worth keeping as ASCII.
PUNCT = {
    "—": " - ", "–": " - ", "―": " - ", "‑": "-",
    "“": '"', "”": '"', "„": '"', "‘": "'", "’": "'",
    "→": " -> ", "←": " <- ", "·": " - ", "•": " - ", "…": "...",
    "×": "x", "≈": "~", "≤": "<=", "≥": ">=", " ": " ",
}


def to_ascii(s: str) -> str:
    """Fold `s` to printable ASCII, keeping as much meaning as survives."""
    out = []
    for ch in s:
        if ch in PUNCT:
            out.append(PUNCT[ch])
            continue
        if " " <= ch <= "~":
            out.append(ch)
            continue
        if ch in "\t\r\n":
            out.append(" ")
            continue
        # Strip the accent, then romanise a Greek letter, then give up on it.
        base = unicodedata.normalize("NFD", ch)
        base = "".join(c for c in base if not unicodedata.combining(c))
        for b in base:
            lower = b.lower()
            if lower in GREEK:
                latin = GREEK[lower]
                out.append(latin.upper() if b.isupper() else latin)
            elif " " <= b <= "~":
                out.append(b)
            # Anything else — emoji, CJK, symbols with no ASCII sense — is
            # dropped rather than turned into a '?' that would index as noise.
    return re.sub(r"\s+", " ", "".join(out)).strip()


def fit_title(title: str, fallback: str) -> str:
    """Cut a title to the row's slot at a word boundary."""
    title = title or fallback
    if len(title) <= TITLE_SLOT:
        return title
    cut = title[:TITLE_SLOT]
    space = cut.rfind(" ")
    # Only honour a word boundary that leaves a usable title behind.
    if space >= TITLE_SLOT // 2:
        cut = cut[:space]
    return cut.rstrip(" -,:;.")


def drop_copies(docs: list) -> list:
    """Collapse documents that are the same document at another path.

    A workspace accumulates them — `iakovos-trading/src/PHASE_4_SIZING.md` and
    two merge copies of it — and each one is a separate row. With five result
    slots, three copies of one file is most of an answer spent saying the same
    thing. Same title and same text is the test; the surviving copy is the one
    the link graph rates highest, which is the one worth showing.
    """
    best = {}
    for d in docs:
        key = (d["t"], d["b"])
        if key not in best or d["pr"] > best[key]["pr"]:
            best[key] = d
    kept = [d for d in docs if best[(d["t"], d["b"])] is d]
    if len(kept) != len(docs):
        print(f"  collapsed {len(docs) - len(kept)} duplicate copies", file=sys.stderr)
    return kept


def unique_titles(docs: list) -> None:
    """Make cut titles distinct again, in place.

    Two files under different directories can share a name, and the Brief
    de-duplicates result rows by their text — so identical titles would erase
    one of the two documents from every answer it should appear in.
    """
    seen = {}
    for d in docs:
        base = d["t"]
        if base not in seen:
            seen[base] = 1
            continue
        seen[base] += 1
        # Room for " (2)" without pushing the slot over.
        suffix = f" ({seen[base]})"
        d["t"] = base[: TITLE_SLOT - len(suffix)].rstrip(" -,:;.") + suffix


def workspace_docs() -> list:
    if not WORKSPACE.exists():
        sys.exit(
            f"no workspace index at {WORKSPACE}\n"
            "Nothing to merge — corpus.json would be seed-only. Re-crawl the\n"
            "workspace first, or pass --seed-only if that is what you meant."
        )
    entries = json.loads(WORKSPACE.read_text())["entries"]
    docs = []
    dropped = {"url": 0, "noise": 0, "thin": 0}
    for e in entries:
        path = e["path"]
        url = "os://" + path
        if len(url) > URL_SLOT:
            # Never truncate: a cut path is a different document, and two cuts
            # that agree silently collapse into one row.
            print(f"  skip (url {len(url)}B > {URL_SLOT}): {path}", file=sys.stderr)
            dropped["url"] += 1
            continue
        if is_noise(path):
            dropped["noise"] += 1
            continue
        title = fit_title(to_ascii(e.get("title", "")), pathlib.Path(path).name)
        if not title:
            continue
        if token_count(title, title, to_ascii(e.get("snippet", ""))) < MIN_TOKENS:
            dropped["thin"] += 1
            continue
        parts = path.split("/")
        cat = parts[0] if len(parts) > 1 else "root"
        docs.append(
            {
                "t": title,
                "u": url,
                "c": to_ascii(cat)[:24],
                "b": to_ascii(e.get("snippet", ""))[:SNIPPET_MAX],
                "pr": round(float(e.get("pr", 0.0)), 4),
            }
        )
    print(
        f"  workspace: kept {len(docs)} of {len(entries)} "
        f"(dropped {dropped['noise']} generated, {dropped['thin']} too thin, "
        f"{dropped['url']} over the URL slot)",
        file=sys.stderr,
    )
    return docs


def build(seed_only: bool) -> dict:
    seed = json.loads(SEED.read_text())
    docs = [dict(d) for d in seed["docs"]]
    for d in docs:
        d["t"] = fit_title(to_ascii(d["t"]), d["u"])
        d["b"] = to_ascii(d.get("b", ""))[:SNIPPET_MAX]
    if not seed_only:
        docs += workspace_docs()
    docs = drop_copies(docs)
    # Only after the copies are gone: what is left sharing a title is genuinely
    # two documents, and they have to stay tellable apart.
    unique_titles(docs)
    for d in docs:
        assert d["t"].isascii() and d["u"].isascii(), d
        assert len(d["t"]) <= TITLE_SLOT, d
        assert len(d["u"]) <= URL_SLOT, d
    return {
        "source": "search/seed.json + the workspace index; built by scripts/bake-corpus.py",
        # Read by scripts/publish-os.sh, which refuses to upload an image built
        # from this corpus. The index is compiled into the ISO, so publishing
        # one baked from the workspace puts the owner's own documents — titles
        # and 320 characters each — on a public download.
        "personal": not seed_only,
        "docs": docs,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="fail if out of date")
    ap.add_argument("--seed-only", action="store_true", help="skip the workspace index")
    args = ap.parse_args()

    built = build(args.seed_only)
    text = json.dumps(built, indent=1, ensure_ascii=True) + "\n"

    if args.check:
        current = CORPUS.read_text() if CORPUS.exists() else ""
        if current != text:
            print(f"{CORPUS} is stale — run ./scripts/bake-corpus.py", file=sys.stderr)
            return 1
        print(f"{CORPUS} is up to date ({len(built['docs'])} docs)")
        return 0

    CORPUS.write_text(text)
    seeded = len(json.loads(SEED.read_text())["docs"])
    print(
        f"{CORPUS}: {len(built['docs'])} docs "
        f"({seeded} curated + {len(built['docs']) - seeded} from the workspace)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
