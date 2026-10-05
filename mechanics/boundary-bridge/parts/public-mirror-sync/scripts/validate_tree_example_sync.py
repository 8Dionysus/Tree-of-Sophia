#!/usr/bin/env python3
from __future__ import annotations

# Executable compatibility only: installed native code owns production behavior.
# Importable functions below remain explicit comparison APIs for retained tests.
if __name__ == "__main__":
    import argparse as _argparse
    import os as _os
    from pathlib import Path as _Path
    import shutil as _shutil
    import sys as _sys
    _parser = _argparse.ArgumentParser()
    _args = _parser.parse_args()
    _selected = _os.environ.get("TOS_OPS_MECHANICS_EXECUTOR")
    _executable = _selected if _selected is not None else _shutil.which("tos-ops-mechanics-plan")
    if not _executable:
        print("[error] install tos-ops-mechanics-plan or set TOS_OPS_MECHANICS_EXECUTOR", file=_sys.stderr)
        raise SystemExit(1)
    _argv = [_executable, "--repo-root", str(_Path(__file__).resolve().parents[5]), "--public-mirror-validate"]
    try:
        _os.execv(_executable, _argv)
    except OSError as _error:
        print(f"[error] cannot execute native public-mirror-validate: {_error}", file=_sys.stderr)
        raise SystemExit(1)
    raise SystemExit(1)


import sys
from pathlib import Path

from tree_example_sync import REPO_ROOT, build_expected_example_payloads, encode_json

Issue = tuple[str, str]


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    for example_path, payload in build_expected_example_payloads(root):
        rel = example_path.relative_to(root).as_posix()
        try:
            actual_text = example_path.read_text(encoding="utf-8")
        except FileNotFoundError:
            issues.append((rel, "missing compatibility mirror"))
            continue

        expected_text = encode_json(payload)
        if actual_text != expected_text:
            issues.append(
                (
                    rel,
                    "out of sync with canonical tree; run python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/sync_tree_examples.py",
                )
            )

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Tree/example sync check failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1

    print("[ok] validated ToS/canon/example compatibility mirrors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
