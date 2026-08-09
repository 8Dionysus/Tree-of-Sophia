#!/usr/bin/env python3
"""Materialize private German source-passage candidates for transfer soil.

The input frame is the already frozen set of thirty-five structural routes.
This builder returns to exact local source layers and slices one numbered unit
to the next numbered unit only when both boundaries resolve inside one named
automatic layer. Resolvable source strings remain mode 0600 below ignored
local-content. Missing boundaries remain explicit tracked negatives.

No output accepts German, aligns source to target, judges translation,
authorizes publication, opens human work, or makes a transfer unit eligible.
"""

from __future__ import annotations

import argparse
import gzip
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
    "works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1"
)
TARGET_CANDIDATE_PATH = GOLD_ROOT / "transfer-target-passage-candidates.v1.json"
OUTPUT_PATH = GOLD_ROOT / "transfer-source-passage-candidates.v1.json"
ANCHOR_PATH = GOLD_ROOT / "transfer-source-passage-anchors.v1.jsonl"
PROVENANCE_PATH = GOLD_ROOT / "transfer-provenance.jsonl"
LOCAL_CONTENT_ROOT = GOLD_ROOT / "local-content/transfer-source-passages/v1"
SCHEMA_PATH = Path("ToS/contracts/transfer-source-passage-candidate-set.schema.json")
SOURCE_ANCHOR_SCHEMA_PATH = Path("ToS/contracts/source-anchor.schema.json")
BOUNDARY_FORENSIC_RETURN = Path(
    "ToS/research-packets/foundation-laboratory-2026-07/"
    "TRANSFER_SOURCE_BOUNDARY_FORENSIC_RETURN.md"
)
EVENT_ID = "tos.event.segmentation.golden-kernel-transfer-source-passages-v1.2026-08-08"
EVENT_AT = "2026-08-08T23:35:00-06:00"
SET_ID = "tos.transfer-candidate-set.golden-kernel-transfer-source-passages-v1"
SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "transfer-source-passage-candidate-set.schema.json"
)
PDFTOTEXT_VERSION = "26.01.0"
XHTML_NAMESPACE = "{http://www.w3.org/1999/xhtml}"
ABBYY_NAMESPACE = "{http://www.abbyy.com/FineReader_xml/FineReader6-schema-v1.xml}"

JENSEITS_ITEM_DIR = SOURCE_ROOT / (
    "works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/"
    "de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/"
    "internet-archive-google-harvard-scan-pdf"
)
JENSEITS_MAP = JENSEITS_ITEM_DIR / "structure/numbered-unit-page-map.json"
GENE_ITEM_DIR = SOURCE_ROOT / (
    "works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/"
    "de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/"
    "items/wikimedia-commons-unc-scan-pdf"
)
GENE_MAP = GENE_ITEM_DIR / "structure/hierarchical-numbered-unit-page-map.json"
ANTI_ADDRESS_ITEM_DIR = SOURCE_ROOT / (
    "collections/friedrich-nietzsche/"
    "nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/"
    "leipzig-c-g-naumann-1906/items/wikimedia-commons-stanford-scan-djvu"
)
ANTI_NAV_ITEM_DIR = SOURCE_ROOT / (
    "collections/friedrich-nietzsche/"
    "nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/"
    "leipzig-c-g-naumann-1906/items/internet-archive-google-stanford-djvu-xml"
)
ANTI_MAP = ANTI_ADDRESS_ITEM_DIR / "structure/hierarchical-numbered-unit-page-map.json"

JENSEITS_UNRESOLVED: dict[str, tuple[str, ...]] = {}
GENE_UNRESOLVED: dict[str, tuple[str, ...]] = {}
ANTI_UNRESOLVED: dict[str, tuple[str, ...]] = {}
JENSEITS_ABBYY_MARKER_OVERRIDES = {"188": "i8s"}
PDF_VISIBLE_MARKER_RETURNS = {
    ("jenseits", 52, "32"): {
        "image_object_number": 252,
        "image_object_generation": 0,
        "image_width_pixels": 2500,
        "image_height_pixels": 3900,
        "pixel_bbox": (1168, 2485, 77, 47),
        "record_order": 39,
        "line_bbox": (54.96, 311.874983, 249.60096, 317.82565),
    },
    ("genealogie", 40, "11"): {
        "image_object_number": 194,
        "image_object_generation": 0,
        "image_width_pixels": 2636,
        "image_height_pixels": 4283,
        "pixel_bbox": (1289, 2469, 72, 32),
        "record_order": 58,
        "line_bbox": (241.40662, 375.444496, 322.070076, 383.544),
    },
}
ANTI_JP2_MARKER_RETURNS = {
    (240, "8"): {
        "leaf_number": 239,
        "member_path": "nietzscheswerke00nietgoog_jp2/nietzscheswerke00nietgoog_0239.jp2",
        "pixel_bbox": (1839, 2011, 56, 57),
        "record_order": 8,
        "line_bbox": (916.0, 2163.0, 2979.0, 2251.0),
    },
    (241, "9"): {
        "leaf_number": 240,
        "member_path": "nietzscheswerke00nietgoog_jp2/nietzscheswerke00nietgoog_0240.jp2",
        "pixel_bbox": (1979, 2253, 61, 62),
        "record_order": 10,
        "line_bbox": (1071.0, 2393.0, 3129.0, 2478.0),
    },
    (290, "44"): {
        "leaf_number": 289,
        "member_path": "nietzscheswerke00nietgoog_jp2/nietzscheswerke00nietgoog_0289.jp2",
        "pixel_bbox": (1834, 3399, 97, 52),
        "record_order": 20,
        "line_bbox": (949.0, 3532.0, 2995.0, 3620.0),
    },
}


class SourcePassageBuildError(RuntimeError):
    """Raised when exact local source-passage preparation drifts."""


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
        raise SourcePassageBuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise SourcePassageBuildError(f"JSON root is not an object: {path}")
    return payload


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise SourcePassageBuildError(f"cannot read {path}: {exc}") from exc
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as exc:
            raise SourcePassageBuildError(
                f"cannot read {path}:{line_number}: {exc}"
            ) from exc
        if not isinstance(record, dict):
            raise SourcePassageBuildError(f"{path}:{line_number} is not an object")
        records.append(record)
    return records


def _render_json(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _render_jsonl(payloads: list[dict[str, Any]]) -> str:
    return "".join(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"
        for payload in payloads
    )


def _numeric_candidate(value: str) -> str:
    compact = "".join(character for character in value.lower() if character.isalnum())
    return compact.translate(
        str.maketrans({"i": "1", "l": "1", "x": "1", "o": "0", "s": "5", "z": "2"})
    )


def _compact_abbyy(value: str) -> str:
    return "".join(
        character.lower()
        for character in value
        if character.isalnum() or character == "»"
    )


def _pdftotext_version() -> str:
    result = subprocess.run(
        ["pdftotext", "-v"], check=False, capture_output=True, text=True
    )
    match = re.search(
        r"pdftotext version ([0-9.]+)", f"{result.stdout}\n{result.stderr}"
    )
    if result.returncode != 0 or match is None:
        raise SourcePassageBuildError("pdftotext is unavailable")
    if match.group(1) != PDFTOTEXT_VERSION:
        raise SourcePassageBuildError(
            f"pdftotext version drifted: expected {PDFTOTEXT_VERSION}, "
            f"got {match.group(1)}"
        )
    return match.group(1)


def _manifest_entry(
    *, repo_root: Path, payload_root: Path, item_dir: Path, media_type: str
) -> tuple[dict[str, Any], dict[str, Any], Path]:
    manifest = _read_json(repo_root / item_dir / "item.manifest.json")
    entries = [
        entry
        for entry in manifest.get("payload_files", [])
        if entry.get("media_type") == media_type
    ]
    if len(entries) != 1:
        raise SourcePassageBuildError(
            f"{item_dir} must bind exactly one {media_type} payload"
        )
    entry = entries[0]
    payload_path = (
        payload_root / item_dir.relative_to(SOURCE_ROOT) / entry["relative_path"]
    )
    if (
        not payload_path.is_file()
        or payload_path.stat().st_size != entry["byte_size"]
        or _sha256_path(payload_path) != entry["sha256"]
    ):
        raise SourcePassageBuildError(f"payload fixity drifted: {payload_path}")
    return manifest, entry, payload_path


def _witness(
    *, item_ref: str, entry: dict[str, Any], rights_ref: Path
) -> dict[str, Any]:
    return {
        "item_ref": item_ref,
        "file_ref": entry["file_id"],
        "file_sha256": entry["sha256"],
        "rights_ref": rights_ref.as_posix(),
    }


def _flat_map(payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    if isinstance(payload.get("unit_starts"), list):
        rows = payload["unit_starts"]
        return {
            str(row["unit_key"]): {
                "series_key": None,
                "unit_key": str(row["unit_key"]),
                "start": row,
                "next": rows[index + 1],
            }
            for index, row in enumerate(rows[:-1])
        }
    result: dict[str, dict[str, Any]] = {}
    for series in payload.get("series", []):
        rows = series.get("unit_starts", [])
        for index, row in enumerate(rows[:-1]):
            qualified = f"{series['series_key']}:{row['unit_key']}"
            result[qualified] = {
                "series_key": str(series["series_key"]),
                "unit_key": str(row["unit_key"]),
                "start": row,
                "next": rows[index + 1],
            }
    return result


def _pdf_pages(
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
        raise SourcePassageBuildError(
            result.stderr.decode("utf-8", errors="replace").strip()
        )
    try:
        root = ET.fromstring(result.stdout)
    except ET.ParseError as exc:
        raise SourcePassageBuildError(f"invalid PDF bbox XHTML: {exc}") from exc
    elements = root.findall(f".//{XHTML_NAMESPACE}page")
    if len(elements) != end_page - start_page + 1:
        raise SourcePassageBuildError("PDF bbox page count drifted")
    pages: dict[int, dict[str, Any]] = {}
    for page_number, page in enumerate(elements, start=start_page):
        records: list[dict[str, Any]] = []
        for order, line in enumerate(page.iter(f"{XHTML_NAMESPACE}line")):
            words = [
                "".join(word.itertext())
                for word in line.findall(f"{XHTML_NAMESPACE}word")
            ]
            if not words:
                continue
            y_min = float(line.get("yMin", "-1"))
            if not 50 <= y_min < 540:
                continue
            records.append(
                {
                    "page": page_number,
                    "order": order,
                    "x_min": float(line.get("xMin", "-1")),
                    "y_min": y_min,
                    "x_max": float(line.get("xMax", "-1")),
                    "y_max": float(line.get("yMax", "-1")),
                    "text": " ".join(words),
                    "words": words,
                }
            )
        records.sort(
            key=lambda record: (record["y_min"], record["x_min"], record["order"])
        )
        pages[page_number] = {
            "page": page_number,
            "width": float(page.get("width", "0")),
            "height": float(page.get("height", "0")),
            "records": records,
        }
    return pages


def _abbyy_pages(path: Path) -> dict[int, dict[str, Any]]:
    pages: dict[int, dict[str, Any]] = {}
    page_number = 0
    try:
        with gzip.open(path, "rb") as source:
            for _event, page in ET.iterparse(source, events=("end",)):
                if page.tag != f"{ABBYY_NAMESPACE}page":
                    continue
                page_number += 1
                records: list[dict[str, Any]] = []
                for order, paragraph in enumerate(page.iter(f"{ABBYY_NAMESPACE}par")):
                    chars = [
                        char.text or ""
                        for char in paragraph.iter(f"{ABBYY_NAMESPACE}charParams")
                    ]
                    text = "".join(chars).strip()
                    lines = list(paragraph.iter(f"{ABBYY_NAMESPACE}line"))
                    if not text or not lines:
                        continue
                    x_min = min(int(line.get("l", "99999")) for line in lines)
                    y_min = min(int(line.get("t", "99999")) for line in lines)
                    x_max = max(int(line.get("r", "-1")) for line in lines)
                    y_max = max(int(line.get("b", "-1")) for line in lines)
                    if not 400 <= y_min < 3500:
                        continue
                    records.append(
                        {
                            "page": page_number,
                            "order": order,
                            "x_min": x_min,
                            "y_min": y_min,
                            "x_max": x_max,
                            "y_max": y_max,
                            "text": text,
                            "words": text.split(),
                            "compact": _compact_abbyy(text),
                        }
                    )
                records.sort(
                    key=lambda record: (
                        record["y_min"],
                        record["x_min"],
                        record["order"],
                    )
                )
                pages[page_number] = {
                    "page": page_number,
                    "width": float(page.get("width", "0")),
                    "height": float(page.get("height", "0")),
                    "records": records,
                }
                page.clear()
    except (OSError, EOFError, ET.ParseError) as exc:
        raise SourcePassageBuildError(f"ABBYY payload is unreadable: {exc}") from exc
    if page_number != 274:
        raise SourcePassageBuildError(
            f"ABBYY page count drifted: expected 274, got {page_number}"
        )
    return pages


def _djvu_pages(
    path: Path, *, expected_count: int, body_y_min: float
) -> dict[int, dict[str, Any]]:
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError as exc:
        raise SourcePassageBuildError(f"DjVuXML is invalid: {exc}") from exc
    elements = root.findall(".//OBJECT")
    if len(elements) != expected_count:
        raise SourcePassageBuildError(
            f"DjVuXML page count drifted: expected {expected_count}, got {len(elements)}"
        )
    pages: dict[int, dict[str, Any]] = {}
    for page_number, page in enumerate(elements, start=1):
        width = float(page.get("width", "0"))
        height = float(page.get("height", "0"))
        records: list[dict[str, Any]] = []
        for order, line in enumerate(page.findall(".//LINE")):
            words = line.findall("./WORD")
            parsed_words: list[dict[str, Any]] = []
            for word in words:
                coords = [float(part) for part in word.get("coords", "").split(",")]
                if len(coords) < 4:
                    continue
                left, bottom, right, top = coords[:4]
                parsed_words.append(
                    {
                        "text": (word.text or "").strip(),
                        "x_min": left,
                        "y_min": top,
                        "x_max": right,
                        "y_max": bottom,
                    }
                )
            parsed_words = [word for word in parsed_words if word["text"]]
            if not parsed_words:
                continue
            y_min = min(word["y_min"] for word in parsed_words)
            if not body_y_min <= y_min < height - 300:
                continue
            records.append(
                {
                    "page": page_number,
                    "order": order,
                    "x_min": min(word["x_min"] for word in parsed_words),
                    "y_min": y_min,
                    "x_max": max(word["x_max"] for word in parsed_words),
                    "y_max": max(word["y_max"] for word in parsed_words),
                    "text": " ".join(word["text"] for word in parsed_words),
                    "words": [word["text"] for word in parsed_words],
                    "word_records": parsed_words,
                }
            )
        pages[page_number] = {
            "page": page_number,
            "width": width,
            "height": height,
            "records": records,
        }
    return pages


def _pdf_marker(
    pages: dict[int, dict[str, Any]], page: int, key: str
) -> dict[str, Any]:
    page_width = pages[page]["width"]
    matches = [
        record
        for record in pages[page]["records"]
        if 0.3 <= record["x_min"] / page_width <= 0.7
        and _numeric_candidate(record["text"]) == key
    ]
    if len(matches) != 1:
        raise SourcePassageBuildError(
            f"expected one PDF marker {key} on page {page}, got {len(matches)}"
        )
    return {
        **matches[0],
        "marker_basis": "exact-normalized-label-in-poppler-pdf-bbox-layer",
    }


def _pdf_visible_marker(
    pages: dict[int, dict[str, Any]],
    *,
    slug: str,
    page: int,
    key: str,
    boundary_role: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    reviewed = PDF_VISIBLE_MARKER_RETURNS.get((slug, page, key))
    if reviewed is None:
        raise SourcePassageBuildError(
            f"missing reviewed PDF marker return for {slug}:{key} on page {page}"
        )
    matches = [
        record
        for record in pages[page]["records"]
        if record["order"] == reviewed["record_order"]
    ]
    if len(matches) != 1:
        raise SourcePassageBuildError(
            f"reviewed PDF following line disappeared for {slug}:{key} on page {page}"
        )
    marker = matches[0]
    observed_line_bbox = (
        marker["x_min"],
        marker["y_min"],
        marker["x_max"],
        marker["y_max"],
    )
    if observed_line_bbox != reviewed["line_bbox"]:
        raise SourcePassageBuildError(
            f"reviewed PDF following-line geometry drifted for {slug}:{key} "
            f"on page {page}"
        )
    x, y, width, height = reviewed["pixel_bbox"]
    image_width = reviewed["image_width_pixels"]
    image_height = reviewed["image_height_pixels"]
    evidence = {
        "boundary_role": boundary_role,
        "expected_unit_key": key,
        "pdf_page": page,
        "pdf_resource_id": f"pdf-page-{page:04d}",
        "image_object_number": reviewed["image_object_number"],
        "image_object_generation": reviewed["image_object_generation"],
        "image_width_pixels": image_width,
        "image_height_pixels": image_height,
        "image_kind": "jbig2-mask",
        "pixel_bbox": {
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "coordinate_space": "top_left_pixels",
        },
        "normalized_bbox": {
            "x": round(x / image_width, 8),
            "y": round(y / image_height, 8),
            "width": round(width / image_width, 8),
            "height": round(height / image_height, 8),
            "coordinate_space": "normalized_0_1",
        },
        "following_poppler_record_order": reviewed["record_order"],
        "following_poppler_line_bbox": {
            "x_min": marker["x_min"],
            "y_min": marker["y_min"],
            "x_max": marker["x_max"],
            "y_max": marker["y_max"],
        },
        "maker_type": "model",
        "human_review_performed": False,
    }
    return {
        **marker,
        "marker_basis": (
            "model-visible-pdf-image-mask-number-marker-plus-first-following-"
            "poppler-pdf-bbox-line"
        ),
    }, evidence


def _abbyy_marker(
    pages: dict[int, dict[str, Any]], page: int, key: str
) -> dict[str, Any]:
    observed = JENSEITS_ABBYY_MARKER_OVERRIDES.get(key, key)
    matches = [
        record for record in pages[page]["records"] if record["compact"] == observed
    ]
    if len(matches) != 1:
        raise SourcePassageBuildError(
            f"expected one ABBYY marker {key}/{observed} on page {page}, "
            f"got {len(matches)}"
        )
    basis = (
        "source-visible-label-over-abbyy-ocr-token"
        if key in JENSEITS_ABBYY_MARKER_OVERRIDES
        else "exact-expected-label-in-abbyy-paragraph-layer"
    )
    return {**matches[0], "marker_basis": basis}


def _djvu_marker(
    pages: dict[int, dict[str, Any]], page: int, key: str
) -> dict[str, Any]:
    matches: list[dict[str, Any]] = []
    width = pages[page]["width"]
    for record in pages[page]["records"]:
        for word in record.get("word_records", []):
            if (
                0.32 <= word["x_min"] / width <= 0.68
                and _numeric_candidate(word["text"]) == key
            ):
                matches.append(record)
                break
    if len(matches) != 1:
        raise SourcePassageBuildError(
            f"expected one DjVuXML marker {key} on page {page}, got {len(matches)}"
        )
    return {
        **matches[0],
        "marker_basis": "exact-normalized-label-in-djvu-xml-word-layer",
    }


def _antichrist_marker(
    pages: dict[int, dict[str, Any]],
    *,
    page: int,
    key: str,
    boundary_role: str,
    jp2_resources: dict[str, dict[str, Any]],
    scandata_resources: dict[str, dict[str, Any]],
) -> tuple[dict[str, Any], dict[str, Any] | None]:
    reviewed = ANTI_JP2_MARKER_RETURNS.get((page, key))
    if reviewed is None:
        return _djvu_marker(pages, page, key), None
    page_payload = pages[page]
    matches = [
        record
        for record in page_payload["records"]
        if record["order"] == reviewed["record_order"]
    ]
    if len(matches) != 1:
        raise SourcePassageBuildError(
            f"reviewed Antichrist line disappeared for {key} on page {page}"
        )
    marker = matches[0]
    observed_line_bbox = (
        marker["x_min"],
        marker["y_min"],
        marker["x_max"],
        marker["y_max"],
    )
    if observed_line_bbox != reviewed["line_bbox"]:
        raise SourcePassageBuildError(
            f"reviewed Antichrist line geometry drifted for {key} on page {page}"
        )
    jp2_resource_id = f"jp2-page-{page:04d}"
    scandata_resource_id = f"scandata-page-{page:04d}"
    jp2_resource = jp2_resources.get(jp2_resource_id)
    scandata_resource = scandata_resources.get(scandata_resource_id)
    expected_locator = {
        "page_index": page,
        "leaf_number": reviewed["leaf_number"],
    }
    if (
        jp2_resource is None
        or any(
            jp2_resource.get("locator", {}).get(field) != value
            for field, value in expected_locator.items()
        )
        or jp2_resource.get("locator", {}).get("member_path") != reviewed["member_path"]
        or scandata_resource is None
        or any(
            scandata_resource.get("locator", {}).get(field) != value
            for field, value in expected_locator.items()
        )
        or scandata_resource.get("locator", {}).get("width_pixels")
        != int(page_payload["width"])
        or scandata_resource.get("locator", {}).get("height_pixels")
        != int(page_payload["height"])
    ):
        raise SourcePassageBuildError(
            f"JP2/scandata/DjVuXML page relation drifted for {key} on page {page}"
        )
    x, y, width, height = reviewed["pixel_bbox"]
    evidence = {
        "boundary_role": boundary_role,
        "expected_unit_key": key,
        "navigation_page": page,
        "leaf_number": reviewed["leaf_number"],
        "jp2_resource_id": jp2_resource_id,
        "scandata_resource_id": scandata_resource_id,
        "member_path": reviewed["member_path"],
        "pixel_bbox": {
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "coordinate_space": "top_left_pixels",
        },
        "normalized_bbox": {
            "x": round(x / page_payload["width"], 8),
            "y": round(y / page_payload["height"], 8),
            "width": round(width / page_payload["width"], 8),
            "height": round(height / page_payload["height"], 8),
            "coordinate_space": "normalized_0_1",
        },
        "following_djvu_xml_record_order": reviewed["record_order"],
        "maker_type": "model",
        "human_review_performed": False,
    }
    return {
        **marker,
        "marker_basis": (
            "model-visible-jp2-number-marker-plus-first-following-djvu-xml-line"
        ),
    }, evidence


def _extract(
    pages: dict[int, dict[str, Any]],
    *,
    start_page: int,
    start_marker: dict[str, Any],
    end_page: int,
    end_marker: dict[str, Any],
) -> tuple[list[dict[str, Any]], str, list[dict[str, Any]]]:
    selected: list[dict[str, Any]] = []
    regions: list[dict[str, Any]] = []
    for page_number in range(start_page, end_page + 1):
        page = pages[page_number]
        records = page["records"]
        page_selected = [
            record
            for record in records
            if not (
                page_number == start_page
                and (record["y_min"], record["x_min"], record["order"])
                < (start_marker["y_min"], start_marker["x_min"], start_marker["order"])
            )
            and not (
                page_number == end_page
                and (record["y_min"], record["x_min"], record["order"])
                >= (end_marker["y_min"], end_marker["x_min"], end_marker["order"])
            )
        ]
        if page_selected:
            selected.extend(page_selected)
            x_min = min(record["x_min"] for record in page_selected)
            y_min = min(record["y_min"] for record in page_selected)
            x_max = max(record["x_max"] for record in page_selected)
            y_max = max(record["y_max"] for record in page_selected)
            regions.append(
                {
                    "type": "page_region",
                    "page": page_number,
                    "x": round(x_min / page["width"], 8),
                    "y": round(y_min / page["height"], 8),
                    "width": round((x_max - x_min) / page["width"], 8),
                    "height": round((y_max - y_min) / page["height"], 8),
                    "coordinate_space": "normalized_0_1",
                }
            )
    if not selected:
        raise SourcePassageBuildError("source passage slice is empty")
    text = "\n".join(record["text"] for record in selected) + "\n"
    return selected, text, regions


def _point(pages: dict[int, dict[str, Any]], marker: dict[str, Any]) -> dict[str, Any]:
    page = pages[marker["page"]]
    return {
        "page": marker["page"],
        "x": round(marker["x_min"] / page["width"], 8),
        "y": round(marker["y_min"] / page["height"], 8),
        "width": round((marker["x_max"] - marker["x_min"]) / page["width"], 8),
        "height": round((marker["y_max"] - marker["y_min"]) / page["height"], 8),
        "coordinate_space": "normalized_0_1",
        "marker_basis": marker["marker_basis"],
    }


def _passage_ref(anchor_ref: str) -> str:
    return (
        anchor_ref.replace("tos.anchor.", "tos.passage.")
        .replace(".pdf-start-page", "")
        .replace(".source-start-page", "")
    )


def _candidate_id(target_id: str) -> str:
    return target_id.replace(
        "tos-target-passage-candidate-", "tos-source-passage-candidate-"
    )


def _anchor_id(passage_ref: str) -> str:
    return (
        passage_ref.replace("tos.passage.", "tos.anchor.")
        + ".source-passage-candidate-v1"
    )


def _input_refs(repo_root: Path) -> list[dict[str, Any]]:
    paths_and_roles = [
        (TARGET_CANDIDATE_PATH, "tracked-target-passage-candidate-frame"),
        (JENSEITS_MAP, "tracked-Jenseits-source-numbered-unit-map"),
        (GENE_MAP, "tracked-Genealogie-source-numbered-unit-map"),
        (ANTI_MAP, "tracked-Antichrist-source-numbered-unit-map"),
        (JENSEITS_ITEM_DIR / "item.manifest.json", "Jenseits-source-item-manifest"),
        (JENSEITS_ITEM_DIR / "resource-inventory.json", "Jenseits-source-inventory"),
        (JENSEITS_ITEM_DIR / "rights.json", "Jenseits-source-rights"),
        (JENSEITS_ITEM_DIR / "forensic-report.md", "Jenseits-baseline-forensic-report"),
        (GENE_ITEM_DIR / "item.manifest.json", "Genealogie-source-item-manifest"),
        (GENE_ITEM_DIR / "resource-inventory.json", "Genealogie-source-inventory"),
        (GENE_ITEM_DIR / "rights.json", "Genealogie-source-rights"),
        (GENE_ITEM_DIR / "forensic-report.md", "Genealogie-baseline-forensic-report"),
        (
            BOUNDARY_FORENSIC_RETURN,
            "bounded-model-visible-PDF-marker-return",
        ),
        (
            ANTI_ADDRESS_ITEM_DIR / "item.manifest.json",
            "Antichrist-address-item-manifest",
        ),
        (
            ANTI_ADDRESS_ITEM_DIR / "resource-inventory.json",
            "Antichrist-address-inventory",
        ),
        (ANTI_ADDRESS_ITEM_DIR / "rights.json", "Antichrist-address-rights"),
        (
            ANTI_NAV_ITEM_DIR / "item.manifest.json",
            "Antichrist-navigation-item-manifest",
        ),
        (
            ANTI_NAV_ITEM_DIR / "resource-inventory.json",
            "Antichrist-navigation-inventory",
        ),
        (ANTI_NAV_ITEM_DIR / "rights.json", "Antichrist-navigation-rights"),
        (ANTI_NAV_ITEM_DIR / "forensic-report.md", "Antichrist-JP2-marker-review"),
        (
            ANTI_NAV_ITEM_DIR / "source-metadata-snapshot.jp2-scandata.2026-08-08.json",
            "Antichrist-JP2-scandata-provider-snapshot",
        ),
        (SCHEMA_PATH, "source-passage-candidate-set-contract"),
        (SOURCE_ANCHOR_SCHEMA_PATH, "source-anchor-contract"),
    ]
    return [
        {"ref": path.as_posix(), "role": role, "sha256": _sha256_path(repo_root / path)}
        for path, role in paths_and_roles
    ]


def _unresolved_record(
    *,
    candidate: dict[str, Any],
    expression_ref: str,
    passage_ref: str,
    address_witness: dict[str, Any],
    relation: str,
    boundaries: tuple[str, ...],
) -> dict[str, Any]:
    return {
        "source_passage_candidate_id": _candidate_id(candidate["passage_candidate_id"]),
        "target_passage_candidate_id": candidate["passage_candidate_id"],
        "frozen_page_candidate_id": candidate["frozen_page_candidate_id"],
        "qualified_unit_key": candidate["qualified_unit_key"],
        "work_ref": candidate["work_ref"],
        "source_expression_ref": expression_ref,
        "source_passage_ref": passage_ref,
        "source_structural_anchor_ref": candidate["source_structural_anchor_ref"],
        "status": "boundary-unresolved",
        "boundary_layer": None,
        "start": None,
        "end_exclusive": None,
        "navigation_page_span": None,
        "address_page_span": None,
        "content_witness": None,
        "address_witness": address_witness,
        "boundary_evidence": None,
        "navigation_relation": relation,
        "source_passage_anchor_ref": None,
        "private_content_ref": None,
        "private_content_sha256": None,
        "private_content_bytes": None,
        "text_character_count": None,
        "record_count": None,
        "word_count": None,
        "unresolved_boundaries": list(boundaries),
        "human_review_performed": False,
        "accepted_source_text": False,
        "source_to_target_alignment_created": False,
        "eligible_for_variant_execution": False,
        "limitations": [
            "one or both numbered-unit markers do not resolve inside one admitted automatic source layer",
            "the source-visible start-page map is insufficient to invent an exact within-page boundary",
            "no private source string is materialized for this unresolved route",
            "the unresolved route creates no German acceptance, alignment, eligibility, gold, semantics, or human task",
        ],
    }


def build_outputs(
    *, repo_root: Path, payload_source_root: Path
) -> tuple[dict[str, Any], list[dict[str, Any]], dict[str, bytes], dict[str, Any]]:
    version = _pdftotext_version()
    target_set = _read_json(repo_root / TARGET_CANDIDATE_PATH)
    target_candidates = target_set.get("passage_candidates", [])
    if len(target_candidates) != 35:
        raise SourcePassageBuildError(
            "target passage candidate frame must contain 35 routes"
        )

    j_map_payload = _read_json(repo_root / JENSEITS_MAP)
    g_map_payload = _read_json(repo_root / GENE_MAP)
    a_map_payload = _read_json(repo_root / ANTI_MAP)
    maps = {
        "jenseits": _flat_map(j_map_payload),
        "genealogie": _flat_map(g_map_payload),
        "antichrist": _flat_map(a_map_payload),
    }

    j_manifest, j_abbyy_entry, j_abbyy_path = _manifest_entry(
        repo_root=repo_root,
        payload_root=payload_source_root,
        item_dir=JENSEITS_ITEM_DIR,
        media_type="application/gzip",
    )
    _j_manifest_pdf, j_pdf_entry, j_pdf_path = _manifest_entry(
        repo_root=repo_root,
        payload_root=payload_source_root,
        item_dir=JENSEITS_ITEM_DIR,
        media_type="application/pdf",
    )
    _j_manifest_again, j_djvu_entry, j_djvu_path = _manifest_entry(
        repo_root=repo_root,
        payload_root=payload_source_root,
        item_dir=JENSEITS_ITEM_DIR,
        media_type="application/vnd.djvu+xml",
    )
    g_manifest, g_pdf_entry, g_pdf_path = _manifest_entry(
        repo_root=repo_root,
        payload_root=payload_source_root,
        item_dir=GENE_ITEM_DIR,
        media_type="application/pdf",
    )
    a_nav_manifest, a_xml_entry, a_xml_path = _manifest_entry(
        repo_root=repo_root,
        payload_root=payload_source_root,
        item_dir=ANTI_NAV_ITEM_DIR,
        media_type="application/vnd.djvu+xml",
    )
    _a_nav_manifest_jp2, a_jp2_entry, _a_jp2_path = _manifest_entry(
        repo_root=repo_root,
        payload_root=payload_source_root,
        item_dir=ANTI_NAV_ITEM_DIR,
        media_type="application/zip",
    )
    _a_nav_manifest_scandata, a_scandata_entry, _a_scandata_path = _manifest_entry(
        repo_root=repo_root,
        payload_root=payload_source_root,
        item_dir=ANTI_NAV_ITEM_DIR,
        media_type="application/xml",
    )
    a_address_manifest = _read_json(
        repo_root / ANTI_ADDRESS_ITEM_DIR / "item.manifest.json"
    )
    address_entries = a_address_manifest.get("payload_files", [])
    if len(address_entries) != 1:
        raise SourcePassageBuildError("Antichrist address Item must bind one payload")
    a_address_entry = address_entries[0]

    abbyy_pages = _abbyy_pages(j_abbyy_path)
    j_djvu_pages = _djvu_pages(j_djvu_path, expected_count=274, body_y_min=350)
    j_pdf_pages = {
        **_pdf_pages(j_pdf_path, start_page=52, end_page=54),
        **_pdf_pages(j_pdf_path, start_page=128, end_page=131),
    }
    g_pdf_pages = _pdf_pages(g_pdf_path, start_page=7, end_page=202)
    a_djvu_pages = _djvu_pages(a_xml_path, expected_count=525, body_y_min=950)
    a_inventory = _read_json(repo_root / ANTI_NAV_ITEM_DIR / "resource-inventory.json")
    a_inventory_profiles = {
        file_inventory["profile"]: file_inventory
        for file_inventory in a_inventory.get("files", [])
    }
    if set(a_inventory_profiles) != {
        "djvu_xml_pages_v1",
        "jp2_zip_pages_v1",
        "scandata_pages_v1",
    }:
        raise SourcePassageBuildError(
            "Antichrist navigation inventory profiles drifted"
        )
    a_jp2_resources = {
        resource["resource_id"]: resource
        for resource in a_inventory_profiles["jp2_zip_pages_v1"]["resources"]
    }
    a_scandata_resources = {
        resource["resource_id"]: resource
        for resource in a_inventory_profiles["scandata_pages_v1"]["resources"]
    }

    j_rights = JENSEITS_ITEM_DIR / "rights.json"
    g_rights = GENE_ITEM_DIR / "rights.json"
    a_address_rights = ANTI_ADDRESS_ITEM_DIR / "rights.json"
    a_nav_rights = ANTI_NAV_ITEM_DIR / "rights.json"
    j_address = _witness(
        item_ref=j_manifest["item_id"], entry=j_pdf_entry, rights_ref=j_rights
    )
    g_address = _witness(
        item_ref=g_manifest["item_id"], entry=g_pdf_entry, rights_ref=g_rights
    )
    a_address = _witness(
        item_ref=a_address_manifest["item_id"],
        entry=a_address_entry,
        rights_ref=a_address_rights,
    )

    results: list[dict[str, Any]] = []
    anchors: list[dict[str, Any]] = []
    private_outputs: dict[str, bytes] = {}
    for candidate in target_candidates:
        work_ref = candidate["work_ref"]
        if "jenseits-von-gut-und-boese" in work_ref:
            slug = "jenseits"
            map_payload = j_map_payload
            expression_ref = map_payload["expression_ref"]
            address_witness = j_address
            unresolved = JENSEITS_UNRESOLVED.get(candidate["qualified_unit_key"])
            relation = "same-fixity-bound-Item-navigation-layer"
        elif "zur-genealogie-der-moral" in work_ref:
            slug = "genealogie"
            map_payload = g_map_payload
            expression_ref = map_payload["expression_ref"]
            address_witness = g_address
            unresolved = GENE_UNRESOLVED.get(candidate["qualified_unit_key"])
            relation = "same-fixity-bound-file-page-index"
        elif "der-antichrist" in work_ref:
            slug = "antichrist"
            map_payload = a_map_payload
            expression_ref = map_payload["expression_ref"]
            address_witness = a_address
            unresolved = ANTI_UNRESOLVED.get(candidate["qualified_unit_key"])
            relation = "bounded-source-visible-two-page-offset-only-no-textual-identity"
        else:
            raise SourcePassageBuildError(f"unsupported source work: {work_ref}")

        unit = maps[slug].get(candidate["qualified_unit_key"])
        if unit is None:
            raise SourcePassageBuildError(
                f"source map misses {candidate['qualified_unit_key']}"
            )
        passage_ref = _passage_ref(unit["start"]["anchor_ref"])
        if unresolved:
            results.append(
                _unresolved_record(
                    candidate=candidate,
                    expression_ref=expression_ref,
                    passage_ref=passage_ref,
                    address_witness=address_witness,
                    relation=relation,
                    boundaries=unresolved,
                )
            )
            continue

        boundary_evidence = None

        if slug == "jenseits" and candidate["qualified_unit_key"] == "32":
            pages = j_pdf_pages
            start_page = int(unit["start"]["pdf_page"])
            end_page = int(unit["next"]["pdf_page"])
            start_marker, start_evidence = _pdf_visible_marker(
                pages,
                slug=slug,
                page=start_page,
                key=unit["unit_key"],
                boundary_role="start",
            )
            end_marker = _pdf_marker(pages, end_page, str(unit["next"]["unit_key"]))
            layer = "pdf-visible-marker-plus-poppler-pdf-bbox-line"
            content_witness = j_address
            boundary_evidence = {
                "pdf_witness": j_address,
                "relation": (
                    "same-PDF-page-image-mask-to-first-following-poppler-bbox-order"
                ),
                "marker_returns": [start_evidence],
            }
            relation = "same-fixity-bound-file-page-index"
            address_offset = 0
        elif slug == "jenseits" and candidate["qualified_unit_key"] == "201":
            layer = "poppler-pdf-bbox-line"
            pages = j_pdf_pages
            start_page = int(unit["start"]["pdf_page"])
            end_page = int(unit["next"]["pdf_page"])
            start_marker = _pdf_marker(pages, start_page, unit["unit_key"])
            end_marker = _pdf_marker(pages, end_page, str(unit["next"]["unit_key"]))
            content_witness = j_address
            relation = "same-fixity-bound-file-page-index"
            address_offset = 0
        elif slug == "jenseits" and candidate["qualified_unit_key"] == "202":
            layer = "djvu-xml-line"
            pages = j_djvu_pages
            start_page = int(unit["start"]["pdf_page"])
            end_page = int(unit["next"]["pdf_page"])
            start_marker = _djvu_marker(pages, start_page, unit["unit_key"])
            end_marker = _djvu_marker(pages, end_page, str(unit["next"]["unit_key"]))
            content_witness = _witness(
                item_ref=j_manifest["item_id"], entry=j_djvu_entry, rights_ref=j_rights
            )
            address_offset = 0
        elif slug == "jenseits":
            layer = "abbyy-xml-paragraph"
            pages = abbyy_pages
            start_page = int(unit["start"]["pdf_page"])
            end_page = int(unit["next"]["pdf_page"])
            start_marker = _abbyy_marker(pages, start_page, unit["unit_key"])
            end_marker = _abbyy_marker(pages, end_page, str(unit["next"]["unit_key"]))
            content_witness = _witness(
                item_ref=j_manifest["item_id"], entry=j_abbyy_entry, rights_ref=j_rights
            )
            address_offset = 0
        elif slug == "genealogie":
            pages = g_pdf_pages
            start_page = int(unit["start"]["navigation_page"])
            end_page = int(unit["next"]["navigation_page"])
            start_marker = _pdf_marker(pages, start_page, unit["unit_key"])
            if candidate["qualified_unit_key"] == "essay-1:10":
                end_marker, end_evidence = _pdf_visible_marker(
                    pages,
                    slug=slug,
                    page=end_page,
                    key=str(unit["next"]["unit_key"]),
                    boundary_role="end_exclusive",
                )
                layer = "pdf-visible-marker-plus-poppler-pdf-bbox-line"
                boundary_evidence = {
                    "pdf_witness": g_address,
                    "relation": (
                        "same-PDF-page-image-mask-to-first-following-poppler-bbox-order"
                    ),
                    "marker_returns": [end_evidence],
                }
            else:
                end_marker = _pdf_marker(pages, end_page, str(unit["next"]["unit_key"]))
                layer = "poppler-pdf-bbox-line"
            content_witness = g_address
            address_offset = 0
        else:
            pages = a_djvu_pages
            start_page = int(unit["start"]["navigation_page"])
            end_page = int(unit["next"]["navigation_page"])
            start_marker, start_evidence = _antichrist_marker(
                pages,
                page=start_page,
                key=unit["unit_key"],
                boundary_role="start",
                jp2_resources=a_jp2_resources,
                scandata_resources=a_scandata_resources,
            )
            end_marker, end_evidence = _antichrist_marker(
                pages,
                page=end_page,
                key=str(unit["next"]["unit_key"]),
                boundary_role="end_exclusive",
                jp2_resources=a_jp2_resources,
                scandata_resources=a_scandata_resources,
            )
            content_witness = _witness(
                item_ref=a_nav_manifest["item_id"],
                entry=a_xml_entry,
                rights_ref=a_nav_rights,
            )
            marker_returns = [
                evidence
                for evidence in (start_evidence, end_evidence)
                if evidence is not None
            ]
            if marker_returns:
                layer = "jp2-visible-marker-plus-djvu-xml-line"
                boundary_evidence = {
                    "image_witness": _witness(
                        item_ref=a_nav_manifest["item_id"],
                        entry=a_jp2_entry,
                        rights_ref=a_nav_rights,
                    ),
                    "scandata_witness": _witness(
                        item_ref=a_nav_manifest["item_id"],
                        entry=a_scandata_entry,
                        rights_ref=a_nav_rights,
                    ),
                    "relation": (
                        "same-Item-scandata-leaf-to-jp2-member-and-djvu-xml-object-order"
                    ),
                    "marker_returns": marker_returns,
                }
            else:
                layer = "djvu-xml-line"
            address_offset = -2

        selected, automatic_text, regions = _extract(
            pages,
            start_page=start_page,
            start_marker=start_marker,
            end_page=end_page,
            end_marker=end_marker,
        )
        navigation_span = sorted({record["page"] for record in selected})
        address_span = [page + address_offset for page in navigation_span]
        source_candidate_id = _candidate_id(candidate["passage_candidate_id"])
        source_anchor_id = _anchor_id(passage_ref)
        private_ref = LOCAL_CONTENT_ROOT / f"{source_candidate_id}.json"
        private_payload = {
            "schema_version": "tos_private_transfer_source_passage_candidate_v1",
            "source_passage_candidate_id": source_candidate_id,
            "target_passage_candidate_id": candidate["passage_candidate_id"],
            "qualified_unit_key": candidate["qualified_unit_key"],
            "source_expression_ref": expression_ref,
            "content_witness": content_witness,
            "boundary_evidence": boundary_evidence,
            "boundary_layer": layer,
            "start": _point(pages, start_marker),
            "end_exclusive": _point(pages, end_marker),
            "records": [
                {
                    "page": record["page"],
                    "x_min": record["x_min"],
                    "y_min": record["y_min"],
                    "x_max": record["x_max"],
                    "y_max": record["y_max"],
                    "text": record["text"],
                    "words": record["words"],
                }
                for record in selected
            ],
            "automatic_candidate_text": automatic_text,
            "authority_boundary": (
                "private automatic source-layer slice only; not diplomatic or "
                "accepted German, source-to-target alignment, translation "
                "evidence, gold, or publication object"
            ),
        }
        private_bytes = _render_json(private_payload).encode("utf-8")
        private_outputs[private_ref.as_posix()] = private_bytes
        result = {
            "source_passage_candidate_id": source_candidate_id,
            "target_passage_candidate_id": candidate["passage_candidate_id"],
            "frozen_page_candidate_id": candidate["frozen_page_candidate_id"],
            "qualified_unit_key": candidate["qualified_unit_key"],
            "work_ref": work_ref,
            "source_expression_ref": expression_ref,
            "source_passage_ref": passage_ref,
            "source_structural_anchor_ref": candidate["source_structural_anchor_ref"],
            "status": "materialized-layer-exact-candidate",
            "boundary_layer": layer,
            "start": _point(pages, start_marker),
            "end_exclusive": _point(pages, end_marker),
            "navigation_page_span": navigation_span,
            "address_page_span": address_span,
            "content_witness": content_witness,
            "address_witness": address_witness,
            "boundary_evidence": boundary_evidence,
            "navigation_relation": relation,
            "source_passage_anchor_ref": source_anchor_id,
            "private_content_ref": private_ref.as_posix(),
            "private_content_sha256": _sha256_bytes(private_bytes),
            "private_content_bytes": len(private_bytes),
            "text_character_count": len(automatic_text),
            "record_count": len(selected),
            "word_count": sum(len(record["words"]) for record in selected),
            "unresolved_boundaries": [],
            "human_review_performed": False,
            "accepted_source_text": False,
            "source_to_target_alignment_created": False,
            "eligible_for_variant_execution": False,
            "limitations": [
                "the boundary is exact only inside the named automatic or model-visible-marker-supported source layer",
                "the private slice is not diplomatic or accepted German text",
                "same numbering and paired structural starts do not establish passage or translation alignment",
                "the candidate remains ineligible and has no target gold or human review",
                "private source text is local-only and not authorized for publication",
            ],
        }
        results.append(result)
        structural_path = [
            f"expression:{expression_ref.split('.')[-1]}",
        ]
        if unit["series_key"] is not None:
            structural_path.append(f"series:{unit['series_key']}")
        structural_path.append(f"numbered-unit:{unit['unit_key']}")
        anchors.append(
            {
                "schema_version": "tos_source_anchor_v1",
                "anchor_id": source_anchor_id,
                "item_id": content_witness["item_ref"],
                "file_id": content_witness["file_ref"],
                "file_sha256": content_witness["file_sha256"],
                "passage_id": passage_ref,
                "selectors": [
                    {
                        "type": "structural",
                        "path": structural_path,
                        "scheme": "transfer-source-layer-exact-numbered-unit-v1",
                    },
                    *regions,
                    {
                        "type": "text_position",
                        "start": 0,
                        "end": len(automatic_text),
                        "text_layer_ref": private_ref.as_posix(),
                    },
                ],
                "selector_method": {
                    "maker_type": "mixed",
                    "method": "numbered label or source-visible marker to next boundary slice",
                    "version": "3",
                    "configuration_ref": OUTPUT_PATH.as_posix(),
                },
                "status": "proposed",
                "provenance_event_ref": EVENT_ID,
                "anchor_version": 1,
                "supersedes_anchor_ref": None,
                "review_ref": None,
            }
        )

    materialized_count = sum(
        result["status"] == "materialized-layer-exact-candidate" for result in results
    )
    unresolved_count = len(results) - materialized_count
    if materialized_count != 35 or unresolved_count != 0 or len(anchors) != 35:
        raise SourcePassageBuildError(
            f"expected 35 materialized / 0 unresolved / 35 anchors, got "
            f"{materialized_count} / {unresolved_count} / {len(anchors)}"
        )
    input_refs = _input_refs(repo_root)
    output = {
        "$schema": SCHEMA_REF,
        "schema_version": "tos_transfer_source_passage_candidate_set_v1",
        "candidate_set_id": SET_ID,
        "target_passage_candidate_set_ref": TARGET_CANDIDATE_PATH.as_posix(),
        "target_passage_candidate_set_sha256": _sha256_path(
            repo_root / TARGET_CANDIDATE_PATH
        ),
        "inputs": input_refs,
        "method": {
            "name": "numbered-label-or-source-visible-marker-to-next-boundary-slice",
            "version": "3",
            "maker_type": "mixed",
            "layers": [
                "abbyy-xml-paragraph",
                "djvu-xml-line",
                "jp2-visible-marker-plus-djvu-xml-line",
                "pdf-visible-marker-plus-poppler-pdf-bbox-line",
                "poppler-pdf-bbox-line",
            ],
            "local_payloads_read": True,
            "private_content_written": True,
            "human_review_performed": False,
        },
        "passage_candidates": results,
        "summary": {
            "conservative_source_route_count": 35,
            "materialized_source_passage_candidate_count": materialized_count,
            "unresolved_source_boundary_count": unresolved_count,
            "accepted_source_passage_count": 0,
            "source_to_target_alignment_count": 0,
            "eligible_target_unit_count": 0,
            "target_gold_count": 0,
            "human_review_count": 0,
        },
        "effects": {
            "candidate_frame_changed": False,
            "private_source_text_materialized": True,
            "tracked_source_text_created": False,
            "layer_exact_source_boundary_candidates_created": True,
            "accepted_source_text_created": False,
            "source_to_target_passage_alignment_created": False,
            "translation_alignment_created": False,
            "target_unit_eligible": False,
            "target_gold_created": False,
            "semantic_work_opened": False,
            "human_work_scheduled": False,
            "canon_effect": False,
        },
        "provenance_event_ref": EVENT_ID,
        "status": "prepared-complete-ineligible",
        "authority_boundary": (
            "all thirty-five private source-passage candidates are exact only "
            "inside their named automatic or model-visible-marker-supported "
            "layers; tracked data is source-text-free, and no "
            "accepted German, source-to-target alignment, translation, "
            "eligibility, gold, human, semantic, publication, or canon "
            "authority follows"
        ),
        "does_not_establish": [
            "diplomatic_transcription",
            "accepted_german",
            "critical_text",
            "source_to_target_passage_alignment",
            "translation_correspondence",
            "translation_equivalence",
            "translation_quality",
            "eligible_target_unit",
            "target_gold",
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
                "role": "tracked-text-free-source-passage-candidate-set",
                "sha256": _sha256_bytes(rendered_output.encode("utf-8")),
            },
            {
                "ref": ANCHOR_PATH.as_posix(),
                "role": "tracked-proposed-source-passage-candidate-anchors",
                "sha256": _sha256_bytes(rendered_anchors.encode("utf-8")),
            },
            *[
                {
                    "ref": ref,
                    "role": "gitignored-private-automatic-source-passage-candidate",
                    "sha256": _sha256_bytes(content),
                }
                for ref, content in sorted(private_outputs.items())
            ],
        ],
        "method": {
            "maker_type": "mixed",
            "name": "numbered-label-or-source-visible-marker-to-next-boundary-slice",
            "version": "3",
            "artifact_digest": _sha256_path(
                repo_root / "scripts/build_golden_kernel_transfer_source_passages.py"
            ),
            "runtime": "Python standard library plus pdftotext 26.01.0",
            "device": "abyss-machine",
            "configuration": {
                "conservative_source_routes": 35,
                "materialized_source_passage_candidates": materialized_count,
                "unresolved_source_boundaries": unresolved_count,
                "private_content_files": len(private_outputs),
                "source_visible_abbyy_marker_overrides": len(
                    JENSEITS_ABBYY_MARKER_OVERRIDES
                ),
                "model_visible_jp2_marker_returns": len(ANTI_JP2_MARKER_RETURNS),
                "model_visible_pdf_marker_returns": len(PDF_VISIBLE_MARKER_RETURNS),
                "tracked_text_created": False,
                "human_review_count": 0,
                "accepted_source_passages": 0,
                "source_to_target_alignments": 0,
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
            "automatic-layer exactness is not diplomatic or accepted German",
            "three Antichrist number markers use exact JP2 source-visible return plus the first following DjVuXML line and have no human repeat",
            "two PDF number markers use exact embedded image-mask return plus the first following Poppler bbox line and have no human repeat",
            "the Antichrist navigation Item is not asserted textually identical to the address Item",
            "shared numbering does not establish passage or translation alignment",
            "no eligibility, gold, human work, semantics, publication, or canon effect was created",
        ],
        "receipt_refs": [OUTPUT_PATH.as_posix()],
        # Three separately assessed source Items contribute here. Their exact
        # rights records are digest-bound inputs; no one record governs the
        # aggregate candidate set or grants publication authority.
        "rights_basis_ref": None,
        "event_version": 3,
        "supersedes_event_ref": None,
    }
    return output, anchors, private_outputs, event


def _write_or_check(
    path: Path, expected: bytes, *, check: bool, mode: int | None = None
) -> bool:
    if check:
        try:
            return path.read_bytes() == expected and (
                mode is None or (path.stat().st_mode & 0o777) == mode
            )
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
    parser.add_argument("--payload-source-root", type=Path, required=True)
    parser.add_argument("--local-output-root", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)
    repo_root = args.repo_root.resolve()
    payload_root = args.payload_source_root.resolve()
    local_output_root = args.local_output_root.resolve()
    try:
        output, anchors, private_outputs, event = build_outputs(
            repo_root=repo_root, payload_source_root=payload_root
        )
        rendered_output = _render_json(output).encode("utf-8")
        rendered_anchors = _render_jsonl(anchors).encode("utf-8")
        events = _read_jsonl(repo_root / PROVENANCE_PATH)
        event_indexes = [
            index
            for index, record in enumerate(events)
            if record.get("event_id") == EVENT_ID
        ]
        if len(event_indexes) > 1:
            raise SourcePassageBuildError("source provenance event is duplicated")
        if event_indexes:
            events[event_indexes[0]] = event
        else:
            events.append(event)
        checks = [
            _write_or_check(repo_root / OUTPUT_PATH, rendered_output, check=args.check),
            _write_or_check(
                repo_root / ANCHOR_PATH, rendered_anchors, check=args.check
            ),
            _write_or_check(
                repo_root / PROVENANCE_PATH,
                _render_jsonl(events).encode("utf-8"),
                check=args.check,
            ),
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
    except SourcePassageBuildError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1
    if not all(checks):
        print("[error] source passage candidate outputs drifted", file=sys.stderr)
        return 1
    action = "match" if args.check else "written"
    summary = output["summary"]
    print(
        f"[ok] {summary['materialized_source_passage_candidate_count']} private "
        f"source passage candidates {action}; "
        f"{summary['unresolved_source_boundary_count']} boundaries remain unresolved"
    )
    print(
        "[boundary] 0 accepted German passages, source-target alignments, eligible "
        "units, target gold, human tasks, semantic effects, or canon effects"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
