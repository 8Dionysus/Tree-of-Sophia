#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
from typing import TypeAlias

REPO_ROOT = Path(__file__).resolve().parents[1]
Issue: TypeAlias = tuple[str, str]

REQUIRED_AGENTS: dict[str, tuple[str, ...]] = {
    "docs/AGENTS.md": (
        "docs/KNOWLEDGE_MODEL.md",
        "docs/NODE_CONTRACT.md",
        "raw witness in `sources/`",
        "candidate structure in `intake/`",
        "canonical authored tree surfaces in `tree/`",
        "compatibility mirrors in `examples/`",
        "derived exports in `generated/`",
        "quest or RPG reflection vocabulary replace node, source, or authority semantics",
        "docs/REVIEW_CHECKLIST.md",
        "python scripts/validate_tiny_entry_route.py",
        "python scripts/generate_kag_export.py",
        "python scripts/validate_kag_export.py",
    ),
    "examples/AGENTS.md": (
        "current public compatibility and entry surfaces",
        "the authored node contract",
        "the current tiny-entry route",
        "bounded quest compatibility artifacts used for public-safe review or transport",
        "examples/review/",
        "one shared `node_id`",
        "`quest_catalog.min.example.json`",
        "`quest_dispatch.min.example.json`",
        "not runtime authority",
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
        "derived export artifacts under `generated/`",
        "Do not hand-edit derived payloads as the normal workflow",
        "tos.source.thus-spoke-zarathustra.prologue",
        "`entry_surface`",
        "`section_handles`",
        "`non_identity_boundary`",
        "python scripts/generate_kag_export.py",
        "python scripts/validate_kag_export.py",
    ),
    "schemas/AGENTS.md": (
        "tos-node-contract.schema.json",
        "tos-tiny-entry-route.schema.json",
        "quest.schema.json",
        "quest_dispatch.schema.json",
        "public contract change",
        "one shared authored identity",
        "Quest contracts here are operational compatibility surfaces only",
        "docs/REVIEW_CHECKLIST.md",
    ),
    "scripts/AGENTS.md": (
        "validate_tiny_entry_route.py",
        "generate_kag_export.py",
        "validate_kag_export.py",
        "validate_nested_agents.py",
        "deterministic",
        "network calls",
        "hidden state",
        "python scripts/validate_tiny_entry_route.py",
        "python scripts/validate_nested_agents.py",
        "python scripts/validate_intake_pack.py",
        "python scripts/validate_tree_node_contracts.py",
        "python scripts/validate_tree_relation_pack.py",
        "python scripts/validate_tree_example_sync.py",
        "python scripts/validate_kag_export.py",
    ),
    "sources/AGENTS.md": (
        "primary witness and source files",
        "authority is witnessed, not where it is already interpreted into node law",
        "candidate structure in `intake/`",
        "canonical authored nodes in `tree/`",
        "compatibility mirrors in `examples/`",
        "do not hide witness uncertainty behind smooth merged prose",
        "docs/REVIEW_CHECKLIST.md",
    ),
    "intake/AGENTS.md": (
        "candidate intake material",
        "staging ledge, not a throne room",
        "not primary witness authority",
        "not canonical tree law",
        "not the public route of truth",
        "not a hidden automation pipeline",
        "the source witness",
        "the pass or extraction frame",
        "the uncertainty that still remains",
        "Quest or progression language may appear here only as bounded work-tracking or compatibility support",
        "docs/REVIEW_CHECKLIST.md",
    ),
    "tree/AGENTS.md": (
        "canonical authored tree",
        "canonical authored source nodes",
        "concept, principle, lineage, event, state, support, analogy, and synthesis nodes",
        "canonical relation packs",
        "directory-scoped `node.json` files as canonical payloads",
        "route-local `edges.csv` files as canonical relation packs",
        "one authored object per directory-scoped `node.json`",
        "explicit relation packs under `tree/relations/`",
        "`examples/*.example.json` are compatibility mirrors",
        "Quest or RPG reflection vocabulary must remain adjunct-only",
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
    "tree/analogy/AGENTS.md": (
        "canonical authored analogy nodes",
        "route-local",
        "node.json",
        "examples/analogy_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
    "tree/synthesis/AGENTS.md": (
        "canonical authored synthesis nodes",
        "route-local",
        "node.json",
        "examples/synthesis_node.example.json",
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
