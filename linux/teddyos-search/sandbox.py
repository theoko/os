"""Re-exec the search inside a sandbox that matches the grants.

This is the module that lets the setup screen say "denied means it cannot reach
the network" without lying. Without it, a denied capability is a branch the
process chooses not to take — which is exactly the arrangement
`docs/thesis-os-doc-v01.md` §2.3 calls bookkeeping rather than a barrier:

    "It is checked by mcp.rs before it opens COM2 — by the same binary that
     holds it, at a call site that could simply not call it."

So the check is moved out of the binary. `teddyos-search` re-executes itself
under a transient `systemd-run --user --pipe` service with properties derived
from the grants, and the second process runs in a namespace where the denied
thing is absent:

    portal.sync denied      PrivateNetwork=yes — loopback only, so a request
                            does not fail a policy check, it fails to route
    workspace.index         TemporaryFileSystem=/home + BindReadOnlyPaths for
                            exactly the chosen folders, so a path outside them
                            cannot be named

One thing PrivateNetwork does NOT block, and it is worth knowing before writing
any test against this: name resolution still works inside the namespace, because
nss-resolve reaches systemd-resolved over a Unix socket. Only the route is gone.
`selftest()` connects rather than resolves for exactly this reason.

What this is NOT: a security boundary against a determined attacker. §4 of the
thesis document is explicit about the ceiling, and namespaces are constructed
by userspace, so the construction is where the bugs are. What it does buy is
that the restriction is real for ordinary code — including our own, including
the case where a later change forgets the grant check.

If systemd is unavailable (a container, a non-systemd distro, no user manager),
we run unsandboxed rather than refusing — but `sandboxed()` reports false and
the UI must degrade its claim accordingly. Silently running unconfined while
still showing "enforced" is the one outcome this module exists to prevent.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

import caps

GUARD = "TEDDYOS_SANDBOXED"


def available() -> bool:
    """True if we can actually build the namespace."""
    if not shutil.which("systemd-run"):
        return False
    # --user needs a running user manager; without XDG_RUNTIME_DIR there is no
    # session bus to talk to and systemd-run fails with a confusing message.
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    return bool(runtime and Path(runtime).is_dir())


def sandboxed() -> bool:
    """True when this process is the confined one."""
    return os.environ.get(GUARD) == "1"


def properties() -> list[str]:
    grants = caps.load()
    props: list[str] = []

    if not grants.get("portal.sync"):
        props.append("PrivateNetwork=yes")

    # Empty out /home, then bind back only what this run legitimately needs.
    # Filtering paths would mean enumerating everything to deny; an empty tree
    # denies by absence and needs no list.
    props.append("TemporaryFileSystem=/home:ro")

    # EVERY bind target must already exist. systemd refuses to start a unit
    # whose BindReadOnlyPaths names a missing directory, and it fails at
    # namespace setup — exit 226, before the command runs, with nothing on
    # stdout. The visible symptom is a search that prints absolutely nothing,
    # which is why this is filtered rather than assumed.
    binds_ro = [caps.USER_CONF.parent]
    if grants.get("workspace.index"):
        binds_ro += [Path(p) for p in caps.workspace_paths()]
    for path in binds_ro:
        if path.is_dir():
            props.append(f"BindReadOnlyPaths={path}")

    # The portal cache lives under /home too, so without a writable bind the
    # index is refetched on every single query — the cache would appear to
    # work and silently never persist.
    if grants.get("portal.sync"):
        from search import CACHE_DIR
        if CACHE_DIR.is_dir():
            props.append(f"BindPaths={CACHE_DIR}")

    # Cheap, unconditional, and unrelated to any grant: these remove exploit
    # primitives rather than user-visible authority, so there is no reason to
    # make them a switch.
    props += [
        "NoNewPrivileges=yes",
        "ProtectKernelTunables=yes",
        "ProtectKernelModules=yes",
        "ProtectControlGroups=yes",
        "RestrictSUIDSGID=yes",
    ]
    return props


# systemd reserves 200-242 for "the unit could not be started", as opposed to
# "the program ran and exited with this". 226 is EXIT_NAMESPACE, which is the
# one a bad bind produces.
SYSTEMD_FAILURE_CODES = range(200, 243)
EXIT_SANDBOX_FAILED = 5


def reexec() -> None:
    """Replace this process with a confined copy. Returns only if it cannot."""
    if sandboxed() or not available():
        return

    # Belt and braces against the recursion above. If the guard is ever lost
    # again — a systemd change, a distro that filters --setenv, a refactor —
    # this turns an unbounded fork bomb that takes down the session manager
    # into one extra process and a legible error. The failure mode is bad
    # enough that a second, independent stop is worth the four lines.
    depth = int(os.environ.get("TEDDYOS_SANDBOX_DEPTH", "0"))
    if depth >= 1:
        _fail("sandbox re-exec recursed; refusing to nest further "
              "(the TEDDYOS_SANDBOXED guard is not reaching the unit)")
    os.environ["TEDDYOS_SANDBOX_DEPTH"] = str(depth + 1)

    # Create the directories we are about to bind, rather than discovering at
    # namespace-setup time that they are missing. This is also what makes a
    # first run work at all: before setup has been answered there is no
    # ~/.config/teddyos, and binding it was an unconditional failure.
    try:
        caps.USER_CONF.parent.mkdir(parents=True, exist_ok=True)
        from search import CACHE_DIR
        CACHE_DIR.mkdir(parents=True, exist_ok=True)
    except OSError:
        pass

    # --pipe, NOT --scope. A scope adopts an already-running process, so it has
    # nowhere to apply sandboxing: `systemd-run --scope -p PrivateNetwork=yes`
    # answers "Unknown assignment: PrivateNetwork=yes" and then runs the command
    # anyway, unconfined and exit-code 0. A permission system whose deny path
    # silently degrades to allow is the worst of all outcomes, and this one
    # printed a line that looked like a warning about a typo.
    #
    # A transient --pipe service is a real unit: verified on this guest that the
    # same curl returns 151751 bytes without the property and http=000 with it.
    cmd = ["systemd-run", "--user", "--pipe", "--quiet", "--collect"]

    # --setenv, NOT the inherited environment. systemd-run starts the unit via
    # the service manager, which builds a clean environment — passing env= to
    # subprocess.run sets it for systemd-run itself and nothing reaches the
    # unit. The guard therefore never arrived, the child believed it was
    # unconfined, and re-executed itself: one transient unit per level,
    # forever. It does not crash, it wedges — and it takes the user's systemd
    # manager with it, which then blocks every new login session on the box.
    cmd += [f"--setenv={GUARD}=1"]
    # Everything the confined copy needs to behave identically. HOME especially:
    # without it the unit gets root's and the grants file is looked up in the
    # wrong place, which reads as "nothing was ever granted".
    for var in ("HOME", "TERM", "LANG", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "TEDDYOS_SANDBOX_DEPTH",
                "TEDDYOS_PORTAL_URL", "TEDDYOS_PORTAL_TTL", "TEDDYOS_CORPUS"):
        if var in os.environ:
            cmd += [f"--setenv={var}={os.environ[var]}"]

    for prop in properties():
        cmd += ["-p", prop]
    cmd += [sys.executable, str(Path(sys.argv[0]).resolve()), *sys.argv[1:]]

    env = dict(os.environ, **{GUARD: "1"})
    try:
        completed = subprocess.run(cmd, env=env, stderr=subprocess.PIPE, text=True)
    except (OSError, subprocess.SubprocessError) as exc:
        _fail(f"could not start a confined process: {exc}")

    # A unit that fails to start exits in systemd's reserved range having run
    # nothing and printed nothing. Propagating that code was the bug: the user
    # typed a query, got no output, no error and no results, and the only
    # signal was $? = 226. Fail loudly instead — and do NOT fall back to
    # running unconfined, because that turns a denied capability into a
    # granted one at exactly the moment something is already wrong.
    if completed.returncode in SYSTEMD_FAILURE_CODES:
        detail = (completed.stderr or "").strip()
        _fail(
            f"the sandbox could not be built (systemd exit {completed.returncode})."
            + (f"\n  {detail}" if detail else "")
            + "\n  Refusing to search unconfined — a denied capability would"
              "\n  otherwise become a granted one. Run `teddyos-search --caps"
              " --verify`."
        )

    if completed.stderr:
        sys.stderr.write(completed.stderr)
    sys.exit(completed.returncode)


def _fail(message: str) -> None:
    print(f"teddyos-search: {message}", file=sys.stderr)
    sys.exit(EXIT_SANDBOX_FAILED)


def selftest() -> tuple[bool, str]:
    """Prove the network namespace actually closes. Used by `--caps --verify`.

    Claiming enforcement is cheap; this is what makes the claim checkable on
    the machine it is being claimed about, on the kernel and systemd it
    actually has, rather than on the one it was developed against.
    """
    if not available():
        return False, "no systemd user manager — nothing is enforced here"
    # Connect, do not resolve. PrivateNetwork=yes leaves a loopback-only
    # namespace, and name resolution still succeeds inside it because
    # nss-resolve talks to systemd-resolved over a Unix socket, which the
    # network namespace does not touch. A DNS-based probe therefore reports
    # PASS on a sandbox that is working and FAIL on one that is not — it was
    # measuring the one part of the stack the namespace deliberately leaves
    # alone. What the namespace actually removes is the route, so the probe
    # has to attempt a TCP connection to prove anything.
    probe = (
        "import socket,sys\n"
        "try:\n"
        "    socket.create_connection(('1.1.1.1', 443), timeout=5).close()\n"
        "    sys.exit(0)\n"   # reached the network — sandbox is NOT working
        "except OSError:\n"
        "    sys.exit(1)\n"   # no route — sandbox is working
    )
    cmd = [
        "systemd-run", "--user", "--pipe", "--quiet", "--collect",
        "-p", "PrivateNetwork=yes", sys.executable, "-c", probe,
    ]
    try:
        rc = subprocess.run(cmd, capture_output=True, timeout=30).returncode
    except (OSError, subprocess.SubprocessError):
        return False, "could not start a confined unit"
    if rc == 1:
        return True, "network namespace verified: outbound connections fail when denied"
    return False, "SANDBOX DID NOT BLOCK THE NETWORK — treat 'enforced' as false"
