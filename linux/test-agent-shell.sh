#!/bin/sh
#
# Tests for linux/agent-shell.
#
# These run against a fake bridge, not the real one, so that the failure paths
# (ERR, truncated reply, connection refused) are reachable at all — the real
# bridge cannot be asked to be unreachable on demand. The fake speaks the same
# line protocol host/bridge/src/main.rs emits.
#
# Every case here fails if agent-shell regresses: each asserts an exit code AND
# the text on stdout/stderr, so "it printed nothing and exited 0" is a failure.
set -u

HERE=$(cd "$(dirname "$0")" && pwd)
CLIENT="$HERE/agent-shell"
WORK=$(mktemp -d)
FAILED=0
# Reserved-but-closed port for the unreachable test; nothing should ever listen
# here, and if something does the test says "expected 3" rather than passing.
DEAD_PORT=7421

cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

fail() {
	printf 'FAIL %s: %s\n' "$1" "$2" >&2
	FAILED=$((FAILED + 1))
}

# Only report a case as passing if it added no failures since it started, so a
# green "ok" line can never sit underneath a FAIL for the same case.
MARK=0
mark() { MARK=$FAILED; }
ok() {
	if [ "$FAILED" -eq "$MARK" ]; then
		printf 'ok   %s\n' "$1"
	else
		printf 'FAIL %s\n' "$1"
	fi
}

# Start a one-shot fake bridge that replies with $2 and print the port it bound.
start_fake() {
	python3 - "$WORK/$1.port" <<PY >/dev/null 2>&1 &
import socket, sys
reply = open("$WORK/$1.reply", "rb").read()
s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(("127.0.0.1", 0))
s.listen(1)
open(sys.argv[1], "w").write(str(s.getsockname()[1]))
c, _ = s.accept()
# Read to EOF, not one line: the injection test needs to see everything the
# client put on the wire, including anything it tried to send as a second line.
req = c.makefile("rb").read()
open("$WORK/$1.req", "wb").write(req)
c.sendall(reply)
c.close()
PY
	# Wait for the port file rather than sleeping a guess.
	i=0
	while [ ! -s "$WORK/$1.port" ]; do
		i=$((i + 1))
		[ "$i" -gt 100 ] && return 1
		sleep 0.05
	done
	cat "$WORK/$1.port"
}

run_case() {
	mark
	name=$1
	reply=$2
	printf '%s' "$reply" >"$WORK/$name.reply"
	port=$(start_fake "$name") || {
		fail "$name" "fake bridge did not start"
		return 1
	}
	BRIDGE_ADDR="127.0.0.1:$port" "$CLIENT" nvda >"$WORK/$name.out" 2>"$WORK/$name.err"
	printf '%s' $? >"$WORK/$name.code"
}

expect_code() {
	got=$(cat "$WORK/$1.code")
	[ "$got" = "$2" ] || fail "$1" "expected exit $2, got $got"
}

expect_out() {
	grep -q -- "$2" "$WORK/$1.out" ||
		fail "$1" "stdout missing \"$2\" (got: $(tr '\n' '/' <"$WORK/$1.out"))"
}

expect_err() {
	grep -q -- "$2" "$WORK/$1.err" ||
		fail "$1" "stderr missing \"$2\" (got: $(tr '\n' '/' <"$WORK/$1.err"))"
}

# --- happy path -------------------------------------------------------------
run_case rows 'OK agent.act n=2 intent=search
SAY 2 matches for nvda.
ROW title=NVDA gamma exposure|url=https://teddysearch.com/gex|why=matched your query
ROW title=Nvidia earnings|url=https://example.test/nvda|why=matched your query
END
'
expect_code rows 0
expect_out rows '2 matches for nvda.'
expect_out rows ' 1. NVDA gamma exposure'
expect_out rows 'https://teddysearch.com/gex'
expect_out rows ' 2. Nvidia earnings'
grep -q 'CALL agent.act goal=nvda k=5 portal=1' "$WORK/rows.req" ||
	fail rows "request line was $(cat "$WORK/rows.req")"
ok rows

# --- a bar inside a field must not truncate the title ------------------------
run_case bars 'OK agent.act n=1 intent=search
SAY 1 match.
ROW title=A|B pair|url=https://example.test/x|why=because
END
'
expect_code bars 0
expect_out bars 'A|B pair'
expect_out bars 'https://example.test/x'
ok bars

# --- CRLF on the wire ---------------------------------------------------------
# The same protocol rides a serial line in the kernel guest, where a CR can be
# appended. A stray CR must not end up glued to the URL we print.
mark
# Written as bytes rather than through a shell variable so the CRs are exactly
# where they are meant to be, including on the final line.
python3 - "$WORK/crlf.reply" <<'PY'
import sys
open(sys.argv[1], "wb").write(
    b"OK agent.act n=1 intent=search\r\n"
    b"SAY 1 match for nvda.\r\n"
    b"ROW title=NVDA  NVIDIA Corporation|url=finance.yahoo.com/quote/NVDA|why=matched\r\n"
    b"END\r\n"
)
PY
port=$(start_fake crlf) || fail crlf "fake bridge did not start"
BRIDGE_ADDR="127.0.0.1:$port" "$CLIENT" nvda >"$WORK/crlf.out" 2>"$WORK/crlf.err"
printf '%s' $? >"$WORK/crlf.code"
expect_code crlf 0
expect_out crlf '1 match for nvda.'
grep -q 'finance.yahoo.com/quote/NVDA$' "$WORK/crlf.out" ||
	fail crlf "CR survived onto the end of the url line"
ok crlf

# --- ERR must not read as success -------------------------------------------
run_case err 'ERR agent.act missing_goal
'
expect_code err 4
expect_err err 'missing_goal'
[ -s "$WORK/err.out" ] && fail err "printed rows for an ERR reply"
ok err

# --- zero rows is not a successful search ------------------------------------
run_case empty 'OK agent.act n=0 intent=search
SAY None of your own sources are switched on.
END
'
expect_code empty 1
expect_out empty 'None of your own sources'
expect_err empty 'no results'
ok empty

# --- a reply cut off before END is a protocol error, not a short list --------
run_case truncated 'OK agent.act n=2 intent=search
SAY 2 matches for nvda.
ROW title=Only one arrived|url=https://example.test/1|why=x
'
expect_code truncated 5
expect_err truncated 'END'
ok truncated

# --- garbage on the wire ------------------------------------------------------
run_case garbage 'HTTP/1.1 200 OK
'
expect_code garbage 5
expect_err garbage 'unexpected reply'
ok garbage

# --- nothing listening --------------------------------------------------------
mark
BRIDGE_ADDR="127.0.0.1:$DEAD_PORT" BRIDGE_TIMEOUT=2 "$CLIENT" nvda \
	>"$WORK/dead.out" 2>"$WORK/dead.err"
printf '%s' $? >"$WORK/dead.code"
expect_code dead 3
expect_err dead 'no reply from bridge'
[ -s "$WORK/dead.out" ] && fail dead "printed something for an unreachable bridge"
ok dead

# --- usage --------------------------------------------------------------------
mark
"$CLIENT" >"$WORK/usage.out" 2>"$WORK/usage.err"
printf '%s' $? >"$WORK/usage.code"
expect_code usage 2
expect_err usage 'usage:'
ok usage

# --- a newline in the query must not become a second CALL ---------------------
mark
printf '%s' 'OK agent.act n=1 intent=search
SAY ok.
ROW title=T|url=U|why=W
END
' >"$WORK/inject.reply"
port=$(start_fake inject) || fail inject "fake bridge did not start"
BRIDGE_ADDR="127.0.0.1:$port" "$CLIENT" 'nvda
CALL email.send to=evil@example.test' >"$WORK/inject.out" 2>"$WORK/inject.err"
printf '%s' $? >"$WORK/inject.code"
expect_code inject 0
# The whole request must be ONE line. Two lines would mean a query string can
# smuggle a second command (email.send, config.unlock…) past the caller.
sent_lines=$(wc -l <"$WORK/inject.req" | tr -d ' ')
[ "$sent_lines" = "1" ] ||
	fail inject "client sent $sent_lines lines: $(tr '\n' '/' <"$WORK/inject.req")"
[ "$(grep -c '^CALL' "$WORK/inject.req")" = "1" ] ||
	fail inject "more than one CALL reached the bridge"
grep -q '^CALL agent.act goal=nvda CALL email.send' "$WORK/inject.req" ||
	fail inject "request line was $(cat "$WORK/inject.req")"
ok inject

if [ "$FAILED" -ne 0 ]; then
	printf '\n%d test(s) failed\n' "$FAILED" >&2
	exit 1
fi
printf '\nall agent-shell tests passed\n'
