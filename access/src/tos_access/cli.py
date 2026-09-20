from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .core import ToSAccessCore
from .doctor import doctor_report, render_doctor
from .query_store import QueryStoreRequired


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="tos", description="Tree of Sophia standalone access platform")
    parser.add_argument("--root", type=Path, help="Tree-of-Sophia repository or standalone runtime-data root")
    parser.add_argument("--prepared-read-model", type=Path,
                        help="Explicit local prepared SQLite publication; requires --prepared-binding")
    parser.add_argument("--prepared-binding", type=Path,
                        help="Owner-selected snapshot binding JSON file, not inferred from the database")
    parser.add_argument("--exploration-checkpoints", type=Path,
                        help="Separate local continuation store for an explicitly selected prepared reader")
    parser.add_argument("--source-inputs", type=Path,
                        help="Explicit retained source vector; requires --root and the matching prepared reader/binding")
    parser.add_argument("--source-local-text-selection", type=Path,
                        help="Explicit protected local text conditions; requires --source-inputs, never enables publication")
    sub = parser.add_subparsers(dest="command", required=True)
    doctor = sub.add_parser("doctor", help="Inspect data, web, contract, MCP, and integration readiness")
    doctor.add_argument("--json", action="store_true", dest="as_json")
    verify = sub.add_parser("verify", help="Fail unless a selected profile is ready")
    verify.add_argument("--profile", choices=("standalone", "abyssos"), default="standalone")
    verify.add_argument("--json", action="store_true", dest="as_json")
    web = sub.add_parser("serve", help="Serve the read-only site and JSON API")
    web.add_argument("--host", default="127.0.0.1")
    web.add_argument("--port", type=int, default=8080)
    sub.add_parser("mcp", help="Run the native Tree of Sophia MCP server")
    reading = sub.add_parser(
        "reading-search",
        help="Read Zarathustra source candidates with occurrence-bound speakers and formulas",
    )
    reading.add_argument("--query", required=True)
    reading.add_argument("--language", choices=("de", "ru", "en"), default="ru")
    reading.add_argument("--limit", type=int, default=20)
    reading.add_argument("--group-by", default="speaker,formula")
    reading.add_argument("--include-semantic-neighbors", action="store_true")
    source = sub.add_parser("source", help="Read exact owner-bound source records without changing the Tree")
    source_sub = source.add_subparsers(dest="source_command", required=True)
    source_sub.add_parser("capabilities")
    source_sub.add_parser("contracts")
    for operation in ("discover", "read"):
        source_sub.add_parser(operation).add_argument("request", help="Bounded source-read request JSON file or - for stdin")
    knowledge = sub.add_parser("knowledge", help="Use the unified human/agent knowledge backend")
    knowledge_sub = knowledge.add_subparsers(dest="knowledge_command", required=True)
    knowledge_sub.add_parser("catalog", help="List fields, vocabularies, limits, and stored lenses")
    knowledge_sub.add_parser("contracts", help="Return the API map and JSON Schemas used by the constructor")
    knowledge_sub.add_parser("search-capabilities", help="Describe selected search engines without building a graph")
    search = knowledge_sub.add_parser("search", help="Search normalized nodes and relations")
    search.add_argument("query", nargs="?", default="")
    search.add_argument("--sources", nargs="*")
    search.add_argument("--kind", action="append", dest="kind_ids")
    search.add_argument("--predicate", action="append", dest="predicate_ids")
    search.add_argument("--offset", type=int, default=0)
    search.add_argument("--limit", type=int, default=40)
    search.add_argument("--mode", choices=("legacy", "indexed", "compressed"), default="legacy")
    search.add_argument("--cursor")
    node = knowledge_sub.add_parser("node", help="Inspect one normalized node")
    node.add_argument("node_id")
    node.add_argument("--relation-limit", type=int, default=200)
    relation = knowledge_sub.add_parser("relation", help="Inspect one normalized relation")
    relation.add_argument("relation_id")
    compare = knowledge_sub.add_parser("temporal-compare", help="Compare two exact Claim date envelopes; JSON file or - for stdin")
    compare.add_argument("request")
    focus = knowledge_sub.add_parser("focus", help="Build a bounded radial lens around one node")
    focus.add_argument("node_id")
    focus.add_argument("--sources", nargs="*")
    focus.add_argument("--depth", type=int, default=1)
    focus.add_argument("--direction", choices=("outgoing", "incoming", "either"), default="either")
    focus.add_argument("--predicate", action="append", dest="predicate_ids")
    focus.add_argument("--node-limit", type=int, default=200)
    focus.add_argument("--relation-limit", type=int, default=400)
    focus.add_argument("--profile", choices=("overview", "all"), default="overview")
    lens = sub.add_parser("lens", help="Compile or open declarative knowledge lenses")
    lens_sub = lens.add_subparsers(dest="lens_command", required=True)
    compile_lens = lens_sub.add_parser("compile", help="Compile a LensSpec JSON file; use - for stdin")
    compile_lens.add_argument("spec")
    open_lens = lens_sub.add_parser("open", help="Open a stored LensSpec by ID")
    open_lens.add_argument("lens_id")
    return parser


def main(argv: list[str] | None = None) -> None:
    args = _parser().parse_args(argv)
    prepared = args.prepared_read_model is not None or args.prepared_binding is not None
    if prepared and (args.prepared_read_model is None or args.prepared_binding is None):
        raise SystemExit("prepared reads require both --prepared-read-model and --prepared-binding")
    if args.exploration_checkpoints is not None and not prepared:
        raise SystemExit("--exploration-checkpoints requires an explicitly selected prepared reader")
    if args.source_inputs is not None and (not prepared or args.root is None):
        raise SystemExit("--source-inputs requires --root and an explicitly selected prepared reader/binding")
    if args.source_local_text_selection is not None and args.source_inputs is None:
        raise SystemExit("--source-local-text-selection requires --source-inputs")
    if args.command in {"doctor", "verify"}:
        if prepared:
            raise SystemExit("doctor/verify check the source-backed profile, not a prepared publication")
        report = doctor_report(
            tos_root=args.root,
            profile=getattr(args, "profile", "standalone"),
            require_mcp=args.command == "verify",
        )
        print(json.dumps(report, ensure_ascii=False, indent=2) if args.as_json else render_doctor(report))
        if not report["ok"]:
            raise SystemExit(1)
        return
    options = {}
    if prepared:
        from .published_read_model import PublishedReadModelError, _json
        try:
            with args.prepared_binding.open("rb") as stream:
                raw = stream.read(65_537)
            if len(raw) > 65_536:
                raise ValueError("prepared binding file exceeds 65536 bytes")
            binding = _json(raw.decode("utf-8"))
        except (OSError, UnicodeError, ValueError, PublishedReadModelError) as exc:
            raise SystemExit(f"cannot read owner-selected prepared binding: {exc}") from exc
        options = {"published_read_model_path": args.prepared_read_model,
                   "published_read_model_expected": binding,
                   "published_exploration_checkpoint_path": args.exploration_checkpoints}
    if args.source_inputs is not None:
        from .source_read_owner import SelectedSourceReadService
        try:
            local_options = ({"local_text_selection": args.source_local_text_selection}
                             if args.source_local_text_selection is not None else {})
            options["source_read_service"] = SelectedSourceReadService(
                args.root, args.source_inputs, expected_revision=binding["source_revision"], **local_options)
        except (OSError, ValueError, KeyError, ImportError) as exc:
            raise SystemExit(f"cannot select source owner: {exc}") from exc
    core = ToSAccessCore.discover(tos_root=args.root, **options)
    if args.command == "reading-search":
        print(json.dumps(core.zarathustra_reading_search(
            args.query,
            args.language,
            args.limit,
            args.include_semantic_neighbors,
            [item.strip() for item in args.group_by.split(",") if item.strip()],
        ), ensure_ascii=False, indent=2))
        return
    if args.command == "serve":
        from .http_server import serve
        serve(core, host=args.host, port=args.port)
        return
    if args.command == "mcp":
        from .mcp_server import _run_server, build_server
        # A released snapshot is already verified by this core. Keep its guard
        # for the server lifetime, as HTTP does: reopening it on every MCP call
        # repeats the full artifact hash and SQLite integrity scan. The guard
        # still checks revocation and each selected member on every request.
        _run_server(build_server(core=core) if prepared or core._data_guard is not None
                    else build_server(tos_root=core.tos_root))
        return
    if args.command == "source":
        if args.source_command == "capabilities":
            packet = core.source_read_capabilities()
        elif args.source_command == "contracts":
            packet = core.source_read_contract()
        else:
            from .source_read import MAX_REQUEST_BYTES, SourceReadError
            try:
                if args.request == "-":
                    raw = sys.stdin.buffer.read(MAX_REQUEST_BYTES + 1)
                else:
                    with Path(args.request).open("rb") as stream:
                        raw = stream.read(MAX_REQUEST_BYTES + 1)
                if len(raw) > MAX_REQUEST_BYTES:
                    raise ValueError("source request exceeds 65536 bytes")
                from .projection_store import _strict_json
                request = _strict_json(raw)
                packet = (core.source_handle_discover(request) if args.source_command == "discover"
                          else core.source_read(request))
            except (OSError, ValueError, SourceReadError) as exc:
                raise SystemExit(f"cannot read exact source: {exc}") from exc
        print(json.dumps(packet, ensure_ascii=False, separators=(",", ":"), allow_nan=False))
        return
    if args.command == "knowledge":
        try:
            if args.knowledge_command == "catalog":
                packet = core.knowledge_catalog()
            elif args.knowledge_command == "contracts":
                packet = core.knowledge_contracts()
            elif args.knowledge_command == "search-capabilities":
                packet = core.knowledge_search_capabilities()
            elif args.knowledge_command == "search":
                if args.mode in {"indexed", "compressed"}:
                    if args.offset:
                        raise SystemExit(f"{args.mode} knowledge search uses --cursor, not --offset")
                    search = core.knowledge_search_indexed if args.mode == "indexed" else core.knowledge_search_compressed
                    packet = search(
                        args.query,
                        sources=args.sources,
                        kind_ids=args.kind_ids,
                        predicate_ids=args.predicate_ids,
                        cursor=args.cursor,
                        limit=args.limit,
                    )
                else:
                    packet = core.knowledge_search(
                        args.query,
                        sources=args.sources,
                        kind_ids=args.kind_ids,
                        predicate_ids=args.predicate_ids,
                        offset=args.offset,
                        limit=args.limit,
                    )
            elif args.knowledge_command == "node":
                packet = core.knowledge_node(args.node_id, args.relation_limit)
            elif args.knowledge_command == "relation":
                packet = core.knowledge_relation(args.relation_id)
            elif args.knowledge_command == "temporal-compare":
                raw = sys.stdin.read() if args.request == "-" else Path(args.request).read_text(encoding="utf-8")
                packet = core.knowledge_temporal_compare(json.loads(raw))
            elif args.knowledge_command == "focus":
                packet = core.knowledge_focus(
                    args.node_id,
                    sources=args.sources,
                    depth=args.depth,
                    direction=args.direction,
                    predicate_ids=args.predicate_ids,
                    node_limit=args.node_limit,
                    relation_limit=args.relation_limit,
                    profile=args.profile,
                )
            else:  # pragma: no cover - argparse owns this branch
                raise SystemExit(f"unknown knowledge command: {args.knowledge_command}")
        except QueryStoreRequired as exc:
            raise SystemExit(str(exc)) from exc
        if packet.get("schema") == "tos_knowledge_search_compressed_v3":
            print(json.dumps(packet, ensure_ascii=False, separators=(",", ":"), allow_nan=False))
        else:
            print(json.dumps(packet, ensure_ascii=False, indent=2))
        return
    if args.command == "lens":
        try:
            if args.lens_command == "open":
                packet = core.stored_knowledge_lens(args.lens_id)
            elif args.lens_command == "compile":
                raw = sys.stdin.read() if args.spec == "-" else Path(args.spec).read_text(encoding="utf-8")
                spec = json.loads(raw)
                if not isinstance(spec, dict):
                    raise SystemExit("LensSpec JSON must be an object")
                packet = core.compile_knowledge_lens(spec)
            else:  # pragma: no cover - argparse owns this branch
                raise SystemExit(f"unknown lens command: {args.lens_command}")
        except QueryStoreRequired as exc:
            raise SystemExit(str(exc)) from exc
        print(json.dumps(packet, ensure_ascii=False, indent=2))
        return
    raise SystemExit(f"unknown command: {args.command}")
