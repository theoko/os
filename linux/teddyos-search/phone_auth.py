"""Phone-friendly auth helpers — QR codes and LAN pairing pages.

Used by Connect (device-code QR) and Email (link-from-phone webmail).
No secrets required. Optional Google OAuth client id is for future use.
"""

from __future__ import annotations

import html
import socket
import subprocess
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from urllib.parse import urlparse


def lan_ipv4() -> str | None:
    """Best-effort primary LAN IPv4 (not 127.0.0.1)."""
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            s.connect(("8.8.8.8", 80))
            ip = s.getsockname()[0]
        finally:
            s.close()
        if ip and not ip.startswith("127."):
            return ip
    except OSError:
        pass
    try:
        for info in socket.getaddrinfo(socket.gethostname(), None, socket.AF_INET):
            ip = info[4][0]
            if ip and not ip.startswith("127."):
                return ip
    except OSError:
        pass
    return None


def make_qr_png(data: str, path: Path, *, size: int = 8) -> bool:
    """Write a QR PNG for *data*. Returns True on success."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    # 1) qrencode (Debian package)
    qrencode = _which("qrencode")
    if qrencode:
        try:
            r = subprocess.run(
                [qrencode, "-o", str(path), "-t", "PNG", "-s", str(size), data],
                capture_output=True,
                timeout=5,
                check=False,
            )
            if r.returncode == 0 and path.is_file() and path.stat().st_size > 0:
                return True
        except (OSError, subprocess.TimeoutExpired):
            pass
    # 2) Python qrcode if installed
    try:
        import qrcode  # type: ignore

        img = qrcode.make(data)
        img.save(str(path))
        return path.is_file() and path.stat().st_size > 0
    except Exception:  # noqa: BLE001
        pass
    return False


def _which(name: str) -> str | None:
    from shutil import which

    return which(name)


def qr_temp_png(data: str) -> Path | None:
    """QR to a temp PNG path, or None if generation failed."""
    fd, name = tempfile.mkstemp(suffix=".png", prefix="teddyos-qr-")
    try:
        import os

        os.close(fd)
    except OSError:
        pass
    p = Path(name)
    if make_qr_png(data, p):
        return p
    try:
        p.unlink(missing_ok=True)
    except OSError:
        pass
    return None


class PhonePairServer:
    """Tiny HTTP server for phone pairing pages (same Wi‑Fi).

    Serves:
      GET /           — instructions + open webmail on the phone
      GET /go         — redirect to the provider URL
      GET /health     — ok
    """

    def __init__(
        self,
        *,
        title: str,
        open_url: str,
        blurb: str = "",
        port: int = 0,
    ):
        self.title = title
        self.open_url = open_url
        self.blurb = blurb or (
            "Sign in on this phone if you want. "
            "Then continue on the computer."
        )
        self._httpd: HTTPServer | None = None
        self._thread: threading.Thread | None = None
        self.port = port
        self.ip = lan_ipv4() or "127.0.0.1"

    @property
    def pair_url(self) -> str:
        return f"http://{self.ip}:{self.port}/"

    def start(self) -> str:
        title = self.title
        open_url = self.open_url
        blurb = self.blurb
        outer = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, fmt: str, *args) -> None:  # noqa: A003
                return

            def do_GET(self) -> None:  # noqa: N802
                parsed = urlparse(self.path)
                if parsed.path in ("/go", "/open"):
                    self.send_response(302)
                    self.send_header("Location", open_url)
                    self.end_headers()
                    return
                if parsed.path == "/health":
                    body = b'{"ok":true}'
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                    return
                # Landing page
                safe_title = html.escape(title)
                safe_blurb = html.escape(blurb)
                safe_url = html.escape(open_url)
                page = f"""<!DOCTYPE html>
<html lang="en"><head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>{safe_title}</title>
<style>
  body {{ font-family: system-ui, -apple-system, sans-serif; margin: 0;
    min-height: 100vh; display: flex; align-items: center; justify-content: center;
    background: linear-gradient(145deg, #6b5cff 0%, #c44dff 55%, #ff6b9d 100%);
    color: #1a1a1a; }}
  .card {{ background: #fbfbfa; border-radius: 20px; padding: 28px 24px;
    max-width: 360px; width: 90%; box-shadow: 0 20px 50px rgba(0,0,0,.18); }}
  h1 {{ font-size: 1.45rem; margin: 0 0 10px; }}
  p {{ color: #555; line-height: 1.45; margin: 0 0 18px; font-size: 1rem; }}
  a.btn {{ display: block; text-align: center; text-decoration: none;
    background: #3584e4; color: #fff; font-weight: 600; padding: 14px 18px;
    border-radius: 999px; font-size: 1.05rem; }}
  a.btn:active {{ opacity: .9; }}
  .fine {{ font-size: .85rem; color: #888; margin-top: 16px; }}
</style></head><body>
<div class="card">
  <h1>{safe_title}</h1>
  <p>{safe_blurb}</p>
  <a class="btn" href="/go">Open on this phone</a>
  <p class="fine">When you’re done here, continue on the computer.</p>
  <p class="fine" style="word-break:break-all">{safe_url}</p>
</div></body></html>"""
                raw = page.encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Content-Length", str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)

        self._httpd = HTTPServer(("0.0.0.0", self.port), Handler)
        self.port = self._httpd.server_address[1]
        self._thread = threading.Thread(target=self._httpd.serve_forever, daemon=True)
        self._thread.start()
        return self.pair_url

    def stop(self) -> None:
        if self._httpd is not None:
            try:
                self._httpd.shutdown()
            except Exception:  # noqa: BLE001
                pass
            self._httpd = None


def github_device_url(code: str, base: str = "") -> str:
    """Prefer a URL that pre-fills the GitHub device code."""
    b = (base or "https://github.com/login/device").rstrip("/")
    if "user_code=" in b:
        return b
    if "github.com/login/device" in b:
        return f"https://github.com/login/device?user_code={code}"
    sep = "&" if "?" in b else "?"
    return f"{b}{sep}user_code={code}"
