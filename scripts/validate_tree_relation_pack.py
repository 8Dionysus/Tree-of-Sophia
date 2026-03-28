#!/usr/bin/env python3
from __future__ import annotations

import sys
from collections import Counter
from pathlib import Path

from validate_intake_pack import (
    EXPECTED_PROMOTED_RELATION_EDGE_KIND_COUNTS,
    REPO_ROOT,
    build_canonical_id_map,
    build_entity_class_maps,
    promoted_relation_rows,
    read_csv,
    split_pipe,
)


EXPECTED_HEADERS = [
    "edge_id",
    "edge_kind",
    "from_id",
    "predicate_id",
    "to_id",
    "layer",
    "anchor_mode",
    "anchor_start_secondary",
    "anchor_end_secondary",
    "anchor_segment_ids",
    "witness_scope",
    "connectivity_role",
    "confidence",
    "note",
]
EXPECTED_PREDICATE_HEADERS = [
    "predicate_id",
    "predicate_ru",
    "inverse_predicate_id",
    "allowed_from_classes",
    "allowed_to_classes",
    "status",
    "note",
    "count_in_master",
    "row_kinds",
]
EXPECTED_CLASS_HEADERS = [
    "class_id",
    "family",
    "parent_class",
    "status",
    "note",
    "count_as_from",
    "count_as_to",
    "example_id",
    "source_side",
]

Issue = tuple[str, str]


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    intake_dir = root / "intake" / "thus-spoke-zarathustra" / "prologue-1" / "mode-b"
    predicates_registry_path = root / "tree" / "registries" / "predicates.csv"
    classes_registry_path = root / "tree" / "registries" / "classes.csv"
    relation_pack_path = (
        root
        / "tree"
        / "relations"
        / "friedrich-nietzsche"
        / "thus-spoke-zarathustra"
        / "prologue-1"
        / "edges.csv"
    )
    if not relation_pack_path.is_file():
        return [(relation_pack_path.relative_to(root).as_posix(), "missing canonical relation pack")]

    headers, relation_rows = read_csv(relation_pack_path)
    if headers != EXPECTED_HEADERS:
        issues.append((relation_pack_path.relative_to(root).as_posix(), "header drift from the relation-pack contract"))

    _, node_rows = read_csv(intake_dir / "nodes.csv")
    _, es_rows = read_csv(intake_dir / "event_state_nodes.csv")
    _, principle_rows = read_csv(intake_dir / "principles.csv")
    _, edge_rows = read_csv(intake_dir / "edges.csv")

    canonical_id_map = build_canonical_id_map(node_rows, es_rows, principle_rows)
    _, canonical_entity_classes = build_entity_class_maps(
        node_rows,
        es_rows,
        principle_rows,
        canonical_id_map,
    )
    expected_rows = promoted_relation_rows(edge_rows, canonical_id_map)

    if len(relation_rows) != 89:
        issues.append((relation_pack_path.relative_to(root).as_posix(), "canonical relation pack must contain exactly 89 promoted edges"))

    edge_kind_counts = Counter(row["edge_kind"] for row in relation_rows)
    if edge_kind_counts != EXPECTED_PROMOTED_RELATION_EDGE_KIND_COUNTS:
        issues.append((relation_pack_path.relative_to(root).as_posix(), "edge_kind counts drifted from the expected 57/11/21 split"))

    if relation_rows != expected_rows:
        issues.append((relation_pack_path.relative_to(root).as_posix(), "canonical relation pack drifted from the promoted intake projection"))

    predicate_headers, predicate_rows = read_csv(predicates_registry_path)
    if predicate_headers != EXPECTED_PREDICATE_HEADERS:
        issues.append((predicates_registry_path.relative_to(root).as_posix(), "predicate registry header drift"))
    predicates_by_id = {row["predicate_id"]: row for row in predicate_rows}

    class_headers, class_rows = read_csv(classes_registry_path)
    if class_headers != EXPECTED_CLASS_HEADERS:
        issues.append((classes_registry_path.relative_to(root).as_posix(), "class registry header drift"))
    class_ids = {row["class_id"] for row in class_rows}

    for row in relation_rows:
        edge_id = row["edge_id"]
        if not row["from_id"].startswith("tos.") or not row["to_id"].startswith("tos."):
            issues.append((relation_pack_path.relative_to(root).as_posix(), f"{edge_id} must use only canonical tos.* ids"))

        from_class = canonical_entity_classes.get(row["from_id"])
        to_class = canonical_entity_classes.get(row["to_id"])
        if from_class is None:
            issues.append((relation_pack_path.relative_to(root).as_posix(), f"{edge_id} points from unknown canonical id {row['from_id']}"))
            continue
        if to_class is None:
            issues.append((relation_pack_path.relative_to(root).as_posix(), f"{edge_id} points to unknown canonical id {row['to_id']}"))
            continue
        if from_class not in class_ids:
            issues.append((relation_pack_path.relative_to(root).as_posix(), f"{edge_id} uses unknown from-class {from_class}"))
        if to_class not in class_ids:
            issues.append((relation_pack_path.relative_to(root).as_posix(), f"{edge_id} uses unknown to-class {to_class}"))

        predicate_row = predicates_by_id.get(row["predicate_id"])
        if predicate_row is None:
            issues.append((relation_pack_path.relative_to(root).as_posix(), f"{edge_id} uses unregistered predicate {row['predicate_id']}"))
            continue

        allowed_from = set(split_pipe(predicate_row["allowed_from_classes"]))
        allowed_to = set(split_pipe(predicate_row["allowed_to_classes"]))
        if from_class not in allowed_from:
            issues.append((relation_pack_path.relative_to(root).as_posix(), f"{edge_id} violates allowed_from_classes for predicate {row['predicate_id']}"))
        if to_class not in allowed_to:
            issues.append((relation_pack_path.relative_to(root).as_posix(), f"{edge_id} violates allowed_to_classes for predicate {row['predicate_id']}"))

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Tree relation-pack validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1

    print("[ok] validated route-local canonical relation pack")
    print("[ok] validated canonical relation predicates and endpoint classes against registries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
