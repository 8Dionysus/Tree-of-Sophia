from __future__ import annotations

import csv
import io
import json
import mimetypes
import secrets
import socket
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, unquote, urlparse

from .core import ToSAccessCore
from .query_store import QueryStoreRequired
from .lens_pagination import KnowledgeRevisionConflict
from .exploration import ExplorationExpired, exploration_capabilities
from .doctor import web_root_for

LOOPBACK_HOSTS = {"127.0.0.1", "localhost", "::1"}
MAX_LENS_REQUEST_BYTES = 64 * 1024

INDEX_TEMPLATE = """<!doctype html>
<html lang="ru"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Древо Софии</title><link rel="stylesheet" href="/static/assets/tos-graph.css"></head>
<body><div id="app"></div><script nonce="__CSP_NONCE__">window.__TOS_GRAPH_BOOT__=__BOOT__;</script>
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


def _boolean(query: dict[str, list[str]], key: str, default: bool = False) -> bool:
    raw = _single(query, key, "true" if default else "false").strip().lower()
    if raw in {"1", "true", "yes", "on"}:
        return True
    if raw in {"0", "false", "no", "off"}:
        return False
    raise ValueError(f"{key} must be a boolean")


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


def _security_headers(csp_nonce: str | None = None) -> dict[str, str]:
    script_source = "'self'"
    if csp_nonce:
        script_source += f" 'nonce-{csp_nonce}'"
    return {
        "Content-Security-Policy": "; ".join(
            (
                "default-src 'self'",
                "base-uri 'none'",
                "connect-src 'self'",
                "font-src 'self'",
                "form-action 'self'",
                "frame-ancestors 'none'",
                "img-src 'self' data:",
                "object-src 'none'",
                f"script-src {script_source}",
                "style-src 'self'",
                "worker-src 'self'",
            )
        ),
        # WebMCP's permission feature is `tools`; self is allowed while
        # cross-origin iframe delegation remains disabled.
        "Permissions-Policy": "tools=(self), accelerometer=(), camera=(), geolocation=(), gyroscope=(), microphone=(), payment=(), usb=()",
        "Cross-Origin-Opener-Policy": "same-origin",
        "Cross-Origin-Embedder-Policy": "require-corp",
        "Cross-Origin-Resource-Policy": "same-origin",
        "Origin-Agent-Cluster": "?1",
        "Referrer-Policy": "no-referrer",
        "X-Frame-Options": "DENY",
    }


def _scale_rows(core: ToSAccessCore, table: str, view_id: str | None, layers: list[str]) -> list[dict[str, Any]]:
    return core.philosophy_scale_rows(table, view_id=view_id, layers=layers)


def build_handler(core: ToSAccessCore, web_root: Path) -> type[BaseHTTPRequestHandler]:
    class Handler(BaseHTTPRequestHandler):
        server_version = "TreeOfSophiaAccess/0.1"

        def log_message(self, format: str, *args: Any) -> None:
            return

        def _send(self, body: bytes, content_type: str, status: int = 200, *, csp_nonce: str | None = None) -> None:
            self.send_response(status)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store" if content_type.startswith("application/json") else "no-cache")
            self.send_header("X-Content-Type-Options", "nosniff")
            for name, value in _security_headers(csp_nonce).items():
                self.send_header(name, value)
            self.end_headers()
            if self.command == "HEAD":
                return
            try:
                self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError):
                # A browser cancellation can close an in-flight response.
                # The request is already gone; do not turn it into a server
                # traceback or a misleading product failure.
                return

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
                    nonce = secrets.token_urlsafe(18)
                    html = INDEX_TEMPLATE.replace("__CSP_NONCE__", nonce).replace("__BOOT__", json.dumps(_boot_payload(core), ensure_ascii=False))
                    self._send(html.encode("utf-8"), "text/html; charset=utf-8", csp_nonce=nonce)
                    return
                if path.startswith("/static/"):
                    self._static(path.removeprefix("/static/"))
                    return
                if path == "/health":
                    errors: list[str] = []
                    knowledge_schema: str | None = None
                    knowledge_counts: dict[str, Any] = {}
                    try:
                        index = core.corpus_header()
                        if index.get("schema_version") != "tos_corpus_index_v1":
                            errors.append("unsupported corpus index schema")
                        else:
                            corpus_views = core.status().get("graph_views", [])
                            if not corpus_views:
                                errors.append("corpus index has no supported graph views")
                            else:
                                core.graph_view(str(corpus_views[0]), limit=1)
                    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as exc:
                        errors.append(f"corpus index invalid: {exc}")
                    try:
                        philosophy = core.philosophy_projection()
                        views = [
                            view
                            for view in philosophy.get("views", [])
                            if isinstance(view, dict) and view.get("view_id")
                        ]
                        if not views:
                            errors.append("philosophy projection has no graph views")
                        else:
                            core.philosophy_view(str(views[0]["view_id"]), limit=1)
                    except (KeyError, OSError, RuntimeError, ValueError, json.JSONDecodeError) as exc:
                        errors.append(f"philosophy projection invalid: {exc}")
                    try:
                        knowledge = core.knowledge_header()
                        catalog = core.knowledge_catalog()
                        counts = knowledge.get("counts", {})
                        coverage = counts.get("display_coverage", {}) if isinstance(counts, dict) else {}
                        knowledge_schema = str(knowledge.get("schema") or "")
                        knowledge_counts = counts if isinstance(counts, dict) else {}
                        if catalog.get("schema") != "tos_knowledge_catalog_v1":
                            errors.append("knowledge catalog schema is not current")
                        if coverage.get("node_titles") != counts.get("nodes"):
                            errors.append("knowledge graph node title coverage is incomplete")
                        if coverage.get("node_summaries") != counts.get("nodes"):
                            errors.append("knowledge graph node summary coverage is incomplete")
                        if coverage.get("relation_labels") != counts.get("relations"):
                            errors.append("knowledge graph relation label coverage is incomplete")
                        if coverage.get("relation_statements") != counts.get("relations"):
                            errors.append("knowledge graph relation statement coverage is incomplete")
                        if coverage.get("relation_explanations") != counts.get("relations"):
                            errors.append("knowledge graph relation explanation coverage is incomplete")
                    except (KeyError, OSError, RuntimeError, ValueError, json.JSONDecodeError) as exc:
                        errors.append(f"knowledge graph invalid: {exc}")
                    health = {
                        "service": "tree-of-sophia-access",
                        "ok": not errors,
                        "write_enabled": False,
                        "errors": errors,
                        "knowledge_schema": knowledge_schema,
                        "knowledge_counts": knowledge_counts,
                    }
                    self._json(health, HTTPStatus.OK if not errors else HTTPStatus.SERVICE_UNAVAILABLE)
                    return
                if path == "/api/corpus/status": self._json(core.status()); return
                if path == "/api/knowledge/catalog": self._json(core.knowledge_catalog()); return
                if path == "/api/knowledge/explore/capabilities": self._json(exploration_capabilities()); return
                if path == "/api/knowledge/explore/contracts": self._json(core.knowledge_exploration_contracts()); return
                if path == "/api/knowledge/contracts": self._json(core.knowledge_contracts()); return
                if path == "/api/knowledge/search":
                    self._json(core.knowledge_search(
                        _single(query, "query"),
                        sources=_list(query, "sources") or None,
                        kind_ids=_list(query, "kind_ids") or None,
                        predicate_ids=_list(query, "predicate_ids") or None,
                        offset=_integer(query, "offset", 0, 0, 100_000),
                        limit=_integer(query, "limit", 40, 1, 100),
                    )); return
                if path.startswith("/api/knowledge/nodes/"):
                    self._json(core.knowledge_node(
                        unquote(path.removeprefix("/api/knowledge/nodes/")),
                        _integer(query, "relation_limit", 200, 0, 1000),
                    )); return
                if path.startswith("/api/knowledge/relations/"):
                    self._json(core.knowledge_relation(unquote(path.removeprefix("/api/knowledge/relations/")))); return
                if path.startswith("/api/knowledge/focus/"):
                    self._json(core.knowledge_focus(
                        unquote(path.removeprefix("/api/knowledge/focus/")),
                        sources=_list(query, "sources") or None,
                        depth=_integer(query, "depth", 1, 0, 5),
                        direction=_single(query, "direction", "either"),
                        profile=_single(query, "profile", "overview"),
                        predicate_ids=_list(query, "predicates") or None,
                        node_limit=_integer(query, "node_limit", 200, 1, 1000),
                        relation_limit=_integer(query, "relation_limit", 400, 0, 2000),
                    )); return
                if path.startswith("/api/knowledge/lenses/"):
                    self._json(core.stored_knowledge_lens(unquote(path.removeprefix("/api/knowledge/lenses/")))); return
                if path == "/api/source-gaps": self._json(core.source_gap_search(_single(query, "query"), _integer(query, "limit", 20, 1, 100))); return
                if path == "/api/zarathustra/word-analysis":
                    self._json(core.zarathustra_word_analysis_task(
                        _single(query, "query"),
                        _single(query, "language", "ru"),
                        _integer(query, "rank", 1, 1, 100),
                        _boolean(query, "include_semantic_neighbors"),
                    )); return
                if path == "/api/corpus/summary": self._json(core.summary()); return
                if path == "/api/corpus/search": self._json(core.search(_single(query, "query"), _integer(query, "limit", 20, 1, 100))); return
                if path.startswith("/api/source/navigation/"):
                    self._json(core.source_descend(
                        unquote(path.removeprefix("/api/source/navigation/")),
                        _integer(query, "max_depth", 8, 1, 8),
                        _integer(query, "limit", 300, 1, 300),
                    )); return
                if path.startswith("/api/source/dossiers/"):
                    self._json(core.source_dossier(
                        unquote(path.removeprefix("/api/source/dossiers/")),
                        _integer(query, "limit", 300, 1, 300),
                    )); return
                if path.startswith("/api/corpus/graph-views/"):
                    self._json(core.graph_view(unquote(path.removeprefix("/api/corpus/graph-views/")), _integer(query, "limit", 100, 1, 1000))); return
                if path.startswith("/api/corpus/query/epistemic/"):
                    packet = core.evidence_lens_packet(
                        "corpus",
                        unquote(path.removeprefix("/api/corpus/query/epistemic/")),
                        _single(query, "view_id") or "route-graph",
                        _integer(query, "limit", 80, 1, 200),
                    ); self._json(packet); return
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
                if path.startswith("/api/philosophy/query/epistemic/"):
                    packet = core.evidence_lens_packet(
                        "philosophy",
                        unquote(path.removeprefix("/api/philosophy/query/epistemic/")),
                        _single(query, "view_id") or None,
                        _integer(query, "limit", 80, 1, 200),
                    ); self._json(packet); return
                if path == "/api/philosophy/query/paths":
                    packet = core.philosophy_path_between(
                        _single(query, "from"),
                        _single(query, "to"),
                        _list(query, "layers"),
                        _list(query, "predicates"),
                        _integer(query, "max_depth", 6, 1, 8),
                        _single(query, "direction", "outgoing"),
                        _single(query, "view_id") or None,
                        _list(query, "exclude"),
                        _integer(query, "alternatives", 1, 1, 5),
                    ); packet["query_backend"] = "json"; self._json(packet); return
                if path.startswith("/api/philosophy/neighborhood/"): self._json(core.philosophy_neighborhood(unquote(path.removeprefix("/api/philosophy/neighborhood/")), _integer(query, "depth", 1, 1, 3), _list(query, "layers"), _list(query, "predicates"), _integer(query, "limit", 80, 1, 300))); return
                if path == "/api/philosophy/paths":
                    self._json(core.philosophy_path_between(
                        _single(query, "from"),
                        _single(query, "to"),
                        _list(query, "layers"),
                        _list(query, "predicates"),
                        _integer(query, "max_depth", 6, 1, 8),
                        _single(query, "direction", "outgoing"),
                        _single(query, "view_id") or None,
                        _list(query, "exclude"),
                        _integer(query, "alternatives", 1, 1, 5),
                    )); return
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
            except QueryStoreRequired as exc:
                self._json({'error': str(exc), 'code': 'query_store_build_required'}, HTTPStatus.SERVICE_UNAVAILABLE)
            except KeyError as exc:
                self._json({"error": str(exc)}, HTTPStatus.NOT_FOUND)
            except (RuntimeError, ValueError) as exc:
                self._json({"error": str(exc)}, HTTPStatus.BAD_REQUEST)

        def do_HEAD(self) -> None:  # noqa: N802
            self.do_GET()

        def do_POST(self) -> None:  # noqa: N802
            parsed = urlparse(self.path)
            if parsed.path not in {"/api/knowledge/lenses/compile", "/api/knowledge/explore"}:
                self._json({"error": "standalone access is read-only"}, HTTPStatus.METHOD_NOT_ALLOWED)
                return
            try:
                if self.headers.get("Transfer-Encoding"):
                    raise ValueError("streamed lens requests are not supported")
                content_type = self.headers.get_content_type()
                if content_type != "application/json":
                    self._json({"error": "lens request must use application/json"}, HTTPStatus.UNSUPPORTED_MEDIA_TYPE)
                    return
                raw_length = self.headers.get("Content-Length")
                if raw_length is None:
                    raise ValueError("lens request requires Content-Length")
                length = int(raw_length)
                if length < 1:
                    raise ValueError(f"lens request must be between 1 and {MAX_LENS_REQUEST_BYTES} bytes")
                if length > MAX_LENS_REQUEST_BYTES:
                    self._json(
                        {"error": f"lens request must not exceed {MAX_LENS_REQUEST_BYTES} bytes"},
                        HTTPStatus.REQUEST_ENTITY_TOO_LARGE,
                    )
                    return
                body = self.rfile.read(length)
                if len(body) != length:
                    raise ValueError("incomplete lens request body")
                spec = json.loads(body.decode("utf-8"))
                if not isinstance(spec, dict):
                    raise ValueError("lens spec must be an object")
                operation = core.knowledge_explore if parsed.path == "/api/knowledge/explore" else core.compile_knowledge_lens
                self._json(operation(spec))
            except QueryStoreRequired as exc:
                self._json({'error': str(exc), 'code': 'query_store_build_required'}, HTTPStatus.SERVICE_UNAVAILABLE)
            except ExplorationExpired as exc:
                self._json({"error": str(exc), "code": "exploration_expired"}, HTTPStatus.GONE)
            except KnowledgeRevisionConflict as exc:
                self._json({"error": str(exc)}, HTTPStatus.CONFLICT)
            except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as exc:
                self._json({"error": str(exc)}, HTTPStatus.BAD_REQUEST)

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
