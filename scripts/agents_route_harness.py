#!/usr/bin/env python3
"""Evaluate documented AGENTS task routes without making behavioral claims."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import time
from pathlib import Path
from typing import Any

from build_agents_route_currentness import inheritance_stack, load_inventory, whitespace_tokens


REPO_ROOT = Path(__file__).resolve().parents[1]


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def route_text(repo_root: Path, stack: list[Path]) -> str:
    return "\n".join(path.read_text(encoding="utf-8") for path in stack if path.is_file()).lower()


def path_exists(repo_root: Path, value: str) -> bool:
    return (repo_root / value).is_file()


def current_ref(repo_root: Path) -> str:
    try:
        return subprocess.run(
            ("git", "rev-parse", "HEAD"),
            cwd=repo_root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "working-tree"


def task_prompt_digest(inventory: dict[str, Any]) -> str:
    payload = "\n".join(f"{task['id']}\n{task['prompt']}" for task in inventory["task_routes"])
    return sha256_bytes(payload.encode("utf-8"))


def evaluate_task(repo_root: Path, inventory: dict[str, Any], task: dict[str, Any], discovered: set[Path]) -> dict[str, Any]:
    started = time.perf_counter_ns()
    owner_path = repo_root / task["owner_route"]
    target_path = repo_root / task["target"]
    stack_names = inheritance_stack(repo_root, owner_path, discovered) if owner_path.is_file() else []
    stack = [repo_root / path for path in stack_names]
    stack_text = route_text(repo_root, stack)
    missing_required_markers = [marker for marker in task["required_markers"] if marker.lower() not in stack_text]
    missing_boundary_markers = [marker for marker in task["boundary_markers"] if marker.lower() not in stack_text]
    missing_validation_paths = [path for path in task["validation_paths"] if not path_exists(repo_root, path)]
    missing_completion = [path for path in task["completion_evidence"] if not path_exists(repo_root, path)]
    missing_on_demand = [path for path in task["on_demand_surfaces"] if not path_exists(repo_root, path)]
    budget = inventory["context_budget"]
    inherited_tokens = sum(
        whitespace_tokens(path.read_text(encoding="utf-8"))
        for path in stack
        if path.is_file()
    )
    additional_tokens = sum(
        whitespace_tokens((repo_root / path).read_text(encoding="utf-8"))
        for path in task["on_demand_surfaces"]
        if path_exists(repo_root, path)
    )
    max_inherited = budget.get("task_overrides", {}).get(
        task["id"], budget["inherited_stack_max_tokens"]
    )
    completion_coverage = (
        (len(task["completion_evidence"]) - len(missing_completion)) / len(task["completion_evidence"])
        if task["completion_evidence"]
        else 0.0
    )
    owner_in_stack = task["owner_route"] in [path.relative_to(repo_root).as_posix() for path in stack]
    budget_violations = []
    if inherited_tokens > max_inherited:
        budget_violations.append(f"inherited_context_tokens>{max_inherited}")
    if additional_tokens > budget["additional_context_max_tokens"]:
        budget_violations.append(
            f"additional_context_tokens>{budget['additional_context_max_tokens']}"
        )
    if len(stack) - 1 > budget["owner_hops_max"]:
        budget_violations.append(f"owner_hops>{budget['owner_hops_max']}")
    if len(missing_required_markers) > budget["missing_task_law_max"]:
        budget_violations.append("missing_task_specific_law")
    if len(missing_boundary_markers) > budget["boundary_deviation_max"]:
        budget_violations.append("boundary_deviation")
    if completion_coverage < budget["completion_coverage_min"]:
        budget_violations.append("completion_coverage")
    elapsed_ms = round((time.perf_counter_ns() - started) / 1_000_000, 3)
    route_success = not (
        missing_required_markers
        or missing_boundary_markers
        or missing_validation_paths
        or missing_completion
        or missing_on_demand
        or not target_path.is_file()
        or not owner_path.is_file()
        or not owner_in_stack
        or budget_violations
    )
    return {
        "id": task["id"],
        "prompt": task["prompt"],
        "target": task["target"],
        "owner_route": task["owner_route"],
        "route_success": route_success,
        "inherited_context_tokens": inherited_tokens,
        "additional_context_tokens": additional_tokens,
        "owner_hops": max(len(stack) - 1, 0),
        "time_to_owner_ms": elapsed_ms,
        "route_resolution_measurement": "harness wall-clock route lookup; not model or human behavior",
        "inheritance_stack": [path.relative_to(repo_root).as_posix() for path in stack],
        "declared_extra_reads": {
            "count": len(task["on_demand_surfaces"]),
            "paths": task["on_demand_surfaces"],
            "missing_paths": missing_on_demand,
            "behavioral_extra_reads": None,
        },
        "selected_validation": {
            "lanes": task["validation_lanes"],
            "paths": task["validation_paths"],
        },
        "boundary_deviation": {
            "count": len(missing_boundary_markers),
            "missing_markers": missing_boundary_markers,
        },
        "missing_task_specific_law": {
            "count": len(missing_required_markers),
            "missing_markers": missing_required_markers,
        },
        "completion_coverage": completion_coverage,
        "completion_evidence": {
            "paths": task["completion_evidence"],
            "missing_paths": missing_completion,
        },
        "target_exists": target_path.is_file(),
        "owner_exists": owner_path.is_file(),
        "owner_in_inheritance_stack": owner_in_stack,
        "budget": {"inherited_stack_max_tokens": max_inherited, "violations": budget_violations},
        "handoff_routes": task["handoff_routes"],
    }


def build_result(repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    inventory_path, inventory = load_inventory(repo_root)
    from build_agents_route_currentness import discover_route_cards

    discovered = set(discover_route_cards(repo_root, inventory))
    tasks = [evaluate_task(repo_root, inventory, task, discovered) for task in inventory["task_routes"]]
    successful = sum(1 for task in tasks if task["route_success"])
    return {
        "schema_version": "tos_agents_route_harness_result_v1",
        "harness": "deterministic route-shape evaluator; no behavioral or semantic acceptance claim",
        "source_ref": current_ref(repo_root),
        "inventory_ref": inventory_path.relative_to(repo_root).as_posix(),
        "inventory_sha256": sha256_bytes(inventory_path.read_bytes()),
        "task_prompt_digest": task_prompt_digest(inventory),
        "task_count": len(tasks),
        "route_success_count": successful,
        "route_success_rate": successful / len(tasks) if tasks else 0.0,
        "behavioral_model_runs": 0,
        "behavioral_claim": None,
        "tasks": tasks,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="return non-zero when a route or budget guard fails")
    parser.add_argument("--output", type=Path, default=None, help="write the result JSON to this path")
    parser.add_argument("--source-ref", default=None, help="record an immutable source ref instead of resolving HEAD")
    args = parser.parse_args()
    result = build_result(REPO_ROOT)
    if args.source_ref:
        result["source_ref"] = args.source_ref
    rendered = json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.output:
        output = args.output if args.output.is_absolute() else REPO_ROOT / args.output
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(rendered, encoding="utf-8")
        print(f"wrote {output}")
    elif not args.check:
        print(rendered, end="")
    failures = [task for task in result["tasks"] if not task["route_success"]]
    if args.check:
        if failures:
            print(f"AGENTS route harness failed for {len(failures)}/{result['task_count']} task routes")
            return 1
        print(f"AGENTS route harness passed for {result['task_count']} task routes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
