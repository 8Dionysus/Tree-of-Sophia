#!/usr/bin/env python3
"""Delegate the Tree of Sophia local stats port to the aoa-stats owner."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys


REPO_ROOT = Path(__file__).resolve().parents[1]
PORT_PATH = REPO_ROOT / "stats" / "port.manifest.json"


def candidate_roots() -> tuple[Path, ...]:
    explicit = os.environ.get("AOA_STATS_ROOT")
    if explicit is not None:
        return (Path(explicit).expanduser(),)
    return (REPO_ROOT / ".deps" / "aoa-stats", REPO_ROOT.parent / "aoa-stats")


def main() -> int:
    candidates = candidate_roots()
    for root in candidates:
        validator = root / "scripts" / "validate_stats_protocol.py"
        if validator.is_file():
            return subprocess.run(
                (sys.executable, str(validator), "--port", str(PORT_PATH)),
                cwd=REPO_ROOT,
                check=False,
            ).returncode

    checked = ", ".join(str(root) for root in candidates)
    print(f"[error] aoa-stats validator is unavailable; checked: {checked}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
