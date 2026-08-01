#!/usr/bin/env python3
"""Query the source-witness bibliographic graph with exact source return."""

from __future__ import annotations

import argparse
import json

from source_witness_bibliographic_graph_common import (
    BibliographicGraphBuildError,
    load_verified_projection,
    query_projection,
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--claim-ref", help="match one exact claim ID")
    parser.add_argument("--subject-ref", help="match one exact subject identity ID")
    parser.add_argument("--object-ref", help="match one exact identity-valued object ID")
    parser.add_argument(
        "--normalized-ref",
        help="match one exact normalized Place, Agent, or Organization inside a provision claim",
    )
    parser.add_argument("--predicate", help="match one exact bibliographic predicate")
    parser.add_argument("--review-status", help="match one exact review status")
    parser.add_argument("--visibility", help="match one exact visibility posture")
    parser.add_argument(
        "--limit",
        type=int,
        default=20,
        help="fail rather than truncate when matches exceed this 1..100 bound",
    )
    parser.add_argument(
        "--pretty",
        action="store_true",
        help="indent the deterministic JSON result",
    )
    args = parser.parse_args()

    try:
        projection = load_verified_projection()
        result = query_projection(
            projection,
            claim_ref=args.claim_ref,
            subject_ref=args.subject_ref,
            object_ref=args.object_ref,
            normalized_ref=args.normalized_ref,
            predicate=args.predicate,
            review_status=args.review_status,
            visibility=args.visibility,
            limit=args.limit,
        )
    except BibliographicGraphBuildError as exc:
        parser.exit(2, f"query rejected: {exc}\n")

    print(
        json.dumps(
            result,
            ensure_ascii=False,
            indent=2 if args.pretty else None,
            separators=None if args.pretty else (",", ":"),
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
