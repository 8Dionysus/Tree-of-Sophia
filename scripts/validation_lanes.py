#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path
from typing import TypeAlias


REPO_ROOT = Path(__file__).resolve().parents[1]
LANES_PATH = Path("docs/validation/validation_lanes.json")

Issue: TypeAlias = tuple[str, str]
CommandStep: TypeAlias = tuple[str, list[str]]

REQUIRED_LANE_FIELDS = (
    "label",
    "layer",
    "mode",
    "owner_surface",
    "purpose",
    "failure_route",
    "does_not_own",
)


def load_manifest(repo_root: Path | None = None) -> dict[str, object]:
    root = repo_root or REPO_ROOT
    path = root / LANES_PATH
    return json.loads(path.read_text(encoding="utf-8"))


def _is_command(value: object) -> bool:
    return isinstance(value, list) and bool(value) and all(isinstance(part, str) and part for part in value)


def _command_timeout_ms(sequence_id: str, step: dict) -> int | None:
    if "command_timeout_ms" not in step:
        return None
    value = step["command_timeout_ms"]
    if sequence_id != "rust_workspace":
        raise ValueError("command_timeout_ms is only supported for rust_workspace")
    if type(value) is not int or not 1 <= value <= 3_600_000:
        raise ValueError("command_timeout_ms must be an integer in 1..=3600000")
    return value


def validate_manifest(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []
    path = root / LANES_PATH
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return [(LANES_PATH.as_posix(), "missing validation lane manifest")]
    except json.JSONDecodeError as exc:
        return [(LANES_PATH.as_posix(), f"invalid JSON: {exc}")]

    if not isinstance(manifest, dict):
        return [(LANES_PATH.as_posix(), "manifest root must be an object")]
    if manifest.get("schema_version") != "tos_validation_lanes_v1":
        issues.append((LANES_PATH.as_posix(), "schema_version must be tos_validation_lanes_v1"))
    if manifest.get("owner_repo") != "Tree-of-Sophia":
        issues.append((LANES_PATH.as_posix(), "owner_repo must be Tree-of-Sophia"))
    if manifest.get("command_authority") != LANES_PATH.as_posix():
        issues.append((LANES_PATH.as_posix(), "command_authority must point at this manifest"))

    lanes = manifest.get("lanes")
    if not isinstance(lanes, dict) or not lanes:
        issues.append((LANES_PATH.as_posix(), "lanes must be a non-empty object"))
        return issues

    sequences = manifest.get("command_sequences")
    if not isinstance(sequences, dict):
        issues.append((LANES_PATH.as_posix(), "command_sequences must be an object"))
        sequences = {}

    for lane_id, lane in lanes.items():
        if not isinstance(lane_id, str) or not lane_id:
            issues.append((LANES_PATH.as_posix(), "lane id must be a non-empty string"))
            continue
        if not isinstance(lane, dict):
            issues.append((LANES_PATH.as_posix(), f"{lane_id} lane must be an object"))
            continue
        for field in REQUIRED_LANE_FIELDS:
            if field not in lane:
                issues.append((LANES_PATH.as_posix(), f"{lane_id}.{field} is required"))
        owner_surface = lane.get("owner_surface")
        if isinstance(owner_surface, str) and owner_surface and not (root / owner_surface).exists():
            issues.append((owner_surface, f"{lane_id}.owner_surface is missing"))
        failure_route = lane.get("failure_route")
        if isinstance(failure_route, str) and failure_route and not (root / failure_route).exists():
            issues.append((failure_route, f"{lane_id}.failure_route is missing"))
        does_not_own = lane.get("does_not_own")
        if not isinstance(does_not_own, list) or not all(isinstance(item, str) and item for item in does_not_own):
            issues.append((LANES_PATH.as_posix(), f"{lane_id}.does_not_own must be a non-empty string list"))
        sequence_id = lane.get("command_sequence")
        focused_target = lane.get("focused_target")
        if sequence_id is None and focused_target is None:
            issues.append((LANES_PATH.as_posix(), f"{lane_id} needs command_sequence or focused_target"))
        if isinstance(sequence_id, str) and sequence_id not in sequences:
            issues.append((LANES_PATH.as_posix(), f"{lane_id}.command_sequence references missing {sequence_id}"))

    for sequence_id, steps in sequences.items():
        if not isinstance(sequence_id, str) or not sequence_id:
            issues.append((LANES_PATH.as_posix(), "command sequence id must be a non-empty string"))
            continue
        if not isinstance(steps, list) or not steps:
            issues.append((LANES_PATH.as_posix(), f"{sequence_id} command sequence must be a non-empty list"))
            continue
        for index, step in enumerate(steps):
            location = f"{sequence_id}[{index}]"
            if not isinstance(step, dict):
                issues.append((LANES_PATH.as_posix(), f"{location} must be an object"))
                continue
            label = step.get("label")
            command = step.get("command")
            if not isinstance(label, str) or not label:
                issues.append((LANES_PATH.as_posix(), f"{location}.label must be a non-empty string"))
            if not _is_command(command):
                issues.append((LANES_PATH.as_posix(), f"{location}.command must be a non-empty string list"))
            try:
                _command_timeout_ms(sequence_id, step)
            except ValueError as exc:
                issues.append((LANES_PATH.as_posix(), f"{location}: {exc}"))

    return issues


def command_sequence(sequence_id: str, repo_root: Path | None = None) -> list[CommandStep]:
    manifest = load_manifest(repo_root)
    sequences = manifest.get("command_sequences")
    if not isinstance(sequences, dict):
        raise ValueError("command_sequences must be an object")
    steps = sequences.get(sequence_id)
    if not isinstance(steps, list):
        raise KeyError(f"unknown command sequence: {sequence_id}")
    if not steps:
        raise ValueError(f"{sequence_id} must contain at least one command")

    resolved: list[CommandStep] = []
    for step in steps:
        if not isinstance(step, dict):
            raise ValueError(f"{sequence_id} contains a non-object step")
        label = step.get("label")
        command = step.get("command")
        if not isinstance(label, str) or not _is_command(command):
            raise ValueError(f"{sequence_id} contains an invalid command step")
        _command_timeout_ms(sequence_id, step)
        command_parts = list(command)
        if command_parts[0] == "python":
            command_parts[0] = sys.executable
        resolved.append((label, command_parts))
    return resolved


def _cargo_test_artifacts(
    messages: list[dict[str, object]], package_name: str, target_name: str
) -> set[Path]:
    """Select the executable emitted for one exact Cargo package test target."""
    artifacts: set[Path] = set()
    for message in messages:
        if message.get("reason") != "compiler-artifact":
            continue
        target = message.get("target")
        executable = message.get("executable")
        package_id = message.get("package_id")
        if not isinstance(target, dict) or not isinstance(package_id, str):
            continue
        kinds = target.get("kind")
        if not isinstance(kinds, list):
            continue
        package_id_name = package_id.rsplit("#", 1)[-1].split("@", 1)[0]
        if (
            package_id_name == package_name
            and target.get("name") == target_name
            and "test" in kinds
            and isinstance(executable, str)
        ):
            artifacts.add(Path(executable))
    return artifacts


def _sha256_executable(
    raw_path: str | Path, description: str, cargo_target: Path, root: Path
) -> tuple[Path, str]:
    """Hash one executable produced in this lane's Cargo target directory."""
    path = Path(raw_path)
    if not path.is_absolute():
        raise ValueError(f"{description} path must be absolute")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as exc:
        raise ValueError(f"{description} is unavailable: {path}") from exc
    if not stat.S_ISREG(metadata.st_mode) or not metadata.st_mode & 0o111:
        raise ValueError(f"{description} must be a regular executable: {path}")
    if metadata.st_size > 512 * 1024 * 1024:
        raise ValueError(f"{description} exceeds the 512 MiB image bound: {path}")
    target_root = cargo_target if cargo_target.is_absolute() else root / cargo_target
    try:
        resolved.relative_to(target_root.resolve())
    except ValueError as exc:
        raise ValueError(f"{description} must be produced inside Cargo target {target_root}: {path}") from exc
    with resolved.open("rb") as stream:
        return resolved, hashlib.file_digest(stream, "sha256").hexdigest()


def _run_cargo_json_build(
    command: list[str], root: Path, env: dict[str, str], package_name: str, target_name: str
) -> tuple[int, set[Path]]:
    """Stream Cargo diagnostics and retain one exact compiler-artifact test image."""
    process = subprocess.Popen(
        command,
        cwd=root,
        env=env,
        stdout=subprocess.PIPE,
        stderr=None,
        text=True,
    )
    artifacts: set[Path] = set()
    assert process.stdout is not None
    with process.stdout:
        for line in process.stdout:
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                print(line, end="", flush=True)
                continue
            if not isinstance(message, dict):
                continue
            artifacts.update(_cargo_test_artifacts([message], package_name, target_name))
            if message.get("reason") == "compiler-message":
                detail = message.get("message")
                rendered = detail.get("rendered") if isinstance(detail, dict) else None
                if isinstance(rendered, str):
                    print(rendered, end="", file=sys.stderr, flush=True)
    return process.wait(), artifacts


def run_sequence(sequence_id: str, repo_root: Path | None = None) -> int:
    root = repo_root or REPO_ROOT
    env = os.environ.copy()
    for label, command in command_sequence(sequence_id, root):
        print(f"[run] {label}: {' '.join(command)}", flush=True)
        if sequence_id == "rust_workspace" and label == "compile exact Rust conformance test image":
            return_code, artifacts = _run_cargo_json_build(
                command, root, env, "tos-conformance", "conformance"
            )
            if return_code != 0:
                print(
                    f"[error] {label} failed with exit code {return_code}",
                    file=sys.stderr,
                    flush=True,
                )
                return return_code
            if len(artifacts) != 1:
                print(
                    f"[error] expected one exact tos-conformance test image; found {len(artifacts)}",
                    file=sys.stderr,
                    flush=True,
                )
                return 1
            target_dir = Path(env.get("CARGO_TARGET_DIR", root / "target"))
            consumer = env.get("TOS_NATIVE_PREPARED_CONSUMER_BIN")
            if not consumer:
                print(
                    "[error] Rust conformance requires TOS_NATIVE_PREPARED_CONSUMER_BIN",
                    file=sys.stderr,
                    flush=True,
                )
                return 1
            try:
                _, consumer_sha = _sha256_executable(consumer, "prepared consumer", target_dir, root)
                case_path, case_sha = _sha256_executable(
                    next(iter(artifacts)), "Claim publication conformance test image", target_dir, root
                )
            except ValueError as exc:
                print(f"[error] {exc}", file=sys.stderr, flush=True)
                return 1
            env["TOS_NATIVE_PREPARED_CONSUMER_SHA256"] = consumer_sha
            env["TOS_NATIVE_CLAIM_PUBLICATION_CASE_SHA256"] = case_sha
            print(f"[artifact] prepared consumer SHA256: {consumer_sha}", flush=True)
            print(f"[artifact] exact Claim publication test image: {case_path}", flush=True)
            print(f"[artifact] Claim publication test image SHA256: {case_sha}", flush=True)
            print(f"[ok] {label}", flush=True)
            continue
        completed = subprocess.run(
            command,
            cwd=root,
            env=env,
            check=False,
        )
        if completed.returncode != 0:
            print(
                f"[error] {label} failed with exit code {completed.returncode}",
                file=sys.stderr,
                flush=True,
            )
            return completed.returncode
        print(f"[ok] {label}", flush=True)
    return 0


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Inspect ToS validation lanes.")
    parser.add_argument("--check", action="store_true", help="validate the lane manifest")
    parser.add_argument("--sequence", help="print a named command sequence")
    parser.add_argument("--run", metavar="SEQUENCE", help="execute a named command sequence")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = _parser()
    args = parser.parse_args(argv)

    if args.check:
        issues = validate_manifest(REPO_ROOT)
        if issues:
            print("Validation lane manifest check failed.", file=sys.stderr)
            for location, message in issues:
                print(f"- {location}: {message}", file=sys.stderr)
            return 1
        print("[ok] validated ToS validation lane manifest")

    if args.sequence:
        try:
            for label, command in command_sequence(args.sequence, REPO_ROOT):
                print(f"{label}: {' '.join(command)}")
        except (KeyError, ValueError) as exc:
            print(f"error: {exc}", file=sys.stderr)
            return 1

    if args.run:
        try:
            return run_sequence(args.run, REPO_ROOT)
        except (KeyError, ValueError) as exc:
            print(f"error: {exc}", file=sys.stderr)
            return 1

    if not args.check and not args.sequence and not args.run:
        parser.print_help()

    return 0


def native_main(argv: list[str] | None = None) -> int:
    """Installed CLI route; imported Python APIs remain available to callers."""
    import shutil

    parser = _parser()
    args = parser.parse_args(argv)
    if not args.check and not args.sequence and not args.run:
        parser.print_help()
        return 0
    selected = os.environ.get("TOS_VALIDATION_LANES_EXECUTOR")
    executable = selected or shutil.which("tos-validation-lanes")
    if not executable:
        print("[error] install tos-validation-lanes or set TOS_VALIDATION_LANES_EXECUTOR", file=sys.stderr)
        return 1
    arguments = []
    if args.check:
        arguments.append("--check")
    if args.sequence:
        arguments.extend(["--sequence", args.sequence])
    if args.run:
        arguments.extend(["--run", args.run])
    try:
        os.execv(executable, [executable, "--repo-root", str(REPO_ROOT),
                              "--python", sys.executable, *arguments])
    except OSError as error:
        print(f"[error] cannot execute native validation lanes: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(native_main())
