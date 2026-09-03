#!/usr/bin/env python3
"""Build or check the reviewed open-work material-discovery queue."""

from __future__ import annotations

import argparse
import sys

from open_work_candidate_queue_common import (
    REPO_ROOT,
    QueueBuildError,
    build_payload,
    check_output,
    render_payload,
    write_output,
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="check generated parity without writing")
    args = parser.parse_args()

    try:
        rendered = render_payload(build_payload(REPO_ROOT))
    except QueueBuildError as exc:
        print(f"Open-work candidate queue build failed: {exc}", file=sys.stderr)
        return 1

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
