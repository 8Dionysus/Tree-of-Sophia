#!/usr/bin/env python3
"""Serve only the local constructor's display files on loopback; no directory listing."""
import argparse
import mimetypes
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlsplit

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("directory", type=Path)
parser.add_argument("--port", type=int, default=44336)
args = parser.parse_args()
directory = args.directory.resolve(strict=True)


class ConstructorHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        route = unquote(urlsplit(self.path).path).lstrip("/") or "constructor.html"
        file = (directory / route).resolve()
        allowed = route in {"constructor.html", "library.json"} or (
            route.startswith("assets/") and file.is_relative_to(directory / "assets")
        )
        if not allowed or not file.is_relative_to(directory) or not file.is_file():
            self.send_error(404)
            return
        data = file.read_bytes()
        self.send_response(200)
        mime = mimetypes.guess_type(file.name)[0] or "application/octet-stream"
        self.send_header("Content-Type", mime + ("; charset=utf-8" if mime.startswith("text/") or mime in {"application/json", "application/javascript"} else ""))
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(data)


print(f"Local Tree of Sophia demo: http://127.0.0.1:{args.port}/constructor.html", flush=True)
ThreadingHTTPServer(("127.0.0.1", args.port), ConstructorHandler).serve_forever()
