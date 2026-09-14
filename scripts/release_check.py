#!/usr/bin/env python3
"""Validate software; full integration audits require explicit selection."""
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


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--phase', choices=('all', 'checks', 'tests'), default='all')
    parser.add_argument('--integration-audit', action='store_true',
                        help='explicitly audit the historical full repository snapshot; requires its data and external owners')
    args = parser.parse_args(argv)
    sequence = 'integration_snapshot_audit' if args.integration_audit else RELEASE_SEQUENCE
    try:
        steps = select_steps(command_sequence(sequence, REPO_ROOT), args.phase)
    except (KeyError, ValueError) as exc:
        print(f'[error] {exc}', flush=True)
        return 2
    for label, command in steps:
        result = run_step(label, command)
        if result:
            return result
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
