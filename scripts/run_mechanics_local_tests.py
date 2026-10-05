"""Compatibility entrypoint for the installed native mechanics-local runner.

The native executor owns discovery, ordering and bounded process custody.
Agon, Experience and Questbook assertions execute natively. Growth Cycle
remains a retained Python reference cohort in the whole route.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import sys


REPO_ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-contracts-only", action="store_true",
        help="run only retained native Agon/Experience/Questbook assertions; Growth Cycle remains pending")
    args = parser.parse_args()
    selected = os.environ.get("TOS_OPS_MECHANICS_EXECUTOR")
    executable = selected or shutil.which("tos-ops-mechanics-plan")
    if not executable:
        print(
            "[error] install tos-ops-mechanics-plan or set TOS_OPS_MECHANICS_EXECUTOR",
            file=sys.stderr,
        )
        return 1
    try:
        # Replace this compatibility process: cancellation goes directly to
        # the native supervisor, with no second process owner or fallback.
        argv = [
            executable, "--execute", "--repo-root", str(REPO_ROOT),
            "--python", sys.executable,
            "--command-timeout-ms", "300000",
            "--lane-timeout-ms", "3600000",
            "--cleanup-grace-ms", "1000",
            "--max-output-bytes", "16777216",
        ]
        if args.native_contracts_only:
            argv.append("--native-contracts-only")
        os.execv(executable, argv)
    except OSError as error:
        print(f"[error] cannot execute native mechanics-local runner: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
