#!/usr/bin/env python3
"""Build the ToS philosophy graph view catalog export."""

from __future__ import annotations

import argparse

from philosophy_graph_views_common import GRAPH_VIEW_CATALOG_PATH, build_payload, render_payload


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build ToS/derived-exports/philosophy_graph_views.min.json.")
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
    GRAPH_VIEW_CATALOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    if args.check:
        current = GRAPH_VIEW_CATALOG_PATH.read_text(encoding="utf-8")
        if current != rendered:
            raise SystemExit("ToS/derived-exports/philosophy_graph_views.min.json is out of date")
        print("[ok] verified ToS/derived-exports/philosophy_graph_views.min.json")
        return 0
    GRAPH_VIEW_CATALOG_PATH.write_text(rendered, encoding="utf-8")
    print("[ok] wrote ToS/derived-exports/philosophy_graph_views.min.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
