#!/usr/bin/env python3
"""Maintained Evidence Lens entry: installed Rust by default, explicit Python oracle.

TOS_NATIVE_PREPARED_CONSUMER_BIN (or PATH's tos) selects software, never data.
Temporary SQLite lives under the caller's TMPDIR; its capacity is the caller's
responsibility. Native build produces a fresh sibling before atomic replacement.
Imported common-module APIs remain the independent Python reference oracle.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile
import time
import uuid

REPO_ROOT = Path(__file__).resolve().parents[1]
PROJECTION_REF = 'ToS/derived-exports/epistemic_evidence_projection.min.json'


def _stat_stamp(value):
    if not stat.S_ISREG(value.st_mode):
        raise ValueError('Evidence companion must be a regular file, not a link')
    return (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns,
            value.st_ctime_ns, value.st_mode)


def _stamp(path: Path):
    try:
        return _stat_stamp(path.lstat())
    except FileNotFoundError:
        return None


def run_native_evidence(source_root: Path, operation: str, *, replace: bool = False,
                        max_seconds: int = 180) -> float:
    """Invoke only installed software over an explicitly selected owned source view.

    The corpus worker uses fresh-output mode; the maintained builder replaces
    its generated companion only after success and unchanged prior output.
    No native failure selects the Python oracle.
    """
    if operation not in ('build', 'check', 'validate') or not 1 <= max_seconds <= 600:
        raise ValueError('invalid Evidence operation or deadline (1..600 seconds)')
    selected = os.environ.get('TOS_NATIVE_PREPARED_CONSUMER_BIN') or shutil.which('tos')
    if not selected or not Path(selected).is_absolute():
        raise ValueError('select installed tos through TOS_NATIVE_PREPARED_CONSUMER_BIN or PATH')
    root = Path(source_root).absolute()
    target = root / PROJECTION_REF
    before = _stamp(target) if operation == 'build' else None
    if before is not None and not replace:
        raise ValueError('Evidence producer output overlaps an existing input')
    candidate = target.parent / ('.epistemic-evidence-' + uuid.uuid4().hex + '.json')
    deadline = time.monotonic() + max_seconds
    def remaining():
        seconds = deadline - time.monotonic()
        if seconds <= 0:
            raise TimeoutError('Evidence caller deadline expired')
        return seconds
    try:
        with tempfile.TemporaryDirectory(prefix='tos-evidence-') as raw:
            command = [selected, 'evidence-projection', operation, '--source-root', str(root),
                       '--staging', str(Path(raw) / 'selected.sqlite'),
                       '--max-seconds', str(max_seconds)]
            if operation == 'build':
                target.parent.mkdir(parents=True, exist_ok=True)
                command += ['--output', str(candidate)]
            # Native's fixed receipt is suppressed so worker stdout stays one
            # JSON result. Errors retain their existing stderr/exit behavior.
            subprocess.run(command, check=True, timeout=remaining(), stdout=subprocess.DEVNULL)
            remaining()
            if operation == 'build':
                produced = _stamp(candidate)
                if produced is None or produced[2] > 1_048_576:
                    raise ValueError('Evidence fresh output missing or over budget')
                file = os.open(candidate, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
                try:
                    if _stat_stamp(os.fstat(file)) != produced:
                        raise ValueError('Evidence fresh output changed before sync')
                    if before is not None:
                        os.fchmod(file, stat.S_IMODE(before[5]))
                    remaining()
                    os.fsync(file)
                    synced = _stat_stamp(os.fstat(file))
                    if _stamp(candidate) != synced or _stamp(target) != before:
                        raise ValueError('Evidence output changed before replacement')
                    remaining()
                    os.replace(candidate, target)
                finally:
                    os.close(file)
                directory = os.open(target.parent, os.O_RDONLY | os.O_DIRECTORY)
                try:
                    os.fsync(directory)
                finally:
                    os.close(directory)
                remaining()
        remaining()
    finally:
        candidate.unlink(missing_ok=True)
    remaining()
    return deadline


def _legacy(root: Path, check: bool) -> None:
    import epistemic_evidence_projection_common as oracle
    # This explicit oracle runs in a single-use command process, as before.
    for name, value in list(vars(oracle).items()):
        if isinstance(value, Path) and value.is_absolute() and value.is_relative_to(REPO_ROOT):
            setattr(oracle, name, root / value.relative_to(REPO_ROOT))
    rendered = oracle.render_payload(oracle.build_payload())
    if check:
        if not oracle.PROJECTION_PATH.is_file() or oracle.PROJECTION_PATH.read_text(encoding='utf-8') != rendered:
            raise ValueError(PROJECTION_REF + ' is out of date')
    else:
        oracle.PROJECTION_PATH.write_text(rendered, encoding='utf-8')


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--native', action='store_true', help='installed Rust (the default)')
    mode.add_argument('--legacy-oracle', action='store_true', help='explicit retained Python reference')
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--source-root', type=Path, default=REPO_ROOT)
    parser.add_argument('--max-seconds', type=int, default=180)
    args = parser.parse_args(argv)
    deadline = None
    try:
        if args.legacy_oracle:
            _legacy(args.source_root.absolute(), args.check)
        else:
            deadline = run_native_evidence(args.source_root, 'check' if args.check else 'build',
                                           replace=True, max_seconds=args.max_seconds)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f'[error] {error}\n')
    print('[ok] ' + ('verified' if args.check else 'wrote') + ' ToS Evidence Lens projection')
    if deadline is not None and time.monotonic() >= deadline:
        parser.exit(1, '[error] Evidence caller deadline after receipt\n')
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
