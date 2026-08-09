#!/usr/bin/env python3
"""Materialize private layer-exact target passage candidates for transfer soil.

The frozen transfer frame contains twenty whole Russian pages and thirty-five
conservative structural routes. This builder reads only the exact local Mysl
PDF, resolves each expected numbered-unit label and its next same-series label
in Poppler's bbox layer, and stores the resulting text-bearing slices under an
explicit ignored local-content root. Tracked outputs contain only structure,
geometry, counts, digests, provenance, and fail-closed authority effects.

The result is exact only inside the named automatic embedded-PDF text/geometry
layer. It is not a diplomatic transcription, accepted Russian, source-to-target
passage alignment, translation evidence, target gold, or transfer eligibility.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = Path("ToS/source-witnesses")
GOLD_ROOT = SOURCE_ROOT / (
    "works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1"
)
PLAN_PATH = GOLD_ROOT / "transfer-samples.json"
OUTPUT_PATH = GOLD_ROOT / "transfer-target-passage-candidates.v1.json"
ANCHOR_PATH = GOLD_ROOT / "transfer-target-passage-anchors.v1.jsonl"
PROVENANCE_PATH = GOLD_ROOT / "transfer-provenance.jsonl"
LOCAL_CONTENT_ROOT = GOLD_ROOT / "local-content/transfer-target-passages/v1"
SCHEMA_PATH = Path(
    "ToS/contracts/transfer-target-passage-candidate-set.schema.json"
)
SOURCE_ANCHOR_SCHEMA_PATH = Path("ToS/contracts/source-anchor.schema.json")
ITEM_DIR = SOURCE_ROOT / (
    "collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/"
    "editions/moscow-mysl-1996-volume-2/items/operator-pdf"
)
MANIFEST_PATH = ITEM_DIR / "item.manifest.json"
INVENTORY_PATH = ITEM_DIR / "resource-inventory.json"
RIGHTS_PATH = ITEM_DIR / "rights.json"
PDF_RELATIVE_ITEM_PATH = Path("payload/Ницше собрание сочинений.pdf")
PDFTOTEXT_VERSION = "26.01.0"
XHTML_NAMESPACE = "{http://www.w3.org/1999/xhtml}"
EVENT_ID = (
    "tos.event.segmentation.golden-kernel-transfer-target-passages-v1."
    "2026-08-08"
)
EVENT_AT = "2026-08-08T22:05:00-06:00"
SET_ID = (
    "tos.transfer-candidate-set.golden-kernel-transfer-target-passages-v1"
)
SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "transfer-target-passage-candidate-set.schema.json"
)
FOOTER_Y_MIN_POINTS = 500.0
MARKER_X_MIN_POINTS = 140.0
MARKER_X_MAX_POINTS = 180.0
MARKER_OCR_OVERRIDES: dict[tuple[int, str], str] = {
    # The visible printed label is 33; the embedded bbox layer exposes 171.
    (268, "33"): "171",
    # The visible printed label is 46; the embedded bbox layer exposes 171.
    (278, "46"): "171",
    # The visible printed label is 47; the embedded bbox layer exposes 171.
    (279, "47"): "171",
    # The visible printed label is 203; the embedded bbox layer exposes 195.
    (322, "203"): "195",
    # The visible printed label is 285; the embedded bbox layer exposes 263.
    # This exact single-marker correction is inherited from the already frozen
    # source-visible target structure review, not inferred from passage text.
    (399, "285"): "263",
    # The visible printed label is 38; the embedded bbox layer exposes 36.
    (662, "38"): "36",
    # The visible printed label is 45; the embedded bbox layer exposes 36.
    (670, "45"): "36",
}
WORK_SLUGS = {
    "jenseits": "jenseits-von-gut-und-boese",
    "genealogie": "zur-genealogie-der-moral",
    "antichrist": "der-antichrist",
}


JENSEITS_MAP = SOURCE_ROOT / (
    "works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/"
    "ru-polilov-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/"
    "numbered-unit-page-map.json"
)
JENSEITS_CROSSWALK = SOURCE_ROOT / (
    "works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/"
    "structure/naumann-1886-polilov-mysl-1996/"
    "transfer-candidate-page-crosswalk.v1.json"
)
JENSEITS_PAIRINGS = SOURCE_ROOT / (
    "works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/"
    "structure/naumann-1886-polilov-mysl-1996/"
    "numbered-unit-label-correspondence.json"
)
GENE_MAP = SOURCE_ROOT / (
    "works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/"
    "ru-svasyan-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/"
    "hierarchical-numbered-unit-page-map.json"
)
GENE_ROUTES = SOURCE_ROOT / (
    "works/friedrich-nietzsche/zur-genealogie-der-moral/alignments/structure/"
    "naumann-1892-second-svasyan-mysl-1996/"
    "transfer-candidate-source-structural-route.v1.json"
)
ANTI_MAP = SOURCE_ROOT / (
    "works/friedrich-nietzsche/der-antichrist/expressions/"
    "ru-flerova-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/"
    "hierarchical-numbered-unit-page-map.json"
)
ANTI_ROUTES = SOURCE_ROOT / (
    "works/friedrich-nietzsche/der-antichrist/alignments/structure/"
    "naumann-1906-flerova-mysl-1996/"
    "transfer-candidate-source-structural-route.v1.json"
)


class TargetPassageBuildError(RuntimeError):
    """Raised when the exact local target-passage route cannot be reproduced."""


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise TargetPassageBuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise TargetPassageBuildError(f"JSON root is not an object: {path}")
    return payload


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise TargetPassageBuildError(f"cannot read {path}: {exc}") from exc
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as exc:
            raise TargetPassageBuildError(
                f"cannot read {path}:{line_number}: {exc}"
            ) from exc
        if not isinstance(record, dict):
            raise TargetPassageBuildError(f"{path}:{line_number} is not an object")
        records.append(record)
    return records


def _render_json(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _render_jsonl(payloads: list[dict[str, Any]]) -> str:
    return "".join(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"
        for payload in payloads
    )


def _pdftotext_version() -> str:
    result = subprocess.run(
        ["pdftotext", "-v"], check=False, capture_output=True, text=True
    )
    match = re.search(
        r"pdftotext version ([0-9.]+)", f"{result.stdout}\n{result.stderr}"
    )
    if result.returncode != 0 or match is None:
        raise TargetPassageBuildError("pdftotext is unavailable")
    version = match.group(1)
    if version != PDFTOTEXT_VERSION:
        raise TargetPassageBuildError(
            f"pdftotext version drifted: expected {PDFTOTEXT_VERSION}, got {version}"
        )
    return version


def _target_pdf(
    *, repo_root: Path, payload_source_root: Path
) -> tuple[dict[str, Any], dict[str, Any], Path]:
    manifest = _read_json(repo_root / MANIFEST_PATH)
    entries = manifest.get("payload_files", [])
    if len(entries) != 1 or entries[0].get("media_type") != "application/pdf":
        raise TargetPassageBuildError("target Item must bind exactly one PDF")
    entry = entries[0]
    relative_item_dir = ITEM_DIR.relative_to(SOURCE_ROOT)
    pdf_path = payload_source_root / relative_item_dir / entry["relative_path"]
    if entry.get("relative_path") != PDF_RELATIVE_ITEM_PATH.as_posix():
        raise TargetPassageBuildError("target PDF relative path drifted")
    if not pdf_path.is_file():
        raise TargetPassageBuildError(f"target PDF is missing: {pdf_path}")
    if pdf_path.stat().st_size != entry.get("byte_size"):
        raise TargetPassageBuildError("target PDF byte size drifted")
    if _sha256_path(pdf_path) != entry.get("sha256"):
        raise TargetPassageBuildError("target PDF digest drifted")
    return manifest, entry, pdf_path


def _bbox_pages(
    pdf_path: Path, *, start_page: int, end_page: int
) -> dict[int, dict[str, Any]]:
    result = subprocess.run(
        [
            "pdftotext",
            "-f",
            str(start_page),
            "-l",
            str(end_page),
            "-bbox-layout",
            str(pdf_path),
            "-",
        ],
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        diagnostic = result.stderr.decode("utf-8", errors="replace").strip()
        raise TargetPassageBuildError(
            f"pdftotext bbox extraction failed: {diagnostic}"
        )
    try:
        root = ET.fromstring(result.stdout)
    except ET.ParseError as exc:
        raise TargetPassageBuildError(f"invalid bbox XHTML: {exc}") from exc
    page_elements = root.findall(f".//{XHTML_NAMESPACE}page")
    expected = end_page - start_page + 1
    if len(page_elements) != expected:
        raise TargetPassageBuildError(
            f"bbox page count drifted: expected {expected}, got {len(page_elements)}"
        )
    pages: dict[int, dict[str, Any]] = {}
    for page_number, page in enumerate(page_elements, start=start_page):
        lines: list[dict[str, Any]] = []
        for source_order, line in enumerate(page.iter(f"{XHTML_NAMESPACE}line")):
            words = [
                "".join(word.itertext())
                for word in line.findall(f"{XHTML_NAMESPACE}word")
            ]
            if not words:
                continue
            lines.append(
                {
                    "source_order": source_order,
                    "x_min": float(line.get("xMin", "-1")),
                    "y_min": float(line.get("yMin", "-1")),
                    "x_max": float(line.get("xMax", "-1")),
                    "y_max": float(line.get("yMax", "-1")),
                    "words": words,
                    "text": " ".join(words),
                }
            )
        lines.sort(key=lambda row: (row["y_min"], row["x_min"], row["source_order"]))
        pages[page_number] = {
            "page": page_number,
            "width": float(page.get("width", "0")),
            "height": float(page.get("height", "0")),
            "lines": lines,
        }
    return pages


def _normalized_marker(text: str) -> str:
    return text.strip().rstrip(".").strip().translate(
        str.maketrans({"а": "a", "А": "a", "б": "6"})
    )


def _find_marker(
    pages: dict[int, dict[str, Any]], *, page_number: int, unit_key: str
) -> dict[str, Any]:
    page = pages.get(page_number)
    if page is None:
        raise TargetPassageBuildError(f"bbox page {page_number} is unavailable")
    expected_bbox_token = MARKER_OCR_OVERRIDES.get(
        (page_number, unit_key), unit_key
    )
    candidates = [
        line
        for line in page["lines"]
        if len(line["words"]) == 1
        and MARKER_X_MIN_POINTS <= line["x_min"] <= MARKER_X_MAX_POINTS
        and line["y_min"] < FOOTER_Y_MIN_POINTS
        and _normalized_marker(line["text"]) == expected_bbox_token
    ]
    if len(candidates) != 1:
        compact = [
            (line["text"], line["x_min"], line["y_min"])
            for line in page["lines"]
            if len(line["words"]) == 1
            and MARKER_X_MIN_POINTS <= line["x_min"] <= MARKER_X_MAX_POINTS
            and line["y_min"] < FOOTER_Y_MIN_POINTS
        ]
        raise TargetPassageBuildError(
            f"expected one {unit_key!r} marker on page {page_number}, "
            f"got {len(candidates)} from {compact}"
        )
    marker = dict(candidates[0])
    marker["marker_basis"] = (
        "source-visible-exact-label-over-ocr-token"
        if (page_number, unit_key) in MARKER_OCR_OVERRIDES
        else "exact-expected-single-line-number-label-in-pdf-bbox-layer"
    )
    return marker


def _flat_map(map_payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    if isinstance(map_payload.get("unit_starts"), list):
        rows = map_payload["unit_starts"]
        result: dict[str, dict[str, Any]] = {}
        for index, row in enumerate(rows):
            if index + 1 >= len(rows):
                continue
            key = str(row["unit_key"])
            result[key] = {
                "qualified_unit_key": key,
                "series_key": None,
                "unit_key": key,
                "start": row,
                "next": rows[index + 1],
                "passage_ref": row["anchor_ref"].replace("tos.anchor.", "tos.passage.").replace(".pdf-start-page", ""),
            }
        return result
    result = {}
    for series in map_payload.get("series", []):
        series_key = str(series["series_key"])
        rows = series.get("unit_starts", [])
        for index, row in enumerate(rows):
            if index + 1 >= len(rows):
                continue
            key = f"{series_key}:{row['unit_key']}"
            result[key] = {
                "qualified_unit_key": key,
                "series_key": series_key,
                "unit_key": str(row["unit_key"]),
                "start": row,
                "next": rows[index + 1],
                "passage_ref": row["anchor_ref"].replace("tos.anchor.", "tos.passage.").replace(".pdf-start-page", ""),
            }
    return result


def _slug_from_candidate(candidate_id: str) -> str:
    match = re.fullmatch(
        r"tos-target-candidate-([a-z0-9-]+)-p[0-9]{4}-(?:random|hard)",
        candidate_id,
    )
    if match is None:
        raise TargetPassageBuildError(f"invalid frozen candidate id: {candidate_id}")
    short_slug = match.group(1)
    try:
        return WORK_SLUGS[short_slug]
    except KeyError as exc:
        raise TargetPassageBuildError(
            f"unknown frozen-candidate work slug: {short_slug}"
        ) from exc


def _route_rows(repo_root: Path, plan: dict[str, Any]) -> list[dict[str, Any]]:
    plan_candidates = {
        row["unit_id"]: row for row in plan.get("candidate_target_units", [])
    }
    if len(plan_candidates) != 20:
        raise TargetPassageBuildError("frozen transfer plan must contain 20 candidates")

    rows: list[dict[str, Any]] = []
    j_crosswalk = _read_json(repo_root / JENSEITS_CROSSWALK)
    j_pairings = {
        row["unit_key"]: row
        for row in _read_json(repo_root / JENSEITS_PAIRINGS).get("pairings", [])
    }
    for candidate in j_crosswalk.get("candidates", []):
        frozen = plan_candidates.get(candidate["candidate_unit_id"])
        if frozen is None or frozen["page"] != candidate["target_pdf_page"]:
            raise TargetPassageBuildError("Jenseits frozen candidate drifted")
        for unit_key in candidate.get("possible_unit_keys", []):
            pairing = j_pairings.get(unit_key)
            if pairing is None:
                raise TargetPassageBuildError(
                    f"Jenseits pairing is missing for {unit_key}"
                )
            rows.append(
                {
                    "frozen": frozen,
                    "qualified_unit_key": unit_key,
                    "source_anchor_ref": pairing["source_anchor_ref"],
                    "source_start_page": pairing["source_pdf_page"],
                    "route_basis_ref": JENSEITS_CROSSWALK,
                    "map_ref": JENSEITS_MAP,
                }
            )

    for route_path, map_path in ((GENE_ROUTES, GENE_MAP), (ANTI_ROUTES, ANTI_MAP)):
        route_payload = _read_json(repo_root / route_path)
        for candidate in route_payload.get("candidates", []):
            frozen = plan_candidates.get(candidate["candidate_unit_id"])
            if frozen is None or frozen["page"] != candidate["target_pdf_page"]:
                raise TargetPassageBuildError(
                    f"frozen candidate drifted in {route_path}"
                )
            for route in candidate.get("possible_source_structural_routes", []):
                rows.append(
                    {
                        "frozen": frozen,
                        "qualified_unit_key": route["qualified_unit_key"],
                        "source_anchor_ref": route["source_anchor_ref"],
                        "source_start_page": route["source_page"],
                        "route_basis_ref": route_path,
                        "map_ref": map_path,
                    }
                )
    if len(rows) != 35:
        raise TargetPassageBuildError(
            f"expected 35 conservative routes, got {len(rows)}"
        )
    if len({(row["frozen"]["unit_id"], row["qualified_unit_key"]) for row in rows}) != 35:
        raise TargetPassageBuildError("conservative route identities are not unique")
    return rows


def _extract_passage(
    pages: dict[int, dict[str, Any]],
    *,
    start_page: int,
    start_marker: dict[str, Any],
    end_page: int,
    end_marker: dict[str, Any],
) -> tuple[list[dict[str, Any]], str, list[dict[str, Any]]]:
    def line_key(line: dict[str, Any]) -> tuple[float, float, int]:
        return (line["y_min"], line["x_min"], line["source_order"])

    start_key = line_key(start_marker)
    end_key = line_key(end_marker)
    selected: list[dict[str, Any]] = []
    regions: list[dict[str, Any]] = []
    for page_number in range(start_page, end_page + 1):
        page = pages[page_number]
        page_lines = []
        for line in page["lines"]:
            if line["y_min"] >= FOOTER_Y_MIN_POINTS:
                continue
            if page_number == start_page and line_key(line) < start_key:
                continue
            if page_number == end_page and line_key(line) >= end_key:
                continue
            page_lines.append(line)
            selected.append({"page": page_number, **line})
        if page_lines:
            x_min = min(line["x_min"] for line in page_lines)
            y_min = min(line["y_min"] for line in page_lines)
            x_max = max(line["x_max"] for line in page_lines)
            y_max = max(line["y_max"] for line in page_lines)
            width = page["width"]
            height = page["height"]
            regions.append(
                {
                    "type": "page_region",
                    "page": page_number,
                    "x": round(x_min / width, 8),
                    "y": round(y_min / height, 8),
                    "width": round((x_max - x_min) / width, 8),
                    "height": round((y_max - y_min) / height, 8),
                    "coordinate_space": "normalized_0_1",
                }
            )
    if not selected:
        raise TargetPassageBuildError(
            f"empty passage slice from page {start_page} to {end_page}"
        )
    if selected[0]["text"].strip().rstrip(".") != start_marker["text"].strip().rstrip("."):
        raise TargetPassageBuildError(
            "passage does not start at the selected label: "
            f"selected={selected[0]['text']!r} marker={start_marker['text']!r} "
            f"page={start_page} y={start_marker['y_min']}"
        )
    text = "\n".join(line["text"] for line in selected) + "\n"
    return selected, text, regions


def _point_boundary(page_number: int, marker: dict[str, Any]) -> dict[str, Any]:
    return {
        "page": page_number,
        "x_min_points": round(marker["x_min"], 6),
        "y_min_points": round(marker["y_min"], 6),
        "x_max_points": round(marker["x_max"], 6),
        "y_max_points": round(marker["y_max"], 6),
        "marker_basis": marker["marker_basis"],
    }


def _passage_candidate_id(frozen_id: str, qualified_unit_key: str) -> str:
    safe_key = qualified_unit_key.replace(":", "-")
    return frozen_id.replace("tos-target-candidate-", "tos-target-passage-candidate-") + f"-unit-{safe_key}"


def _passage_anchor_id(
    *, work_slug: str, expression_ref: str, qualified_unit_key: str
) -> str:
    expression_key = expression_ref.split(".")[-1]
    safe_key = qualified_unit_key.replace(":", ".")
    return (
        f"tos.anchor.friedrich-nietzsche.{work_slug}.{expression_key}."
        f"unit-{safe_key}.target-passage-candidate-v1"
    )


def _private_payload(
    *,
    passage_candidate_id: str,
    frozen: dict[str, Any],
    qualified_unit_key: str,
    start: dict[str, Any],
    end_exclusive: dict[str, Any],
    selected_lines: list[dict[str, Any]],
    automatic_text: str,
) -> dict[str, Any]:
    return {
        "schema_version": "tos_private_transfer_target_passage_candidate_v1",
        "passage_candidate_id": passage_candidate_id,
        "frozen_page_candidate_id": frozen["unit_id"],
        "frozen_candidate_page": frozen["page"],
        "work_ref": frozen["work_ref"],
        "expression_ref": frozen["expression_ref"],
        "item_ref": frozen["item_ref"],
        "file_ref": frozen["file_ref"],
        "file_sha256": frozen["file_ref"].removeprefix("tos.file.sha256."),
        "qualified_unit_key": qualified_unit_key,
        "source_layer": "embedded-pdf-bbox-pdftotext-automatic-candidate",
        "start": start,
        "end_exclusive": end_exclusive,
        "lines": [
            {
                "page": line["page"],
                "x_min_points": round(line["x_min"], 6),
                "y_min_points": round(line["y_min"], 6),
                "x_max_points": round(line["x_max"], 6),
                "y_max_points": round(line["y_max"], 6),
                "words": line["words"],
                "text": line["text"],
            }
            for line in selected_lines
        ],
        "automatic_candidate_text": automatic_text,
        "authority_boundary": (
            "private automatic bbox-layer slice only; not a diplomatic "
            "transcription, accepted Russian, target gold, alignment, or "
            "publication object"
        ),
    }


def _input_refs(repo_root: Path) -> list[dict[str, Any]]:
    paths_and_roles = [
        (PLAN_PATH, "frozen-golden-kernel-transfer-page-candidate-plan"),
        (MANIFEST_PATH, "exact-local-target-item-manifest"),
        (INVENTORY_PATH, "tracked-target-resource-inventory"),
        (RIGHTS_PATH, "target-rights-posture"),
        (JENSEITS_MAP, "tracked-Jenseits-target-numbered-unit-map"),
        (JENSEITS_CROSSWALK, "tracked-Jenseits-conservative-page-crosswalk"),
        (JENSEITS_PAIRINGS, "tracked-Jenseits-source-route-pairings"),
        (GENE_MAP, "tracked-Genealogie-target-numbered-unit-map"),
        (GENE_ROUTES, "tracked-Genealogie-source-structural-routes"),
        (ANTI_MAP, "tracked-Antichrist-target-numbered-unit-map"),
        (ANTI_ROUTES, "tracked-Antichrist-source-structural-routes"),
        (SCHEMA_PATH, "target-passage-candidate-set-contract"),
        (SOURCE_ANCHOR_SCHEMA_PATH, "source-anchor-contract"),
    ]
    return [
        {"ref": path.as_posix(), "role": role, "sha256": _sha256_path(repo_root / path)}
        for path, role in paths_and_roles
    ]


def build_outputs(
    *,
    repo_root: Path,
    payload_source_root: Path,
) -> tuple[
    dict[str, Any],
    list[dict[str, Any]],
    dict[str, bytes],
    dict[str, Any],
]:
    plan = _read_json(repo_root / PLAN_PATH)
    manifest, pdf_entry, pdf_path = _target_pdf(
        repo_root=repo_root, payload_source_root=payload_source_root
    )
    if plan.get("target_source", {}).get("file_sha256") != pdf_entry["sha256"]:
        raise TargetPassageBuildError("transfer plan target digest drifted")
    if plan.get("target_source", {}).get("rights_record_ref") != RIGHTS_PATH.as_posix():
        raise TargetPassageBuildError("transfer plan rights route drifted")
    version = _pdftotext_version()
    route_rows = _route_rows(repo_root, plan)
    map_payloads = {
        path: _read_json(repo_root / path)
        for path in {row["map_ref"] for row in route_rows}
    }
    flat_maps = {path: _flat_map(payload) for path, payload in map_payloads.items()}

    page_ranges: dict[Path, tuple[int, int]] = {}
    for row in route_rows:
        unit = flat_maps[row["map_ref"]].get(row["qualified_unit_key"])
        if unit is None:
            raise TargetPassageBuildError(
                f"target map misses {row['qualified_unit_key']}"
            )
        start_page = int(unit["start"]["pdf_page"])
        end_page = int(unit["next"]["pdf_page"])
        current = page_ranges.get(row["map_ref"])
        page_ranges[row["map_ref"]] = (
            min(start_page, current[0]) if current else start_page,
            max(end_page, current[1]) if current else end_page,
        )
    bbox_by_map = {
        path: _bbox_pages(pdf_path, start_page=start, end_page=end)
        for path, (start, end) in page_ranges.items()
    }

    candidates: list[dict[str, Any]] = []
    anchors: list[dict[str, Any]] = []
    private_outputs: dict[str, bytes] = {}
    for row in route_rows:
        frozen = row["frozen"]
        unit = flat_maps[row["map_ref"]][row["qualified_unit_key"]]
        start_page = int(unit["start"]["pdf_page"])
        end_page = int(unit["next"]["pdf_page"])
        pages = bbox_by_map[row["map_ref"]]
        start_marker = _find_marker(
            pages, page_number=start_page, unit_key=unit["unit_key"]
        )
        end_marker = _find_marker(
            pages,
            page_number=end_page,
            unit_key=str(unit["next"]["unit_key"]),
        )
        selected_lines, automatic_text, page_regions = _extract_passage(
            pages,
            start_page=start_page,
            start_marker=start_marker,
            end_page=end_page,
            end_marker=end_marker,
        )
        passage_candidate_id = _passage_candidate_id(
            frozen["unit_id"], row["qualified_unit_key"]
        )
        work_slug = _slug_from_candidate(frozen["unit_id"])
        anchor_id = _passage_anchor_id(
            work_slug=work_slug,
            expression_ref=frozen["expression_ref"],
            qualified_unit_key=row["qualified_unit_key"],
        )
        private_ref = LOCAL_CONTENT_ROOT / f"{passage_candidate_id}.json"
        start = _point_boundary(start_page, start_marker)
        end_exclusive = _point_boundary(end_page, end_marker)
        private_payload = _private_payload(
            passage_candidate_id=passage_candidate_id,
            frozen=frozen,
            qualified_unit_key=row["qualified_unit_key"],
            start=start,
            end_exclusive=end_exclusive,
            selected_lines=selected_lines,
            automatic_text=automatic_text,
        )
        private_bytes = _render_json(private_payload).encode("utf-8")
        private_outputs[private_ref.as_posix()] = private_bytes
        candidate_page_line_count = sum(
            line["page"] == frozen["page"] for line in selected_lines
        )
        intersects = candidate_page_line_count > 0
        page_span = sorted({line["page"] for line in selected_lines})
        candidate = {
            "passage_candidate_id": passage_candidate_id,
            "frozen_page_candidate_id": frozen["unit_id"],
            "frozen_candidate_page": frozen["page"],
            "stratum": frozen["stratum"],
            "work_ref": frozen["work_ref"],
            "expression_ref": frozen["expression_ref"],
            "qualified_unit_key": row["qualified_unit_key"],
            "passage_ref": unit["passage_ref"],
            "passage_anchor_ref": anchor_id,
            "prior_start_anchor_ref": unit["start"]["anchor_ref"],
            "source_structural_anchor_ref": row["source_anchor_ref"],
            "source_structural_start_page": row["source_start_page"],
            "route_basis_ref": row["route_basis_ref"].as_posix(),
            "start": start,
            "end_exclusive": end_exclusive,
            "page_span": page_span,
            "candidate_page_line_count": candidate_page_line_count,
            "candidate_page_intersects_passage": intersects,
            "private_content_ref": private_ref.as_posix(),
            "private_content_sha256": _sha256_bytes(private_bytes),
            "private_content_bytes": len(private_bytes),
            "text_character_count": len(automatic_text),
            "line_count": len(selected_lines),
            "word_count": sum(len(line["words"]) for line in selected_lines),
            "source_layer": "embedded-pdf-bbox-pdftotext-automatic-candidate",
            "boundary_posture": "layer-exact-proposed-not-diplomatic",
            "status": (
                "proposed-intersecting-layer-exact"
                if intersects
                else "rejected-nonintersecting-layer-exact"
            ),
            "human_review_performed": False,
            "accepted_target_text": False,
            "eligible_for_variant_execution": False,
            "target_gold_status": "not_started",
            "limitations": [
                "the boundary is exact only inside the automatic embedded-PDF bbox layer",
                "the private slice is not a diplomatic transcription or accepted Russian text",
                "shared numbering and a source structural route do not establish passage or translation alignment",
                "the candidate remains ineligible and has no target gold or human review",
                "private target text is local-only and not authorized for publication",
            ],
        }
        candidates.append(candidate)
        structural_path = [
            f"work:{work_slug}",
            f"expression:{frozen['expression_ref'].split('.')[-1]}",
        ]
        if unit["series_key"] is not None:
            structural_path.append(f"series:{unit['series_key']}")
        structural_path.append(f"numbered-unit:{unit['unit_key']}")
        anchors.append(
            {
                "schema_version": "tos_source_anchor_v1",
                "anchor_id": anchor_id,
                "item_id": manifest["item_id"],
                "file_id": pdf_entry["file_id"],
                "file_sha256": pdf_entry["sha256"],
                "passage_id": unit["passage_ref"],
                "selectors": [
                    {
                        "type": "structural",
                        "path": structural_path,
                        "scheme": "transfer-target-layer-exact-numbered-unit-v1",
                    },
                    *page_regions,
                    {
                        "type": "text_position",
                        "start": 0,
                        "end": len(automatic_text),
                        "text_layer_ref": private_ref.as_posix(),
                    },
                ],
                "selector_method": {
                    "maker_type": "mixed",
                    "method": "expected numbered-unit label to next same-series label in Poppler bbox layer",
                    "version": "1",
                    "configuration_ref": OUTPUT_PATH.as_posix(),
                },
                "status": "proposed" if intersects else "rejected",
                "provenance_event_ref": EVENT_ID,
                "anchor_version": 1,
                "supersedes_anchor_ref": None,
                "review_ref": None,
            }
        )

    intersection_count = sum(
        candidate["candidate_page_intersects_passage"] for candidate in candidates
    )
    input_refs = _input_refs(repo_root)
    output = {
        "$schema": SCHEMA_REF,
        "schema_version": "tos_transfer_target_passage_candidate_set_v1",
        "candidate_set_id": SET_ID,
        "transfer_plan_ref": PLAN_PATH.as_posix(),
        "transfer_plan_sha256": _sha256_path(repo_root / PLAN_PATH),
        "target_witness": {
            "item_ref": manifest["item_id"],
            "file_ref": pdf_entry["file_id"],
            "file_sha256": pdf_entry["sha256"],
            "rights_ref": RIGHTS_PATH.as_posix(),
            "rights_sha256": _sha256_path(repo_root / RIGHTS_PATH),
        },
        "inputs": input_refs,
        "method": {
            "name": "expected-structural-label-to-next-label-bbox-slice",
            "version": "1",
            "maker_type": "mixed",
            "navigation_tool": {
                "name": "pdftotext",
                "version": version,
                "mode": "bbox-layout",
            },
            "selection_law": "include expected numbered-unit label line and following bbox lines until the next same-series expected label, excluding the next label and page footer",
            "local_payloads_read": True,
            "private_content_written": True,
            "human_review_performed": False,
        },
        "passage_candidates": candidates,
        "summary": {
            "frozen_page_candidate_count": 20,
            "conservative_structural_route_count": len(candidates),
            "layer_exact_passage_candidate_count": len(candidates),
            "candidate_page_intersection_count": intersection_count,
            "nonintersecting_route_count": len(candidates) - intersection_count,
            "accepted_target_passage_count": 0,
            "eligible_target_unit_count": 0,
            "target_gold_count": 0,
            "human_review_count": 0,
        },
        "effects": {
            "candidate_frame_changed": False,
            "private_target_text_materialized": True,
            "tracked_target_text_created": False,
            "layer_exact_target_boundary_candidates_created": True,
            "accepted_target_text_created": False,
            "source_passage_boundary_created": False,
            "source_to_target_passage_alignment_created": False,
            "translation_alignment_created": False,
            "target_unit_eligible": False,
            "target_gold_created": False,
            "semantic_work_opened": False,
            "human_work_scheduled": False,
            "canon_effect": False,
        },
        "provenance_event_ref": EVENT_ID,
        "status": "prepared-ineligible",
        "authority_boundary": (
            "private target passage materialization exact only within one "
            "fixity-bound automatic embedded-PDF bbox layer; tracked data is "
            "text-free, boundaries remain proposed or rejected, and no "
            "accepted Russian, source passage, passage or translation "
            "alignment, eligibility, gold, human, semantic, publication, or "
            "canon authority follows"
        ),
        "does_not_establish": [
            "diplomatic_transcription",
            "accepted_russian",
            "source_passage_boundary",
            "source_to_target_passage_alignment",
            "translation_correspondence",
            "translation_equivalence",
            "translation_quality",
            "target_gold",
            "eligible_target_unit",
            "human_review",
            "semantic_relation",
            "rights_clearance",
            "publication_permission",
            "canon_promotion",
        ],
    }
    rendered_output = _render_json(output)
    rendered_anchors = _render_jsonl(anchors)
    event = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": EVENT_ID,
        "event_type": "segmentation",
        "started_at": EVENT_AT,
        "ended_at": EVENT_AT,
        "agent_refs": [
            "model:codex",
            "software:python-standard-library",
            f"software:poppler-{version}",
        ],
        "inputs": input_refs,
        "outputs": [
            {
                "ref": OUTPUT_PATH.as_posix(),
                "role": "tracked-text-free-layer-exact-target-passage-candidate-set",
                "sha256": _sha256_bytes(rendered_output.encode("utf-8")),
            },
            {
                "ref": ANCHOR_PATH.as_posix(),
                "role": "tracked-proposed-or-rejected-layer-exact-target-passage-anchors",
                "sha256": _sha256_bytes(rendered_anchors.encode("utf-8")),
            },
            *[
                {
                    "ref": ref,
                    "role": "gitignored-private-automatic-target-passage-candidate",
                    "sha256": _sha256_bytes(content),
                }
                for ref, content in sorted(private_outputs.items())
            ],
        ],
        "method": {
            "maker_type": "mixed",
            "name": "expected-structural-label-to-next-label-bbox-slice",
            "version": "1",
            "artifact_digest": _sha256_path(
                repo_root / "scripts/build_golden_kernel_transfer_target_passages.py"
            ),
            "runtime": "Python standard library plus pdftotext 26.01.0",
            "device": "abyss-machine",
            "configuration": {
                "frozen_page_candidates": 20,
                "conservative_routes": len(candidates),
                "layer_exact_passage_candidates": len(candidates),
                "candidate_page_intersections": intersection_count,
                "nonintersecting_routes": len(candidates) - intersection_count,
                "private_content_files": len(private_outputs),
                "source_visible_marker_overrides": len(MARKER_OCR_OVERRIDES),
                "tracked_text_created": False,
                "human_review_count": 0,
                "accepted_target_passages": 0,
                "eligible_target_units": 0,
                "target_gold_count": 0,
            },
            "prompt_or_instruction_ref": (
                "ToS/research-packets/foundation-laboratory-2026-07/"
                "GOLDEN_KERNEL_TRANSFER_REPORT.md"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            "bbox-layer exactness is not diplomatic or accepted target text",
            "a conservative page route may be rejected when the bounded unit has no line on that frozen page",
            "shared numbering and source start routes do not establish passage or translation alignment",
            "private text remains ignored, local-only, and unauthorized for publication",
            "no target gold, eligibility, human work, semantics, transfer execution, or canon effect was created",
        ],
        "receipt_refs": [OUTPUT_PATH.as_posix()],
        "rights_basis_ref": RIGHTS_PATH.as_posix(),
        "event_version": 1,
        "supersedes_event_ref": None,
    }
    return output, anchors, private_outputs, event


def _write_or_check(path: Path, expected: bytes, *, check: bool, mode: int | None = None) -> bool:
    if check:
        try:
            if path.read_bytes() != expected:
                return False
            if mode is not None and (path.stat().st_mode & 0o777) != mode:
                return False
            return True
        except OSError:
            return False
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(expected)
    if mode is not None:
        os.chmod(path, mode)
    return True


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        required=True,
        help="explicit source-witness root containing the ignored target PDF",
    )
    parser.add_argument(
        "--local-output-root",
        type=Path,
        required=True,
        help="explicit source-witness root owning ignored private passage files",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)
    repo_root = args.repo_root.resolve()
    payload_root = args.payload_source_root.resolve()
    local_output_root = args.local_output_root.resolve()
    try:
        output, anchors, private_outputs, event = build_outputs(
            repo_root=repo_root,
            payload_source_root=payload_root,
        )
        rendered_output = _render_json(output).encode("utf-8")
        rendered_anchors = _render_jsonl(anchors).encode("utf-8")
        current_events = [
            record
            for record in _read_jsonl(repo_root / PROVENANCE_PATH)
            if record.get("event_id") != EVENT_ID
        ]
        current_events.append(event)
        rendered_events = _render_jsonl(current_events).encode("utf-8")
        checks = [
            _write_or_check(repo_root / OUTPUT_PATH, rendered_output, check=args.check),
            _write_or_check(repo_root / ANCHOR_PATH, rendered_anchors, check=args.check),
            _write_or_check(repo_root / PROVENANCE_PATH, rendered_events, check=args.check),
        ]
        for ref, content in private_outputs.items():
            relative = Path(ref).relative_to(SOURCE_ROOT)
            checks.append(
                _write_or_check(
                    local_output_root / relative,
                    content,
                    check=args.check,
                    mode=0o600,
                )
            )
    except TargetPassageBuildError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1
    if not all(checks):
        print("[error] target passage candidate outputs drifted", file=sys.stderr)
        return 1
    action = "match" if args.check else "written"
    summary = output["summary"]
    print(
        f"[ok] {summary['layer_exact_passage_candidate_count']} private target "
        f"passage candidates {action}; {summary['candidate_page_intersection_count']} "
        f"intersect frozen pages and {summary['nonintersecting_route_count']} do not"
    )
    print(
        "[boundary] 0 accepted target passages, source alignments, eligible "
        "units, target gold, human tasks, semantic effects, or canon effects"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
