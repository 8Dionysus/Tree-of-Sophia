#!/usr/bin/env python3
"""Build the generated read model for AGENTS route-card currentness."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = REPO_ROOT / "docs/validation/agents_route_inventory.json"
DEFAULT_CURRENTNESS_PATH = REPO_ROOT / ".agents/agents-route.current.json"


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def sha256_path(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def relative_path(repo_root: Path, path: Path) -> str:
    return path.relative_to(repo_root).as_posix()


def whitespace_tokens(text: str) -> int:
    return len(text.split())


def load_inventory(repo_root: Path = REPO_ROOT) -> tuple[Path, dict[str, Any]]:
    path = repo_root / "docs/validation/agents_route_inventory.json"
    return path, json.loads(path.read_text(encoding="utf-8"))


def discover_route_cards(repo_root: Path, inventory: dict[str, Any]) -> list[Path]:
    discovery = inventory["route_card_discovery"]
    paths = [repo_root / value for value in discovery["root_cards"]]
    for root_name in discovery["route_roots"]:
        root = repo_root / root_name
        if root.exists():
            paths.extend(sorted(root.rglob("AGENTS.md")))
    return sorted({path for path in paths if path.is_file()})


def tracked_route_cards(repo_root: Path) -> list[str]:
    result = subprocess.run(
        ("git", "ls-files", "--", "*.md"),
        cwd=repo_root,
        check=True,
        capture_output=True,
        text=True,
    )
    return sorted(
        value
        for value in result.stdout.splitlines()
        if Path(value).name == "AGENTS.md"
    )


def inheritance_stack(repo_root: Path, card: Path, discovered: set[Path]) -> list[str]:
    ancestors: list[Path] = []
    cursor = card.parent
    while True:
        candidate = repo_root / "AGENTS.md" if cursor == repo_root else cursor / "AGENTS.md"
        if candidate in discovered:
            ancestors.append(candidate)
        if cursor == repo_root:
            break
        cursor = cursor.parent
    ancestors.reverse()
    if card not in ancestors:
        ancestors.append(card)
    return [relative_path(repo_root, value) for value in ancestors]


def target_inheritance_stack(repo_root: Path, target: Path, discovered: set[Path]) -> list[str]:
    """Resolve the inherited cards from the file being acted on.

    An owner route may be a deliberate handoff outside the target's local
    scope.  It must not replace the nearest cards that govern the target.
    """

    ancestors: list[Path] = []
    cursor = target if target.is_dir() else target.parent
    while True:
        candidate = repo_root / "AGENTS.md" if cursor == repo_root else cursor / "AGENTS.md"
        if candidate in discovered:
            ancestors.append(candidate)
        if cursor == repo_root:
            break
        cursor = cursor.parent
    ancestors.reverse()
    return [relative_path(repo_root, value) for value in ancestors]


def path_record(repo_root: Path, value: str, *, role: str) -> dict[str, Any]:
    path = repo_root / value
    if not path.is_file():
        return {"path": value, "role": role, "exists": False}
    payload = path.read_bytes()
    text = payload.decode("utf-8")
    return {
        "path": value,
        "role": role,
        "exists": True,
        "bytes": len(payload),
        "lines": len(text.splitlines()),
        "whitespace_tokens": whitespace_tokens(text),
        "sha256": sha256_bytes(payload),
    }


def build_currentness(repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    inventory_path, inventory = load_inventory(repo_root)
    discovered_paths = discover_route_cards(repo_root, inventory)
    discovered = set(discovered_paths)
    discovered_names = sorted(relative_path(repo_root, path) for path in discovered_paths)
    tracked_names = tracked_route_cards(repo_root)
    tracked = set(tracked_names)
    card_records: list[dict[str, Any]] = []
    for path in discovered_paths:
        value = relative_path(repo_root, path)
        text = path.read_text(encoding="utf-8")
        card_records.append(
            {
                "path": value,
                "tracked": value in tracked,
                "bytes": path.stat().st_size,
                "lines": len(text.splitlines()),
                "whitespace_tokens": whitespace_tokens(text),
                "sha256": sha256_path(path),
                "inheritance_stack": inheritance_stack(repo_root, path, discovered),
            }
        )

    task_routes: list[dict[str, Any]] = []
    for task in inventory["task_routes"]:
        target = repo_root / task["target"]
        owner = repo_root / task["owner_route"]
        owner_exists = owner.is_file()
        stack = target_inheritance_stack(repo_root, target, discovered)
        inherited_tokens = sum(
            whitespace_tokens((repo_root / path).read_text(encoding="utf-8"))
            for path in stack
            if (repo_root / path).is_file()
        )
        owner_handoff = path_record(repo_root, task["owner_route"], role="owner_handoff")
        additional = [path_record(repo_root, path, role="on_demand") for path in task["on_demand_surfaces"]]
        task_routes.append(
            {
                "id": task["id"],
                "target": task["target"],
                "target_exists": target.is_file(),
                "owner_route": task["owner_route"],
                "owner_exists": owner_exists,
                "inheritance_stack": stack,
                "inherited_context_tokens": inherited_tokens,
                "owner_in_inheritance_stack": task["owner_route"] in stack,
                "owner_handoff": owner_handoff,
                "additional_context_tokens": sum(
                    item.get("whitespace_tokens", 0) for item in additional
                ),
                "on_demand_surfaces": additional,
                "validation_paths": [
                    path_record(repo_root, path, role="validation")
                    for path in task["validation_paths"]
                ],
                "completion_evidence": [
                    path_record(repo_root, path, role="completion")
                    for path in task["completion_evidence"]
                ],
            }
        )

    preserved = [
        path_record(repo_root, entry["path"], role=entry["role"])
        for entry in inventory["route_card_discovery"]["preserved_non_cards"]
    ]
    influencing = [
        path_record(repo_root, entry["path"], role=entry["role"])
        for entry in inventory["influencing_surfaces"]
    ]
    inventory_bytes = inventory_path.read_bytes()
    return {
        "schema_version": "tos_agents_route_currentness_v1",
        "source_inventory": relative_path(repo_root, inventory_path),
        "source_inventory_sha256": sha256_bytes(inventory_bytes),
        "generated_by": "scripts/build_agents_route_currentness.py",
        "route_card_counts": {
            "tracked_exact_basename": len(tracked_names),
            "discovered_by_inventory": len(discovered_names),
            "tracked_only": sorted(tracked - set(discovered_names)),
            "discovered_untracked": sorted(set(discovered_names) - tracked),
        },
        "route_card_discovery": inventory["route_card_discovery"],
        "cards": card_records,
        "preserved_non_cards": preserved,
        "influencing_surfaces": influencing,
        "task_routes": task_routes,
        "context_budget": inventory["context_budget"],
        "scope_duplication_policy": inventory["scope_duplication_policy"],
    }


def render_currentness(data: dict[str, Any]) -> str:
    return json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def resolve_output_path(repo_root: Path, output: Path | None) -> Path:
    if output is None:
        inventory = json.loads(INVENTORY_PATH.read_text(encoding="utf-8"))
        return repo_root / inventory["currentness"]
    return output if output.is_absolute() else repo_root / output


def display_output_path(repo_root: Path, output: Path) -> str:
    try:
        return output.relative_to(repo_root).as_posix()
    except ValueError:
        return output.as_posix()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="check generated currentness without writing")
    parser.add_argument("--output", type=Path, default=None, help="override the generated output path")
    args = parser.parse_args()
    output = resolve_output_path(REPO_ROOT, args.output)
    rendered = render_currentness(build_currentness(REPO_ROOT))
    if args.check:
        if not output.is_file() or output.read_text(encoding="utf-8") != rendered:
            print(f"AGENTS route currentness is stale or missing: {display_output_path(REPO_ROOT, output)}")
            return 1
        print(f"AGENTS route currentness is current: {display_output_path(REPO_ROOT, output)}")
        return 0
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(rendered, encoding="utf-8")
    print(f"wrote {display_output_path(REPO_ROOT, output)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
