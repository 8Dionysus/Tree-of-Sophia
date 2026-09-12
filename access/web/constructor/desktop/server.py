#!/usr/bin/env python3
"""Private loopback delivery for one immutable Tree of Sophia demo release."""
from __future__ import annotations

import argparse
import hashlib
import json
import mimetypes
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlsplit

APP_ID = "tos-sophia-demo"
IDENTITY_ROUTE = "/__tos_demo_identity"


def display_files(release: Path) -> list[Path]:
    """Enumerate the same allowlist used by HTTP; never fingerprint private neighbors."""
    release = release.resolve(strict=True)
    files = [release / "constructor.html", release / "library.json"]
    assets = release / "assets"
    if not assets.is_dir() or assets.is_symlink():
        raise ValueError("Release must contain a real assets directory")
    for parent, dirs, names in os.walk(assets, followlinks=False):
        dirs[:] = sorted(name for name in dirs if not (Path(parent) / name).is_symlink())
        files.extend(Path(parent) / name for name in sorted(names))
        if len(files) > 10_000:
            raise ValueError("Release exceeds the 10,000-file desktop limit")
    for path in files:
        if path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(release):
            raise ValueError(f"Display file is missing, a symlink, or outside release: {path.name}")
    return sorted(files)


def release_digest(release: Path) -> str:
    release = release.resolve(strict=True)
    digest = hashlib.sha256()
    for path in display_files(release):
        name = path.relative_to(release).as_posix().encode()
        digest.update(len(name).to_bytes(4, "big")); digest.update(name)
        with path.open("rb") as stream:
            content = hashlib.file_digest(stream, "sha256").digest()
        digest.update(content)
    return digest.hexdigest()


def make_server(release: Path, port: int, expected_digest: str) -> ThreadingHTTPServer:
    release = release.resolve(strict=True)
    actual_digest = release_digest(release)
    if actual_digest != expected_digest:
        raise ValueError("Release content differs from the installed configuration; rebuild/review before changing it")
    allowed = {path.relative_to(release).as_posix(): path for path in display_files(release)}

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            # Binding loopback alone does not reject a foreign Host from DNS rebinding.
            if self.headers.get("Host") not in {f"127.0.0.1:{self.server.server_port}", f"localhost:{self.server.server_port}"}:
                self.send_error(403); return
            try:
                route = unquote(urlsplit(self.path).path)
                if route == IDENTITY_ROUTE:
                    data = json.dumps({"schema": "tos_demo_server_identity_v1", "app_id": APP_ID,
                        "release": str(release), "release_digest": actual_digest, "pid": os.getpid()}).encode()
                    self.respond(data, "application/json; charset=utf-8"); return
                name = route.lstrip("/") or "constructor.html"
                path = allowed.get(name)
                # Exact name allowlist rejects traversal, including normalized aliases.
                if path is None or path.is_symlink() or path.resolve() != path:
                    self.send_error(404); return
                data = path.read_bytes()
            except (OSError, ValueError):
                self.send_error(404); return
            mime = mimetypes.guess_type(path.name)[0] or "application/octet-stream"
            if mime.startswith("text/") or mime in {"application/json", "application/javascript"}:
                mime += "; charset=utf-8"
            self.respond(data, mime)

        def respond(self, data: bytes, mime: str):
            self.send_response(200)
            self.send_header("Content-Type", mime)
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.end_headers()
            try:
                self.wfile.write(data)
            except (BrokenPipeError, ConnectionResetError):
                pass

        def log_message(self, fmt, *args):
            # Normal requests do not create an unbounded access history.
            if args and str(args[1] if len(args) > 1 else "").startswith(("4", "5")):
                super().log_message(fmt, *args)

    server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    server.daemon_threads = True
    return server


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release", required=True, type=Path)
    parser.add_argument("--port", type=int, default=44339)
    parser.add_argument("--release-digest", required=True)
    parser.add_argument("--log", type=Path)
    args = parser.parse_args()
    if args.log:
        # A systemd --pipe wrapper may exit; the HTTP worker must never inherit its pipes.
        with open(os.devnull, "rb") as input_file, args.log.open("ab", buffering=0) as log:
            os.dup2(input_file.fileno(), 0); os.dup2(log.fileno(), 1); os.dup2(log.fileno(), 2)
    server = make_server(args.release, args.port, args.release_digest)
    print(json.dumps({"ready": True, "app_id": APP_ID, "release": str(args.release.resolve()),
                      "port": server.server_port, "pid": os.getpid()}), flush=True)
    try:
        server.serve_forever()
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
