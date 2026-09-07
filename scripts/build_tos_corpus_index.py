#!/usr/bin/env python3
"""Build the ToS whole-corpus index derived export."""

from __future__ import annotations

import argparse

from tos_corpus_index_common import REPO_ROOT, TOS_CORPUS_INDEX_PATH, build_payload, render_payload
from source_witness_human_forms import add_assessed_build_arguments, assessed_build_input, write_assessed_candidate


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
    payload = build_payload(assessed_forms=assessed)
    rendered = render_payload(payload)
    if assessed is not None:
        if args.check:
            assessed.verify_current()
            if target.read_text(encoding='utf-8') != rendered:
                raise SystemExit('local assessed corpus differs from current source/journal inputs')
            print('[ok] local assessed corpus matches current source/journal inputs; no publication clearance')
        else:
            write_assessed_candidate(target, rendered, assessed)
            print(f'[ok] wrote local assessed corpus candidate: {target}')
        return 0
    TOS_CORPUS_INDEX_PATH.parent.mkdir(parents=True, exist_ok=True)
    if args.check:
        current = TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8")
        if current != rendered:
            raise SystemExit("ToS/derived-exports/tos_corpus_index.min.json is out of date")
        print("[ok] verified ToS/derived-exports/tos_corpus_index.min.json")
        return 0
    TOS_CORPUS_INDEX_PATH.write_text(rendered, encoding="utf-8")
    print("[ok] wrote ToS/derived-exports/tos_corpus_index.min.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
