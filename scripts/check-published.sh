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
fetch() { curl -fsS --max-time 120 --retry 2 --retry-delay 3 --retry-all-errors "$@"; }

[ "$QUIET" = 1 ] || echo "checking $SITE"

# 1. The page exists and is the download page, not a 404 body served as 200.
page="$(fetch "$SITE/os.html" || true)"
if [ -z "$page" ]; then
  bad "os.html did not respond"
elif ! printf '%s' "$page" | grep -q 'teddy OS'; then
  bad "os.html responded but does not look like the download page"
else
  ok "os.html"
fi

# 2. The way in. This link lives in index.html, which other work deploys by
#    rsync — losing it is silent, and the page becomes unreachable rather
#    than broken, which nobody reports.
# Fetched to a variable, not piped: `grep -q` exits on the first match, curl
# takes SIGPIPE, and with `pipefail` that curl failure masks grep's success —
# which reported the link missing when it was there.
home="$(fetch "$SITE/" || true)"
if [ -z "$home" ]; then
  bad "the homepage did not respond"
elif printf '%s' "$home" | grep -q 'os\.html'; then
  ok "homepage links to os.html"
else
  bad "the homepage no longer links to os.html — the download page is orphaned"
fi

# 3. What release is published, and the checksums that go with it.
info="$(fetch "$SITE/os/BUILD-INFO.txt" || true)"
commit="$(printf '%s' "$info" | awk '/commit:/{print $2}')"
sums="$(fetch "$SITE/os/SHA256SUMS" || true)"
if [ -z "$commit" ] || [ -z "$sums" ]; then
  bad "BUILD-INFO.txt or SHA256SUMS is missing — cannot verify anything else"
  exit 1
fi
ok "published release $commit"

# 4. The images themselves, fetched the way the page links them. This is the
#    check that matters: it is the exact bytes a visitor receives.
tmp="$(mktemp -d -t os-check)"
trap 'rm -rf "$tmp"' EXIT
for image in os.iso os-arm64.iso; do
  want="$(printf '%s' "$sums" | awk -v f="$image" '$2 == f {print $1}')"
  if [ -z "$want" ]; then
    bad "$image has no entry in SHA256SUMS"
    continue
  fi
  if ! fetch -o "$tmp/$image" "$SITE/os/$image?v=$commit"; then
    bad "$image could not be downloaded"
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
  bare="$(fetch -o "$tmp/bare-$image" "$SITE/os/$image" && shasum -a 256 "$tmp/bare-$image" | awk '{print $1}')"
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
