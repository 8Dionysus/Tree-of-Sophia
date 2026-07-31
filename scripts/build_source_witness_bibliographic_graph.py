#!/usr/bin/env python3
"""Build the source-witness bibliographic claim graph projection."""

from __future__ import annotations

import argparse

from source_witness_bibliographic_graph_common import (
    GRAPH_PATH,
    build_payload,
    render_payload,
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify generated parity without rewriting the projection",
    )
    args = parser.parse_args()
    rendered = render_payload(build_payload())
    if args.check:
        try:
            current = GRAPH_PATH.read_text(encoding="utf-8")
        except FileNotFoundError:
            raise SystemExit(
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json is missing"
            )
        if current != rendered:
            raise SystemExit(
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json is out of date"
            )
        print("[ok] source-witness bibliographic claim graph matches authored inputs")
        return 0
    GRAPH_PATH.parent.mkdir(parents=True, exist_ok=True)
    GRAPH_PATH.write_text(rendered, encoding="utf-8")
    print("[ok] wrote source-witness bibliographic claim graph")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
