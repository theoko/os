"""Shared QEMU argv + assertions for smoke-qemu / smoke-bridge."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

HELLO = b"os: hello from kernel"
MCP_EMAIL = b"mcp: email connected"
# isa-debug-exit: guest writes 0x10 → host status ((0x10 << 1) | 1) = 33
OK_STATUS = 33


def qemu_argv(iso: Path, serial_path: Path, *, com2: str | None = None) -> list[str]:
    argv = [
        "qemu-system-x86_64",
        "-M",
        "q35",
        "-m",
        "512M",
        "-cdrom",
        str(iso),
        "-boot",
        "d",
        "-display",
        "none",
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
    if need_mcp and MCP_EMAIL not in serial:
        print("error: MCP bridge not connected", file=sys.stderr)
        sys.stderr.buffer.write(serial + b"\n")
        sys.exit(1)
