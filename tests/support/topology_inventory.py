from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
TEST_INVENTORY_PATH = REPO_ROOT / "tests" / "test_inventory.json"
TEST_TOPOLOGY_PATH = REPO_ROOT / "docs" / "testing" / "TEST_TOPOLOGY.md"
VALIDATION_LANES_PATH = REPO_ROOT / "docs" / "validation" / "validation_lanes.json"

COMMAND_RE = re.compile(r"(^|\\s)(python|pytest|bash|sh)\\s|scripts/.+\\.py")


def load_inventory() -> dict[str, Any]:
    return json.loads(TEST_INVENTORY_PATH.read_text(encoding="utf-8"))


def load_validation_lanes() -> dict[str, Any]:
    return json.loads(VALIDATION_LANES_PATH.read_text(encoding="utf-8"))["lanes"]


def inventory_entries() -> list[dict[str, Any]]:
    return load_inventory()["tests"]


def discovered_test_files(repo_root: Path = REPO_ROOT) -> set[str]:
    return {
        path.relative_to(repo_root).as_posix()
        for path in repo_root.rglob("test*.py")
        if path.is_file()
        and "__pycache__" not in path.parts
        and path.suffix == ".py"
        and ".venv" not in path.parts
    }


def classify_test_home(relative_path: str) -> tuple[str, str]:
    path = Path(relative_path)
    parts = path.parts

    if len(parts) >= 2 and parts[0] == "tests":
        return "root", "tests"
    if len(parts) >= 4 and parts[0] == "mechanics" and parts[2] == "tests":
        return "mechanic-level", "/".join(parts[:3])
    if "parts" in parts and "tests" in parts:
        tests_index = parts.index("tests")
        if parts[0] == "mechanics" and tests_index >= 4:
            return "part-local", "/".join(parts[:tests_index])
    if len(parts) >= 3 and parts[0] == ".agents" and "tests" in parts:
        tests_index = parts.index("tests")
        return "agent-lane", "/".join(parts[: tests_index + 1])

    return "root", str(path.parent)


def looks_like_command(value: str) -> bool:
    return bool(COMMAND_RE.search(value))
