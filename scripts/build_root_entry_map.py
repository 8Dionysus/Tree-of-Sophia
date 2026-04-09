#!/usr/bin/env python3
"""Build the ToS root entry capsule."""

from __future__ import annotations

import argparse

from root_entry_map_common import ROOT_ENTRY_MAP_PATH, build_payload, render_payload


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build Tree-of-Sophia generated/root_entry_map.min.json.")
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify the generated file matches the canonical rebuild instead of rewriting it.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    payload = build_payload()
    rendered = render_payload(payload)
    ROOT_ENTRY_MAP_PATH.parent.mkdir(parents=True, exist_ok=True)
    if args.check:
        current = ROOT_ENTRY_MAP_PATH.read_text(encoding="utf-8")
        if current != rendered:
            raise SystemExit("generated/root_entry_map.min.json is out of date")
        print("[ok] verified generated/root_entry_map.min.json")
        return 0
    ROOT_ENTRY_MAP_PATH.write_text(rendered, encoding="utf-8")
    print("[ok] wrote generated/root_entry_map.min.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
