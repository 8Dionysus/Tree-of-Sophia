#!/usr/bin/env python3
"""Validate the standalone software release."""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess

from validation_lanes import command_sequence

REPO_ROOT = Path(__file__).resolve().parents[1]
RELEASE_SEQUENCE = 'release_check'
REPOSITORY_TEST_LABEL = 'run tests'


def select_steps(steps: list[tuple[str, list[str]]], phase: str) -> list[tuple[str, list[str]]]:
    if phase == 'all':
        return steps
    positions = [i for i, (label, _) in enumerate(steps) if label == REPOSITORY_TEST_LABEL]
    if positions != [len(steps) - 1]:
        raise ValueError('selected sequence must contain exactly one final run tests step')
    if phase == 'checks':
        return steps[:-1]
    if phase == 'tests':
        return steps[-1:]
    raise ValueError(f'unknown phase: {phase}')


def run_step(label: str, command: list[str]) -> int:
    print(f'[run] {label}: {subprocess.list2cmdline(command)}', flush=True)
    env = os.environ.copy()
    env.setdefault('PYTEST_DISABLE_PLUGIN_AUTOLOAD', '1')
    completed = subprocess.run(command, cwd=REPO_ROOT, env=env, check=False)
    if completed.returncode:
        print(f'[error] {label} failed with exit code {completed.returncode}', flush=True)
    return completed.returncode


def _arguments(argv: list[str] | None = None, *, native: bool = False) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--phase', choices=('all', 'checks', 'tests'), default='all')
    if native:
        for option in ('command-timeout-ms', 'lane-timeout-ms',
                       'cleanup-grace-ms', 'max-output-bytes'):
            parser.add_argument('--' + option, type=int, default=None)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _arguments(argv)
    try:
        steps = select_steps(command_sequence(RELEASE_SEQUENCE, REPO_ROOT), args.phase)
    except (KeyError, ValueError) as exc:
        print(f'[error] {exc}', flush=True)
        return 2
    for label, command in steps:
        result = run_step(label, command)
        if result:
            return result
    return 0


def native_main(argv: list[str] | None = None) -> int:
    """Installed CLI route; phase APIs remain available to Python callers."""
    import shutil
    import sys

    args = _arguments(argv, native=True)
    selected = os.environ.get('TOS_RELEASE_CHECK_EXECUTOR')
    executable = selected or shutil.which('tos-release-check')
    if not executable:
        print('[error] install tos-release-check or set TOS_RELEASE_CHECK_EXECUTOR', file=sys.stderr)
        return 1
    arguments = ['--phase', args.phase]
    # Forward explicit owner limits; native validation remains authoritative.
    # Omitted values retain the native defaults rather than duplicating them.
    for option in ('command-timeout-ms', 'lane-timeout-ms',
                   'cleanup-grace-ms', 'max-output-bytes'):
        value = getattr(args, option.replace('-', '_'))
        if value is not None:
            arguments.extend(['--' + option, str(value)])
    try:
        os.execv(executable, [executable, '--repo-root', str(REPO_ROOT),
                              '--python', sys.executable, *arguments])
    except OSError as error:
        print(f'[error] cannot execute native release check: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(native_main())
