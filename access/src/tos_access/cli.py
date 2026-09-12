from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .core import ToSAccessCore
from .doctor import doctor_report, render_doctor


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="tos", description="Tree of Sophia standalone access platform")
    parser.add_argument("--root", type=Path, help="Tree-of-Sophia repository or standalone runtime-data root")
    parser.add_argument("--prepared-read-model", type=Path,
                        help="Explicit local prepared SQLite publication; requires --prepared-binding")
    parser.add_argument("--prepared-binding", type=Path,
                        help="Owner-selected snapshot binding JSON file, not inferred from the database")
    parser.add_argument("--exploration-checkpoints", type=Path,
                        help="Separate local continuation store for an explicitly selected prepared reader")
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
    core = ToSAccessCore.discover(tos_root=args.root, **options)
    if args.command == "serve":
        from .http_server import serve
        serve(core, host=args.host, port=args.port)
        return
    if args.command == "mcp":
        from .mcp_server import _run_server, build_server
        _run_server(build_server(core=core) if prepared else build_server(tos_root=core.tos_root))
        return
    if args.command == "knowledge":
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
        if packet.get("schema") == "tos_knowledge_search_compressed_v3":
            print(json.dumps(packet, ensure_ascii=False, separators=(",", ":"), allow_nan=False))
        else:
            print(json.dumps(packet, ensure_ascii=False, indent=2))
        return
    if args.command == "lens":
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
        print(json.dumps(packet, ensure_ascii=False, indent=2))
        return
    raise SystemExit(f"unknown command: {args.command}")
