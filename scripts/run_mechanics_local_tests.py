from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from typing import Iterable


REPO_ROOT = Path(__file__).resolve().parents[1]
MECHANIC_LEVEL_TEST_GLOB = "mechanics/*/tests/test*.py"
PART_LOCAL_TEST_GLOB = "mechanics/*/parts/*/tests/test*.py"
MECHANIC_LEVEL_SCRIPT_GLOB = "mechanics/*/scripts/*.py"
PART_LOCAL_SCRIPT_GLOB = "mechanics/*/parts/*/scripts/*.py"


def discovered_mechanics_local_test_files(repo_root: Path = REPO_ROOT) -> tuple[Path, ...]:
    return tuple(
        sorted(
            (
                *repo_root.glob(MECHANIC_LEVEL_TEST_GLOB),
                *repo_root.glob(PART_LOCAL_TEST_GLOB),
            )
        )
    )


def local_home_for_test(test_path: Path) -> Path:
    parts = test_path.parts
    test_index = parts.index("tests")
    return Path(*parts[:test_index])


def discovered_test_homes(repo_root: Path = REPO_ROOT) -> tuple[Path, ...]:
    homes = {
        local_home_for_test(path.relative_to(repo_root))
        for path in discovered_mechanics_local_test_files(repo_root)
    }
    return tuple(sorted(homes))


def local_home_for_script(script_path: Path) -> Path:
    parts = script_path.parts
    scripts_index = parts.index("scripts")
    return Path(*parts[:scripts_index])


def discovered_script_homes(repo_root: Path = REPO_ROOT) -> tuple[Path, ...]:
    homes = {
        local_home_for_script(path.relative_to(repo_root))
        for path in (
            *repo_root.glob(MECHANIC_LEVEL_SCRIPT_GLOB),
            *repo_root.glob(PART_LOCAL_SCRIPT_GLOB),
        )
    }
    return tuple(sorted(homes))


def unittest_commands(repo_root: Path = REPO_ROOT) -> tuple[tuple[str, ...], ...]:
    return tuple(
        ("python", "-m", "unittest", "discover", "-s", (home / "tests").as_posix(), "-p", "test*.py")
        for home in discovered_test_homes(repo_root)
    )


def builder_check_commands(repo_root: Path = REPO_ROOT) -> tuple[tuple[str, ...], ...]:
    commands: list[tuple[str, ...]] = []
    for home in discovered_script_homes(repo_root):
        for script_path in sorted((repo_root / home / "scripts").glob("build_*.py")):
            commands.append(("python", script_path.relative_to(repo_root).as_posix(), "--check"))
    return tuple(commands)


def validator_commands(repo_root: Path = REPO_ROOT) -> tuple[tuple[str, ...], ...]:
    commands: list[tuple[str, ...]] = []
    for home in discovered_script_homes(repo_root):
        for script_path in sorted((repo_root / home / "scripts").glob("validate_*.py")):
            commands.append(("python", script_path.relative_to(repo_root).as_posix()))
    return tuple(commands)


def coverage_commands(repo_root: Path = REPO_ROOT) -> tuple[tuple[str, ...], ...]:
    return (
        *unittest_commands(repo_root),
        *builder_check_commands(repo_root),
        *validator_commands(repo_root),
    )


def resolve_command(command: tuple[str, ...]) -> tuple[str, ...]:
    if command and command[0] == "python":
        return (sys.executable, *command[1:])
    return command


def run_commands(commands: Iterable[tuple[str, ...]], repo_root: Path = REPO_ROOT) -> int:
    ran = 0
    for command in commands:
        print(f"[mechanics-local] {' '.join(command)}", flush=True)
        subprocess.run(resolve_command(command), cwd=repo_root, check=True)
        ran += 1
    return ran


def main() -> int:
    test_files = discovered_mechanics_local_test_files(REPO_ROOT)
    if not test_files:
        print("[error] no mechanics-local unittest files were discovered", file=sys.stderr)
        return 1

    builders = builder_check_commands(REPO_ROOT)
    validators = validator_commands(REPO_ROOT)
    if not builders:
        print("[error] no mechanics-local builder --check commands were discovered", file=sys.stderr)
        return 1
    if not validators:
        print("[error] no mechanics-local validator commands were discovered", file=sys.stderr)
        return 1

    run_commands(coverage_commands(REPO_ROOT), REPO_ROOT)
    print(
        "[ok] completed mechanics-local unittest, builder, and validator "
        f"coverage across {len(test_files)} test files"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
