#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path

from validation_lanes import command_sequence

REPO_ROOT = Path(__file__).resolve().parents[1]
RELEASE_SEQUENCE = "release_check"
REPOSITORY_TEST_LABEL = "run tests"


def select_steps(
    steps: list[tuple[str, list[str]]],
    phase: str,
) -> list[tuple[str, list[str]]]:
    if phase == "all":
        return steps
    test_positions = [index for index, (label, _) in enumerate(steps) if label == REPOSITORY_TEST_LABEL]
    if test_positions != [len(steps) - 1]:
        raise ValueError(
            f"{RELEASE_SEQUENCE} must contain exactly one final {REPOSITORY_TEST_LABEL!r} step"
        )
    if phase == "checks":
        return steps[:-1]
    if phase == "tests":
        return steps[-1:]
    raise ValueError(f"unknown release phase: {phase}")


def run_step(label: str, command: list[str]) -> int:
    print(f"[run] {label}: {subprocess.list2cmdline(command)}", flush=True)
    completed = subprocess.run(command, cwd=REPO_ROOT, env=os.environ.copy(), check=False)
    if completed.returncode != 0:
        print(f"[error] {label} failed with exit code {completed.returncode}", flush=True)
        return completed.returncode
    print(f"[ok] {label}", flush=True)
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run the manifest-owned Tree of Sophia release sequence.")
    parser.add_argument(
        "--phase",
        choices=("all", "checks", "tests"),
        default="all",
        help="run the full sequence or one CI-safe partition",
    )
    args = parser.parse_args(argv)
    try:
        steps = select_steps(command_sequence(RELEASE_SEQUENCE, REPO_ROOT), args.phase)
    except ValueError as exc:
        print(f"[error] {exc}", flush=True)
        return 2
    for label, command in steps:
        exit_code = run_step(label, command)
        if exit_code != 0:
            return exit_code
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
