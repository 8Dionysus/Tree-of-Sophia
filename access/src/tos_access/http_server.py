from __future__ import annotations

import csv
import io
import json
import mimetypes
import socket
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, unquote, urlparse

from .core import ToSAccessCore
from .doctor import web_root_for

LOOPBACK_HOSTS = {"127.0.0.1", "localhost", "::1"}

INDEX_TEMPLATE = """<!doctype html>
<html lang="ru"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Древо Софии</title><link rel="stylesheet" href="/static/assets/tos-graph.css"></head>
<body><div id="app"></div><script>window.__TOS_GRAPH_BOOT__=__BOOT__;</script>
<script type="module" src="/static/assets/tos-graph.js"></script></body></html>"""


def _single(query: dict[str, list[str]], key: str, default: str = "") -> str:
    values = query.get(key, [])
    return values[0] if values else default


def _integer(query: dict[str, list[str]], key: str, default: int, low: int, high: int) -> int:
    try:
        value = int(_single(query, key, str(default)))
    except ValueError:
        value = default
    return max(low, min(high, value))


def _list(query: dict[str, list[str]], key: str) -> list[str]:
    return [item for item in _single(query, key).split(",") if item]


def _boot_payload(core: ToSAccessCore) -> dict[str, Any]:
    corpus = core.summary()
    philosophy = core.philosophy_views()
    return {
        "service": "tree-of-sophia-access",
        "default_view": next((str(item.get("view_id")) for item in corpus.get("graph_views", []) if item.get("view_id")), ""),
        "default_philosophy_view": next((str(item.get("view_id")) for item in philosophy.get("views", []) if item.get("view_id")), "chronology"),
        "write_enabled": False,
        "projection_mode": "json",
        "neo4j": {"configured": False, "ready": False, "note": "Standalone JSON backend"},
    }


def _scale_rows(core: ToSAccessCore, table: str, view_id: str | None, layers: list[str]) -> list[dict[str, Any]]:
    return core.philosophy_scale_rows(table, view_id=view_id, layers=layers)


def build_handler(core: ToSAccessCore, web_root: Path) -> type[BaseHTTPRequestHandler]:
    class Handler(BaseHTTPRequestHandler):
        server_version = "TreeOfSophiaAccess/0.1"

        def log_message(self, format: str, *args: Any) -> None:
            return

        def _send(self, body: bytes, content_type: str, status: int = 200) -> None:
            self.send_response(status)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store" if content_type.startswith("application/json") else "no-cache")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.end_headers()
            self.wfile.write(body)

        def _json(self, payload: Any, status: int = 200) -> None:
            self._send(json.dumps(payload, ensure_ascii=False).encode("utf-8"), "application/json; charset=utf-8", status)

        def _static(self, relative: str) -> None:
            target = (web_root / relative).resolve()
            if web_root.resolve() not in target.parents or not target.is_file():
                self._json({"error": "not found"}, HTTPStatus.NOT_FOUND)
                return
            kind = mimetypes.guess_type(target.name)[0] or "application/octet-stream"
            self._send(target.read_bytes(), kind)

        def do_GET(self) -> None:  # noqa: N802
            parsed = urlparse(self.path)
            query = parse_qs(parsed.query)
            path = parsed.path
            try:
                if path == "/":
                    html = INDEX_TEMPLATE.replace("__BOOT__", json.dumps(_boot_payload(core), ensure_ascii=False))
                    self._send(html.encode("utf-8"), "text/html; charset=utf-8")
                    return
                if path.startswith("/static/"):
                    self._static(path.removeprefix("/static/"))
                    return
                if path == "/health":
                    self._json({"service": "tree-of-sophia-access", "ok": core.index_exists() and core.philosophy_projection_exists(), "write_enabled": False})
                    return
                if path == "/api/corpus/status": self._json(core.status()); return
                if path == "/api/corpus/summary": self._json(core.summary()); return
                if path == "/api/corpus/search": self._json(core.search(_single(query, "query"), _integer(query, "limit", 20, 1, 100))); return
                if path.startswith("/api/corpus/graph-views/"):
                    self._json(core.graph_view(unquote(path.removeprefix("/api/corpus/graph-views/")), _integer(query, "limit", 100, 1, 1000))); return
                if path.startswith("/api/corpus/nodes/"): self._json(core.node(unquote(path.removeprefix("/api/corpus/nodes/")))); return
                if path.startswith("/api/corpus/relation-packs/"): self._json(core.relation_pack(unquote(path.removeprefix("/api/corpus/relation-packs/")))); return
                if path == "/api/philosophy/status": self._json(core.philosophy_status()); return
                if path == "/api/philosophy/views": self._json(core.philosophy_views()); return
                if path == "/api/philosophy/layers": self._json(core.philosophy_layers()); return
                if path == "/api/philosophy/contracts": self._json(core.philosophy_contracts()); return
                if path == "/api/philosophy/clusters": self._json(core.philosophy_clusters(_single(query, "view_id") or None, _single(query, "kind") or None, _integer(query, "limit", 80, 1, 1000))); return
                if path == "/api/philosophy/review-packet": self._json(core.philosophy_review_packet(_single(query, "view_id", "chronology"))); return
                if path == "/api/philosophy/snapshot": self._json(core.philosophy_snapshot()); return
                if path == "/api/philosophy/audit": self._json(core.philosophy_audit()); return
                if path == "/api/philosophy/unresolved": self._json(core.philosophy_unresolved(_single(query, "view_id") or None)); return
                if path == "/api/philosophy/search": self._json(core.philosophy_search(_single(query, "query"), _integer(query, "limit", 40, 1, 100))); return
                if path == "/api/philosophy/packet": self._json(core.philosophy_packet(_single(query, "query"), _single(query, "view_id") or None, _integer(query, "limit", 20, 1, 100))); return
                if path.startswith("/api/philosophy/query/neighborhood/"):
                    packet = core.philosophy_neighborhood(unquote(path.removeprefix("/api/philosophy/query/neighborhood/")), _integer(query, "depth", 1, 1, 3), _list(query, "layers"), _list(query, "predicates"), _integer(query, "limit", 80, 1, 300)); packet["query_backend"] = "json"; self._json(packet); return
                if path == "/api/philosophy/query/paths":
                    packet = core.philosophy_path_between(_single(query, "from"), _single(query, "to"), _list(query, "layers"), _list(query, "predicates"), _integer(query, "max_depth", 6, 1, 8)); packet["query_backend"] = "json"; self._json(packet); return
                if path.startswith("/api/philosophy/neighborhood/"): self._json(core.philosophy_neighborhood(unquote(path.removeprefix("/api/philosophy/neighborhood/")), _integer(query, "depth", 1, 1, 3), _list(query, "layers"), _list(query, "predicates"), _integer(query, "limit", 80, 1, 300))); return
                if path == "/api/philosophy/paths": self._json(core.philosophy_path_between(_single(query, "from"), _single(query, "to"), _list(query, "layers"), _list(query, "predicates"), _integer(query, "max_depth", 6, 1, 8))); return
                if path.startswith("/api/philosophy/nodes/"): self._json(core.philosophy_node(unquote(path.removeprefix("/api/philosophy/nodes/")))); return
                if path.startswith("/api/philosophy/edges/"): self._json(core.philosophy_edge(unquote(path.removeprefix("/api/philosophy/edges/")))); return
                if path == "/api/philosophy/scale-export/manifest": self._json(core.philosophy_scale_manifest(_single(query, "view_id") or None, _list(query, "layers"))); return
                if path.startswith("/api/philosophy/scale-export/"):
                    name = path.removeprefix("/api/philosophy/scale-export/")
                    table, separator, file_format = name.rpartition(".")
                    if not separator or file_format not in {"jsonl", "csv"}: raise KeyError("unknown scale export format")
                    rows = _scale_rows(core, table, _single(query, "view_id") or None, _list(query, "layers"))
                    if file_format == "jsonl":
                        body = "".join(json.dumps(row, ensure_ascii=False) + "\n" for row in rows).encode("utf-8")
                        self._send(body, "application/x-ndjson; charset=utf-8"); return
                    columns = sorted({key for row in rows for key in row})
                    stream = io.StringIO(); writer = csv.DictWriter(stream, fieldnames=columns); writer.writeheader()
                    writer.writerows({key: json.dumps(value, ensure_ascii=False) if isinstance(value, (dict, list)) else value for key, value in row.items()} for row in rows)
                    self._send(stream.getvalue().encode("utf-8"), "text/csv; charset=utf-8"); return
                if path.startswith("/api/philosophy/views/"):
                    view_id = unquote(path.removeprefix("/api/philosophy/views/").split("/", 1)[0]); self._json(core.philosophy_view(view_id, _integer(query, "limit", 1000, 1, 1000))); return
                self._json({"error": "not found", "path": path}, HTTPStatus.NOT_FOUND)
            except KeyError as exc:
                self._json({"error": str(exc)}, HTTPStatus.NOT_FOUND)
            except (RuntimeError, ValueError) as exc:
                self._json({"error": str(exc)}, HTTPStatus.BAD_REQUEST)

        def do_POST(self) -> None:  # noqa: N802
            self._json({"error": "standalone access is read-only"}, HTTPStatus.METHOD_NOT_ALLOWED)

    return Handler


def make_server(core: ToSAccessCore, host: str = "127.0.0.1", port: int = 8080) -> ThreadingHTTPServer:
    if host not in LOOPBACK_HOSTS:
        raise ValueError("standalone HTTP host must remain loopback-only")
    web_root = web_root_for(core)
    if web_root is None:
        raise RuntimeError("missing built web assets; run npm --prefix access/web run build")
    if host == "::1":
        class IPv6ThreadingHTTPServer(ThreadingHTTPServer):
            address_family = socket.AF_INET6

        server_class = IPv6ThreadingHTTPServer
    else:
        server_class = ThreadingHTTPServer
    return server_class((host, port), build_handler(core, web_root))


def serve(core: ToSAccessCore, host: str = "127.0.0.1", port: int = 8080) -> None:
    server = make_server(core, host=host, port=port)
    print(f"Tree of Sophia access listening on http://{host}:{server.server_port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
