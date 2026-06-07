#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
from typing import TypeAlias

REPO_ROOT = Path(__file__).resolve().parents[1]
Issue: TypeAlias = tuple[str, str]

REQUIRED_AGENTS: dict[str, tuple[str, ...]] = {
    "ToS/AGENTS.md": (
        "source-home organ for Tree of Sophia",
        "ToS/source_home.manifest.json",
        "ToS/doctrine/KNOWLEDGE_MODEL.md",
        "ToS/doctrine/NODE_CONTRACT.md",
        "source-witnesses/",
        "philosophy/",
        "candidate-intake/",
        "canon/",
        "public-compatibility/",
        "derived-exports/",
        "contracts/",
        "review-ledger/",
        "python scripts/validate_tos_source_home.py",
        "python scripts/validate_philosophy_topology.py",
    ),
    "ToS/philosophy/AGENTS.md": (
        "growing domain branch for philosophy",
        "trunk law",
        "era branches",
        "region branches",
        "tradition subtrees",
        "works, figures, concepts, transmissions, sources, and local graph workbenches",
        "not `candidate-intake/`",
        "Name branches by what they are in the philosophical tree",
        "trunk -> eras -> regions -> traditions",
        "Do not create `zagotovki`, `world-written-philosophy`, `raw-pages`",
        "python scripts/validate_philosophy_topology.py",
    ),
    "ToS/source-witnesses/notion/philosophy/AGENTS.md": (
        "Notion witness surfaces for `ToS/philosophy/`",
        "without letting Notion UI labels become repository topology",
        "Keep source page names in metadata, not in path names",
        "Route branch-shaped philosophical material into `ToS/philosophy/`",
        "python scripts/validate_philosophy_topology.py",
    ),
    "ToS/doctrine/AGENTS.md": (
        "ToS knowledge law",
        "Durable rationale lives in `docs/decisions/`",
        "ToS/doctrine/KNOWLEDGE_MODEL.md",
        "ToS/doctrine/NODE_CONTRACT.md",
        "ToS/doctrine/REVIEW_CHECKLIST.md",
    ),
    "ToS/review-ledger/AGENTS.md": (
        "dated review notes",
        "do not overrule current doctrine",
        "Route durable rationale to `docs/decisions/`",
        "python scripts/validate_tos_source_home.py",
    ),
    "docs/AGENTS.md": (
        "repository-level documentation surfaces",
        "docs/decisions/",
        "docs/RELEASING.md",
        "docs/AGENTS_ROOT_REFERENCE.md",
        "ToS knowledge law",
        "raw witness in `ToS/source-witnesses/`",
        "domain philosophy topology in `ToS/philosophy/`",
        "candidate structure in `ToS/candidate-intake/`",
        "canonical authored canon in `ToS/canon/`",
        "compatibility mirrors in `ToS/public-compatibility/`",
        "derived exports in `ToS/derived-exports/`",
        "python scripts/validate_tos_source_home.py",
    ),
    "docs/decisions/AGENTS.md": (
        "durable ToS decision rationale",
        "Decision notes explain why",
        "weaker than the current source",
        "TOS-D-####",
        "Index Metadata",
        "ToS layers",
        "Tree classes",
        "python scripts/generate_decision_indexes.py --check",
        "python scripts/validate_decision_records.py",
    ),
    "ToS/public-compatibility/AGENTS.md": (
        "current public compatibility and entry surfaces",
        "the authored node contract",
        "the current tiny-entry route",
        "bounded quest compatibility artifacts used for public-safe review or transport",
        "ToS/public-compatibility/review/",
        "one shared `node_id`",
        "`quest_catalog.min.example.json`",
        "`quest_dispatch.min.example.json`",
        "not runtime authority",
        "ToS/doctrine/REVIEW_CHECKLIST.md",
    ),
    "ToS/public-compatibility/review/AGENTS.md": (
        "review/archive material",
        "superseded",
        "not active canon",
        "ToS/public-compatibility/review/calibration-family-pilot/",
        "ToS/doctrine/REVIEW_CHECKLIST.md",
    ),
    "ToS/derived-exports/AGENTS.md": (
        "derived export artifacts under `ToS/derived-exports/`",
        "Do not hand-edit derived payloads as the normal workflow",
        "tos.source.thus-spoke-zarathustra.prologue",
        "`entry_surface`",
        "`section_handles`",
        "`non_identity_boundary`",
        "python scripts/generate_kag_export.py",
        "python scripts/validate_kag_export.py",
    ),
    "ToS/contracts/AGENTS.md": (
        "tos-node-contract.schema.json",
        "tos-tiny-entry-route.schema.json",
        "quest.schema.json",
        "quest_dispatch.schema.json",
        "public contract change",
        "one shared authored identity",
        "Quest contracts here are operational compatibility surfaces only",
        "ToS/doctrine/REVIEW_CHECKLIST.md",
    ),
    "scripts/AGENTS.md": (
        "validate_tiny_entry_route.py",
        "validate_philosophy_topology.py",
        "generate_kag_export.py",
        "validate_kag_export.py",
        "validate_nested_agents.py",
        "deterministic",
        "network calls",
        "hidden state",
        "python scripts/validate_tiny_entry_route.py",
        "python scripts/validate_philosophy_topology.py",
        "python scripts/validate_nested_agents.py",
        "python scripts/validate_intake_pack.py",
        "python scripts/validate_tree_node_contracts.py",
        "python scripts/validate_tree_relation_pack.py",
        "python scripts/validate_tree_example_sync.py",
        "python scripts/validate_kag_export.py",
        "python scripts/generate_decision_indexes.py --check",
        "python scripts/validate_decision_records.py",
    ),
    "ToS/source-witnesses/AGENTS.md": (
        "primary witness and source files",
        "authority is witnessed, not where it is already interpreted into node law",
        "candidate structure in `ToS/candidate-intake/`",
        "canonical authored nodes in `ToS/canon/`",
        "compatibility mirrors in `ToS/public-compatibility/`",
        "do not hide witness uncertainty behind smooth merged prose",
        "ToS/doctrine/REVIEW_CHECKLIST.md",
    ),
    "ToS/candidate-intake/AGENTS.md": (
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
        "ToS/doctrine/REVIEW_CHECKLIST.md",
    ),
    "ToS/canon/AGENTS.md": (
        "canonical authored tree",
        "canonical authored source nodes",
        "concept, principle, lineage, event, state, support, analogy, and synthesis nodes",
        "canonical relation packs",
        "directory-scoped `node.json` files as canonical payloads",
        "route-local `edges.csv` files as canonical relation packs",
        "one authored object per directory-scoped `node.json`",
        "explicit relation packs under `ToS/canon/relations/`",
        "`ToS/public-compatibility/*.example.json` are compatibility mirrors",
        "Quest or RPG reflection vocabulary must remain adjunct-only",
        "python scripts/validate_tree_relation_pack.py",
        "python scripts/validate_tree_example_sync.py",
    ),
    "ToS/canon/lineage/AGENTS.md": (
        "canonical authored lineage nodes",
        "route-local",
        "node.json",
        "ToS/public-compatibility/lineage_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
    "ToS/canon/event/AGENTS.md": (
        "canonical authored event nodes",
        "route-local",
        "node.json",
        "ToS/public-compatibility/event_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
    "ToS/canon/principle/AGENTS.md": (
        "canonical authored principle nodes",
        "route-local",
        "node.json",
        "ToS/public-compatibility/principle_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
    "ToS/canon/support/AGENTS.md": (
        "canonical authored support nodes",
        "route-local",
        "node.json",
        "ToS/public-compatibility/support_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
    "ToS/canon/relations/AGENTS.md": (
        "canonical relation packs",
        "route-local",
        "edges.csv",
        "canonical tos.* ids",
        "registered predicates",
        "python scripts/validate_tree_relation_pack.py",
    ),
    "ToS/canon/state/AGENTS.md": (
        "canonical authored state nodes",
        "route-local",
        "node.json",
        "ToS/public-compatibility/state_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
    "ToS/canon/analogy/AGENTS.md": (
        "canonical authored analogy nodes",
        "route-local",
        "node.json",
        "ToS/public-compatibility/analogy_node.example.json",
        "python scripts/validate_tree_node_contracts.py",
    ),
    "ToS/canon/synthesis/AGENTS.md": (
        "canonical authored synthesis nodes",
        "route-local",
        "node.json",
        "ToS/public-compatibility/synthesis_node.example.json",
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
