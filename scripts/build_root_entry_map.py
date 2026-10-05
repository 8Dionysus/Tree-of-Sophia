#!/usr/bin/env python3
"""Build the ToS root entry capsule."""

from __future__ import annotations

# Maintained command/main dispatch; pure imported helpers remain migration
# reference APIs until receiving native CLI acceptance. No Python fallback.
import argparse as _native_argparse
import os as _native_os
from pathlib import Path as _NativePath
import shutil as _native_shutil
import sys as _native_sys


def native_main(argv: list[str] | None = None) -> int:
    parser = _native_argparse.ArgumentParser(description="Native owned route operation")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--kag-export", type=_NativePath, help="explicit verified source-return export for runtime export_ref resolution")
    args = parser.parse_args(argv)
    if args.kag_export is not None and not args.kag_export.is_absolute():
        parser.error("--kag-export must be an absolute directory")
    selected = _native_os.environ.get("TOS_OPS_MECHANICS_EXECUTOR")
    executable = selected if selected is not None else _native_shutil.which("tos-ops-mechanics-plan")
    if executable is None or not _NativePath(executable).is_absolute():
        print("[error] install tos-ops-mechanics-plan or set absolute TOS_OPS_MECHANICS_EXECUTOR", file=_native_sys.stderr)
        return 2
    arguments = [executable, "--repo-root", str(_NativePath(__file__).resolve().parents[1]), '--root-entry-map-build']
    if args.check:
        arguments.append("--check")
    if args.kag_export is not None:
        arguments.extend(["--kag-export", str(args.kag_export)])
    try:
        _native_os.execv(executable, arguments)
    except OSError as error:
        print(f"[error] cannot execute native route operation: {error}", file=_native_sys.stderr)
        return 2
    return 2


if __name__ == "__main__":
    raise SystemExit(native_main())


import argparse

from root_entry_map_common import ROOT_ENTRY_MAP_PATH, build_payload, render_payload


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build Tree-of-Sophia ToS/derived-exports/root_entry_map.min.json.")
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify the generated file matches the canonical rebuild instead of rewriting it.",
    )
    return parser.parse_args()


def legacy_main() -> int:
    args = parse_args()
    payload = build_payload()
    rendered = render_payload(payload)
    ROOT_ENTRY_MAP_PATH.parent.mkdir(parents=True, exist_ok=True)
    if args.check:
        current = ROOT_ENTRY_MAP_PATH.read_text(encoding="utf-8")
        if current != rendered:
            raise SystemExit("ToS/derived-exports/root_entry_map.min.json is out of date")
        print("[ok] verified ToS/derived-exports/root_entry_map.min.json")
        return 0
    ROOT_ENTRY_MAP_PATH.write_text(rendered, encoding="utf-8")
    print("[ok] wrote ToS/derived-exports/root_entry_map.min.json")
    return 0


def main() -> int:
    return native_main()
