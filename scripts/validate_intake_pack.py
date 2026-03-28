#!/usr/bin/env python3
from __future__ import annotations

import csv
import json
import sys
from collections import Counter
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
INTAKE_DIR = REPO_ROOT / "intake" / "thus-spoke-zarathustra" / "prologue-1" / "mode-b"
TREE_SOURCE_NODE_PATH = (
    REPO_ROOT
    / "tree"
    / "source"
    / "friedrich-nietzsche"
    / "thus-spoke-zarathustra"
    / "prologue-1"
    / "node.json"
)
REGISTRY_DIR = REPO_ROOT / "tree" / "registries"
PREDICATES_REGISTRY_PATH = REGISTRY_DIR / "predicates.csv"
CLASSES_REGISTRY_PATH = REGISTRY_DIR / "classes.csv"

EXPECTED_FILES = (
    "README.md",
    "corpus_map.csv",
    "witnesses.csv",
    "segments.csv",
    "nodes.csv",
    "event_state_nodes.csv",
    "edges.csv",
    "translation_tensions.csv",
    "witness_glosses.csv",
    "principles.csv",
)

EXPECTED_HEADERS = {
    "corpus_map.csv": [
        "corpus_row_id",
        "work_id",
        "part_no",
        "chapter_no",
        "subchapter_no",
        "paragraph_no",
        "source_secondary",
        "sort_key",
        "title_ru",
        "title_en",
        "note",
    ],
    "witnesses.csv": [
        "witness_id",
        "language",
        "witness_role",
        "authority_level",
        "author_or_translator",
        "edition_or_source",
        "publication_year",
        "based_on",
        "normalization_note",
        "active",
    ],
    "segments.csv": [
        "segment_id",
        "source_secondary",
        "paragraph_anchor",
        "sort_key",
        "witness_scope",
        "line_span",
        "cluster_id",
        "working_name",
        "note",
    ],
    "nodes.csv": [
        "node_id",
        "label_ru",
        "node_class",
        "layer",
        "first_segment_id",
        "first_source_secondary",
        "canonical_label",
        "label_en",
        "status",
        "note",
    ],
    "event_state_nodes.csv": [
        "es_id",
        "kind",
        "label_ru",
        "anchor_mode",
        "anchor_start_secondary",
        "anchor_end_secondary",
        "anchor_segment_ids",
        "subject_hint",
        "es_class",
        "repeatable",
        "status",
        "note",
    ],
    "edges.csv": [
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
        "status",
    ],
    "translation_tensions.csv": [
        "tension_id",
        "normalized_core",
        "anchor_mode",
        "anchor_start_secondary",
        "anchor_end_secondary",
        "anchor_segment_ids",
        "witness_ids",
        "why_load_bearing",
        "decision_status",
        "preferred_handling",
        "note",
    ],
    "witness_glosses.csv": [
        "gloss_id",
        "witness_id",
        "segment_id",
        "source_secondary",
        "token_or_phrase",
        "normalized_core",
        "tension_id",
        "gloss_note",
        "status",
    ],
    "principles.csv": [
        "principle_id",
        "layer",
        "formula_ru",
        "anchor_mode",
        "anchor_start_secondary",
        "anchor_end_secondary",
        "anchor_segment_ids",
        "status",
        "note",
    ],
}

EXPECTED_WITNESS_IDS = [
    "w.de.nietzsche.canonical_source.v1",
    "w.ru.dionysus.working_translation.v1",
    "w.en.dionysus.bridge_translation.v1",
]

EXPECTED_SEGMENT_IDS = [f"seg.1.1.1.{index}" for index in range(1, 13)]
EXPECTED_EDGE_KIND_COUNTS = Counter(
    {
        "source_edge": 95,
        "bridge_edge": 11,
        "principle_edge": 22,
    }
)
EXPECTED_PROMOTED_RELATION_EDGE_KIND_COUNTS = Counter(
    {
        "source_edge": 90,
        "bridge_edge": 11,
        "principle_edge": 21,
    }
)
EXPECTED_EDGE_LEDGER_STATUS_COUNTS = Counter(
    {
        "promoted": 122,
        "deferred_literal": 3,
        "deferred_analogy": 2,
        "deferred_commentary": 1,
    }
)
PROMOTED_PRINCIPLE_IDS = {
    "pr.solitude_as_ripening",
    "pr.wisdom_can_overfill",
    "pr.happiness_is_relational",
    "pr.blessing_is_reciprocal",
    "pr.excess_seeks_recipients",
    "pr.overflow_can_be_received",
    "pr.descent_is_required_by_gift",
    "pr.gift_has_dual_mode",
    "pr.go_under_is_human_name",
    "pr.reflected_light_can_be_carried",
    "pr.tranquil_vision_without_envy",
    "pr.return_as_action",
    "pr.beginning_through_going_under",
}
DEFERRED_COMMENTARY_PRINCIPLE_ID = "pr.departure_from_reflective_origin"
DEFERRED_ANALOGY_EVENT_STATE_ID = "ev.p5.bee_honey_analogy"
DEFERRED_LITERAL_NODE_IDS = {"literal.ten_years", "literal.too_much"}

Issue = tuple[str, str]


def read_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def read_csv(path: Path) -> tuple[list[str], list[dict[str, str]]]:
    with path.open("r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.DictReader(handle)
        headers = reader.fieldnames or []
        rows = [{key: (value or "") for key, value in row.items()} for row in reader]
    return headers, rows


def split_pipe(value: str) -> list[str]:
    return [item.strip() for item in value.split("|") if item.strip()]


def parse_bool(value: str) -> bool | None:
    lowered = value.strip().lower()
    if lowered in {"true", "false"}:
        return lowered == "true"
    return None


def build_canonical_id_map(
    node_rows: list[dict[str, str]],
    es_rows: list[dict[str, str]],
    principle_rows: list[dict[str, str]],
) -> dict[str, str]:
    mapping: dict[str, str] = {}

    for row in node_rows:
        if row["status"] != "promoted":
            continue
        slug = row["canonical_label"].replace("_", "-")
        mapping[row["node_id"]] = f"tos.support.thus-spoke-zarathustra.prologue.{slug}"

    for row in es_rows:
        if row["status"] != "promoted":
            continue
        slug = row["es_id"].split(".", 2)[2].replace("_", "-")
        mapping[row["es_id"]] = f"tos.{row['kind']}.thus-spoke-zarathustra.prologue.{slug}"

    for row in principle_rows:
        if row["status"] != "promoted":
            continue
        slug = row["principle_id"].split(".", 1)[1].replace("_", "-")
        mapping[row["principle_id"]] = f"tos.principle.thus-spoke-zarathustra.prologue.{slug}"

    return mapping


def build_entity_class_maps(
    node_rows: list[dict[str, str]],
    es_rows: list[dict[str, str]],
    principle_rows: list[dict[str, str]],
    canonical_id_map: dict[str, str],
) -> tuple[dict[str, str], dict[str, str]]:
    raw_entity_classes: dict[str, str] = {}
    for row in node_rows:
        raw_entity_classes[row["node_id"]] = row["node_class"]
    for row in es_rows:
        raw_entity_classes[row["es_id"]] = row["kind"]
    for row in principle_rows:
        raw_entity_classes[row["principle_id"]] = "principle"

    canonical_entity_classes = {
        canonical_id_map[raw_id]: class_id
        for raw_id, class_id in raw_entity_classes.items()
        if raw_id in canonical_id_map
    }
    return raw_entity_classes, canonical_entity_classes


def classify_edge_status(row: dict[str, str], canonical_raw_ids: set[str]) -> str:
    raw_ids = {row["from_id"], row["to_id"]}
    if raw_ids <= canonical_raw_ids:
        return "promoted"
    if DEFERRED_COMMENTARY_PRINCIPLE_ID in raw_ids:
        return "deferred_commentary"
    if DEFERRED_ANALOGY_EVENT_STATE_ID in raw_ids:
        return "deferred_analogy"
    if raw_ids & DEFERRED_LITERAL_NODE_IDS:
        return "deferred_literal"
    return "invalid_residue"


def promoted_relation_rows(
    edge_rows: list[dict[str, str]],
    canonical_id_map: dict[str, str],
) -> list[dict[str, str]]:
    canonical_raw_ids = set(canonical_id_map)
    promoted_rows: list[dict[str, str]] = []
    for row in edge_rows:
        if classify_edge_status(row, canonical_raw_ids) != "promoted":
            continue
        promoted_row = {key: value for key, value in row.items() if key != "status"}
        promoted_row["from_id"] = canonical_id_map[row["from_id"]]
        promoted_row["to_id"] = canonical_id_map[row["to_id"]]
        promoted_rows.append(promoted_row)
    return promoted_rows


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    actual_files = {path.name for path in INTAKE_DIR.iterdir() if path.is_file()}
    if actual_files != set(EXPECTED_FILES):
        issues.append(
            (
                INTAKE_DIR.relative_to(root).as_posix(),
                "intake pack file set does not match the 9-table contract",
            )
        )

    tables: dict[str, list[dict[str, str]]] = {}
    for filename, expected_headers in EXPECTED_HEADERS.items():
        path = INTAKE_DIR / filename
        if not path.is_file():
            issues.append((path.relative_to(root).as_posix(), "missing required CSV"))
            continue
        headers, rows = read_csv(path)
        if headers != expected_headers:
            issues.append(
                (
                    path.relative_to(root).as_posix(),
                    "header drift from the current tabular base contract",
                )
            )
        tables[filename] = rows

    source_payload = read_json(TREE_SOURCE_NODE_PATH)
    if not isinstance(source_payload, dict):
        issues.append(("tree source node", "canonical source node must remain a JSON object"))
        source_payload = {}

    corpus_rows = tables.get("corpus_map.csv", [])
    witness_rows = tables.get("witnesses.csv", [])
    segment_rows = tables.get("segments.csv", [])
    node_rows = tables.get("nodes.csv", [])
    es_rows = tables.get("event_state_nodes.csv", [])
    edge_rows = tables.get("edges.csv", [])
    tension_rows = tables.get("translation_tensions.csv", [])
    gloss_rows = tables.get("witness_glosses.csv", [])
    principle_rows = tables.get("principles.csv", [])

    source_secondary_values = [f"1,1,1,{index}" for index in range(1, 13)]
    source_secondary_set = set(source_secondary_values)

    if len(corpus_rows) != 12:
        issues.append(("corpus_map.csv", "expected 12 bounded corpus rows"))
    actual_sort_keys = [row["sort_key"] for row in corpus_rows]
    if actual_sort_keys != [str(index) for index in range(1, 13)]:
        issues.append(("corpus_map.csv", "sort_key must run strictly from 1 to 12"))
    for row in corpus_rows:
        if row["source_secondary"] not in source_secondary_set:
            issues.append(("corpus_map.csv", f"unknown source_secondary {row['source_secondary']}"))

    if len(witness_rows) != 3:
        issues.append(("witnesses.csv", "expected exactly 3 witness rows"))
    actual_witness_ids = [row["witness_id"] for row in witness_rows]
    if actual_witness_ids != EXPECTED_WITNESS_IDS:
        issues.append(("witnesses.csv", "witness ids drifted from the current ToS vocabulary"))
    for row in witness_rows:
        if row["witness_role"] not in {"canonical_source", "working_translation", "bridge_translation"}:
            issues.append(("witnesses.csv", f"unknown witness_role {row['witness_role']}"))
        if parse_bool(row["active"]) is None:
            issues.append(("witnesses.csv", f"invalid active value {row['active']}"))

    if len(segment_rows) != 12:
        issues.append(("segments.csv", "expected exactly 12 segment rows"))
    actual_segment_ids = [row["segment_id"] for row in segment_rows]
    if actual_segment_ids != EXPECTED_SEGMENT_IDS:
        issues.append(("segments.csv", "segment spine must be seg.1.1.1.1 through seg.1.1.1.12"))
    segment_set = set(actual_segment_ids)
    paragraph_anchors = [row["paragraph_anchor"] for row in segment_rows]
    if paragraph_anchors != [f"[{index}]" for index in range(1, 13)]:
        issues.append(("segments.csv", "paragraph_anchor must stay aligned to [1]...[12]"))
    for index, row in enumerate(segment_rows, start=1):
        if row["source_secondary"] != f"1,1,1,{index}":
            issues.append(("segments.csv", f"segment {row['segment_id']} is out of source_secondary order"))

    if len(edge_rows) != 128:
        issues.append(("edges.csv", "expected 128 rows from the current full master projection"))
    edge_kind_counts = Counter(row["edge_kind"] for row in edge_rows)
    if edge_kind_counts != EXPECTED_EDGE_KIND_COUNTS:
        issues.append(("edges.csv", "edge_kind counts drifted from the expected 95/11/22 split"))

    node_ids = {row["node_id"] for row in node_rows}
    es_ids = {row["es_id"] for row in es_rows}
    principle_ids = {row["principle_id"] for row in principle_rows}
    graph_ids = node_ids | es_ids | principle_ids
    canonical_id_map = build_canonical_id_map(node_rows, es_rows, principle_rows)
    canonical_raw_ids = set(canonical_id_map)

    if "literal.ten_years" not in node_ids or "literal.too_much" not in node_ids:
        issues.append(("nodes.csv", "literal.ten_years and literal.too_much must remain explicit node rows"))

    for row in node_rows:
        if row["first_segment_id"] not in segment_set:
            issues.append(("nodes.csv", f"unknown first_segment_id {row['first_segment_id']}"))
        if row["first_source_secondary"] not in source_secondary_set:
            issues.append(("nodes.csv", f"unknown first_source_secondary {row['first_source_secondary']}"))
        if row["node_id"] in DEFERRED_LITERAL_NODE_IDS:
            if row["status"] != "deferred_literal":
                issues.append(("nodes.csv", f"{row['node_id']} must be marked deferred_literal"))
        else:
            if row["status"] != "promoted":
                issues.append(("nodes.csv", f"{row['node_id']} must be marked promoted"))
    if len(node_rows) != 48:
        issues.append(("nodes.csv", "expected 48 support-ledger rows for the current bounded route"))
    node_status_counts = Counter(row["status"] for row in node_rows)
    expected_node_status_counts = Counter(
        {
            "promoted": 46,
            "deferred_literal": 2,
        }
    )
    if node_status_counts != expected_node_status_counts:
        issues.append(("nodes.csv", "support-node status split drifted from the expected 46/2 ledger"))

    def validate_anchor_row(
        *,
        table_name: str,
        row_id: str,
        anchor_mode: str,
        anchor_start_secondary: str,
        anchor_end_secondary: str,
        anchor_segment_ids: str,
    ) -> None:
        if anchor_mode not in {"single", "multi"}:
            issues.append((table_name, f"{row_id} has invalid anchor_mode {anchor_mode}"))
        if anchor_start_secondary not in source_secondary_set:
            issues.append((table_name, f"{row_id} has invalid anchor_start_secondary {anchor_start_secondary}"))
        if anchor_end_secondary and anchor_end_secondary not in source_secondary_set:
            issues.append((table_name, f"{row_id} has invalid anchor_end_secondary {anchor_end_secondary}"))
        if anchor_mode == "single" and anchor_end_secondary:
            issues.append((table_name, f"{row_id} should keep anchor_end_secondary empty for single anchors"))
        if anchor_mode == "multi" and not anchor_end_secondary:
            issues.append((table_name, f"{row_id} needs anchor_end_secondary for multi anchors"))
        segment_ids = split_pipe(anchor_segment_ids)
        if not segment_ids:
            issues.append((table_name, f"{row_id} must keep at least one anchor_segment_id"))
        for segment_id in segment_ids:
            if segment_id not in segment_set:
                issues.append((table_name, f"{row_id} points at unknown anchor segment {segment_id}"))

    for row in es_rows:
        validate_anchor_row(
            table_name="event_state_nodes.csv",
            row_id=row["es_id"],
            anchor_mode=row["anchor_mode"],
            anchor_start_secondary=row["anchor_start_secondary"],
            anchor_end_secondary=row["anchor_end_secondary"],
            anchor_segment_ids=row["anchor_segment_ids"],
        )
        if parse_bool(row["repeatable"]) is None:
            issues.append(("event_state_nodes.csv", f"{row['es_id']} has invalid repeatable value {row['repeatable']}"))
        if row["kind"] == "event":
            if row["status"] != "promoted":
                issues.append(("event_state_nodes.csv", f"{row['es_id']} must be marked promoted"))
        elif row["kind"] == "state":
            if row["status"] != "promoted":
                issues.append(("event_state_nodes.csv", f"{row['es_id']} must be marked promoted"))
        elif row["kind"] == "analogy":
            if row["es_id"] != DEFERRED_ANALOGY_EVENT_STATE_ID:
                issues.append(("event_state_nodes.csv", f"unexpected analogy row {row['es_id']}"))
            if row["status"] != "deferred_analogy":
                issues.append(("event_state_nodes.csv", f"{row['es_id']} must be marked deferred_analogy"))
        else:
            issues.append(("event_state_nodes.csv", f"unexpected kind {row['kind']}"))

    if len(es_rows) != 28:
        issues.append(("event_state_nodes.csv", "expected 28 event/state ledger rows for the current bounded route"))
    es_status_counts = Counter((row["kind"], row["status"]) for row in es_rows)
    expected_es_status_counts = Counter(
        {
            ("event", "promoted"): 18,
            ("state", "promoted"): 9,
            ("analogy", "deferred_analogy"): 1,
        }
    )
    if es_status_counts != expected_es_status_counts:
        issues.append(("event_state_nodes.csv", "event/state status split drifted from the expected 18/9/1 ledger"))

    for row in principle_rows:
        validate_anchor_row(
            table_name="principles.csv",
            row_id=row["principle_id"],
            anchor_mode=row["anchor_mode"],
            anchor_start_secondary=row["anchor_start_secondary"],
            anchor_end_secondary=row["anchor_end_secondary"],
            anchor_segment_ids=row["anchor_segment_ids"],
        )
        if row["principle_id"] in PROMOTED_PRINCIPLE_IDS:
            if row["status"] != "promoted":
                issues.append(("principles.csv", f"{row['principle_id']} must be marked promoted"))
        elif row["principle_id"] == DEFERRED_COMMENTARY_PRINCIPLE_ID:
            if row["status"] != "deferred_commentary":
                issues.append(("principles.csv", f"{row['principle_id']} must be marked deferred_commentary"))
        else:
            issues.append(("principles.csv", f"unexpected principle id {row['principle_id']}"))
    if len(principle_rows) != 14:
        issues.append(("principles.csv", "expected 14 principle rows for the current bounded route"))
    actual_principle_ids = {row["principle_id"] for row in principle_rows}
    expected_principle_ids = PROMOTED_PRINCIPLE_IDS | {DEFERRED_COMMENTARY_PRINCIPLE_ID}
    if actual_principle_ids != expected_principle_ids:
        issues.append(("principles.csv", "principle id set drifted from the current bounded route"))

    for row in edge_rows:
        if row["from_id"] not in graph_ids:
            issues.append(("edges.csv", f"{row['edge_id']} points from unknown id {row['from_id']}"))
        if row["to_id"] not in graph_ids:
            issues.append(("edges.csv", f"{row['edge_id']} points to unknown id {row['to_id']}"))
        validate_anchor_row(
            table_name="edges.csv",
            row_id=row["edge_id"],
            anchor_mode=row["anchor_mode"],
            anchor_start_secondary=row["anchor_start_secondary"],
            anchor_end_secondary=row["anchor_end_secondary"],
            anchor_segment_ids=row["anchor_segment_ids"],
        )
        expected_status = classify_edge_status(row, canonical_raw_ids)
        if row["status"] != expected_status:
            issues.append(("edges.csv", f"{row['edge_id']} must be marked {expected_status}"))

    edge_status_counts = Counter(row["status"] for row in edge_rows)
    if edge_status_counts != EXPECTED_EDGE_LEDGER_STATUS_COUNTS:
        issues.append(("edges.csv", "edge status split drifted from the expected 122/3/2/1 ledger"))

    promoted_edge_rows = promoted_relation_rows(edge_rows, canonical_id_map)
    promoted_edge_kind_counts = Counter(row["edge_kind"] for row in promoted_edge_rows)
    if promoted_edge_kind_counts != EXPECTED_PROMOTED_RELATION_EDGE_KIND_COUNTS:
        issues.append(("edges.csv", "promoted relation subset drifted from the expected 90/11/21 canonical split"))

    source_tensions = source_payload.get("translation_tensions", [])
    if not isinstance(source_tensions, list):
        source_tensions = []
    if len(tension_rows) != len(source_tensions):
        issues.append(
            (
                "translation_tensions.csv",
                "row count must stay aligned with the compact source-node translation_tensions surface",
            )
        )
    tension_ids = {row["tension_id"] for row in tension_rows}
    for row in tension_rows:
        validate_anchor_row(
            table_name="translation_tensions.csv",
            row_id=row["tension_id"],
            anchor_mode=row["anchor_mode"],
            anchor_start_secondary=row["anchor_start_secondary"],
            anchor_end_secondary=row["anchor_end_secondary"],
            anchor_segment_ids=row["anchor_segment_ids"],
        )
        for witness_id in split_pipe(row["witness_ids"]):
            if witness_id not in EXPECTED_WITNESS_IDS:
                issues.append(("translation_tensions.csv", f"{row['tension_id']} points at unknown witness {witness_id}"))

    expected_gloss_rows = len(tension_rows) * 3
    if len(gloss_rows) != expected_gloss_rows:
        issues.append(("witness_glosses.csv", "expected exactly 3 gloss rows per translation tension"))
    for row in gloss_rows:
        if row["witness_id"] not in EXPECTED_WITNESS_IDS:
            issues.append(("witness_glosses.csv", f"{row['gloss_id']} points at unknown witness {row['witness_id']}"))
        if row["segment_id"] not in segment_set:
            issues.append(("witness_glosses.csv", f"{row['gloss_id']} points at unknown segment {row['segment_id']}"))
        if row["source_secondary"] not in source_secondary_set:
            issues.append(("witness_glosses.csv", f"{row['gloss_id']} points at unknown source_secondary {row['source_secondary']}"))
        if row["tension_id"] and row["tension_id"] not in tension_ids:
            issues.append(("witness_glosses.csv", f"{row['gloss_id']} points at unknown tension_id {row['tension_id']}"))

    covered_secondaries: Counter[str] = Counter()
    for row in edge_rows:
        start = int(row["anchor_start_secondary"].split(",")[-1])
        end = int((row["anchor_end_secondary"] or row["anchor_start_secondary"]).split(",")[-1])
        for index in range(start, end + 1):
            covered_secondaries[f"1,1,1,{index}"] += 1
    missing_coverage = [value for value in source_secondary_values if covered_secondaries[value] == 0]
    if missing_coverage:
        issues.append(("edges.csv", f"edge coverage is missing paragraphs: {', '.join(missing_coverage)}"))

    predicate_headers, predicate_rows = read_csv(PREDICATES_REGISTRY_PATH)
    if predicate_headers != [
        "predicate_id",
        "predicate_ru",
        "inverse_predicate_id",
        "allowed_from_classes",
        "allowed_to_classes",
        "status",
        "note",
        "count_in_master",
        "row_kinds",
    ]:
        issues.append(("tree/registries/predicates.csv", "predicate registry header drift"))

    class_headers, class_rows = read_csv(CLASSES_REGISTRY_PATH)
    if class_headers != [
        "class_id",
        "family",
        "parent_class",
        "status",
        "note",
        "count_as_from",
        "count_as_to",
        "example_id",
        "source_side",
    ]:
        issues.append(("tree/registries/classes.csv", "class registry header drift"))

    predicate_edge_counts = Counter(row["predicate_id"] for row in edge_rows)
    predicate_row_kinds: dict[str, str] = {}
    for predicate_id in predicate_edge_counts:
        row_kinds = sorted({row["edge_kind"] for row in edge_rows if row["predicate_id"] == predicate_id})
        predicate_row_kinds[predicate_id] = "|".join(row_kinds)
    for row in predicate_rows:
        count = predicate_edge_counts.get(row["predicate_id"], 0)
        if row["count_in_master"] != str(count):
            issues.append(("tree/registries/predicates.csv", f"count drift for predicate {row['predicate_id']}"))
        if row["row_kinds"] != predicate_row_kinds.get(row["predicate_id"], ""):
            issues.append(("tree/registries/predicates.csv", f"row_kinds drift for predicate {row['predicate_id']}"))

    entity_classes: dict[str, str] = {}
    for row in node_rows:
        entity_classes[row["node_id"]] = row["node_class"]
    for row in es_rows:
        entity_classes[row["es_id"]] = row["kind"]
    for row in principle_rows:
        entity_classes[row["principle_id"]] = "principle"

    count_as_from = Counter()
    count_as_to = Counter()
    for row in edge_rows:
        if row["from_id"] in entity_classes:
            count_as_from[entity_classes[row["from_id"]]] += 1
        if row["to_id"] in entity_classes:
            count_as_to[entity_classes[row["to_id"]]] += 1

    def compute_source_side(from_count: int, to_count: int) -> str:
        if from_count > 0 and to_count > 0:
            return "from+to"
        if from_count > 0:
            return "from_only"
        if to_count > 0:
            return "to_only"
        return "unused"

    for row in class_rows:
        from_count = count_as_from.get(row["class_id"], 0)
        to_count = count_as_to.get(row["class_id"], 0)
        if row["count_as_from"] != str(from_count):
            issues.append(("tree/registries/classes.csv", f"count_as_from drift for class {row['class_id']}"))
        if row["count_as_to"] != str(to_count):
            issues.append(("tree/registries/classes.csv", f"count_as_to drift for class {row['class_id']}"))
        if row["source_side"] != compute_source_side(from_count, to_count):
            issues.append(("tree/registries/classes.csv", f"source_side drift for class {row['class_id']}"))

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Intake pack validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1

    print("[ok] validated v6.1 tabular base intake pack")
    print("[ok] validated tabular registries against the current edges.csv")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
