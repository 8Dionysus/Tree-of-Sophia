from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts/validate_local_stats_port.py"


def _write_owner(stats_root: Path, *, exit_code: int, marker: str) -> Path:
    validator = stats_root / "scripts/validate_stats_protocol.py"
    validator.parent.mkdir(parents=True, exist_ok=True)
    validator.write_text(
        """from __future__ import annotations

import os
import sys

print(%r)
print(%r, file=sys.stderr)
print(f"owner args={sys.argv[1:]!r}")
print(f"owner cwd={os.getcwd()}")
raise SystemExit(%d)
"""
        % (marker, marker + "-stderr", exit_code),
        encoding="utf-8",
    )
    return validator


def _write_port(path: Path) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("{}\n", encoding="utf-8")
    return path


def _run_wrapper(
    stats_root: Path,
    port: Path,
    *,
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    if environment:
        env.update(environment)
    return subprocess.run(
        [
            sys.executable,
            str(SCRIPT_PATH),
            "--stats-root",
            str(stats_root),
            "--port",
            str(port),
        ],
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )


class LocalStatsPortCliTests(unittest.TestCase):
    def test_explicit_owner_and_port_with_spaces_preserve_streams_and_status(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-stats-port-") as raw:
            root = Path(raw)
            stats_root = root / "explicit owner with spaces"
            _write_owner(stats_root, exit_code=23, marker="explicit-owner")
            port = _write_port(root / "port parent with spaces" / "nested" / "port.json")

            result = _run_wrapper(stats_root, port)

            absolute_port = port.absolute()
            expected_cwd = absolute_port.parent.parent
            self.assertEqual(result.returncode, 23)
            self.assertIn("explicit-owner", result.stdout)
            self.assertIn("explicit-owner-stderr", result.stderr)
            self.assertIn(f"owner args=['--port', {str(absolute_port)!r}]", result.stdout)
            self.assertIn(f"owner cwd={expected_cwd}", result.stdout)

    def test_explicit_owner_ignores_environment_and_legacy_fallbacks(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-stats-port-fallback-") as raw:
            root = Path(raw)
            explicit_root = root / "explicit"
            environment_root = root / "fake env owner"
            sibling_root = root / "legacy sibling owner"
            _write_owner(explicit_root, exit_code=31, marker="explicit-owner")
            _write_owner(environment_root, exit_code=73, marker="environment-owner")
            _write_owner(sibling_root, exit_code=89, marker="sibling-owner")
            port = _write_port(root / "port" / "port.json")

            result = _run_wrapper(
                explicit_root,
                port,
                environment={"AOA_STATS_ROOT": str(environment_root)},
            )

            self.assertEqual(result.returncode, 31)
            self.assertIn("explicit-owner", result.stdout)
            self.assertNotIn("environment-owner", result.stdout)
            self.assertNotIn("sibling-owner", result.stdout)
            wrapper_source = SCRIPT_PATH.read_text(encoding="utf-8")
            self.assertNotIn("AOA_STATS_ROOT", wrapper_source)
            self.assertNotIn("candidate_roots", wrapper_source)
            self.assertNotIn(".deps", wrapper_source)

    def test_required_options_are_rejected_when_missing(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-stats-port-required-") as raw:
            root = Path(raw)
            stats_root = root / "owner"
            _write_owner(stats_root, exit_code=0, marker="should-not-run")
            port = _write_port(root / "port.json")

            cases = (
                [],
                ["--stats-root", str(stats_root)],
                ["--port", str(port)],
            )
            for arguments in cases:
                with self.subTest(arguments=arguments):
                    result = subprocess.run(
                        [sys.executable, str(SCRIPT_PATH), *arguments],
                        cwd=REPO_ROOT,
                        capture_output=True,
                        text=True,
                        check=False,
                    )
                    self.assertNotEqual(result.returncode, 0)

    def test_missing_and_symlink_inputs_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-stats-port-inputs-") as raw:
            root = Path(raw)
            valid_root = root / "valid-owner"
            _write_owner(valid_root, exit_code=0, marker="should-not-run")
            valid_port = _write_port(root / "valid-port.json")

            root_link = root / "owner-link"
            root_link.symlink_to(valid_root, target_is_directory=True)
            port_link = root / "port-link.json"
            port_link.symlink_to(valid_port)

            symlink_program_root = root / "symlink-program-owner"
            target_program = root / "program-target.py"
            target_program.write_text("raise SystemExit(0)\n", encoding="utf-8")
            symlink_program = symlink_program_root / "scripts/validate_stats_protocol.py"
            symlink_program.parent.mkdir(parents=True)
            symlink_program.symlink_to(target_program)

            cases = (
                (root / "missing-owner", valid_port),
                (valid_root, root / "missing-port.json"),
                (root_link, valid_port),
                (valid_root, port_link),
                (symlink_program_root, valid_port),
            )
            for stats_root, port in cases:
                with self.subTest(stats_root=stats_root, port=port):
                    result = _run_wrapper(stats_root, port)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("[error]", result.stderr)


if __name__ == "__main__":
    unittest.main()
