#!/usr/bin/env python3
"""Plant the prepared Table I dossier corpus into the ToS philosophy tree.

This script reads the operator-local DOCX corpus and writes only structured
ToS surfaces: atlas indexes, branch bodies, source-anchor backlogs, and
pre-canon graph workbench rows.
"""

from __future__ import annotations

import json
import re
import shutil
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from docx import Document


REPO_ROOT = Path(__file__).resolve().parents[1]
DOC_ROOT = Path("/home/dionysus/Загрузки/ToS")
TABLE_I_ROWS = REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-i/rows.jsonl"
TABLE_I_MANIFEST = REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-i/table.manifest.json"
ATLAS_MANIFEST = REPO_ROOT / "ToS/philosophy/atlas/atlas.manifest.json"
PHILOSOPHY_MANIFEST = REPO_ROOT / "ToS/philosophy/philosophy.manifest.json"
DOSSIER_INDEX = REPO_ROOT / "ToS/philosophy/atlas/dossiers/index.jsonl"
DOSSIER_SUMMARY = REPO_ROOT / "ToS/philosophy/atlas/dossiers/graph-shape-summary.json"
DOSSIER_BRANCH_MANIFEST = REPO_ROOT / "ToS/philosophy/atlas/dossiers/branch.manifest.json"
SOURCE_ANCHOR_BACKLOG = REPO_ROOT / "ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl"
TERM_INDEX = REPO_ROOT / "ToS/philosophy/atlas/dossiers/term-index.jsonl"
TRANSMISSION_BACKLOG = REPO_ROOT / "ToS/philosophy/atlas/dossiers/transmission-backlog.jsonl"
PROPOSED_NODES = REPO_ROOT / "ToS/philosophy/graph-workbench/proposed-nodes/table-i-prepared-dossiers.jsonl"
PROPOSED_RELATIONS = REPO_ROOT / "ToS/philosophy/graph-workbench/proposed-relations/table-i-prepared-dossiers.jsonl"
BRANCH_FRAGMENTS = REPO_ROOT / "ToS/philosophy/graph-workbench/branch-fragments/table-i-prepared-dossier-branches.json"
PROMOTION_LEDGER = REPO_ROOT / "ToS/philosophy/graph-workbench/promotion-ledger/table-i-prepared-dossiers.md"
OBSOLETE_GENERATED_BRANCH = REPO_ROOT / "ToS/philosophy/eras/bronze-age/regions/ancient-near-east"


BRANCHES: dict[str, tuple[str, str]] = {
    "A01": (
        "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/proto-cuneiform-accounting-ontologies",
        "proto-cuneiform accounting, metrology, lexical-list, and tablet rationality branch",
    ),
    "A02": (
        "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/old-babylonian-scribal-wisdom-law",
        "Old Babylonian school literature, wisdom, law, and scribal ethics branch",
    ),
    "A03": (
        "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/first-millennium-mesopotamian-scholarly-commentary",
        "first-millennium Mesopotamian omen, medicine, astrology, and commentary branch",
    ),
    "A04": (
        "ToS/philosophy/eras/bronze-age/regions/north-africa/traditions/egyptian-sebayt-scribal-ethics",
        "Egyptian sebayt, Ma'at, scribal ethics, and instruction branch",
    ),
    "A05": (
        "ToS/philosophy/eras/bronze-age/regions/north-africa/traditions/egyptian-mortuary-theological-dialogic-corpora",
        "Egyptian mortuary, theological, and crisis-dialogic corpus branch",
    ),
    "A06": (
        "ToS/philosophy/eras/bronze-age/regions/north-africa/traditions/late-egyptian-demotic-temple-scholarship",
        "late Egyptian demotic temple scholarship and Hermetic-background branch",
    ),
    "A07": (
        "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/elamite-script-frontiers",
        "proto-Elamite and Linear Elamite script-frontier branch",
    ),
    "A08": (
        "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/elamite-cuneiform-achaemenid-multilingualism",
        "Elamite cuneiform and Achaemenid multilingual inscription branch",
    ),
    "A09": (
        "ToS/philosophy/eras/bronze-age/regions/south-asia/traditions/indus-signs-seals-reconstruction",
        "Indus signs, seals, and reconstruction-limit branch",
    ),
    "A10": (
        "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/hittite-state-ritual-writing",
        "Hittite state, treaty, ritual, and archival writing branch",
    ),
    "A11": (
        "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/ugaritic-alphabetic-levant",
        "Ugaritic and early Northwest Semitic alphabetic Levant branch",
    ),
    "A12": (
        "ToS/philosophy/eras/axial-age/regions/levant/traditions/levantine-wisdom-hebrew-aramaic-documentary",
        "Levantine wisdom, Hebrew-Aramaic documentary, and scribal tradition branch",
    ),
    "A13": (
        "ToS/philosophy/eras/axial-age/regions/levant/traditions/second-temple-judean-scripturalization",
        "Second Temple Judean scripturalization and commentary branch",
    ),
    "A14": (
        "ToS/philosophy/eras/bronze-age/regions/east-asia/traditions/shang-early-zhou-oracle-bronze-writing",
        "Shang and early Zhou oracle-bone, bronze, and royal writing branch",
    ),
    "A15": (
        "ToS/philosophy/eras/axial-age/regions/east-asia/traditions/hundred-schools-warring-states",
        "Spring and Autumn, Warring States, and Hundred Schools branch",
    ),
    "A16": (
        "ToS/philosophy/eras/early-imperial-age/regions/east-asia/traditions/han-sui-imperial-confucianism",
        "Han to Sui imperial Confucian canon, school, and statecraft branch",
    ),
    "A17": (
        "ToS/philosophy/eras/axial-age/regions/south-asia/traditions/vedic-brahmanical-upanishadic",
        "Vedic, Brahmanical, ritual, and early Upanishadic branch",
    ),
    "A18": (
        "ToS/philosophy/eras/axial-age/regions/south-asia/traditions/shramana-early-buddhist-jain",
        "Shramana, early Buddhist, Jain, discipline, and renunciation branch",
    ),
    "A19": (
        "ToS/philosophy/eras/classical-south-asia/regions/south-asia/traditions/brahmanical-shastra-darshana",
        "classical Brahmanical shastra and darshana branch",
    ),
    "A20": (
        "ToS/philosophy/eras/classical-south-asia/regions/south-asia/traditions/indian-buddhist-shastra",
        "classical Indian Buddhist shastra and scholastic branch",
    ),
    "A21": (
        "ToS/philosophy/eras/classical-south-asia/regions/south-asia/traditions/theravada-pali-canon",
        "Theravada and written Pali canon branch",
    ),
    "A22": (
        "ToS/philosophy/eras/axial-age/regions/iranian-world/traditions/avestan-achaemenid-inscriptional",
        "ancient Iranian, Avestan, and Achaemenid inscriptional branch",
    ),
    "A23": (
        "ToS/philosophy/eras/late-antiquity/regions/iranian-world/traditions/sasanian-avesta-pahlavi-scholastic",
        "Sasanian Avesta and Pahlavi scholastic branch",
    ),
    "A35": (
        "ToS/philosophy/eras/late-antiquity/regions/eastern-mediterranean-west-asia/traditions/hermetic-gnostic-mandaean-complexes",
        "Hermetic, Gnostic, and Mandaean late-antique written complex branch",
    ),
    "A36": (
        "ToS/philosophy/eras/late-antiquity/regions/west-asia-central-asia/traditions/manichaean-multilingual-corpus",
        "Mani, Manichaeism, and the multilingual Manichaean corpus branch",
    ),
    "A37": (
        "ToS/philosophy/eras/late-antiquity/regions/north-africa/traditions/coptic-egypt",
        "Coptic Egyptian late-antique monastic, theological, and textual branch",
    ),
    "A40": (
        "ToS/philosophy/eras/late-antiquity/regions/arabian-peninsula/traditions/south-north-arabian-epigraphy",
        "South Arabian, Himyarite, and North Arabian epigraphic branch",
    ),
    "A41": (
        "ToS/philosophy/eras/late-antiquity/regions/central-asia/traditions/central-asian-textual-corridor",
        "Central Asian multilingual textual corridor branch",
    ),
    "A42": (
        "ToS/philosophy/eras/late-antiquity/regions/east-asia/traditions/early-chinese-buddhism",
        "early Chinese Buddhist translation, canon, and doctrinal reception branch",
    ),
    "A43": (
        "ToS/philosophy/eras/late-antiquity/regions/southeast-asia/traditions/sanskrit-pali-inscriptions",
        "early Southeast Asian Sanskrit and Pali inscriptional branch",
    ),
}


NODE_TABLE = ("Node ID", "Тип узла", "Название", "Период", "Связи", "Приоритет")
RELATION_TABLE = ("Source node", "Relation", "Target node", "Комментарий", "Уверенность")


@dataclass
class Dossier:
    dossier_id: str
    title: str
    source_document: str
    docx_path: Path
    paragraph_count: int
    table_count: int
    table_row: str
    master_table: str
    node_rows: list[dict[str, Any]]
    relation_rows: list[dict[str, Any]]
    source_rows: list[dict[str, Any]]
    term_rows: list[dict[str, Any]]
    transmission_rows: list[dict[str, Any]]


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
    paths = sorted([*DOC_ROOT.glob("1.1/*.docx"), *DOC_ROOT.glob("1.2/*.docx")])
    ids = [extract_dossier_id(path) for path in paths]
    expected = sorted(BRANCHES)
    if sorted(ids) != expected:
        raise SystemExit(f"Expected prepared dossier ids {expected}, found {sorted(ids)}")
    return paths


def parse_dossier(path: Path) -> Dossier:
    dossier_id = extract_dossier_id(path)
    document = Document(path)
    paragraphs = [scrub(paragraph.text) for paragraph in document.paragraphs if scrub(paragraph.text)]
    title = paragraphs[0] if paragraphs else f"ToS Deep Research: {dossier_id}"
    node_rows: list[dict[str, Any]] = []
    relation_rows: list[dict[str, Any]] = []
    source_rows: list[dict[str, Any]] = []
    term_rows: list[dict[str, Any]] = []
    transmission_rows: list[dict[str, Any]] = []
    table_row = dossier_id
    master_table = "I"

    for table_index, table in enumerate(document.tables, 1):
        header = table_header(table)
        rows = table_body(table)
        if header == ("Поле", "Значение"):
            for cells in rows:
                row = row_dict(header, cells)
                if row.get("Поле") == "ROW_TO_EXPAND":
                    table_row = row.get("Значение") or table_row
                if row.get("Поле") == "Таблица":
                    master_table = row.get("Значение") or master_table
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
        elif header == ("Источник / корпус", "Тип", "Что даёт", "Доступ / где искать", "Надёжность"):
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
        elif header == ("Источник", "Тип", "Зачем нужен", "Ограничения"):
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
        elif header in (
            ("Проблема", "В чём риск", "Как контролировать в ToS", "Какие источники нужны"),
            ("Проблема", "Риск", "Как контролировать в ToS", "Какие источники нужны"),
        ):
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
        elif header == ("Термин", "Язык", "Транслитерация", "Краткое значение", "Роль в ToS"):
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
        elif header == ("Источник / предыдущий узел", "Что передано", "Канал передачи", "Уверенность", "Примечание"):
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
        elif header == ("Следующий узел / эпоха", "Что передаётся", "Канал", "Уверенность", "Что проверить дальше"):
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

    return Dossier(
        dossier_id=dossier_id,
        title=title,
        source_document=path.name,
        docx_path=path,
        paragraph_count=len(paragraphs),
        table_count=len(document.tables),
        table_row=table_row,
        master_table=master_table,
        node_rows=node_rows,
        relation_rows=relation_rows,
        source_rows=source_rows,
        term_rows=term_rows,
        transmission_rows=transmission_rows,
    )


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
    write_json(ATLAS_MANIFEST, atlas_manifest)

    dossier_branch = load_json(DOSSIER_BRANCH_MANIFEST)
    dossier_branch["dossier_count"] = len(dossiers)
    dossier_branch["source_anchor_backlog"] = repo_ref(SOURCE_ANCHOR_BACKLOG)
    dossier_branch["term_index"] = repo_ref(TERM_INDEX)
    dossier_branch["transmission_backlog"] = repo_ref(TRANSMISSION_BACKLOG)
    write_json(DOSSIER_BRANCH_MANIFEST, dossier_branch)


def write_dossier_indexes(dossiers: list[Dossier]) -> None:
    node_counter: Counter[str] = Counter()
    relation_counter: Counter[str] = Counter()
    index_rows: list[dict[str, Any]] = []
    all_sources: list[dict[str, Any]] = []
    all_terms: list[dict[str, Any]] = []
    all_transmissions: list[dict[str, Any]] = []
    for dossier in dossiers:
        node_type_counts = Counter(str(row.get("node_kind") or "unspecified") for row in dossier.node_rows)
        relation_counts = Counter(str(row.get("relation_kind") or "related_to") for row in dossier.relation_rows)
        node_counter.update(node_type_counts)
        relation_counter.update(relation_counts)
        index_rows.append(
            {
                "atlas_status": "prepared_dossier_indexed",
                "branch_path": BRANCHES[dossier.dossier_id][0],
                "dossier_id": dossier.dossier_id,
                "master_table": dossier.master_table,
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


def write_graph_workbench(dossiers: list[Dossier]) -> None:
    nodes, relations = resolve_relation_endpoints(dossiers)
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
            "branches": [
                {
                    "branch_path": BRANCHES[dossier.dossier_id][0],
                    "dossier_id": dossier.dossier_id,
                    "title": dossier.title,
                    "node_row_count": len(dossier.node_rows),
                    "relation_row_count": len(dossier.relation_rows),
                    "source_anchor_count": len(dossier.source_rows),
                    "term_count": len(dossier.term_rows),
                    "transmission_count": len(dossier.transmission_rows),
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
        f"| branch fragments | {len(dossiers)} | era/region/tradition branch bodies |\n\n"
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
        "## Local Surfaces\n\n"
        "| Surface | Role |\n"
        "| --- | --- |\n"
        "| `sources/source-anchor-backlog.jsonl` | future real source witness and edition anchors for this branch |\n"
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
    for path_ref, _role in BRANCHES.values():
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
            "role": existing.get("role") or f"{Path(path_ref).name} philosophy branch",
        }
        write_json(path / "branch.manifest.json", payload)

    for dossier in dossiers:
        path_ref, role = BRANCHES[dossier.dossier_id]
        path = REPO_ROOT / path_ref
        path.mkdir(parents=True, exist_ok=True)
        write_json(
            path / "branch.manifest.json",
            {
                "branch_id": branch_id_for(path_ref),
                "path": path_ref,
                "role": role,
                "atlas_rows": [dossier.dossier_id],
                "prepared_dossiers": [dossier.dossier_id],
                "evidence_status": "prepared_dossier_branch",
                "expected_local_children": ["sources", "graph-workbench"],
                "source_anchor_backlog": f"{path_ref}/sources/source-anchor-backlog.jsonl",
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
                "role": f"source-anchor backlog for {dossier.dossier_id}",
                "anchor_count": len(dossier.source_rows),
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
        }
    )
    manifest["atlas_routes"] = sorted(atlas_routes)
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
        "| `source-anchor-backlog.jsonl` | future source witness, edition, corpus, and risk-control anchors |\n"
        "| `term-index.jsonl` | prepared term rows extracted from dossier terminology tables |\n"
        "| `transmission-backlog.jsonl` | incoming and outgoing transmission rows extracted from dossier tables |\n\n"
        "Branch bodies live under `ToS/philosophy/eras/...`, and pre-canon graph rows live under "
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
        "```\n\n"
        "The atlas is prepared navigation and growth pressure. Branch bodies live in "
        "`ToS/philosophy/eras/...`; pre-canon graph material lives in "
        "`ToS/philosophy/graph-workbench/`; authored canon relation packs live in the canon route.\n",
        encoding="utf-8",
    )


def main() -> int:
    dossiers = [parse_dossier(path) for path in discover_docx()]
    dossiers.sort(key=lambda dossier: int(dossier.dossier_id[1:]))
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
