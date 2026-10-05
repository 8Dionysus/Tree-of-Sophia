#!/usr/bin/env python3
"""Build the private source-returnable material library for the local constructor.

This is presentation data, not a new corpus contract or semantic projection.
The native producer retains all 210 entries of the existing eternal-return
evidence dossier, including ambiguous, excluded, and one-sided entries. Exact
witness text is read only from anchored local inputs; private text is never
embedded in this tracked launcher.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


DEFAULT_SOURCE = Path("/srv/AbyssOS/Tree-of-Sophia")
DEFAULT_DEMO = Path("/srv/abyss-machine/storage/artifacts/tos-eternal-return-demo-20260910/demo-data.json")
DEFAULT_OUTPUT = Path("/srv/abyss-machine/storage/artifacts/tos-tree-constructor-20260910/library.json")


def _builder_repo() -> Path:
    return Path(__file__).resolve().parents[3]


def _native_binary() -> str:
    configured = os.environ.get("TOS_CONSTRUCTOR_LIBRARY_BIN")
    if configured:
        return configured
    found = shutil.which("tos-constructor-library")
    if found:
        return found
    raise FileNotFoundError(
        "tos-constructor-library is unavailable; set TOS_CONSTRUCTOR_LIBRARY_BIN "
        "or install it in the software prefix bin directory on PATH"
    )


def _command(binary: str, repo: Path, demo: Path, output: Path, check: bool) -> list[str]:
    command = [
        binary,
        "--source-repo",
        str(repo),
        "--demo-packet",
        str(demo),
        "--output",
        str(output),
        "--builder-repo",
        str(_builder_repo()),
    ]
    if check:
        command.append("--check")
    return command


def build(repo: Path, demo_path: Path) -> tuple[dict, dict]:
    """Compatibility adapter that returns native data and its native report."""
    with tempfile.TemporaryDirectory(prefix="tos-constructor-library-") as temporary:
        output = Path(temporary) / "library.json"
        command = _command(_native_binary(), Path(repo), Path(demo_path), output, False)
        completed = subprocess.run(command, text=True, stdout=subprocess.PIPE, check=False)
        if completed.returncode:
            raise subprocess.CalledProcessError(completed.returncode, command)
        data = json.loads(output.read_text(encoding="utf-8"))
        summary = json.loads(completed.stdout)
        report = {key: value for key, value in summary.items() if key not in {"status", "output", "bytes", "sha256"}}
        return data, report


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-repo", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--demo-packet", type=Path, default=DEFAULT_DEMO)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true", help="Verify inputs and exact existing output bytes without writing")
    args = parser.parse_args()
    command = _command(_native_binary(), args.source_repo, args.demo_packet, args.output, args.check)
    os.execvp(command[0], command)


if __name__ == "__main__":
    main()
