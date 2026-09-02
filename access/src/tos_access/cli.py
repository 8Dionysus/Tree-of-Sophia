from __future__ import annotations

import argparse
import json
from pathlib import Path

from .core import ToSAccessCore
from .doctor import doctor_report, render_doctor


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="tos", description="Tree of Sophia standalone access platform")
    parser.add_argument("--root", type=Path, help="Tree-of-Sophia repository or standalone runtime-data root")
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
    return parser


def main(argv: list[str] | None = None) -> None:
    args = _parser().parse_args(argv)
    if args.command in {"doctor", "verify"}:
        report = doctor_report(
            tos_root=args.root,
            profile=getattr(args, "profile", "standalone"),
            require_mcp=args.command == "verify",
        )
        print(json.dumps(report, ensure_ascii=False, indent=2) if args.as_json else render_doctor(report))
        if not report["ok"]:
            raise SystemExit(1)
        return
    core = ToSAccessCore.discover(tos_root=args.root)
    if args.command == "serve":
        from .http_server import serve
        serve(core, host=args.host, port=args.port)
        return
    if args.command == "mcp":
        from .mcp_server import _run_server, build_server
        _run_server(build_server(tos_root=core.tos_root))
        return
    raise SystemExit(f"unknown command: {args.command}")
