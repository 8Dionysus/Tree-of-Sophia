#!/usr/bin/env python3
"""Build the ToS whole-corpus index derived export."""

from __future__ import annotations

import argparse

from tos_corpus_index_common import REPO_ROOT, TOS_CORPUS_INDEX_PATH, build_payload, render_payload
from source_witness_human_forms import add_assessed_build_arguments, assessed_build_input, write_assessed_candidate
from partitioned_projection_common import build_storage, write_partitioned_payload, check_partitioned_payload


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build ToS/derived-exports/tos_corpus_index.min.json.")
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify the generated file matches the canonical rebuild instead of rewriting it.",
    )
    add_assessed_build_arguments(parser)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        assessed, target = assessed_build_input(args, REPO_ROOT, TOS_CORPUS_INDEX_PATH)
    except ValueError as exc:
        raise SystemExit(str(exc)) from exc
    if assessed is not None:
        rendered = render_payload(build_payload(assessed_forms=assessed))
        if args.check:
            assessed.verify_current()
            if target.read_text(encoding='utf-8') != rendered:
                raise SystemExit('local assessed corpus differs from current source/journal inputs')
            print('[ok] local assessed corpus matches current source/journal inputs; no publication clearance')
        else:
            write_assessed_candidate(target, rendered, assessed)
            print(f'[ok] wrote local assessed corpus candidate: {target}')
        return 0
    with build_storage() as storage:
        payload = build_payload(storage=storage)
        if args.check:
            check_partitioned_payload(TOS_CORPUS_INDEX_PATH, payload)
            print("[ok] verified partitioned ToS corpus index and its complete closure")
        else:
            write_partitioned_payload(TOS_CORPUS_INDEX_PATH, payload, prune=True)
            print("[ok] wrote partitioned ToS corpus index")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
