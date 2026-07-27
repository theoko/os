#!/usr/bin/env bash
# Check that the published OS download still works, from outside.
#
# Why this exists: everything here has already failed once. The CDN served an
# older image than the checksums advertised, which to anyone verifying a
# download looks like tampering. The homepage link lives in a file two other
# people deploy by rsync, so it can be removed by a deploy that was about
# something else entirely. And an ISO can be replaced by a truncated one
# without anything on the server noticing.
#
# So this asks the questions a visitor's experience actually depends on, from
# the public internet, and says which one failed. Exit 0 = all good.
#
#   ./scripts/check-published.sh          check the live site
#   ./scripts/check-published.sh --quiet  only print problems (for cron)
set -uo pipefail

SITE="${CHECK_SITE:-https://teddysearch.com/tsearch}"
QUIET=0
[ "${1:-}" = "--quiet" ] && QUIET=1

fails=0
warns=0
say()  { [ "$QUIET" = 1 ] || printf '  %s\n' "$*"; }
ok()   { [ "$QUIET" = 1 ] || printf '  ok    %s\n' "$*"; }
bad()  { printf '  FAIL  %s\n' "$*" >&2; fails=$((fails + 1)); }
warn() { printf '  warn  %s\n' "$*" >&2; warns=$((warns + 1)); }

# Retried: this runs unattended, and a monitor that reports a blip as an
# outage gets muted, after which it reports nothing at all. One flaky fetch
# should not wake anyone; a real outage survives three tries.
#
# `fetch <url> <file>` — to a FILE, over HTTP/1.1, failing loudly.
#
# Three lessons are baked into that one line. Cloudflare intermittently breaks
# the stream ("curl: (16) Error in the HTTP2 framing layer"); captured through
# a command substitution that left a TRUNCATED page in the variable, and a
# truncated homepage does not contain the link — so a network blip reported
# itself as "the download page is orphaned", about one run in five. HTTP/1.1
# avoids the framing bug; writing to a file means a retry restarts cleanly
# instead of appending to half a page; and grepping a file needs no pipe (a
# `grep -q` reader exits on the first match, the writer takes SIGPIPE, and
# `pipefail` calls the whole pipeline failed — the same false alarm in a
# different disguise).
#
# The rule underneath all three: never report something as broken because we
# could not look at it. A transport failure and a wrong page are different
# findings and must read differently.
fetch() {
  curl -fsS --http1.1 --max-time 120 --retry 3 --retry-delay 2 --retry-all-errors \
    -o "$2" "$1"
}

WORK="$(mktemp -d -t os-check)"
trap 'rm -rf "$WORK"' EXIT

[ "$QUIET" = 1 ] || echo "checking $SITE"

# 1. The page exists and is the download page, not a 404 body served as 200.
if ! fetch "$SITE/os.html" "$WORK/os.html"; then
  bad "could not fetch os.html (network or CDN — this is not a claim about the page)"
elif ! grep -q 'teddy OS' "$WORK/os.html"; then
  bad "os.html responded but does not look like the download page"
else
  ok "os.html"
fi

# 2. The way in. This link lives in index.html, which other work deploys by
#    rsync — losing it is silent, and the page becomes unreachable rather
#    than broken, which nobody reports.
if ! fetch "$SITE/" "$WORK/index.html"; then
  bad "could not fetch the homepage (network or CDN — this is not a claim about the page)"
elif grep -q 'os\.html' "$WORK/index.html"; then
  ok "homepage links to os.html"
else
  bad "the homepage no longer links to os.html — the download page is orphaned"
fi

# 3. What release is published, and the checksums that go with it.
fetch "$SITE/os/BUILD-INFO.txt" "$WORK/BUILD-INFO.txt" || true
fetch "$SITE/os/SHA256SUMS" "$WORK/SHA256SUMS" || true
commit="$(awk '/commit:/{print $2}' "$WORK/BUILD-INFO.txt" 2>/dev/null || true)"
if [ -z "$commit" ] || [ ! -s "$WORK/SHA256SUMS" ]; then
  bad "BUILD-INFO.txt or SHA256SUMS is missing — cannot verify anything else"
  exit 1
fi
ok "published release $commit"

# 3b. The machine-readable descriptor. `CALL update.check` on every installed
#     machine reads this, and /tsearch/ has a catch-all — so a missing
#     manifest answers HTTP 200 with the homepage rather than 404. Judged by
#     its body for that reason; a status code cannot tell the two apart.
#
#     A warning, not a failure: a visitor's download works without it. What it
#     costs is that machines cannot tell whether they are behind — they report
#     state=undetermined, because a commit and a version are not comparable.
fetch "$SITE/os/manifest.json" "$WORK/manifest.json" || true
if grep -q '"x86_sha256"' "$WORK/manifest.json" 2>/dev/null; then
  ok "manifest.json (update.check can name a version)"
else
  warn "manifest.json is not published — the catch-all page is answering, so update.check reports state=undetermined. Fixed by the next 'make publish-os'."
fi

# 4. The images themselves, fetched the way the page links them. This is the
#    check that matters: it is the exact bytes a visitor receives.
tmp="$WORK/images"
mkdir -p "$tmp"
for image in os.iso os-arm64.iso; do
  want="$(awk -v f="$image" '$2 == f {print $1}' "$WORK/SHA256SUMS")"
  if [ -z "$want" ]; then
    bad "$image has no entry in SHA256SUMS"
    continue
  fi
  if ! fetch "$SITE/os/$image?v=$commit" "$tmp/$image"; then
    bad "could not download $image (network or CDN — this is not a claim about the image)"
    continue
  fi
  got="$(shasum -a 256 "$tmp/$image" | awk '{print $1}')"
  if [ "$got" != "$want" ]; then
    bad "$image does not match its published checksum — a visitor verifying this download would conclude it was tampered with"
  else
    size="$(wc -c < "$tmp/$image" | tr -d ' ')"
    ok "$image ($size bytes) matches SHA256SUMS"
  fi

  # The bare URL is what someone gets if they type it or follow an old link.
  # Expected to lag the CDN TTL after a release, so this is a warning.
  bare=""
  if fetch "$SITE/os/$image" "$tmp/bare-$image"; then
    bare="$(shasum -a 256 "$tmp/bare-$image" | awk '{print $1}')"
  fi
  if [ -n "$bare" ] && [ "$bare" != "$want" ]; then
    warn "$image at the un-versioned URL is a cached older build (harmless: the page links the versioned one)"
  fi
done

if [ "$fails" -gt 0 ]; then
  echo "FAILED: $fails problem(s), $warns warning(s)" >&2
  exit 1
fi
[ "$QUIET" = 1 ] || echo "all good${warns:+ ($warns warning(s))}"
exit 0
