# linux/ — the guest side of the bridge, on a Linux substrate

`agent-shell` is a Linux guest doing what the kernel guest does today: ask the
host bridge a question and render the answer.

That is the whole milestone. If a stock Alpine aarch64 image can talk to
`host/bridge` with nothing but busybox, then the substrate decision in
`docs/linux-os-doc-v02.md` is not blocked on a port — the differentiated parts
(capability model, agent, multi-source search, the bridge itself) are already
reachable from Linux, and the drivers being deleted are not in the way.

## Why a shell script and not Rust

The obvious move is a small `std` Rust binary — `TcpStream`, `BufReader`,
`lines()`. It is maybe eighty lines and it is the right long-term answer.

It is the wrong *first* answer, for one reason: the guest is
`aarch64-unknown-linux-musl` and the host is macOS. Producing that binary here
means a cross toolchain — the Rust target, a musl sysroot, and a linker that
agrees with both. None of that is installed, none of it is interesting, and all
of it is a linker fight rather than a substrate proof. A milestone that ends in
"the substrate works, once you fix your linker" has not proved anything.

busybox already ships `nc`, `sed`, `awk` and a POSIX shell. The bridge speaks a
line protocol precisely so that a client this small is possible. So the first
client is the one that needs no toolchain at all, no build step, and no
artefact to copy in beyond a 5 KB text file.

A Rust port comes later, and it should — the shell client cannot hold a
connection open across several calls, and that matters once the guest is doing
more than one query at a time. It does not matter yet.

## Protocol

One request, one reply, one connection:

```
-> CALL agent.act goal=<query> k=5 portal=1
<- OK agent.act n=<count> intent=<intent>
   SAY <sentence>
   ROW title=…|url=…|why=…        (zero or more)
   END
```

or, on failure, a single line:

```
<- ERR agent.act <why>
```

`goal=` is sent first and `k=` terminates it, which is how a multi-word query
folds into one goal: the bridge's own argument parser (`parse_args` in
`host/bridge/src/main.rs`) appends any token that does not look like `key=` to
the value before it.

## Usage

```sh
./agent-shell nvda
./agent-shell wheel strategy assignment risk
```

| variable | default | meaning |
|---|---|---|
| `BRIDGE_ADDR` | `10.0.2.2:7420` | host:port of the bridge |
| `BRIDGE_TIMEOUT` | `60` | seconds passed to `nc -w` |
| `AGENT_K` | `5` | rows requested (`k=`) |
| `BRIDGE_NC_OPTS` | `-w $BRIDGE_TIMEOUT` | full `nc` option string, for a busybox built without `-w` |

`10.0.2.2` is the default because that is where QEMU user-mode networking puts
the host — `scripts/linux-vm.sh` boots with `-netdev user`, so from inside the
guest the host is never `127.0.0.1`. Point `BRIDGE_ADDR` at `127.0.0.1:7420`
to test on the host itself.

## Exit codes

These are the contract. The point of having more than "0 or 1" is that a failed
search must never be mistaken for an empty one.

| code | meaning |
|---|---|
| 0 | at least one row printed |
| 1 | bridge answered `OK` with zero rows — the `SAY` line is still printed, and a `no results` note goes to stderr |
| 2 | usage / bad `BRIDGE_ADDR` |
| 3 | bridge unreachable, or answered nothing |
| 4 | bridge replied `ERR` |
| 5 | reply was not the protocol: no `OK agent.act` first line, or no `END` marker |

Nothing is printed to stdout on codes 2–5. A truncated reply is code 5 rather
than a short list, because a list that is missing its tail looks exactly like a
complete one.

## Getting it into the guest

It is a single text file with no build step, so any of these work:

```sh
# from the host, over the same user-mode network the bridge uses
# (in the guest, with a shell and busybox wget)
wget -O agent-shell http://10.0.2.2:8000/agent-shell && chmod +x agent-shell

# or paste it: it is ~5 KB of ASCII
```

The guest needs busybox `nc`, `sed`, `awk`, `tr`, `grep` — all present in
`alpine-virt`. On Alpine, `nc` comes from busybox by default; nothing to
install.

## Tests

```sh
./test-agent-shell.sh          # or: dash test-agent-shell.sh
```

The suite runs against a fake bridge, not the real one, because the failure
paths are the interesting ones and the real bridge cannot be asked to be
unreachable, truncated, or wrong on demand. Cases: rows rendered, `|` inside a
field, CRLF on the wire, `ERR`, zero rows, truncated reply, garbage reply,
nothing listening, no arguments, and a newline in the query (which must not
become a second `CALL` — without the flattening step it does, and the test
proves it).

Each case asserts an exit code *and* the text produced, so "printed nothing,
exited 0" fails.

## Known limits

- One request per invocation. Each call is a fresh TCP connection, so the
  bridge's per-connection operator unlock (`config.unlock`) cannot be used from
  here — that needs the Rust client.
- A query containing a bare `word=value` token will be read by the bridge as an
  argument rather than as part of the goal. Same behaviour the kernel guest has.
- `BRIDGE_ADDR` splits on the last `:`, so a bare IPv6 literal needs brackets.
- Only `agent.act` is implemented. The other tools (`search.query`,
  `skills.*`, `portal.*`) are the same shape and would be a few lines each, but
  `agent.act` is the one that proves the path.

## Verified

Host-side, against a real `host/bridge` on `127.0.0.1:7420`:

```
$ BRIDGE_ADDR=127.0.0.1:7420 ./linux/agent-shell nvda
5 matches for nvda.

 1. NVDA  NVIDIA Corporation
    finance.yahoo.com/quote/NVDA
 2. ANET  Arista Networks Inc
    finance.yahoo.com/quote/ANET
 3. The wheel strategy's honest math on a ~$120K three-account portfolio (2026)
    teddysearch.com/research/wheel-strategy-honest-math-120k
 4. Cash-secured puts: what volatility pays at matched assignment risk  S&P 500 vs NVDA vs IRE
    teddysearch.com/research/csp-matched-risk-comparison
 5. 13F consensus basket  backtest verdict
    /tsearch/
```

Run under both `/bin/sh` and `dash` to keep busybox `ash` honest.

**Not yet verified inside the guest.** The Alpine VM has not been booted and
run against this. Everything above is host-side, which proves the protocol and
the parsing but not that Alpine's busybox behaves like Apple's `nc` — the one
place they could differ is stdin-EOF handling, and both are documented to
half-close and keep reading, which is what this depends on.
