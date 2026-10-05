#!/usr/bin/env python3
"""Validate ToS decision records without rewriting generated indexes."""

from __future__ import annotations

# Maintained command dispatch. Imported helpers below remain reference APIs;
# native execution never falls back to the Python validator or generator.
def native_main(argv: list[str] | None = None) -> int:
    import argparse as _argparse
    import os as _os
    from pathlib import Path as _Path
    import shutil as _shutil
    import sys as _sys
    parser = _argparse.ArgumentParser(description=__doc__)

    args = parser.parse_args(argv)
    selected = _os.environ.get("TOS_OPS_MECHANICS_EXECUTOR")
    executable = selected if selected is not None else _shutil.which("tos-ops-mechanics-plan")
    if executable is None or not _Path(executable).is_absolute():
        print("[error] install tos-ops-mechanics-plan or set absolute TOS_OPS_MECHANICS_EXECUTOR", file=_sys.stderr)
        return 2
    command = [executable, "--repo-root", str(_Path(__file__).resolve().parents[1]), "--decision-records-validate"]
    try:
        _os.execv(executable, command)
    except OSError as error:
        print(f"[error] cannot execute native decision operation: {error}", file=_sys.stderr)
        return 2
    return 2


if __name__ == "__main__":
    raise SystemExit(native_main())


from pathlib import Path

from generate_decision_indexes import REPO_ROOT, collect_decision_records, validate_decision_lane_surfaces, validate_index_contract


def validate_decision_records(repo_root: Path = REPO_ROOT) -> list[tuple[str, str]]:
    records, issues = collect_decision_records(repo_root)
    issues.extend(validate_index_contract(repo_root))
    issues.extend(validate_decision_lane_surfaces(repo_root))
    if not records:
        issues.append(("docs/decisions", "no decision records available for validation"))
    return issues


def main() -> int:
    issues = validate_decision_records(REPO_ROOT)
    if issues:
        print("Decision record validation failed.")
        for location, message in issues:
            print(f"- {location}: {message}")
        return 1

    print("[ok] decision records validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
