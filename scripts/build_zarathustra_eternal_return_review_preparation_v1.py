#!/usr/bin/env python3
"""Native-only Eternal review producer and retained-product consumer entry."""
from __future__ import annotations
import os
from pathlib import Path
import shutil
import sys
REPO = Path(__file__).resolve().parents[1]
COMMAND = "zarathustra-eternal-return-review-preparation-v1"

def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    # The dedicated native research parser accepts separate option/value tokens.
    args = [token for arg in args for token in (
        ["--source-root", arg.split("=", 1)[1]]
        if arg.startswith("--source-root=") else [arg]
    )]
    native = os.environ.get("TOS_NATIVE_PREPARED_CONSUMER_BIN") or shutil.which("tos")
    if not native or not Path(native).is_absolute():
        print("error: select installed tos through TOS_NATIVE_PREPARED_CONSUMER_BIN or PATH", file=sys.stderr)
        return 1
    command = [native, COMMAND]
    if args not in (["--help"], ["-h"]) and not any(
        arg == "--source-root" or arg.startswith("--source-root=") for arg in args
    ):
        command += ["--source-root", str(REPO)]
    command += args
    try:
        os.execv(native, command)
    except OSError as exc:
        print(f"error: cannot execute native tos: {exc}", file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
