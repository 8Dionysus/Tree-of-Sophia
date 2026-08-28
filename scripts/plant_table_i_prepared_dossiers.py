#!/usr/bin/env python3
"""Plant the prepared Table I dossier corpus into the ToS philosophy tree.

This script reads the operator-local DOCX corpus and writes only structured
ToS surfaces: atlas indexes, branch bodies, source-anchor backlogs, and
pre-canon graph workbench rows.
"""

from __future__ import annotations

import hashlib
import json
import re
import shutil
import zipfile
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from xml.etree import ElementTree

from philosophy_multilingual_common import english_label, russian_label

REPO_ROOT = Path(__file__).resolve().parents[1]
DOC_ROOT = Path("/home/dionysus/Загрузки/ToS")
TABLE_I_ROWS = REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-i/rows.jsonl"
TABLE_I_MANIFEST = REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-i/table.manifest.json"
ATLAS_MANIFEST = REPO_ROOT / "ToS/philosophy/atlas/atlas.manifest.json"
PHILOSOPHY_MANIFEST = REPO_ROOT / "ToS/philosophy/philosophy.manifest.json"
DOSSIER_INDEX = REPO_ROOT / "ToS/philosophy/atlas/dossiers/index.jsonl"
DOSSIER_SUMMARY = REPO_ROOT / "ToS/philosophy/atlas/dossiers/graph-shape-summary.json"
DOSSIER_BRANCH_MANIFEST = REPO_ROOT / "ToS/philosophy/atlas/dossiers/branch.manifest.json"
PREPARED_DOSSIER_ROUTES = REPO_ROOT / "ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json"
INTAKE_MANIFEST = REPO_ROOT / "ToS/research-packets/deep-research/philosophy/dossiers/table-i-docx-intake.manifest.json"
EXTRACTION_COVERAGE = REPO_ROOT / "ToS/research-packets/deep-research/philosophy/dossiers/table-i-docx-extraction-coverage.json"
SOURCE_ANCHOR_BACKLOG = REPO_ROOT / "ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl"
TERM_INDEX = REPO_ROOT / "ToS/philosophy/atlas/dossiers/term-index.jsonl"
TRANSMISSION_BACKLOG = REPO_ROOT / "ToS/philosophy/atlas/dossiers/transmission-backlog.jsonl"
PROPOSED_NODES = REPO_ROOT / "ToS/philosophy/graph-workbench/proposed-nodes/table-i-prepared-dossiers.jsonl"
PROPOSED_RELATIONS = REPO_ROOT / "ToS/philosophy/graph-workbench/proposed-relations/table-i-prepared-dossiers.jsonl"
LANGUAGE_PACKETS = REPO_ROOT / "ToS/philosophy/graph-workbench/language-packets/table-i-text-bearing-nodes.jsonl"
LANGUAGE_PACKETS_MANIFEST = REPO_ROOT / "ToS/philosophy/graph-workbench/language-packets/branch.manifest.json"
LANGUAGE_PACKETS_README = REPO_ROOT / "ToS/philosophy/graph-workbench/language-packets/README.md"
LANGUAGE_PACKET_CONTRACT_REF = "ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json"
LANGUAGE_REGISTRY_REF = "ToS/philosophy/atlas/multilingual/language-registry.json"
BRANCH_FRAGMENTS = REPO_ROOT / "ToS/philosophy/graph-workbench/branch-fragments/table-i-prepared-dossier-branches.json"
PROMOTION_LEDGER = REPO_ROOT / "ToS/philosophy/graph-workbench/promotion-ledger/table-i-prepared-dossiers.md"
OBSOLETE_GENERATED_BRANCH = REPO_ROOT / "ToS/philosophy/eras/bronze-age/regions/ancient-near-east"
TEXT_BEARING_NODE_KINDS = {"text_corpus"}
INTAKE_RECORDED_ON = "2026-08-27"


def load_table_i_package() -> dict[str, Any]:
    payload = json.loads(PREPARED_DOSSIER_ROUTES.read_text(encoding="utf-8"))
    packages = payload.get("packages")
    if not isinstance(packages, dict):
        raise ValueError("prepared-dossier-routes.json must expose packages")
    table_i = packages.get("table-i")
    if not isinstance(table_i, dict):
        raise ValueError("prepared-dossier-routes.json must expose a table-i package")
    return table_i


TABLE_I_PACKAGE = load_table_i_package()


def load_table_i_routes() -> dict[str, dict[str, Any]]:
    table_i = TABLE_I_PACKAGE
    routes = table_i.get("routes")
    if not isinstance(routes, list):
        raise ValueError("prepared-dossier-routes.json table-i.routes must be a list")
    defaults = table_i.get("route_defaults")
    if not isinstance(defaults, dict):
        raise ValueError("prepared-dossier-routes.json table-i.route_defaults must be an object")
    prepared_routes: dict[str, dict[str, Any]] = {}
    for route in routes:
        if not isinstance(route, dict):
            raise ValueError("prepared dossier routes must be objects")
        merged = {**defaults, **route}
        dossier_id = str(merged.get("dossier_id") or "")
        branch_path = str(merged.get("branch_path") or "")
        branch_role = str(merged.get("branch_role") or "")
        if not dossier_id or not branch_path or not branch_role:
            raise ValueError("prepared dossier routes must carry dossier_id, branch_path, and branch_role")
        if dossier_id in prepared_routes:
            raise ValueError(f"duplicate prepared dossier route: {dossier_id}")
        review_posture = str(merged.get("review_posture") or "")
        route_kind = str(merged.get("route_kind") or "")
        if not review_posture or not route_kind:
            raise ValueError("prepared dossier routes must resolve review_posture and route_kind")
        prepared_routes[dossier_id] = merged
    return prepared_routes


ROUTES = load_table_i_routes()
BRANCHES = {
    dossier_id: (str(route["branch_path"]), str(route["branch_role"]))
    for dossier_id, route in ROUTES.items()
}
TABLE_I_DOCX_SECTIONS = tuple(str(value) for value in TABLE_I_PACKAGE.get("docx_sections", []))
if not TABLE_I_DOCX_SECTIONS:
    raise ValueError("prepared-dossier-routes.json table-i.docx_sections must be non-empty")

NODE_TABLE = ("Node ID", "Тип узла", "Название", "Период", "Связи", "Приоритет")
RELATION_TABLE = ("Source node", "Relation", "Target node", "Комментарий", "Уверенность")
CORPUS_SOURCE_TABLE = ("Источник / корпус", "Тип", "Что даёт", "Доступ / где искать", "Надёжность")
CONTROL_SOURCE_TABLE = ("Источник", "Тип", "Зачем нужен", "Ограничения")
RISK_TABLES = {
    ("Проблема", "В чём риск", "Как контролировать в ToS", "Какие источники нужны"),
    ("Проблема", "Риск", "Как контролировать в ToS", "Какие источники нужны"),
}
TERM_TABLE = ("Термин", "Язык", "Транслитерация", "Краткое значение", "Роль в ToS")
INCOMING_TRANSMISSION_TABLE = (
    "Источник / предыдущий узел",
    "Что передано",
    "Канал передачи",
    "Уверенность",
    "Примечание",
)
OUTGOING_TRANSMISSION_TABLE = (
    "Следующий узел / эпоха",
    "Что передаётся",
    "Канал",
    "Уверенность",
    "Что проверить дальше",
)
IDENTITY_METADATA_TABLES = {("Поле", "Значение")}
METADATA_TABLES = IDENTITY_METADATA_TABLES | {("Параметр", "Значение")}
EXTRACTED_TABLE_FAMILIES = {
    NODE_TABLE: "proposed_nodes",
    RELATION_TABLE: "proposed_relations",
    CORPUS_SOURCE_TABLE: "corpus_or_edition_anchors",
    CONTROL_SOURCE_TABLE: "control_or_review_anchors",
    **{header: "risk_control_source_needs" for header in RISK_TABLES},
    TERM_TABLE: "terms",
    INCOMING_TRANSMISSION_TABLE: "incoming_transmissions",
    OUTGOING_TRANSMISSION_TABLE: "outgoing_transmissions",
}
DEFERRED_CONTEXT_FAMILIES = {
    ("Измерение", "Граница"): "boundary_dimensions",
    ("Язык / письменность / медиум", "Роль в строке", "Период", "Что сохранилось", "Риск"): "language_script_medium",
    ("Корпус / текст / артефакт", "Дата / слой", "Язык", "Жанр", "Сохранность", "Почему важен для ToS"): "corpora_texts_artifacts",
    ("Фигура / тип авторства", "Период", "Роль", "Связанные тексты", "Уверенность"): "figures_authorship",
    ("Фигура / тип авторства", "Период", "Роль", "Связанные тексты / объекты", "Уверенность"): "figures_authorship",
    ("Фигура / тип авторства", "Период", "Роль", "Связанные тексты / вещи", "Уверенность"): "figures_authorship",
    ("Жанр", "Функция", "Примеры", "Философская значимость"): "genres",
    ("Уровень", "Оценка"): "audit_levels",
}


@dataclass
class Dossier:
    dossier_id: str
    title: str
    source_document: str
    docx_path: Path
    docx_section: str
    paragraph_count: int
    table_count: int
    table_row: str
    master_table: str
    master_status: str
    master_confidence: str
    node_rows: list[dict[str, Any]]
    relation_rows: list[dict[str, Any]]
    source_rows: list[dict[str, Any]]
    term_rows: list[dict[str, Any]]
    transmission_rows: list[dict[str, Any]]
    metadata_identity_posture: str
    metadata_headers: list[list[str]]
    coverage_tables: list[dict[str, Any]]


def repo_ref(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def scrub(text: str) -> str:
    text = re.sub(r"\ue200filecite\ue202[^ \n\t]*\ue201", "", text)
    text = re.sub(r"\s+", " ", text.replace("\xa0", " ")).strip()
    return text


def slugify(value: str) -> str:
    value = value.lower()
    value = value.replace("ё", "е")
    value = re.sub(r"[^a-z0-9а-я]+", "-", value)
    return value.strip("-") or "unnamed"


def branch_id_for(path_ref: str) -> str:
    suffix = path_ref.removeprefix("ToS/philosophy/").replace("/", ".")
    return f"philosophy.{suffix}"


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{repo_ref(path)} must contain a JSON object")
    return payload


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            rows.append(json.loads(line))
    return rows


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    rendered = "".join(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n" for row in rows)
    path.write_text(rendered, encoding="utf-8")


def row_dict(headers: tuple[str, ...], cells: list[str]) -> dict[str, str]:
    return {headers[index]: scrub(cells[index]) if index < len(cells) else "" for index in range(len(headers))}


def table_header(table: Any) -> tuple[str, ...]:
    if not table.rows:
        return ()
    return tuple(scrub(cell.text) for cell in table.rows[0].cells)


def table_body(table: Any) -> list[list[str]]:
    body: list[list[str]] = []
    for row in table.rows[1:]:
        cells = [scrub(cell.text) for cell in row.cells]
        if any(cells):
            body.append(cells)
    return body


def extract_dossier_id(path: Path) -> str:
    match = re.search(r"\bA\d{2}\b", path.name)
    if not match:
        raise ValueError(f"Cannot find dossier id in {path}")
    return match.group(0)


def discover_docx() -> list[Path]:
    paths = sorted(
        path
        for section in TABLE_I_DOCX_SECTIONS
        for path in (DOC_ROOT / section).glob("*.docx")
        if re.search(r"\bA\d{2}\b", path.name)
    )
    ids = [extract_dossier_id(path) for path in paths]
    expected = sorted(BRANCHES)
    if len(ids) != len(set(ids)):
        raise SystemExit(f"Prepared dossier ids must be unique, found {sorted(ids)}")
    if sorted(ids) != expected:
        raise SystemExit(f"Expected prepared dossier ids {expected}, found {sorted(ids)}")
    return paths


def load_docx_document(path: Path) -> Any:
    try:
        from docx import Document
    except ModuleNotFoundError as exc:
        raise SystemExit("Install python-docx to plant prepared DOCX dossiers.") from exc
    return Document(path)


def parse_dossier(path: Path, master_row: dict[str, Any]) -> Dossier:
    dossier_id = extract_dossier_id(path)
    if master_row.get("row_id") != dossier_id or master_row.get("table_id") != "table-i":
        raise ValueError(f"{dossier_id} does not match its Table I master row")
    document = load_docx_document(path)
    paragraphs = [scrub(paragraph.text) for paragraph in document.paragraphs if scrub(paragraph.text)]
    title = paragraphs[0] if paragraphs else f"ToS Deep Research: {dossier_id}"
    node_rows: list[dict[str, Any]] = []
    relation_rows: list[dict[str, Any]] = []
    source_rows: list[dict[str, Any]] = []
    term_rows: list[dict[str, Any]] = []
    transmission_rows: list[dict[str, Any]] = []
    metadata_headers: list[list[str]] = []
    coverage_tables: list[dict[str, Any]] = []
    observed_table_value: str | None = None
    observed_row_value: str | None = None
    table_row = dossier_id
    master_table = "I"

    for table_index, table in enumerate(document.tables, 1):
        header = table_header(table)
        rows = table_body(table)
        if header in EXTRACTED_TABLE_FAMILIES:
            coverage_class = "structured_primary_extracted"
            coverage_family = EXTRACTED_TABLE_FAMILIES[header]
        elif header in IDENTITY_METADATA_TABLES:
            coverage_class = "identity_metadata_examined"
            coverage_family = "dossier_identity_metadata"
        else:
            coverage_class = "deferred_context"
            coverage_family = DEFERRED_CONTEXT_FAMILIES.get(header, "other_context")
        coverage_tables.append(
            {
                "coverage_class": coverage_class,
                "family": coverage_family,
                "header": list(header),
                "row_count": len(rows),
                "source_table_index": table_index,
            }
        )
        if header in METADATA_TABLES:
            metadata_headers.append(list(header))
            for cells in rows:
                row = row_dict(header, cells)
                field_name = row.get(header[0])
                if field_name == "ROW_TO_EXPAND":
                    observed_row_value = row.get("Значение") or observed_row_value
                if field_name == "Таблица":
                    observed_table_value = row.get("Значение") or observed_table_value
        if header == NODE_TABLE:
            for row_index, cells in enumerate(rows, 1):
                row = row_dict(header, cells)
                original_id = row.get("Node ID") or f"{dossier_id}-node-{row_index:03d}"
                candidate_id = f"table-i-{dossier_id.lower()}-node-{row_index:03d}"
                node_rows.append(
                    {
                        "atlas_row_id": dossier_id,
                        "authority_posture": "prepared_research_candidate",
                        "branch_path": BRANCHES[dossier_id][0],
                        "candidate_id": candidate_id,
                        "canon_status": "pre-canon",
                        "connections": row.get("Связи", ""),
                        "dossier_id": dossier_id,
                        "label": row.get("Название") or original_id,
                        "node_kind": row.get("Тип узла") or "unspecified",
                        "original_node_id": original_id,
                        "period": row.get("Период", ""),
                        "priority": row.get("Приоритет", ""),
                        "source_document": path.name,
                        "source_row_index": row_index,
                        "source_table_index": table_index,
                        "source_ref": repo_ref(PROPOSED_NODES),
                    }
                )
        elif header == RELATION_TABLE:
            for row_index, cells in enumerate(rows, 1):
                row = row_dict(header, cells)
                relation_kind = slugify(row.get("Relation", "")).replace("-", "_")
                candidate_id = f"table-i-{dossier_id.lower()}-relation-{len(relation_rows) + 1:03d}"
                relation_rows.append(
                    {
                        "atlas_row_id": dossier_id,
                        "authority_posture": "prepared_research_candidate",
                        "branch_path": BRANCHES[dossier_id][0],
                        "candidate_id": candidate_id,
                        "canon_status": "pre-canon",
                        "comment": row.get("Комментарий", ""),
                        "confidence": row.get("Уверенность", ""),
                        "dossier_id": dossier_id,
                        "relation_kind": relation_kind or "related_to",
                        "relation_label": row.get("Relation", ""),
                        "source_document": path.name,
                        "source_endpoint_label": row.get("Source node", ""),
                        "source_ref": repo_ref(PROPOSED_RELATIONS),
                        "source_row_index": row_index,
                        "source_table_index": table_index,
                        "target_endpoint_label": row.get("Target node", ""),
                    }
                )
        elif header == CORPUS_SOURCE_TABLE:
            for row_index, cells in enumerate(rows, 1):
                row = row_dict(header, cells)
                source_rows.append(
                    {
                        "anchor_kind": "corpus_or_edition_anchor",
                        "atlas_row_id": dossier_id,
                        "branch_path": BRANCHES[dossier_id][0],
                        "contribution": row.get("Что даёт", ""),
                        "dossier_id": dossier_id,
                        "reliability": row.get("Надёжность", ""),
                        "route_status": "source_anchor_backlog",
                        "source_access": row.get("Доступ / где искать", ""),
                        "source_document": path.name,
                        "source_label": row.get("Источник / корпус", ""),
                        "source_ref": repo_ref(SOURCE_ANCHOR_BACKLOG),
                        "source_row_index": row_index,
                        "source_table_index": table_index,
                        "source_type": row.get("Тип", ""),
                    }
                )
        elif header == CONTROL_SOURCE_TABLE:
            for row_index, cells in enumerate(rows, 1):
                row = row_dict(header, cells)
                source_rows.append(
                    {
                        "anchor_kind": "control_or_review_anchor",
                        "atlas_row_id": dossier_id,
                        "branch_path": BRANCHES[dossier_id][0],
                        "contribution": row.get("Зачем нужен", ""),
                        "dossier_id": dossier_id,
                        "limitations": row.get("Ограничения", ""),
                        "route_status": "source_anchor_backlog",
                        "source_document": path.name,
                        "source_label": row.get("Источник", ""),
                        "source_ref": repo_ref(SOURCE_ANCHOR_BACKLOG),
                        "source_row_index": row_index,
                        "source_table_index": table_index,
                        "source_type": row.get("Тип", ""),
                    }
                )
        elif header in RISK_TABLES:
            for row_index, cells in enumerate(rows, 1):
                row = row_dict(header, cells)
                source_rows.append(
                    {
                        "anchor_kind": "risk_control_source_need",
                        "atlas_row_id": dossier_id,
                        "branch_path": BRANCHES[dossier_id][0],
                        "control": row.get("Как контролировать в ToS", ""),
                        "dossier_id": dossier_id,
                        "needed_sources": row.get("Какие источники нужны", ""),
                        "problem": row.get("Проблема", ""),
                        "risk": row.get("В чём риск", "") or row.get("Риск", ""),
                        "route_status": "source_anchor_backlog",
                        "source_document": path.name,
                        "source_ref": repo_ref(SOURCE_ANCHOR_BACKLOG),
                        "source_row_index": row_index,
                        "source_table_index": table_index,
                    }
                )
        elif header == TERM_TABLE:
            for row_index, cells in enumerate(rows, 1):
                row = row_dict(header, cells)
                term_rows.append(
                    {
                        "atlas_row_id": dossier_id,
                        "branch_path": BRANCHES[dossier_id][0],
                        "dossier_id": dossier_id,
                        "language": row.get("Язык", ""),
                        "meaning": row.get("Краткое значение", ""),
                        "role_in_tos": row.get("Роль в ToS", ""),
                        "source_document": path.name,
                        "source_ref": repo_ref(TERM_INDEX),
                        "source_row_index": row_index,
                        "source_table_index": table_index,
                        "term": row.get("Термин", ""),
                        "transliteration": row.get("Транслитерация", ""),
                    }
                )
        elif header == INCOMING_TRANSMISSION_TABLE:
            for row_index, cells in enumerate(rows, 1):
                row = row_dict(header, cells)
                transmission_rows.append(
                    {
                        "atlas_row_id": dossier_id,
                        "branch_path": BRANCHES[dossier_id][0],
                        "confidence": row.get("Уверенность", ""),
                        "dossier_id": dossier_id,
                        "direction": "incoming",
                        "from_or_to": row.get("Источник / предыдущий узел", ""),
                        "note": row.get("Примечание", ""),
                        "source_document": path.name,
                        "source_ref": repo_ref(TRANSMISSION_BACKLOG),
                        "source_row_index": row_index,
                        "source_table_index": table_index,
                        "transmission_channel": row.get("Канал передачи", ""),
                        "transmitted": row.get("Что передано", ""),
                    }
                )
        elif header == OUTGOING_TRANSMISSION_TABLE:
            for row_index, cells in enumerate(rows, 1):
                row = row_dict(header, cells)
                transmission_rows.append(
                    {
                        "atlas_row_id": dossier_id,
                        "branch_path": BRANCHES[dossier_id][0],
                        "confidence": row.get("Уверенность", ""),
                        "dossier_id": dossier_id,
                        "direction": "outgoing",
                        "from_or_to": row.get("Следующий узел / эпоха", ""),
                        "source_document": path.name,
                        "source_ref": repo_ref(TRANSMISSION_BACKLOG),
                        "source_row_index": row_index,
                        "source_table_index": table_index,
                        "transmission_channel": row.get("Канал", ""),
                        "transmitted": row.get("Что передаётся", ""),
                        "verify_next": row.get("Что проверить дальше", ""),
                    }
                )

    if observed_row_value:
        observed_id = re.search(r"\bA\d{2}\b", observed_row_value)
        if not observed_id or observed_id.group(0) != dossier_id:
            raise ValueError(f"{dossier_id} DOCX ROW_TO_EXPAND does not match its master-table identity")
        table_row = observed_row_value
    if observed_table_value:
        normalized_table = scrub(observed_table_value)
        if normalized_table not in {"I", "Table I", "Таблица I"}:
            raise ValueError(f"{dossier_id} DOCX table identity is not Table I: {normalized_table}")
        master_table = "I"
    if observed_row_value and observed_table_value:
        metadata_identity_posture = "docx_metadata_cross_checked"
    elif observed_row_value or observed_table_value:
        metadata_identity_posture = "partial_docx_metadata_cross_checked"
    else:
        metadata_identity_posture = "master_table_identity_fallback"

    normalized_master = master_row.get("normalized")
    if not isinstance(normalized_master, dict):
        raise ValueError(f"{dossier_id} master row must expose normalized metadata")
    route_projection = explicit_route_fields(
        dossier_id,
        master_status=str(normalized_master.get("status") or ""),
        master_confidence=str(normalized_master.get("confidence") or ""),
    )
    for rows in (node_rows, relation_rows, source_rows, term_rows, transmission_rows):
        for row in rows:
            row.update(route_projection)

    return Dossier(
        dossier_id=dossier_id,
        title=title,
        source_document=path.name,
        docx_path=path,
        docx_section=path.parent.name,
        paragraph_count=len(paragraphs),
        table_count=len(document.tables),
        table_row=table_row,
        master_table=master_table,
        master_status=str(normalized_master.get("status") or ""),
        master_confidence=str(normalized_master.get("confidence") or ""),
        node_rows=node_rows,
        relation_rows=relation_rows,
        source_rows=source_rows,
        term_rows=term_rows,
        transmission_rows=transmission_rows,
        metadata_identity_posture=metadata_identity_posture,
        metadata_headers=metadata_headers,
        coverage_tables=coverage_tables,
    )


def route_metadata(dossier_id: str) -> dict[str, Any]:
    route = ROUTES.get(dossier_id)
    if not isinstance(route, dict):
        raise ValueError(f"missing prepared dossier route for {dossier_id}")
    return route


def explicit_route_fields(
    dossier_id: str,
    *,
    master_status: str = "",
    master_confidence: str = "",
) -> dict[str, Any]:
    """Return only route metadata that changes the default planting contract."""
    route = route_metadata(dossier_id)
    defaults = TABLE_I_PACKAGE["route_defaults"]
    manual_review = route["review_posture"] == "manual_review_required"
    non_default_route = route["route_kind"] != defaults["route_kind"]
    if not manual_review and not non_default_route:
        return {}

    payload: dict[str, Any] = {
        "review_posture": str(route["review_posture"]),
        "route_kind": str(route["route_kind"]),
    }
    if manual_review:
        payload.update(
            {
                "master_confidence": master_confidence,
                "master_status": master_status,
            }
        )
    if route.get("review_reason"):
        payload["review_reason"] = str(route["review_reason"])
    if isinstance(route.get("route_constraints"), list):
        payload["route_constraints"] = [str(value) for value in route["route_constraints"]]
    return payload


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _xml_text(root: ElementTree.Element, path: str, namespaces: dict[str, str]) -> str | None:
    element = root.find(path, namespaces)
    if element is None or element.text is None:
        return None
    value = scrub(element.text)
    return value or None


def docx_package_metadata(path: Path) -> dict[str, Any]:
    custom_properties: dict[str, str] = {}
    creator: str | None = None
    last_modified_by: str | None = None
    signature_parts: list[str] = []
    with zipfile.ZipFile(path) as package:
        names = package.namelist()
        signature_parts = sorted(name for name in names if name.startswith("_xmlsignatures/"))
        if "docProps/custom.xml" in names:
            custom_root = ElementTree.fromstring(package.read("docProps/custom.xml"))
            for property_element in custom_root:
                name = property_element.attrib.get("name")
                if not name:
                    continue
                value = scrub("".join(property_element.itertext()))
                if value:
                    custom_properties[name] = value
        if "docProps/core.xml" in names:
            core_root = ElementTree.fromstring(package.read("docProps/core.xml"))
            creator = _xml_text(core_root, "dc:creator", {"dc": "http://purl.org/dc/elements/1.1/"})
            last_modified_by = _xml_text(
                core_root,
                "cp:lastModifiedBy",
                {"cp": "http://schemas.openxmlformats.org/package/2006/metadata/core-properties"},
            )
    return {
        "creator": creator,
        "custom_generator": custom_properties.get("generator"),
        "last_modified_by": last_modified_by,
        "signature_part_count": len(signature_parts),
    }


def intake_fingerprint(records: list[dict[str, Any]]) -> str:
    body = "".join(
        f"{row['relative_path']}\t{row['size_bytes']}\t{row['sha256']}\n"
        for row in sorted(records, key=lambda item: str(item["relative_path"]))
    )
    return hashlib.sha256(body.encode("utf-8")).hexdigest()


def build_intake_manifest(dossiers: list[Dossier]) -> dict[str, Any]:
    records: list[dict[str, Any]] = []
    for dossier in sorted(dossiers, key=lambda item: int(item.dossier_id[1:])):
        package_metadata = docx_package_metadata(dossier.docx_path)
        records.append(
            {
                "creator": package_metadata["creator"],
                "custom_generator": package_metadata["custom_generator"],
                "dossier_id": dossier.dossier_id,
                "last_modified_by": package_metadata["last_modified_by"],
                "relative_path": f"{dossier.docx_section}/{dossier.source_document}",
                "section": dossier.docx_section,
                "sha256": file_sha256(dossier.docx_path),
                "signature_part_count": package_metadata["signature_part_count"],
                "size_bytes": dossier.docx_path.stat().st_size,
            }
        )
    by_section = {
        section: [row for row in records if row["section"] == section]
        for section in TABLE_I_DOCX_SECTIONS
    }
    return {
        "schema_version": "tos_philosophy_docx_intake_manifest_v1",
        "path": repo_ref(INTAKE_MANIFEST),
        "owner_repo": "Tree-of-Sophia",
        "owner_surface": "ToS/research-packets/deep-research/philosophy/packet-contract.md",
        "recorded_on": INTAKE_RECORDED_ON,
        "artifact_role": "operator-local prepared Table I dossier extraction input",
        "capture_posture": {
            "custody": "operator_local_untracked_payload",
            "origin_verification": "unverified",
            "author_identity": None,
            "session_or_export_id": None,
            "generator_property_observed": sorted(
                {str(row["custom_generator"]) for row in records if row.get("custom_generator")}
            ),
            "signature_posture": (
                "signature_parts_present"
                if any(int(row["signature_part_count"]) for row in records)
                else "no_ooxml_signature_parts_observed"
            ),
        },
        "fingerprint_contract": {
            "algorithm": "sha256",
            "record_format": "relative_path<TAB>size_bytes<TAB>sha256<LF>",
            "ordering": "relative_path_utf8_ascending",
        },
        "bundle_fingerprint": intake_fingerprint(records),
        "section_fingerprints": {
            section: intake_fingerprint(section_records)
            for section, section_records in by_section.items()
        },
        "section_counts": {section: len(section_records) for section, section_records in by_section.items()},
        "file_count": len(records),
        "files": records,
        "claim_limit": (
            "This manifest proves only the bytes, sizes, logical section paths, and OOXML metadata observed in the "
            "operator-local capture at planting time. It does not prove authorship, export-session identity, origin, "
            "signature trust, source-witness status, claim truth, rights, review, doctrine, or canon acceptance."
        ),
    }


def coverage_summary(dossiers: list[Dossier]) -> dict[str, Any]:
    class_counts: Counter[str] = Counter()
    family_counts: Counter[str] = Counter()
    header_counts: Counter[tuple[str, ...]] = Counter()
    for dossier in dossiers:
        for table in dossier.coverage_tables:
            row_count = int(table["row_count"])
            class_counts[str(table["coverage_class"])] += row_count
            family_counts[str(table["family"])] += row_count
            header_counts[tuple(str(value) for value in table["header"])] += row_count
    return {
        "dossier_count": len(dossiers),
        "table_body_row_count": sum(class_counts.values()),
        "coverage_class_counts": dict(sorted(class_counts.items())),
        "family_row_counts": dict(sorted(family_counts.items())),
        "headers": [
            {"header": list(header), "row_count": count}
            for header, count in sorted(header_counts.items(), key=lambda item: (item[0], item[1]))
        ],
    }


def build_extraction_coverage(dossiers: list[Dossier]) -> dict[str, Any]:
    diagnostics: list[dict[str, Any]] = []
    dossier_rows: list[dict[str, Any]] = []
    for dossier in sorted(dossiers, key=lambda item: int(item.dossier_id[1:])):
        summary = coverage_summary([dossier])
        risk_rows = int(summary["family_row_counts"].get("risk_control_source_needs", 0))
        dossier_diagnostics: list[str] = []
        if risk_rows == 0:
            dossier_diagnostics.append("structured_risk_table_absent")
            diagnostics.append(
                {
                    "code": "structured_risk_table_absent",
                    "dossier_id": dossier.dossier_id,
                    "posture": "prose_only_not_synthesized",
                    "message": "No structured risk rows were extracted; prose risk language remains only in the local DOCX.",
                }
            )
        if dossier.metadata_identity_posture != "docx_metadata_cross_checked":
            dossier_diagnostics.append(dossier.metadata_identity_posture)
        dossier_rows.append(
            {
                "dossier_id": dossier.dossier_id,
                "docx_section": dossier.docx_section,
                "metadata_headers": dossier.metadata_headers,
                "metadata_identity_posture": dossier.metadata_identity_posture,
                "review_posture": str(route_metadata(dossier.dossier_id)["review_posture"]),
                "route_kind": str(route_metadata(dossier.dossier_id)["route_kind"]),
                "structured_risk_row_count": risk_rows,
                "coverage": summary,
                "diagnostics": dossier_diagnostics,
            }
        )
    by_section = {
        section: coverage_summary([dossier for dossier in dossiers if dossier.docx_section == section])
        for section in TABLE_I_DOCX_SECTIONS
    }
    return {
        "schema_version": "tos_philosophy_docx_extraction_coverage_v1",
        "path": repo_ref(EXTRACTION_COVERAGE),
        "owner_repo": "Tree-of-Sophia",
        "owner_surface": "ToS/philosophy/graph-workbench/PLANTING_INTERFACE.md",
        "intake_manifest_ref": repo_ref(INTAKE_MANIFEST),
        "route_map_ref": repo_ref(PREPARED_DOSSIER_ROUTES),
        "coverage_posture": "bounded_structured_planting_not_full_dossier_transfer",
        "structured_primary_families": sorted(set(EXTRACTED_TABLE_FAMILIES.values())),
        "identity_metadata_rule": (
            "Поле|Значение rows are examined only to cross-check Table I and ROW_TO_EXPAND; other metadata values "
            "are not represented as full dossier transfer. The A44 Параметр|Значение alias is also inspected for "
            "those two identity fields, but the table remains counted as deferred context."
        ),
        "deferred_context_rule": (
            "Context table rows remain in the operator-local DOCX and are counted here; they are not silently "
            "promoted into nodes, relations, source anchors, terms, transmissions, source witnesses, or canon."
        ),
        "summary": coverage_summary(dossiers),
        "sections": by_section,
        "dossiers": dossier_rows,
        "diagnostics": diagnostics,
        "claim_limit": (
            "Coverage accounts for non-empty DOCX table-body rows under the current parser. It does not cover prose "
            "paragraph semantics, verify citations, accept risk judgments, or establish complete dossier transfer."
        ),
    }


def write_intake_and_coverage_surfaces(dossiers: list[Dossier]) -> None:
    write_json(INTAKE_MANIFEST, build_intake_manifest(dossiers))
    write_json(EXTRACTION_COVERAGE, build_extraction_coverage(dossiers))


def update_atlas(dossiers: list[Dossier]) -> None:
    by_id = {dossier.dossier_id: dossier for dossier in dossiers}
    rows = load_jsonl(TABLE_I_ROWS)
    for row in rows:
        row_id = str(row.get("row_id") or "")
        if row_id in by_id:
            row["dossier_available"] = True
            row["dossier_id"] = row_id
            row.setdefault("normalized", {})["prepared_branch_path"] = BRANCHES[row_id][0]
        else:
            row["dossier_available"] = False
            row["dossier_id"] = None
            if isinstance(row.get("normalized"), dict):
                row["normalized"].pop("prepared_branch_path", None)
    write_jsonl(TABLE_I_ROWS, rows)

    table_manifest = load_json(TABLE_I_MANIFEST)
    table_manifest["available_dossiers"] = sorted(by_id, key=lambda value: int(value[1:]))
    write_json(TABLE_I_MANIFEST, table_manifest)

    atlas_manifest = load_json(ATLAS_MANIFEST)
    atlas_manifest["dossiers"]["available_count"] = len(dossiers)
    atlas_manifest["dossiers"]["route_map"] = repo_ref(PREPARED_DOSSIER_ROUTES)
    atlas_manifest["dossiers"]["intake_manifest"] = repo_ref(INTAKE_MANIFEST)
    atlas_manifest["dossiers"]["extraction_coverage"] = repo_ref(EXTRACTION_COVERAGE)
    write_json(ATLAS_MANIFEST, atlas_manifest)

    dossier_branch = load_json(DOSSIER_BRANCH_MANIFEST)
    dossier_branch["dossier_count"] = len(dossiers)
    dossier_branch["source_anchor_backlog"] = repo_ref(SOURCE_ANCHOR_BACKLOG)
    dossier_branch["term_index"] = repo_ref(TERM_INDEX)
    dossier_branch["transmission_backlog"] = repo_ref(TRANSMISSION_BACKLOG)
    dossier_branch["prepared_dossier_routes"] = repo_ref(PREPARED_DOSSIER_ROUTES)
    dossier_branch["intake_manifest"] = repo_ref(INTAKE_MANIFEST)
    dossier_branch["extraction_coverage"] = repo_ref(EXTRACTION_COVERAGE)
    write_json(DOSSIER_BRANCH_MANIFEST, dossier_branch)


def write_dossier_indexes(dossiers: list[Dossier]) -> None:
    node_counter: Counter[str] = Counter()
    relation_counter: Counter[str] = Counter()
    index_rows: list[dict[str, Any]] = []
    all_sources: list[dict[str, Any]] = []
    all_terms: list[dict[str, Any]] = []
    all_transmissions: list[dict[str, Any]] = []
    for dossier in dossiers:
        route = route_metadata(dossier.dossier_id)
        node_type_counts = Counter(str(row.get("node_kind") or "unspecified") for row in dossier.node_rows)
        relation_counts = Counter(str(row.get("relation_kind") or "related_to") for row in dossier.relation_rows)
        node_counter.update(node_type_counts)
        relation_counter.update(relation_counts)
        index_rows.append(
            {
                "atlas_status": "prepared_dossier_indexed",
                "branch_path": BRANCHES[dossier.dossier_id][0],
                "dossier_id": dossier.dossier_id,
                "docx_section": dossier.docx_section,
                "extraction_coverage_ref": repo_ref(EXTRACTION_COVERAGE),
                "intake_manifest_ref": repo_ref(INTAKE_MANIFEST),
                "master_table": dossier.master_table,
                "master_confidence": dossier.master_confidence,
                "master_status": dossier.master_status,
                "metadata_identity_posture": dossier.metadata_identity_posture,
                "node_row_count": len(dossier.node_rows),
                "node_type_counts": dict(sorted(node_type_counts.items())),
                "paragraph_count": dossier.paragraph_count,
                "relation_counts": dict(sorted(relation_counts.items())),
                "relation_row_count": len(dossier.relation_rows),
                "source_anchor_count": len(dossier.source_rows),
                "source_document": dossier.source_document,
                "table_count": dossier.table_count,
                "table_row": dossier.table_row,
                "term_count": len(dossier.term_rows),
                "title": dossier.title,
                "transmission_count": len(dossier.transmission_rows),
                "review_posture": str(route["review_posture"]),
                **({"review_reason": str(route["review_reason"])} if route.get("review_reason") else {}),
                "route_kind": str(route["route_kind"]),
                **(
                    {"route_constraints": [str(value) for value in route["route_constraints"]]}
                    if isinstance(route.get("route_constraints"), list)
                    else {}
                ),
            }
        )
        all_sources.extend(dossier.source_rows)
        all_terms.extend(dossier.term_rows)
        all_transmissions.extend(dossier.transmission_rows)

    write_jsonl(DOSSIER_INDEX, sorted(index_rows, key=lambda row: int(row["dossier_id"][1:])))
    write_json(
        DOSSIER_SUMMARY,
        {
            "schema_version": "tos_philosophy_atlas_dossier_graph_shape_v1",
            "path": repo_ref(DOSSIER_SUMMARY),
            "source": "prepared A-series Deep Research dossier DOCX files",
            "source_posture": "bounded structured extraction from operator-local non-authoritative research packets",
            "intake_manifest_ref": repo_ref(INTAKE_MANIFEST),
            "extraction_coverage_ref": repo_ref(EXTRACTION_COVERAGE),
            "dossier_count": len(dossiers),
            "node_row_count": sum(len(dossier.node_rows) for dossier in dossiers),
            "relation_row_count": sum(len(dossier.relation_rows) for dossier in dossiers),
            "node_type_counts": dict(sorted(node_counter.items())),
            "relation_counts": dict(sorted(relation_counter.items())),
            "source_anchor_count": len(all_sources),
            "term_count": len(all_terms),
            "transmission_count": len(all_transmissions),
        },
    )
    write_jsonl(SOURCE_ANCHOR_BACKLOG, all_sources)
    write_jsonl(TERM_INDEX, all_terms)
    write_jsonl(TRANSMISSION_BACKLOG, all_transmissions)


def resolve_relation_endpoints(dossiers: list[Dossier]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    all_nodes: list[dict[str, Any]] = []
    all_relations: list[dict[str, Any]] = []
    nodes_by_dossier: dict[str, dict[str, str]] = {}
    for dossier in dossiers:
        lookup: dict[str, str] = {}
        for row in dossier.node_rows:
            all_nodes.append(row)
            for key in ("original_node_id", "label"):
                value = str(row.get(key) or "").strip()
                if value:
                    lookup[value] = str(row["candidate_id"])
        nodes_by_dossier[dossier.dossier_id] = lookup
    for dossier in dossiers:
        lookup = nodes_by_dossier[dossier.dossier_id]
        for row in dossier.relation_rows:
            source = str(row.get("source_endpoint_label") or "")
            target = str(row.get("target_endpoint_label") or "")
            row["source_candidate_id"] = lookup.get(source)
            row["target_candidate_id"] = lookup.get(target)
            row["endpoint_resolution"] = (
                "matched_nodes" if row["source_candidate_id"] and row["target_candidate_id"] else "label_endpoint"
            )
            all_relations.append(row)
    return all_nodes, all_relations


def _text_status_for_ru(label: str) -> tuple[str, str]:
    translated, status = russian_label(label)
    if re.search(r"[А-Яа-яЁё]", label):
        return translated, status
    if translated != label or status == "reviewed":
        return translated, status
    return translated, "pending"


def _text_status_for_en(label: str) -> tuple[str, str]:
    translated, status = english_label(label)
    if status == "draft" and translated:
        translated = translated[0].upper() + translated[1:]
    return translated, status if translated != label or status != "source" else "source"


def text_bearing_language_packet(row: dict[str, Any]) -> dict[str, Any]:
    label = str(row.get("label") or "").strip()
    ru, ru_status = _text_status_for_ru(label)
    en, en_status = _text_status_for_en(label)
    source_ref = repo_ref(LANGUAGE_PACKETS)
    source_node_ref = str(row.get("source_ref") or repo_ref(PROPOSED_NODES))
    source_refs = sorted({source_ref, source_node_ref, LANGUAGE_PACKET_CONTRACT_REF, LANGUAGE_REGISTRY_REF})
    return {
        "schema_version": "tos_philosophy_text_bearing_language_packet_v1",
        "packet_id": f"language-packet:{row['candidate_id']}",
        "node_ref": {
            "id": row["candidate_id"],
            "id_kind": "candidate_id",
            "source_ref": source_node_ref,
        },
        "node_kind": row.get("node_kind"),
        "branch_path": row.get("branch_path"),
        "authority_posture": row.get("authority_posture"),
        "canon_status": row.get("canon_status"),
        "atlas_row_id": row.get("atlas_row_id"),
        "dossier_id": row.get("dossier_id"),
        "source_document": row.get("source_document"),
        "source_row_index": row.get("source_row_index"),
        "source_table_index": row.get("source_table_index"),
        "source_label": label,
        "source_ref": source_ref,
        "source_refs": source_refs,
        "language_registry_ref": LANGUAGE_REGISTRY_REF,
        "text_bearing_nodes_contract_ref": LANGUAGE_PACKET_CONTRACT_REF,
        "title_block": {
            "original": {
                "value": None,
                "language": "und",
                "script": "Zzzz",
                "transliteration": None,
                "attestation_status": "unknown",
                "source_ref": source_node_ref,
                "review_status": "pending_original_witness",
            },
            "ru": {
                "value": ru,
                "translation_status": ru_status,
                "source_ref": source_node_ref,
            },
            "en": {
                "value": en,
                "translation_status": en_status,
                "source_ref": source_node_ref,
            },
        },
        "witness_block": {
            "source_witnesses": [],
            "witness_status": "pending_source_witness_anchor",
            "required_next_fields": [
                "witness_id",
                "witness_kind",
                "language",
                "script",
                "repository_or_corpus",
                "source_ref",
            ],
        },
        "relation_pressure": [
            {
                "predicate": "has_original_language",
                "target_status": "unresolved",
                "review_route": LANGUAGE_PACKET_CONTRACT_REF,
            },
            {
                "predicate": "uses_script",
                "target_status": "unresolved",
                "review_route": LANGUAGE_PACKET_CONTRACT_REF,
            },
            {
                "predicate": "has_witness",
                "target_status": "unresolved",
                "review_route": "ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl",
            },
        ],
    }


def build_language_packets(nodes: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        text_bearing_language_packet(row)
        for row in nodes
        if str(row.get("node_kind") or "") in TEXT_BEARING_NODE_KINDS
    ]


def write_language_packet_surfaces(nodes: list[dict[str, Any]]) -> list[dict[str, Any]]:
    packets = build_language_packets(nodes)
    write_jsonl(LANGUAGE_PACKETS, packets)
    write_json(
        LANGUAGE_PACKETS_MANIFEST,
        {
            "branch_id": "philosophy.graph-workbench.language-packets",
            "path": "ToS/philosophy/graph-workbench/language-packets",
            "role": "text-bearing language packets emitted by prepared dossier planting before canon promotion",
            "contract_ref": LANGUAGE_PACKET_CONTRACT_REF,
            "language_registry_ref": LANGUAGE_REGISTRY_REF,
            "packet_count": len(packets),
            "source_ref": repo_ref(LANGUAGE_PACKETS),
        },
    )
    LANGUAGE_PACKETS_README.write_text(
        "# Language Packets\n\n"
        "`language-packets/` contains pre-canon language packets for text-bearing philosophy nodes.\n\n"
        "The current Table I packet file is generated from prepared dossier proposed nodes. "
        "It records original-language uncertainty, Russian and English display slots, witness posture, "
        "and language/script relation pressure without promoting any source claim to canon.\n\n"
        "| Surface | Role |\n"
        "| --- | --- |\n"
        "| `table-i-text-bearing-nodes.jsonl` | generated packets for Table I text-corpus candidates |\n"
        f"| `{LANGUAGE_PACKET_CONTRACT_REF}` | packet contract |\n"
        f"| `{LANGUAGE_REGISTRY_REF}` | language and script registry |\n",
        encoding="utf-8",
    )
    return packets


def write_graph_workbench(dossiers: list[Dossier]) -> None:
    nodes, relations = resolve_relation_endpoints(dossiers)
    language_packets = write_language_packet_surfaces(nodes)
    write_jsonl(PROPOSED_NODES, nodes)
    write_jsonl(PROPOSED_RELATIONS, relations)
    write_json(
        BRANCH_FRAGMENTS,
        {
            "schema_version": "tos_philosophy_branch_fragments_v1",
            "path": repo_ref(BRANCH_FRAGMENTS),
            "source_ref": repo_ref(DOSSIER_INDEX),
            "canon_status": "pre-canon",
            "branch_count": len(dossiers),
            "language_packet_count": len(language_packets),
            "language_packets_ref": repo_ref(LANGUAGE_PACKETS),
            "branches": [
                {
                    "branch_path": BRANCHES[dossier.dossier_id][0],
                    "dossier_id": dossier.dossier_id,
                    "docx_section": dossier.docx_section,
                    "title": dossier.title,
                    "node_row_count": len(dossier.node_rows),
                    "relation_row_count": len(dossier.relation_rows),
                    "source_anchor_count": len(dossier.source_rows),
                    "term_count": len(dossier.term_rows),
                    "transmission_count": len(dossier.transmission_rows),
                    "review_posture": str(route_metadata(dossier.dossier_id)["review_posture"]),
                    "route_kind": str(route_metadata(dossier.dossier_id)["route_kind"]),
                }
                for dossier in sorted(dossiers, key=lambda item: int(item.dossier_id[1:]))
            ],
        },
    )
    PROMOTION_LEDGER.parent.mkdir(parents=True, exist_ok=True)
    PROMOTION_LEDGER.write_text(
        "# Table I Prepared Dossiers\n\n"
        "This ledger records the first living planting of the prepared Table I corpus.\n\n"
        "| Surface | Count | Status |\n"
        "| --- | ---: | --- |\n"
        f"| prepared dossiers | {len(dossiers)} | atlas indexed |\n"
        f"| proposed nodes | {len(nodes)} | pre-canon graph workbench |\n"
        f"| proposed relations | {len(relations)} | pre-canon graph workbench |\n"
        f"| text-bearing language packets | {len(language_packets)} | pre-canon multilingual review |\n"
        f"| branch fragments | {len(dossiers)} | era/region/tradition or explicit frontier branch bodies |\n\n"
        "Promotion remains a later authored review step through ToS canon route cards.\n",
        encoding="utf-8",
    )


def branch_title(dossier: Dossier) -> str:
    return dossier.title.replace("ToS Deep Research:", "").strip()


def top_counts(rows: list[dict[str, Any]], key: str, limit: int = 8) -> list[str]:
    counter = Counter(str(row.get(key) or "unspecified") for row in rows)
    return [f"{name}: {count}" for name, count in counter.most_common(limit)]


def render_branch_readme(dossier: Dossier) -> str:
    title = branch_title(dossier)
    node_pressure = ", ".join(top_counts(dossier.node_rows, "node_kind")) or "none"
    relation_pressure = ", ".join(top_counts(dossier.relation_rows, "relation_kind")) or "none"
    path_ref, _role = BRANCHES[dossier.dossier_id]
    route = route_metadata(dossier.dossier_id)
    planting_paths = sorted(
        (REPO_ROOT / path_ref / "sources/plantings").glob(
            "*/source-planting.json"
        )
    )
    planting_row = (
        "| `sources/plantings/` | exact source-witness routes already planted from backlog anchors |\n"
        if planting_paths
        else ""
    )
    review_reason = str(route.get("review_reason") or "")
    route_constraints = route.get("route_constraints")
    review_block = (
        "## Manual Review Gate\n\n"
        f"- Posture: `{route['review_posture']}`\n"
        f"- Master-table status/confidence: `{dossier.master_status}/{dossier.master_confidence}`\n"
        f"- Reason: {review_reason}\n\n"
        if route["review_posture"] == "manual_review_required"
        else ""
    )
    constraint_block = (
        "## Route Constraints\n\n"
        + "".join(f"- `{value}`\n" for value in route_constraints)
        + "\n"
        if isinstance(route_constraints, list) and route_constraints
        else ""
    )
    return (
        f"# {title}\n\n"
        f"Atlas row: `{dossier.dossier_id}`. Prepared dossier: `{dossier.source_document}`.\n\n"
        "This branch is the ToS philosophy home for the prepared dossier's first tree-shaped growth. "
        "It keeps the dossier material in the philosophy tree while leaving canon promotion to a later authored review.\n\n"
        "## Branch Pressure\n\n"
        f"- Candidate node rows: {len(dossier.node_rows)}\n"
        f"- Candidate relation rows: {len(dossier.relation_rows)}\n"
        f"- Source-anchor backlog rows: {len(dossier.source_rows)}\n"
        f"- Term rows: {len(dossier.term_rows)}\n"
        f"- Transmission rows: {len(dossier.transmission_rows)}\n"
        f"- Node pressure: {node_pressure}\n"
        f"- Relation pressure: {relation_pressure}\n\n"
        f"{review_block}"
        f"{constraint_block}"
        "## Local Surfaces\n\n"
        "| Surface | Role |\n"
        "| --- | --- |\n"
        "| `sources/source-anchor-backlog.jsonl` | future real source witness and edition anchors for this branch |\n"
        f"{planting_row}"
        "| `graph-workbench/pre-canon-summary.json` | local summary of proposed graph rows before canon review |\n\n"
        "Global proposed node and relation rows for this branch are aggregated in "
        "`ToS/philosophy/graph-workbench/proposed-nodes/table-i-prepared-dossiers.jsonl` and "
        "`ToS/philosophy/graph-workbench/proposed-relations/table-i-prepared-dossiers.jsonl`.\n"
    )


def remove_obsolete_generated_branch() -> None:
    if not OBSOLETE_GENERATED_BRANCH.exists():
        return
    files = [path for path in OBSOLETE_GENERATED_BRANCH.rglob("*") if path.is_file()]
    if any(path.name != "branch.manifest.json" for path in files):
        raise SystemExit(f"{repo_ref(OBSOLETE_GENERATED_BRANCH)} contains non-generated files; refusing to remove it")
    shutil.rmtree(OBSOLETE_GENERATED_BRANCH)


def write_branch_surfaces(dossiers: list[Dossier]) -> None:
    remove_obsolete_generated_branch()
    parent_children: dict[str, set[str]] = defaultdict(set)
    parent_roles: dict[str, str] = {}
    for dossier_id, (path_ref, _role) in BRANCHES.items():
        ancestor_roles = route_metadata(dossier_id).get("ancestor_roles")
        if isinstance(ancestor_roles, dict):
            for ancestor_path, ancestor_role in ancestor_roles.items():
                if ancestor_path in parent_roles and parent_roles[ancestor_path] != ancestor_role:
                    raise ValueError(f"conflicting ancestor role for {ancestor_path}")
                parent_roles[str(ancestor_path)] = str(ancestor_role)
        parts = Path(path_ref).parts
        for index in range(3, len(parts)):
            parent = Path(*parts[:index]).as_posix()
            child = Path(*parts[index : index + 1]).as_posix()
            parent_children[parent].add(child)

    existing_manifest_paths = {
        repo_ref(path): load_json(path)
        for path in (REPO_ROOT / "ToS/philosophy").glob("**/branch.manifest.json")
        if not path.is_relative_to(OBSOLETE_GENERATED_BRANCH)
    }

    for path_ref, children in sorted(parent_children.items()):
        path = REPO_ROOT / path_ref
        path.mkdir(parents=True, exist_ok=True)
        existing = existing_manifest_paths.get(f"{path_ref}/branch.manifest.json", {})
        existing_children = {
            child for child in existing.get("children", []) if isinstance(child, str) and child != "regions/ancient-near-east"
        }
        payload = {
            "branch_id": branch_id_for(path_ref),
            "children": sorted(existing_children | {child for child in children}),
            "path": path_ref,
            "role": existing.get("role") or parent_roles.get(path_ref) or f"{Path(path_ref).name} philosophy branch",
        }
        write_json(path / "branch.manifest.json", payload)

    for dossier in dossiers:
        path_ref, role = BRANCHES[dossier.dossier_id]
        path = REPO_ROOT / path_ref
        path.mkdir(parents=True, exist_ok=True)
        planting_refs = sorted(
            repo_ref(planting_path)
            for planting_path in (path / "sources/plantings").glob(
                "*/source-planting.json"
            )
        )
        write_json(
            path / "branch.manifest.json",
            {
                "branch_id": branch_id_for(path_ref),
                "path": path_ref,
                "role": role,
                "atlas_rows": [dossier.dossier_id],
                "prepared_dossiers": [dossier.dossier_id],
                "evidence_status": "prepared_dossier_branch",
                **explicit_route_fields(
                    dossier.dossier_id,
                    master_status=dossier.master_status,
                    master_confidence=dossier.master_confidence,
                ),
                "expected_local_children": ["sources", "graph-workbench"],
                "source_anchor_backlog": f"{path_ref}/sources/source-anchor-backlog.jsonl",
                **(
                    {
                        "source_planting_count": len(planting_refs),
                        "source_planting_refs": planting_refs,
                    }
                    if planting_refs
                    else {}
                ),
                "local_graph_summary": f"{path_ref}/graph-workbench/pre-canon-summary.json",
            },
        )
        (path / "README.md").write_text(render_branch_readme(dossier), encoding="utf-8")

        sources_path = path / "sources"
        graph_path = path / "graph-workbench"
        sources_path.mkdir(exist_ok=True)
        graph_path.mkdir(exist_ok=True)
        write_json(
            sources_path / "branch.manifest.json",
            {
                "branch_id": branch_id_for(f"{path_ref}/sources"),
                "path": f"{path_ref}/sources",
                "role": (
                    f"source-anchor backlog and planted source routes for {dossier.dossier_id}"
                    if planting_refs
                    else f"source-anchor backlog for {dossier.dossier_id}"
                ),
                "anchor_count": len(dossier.source_rows),
                **explicit_route_fields(
                    dossier.dossier_id,
                    master_status=dossier.master_status,
                    master_confidence=dossier.master_confidence,
                ),
                **(
                    {
                        "planting_count": len(planting_refs),
                        "planting_refs": planting_refs,
                    }
                    if planting_refs
                    else {}
                ),
            },
        )
        write_jsonl(sources_path / "source-anchor-backlog.jsonl", dossier.source_rows)
        write_json(
            graph_path / "branch.manifest.json",
            {
                "branch_id": branch_id_for(f"{path_ref}/graph-workbench"),
                "path": f"{path_ref}/graph-workbench",
                "role": f"local pre-canon graph summary for {dossier.dossier_id}",
                "proposed_node_count": len(dossier.node_rows),
                "proposed_relation_count": len(dossier.relation_rows),
                **explicit_route_fields(
                    dossier.dossier_id,
                    master_status=dossier.master_status,
                    master_confidence=dossier.master_confidence,
                ),
            },
        )
        write_json(
            graph_path / "pre-canon-summary.json",
            {
                "schema_version": "tos_philosophy_local_graph_summary_v1",
                "path": f"{path_ref}/graph-workbench/pre-canon-summary.json",
                "atlas_row_id": dossier.dossier_id,
                "dossier_id": dossier.dossier_id,
                "branch_path": path_ref,
                "canon_status": "pre-canon",
                **explicit_route_fields(
                    dossier.dossier_id,
                    master_status=dossier.master_status,
                    master_confidence=dossier.master_confidence,
                ),
                "proposed_nodes_ref": repo_ref(PROPOSED_NODES),
                "proposed_relations_ref": repo_ref(PROPOSED_RELATIONS),
                "node_row_count": len(dossier.node_rows),
                "relation_row_count": len(dossier.relation_rows),
                "node_type_counts": dict(sorted(Counter(str(row.get("node_kind")) for row in dossier.node_rows).items())),
                "relation_counts": dict(sorted(Counter(str(row.get("relation_kind")) for row in dossier.relation_rows).items())),
            },
        )


def refresh_philosophy_manifest() -> None:
    manifest = load_json(PHILOSOPHY_MANIFEST)
    manifest["branch_manifests"] = sorted(
        repo_ref(path)
        for path in (REPO_ROOT / "ToS/philosophy").glob("**/branch.manifest.json")
    )
    atlas_routes = set(manifest.get("atlas_routes", []))
    atlas_routes.update(
        {
            repo_ref(SOURCE_ANCHOR_BACKLOG),
            repo_ref(TERM_INDEX),
            repo_ref(TRANSMISSION_BACKLOG),
            repo_ref(PREPARED_DOSSIER_ROUTES),
            LANGUAGE_REGISTRY_REF,
            LANGUAGE_PACKET_CONTRACT_REF,
        }
    )
    manifest["atlas_routes"] = sorted(atlas_routes)
    research_packet_contracts = set(manifest.get("research_packet_contracts", []))
    research_packet_contracts.update({repo_ref(INTAKE_MANIFEST), repo_ref(EXTRACTION_COVERAGE)})
    manifest["research_packet_contracts"] = sorted(research_packet_contracts)
    write_json(PHILOSOPHY_MANIFEST, manifest)


def write_readmes() -> None:
    (REPO_ROOT / "ToS/philosophy/atlas/dossiers/README.md").write_text(
        "# Dossiers\n\n"
        "`dossiers/` indexes the prepared A-series Deep Research documents for the philosophy atlas.\n\n"
        "The first Table I planting records dossier identity, branch route, graph-row pressure, "
        "source-anchor backlog, terms, and transmission rows while keeping canon promotion separate.\n\n"
        "| Surface | Role |\n"
        "| --- | --- |\n"
        "| `index.jsonl` | one entry per prepared A-series dossier |\n"
        "| `graph-shape-summary.json` | aggregate node, relation, source-anchor, term, and transmission pressure |\n"
        "| `prepared-dossier-routes.json` | source-owned route map from prepared dossier ids to philosophy branch homes |\n"
        f"| `{repo_ref(INTAKE_MANIFEST)}` | tracked fixity and capture-posture manifest for the untracked local DOCX bytes |\n"
        f"| `{repo_ref(EXTRACTION_COVERAGE)}` | explicit structured extraction and deferred-context coverage |\n"
        "| `source-anchor-backlog.jsonl` | future source witness, edition, corpus, and risk-control anchors |\n"
        "| `term-index.jsonl` | prepared term rows extracted from dossier terminology tables |\n"
        "| `transmission-backlog.jsonl` | incoming and outgoing transmission rows extracted from dossier tables |\n\n"
        "Text-bearing language packets for graph review live in "
        "`ToS/philosophy/graph-workbench/language-packets/` and follow "
        "`ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json`.\n\n"
        "Branch bodies live under `ToS/philosophy/eras/...` or an explicit `ToS/philosophy/frontiers/...` route, "
        "and pre-canon graph rows live under "
        "`ToS/philosophy/graph-workbench/`.\n",
        encoding="utf-8",
    )
    (REPO_ROOT / "ToS/philosophy/atlas/README.md").write_text(
        "# Philosophy Atlas\n\n"
        "`atlas/` is the prepared navigation body for the whole ToS philosophy tree.\n\n"
        "It holds the master-table row spine, prepared A-series dossier index, and aggregate "
        "pressure maps that tell the philosophy tree what must grow next.\n\n"
        "## Shape\n\n"
        "```text\n"
        "atlas/\n"
        "  master-tables/\n"
        "    table-i/\n"
        "    table-ii/\n"
        "    table-iii/\n"
        "  dossiers/\n"
        "    index.jsonl\n"
        "    graph-shape-summary.json\n"
        "    source-anchor-backlog.jsonl\n"
        "    term-index.jsonl\n"
        "    transmission-backlog.jsonl\n"
        "  multilingual/\n"
        "    content-labels.json\n"
        "    language-registry.json\n"
        "    text-bearing-nodes.contract.json\n"
        "```\n\n"
        "The atlas is prepared navigation and growth pressure. Branch bodies live in "
        "`ToS/philosophy/eras/...` or the explicit non-era `ToS/philosophy/frontiers/...` route; pre-canon graph material lives in "
        "`ToS/philosophy/graph-workbench/`; authored canon relation packs live in the canon route.\n\n"
        "`multilingual/` is a source-owned display companion for the atlas and generated "
        "graph projections. It preserves the route rule that planted works must carry "
        "their attested original form when available, plus Russian and English display "
        "labels for review and downstream visualization.\n\n"
        "For works, corpora, inscriptions, source witnesses, translations, versions, and "
        "commentaries, `multilingual/text-bearing-nodes.contract.json` is the planting "
        "contract. It keeps original-language title posture, transliteration, Russian "
        "review text, English review/runtime text, witness posture, and graph relation "
        "pressure in one source-owned packet shape.\n",
        encoding="utf-8",
    )


def main() -> int:
    master_rows = {str(row.get("row_id")): row for row in load_jsonl(TABLE_I_ROWS)}
    dossiers = [parse_dossier(path, master_rows[extract_dossier_id(path)]) for path in discover_docx()]
    dossiers.sort(key=lambda dossier: int(dossier.dossier_id[1:]))
    write_intake_and_coverage_surfaces(dossiers)
    update_atlas(dossiers)
    write_dossier_indexes(dossiers)
    write_graph_workbench(dossiers)
    write_branch_surfaces(dossiers)
    refresh_philosophy_manifest()
    write_readmes()
    print(
        "[ok] planted Table I prepared dossiers: "
        f"{len(dossiers)} dossiers, "
        f"{sum(len(dossier.node_rows) for dossier in dossiers)} proposed nodes, "
        f"{sum(len(dossier.relation_rows) for dossier in dossiers)} proposed relations"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
