from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_INVENTORY_PATH = REPO_ROOT / "docs" / "validation" / "script_inventory.json"
SCRIPT_TOPOLOGY_PATH = REPO_ROOT / "docs" / "validation" / "SCRIPT_TOPOLOGY.md"
VALIDATION_LANES_PATH = REPO_ROOT / "docs" / "validation" / "validation_lanes.json"

SCRIPT_REF_RE = re.compile(r"(?<![\\w./-])([\\w./-]*scripts/[\\w./-]+\\.py)")


def load_inventory() -> dict[str, Any]:
    return json.loads(SCRIPT_INVENTORY_PATH.read_text(encoding="utf-8"))


def inventory_entries() -> list[dict[str, Any]]:
    return load_inventory()["script_surfaces"]


def inventory_paths() -> set[str]:
    return {entry["path"] for entry in inventory_entries()}


def load_validation_lanes() -> dict[str, Any]:
    return json.loads(VALIDATION_LANES_PATH.read_text(encoding="utf-8"))["lanes"]


def discovered_script_surfaces(repo_root: Path = REPO_ROOT) -> set[str]:
    return {
        path.relative_to(repo_root).as_posix()
        for path in repo_root.rglob("*")
        if path.is_file()
        and "/scripts/" in f"/{path.relative_to(repo_root).as_posix()}"
        and "__pycache__" not in path.parts
        and path.suffix != ".pyc"
    }


def command_script_paths() -> set[str]:
    manifest = json.loads(VALIDATION_LANES_PATH.read_text(encoding="utf-8"))
    paths: set[str] = set()
    for steps in manifest["command_sequences"].values():
        for step in steps:
            command = step.get("command", [])
            if not isinstance(command, list):
                continue
            for item in command:
                if isinstance(item, str) and item.endswith(".py") and "/" in item:
                    paths.add(item)
    return paths


def resolve_local_script_ref(doc: Path, raw_ref: str) -> str | None:
    raw_ref = raw_ref.removeprefix("./")
    if raw_ref.startswith("../"):
        resolved = (doc.parent / raw_ref).resolve()
        try:
            return resolved.relative_to(REPO_ROOT).as_posix()
        except ValueError:
            return None
    if raw_ref.startswith((".agents/", "mechanics/", "scripts/")):
        return raw_ref
    return None
