#!/usr/bin/env python3
"""Delegate the Tree of Sophia local stats port to the aoa-stats owner."""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys


def _absolute_path(raw: str) -> Path:
    return Path(raw).expanduser().absolute()


def _require_directory(path: Path, *, label: str) -> str | None:
    if path.is_symlink() or path.absolute() != path.resolve():
        return f"{label} must not be a symlink: {path}"
    if not path.exists():
        return f"{label} does not exist: {path}"
    if not path.is_dir():
        return f"{label} must be a directory: {path}"
    return None


def _require_file(path: Path, *, label: str) -> str | None:
    if path.is_symlink() or path.absolute() != path.resolve():
        return f"{label} must not be a symlink: {path}"
    if not path.exists():
        return f"{label} does not exist: {path}"
    if not path.is_file():
        return f"{label} must be a regular file: {path}"
    return None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stats-root", required=True, help="exact aoa-stats owner root")
    parser.add_argument("--port", required=True, help="local stats port manifest")
    args = parser.parse_args(argv)

    stats_root = _absolute_path(args.stats_root)
    port = _absolute_path(args.port)
    if error := _require_directory(stats_root, label="stats root"):
        print(f"[error] {error}", file=sys.stderr)
        return 1
    if error := _require_file(port, label="port"):
        print(f"[error] {error}", file=sys.stderr)
        return 1

    validator = stats_root / "scripts" / "validate_stats_protocol.py"
    if error := _require_file(validator, label="stats validator"):
        print(f"[error] {error}", file=sys.stderr)
        return 1

    try:
        completed = subprocess.run(
            (sys.executable, str(validator), "--port", str(port)),
            cwd=port.parent.parent,
            check=False,
        )
    except OSError as exc:
        print(f"[error] could not execute stats validator: {exc}", file=sys.stderr)
        return 1
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
