#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
from typing import TypeAlias

REPO_ROOT = Path(__file__).resolve().parents[1]
Issue: TypeAlias = tuple[str, str]

REQUIRED_AGENTS: dict[str, tuple[str, ...]] = {
    "ToS/AGENTS.md": (
        "source-home organ for Tree of Sophia",
        "## Operating Card",
        "| input | source witnesses, Zarathustra route surfaces, research packets, philosophy branches",
        "| output | a branch-shaped ToS surface",
        "| next route | witness or research packet -> zarathustra, philosophy, or candidate intake -> canon -> public compatibility -> derived export |",
        "ToS/source_home.manifest.json",
        "ToS/doctrine/KNOWLEDGE_MODEL.md",
        "ToS/doctrine/NODE_CONTRACT.md",
        "source-witnesses/",
        "zarathustra/",
        "research-packets/",
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
    "ToS/zarathustra/AGENTS.md": (
        "golden route branch",
        "## Operating Card",
        "| input | bounded Zarathustra witness route, public-entry capsule, source/canon refs, and tiny-entry orientation |",
        "| next route | witness -> Zarathustra capsule -> canon source node -> bounded concept hop -> compatibility -> derived export |",
        "not a generic canon bucket",
        "not a mechanics package",
        "Source text remains in `ToS/source-witnesses/`",
        "Canonical authored nodes remain in `ToS/canon/`",
        "operational process docs remain in `mechanics/`",
        "python scripts/validate_tiny_entry_route.py",
        "python scripts/validate_kag_export.py",
    ),
    "ToS/philosophy/AGENTS.md": (
        "growing domain branch for philosophy",
        "## Operating Card",
        "| input | era, region, tradition, work, figure, concept, source-corpus, transmission, research packet, and branch-graph material |",
        "| next route | source witness or research packet -> local domain branch -> source anchoring -> graph workbench -> review -> `ToS/canon/` promotion when ready |",
        "trunk law",
        "era branches",
        "region branches",
        "tradition subtrees",
        "works, figures, concepts, transmissions, sources, and local graph workbenches",
        "`ToS/philosophy/` owns authored domain growth",
        "`ToS/candidate-intake/` owns provisional extraction",
        "Name branches by what they are in the philosophical tree",
        "trunk -> eras -> regions -> traditions",
        "Capture page labels and workbench nicknames stay in metadata",
        "Repository paths describe the philosophical branch they belong to",
        "`ToS/research-packets/deep-research/philosophy/`",
        "not a source witness",
        "python scripts/validate_philosophy_topology.py",
    ),
    "ToS/research-packets/AGENTS.md": (
        "non-authoritative research packets",
        "## Operating Card",
        "| input | AI-generated research scaffold, secondary synthesis, capture metadata, or unpacking notes |",
        "| output | bounded packet metadata and reviewable leads for `ToS/philosophy/` |",
        "Do not call these packets source witnesses",
        "Do not cite AI-generated packet material as author",
        "Keep capture containers and UI titles as metadata only",
        "python scripts/validate_philosophy_topology.py",
    ),
    "ToS/research-packets/deep-research/philosophy/AGENTS.md": (
        "Deep Research packet metadata for `ToS/philosophy/`",
        "## Operating Card",
        "| input | AI-generated research scaffold metadata, capture page identity, titles, and child-page pointers |",
        "| output | research packet metadata that can feed branch review without becoming source, canon, or doctrine |",
        "Never cite this packet as a source witness",
        "Route branch-shaped philosophical material into `ToS/philosophy/`",
        "Route claims that need authority to real source witnesses",
        "python scripts/validate_philosophy_topology.py",
    ),
    "ToS/doctrine/AGENTS.md": (
        "ToS knowledge law",
        "Durable rationale lives in `docs/decisions/`",
        "ToS/doctrine/KNOWLEDGE_MODEL.md",
        "ToS/doctrine/NODE_CONTRACT.md",
        "mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md",
    ),
    "ToS/review-ledger/AGENTS.md": (
        "dated review notes",
        "do not overrule current doctrine",
        "Route durable rationale to `docs/decisions/`",
        "python scripts/validate_tos_source_home.py",
    ),
    "docs/AGENTS.md": (
        "repository-level documentation surfaces",
        "## Boundary Routes",
        "docs/decisions/",
        "docs/RELEASING.md",
        "docs/AGENTS_ROOT_REFERENCE.md",
        "ToS knowledge law",
        "raw witness in `ToS/source-witnesses/`",
        "golden Zarathustra route surfaces in `ToS/zarathustra/`",
        "non-authoritative research packets in `ToS/research-packets/`",
        "domain philosophy topology in `ToS/philosophy/`",
        "candidate structure in `ToS/candidate-intake/`",
        "canonical authored canon in `ToS/canon/`",
        "compatibility mirrors in `ToS/public-compatibility/`",
        "derived exports in `ToS/derived-exports/`",
        "operation mechanics in `mechanics/`",
        "python scripts/validate_tos_source_home.py",
    ),
    "docs/decisions/AGENTS.md": (
        "durable ToS decision rationale",
        "## Boundary Routes",
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
        "## Operating Card",
        "| input | reviewed canon object, contract change, public-safe route example, or bounded compatibility artifact |",
        "| output | example payload aligned with canon, contract, and export validators |",
        "the authored node contract",
        "the current tiny-entry route",
        "ToS/public-compatibility/review/",
        "one shared `node_id`",
        "Keep authored nodes distinct from operational artifacts and generated companions",
        "mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md",
    ),
    "ToS/public-compatibility/review/AGENTS.md": (
        "review/archive material",
        "superseded",
        "Active canon stays under `ToS/canon/`",
        "ToS/public-compatibility/review/calibration-family-pilot/",
        "mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md",
    ),
    "ToS/derived-exports/AGENTS.md": (
        "derived export artifacts under `ToS/derived-exports/`",
        "## Operating Card",
        "| input | owned canon, compatibility examples, contracts, and generator logic |",
        "| output | generated export payloads and compact read models |",
        "Change the source-owned inputs or generation logic, then regenerate",
        "tos.source.thus-spoke-zarathustra.prologue",
        "`entry_surface`",
        "`section_handles`",
        "`non_identity_boundary`",
        "python scripts/generate_kag_export.py",
        "python scripts/validate_kag_export.py",
    ),
    "ToS/contracts/AGENTS.md": (
        "## Operating Card",
        "tos-node-contract.schema.json",
        "tos-tiny-entry-route.schema.json",
        "public contract change",
        "one shared authored identity",
        "non-source operational contracts to their owning surfaces",
        "mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md",
    ),
    "scripts/AGENTS.md": (
        "## Operating Card",
        "| input | schema, example, intake, canon, decision, topology, or export surface |",
        "| output | generated payload, generated index, or pass/fail validation signal |",
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
        "## Operating Card",
        "| input | primary-language text, bridge translation, donor markdown, source-page metadata, and provenance notes |",
        "| output | reviewable witness surface with explicit source posture |",
        "witnesses authority and routes later interpretation",
        "candidate structure in `ToS/candidate-intake/`",
        "canonical authored nodes in `ToS/canon/`",
        "compatibility mirrors in `ToS/public-compatibility/`",
        "keep witness uncertainty visible",
        "mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md",
    ),
    "ToS/candidate-intake/AGENTS.md": (
        "candidate intake material",
        "## Operating Card",
        "| input | source-linked extraction pass, candidate node table, relation table, normalization note, or promotion residue |",
        "| output | reviewable candidate pack with source pointer, pass frame, blocker state, and uncertainty |",
        "staging ledge for material that still needs review",
        "primary witness authority in `ToS/source-witnesses/`",
        "canonical tree law in `ToS/canon/`",
        "public route examples in `ToS/public-compatibility/`",
        "deterministic tooling in `scripts/`",
        "the source witness",
        "the pass or extraction frame",
        "the uncertainty that still remains",
        "Quest or progression language may appear here only as bounded work-tracking or compatibility support",
        "mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md",
    ),
    "ToS/canon/AGENTS.md": (
        "canonical authored tree",
        "## Operating Card",
        "| input | reviewed source-grounded authored object, canonical relation pack, or vocabulary registry change |",
        "| output | canonical `node.json`, `edges.csv`, or registry surface with stable identity |",
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

BANNED_ROUTE_SECTION_MARKERS = (
    "## Stop Lines",
    "## Hard no",
)


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
        for marker in BANNED_ROUTE_SECTION_MARKERS:
            if marker in raw_text:
                issues.append((relative_path, f"use Operating Card/Boundary Routes instead of {marker}"))

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
