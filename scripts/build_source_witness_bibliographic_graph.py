#!/usr/bin/env python3
"""Build the source-witness bibliographic claim graph projection."""

from __future__ import annotations

import argparse

from source_witness_bibliographic_graph_common import (
    GRAPH_PATH,
    REPO_ROOT,
    build_payload,
    render_payload,
)
from source_witness_human_forms import add_assessed_build_arguments, assessed_build_input, write_assessed_candidate
from partitioned_projection_common import build_storage, write_partitioned_payload, check_partitioned_payload


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify generated parity without rewriting the projection",
    )
    add_assessed_build_arguments(parser)
    args = parser.parse_args()
    try:
        assessed, target = assessed_build_input(args, REPO_ROOT, GRAPH_PATH)
    except ValueError as exc:
        parser.error(str(exc))
    if assessed is not None:
        rendered = render_payload(build_payload(assessed_forms=assessed))
        if args.check:
            assessed.verify_current()
            if target.read_text(encoding='utf-8') != rendered:
                raise SystemExit('local assessed graph differs from current source/journal inputs')
            print('[ok] local assessed graph matches current source/journal inputs; no publication clearance')
        else:
            write_assessed_candidate(target, rendered, assessed)
            print(f'[ok] wrote local assessed graph candidate: {target}')
        return 0
    with build_storage() as storage:
        payload = build_payload(storage=storage)
        if args.check:
            check_partitioned_payload(GRAPH_PATH, payload)
            print("[ok] partitioned bibliographic graph and closure match authored inputs")
        else:
            write_partitioned_payload(GRAPH_PATH, payload, prune=True)
            print("[ok] wrote partitioned source-witness bibliographic claim graph")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
