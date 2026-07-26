"""Shared QEMU argv + assertions for `make smoke` / `make smoke-bridge`."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

HELLO = b"os: hello from kernel"
MCP_BRIDGE = b"mcp: bridge live"
# isa-debug-exit: guest writes 0x10 → host status ((0x10 << 1) | 1) = 33
OK_STATUS = 33


def qemu_argv(iso: Path, serial_path: Path, *, com2: str | None = None) -> list[str]:
    # Shared with `make run` via exported Makefile `QEMU_MACHINE`.
    raw = os.environ.get("QEMU_MACHINE")
    if not raw:
        print(
            "error: QEMU_MACHINE unset — run via `make smoke` / `make smoke-bridge`",
            file=sys.stderr,
        )
        sys.exit(1)
    machine = raw.split()
    argv = [
        "qemu-system-x86_64",
        *machine,
        "-cdrom",
        str(iso),
        "-boot",
        "d",
        "-serial",
        f"file:{serial_path}",
    ]
    if com2 is not None:
        argv += ["-serial", f"tcp:{com2},server"]
    argv += [
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-no-reboot",
    ]
    return argv


def run_qemu(
    argv: list[str],
    *,
    cwd: Path,
    serial_path: Path,
    timeout: float,
    log_path: Path | None = None,
) -> tuple[int, bytes]:
    if log_path is None:
        proc = subprocess.Popen(argv, cwd=cwd)
    else:
        with log_path.open("wb") as logf:
            proc = subprocess.Popen(
                argv, cwd=cwd, stdout=logf, stderr=subprocess.STDOUT
            )
    try:
        status = proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
        print(f"error: QEMU timed out after {timeout:g}s", file=sys.stderr)
        sys.exit(1)
    return status, serial_path.read_bytes()


def check_smoke(
    status: int,
    serial: bytes,
    *,
    need_mcp: bool = False,
    qemu_log: Path | None = None,
) -> None:
    if status != OK_STATUS:
        print(
            f"error: QEMU exited with status {status} (expected {OK_STATUS})",
            file=sys.stderr,
        )
        print("--- serial ---", file=sys.stderr)
        sys.stderr.buffer.write(serial + b"\n")
        if qemu_log is not None:
            print("--- qemu ---", file=sys.stderr)
            sys.stderr.buffer.write(qemu_log.read_bytes())
        sys.exit(1)
    if HELLO not in serial:
        print("error: hello banner not found on serial", file=sys.stderr)
        sys.stderr.buffer.write(serial + b"\n")
        sys.exit(1)
    if need_mcp and MCP_BRIDGE not in serial:
        print("error: MCP bridge not connected", file=sys.stderr)
        sys.stderr.buffer.write(serial + b"\n")
        sys.exit(1)


def _root_and_iso() -> tuple[Path, Path]:
    root = Path(os.environ.get("OS_SMOKE_ROOT", Path(__file__).resolve().parent.parent))
    iso_name = os.environ.get("OS_SMOKE_ISO", f"{os.environ.get('IMAGE_NAME', 'os')}.iso")
    iso = root / iso_name
    if not iso.is_file():
        print(f"error: {iso.name} missing — run 'make iso' first", file=sys.stderr)
        sys.exit(1)
    return root, iso


def run_serial_smoke() -> None:
    root, iso = _root_and_iso()
    serial_path = Path(tempfile.mkstemp(prefix="os-smoke-serial.")[1])
    qemu_log = Path(tempfile.mkstemp(prefix="os-smoke-qemu.")[1])
    try:
        argv = qemu_argv(iso, serial_path)
        status, serial = run_qemu(
            argv, cwd=root, serial_path=serial_path, timeout=90, log_path=qemu_log
        )
        check_smoke(status, serial, qemu_log=qemu_log)
        print("smoke ok: serial hello + qemu exit 33")
    finally:
        serial_path.unlink(missing_ok=True)
        qemu_log.unlink(missing_ok=True)


def _ensure_bridge(root: Path, addr: str) -> Path:
    """Start dial-mode bridge via ensure-bridge.sh; return .bridge.pid path."""
    env = os.environ.copy()
    env["OS_MCP_BRIDGE_CONNECT"] = f"tcp:{addr}"
    subprocess.run(
        [str(root / "scripts" / "ensure-bridge.sh")],
        cwd=root,
        env=env,
        check=True,
    )
    return root / ".bridge.pid"


def _stop_bridge(pid_file: Path) -> None:
    if pid_file.is_file():
        try:
            pid = int(pid_file.read_text().strip())
            os.kill(pid, 15)
        except (ValueError, OSError, ProcessLookupError):
            pass
    pid_file.unlink(missing_ok=True)


def run_bridge_smoke() -> None:
    root, iso = _root_and_iso()
    addr = os.environ.get("OS_SMOKE_ADDR", os.environ.get("OS_MCP_BRIDGE_ADDR", "127.0.0.1:7420"))
    serial_path = Path(tempfile.mkstemp(prefix="os-bridge-serial.")[1])
    pid_file = _ensure_bridge(root, addr)
    try:
        # Guest listens; dialing bridge must already be up (early PING).
        argv = qemu_argv(iso, serial_path, com2=addr)
        status, serial = run_qemu(argv, cwd=root, serial_path=serial_path, timeout=120)
        check_smoke(status, serial, need_mcp=True)
        print("smoke-bridge ok: hello + mcp bridge live")
    finally:
        _stop_bridge(pid_file)
        serial_path.unlink(missing_ok=True)


def main(argv: list[str] | None = None) -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "mode",
        choices=("serial", "bridge"),
        help="serial = COM1 hello only; bridge = COM2 MCP dial smoke",
    )
    args = p.parse_args(argv)
    if args.mode == "serial":
        run_serial_smoke()
    else:
        run_bridge_smoke()


if __name__ == "__main__":
    main()
