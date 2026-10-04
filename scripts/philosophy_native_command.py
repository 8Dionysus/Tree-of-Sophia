"""Maintained native philosophy command adapters; retained commons are oracles."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys


def _executable() -> str:
    selected = os.environ.get("TOS_OPS_MECHANICS_EXECUTOR")
    executable = selected if selected is not None else shutil.which("tos-ops-mechanics-plan")
    if not executable or not Path(executable).is_absolute():
        raise RuntimeError("install tos-ops-mechanics-plan or select its absolute path with TOS_OPS_MECHANICS_EXECUTOR")
    return executable


def command(product: str, source: Path, output: Path, mode: str,
            max_seconds: int = 600, scratch_bytes: int = 256 * 1024 * 1024) -> list[str]:
    return [_executable(), "--philosophy-product", product,
            "--source-root", str(source), "--output-root", str(output),
            "--mode", mode, "--max-seconds", str(max_seconds),
            "--scratch-bytes", str(scratch_bytes)]


def main(product: str, *, validate: bool = False, argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=f"Build or verify the native philosophy {product} product.")
    parser.add_argument("--source-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--max-seconds", type=int, default=600)
    parser.add_argument("--scratch-bytes", type=int, default=256 * 1024 * 1024)
    if not validate:
        parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)
    mode = "validate" if validate else "check" if args.check else "build"
    try:
        selected = command(product, args.source_root, args.output_root or args.source_root,
                           mode, args.max_seconds, args.scratch_bytes)
        os.execv(selected[0], selected)
    except (OSError, RuntimeError) as error:
        print(f"[error] {error}", file=sys.stderr)
        return 1


def build_corpus_products(source: Path) -> None:
    """Derive the three products on the caller's genuine selected private view."""
    subprocess.run(command("corpus", source, source, "build"), check=True)


def prepared_main(*, plant: bool = False, argv: list[str] | None = None) -> int:
    """Forward prepared-dossier compatibility args to the same native owner."""
    args = list(sys.argv[1:] if argv is None else argv)
    explicit_source = any(arg == "--source-root" or arg.startswith("--source-root=") for arg in args)
    source = [] if explicit_source else ["--source-root", str(Path(__file__).resolve().parents[1])]
    if plant and "--plant" not in args:
        args.append("--plant")
    try:
        executable = _executable()
        os.execv(executable, [executable, "--prepared-dossier", *source, *args])
    except (OSError, RuntimeError) as error:
        print(f"[error] {error}", file=sys.stderr)
        return 1
