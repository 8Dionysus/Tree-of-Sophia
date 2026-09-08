#!/usr/bin/env python3
from __future__ import annotations

import argparse
from fnmatch import fnmatchcase
import os
import subprocess
from pathlib import Path, PurePosixPath
from typing import Sequence

from validation_lanes import command_sequence

REPO_ROOT = Path(__file__).resolve().parents[1]
RELEASE_SEQUENCE = "release_check"
REPOSITORY_TEST_LABEL = "run tests"


ACCESS_TESTS = (
    "access/tests/test_access_contract.py",
    "access/tests/test_cloudflare_tunnel_deploy.py",
    "access/tests/test_http_security.py",
)
NAMING_TESTS = ("tests/test_validate_active_naming.py",)
GRAPH_TESTS = ("tests/test_source_witness_bibliographic_graph.py",)
CORPUS_TESTS = ("tests/test_tos_corpus_index.py",)
VALIDATION_LANES_TESTS = ("tests/test_validation_lanes.py",)
FEEDBACK_PYTEST_AUTOLOAD_ENV = "PYTEST_DISABLE_PLUGIN_AUTOLOAD"

# This small table contains only implementation routes with reviewed direct
# consumers. A path outside it intentionally returns None so the caller uses
# the complete manifest-owned release sequence. In particular, ToS data,
# schemas, and generated outputs are not inferred from their existence here.
FEEDBACK_ROUTES = (
    (
        (
            "access/src/tos_access/**",
            "access/contracts/**",
            "access/profiles/**",
            "access/packaging/**",
        ),
        ACCESS_TESTS,
    ),
    (("access/deploy/cloudflare-tunnel/**",), ("access/tests/test_cloudflare_tunnel_deploy.py",)),
    (("scripts/validate_active_naming.py",), NAMING_TESTS),
    (
        (
            "scripts/source_witness_bibliographic_graph_common.py",
            "scripts/build_source_witness_bibliographic_graph.py",
            "scripts/query_source_witness_bibliographic_graph.py",
            "scripts/validate_source_witness_bibliographic_graph.py",
        ),
        GRAPH_TESTS,
    ),
    (
        (
            "scripts/tos_corpus_index_common.py",
            "scripts/build_tos_corpus_index.py",
            "scripts/validate_tos_corpus_index.py",
        ),
        CORPUS_TESTS,
    ),
    (("scripts/release_check.py",), VALIDATION_LANES_TESTS),
)

# Test files are opt-in, not selected by a filename prefix. The access contract
# test exports helpers used by the other access tests, so its edits exercise
# the complete access group. Other explicit test paths are deliberately left
# to the full release oracle until their imports are reviewed.
DIRECT_TEST_ROUTES = (
    ("access/tests/test_access_contract.py", ACCESS_TESTS),
)


def _normalize_changed_paths(changed_paths: Sequence[str]) -> tuple[str, ...]:
    normalized: list[str] = []
    for raw in changed_paths:
        if not raw or "\\" in raw or "\x00" in raw:
            raise ValueError(f"changed path must be repository-relative: {raw!r}")
        path = PurePosixPath(raw)
        if path.is_absolute() or ".." in path.parts:
            raise ValueError(f"changed path must be repository-relative: {raw!r}")
        normalized_path = path.as_posix()
        if normalized_path in {"", "."}:
            raise ValueError(f"changed path must name a repository path: {raw!r}")
        normalized.append(normalized_path)
    if not normalized:
        raise ValueError("--feedback requires at least one --changed-path")
    return tuple(dict.fromkeys(normalized))


def _existing_tests(
    test_paths: Sequence[str],
    repo_root: Path,
) -> tuple[str, ...] | None:
    tests = tuple(sorted(set(test_paths)))
    if not tests or any(not (repo_root / path).is_file() for path in tests):
        return None
    return tests


def _tests_for_path(path: str, repo_root: Path) -> tuple[str, ...] | None:
    for test_path, test_paths in DIRECT_TEST_ROUTES:
        if path == test_path:
            return _existing_tests(test_paths, repo_root)
    matched = False
    selected: set[str] = set()
    for patterns, test_paths in FEEDBACK_ROUTES:
        if any(fnmatchcase(path, pattern) for pattern in patterns):
            matched = True
            tests = _existing_tests(test_paths, repo_root)
            if tests is None:
                return None
            selected.update(tests)
    return tuple(sorted(selected)) if matched and selected else None


def feedback_test_paths(
    changed_paths: Sequence[str],
    repo_root: Path = REPO_ROOT,
) -> tuple[str, ...] | None:
    """Return a union of existing focused tests, or ``None`` for full fallback.

    This is intentionally a local edit-feedback selector. It makes no claim
    about release completeness; an unsupported, shared, or topology-drifted
    path returns ``None`` so the caller runs the full manifest-owned release
    sequence. Malformed paths are rejected by ``_normalize_changed_paths``.
    """

    selected: set[str] = set()
    for path in _normalize_changed_paths(changed_paths):
        tests = _tests_for_path(path, repo_root)
        if tests is None:
            return None
        selected.update(tests)
    ordered = tuple(sorted(selected))
    if not ordered or any(not (repo_root / path).is_file() for path in ordered):
        return None
    return ordered


def feedback_pytest_command(
    steps: Sequence[tuple[str, list[str]]],
    test_paths: Sequence[str],
) -> list[str]:
    """Reuse the release manifest's pytest flags with focused path operands."""

    test_steps = [command for label, command in steps if label == REPOSITORY_TEST_LABEL]
    if len(test_steps) != 1:
        raise ValueError("release sequence must contain exactly one repository test step")
    command = list(test_steps[0])
    if not command or command[-1] != "tests" or "pytest" not in command:
        raise ValueError("repository test step must end in the manifest-owned tests target")
    return [*command[:-1], *test_paths]


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


def feedback_test_env() -> dict[str, str]:
    """Return the caller environment with focused pytest isolation enabled."""

    environment = os.environ.copy()
    environment.setdefault(FEEDBACK_PYTEST_AUTOLOAD_ENV, "1")
    return environment


def run_step(
    label: str,
    command: list[str],
    *,
    env: dict[str, str] | None = None,
) -> int:
    print(f"[run] {label}: {subprocess.list2cmdline(command)}", flush=True)
    completed = subprocess.run(
        command,
        cwd=REPO_ROOT,
        env=os.environ.copy() if env is None else env,
        check=False,
    )
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
    parser.add_argument(
        "--feedback",
        action="store_true",
        help="run local owner-focused tests for explicit changed paths",
    )
    parser.add_argument(
        "--changed-path",
        action="append",
        default=[],
        metavar="PATH",
        help="repository-relative changed path; repeat for a multi-file edit",
    )
    args = parser.parse_args(argv)
    if args.feedback and args.phase != "all":
        parser.error("--feedback cannot be combined with --phase")
    if args.changed_path and not args.feedback:
        parser.error("--changed-path requires --feedback")
    if args.feedback and not args.changed_path:
        parser.error("--feedback requires at least one --changed-path")
    try:
        steps = select_steps(command_sequence(RELEASE_SEQUENCE, REPO_ROOT), args.phase)
    except ValueError as exc:
        print(f"[error] {exc}", flush=True)
        return 2
    if args.feedback:
        try:
            changed_paths = _normalize_changed_paths(args.changed_path)
            test_paths = feedback_test_paths(changed_paths, REPO_ROOT)
        except ValueError as exc:
            print(f"[error] {exc}", flush=True)
            return 2
        print(f"[feedback] changed paths: {', '.join(changed_paths)}", flush=True)
        if test_paths is None:
            print(
                "[feedback] unsupported/shared path or route drift; running full release oracle",
                flush=True,
            )
            selected_steps = steps
            feedback_env = None
        else:
            print(
                f"[feedback] selected {len(test_paths)} existing test targets: "
                f"{', '.join(test_paths)}",
                flush=True,
            )
            selected_steps = [("affected tests", feedback_pytest_command(steps, test_paths))]
            feedback_env = feedback_test_env()
        for label, command in selected_steps:
            if feedback_env is None:
                exit_code = run_step(label, command)
            else:
                exit_code = run_step(label, command, env=feedback_env)
            if exit_code != 0:
                return exit_code
        return 0
    for label, command in steps:
        exit_code = run_step(label, command)
        if exit_code != 0:
            return exit_code
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
