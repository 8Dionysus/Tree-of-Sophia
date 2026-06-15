#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
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

    return issues


def command_sequence(sequence_id: str, repo_root: Path | None = None) -> list[CommandStep]:
    manifest = load_manifest(repo_root)
    sequences = manifest.get("command_sequences")
    if not isinstance(sequences, dict):
        raise ValueError("command_sequences must be an object")
    steps = sequences.get(sequence_id)
    if not isinstance(steps, list):
        raise KeyError(f"unknown command sequence: {sequence_id}")

    resolved: list[CommandStep] = []
    for step in steps:
        if not isinstance(step, dict):
            raise ValueError(f"{sequence_id} contains a non-object step")
        label = step.get("label")
        command = step.get("command")
        if not isinstance(label, str) or not _is_command(command):
            raise ValueError(f"{sequence_id} contains an invalid command step")
        command_parts = list(command)
        if command_parts[0] == "python":
            command_parts[0] = sys.executable
        resolved.append((label, command_parts))
    return resolved


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Inspect ToS validation lanes.")
    parser.add_argument("--check", action="store_true", help="validate the lane manifest")
    parser.add_argument("--sequence", help="print a named command sequence")
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
        for label, command in command_sequence(args.sequence, REPO_ROOT):
            print(f"{label}: {' '.join(command)}")

    if not args.check and not args.sequence:
        parser.print_help()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
