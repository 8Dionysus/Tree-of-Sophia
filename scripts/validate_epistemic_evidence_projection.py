#!/usr/bin/env python3
"""Validate Evidence Lens through installed Rust; Python is an explicit oracle."""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import time

from build_epistemic_evidence_projection import REPO_ROOT, run_native_evidence


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--native', action='store_true', help='installed Rust (the default)')
    mode.add_argument('--legacy-oracle', action='store_true', help='explicit retained Python reference')
    parser.add_argument('--source-root', type=Path, default=REPO_ROOT)
    parser.add_argument('--max-seconds', type=int, default=180)
    args = parser.parse_args(argv)
    deadline = None
    try:
        if args.legacy_oracle:
            import epistemic_evidence_projection_common as oracle
            root = args.source_root.absolute()
            for name, value in list(vars(oracle).items()):
                if isinstance(value, Path) and value.is_absolute() and value.is_relative_to(REPO_ROOT):
                    setattr(oracle, name, root / value.relative_to(REPO_ROOT))
            payload = oracle.build_payload()
            oracle.validate_payload(payload)
            if (not oracle.PROJECTION_PATH.is_file()
                    or oracle.PROJECTION_PATH.read_text(encoding='utf-8') != oracle.render_payload(payload)):
                raise ValueError('ToS/derived-exports/epistemic_evidence_projection.min.json is out of date')
        else:
            deadline = run_native_evidence(args.source_root, 'validate', max_seconds=args.max_seconds)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f'[error] {error}\n')
    print("[ok] validated ToS Evidence Lens projection")
    if deadline is not None and time.monotonic() >= deadline:
        parser.exit(1, '[error] Evidence caller deadline after receipt\n')
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
