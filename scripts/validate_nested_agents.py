#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
from typing import TypeAlias

REPO_ROOT = Path(__file__).resolve().parents[1]
Issue: TypeAlias = tuple[str, str]

REQUIRED_AGENTS: dict[str, tuple[str, ...]] = {
    "docs/AGENTS.md": (
        "KNOWLEDGE_MODEL.md",
        "PRINCIPLE_NODE_TEMPLATE.md",
        "EVENT_NODE_TEMPLATE.md",
        "STATE_NODE_TEMPLATE.md",
        "SUPPORT_NODE_TEMPLATE.md",
        "RELATION_PACK_CONTRACT.md",
        "ZARATHUSTRA_TRILINGUAL_ENTRY.md",
        "Keep `README.md` as the current public `tos-root`",
        "docs/REVIEW_CHECKLIST.md",
        "python scripts/validate_kag_export.py",
    ),
    "examples/AGENTS.md": (
        "source_node.example.json",
        "concept_node.example.json",
        "principle_node.example.json",
        "lineage_node.example.json",
        "event_node.example.json",
        "state_node.example.json",
        "support_node.example.json",
        "tos_tiny_entry_route.example.json",
        "one shared `node_id`",
        "examples/review/",
        "docs/REVIEW_CHECKLIST.md",
    ),
    "examples/review/AGENTS.md": (
        "review/archive material",
        "superseded",
        "not active canon",
        "examples/review/calibration-family-pilot/",
        "docs/REVIEW_CHECKLIST.md",
    ),
    "generated/AGENTS.md": (
        "kag_export.json",
        "kag_export.min.json",
        "Do not hand-edit",
        "tos.source.thus-spoke-zarathustra.prologue",
        "examples/source_node.example.json",
        "python scripts/generate_kag_export.py",
    ),
    "schemas/AGENTS.md": (
        "tos-node-contract.schema.json",
        "tos-tiny-entry-route.schema.json",
        "public contract change",
        "one shared authored node",
        "docs/REVIEW_CHECKLIST.md",
    ),
    "scripts/AGENTS.md": (
        "generate_kag_export.py",
        "tree_example_sync.py",
        "sync_tree_examples.py",
        "validate_tree_example_sync.py",
        "validate_tree_node_contracts.py",
        "validate_tree_relation_pack.py",
        "validate_kag_export.py",
        "validate_nested_agents.py",
        "deterministic",
        "tree-to-example compatibility",
        "python scripts/validate_nested_agents.py",
        "python scripts/validate_tree_node_contracts.py",
        "python scripts/validate_tree_relation_pack.py",
        "python scripts/validate_tree_example_sync.py",
        "python scripts/validate_kag_export.py",
    ),
    "sources/AGENTS.md": (
        "primary witness and source files",
        "not node law",
        "candidate",
        "canonical authored nodes in `tree/`",
        "docs/REVIEW_CHECKLIST.md",
    ),
    "intake/AGENTS.md": (
        "candidate intake material",
        "candidate material, not source authority",
        "sources/",
        "tree/",
        "docs/REVIEW_CHECKLIST.md",
    ),
    "tree/AGENTS.md": (
        "canonical authored tree",
        "canonical authored event nodes",
        "canonical authored state nodes",
        "canonical authored support nodes",
        "canonical authored principle nodes",
        "canonical authored lineage nodes",
        "canonical authored relation packs",
        "node.json",
        "examples/event_node.example.json",
        "examples/state_node.example.json",
        "examples/support_node.example.json",
        "examples/principle_node.example.json",
        "examples/lineage_node.example.json",
        "examples/source_node.example.json",
        "compatibility mirrors",
        "python scripts/validate_tree_relation_pack.py",
        "python scripts/validate_tree_example_sync.py",
    ),
    "tree/lineage/AGENTS.md": (
        "canonical authored lineage nodes",
        "route-local",
        "node.json",
        "examples/lineage_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
    "tree/event/AGENTS.md": (
        "canonical authored event nodes",
        "route-local",
        "node.json",
        "examples/event_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
    "tree/principle/AGENTS.md": (
        "canonical authored principle nodes",
        "route-local",
        "node.json",
        "examples/principle_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
    "tree/support/AGENTS.md": (
        "canonical authored support nodes",
        "route-local",
        "node.json",
        "examples/support_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
    "tree/relations/AGENTS.md": (
        "canonical relation packs",
        "route-local",
        "edges.csv",
        "canonical tos.* ids",
        "registered predicates",
        "python scripts/validate_tree_relation_pack.py",
    ),
    "tree/state/AGENTS.md": (
        "canonical authored state nodes",
        "route-local",
        "node.json",
        "examples/state_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
}


def normalize(text: str) -> str:
    return " ".join(text.lower().split())


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    repo_root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    for relative_path, required_phrases in REQUIRED_AGENTS.items():
        path = repo_root / relative_path
        if not path.is_file():
            issues.append((relative_path, "missing required nested AGENTS.md"))
            continue

        raw_text = path.read_text(encoding="utf-8")
        stripped = raw_text.strip()
        if not stripped.startswith("# AGENTS.md"):
            issues.append((relative_path, "missing '# AGENTS.md' heading"))

        text = normalize(raw_text)
        for phrase in required_phrases:
            if normalize(phrase) not in text:
                issues.append((relative_path, f"missing required phrase: {phrase}"))

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Nested AGENTS docs check failed.")
        for location, message in issues:
            print(f"- {location}: {message}")
        return 1

    print(f"Nested AGENTS docs check passed for {len(REQUIRED_AGENTS)} directories.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
