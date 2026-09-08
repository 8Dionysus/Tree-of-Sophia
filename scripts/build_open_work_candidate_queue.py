#!/usr/bin/env python3
"""Build or check the reviewed open-work material-discovery queue."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from open_work_candidate_queue_common import (
    REPO_ROOT,
    QueueBuildError,
    build_payload,
    build_readiness_payload,
    check_output,
    render_payload,
    write_output,
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="check generated parity without writing")
    parser.add_argument("--selection-mode", choices=("chronological", "readiness"), default="chronological")
    parser.add_argument("--readiness-plan", type=Path, help="repository-relative owner readiness plan")
    parser.add_argument("--dry-run", action="store_true", help="print the exact plan without writing")
    args = parser.parse_args()
    if args.readiness_plan and args.selection_mode != "readiness":
        parser.error("--readiness-plan requires --selection-mode readiness")
    if args.selection_mode == "readiness" and not (args.dry_run or args.check):
        parser.error("readiness preparation requires --dry-run or --check; historical queue is unchanged")

    try:
        payload = (
            build_readiness_payload(REPO_ROOT, readiness_plan=args.readiness_plan)
            if args.selection_mode == "readiness" else build_payload(REPO_ROOT)
        )
        rendered = render_payload(payload)
    except QueueBuildError as exc:
        print(f"Open-work candidate queue build failed: {exc}", file=sys.stderr)
        return 1

    if args.dry_run:
        print(rendered, end="")
        return 0
    if args.check and args.selection_mode == "readiness":
        print("[ok] readiness plan references, digests and selection; no acquisition or semantic acceptance")
        return 0
    if args.check:
        issues = check_output(REPO_ROOT, rendered)
        if issues:
            print("Open-work candidate queue parity failed.", file=sys.stderr)
            for issue in issues:
                print(f"- {issue}", file=sys.stderr)
            return 1
        print("[ok] reviewed open-work candidate queue matches authored candidates and receipts")
        return 0

    write_output(REPO_ROOT, rendered)
    print("[ok] generated reviewed open-work candidate queue")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
