#!/usr/bin/env python3
"""Build the witness-local Antonovsky 1911 PDF technical layout spine.

The exact scan, full Poppler bbox observation, and sequential embedded-text
layer stay below the ignored mode-0600 source boundary. Tracked artifacts are
text-free and retain only identity, locator, geometry, digest, count, rights,
and explicit proposal posture.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import secrets
import shutil
import stat
import statistics
import subprocess
import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
ROUTE_ROOT = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/technical-markup/"
    "antonovsky-1911-pdf-layout-v1"
)
PLAN_REF = f"{ROUTE_ROOT}/plan.v1.json"
SCREENING_PLAN_REF = f"{ROUTE_ROOT}/technical-screening-plan.v1.json"
BUILDER_REF = "scripts/build_antonovsky_1911_technical_markup.py"
SCHEMA_REF = "ToS/contracts/source-text-unit-packet-v1.schema.json"
EVENT_ID = (
    "tos.event.segmentation.zarathustra-antonovsky-1911-"
    "pdf-layout-v1.2026-09-01"
)
SCREENING_EVENT_ID = (
    "tos.event.screening.zarathustra-antonovsky-1911-"
    "pdf-layout-v1.2026-09-01"
)
PANEL_ORDER = {"left": 0, "right": 1}
ID_PREFIXES = {
    "packet_id": "source-text-unit-packet",
    "scheme_id": "text-unit-scheme",
    "segmentation_id": "text-segmentation",
    "projection_id": "text-unit-projection",
}
FORBIDDEN_TRACKED_KEYS = {
    "source_text",
    "text",
    "content",
    "exact_text",
    "ocr_text",
    "heading_text",
    "label_text",
    "translation",
    "lemma",
    "concept",
    "sign",
    "relation",
}


class AntonovskyMarkupError(RuntimeError):
    """Raised when a source, method, identity, or parity gate drifts."""


@dataclass
class BlockObservation:
    page_number: int
    panel: str
    flow_ordinal: int
    block_ordinal: int
    bbox: tuple[float, float, float, float]
    lines: list[str]
    line_bboxes: list[tuple[float, float, float, float]]
    line_word_counts: list[int]
    word_count: int
    median_word_height: float
    gap_before: float = 0.0
    gap_after: float = 0.0
    heading_candidate: bool = False
    region_id: str = ""
    locator: str = ""
    start: int = 0
    end: int = 0

    @property
    def text(self) -> str:
        return "\n".join(self.lines)

    @property
    def line_count(self) -> int:
        return len(self.lines)


@dataclass
class PanelObservation:
    page_number: int
    panel: str
    blocks: list[BlockObservation]
    region_id: str
    locator: str
    start: int = 0
    end: int = 0


@dataclass
class PageObservation:
    page_number: int
    resource_id: str
    width: float
    height: float
    panels: list[PanelObservation]
    locator: str
    flow_count: int
    start: int = 0
    end: int = 0


@dataclass
class DocumentObservation:
    pages: list[PageObservation]
    locator: str = "pdf-document"
    start: int = 0
    end: int = 0
    text_layer: str = ""
    bbox_bytes: bytes = b""
    warning_lines: list[str] = field(default_factory=list)
    pdftotext_path: str = ""
    pdftotext_version: str = ""


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_path(path: Path) -> str:
    return _sha256_bytes(path.read_bytes())


def _json_bytes(payload: Any) -> bytes:
    return (
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def _jsonl_bytes(rows: list[dict[str, Any]]) -> bytes:
    return (
        "".join(
            json.dumps(row, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
            + "\n"
            for row in rows
        )
    ).encode("utf-8")


def _canonical_sha256(payload: Any) -> str:
    return _sha256_bytes(
        json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        .encode("utf-8")
    )


def _resolve(repo_root: Path, ref: str) -> Path:
    path = (repo_root / ref).resolve()
    root = repo_root.resolve()
    try:
        path.relative_to(root)
    except ValueError as exc:
        raise AntonovskyMarkupError(f"path escapes repository: {ref}") from exc
    return path


def _load_json(repo_root: Path, ref: str) -> Any:
    try:
        return json.loads(_resolve(repo_root, ref).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise AntonovskyMarkupError(f"cannot load JSON {ref}: {exc}") from exc


def _binding(repo_root: Path, ref: str) -> dict[str, str]:
    return {"ref": ref, "sha256": _sha256_path(_resolve(repo_root, ref))}


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def _children(element: ET.Element, name: str) -> list[ET.Element]:
    return [child for child in list(element) if _local_name(child.tag) == name]


def _load_plan(repo_root: Path) -> dict[str, Any]:
    plan = _load_json(repo_root, PLAN_REF)
    if plan.get("contract_ref") != SCHEMA_REF:
        raise AntonovskyMarkupError("plan contract reference drifted")
    if plan.get("route_root") != ROUTE_ROOT:
        raise AntonovskyMarkupError("plan route root drifted")
    return plan


def _load_screening_plan(
    repo_root: Path, plan: dict[str, Any]
) -> dict[str, Any]:
    ref = plan.get("technical_screening_plan_ref")
    if ref != SCREENING_PLAN_REF:
        raise AntonovskyMarkupError("technical screening plan reference drifted")
    screening = _load_json(repo_root, SCREENING_PLAN_REF)
    if screening.get("source_plan_ref") != PLAN_REF:
        raise AntonovskyMarkupError("screening source-plan binding drifted")
    if screening.get("source_packet_ref") != plan["outputs"]["packet_ref"]:
        raise AntonovskyMarkupError("screening source-packet binding drifted")
    if (
        screening.get("source_heading_candidates_ref")
        != plan["outputs"]["heading_candidates_ref"]
    ):
        raise AntonovskyMarkupError("screening heading-set binding drifted")
    if screening.get("source_file_sha256") != plan["source_item"]["file_sha256"]:
        raise AntonovskyMarkupError("screening source-file fixity drifted")
    if (
        screening.get("source_bbox_sha256")
        != plan["extraction_policy"]["expected_bbox_sha256"]
    ):
        raise AntonovskyMarkupError("screening bbox fixity drifted")

    heading = screening.get("heading_screening", {})
    expected = heading.get("expected_candidate_count")
    if expected != plan["expected_counts"]["heading_candidates"]:
        raise AntonovskyMarkupError("screening heading count binding drifted")
    ordinals: list[int] = []
    roles: list[str] = []
    for group in heading.get("classification_groups", []):
        role = group.get("technical_role")
        if not isinstance(role, str) or not role:
            raise AntonovskyMarkupError("screening technical role is invalid")
        group_ordinals = group.get("candidate_ordinals", [])
        if not isinstance(group_ordinals, list) or not all(
            isinstance(value, int) for value in group_ordinals
        ):
            raise AntonovskyMarkupError("screening candidate ordinal set is invalid")
        ordinals.extend(group_ordinals)
        roles.extend([role] * len(group_ordinals))
    if sorted(ordinals) != list(range(1, expected + 1)):
        raise AntonovskyMarkupError(
            "screening heading classifications are not exact and exhaustive"
        )
    if len(ordinals) != len(set(ordinals)) or len(roles) != expected:
        raise AntonovskyMarkupError("screening heading classification collision")
    return screening


def _source_pdf(repo_root: Path, plan: dict[str, Any]) -> Path:
    source = plan["source_item"]
    manifest_ref = source["manifest_ref"]
    manifest = _load_json(repo_root, manifest_ref)
    if manifest.get("item_id") != source["item_ref"]:
        raise AntonovskyMarkupError("source item identity drifted")
    if manifest.get("embodiment_ref") != source["edition_ref"]:
        raise AntonovskyMarkupError("source edition binding drifted")
    matches = [
        row
        for row in manifest.get("payload_files", [])
        if row.get("file_id") == source["file_ref"]
    ]
    if len(matches) != 1:
        raise AntonovskyMarkupError("exact source PDF is not unique in manifest")
    entry = matches[0]
    if entry.get("sha256") != source["file_sha256"]:
        raise AntonovskyMarkupError("manifest PDF fixity drifted")
    path = _resolve(repo_root, str(Path(manifest_ref).parent / entry["relative_path"]))
    if not path.is_file():
        raise AntonovskyMarkupError(f"exact local PDF is unavailable: {path}")
    if _sha256_path(path) != source["file_sha256"]:
        raise AntonovskyMarkupError("local PDF digest drifted")
    return path


def _inventory_pages(
    repo_root: Path, plan: dict[str, Any]
) -> list[dict[str, Any]]:
    source = plan["source_item"]
    inventory = _load_json(repo_root, source["resource_inventory_ref"])
    if inventory.get("item_id") != source["item_ref"]:
        raise AntonovskyMarkupError("resource inventory item identity drifted")
    files = [
        row
        for row in inventory.get("files", [])
        if row.get("file_id") == source["file_ref"]
    ]
    if len(files) != 1:
        raise AntonovskyMarkupError("resource inventory PDF entry is not unique")
    if files[0].get("file_sha256") != source["file_sha256"]:
        raise AntonovskyMarkupError("resource inventory PDF fixity drifted")
    pages = files[0].get("resources", [])
    expected = plan["expected_counts"]["pdf_pages"]
    if len(pages) != expected:
        raise AntonovskyMarkupError(
            f"resource inventory page count drifted: {len(pages)} != {expected}"
        )
    expected_ids = [f"pdf-page-{ordinal:04d}" for ordinal in range(1, expected + 1)]
    observed_ids = [row.get("resource_id") for row in pages]
    if observed_ids != expected_ids:
        raise AntonovskyMarkupError("resource inventory page identities drifted")
    return pages


def _pdftotext_binary_and_version(
    plan: dict[str, Any],
) -> tuple[Path, str]:
    resolved = shutil.which("pdftotext")
    if resolved is None:
        raise AntonovskyMarkupError("pdftotext is unavailable")
    binary = Path(resolved).resolve()
    result = subprocess.run(
        [str(binary), "-v"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    output = (result.stdout + result.stderr).decode("utf-8", errors="strict")
    first = output.splitlines()[0] if output.splitlines() else ""
    prefix = "pdftotext version "
    if result.returncode != 0 or not first.startswith(prefix):
        raise AntonovskyMarkupError("cannot establish pdftotext version")
    version = first[len(prefix) :]
    expected = plan["extraction_policy"]["software_version"]
    if version != expected:
        raise AntonovskyMarkupError(
            f"pdftotext version drifted: {version} != {expected}"
        )
    return binary, version


def _extract_bbox(
    source_pdf: Path, plan: dict[str, Any]
) -> tuple[bytes, list[str], Path, str]:
    binary, version = _pdftotext_binary_and_version(plan)
    result = subprocess.run(
        [str(binary), "-bbox-layout", str(source_pdf), "-"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        raise AntonovskyMarkupError(
            f"pdftotext bbox extraction failed with exit code {result.returncode}"
        )
    policy = plan["extraction_policy"]
    bbox_bytes = result.stdout
    if len(bbox_bytes) != policy["expected_bbox_bytes"]:
        raise AntonovskyMarkupError("Poppler bbox byte count drifted")
    if _sha256_bytes(bbox_bytes) != policy["expected_bbox_sha256"]:
        raise AntonovskyMarkupError("Poppler bbox digest drifted")
    warnings = result.stderr.decode("utf-8", errors="strict").splitlines()
    expected_warnings = [policy["expected_warning"]] * policy["expected_warning_count"]
    if warnings != expected_warnings:
        raise AntonovskyMarkupError("Poppler warning surface drifted")
    return bbox_bytes, warnings, binary, version


def _panel_key(page_number: int, panel: str) -> tuple[int, int]:
    return (page_number, PANEL_ORDER[panel])


def _region_for(
    page_number: int, panel: str, plan: dict[str, Any]
) -> str:
    key = _panel_key(page_number, panel)
    matches: list[str] = []
    for region in plan["region_candidates"]:
        start = _panel_key(region["start"]["pdf_page"], region["start"]["panel"])
        end = _panel_key(region["end"]["pdf_page"], region["end"]["panel"])
        if start <= key <= end:
            matches.append(region["region_id"])
    if len(matches) != 1:
        raise AntonovskyMarkupError(
            f"panel region closure failed for page {page_number} {panel}: {matches}"
        )
    return matches[0]


def _heading_candidate(
    block: BlockObservation,
    panel_center: float,
    page_height: float,
    rule: dict[str, Any],
) -> bool:
    x_min, y_min, x_max, y_max = block.bbox
    horizontal_center = (x_min + x_max) / 2.0
    return bool(
        block.word_count > 0
        and block.line_count <= rule["maximum_line_count"]
        and block.word_count <= rule["maximum_word_count"]
        and x_max - x_min <= rule["maximum_width_points"]
        and abs(horizontal_center - panel_center)
        <= rule["panel_center_tolerance_points"]
        and y_min >= rule["minimum_y_points"]
        and y_max <= page_height - rule["bottom_margin_points"]
        and block.gap_before >= rule["minimum_gap_before_points"]
        and block.gap_after >= rule["minimum_gap_after_points"]
        and block.median_word_height
        >= rule["minimum_median_word_height_points"]
    )


def _parse_observation(
    bbox_bytes: bytes,
    warning_lines: list[str],
    binary: Path,
    version: str,
    inventory_pages: list[dict[str, Any]],
    plan: dict[str, Any],
) -> DocumentObservation:
    try:
        root = ET.fromstring(bbox_bytes)
    except ET.ParseError as exc:
        raise AntonovskyMarkupError(f"cannot parse Poppler bbox XML: {exc}") from exc
    page_elements = [element for element in root.iter() if _local_name(element.tag) == "page"]
    if len(page_elements) != len(inventory_pages):
        raise AntonovskyMarkupError("Poppler and inventory page counts differ")

    pages: list[PageObservation] = []
    for page_number, (page_element, inventory_page) in enumerate(
        zip(page_elements, inventory_pages, strict=True), start=1
    ):
        width = float(page_element.attrib["width"])
        height = float(page_element.attrib["height"])
        locator = inventory_page["locator"]
        if (
            abs(width - float(locator["width_points"])) > 1e-6
            or abs(height - float(locator["height_points"])) > 1e-6
        ):
            raise AntonovskyMarkupError(
                f"page geometry drifted for pdf-page-{page_number:04d}"
            )
        grouped: dict[str, list[BlockObservation]] = {"left": [], "right": []}
        flows = _children(page_element, "flow")
        for flow_ordinal, flow in enumerate(flows, start=1):
            for block_ordinal, block_element in enumerate(
                _children(flow, "block"), start=1
            ):
                bbox = tuple(
                    float(block_element.attrib[key])
                    for key in ("xMin", "yMin", "xMax", "yMax")
                )
                lines: list[str] = []
                line_bboxes: list[tuple[float, float, float, float]] = []
                line_word_counts: list[int] = []
                word_heights: list[float] = []
                word_count = 0
                for line_element in _children(block_element, "line"):
                    words = _children(line_element, "word")
                    texts: list[str] = []
                    for word in words:
                        texts.append(word.text or "")
                        word_heights.append(
                            float(word.attrib["yMax"]) - float(word.attrib["yMin"])
                        )
                    word_count += len(words)
                    lines.append(" ".join(texts))
                    line_bboxes.append(
                        tuple(
                            float(line_element.attrib[key])
                            for key in ("xMin", "yMin", "xMax", "yMax")
                        )
                    )
                    line_word_counts.append(len(words))
                panel = "left" if (bbox[0] + bbox[2]) / 2.0 < width / 2.0 else "right"
                block_locator = (
                    f"pdf-page-{page_number:04d}/flow-{flow_ordinal:04d}/"
                    f"block-{block_ordinal:04d}"
                )
                grouped[panel].append(
                    BlockObservation(
                        page_number=page_number,
                        panel=panel,
                        flow_ordinal=flow_ordinal,
                        block_ordinal=block_ordinal,
                        bbox=bbox,
                        lines=lines,
                        line_bboxes=line_bboxes,
                        line_word_counts=line_word_counts,
                        word_count=word_count,
                        median_word_height=(
                            statistics.median(word_heights) if word_heights else 0.0
                        ),
                        region_id=_region_for(page_number, panel, plan),
                        locator=block_locator,
                    )
                )

        panels: list[PanelObservation] = []
        rule = plan["heading_candidate_rule"]
        for panel_name in plan["extraction_policy"]["panel_order"]:
            blocks = grouped[panel_name]
            if not blocks:
                continue
            blocks.sort(
                key=lambda row: (
                    row.bbox[1],
                    row.bbox[0],
                    row.flow_ordinal,
                    row.block_ordinal,
                )
            )
            for index, block in enumerate(blocks):
                previous = blocks[index - 1] if index else None
                following = blocks[index + 1] if index + 1 < len(blocks) else None
                block.gap_before = block.bbox[1] - (
                    previous.bbox[3] if previous else 0.0
                )
                block.gap_after = (
                    following.bbox[1] if following else height
                ) - block.bbox[3]
                center = width * (0.25 if panel_name == "left" else 0.75)
                block.heading_candidate = _heading_candidate(
                    block, center, height, rule
                )
            panels.append(
                PanelObservation(
                    page_number=page_number,
                    panel=panel_name,
                    blocks=blocks,
                    region_id=_region_for(page_number, panel_name, plan),
                    locator=f"pdf-page-{page_number:04d}/panel-{panel_name}",
                )
            )
        pages.append(
            PageObservation(
                page_number=page_number,
                resource_id=inventory_page["resource_id"],
                width=width,
                height=height,
                panels=panels,
                locator=f"pdf-page-{page_number:04d}",
                flow_count=len(flows),
            )
        )

    document = DocumentObservation(
        pages=pages,
        bbox_bytes=bbox_bytes,
        warning_lines=warning_lines,
        pdftotext_path=str(binary),
        pdftotext_version=version,
    )
    _serialize_text_layer(document, plan)
    _validate_observed_counts(document, plan)
    return document


def _serialize_text_layer(
    document: DocumentObservation, plan: dict[str, Any]
) -> None:
    pieces: list[str] = []
    position = 0

    def append(value: str) -> None:
        nonlocal position
        pieces.append(value)
        position += len(value)

    block_separator = "\n\n"
    panel_separator = "\n\n\n"
    page_separator = "\f"
    document.start = 0
    for page_index, page in enumerate(document.pages):
        page.start = position
        for panel_index, panel in enumerate(page.panels):
            panel.start = position
            for block_index, block in enumerate(panel.blocks):
                block.start = position
                append(block.text)
                block.end = position
                if block_index + 1 < len(panel.blocks):
                    append(block_separator)
            panel.end = position
            if panel_index + 1 < len(page.panels):
                append(panel_separator)
        if page_index + 1 < len(document.pages):
            append(page_separator)
        page.end = position
    document.end = position
    document.text_layer = "".join(pieces)
    if len(document.text_layer) != position:
        raise AntonovskyMarkupError("text-layer position accounting drifted")


def _observed_counts(document: DocumentObservation) -> dict[str, Any]:
    blocks = [
        block
        for page in document.pages
        for panel in page.panels
        for block in panel.blocks
    ]
    panels = [panel for page in document.pages for panel in page.panels]
    region_counts: dict[str, dict[str, int]] = {}
    for panel in panels:
        counts = region_counts.setdefault(
            panel.region_id,
            {"panels": 0, "blocks": 0, "lines": 0, "words": 0, "heading_candidates": 0},
        )
        counts["panels"] += 1
        for block in panel.blocks:
            counts["blocks"] += 1
            counts["lines"] += block.line_count
            counts["words"] += block.word_count
            counts["heading_candidates"] += int(block.heading_candidate)
    return {
        "pdf_pages": len(document.pages),
        "observed_panels": len(panels),
        "poppler_flows": sum(page.flow_count for page in document.pages),
        "layout_block_paragraph_candidates": len(blocks),
        "physical_lines_counted_not_unitized": sum(block.line_count for block in blocks),
        "embedded_words_counted_not_unitized": sum(block.word_count for block in blocks),
        "heading_candidates": sum(block.heading_candidate for block in blocks),
        "tracked_units": 1 + len(document.pages) + len(panels) + len(blocks),
        "regions": region_counts,
    }


def _validate_observed_counts(
    document: DocumentObservation, plan: dict[str, Any]
) -> None:
    observed = _observed_counts(document)
    expected = plan["expected_counts"]
    if observed != expected:
        raise AntonovskyMarkupError(
            "technical count surface drifted: "
            + json.dumps({"expected": expected, "observed": observed}, sort_keys=True)
        )


def _all_locators(document: DocumentObservation) -> list[str]:
    locators = [document.locator]
    for page in document.pages:
        locators.append(page.locator)
        for panel in page.panels:
            locators.append(panel.locator)
            locators.extend(block.locator for block in panel.blocks)
    if len(locators) != len(set(locators)):
        raise AntonovskyMarkupError("source locator collision")
    return locators


def _mint(prefix: str) -> str:
    return f"tos.{prefix}.sid-{secrets.token_hex(16)}"


def _issue_identities(
    repo_root: Path,
    plan: dict[str, Any],
    document: DocumentObservation,
) -> dict[str, Any]:
    ref = plan["identity_policy"]["issuance_ref"]
    path = _resolve(repo_root, ref)
    if path.exists():
        raise AntonovskyMarkupError(
            "identity issuance already exists; refusing to remint"
        )
    fixed = {name: _mint(prefix) for name, prefix in ID_PREFIXES.items()}
    units = [
        {
            "source_locator": locator,
            "unit_id": _mint("text-unit"),
            "anchor_ref": _mint("anchor.zarathustra-antonovsky-1911-layout-v1"),
        }
        for locator in _all_locators(document)
    ]
    issuance = {
        "schema_version": "tos_opaque_identity_issuance_v1",
        "issuance_id": "tos.identity-issuance.zarathustra-antonovsky-1911-pdf-layout-v1",
        "issued_at": plan["created_at"],
        "identity_policy": plan["identity_policy"]["policy"],
        "source_locator_is_binding_not_identity": True,
        "automatic_remint_on_locator_drift": False,
        "source_file_ref": plan["source_item"]["file_ref"],
        "source_file_sha256": plan["source_item"]["file_sha256"],
        "bbox_sha256": _sha256_bytes(document.bbox_bytes),
        "configuration_ref": PLAN_REF,
        "fixed_ids": fixed,
        "units": units,
        "source_text_included": False,
        "semantic_promotion": False,
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(_json_bytes(issuance))
    return issuance


def _load_and_validate_issuance(
    repo_root: Path,
    plan: dict[str, Any],
    document: DocumentObservation | None,
) -> dict[str, Any]:
    ref = plan["identity_policy"]["issuance_ref"]
    issuance = _load_json(repo_root, ref)
    if issuance.get("identity_policy") != plan["identity_policy"]["policy"]:
        raise AntonovskyMarkupError("identity issuance policy drifted")
    if issuance.get("source_file_sha256") != plan["source_item"]["file_sha256"]:
        raise AntonovskyMarkupError("identity issuance source fixity drifted")
    if document is not None and issuance.get("bbox_sha256") != _sha256_bytes(
        document.bbox_bytes
    ):
        raise AntonovskyMarkupError("identity issuance bbox binding drifted")
    fixed = issuance.get("fixed_ids", {})
    for name, prefix in ID_PREFIXES.items():
        value = fixed.get(name)
        if not isinstance(value, str) or not value.startswith(f"tos.{prefix}.sid-"):
            raise AntonovskyMarkupError(f"invalid issued fixed identity: {name}")
    rows = issuance.get("units", [])
    locators = [row.get("source_locator") for row in rows]
    unit_ids = [row.get("unit_id") for row in rows]
    anchor_ids = [row.get("anchor_ref") for row in rows]
    if (
        not rows
        or len(locators) != len(set(locators))
        or len(unit_ids) != len(set(unit_ids))
        or len(anchor_ids) != len(set(anchor_ids))
    ):
        raise AntonovskyMarkupError("identity issuance collision or emptiness")
    if document is not None and locators != _all_locators(document):
        raise AntonovskyMarkupError(
            "source locator set/order drifted; reviewed successor issuance required"
        )
    return issuance


def _identity_map(issuance: dict[str, Any]) -> dict[str, dict[str, str]]:
    return {row["source_locator"]: row for row in issuance["units"]}


def _citation_for_block(panel_citation: str, ordinal: int) -> str:
    return f"{panel_citation}.b{ordinal:04d}"


def _source_return_ref(
    plan: dict[str, Any],
    kind: str,
    page: PageObservation | None = None,
    panel: PanelObservation | None = None,
    block: BlockObservation | None = None,
) -> str:
    source = plan["source_item"]
    if kind == "document":
        return source["manifest_ref"]
    if page is None:
        raise AntonovskyMarkupError("page is required for source return")
    if kind == "page":
        return f"{source['resource_inventory_ref']}#{page.resource_id}"
    suffix = f"{page.resource_id}/panel-{panel.panel if panel else block.panel}"
    if block is not None:
        suffix += (
            f"/flow-{block.flow_ordinal:04d}/block-{block.block_ordinal:04d}"
        )
    return f"{source['manifest_ref']}#{suffix}"


def _make_anchor(
    *,
    identity: dict[str, str],
    ordinal: int,
    start: int,
    end: int,
    text_layer: str,
    text_layer_ref: str,
    text_layer_sha256: str,
    anchor_role: str,
    locator_ref: str,
) -> dict[str, Any]:
    return {
        "anchor_ref": identity["anchor_ref"],
        "ordinal": ordinal,
        "text_layer_ref": text_layer_ref,
        "text_layer_sha256": text_layer_sha256,
        "selector": {
            "type": "text_position",
            "start": start,
            "end": end,
            "position_unit": "unicode_code_point",
            "interval": "half_open",
        },
        "exact_sha256": _sha256_bytes(text_layer[start:end].encode("utf-8")),
        "anchor_role": anchor_role,
        "source_return": {"required": True, "locator_ref": locator_ref},
    }


def _make_unit(
    *,
    identity: dict[str, str],
    unit_kind: str,
    parent_ids: list[str],
    child_ids: list[str],
    boundary_posture: str,
    certainty: float,
    reason: str,
) -> dict[str, Any]:
    return {
        "unit_id": identity["unit_id"],
        "unit_version": 1,
        "supersedes_unit_ref": None,
        "identity_policy": (
            "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis"
        ),
        "unit_kind": unit_kind,
        "surface_posture": "source_bearing",
        "continuity": "contiguous",
        "ordered_anchor_refs": [identity["anchor_ref"]],
        "parent_unit_refs": parent_ids,
        "ordered_child_unit_refs": child_ids,
        "boundary_posture": boundary_posture,
        "certainty": {
            "value": certainty,
            "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        },
        "status_reason": reason,
        "source_text_mutated": False,
        "semantic_promotion": False,
    }


def _block_counts(panel: PanelObservation) -> tuple[int, int]:
    return len(panel.blocks), sum(block.heading_candidate for block in panel.blocks)


def _build_views(
    document: DocumentObservation,
    plan: dict[str, Any],
    issuance: dict[str, Any],
) -> tuple[
    list[dict[str, Any]],
    list[dict[str, Any]],
    list[dict[str, Any]],
    list[dict[str, Any]],
    list[dict[str, Any]],
]:
    identities = _identity_map(issuance)
    text_layer_ref = plan["outputs"]["private_text_layer_ref"]
    text_layer_sha256 = _sha256_bytes(document.text_layer.encode("utf-8"))
    anchors: list[dict[str, Any]] = []
    units: list[dict[str, Any]] = []
    citations: list[dict[str, Any]] = []
    headings: list[dict[str, Any]] = []
    ordinal = 0

    document_identity = identities[document.locator]
    page_child_ids = [identities[page.locator]["unit_id"] for page in document.pages]
    ordinal += 1
    document_anchor = _make_anchor(
        identity=document_identity,
        ordinal=ordinal,
        start=document.start,
        end=document.end,
        text_layer=document.text_layer,
        text_layer_ref=text_layer_ref,
        text_layer_sha256=text_layer_sha256,
        anchor_role="scope",
        locator_ref=_source_return_ref(plan, "document"),
    )
    anchors.append(document_anchor)
    units.append(
        _make_unit(
            identity=document_identity,
            unit_kind="document",
            parent_ids=[],
            child_ids=page_child_ids,
            boundary_posture="source_attested",
            certainty=1.0,
            reason="The exact item-level PDF document boundary is source-attested.",
        )
    )
    all_region_ids = [row["region_id"] for row in plan["region_candidates"]]
    citations.append(
        {
            "schema_version": "tos_zarathustra_antonovsky_technical_citation_v1",
            "ordinal": ordinal,
            "unit_id": document_identity["unit_id"],
            "anchor_ref": document_identity["anchor_ref"],
            "unit_kind": "document",
            "structural_role": "pdf_item_scope",
            "source_locator": document.locator,
            "display_citation": "Za-RU-Ant1911",
            "parent_unit_id": None,
            "ordered_child_unit_ids": page_child_ids,
            "region_candidate_ids": all_region_ids,
            "pdf_page": None,
            "panel": None,
            "source_resource_id": None,
            "poppler_flow_ordinal": None,
            "poppler_block_ordinal": None,
            "bbox_points": None,
            "line_count": sum(
                block.line_count
                for page in document.pages
                for panel in page.panels
                for block in panel.blocks
            ),
            "word_count": sum(
                block.word_count
                for page in document.pages
                for panel in page.panels
                for block in panel.blocks
            ),
            "contained_paragraph_candidate_count": sum(
                len(panel.blocks) for page in document.pages for panel in page.panels
            ),
            "contained_heading_candidate_count": sum(
                block.heading_candidate
                for page in document.pages
                for panel in page.panels
                for block in panel.blocks
            ),
            "selector_start": document.start,
            "selector_end": document.end,
            "exact_sha256": document_anchor["exact_sha256"],
            "boundary_posture": "source_attested",
            "source_text_included": False,
            "semantic_promotion": False,
        }
    )

    for page in document.pages:
        page_identity = identities[page.locator]
        panel_child_ids = [
            identities[panel.locator]["unit_id"] for panel in page.panels
        ]
        page_regions = list(dict.fromkeys(panel.region_id for panel in page.panels))
        page_block_count = sum(len(panel.blocks) for panel in page.panels)
        page_heading_count = sum(
            block.heading_candidate for panel in page.panels for block in panel.blocks
        )
        ordinal += 1
        page_anchor = _make_anchor(
            identity=page_identity,
            ordinal=ordinal,
            start=page.start,
            end=page.end,
            text_layer=document.text_layer,
            text_layer_ref=text_layer_ref,
            text_layer_sha256=text_layer_sha256,
            anchor_role="scope",
            locator_ref=_source_return_ref(plan, "page", page=page),
        )
        anchors.append(page_anchor)
        units.append(
            _make_unit(
                identity=page_identity,
                unit_kind="section",
                parent_ids=[document_identity["unit_id"]],
                child_ids=panel_child_ids,
                boundary_posture="source_attested",
                certainty=1.0,
                reason="The exact PDF page boundary is source-attested; printed-page interpretation is separate.",
            )
        )
        page_citation = f"Za-RU-Ant1911.pdf{page.page_number:03d}"
        citations.append(
            {
                "schema_version": "tos_zarathustra_antonovsky_technical_citation_v1",
                "ordinal": ordinal,
                "unit_id": page_identity["unit_id"],
                "anchor_ref": page_identity["anchor_ref"],
                "unit_kind": "section",
                "structural_role": "source_attested_pdf_page",
                "source_locator": page.locator,
                "display_citation": page_citation,
                "parent_unit_id": document_identity["unit_id"],
                "ordered_child_unit_ids": panel_child_ids,
                "region_candidate_ids": page_regions,
                "pdf_page": page.page_number,
                "panel": None,
                "source_resource_id": page.resource_id,
                "poppler_flow_ordinal": None,
                "poppler_block_ordinal": None,
                "bbox_points": [0.0, 0.0, page.width, page.height],
                "line_count": sum(
                    block.line_count for panel in page.panels for block in panel.blocks
                ),
                "word_count": sum(
                    block.word_count for panel in page.panels for block in panel.blocks
                ),
                "contained_paragraph_candidate_count": page_block_count,
                "contained_heading_candidate_count": page_heading_count,
                "selector_start": page.start,
                "selector_end": page.end,
                "exact_sha256": page_anchor["exact_sha256"],
                "boundary_posture": "source_attested",
                "source_text_included": False,
                "semantic_promotion": False,
            }
        )

        for panel in page.panels:
            panel_identity = identities[panel.locator]
            block_child_ids = [
                identities[block.locator]["unit_id"] for block in panel.blocks
            ]
            ordinal += 1
            panel_anchor = _make_anchor(
                identity=panel_identity,
                ordinal=ordinal,
                start=panel.start,
                end=panel.end,
                text_layer=document.text_layer,
                text_layer_ref=text_layer_ref,
                text_layer_sha256=text_layer_sha256,
                anchor_role="scope",
                locator_ref=_source_return_ref(plan, "panel", page=page, panel=panel),
            )
            anchors.append(panel_anchor)
            units.append(
                _make_unit(
                    identity=panel_identity,
                    unit_kind="section",
                    parent_ids=[page_identity["unit_id"]],
                    child_ids=block_child_ids,
                    boundary_posture="method_proposed",
                    certainty=0.85,
                    reason="The two-up page-side panel is assigned by the fixed bbox-center midpoint rule.",
                )
            )
            panel_citation = f"{page_citation}.{'L' if panel.panel == 'left' else 'R'}"
            panel_block_count, panel_heading_count = _block_counts(panel)
            citations.append(
                {
                    "schema_version": "tos_zarathustra_antonovsky_technical_citation_v1",
                    "ordinal": ordinal,
                    "unit_id": panel_identity["unit_id"],
                    "anchor_ref": panel_identity["anchor_ref"],
                    "unit_kind": "section",
                    "structural_role": "two_up_page_side_panel_candidate",
                    "source_locator": panel.locator,
                    "display_citation": panel_citation,
                    "parent_unit_id": page_identity["unit_id"],
                    "ordered_child_unit_ids": block_child_ids,
                    "region_candidate_ids": [panel.region_id],
                    "pdf_page": page.page_number,
                    "panel": panel.panel,
                    "source_resource_id": page.resource_id,
                    "poppler_flow_ordinal": None,
                    "poppler_block_ordinal": None,
                    "bbox_points": [
                        0.0 if panel.panel == "left" else page.width / 2.0,
                        0.0,
                        page.width / 2.0 if panel.panel == "left" else page.width,
                        page.height,
                    ],
                    "line_count": sum(block.line_count for block in panel.blocks),
                    "word_count": sum(block.word_count for block in panel.blocks),
                    "contained_paragraph_candidate_count": panel_block_count,
                    "contained_heading_candidate_count": panel_heading_count,
                    "selector_start": panel.start,
                    "selector_end": panel.end,
                    "exact_sha256": panel_anchor["exact_sha256"],
                    "boundary_posture": "method_proposed",
                    "source_text_included": False,
                    "semantic_promotion": False,
                }
            )

            for block_index, block in enumerate(panel.blocks, start=1):
                block_identity = identities[block.locator]
                ordinal += 1
                block_anchor = _make_anchor(
                    identity=block_identity,
                    ordinal=ordinal,
                    start=block.start,
                    end=block.end,
                    text_layer=document.text_layer,
                    text_layer_ref=text_layer_ref,
                    text_layer_sha256=text_layer_sha256,
                    anchor_role="content",
                    locator_ref=_source_return_ref(plan, "block", page=page, block=block),
                )
                anchors.append(block_anchor)
                units.append(
                    _make_unit(
                        identity=block_identity,
                        unit_kind="paragraph",
                        parent_ids=[panel_identity["unit_id"]],
                        child_ids=[],
                        boundary_posture="method_proposed",
                        certainty=0.55,
                        reason="The unit is one Poppler bbox-layout block counted only as an unreviewed paragraph candidate.",
                    )
                )
                block_citation = _citation_for_block(panel_citation, block_index)
                row = {
                    "schema_version": "tos_zarathustra_antonovsky_technical_citation_v1",
                    "ordinal": ordinal,
                    "unit_id": block_identity["unit_id"],
                    "anchor_ref": block_identity["anchor_ref"],
                    "unit_kind": "paragraph",
                    "structural_role": "layout_block_paragraph_candidate",
                    "source_locator": block.locator,
                    "display_citation": block_citation,
                    "parent_unit_id": panel_identity["unit_id"],
                    "ordered_child_unit_ids": [],
                    "region_candidate_ids": [block.region_id],
                    "pdf_page": page.page_number,
                    "panel": panel.panel,
                    "source_resource_id": page.resource_id,
                    "poppler_flow_ordinal": block.flow_ordinal,
                    "poppler_block_ordinal": block.block_ordinal,
                    "bbox_points": list(block.bbox),
                    "line_count": block.line_count,
                    "word_count": block.word_count,
                    "contained_paragraph_candidate_count": 1,
                    "contained_heading_candidate_count": int(block.heading_candidate),
                    "selector_start": block.start,
                    "selector_end": block.end,
                    "exact_sha256": block_anchor["exact_sha256"],
                    "boundary_posture": "method_proposed",
                    "source_text_included": False,
                    "semantic_promotion": False,
                }
                citations.append(row)
                if block.heading_candidate:
                    headings.append(
                        {
                            "schema_version": "tos_zarathustra_antonovsky_heading_candidate_v1",
                            "candidate_ordinal": len(headings) + 1,
                            "unit_id": block_identity["unit_id"],
                            "anchor_ref": block_identity["anchor_ref"],
                            "source_locator": block.locator,
                            "display_citation": block_citation,
                            "region_candidate_id": block.region_id,
                            "pdf_page": page.page_number,
                            "panel": panel.panel,
                            "source_resource_id": page.resource_id,
                            "bbox_points": list(block.bbox),
                            "line_count": block.line_count,
                            "word_count": block.word_count,
                            "median_word_height_points": round(
                                block.median_word_height, 6
                            ),
                            "gap_before_points": round(block.gap_before, 6),
                            "gap_after_points": round(block.gap_after, 6),
                            "rule_version": plan["heading_candidate_rule"][
                                "rule_version"
                            ],
                            "status": "proposed",
                            "coverage_posture": plan["heading_candidate_rule"][
                                "coverage_posture"
                            ],
                            "source_text_included": False,
                            "accepted_section": False,
                            "semantic_promotion": False,
                        }
                    )

    if ordinal != len(anchors) or ordinal != len(units) or ordinal != len(citations):
        raise AntonovskyMarkupError("unit/anchor/citation ordinal closure drifted")
    regions = _region_rows(document, plan, citations)
    return anchors, units, citations, regions, headings


def _region_rows(
    document: DocumentObservation,
    plan: dict[str, Any],
    citations: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    citation_by_locator = {row["source_locator"]: row for row in citations}
    panels = [panel for page in document.pages for panel in page.panels]
    rows: list[dict[str, Any]] = []
    for ordinal, region in enumerate(plan["region_candidates"], start=1):
        selected = [panel for panel in panels if panel.region_id == region["region_id"]]
        if not selected:
            raise AntonovskyMarkupError(f"empty region candidate: {region['region_id']}")
        blocks = [block for panel in selected for block in panel.blocks]
        first = selected[0]
        last = selected[-1]
        first_citation = citation_by_locator[first.locator]
        last_citation = citation_by_locator[last.locator]
        rows.append(
            {
                "schema_version": "tos_zarathustra_antonovsky_region_candidate_v1",
                "region_ordinal": ordinal,
                "region_candidate_id": region["region_id"],
                "display_label": region["display_label"],
                "declared_start": region["start"],
                "declared_end": region["end"],
                "first_observed_panel_citation": first_citation["display_citation"],
                "first_observed_panel_unit_id": first_citation["unit_id"],
                "last_observed_panel_citation": last_citation["display_citation"],
                "last_observed_panel_unit_id": last_citation["unit_id"],
                "observed_panel_count": len(selected),
                "paragraph_candidate_count": len(blocks),
                "physical_line_count": sum(block.line_count for block in blocks),
                "embedded_word_count": sum(block.word_count for block in blocks),
                "heading_candidate_count": sum(
                    block.heading_candidate for block in blocks
                ),
                "status": "proposed",
                "human_review_performed": False,
                "german_correspondence_created": False,
                "source_text_included": False,
                "semantic_promotion": False,
            }
        )
    return rows


def _heading_role_map(screening: dict[str, Any]) -> dict[int, str]:
    result: dict[int, str] = {}
    for group in screening["heading_screening"]["classification_groups"]:
        for ordinal in group["candidate_ordinals"]:
            if ordinal in result:
                raise AntonovskyMarkupError(
                    f"duplicate screened heading ordinal: {ordinal}"
                )
            result[ordinal] = group["technical_role"]
    return result


def _split_line_ordinals(
    block: BlockObservation, rule: dict[str, Any]
) -> list[int]:
    if block.region_id not in set(rule["scope_region_candidate_ids"]):
        return []
    if not (
        rule["minimum_block_median_word_height_points"]
        <= block.median_word_height
        <= rule["maximum_block_median_word_height_points"]
    ):
        return []
    split_ordinals = [
        line_index + 1
        for line_index, (bbox, word_count) in enumerate(
            zip(block.line_bboxes[1:], block.line_word_counts[1:], strict=True),
            start=1,
        )
        if (
            rule["minimum_indent_from_block_left_points"]
            <= bbox[0] - block.bbox[0]
            <= rule["maximum_indent_from_block_left_points"]
            and word_count >= rule["minimum_words_on_split_line"]
        )
    ]
    verse_guard = rule["verse_guard"]
    if (
        block.line_count >= verse_guard["minimum_block_line_count"]
        and split_ordinals
        and len(split_ordinals) / (block.line_count - 1)
        > verse_guard["maximum_candidate_split_ratio"]
    ):
        return []
    return split_ordinals


def _screening_views(
    document: DocumentObservation,
    plan: dict[str, Any],
    screening: dict[str, Any],
    citations: list[dict[str, Any]],
    headings: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    if screening["private_text_layer_sha256"] != _sha256_bytes(
        document.text_layer.encode("utf-8")
    ):
        raise AntonovskyMarkupError("screening private text-layer fixity drifted")
    citation_by_locator = {row["source_locator"]: row for row in citations}
    role_by_ordinal = _heading_role_map(screening)
    heading_rows: list[dict[str, Any]] = []
    role_counts: dict[str, int] = {}
    for heading in headings:
        ordinal = heading["candidate_ordinal"]
        role = role_by_ordinal[ordinal]
        role_counts[role] = role_counts.get(role, 0) + 1
        heading_rows.append(
            {
                "schema_version": "tos_zarathustra_antonovsky_heading_screening_v1",
                "screening_id": screening["screening_id"],
                "candidate_ordinal": ordinal,
                "unit_id": heading["unit_id"],
                "anchor_ref": heading["anchor_ref"],
                "source_locator": heading["source_locator"],
                "display_citation": heading["display_citation"],
                "region_candidate_id": heading["region_candidate_id"],
                "technical_role": role,
                "screening_outcome": "retained_role_classified",
                "maker_kind": "model_source_visible",
                "source_visible": True,
                "real_human_reviewer": False,
                "packet_review_created": False,
                "accepted_section": False,
                "source_text_included": False,
                "semantic_promotion": False,
            }
        )

    rule = screening["paragraph_split_screening"]
    segment_rows: list[dict[str, Any]] = []
    affected_blocks = 0
    split_boundaries = 0
    excluded_verse_blocks = 0
    excluded_verse_boundaries = 0
    raw_in_scope_blocks = 0
    raw_in_scope_boundaries = 0
    segment_ordinal = 0
    for page in document.pages:
        for panel in page.panels:
            for block in panel.blocks:
                raw_rule = dict(rule)
                raw_rule["verse_guard"] = {
                    **rule["verse_guard"],
                    "minimum_block_line_count": 10**9,
                }
                raw_ordinals = _split_line_ordinals(block, raw_rule)
                if raw_ordinals:
                    raw_in_scope_blocks += 1
                    raw_in_scope_boundaries += len(raw_ordinals)
                split_ordinals = _split_line_ordinals(block, rule)
                if raw_ordinals and not split_ordinals:
                    excluded_verse_blocks += 1
                    excluded_verse_boundaries += len(raw_ordinals)
                if not split_ordinals:
                    continue
                affected_blocks += 1
                split_boundaries += len(split_ordinals)
                citation = citation_by_locator[block.locator]
                line_starts: list[int] = []
                position = block.start
                for line_index, line in enumerate(block.lines):
                    line_starts.append(position)
                    position += len(line)
                    if line_index + 1 < block.line_count:
                        position += 1
                if position != block.end:
                    raise AntonovskyMarkupError(
                        f"screening line offsets drifted: {block.locator}"
                    )
                boundary_line_indexes = [1] + split_ordinals + [block.line_count + 1]
                for within_block, (line_start, next_line_start) in enumerate(
                    zip(boundary_line_indexes, boundary_line_indexes[1:]), start=1
                ):
                    selector_start = line_starts[line_start - 1]
                    selector_end = (
                        block.end
                        if next_line_start == block.line_count + 1
                        else line_starts[next_line_start - 1]
                    )
                    if selector_start >= selector_end:
                        raise AntonovskyMarkupError(
                            f"empty screened paragraph span: {block.locator}"
                        )
                    segment_ordinal += 1
                    segment_rows.append(
                        {
                            "schema_version": "tos_zarathustra_antonovsky_paragraph_segment_overlay_v1",
                            "screening_id": screening["screening_id"],
                            "overlay_segment_ordinal": segment_ordinal,
                            "source_block_unit_id": citation["unit_id"],
                            "source_block_anchor_ref": citation["anchor_ref"],
                            "source_locator": block.locator,
                            "source_block_display_citation": citation["display_citation"],
                            "region_candidate_id": block.region_id,
                            "segment_ordinal_within_source_block": within_block,
                            "line_start_ordinal": line_start,
                            "line_end_ordinal": next_line_start - 1,
                            "selector_start": selector_start,
                            "selector_end": selector_end,
                            "exact_sha256": _sha256_bytes(
                                document.text_layer[
                                    selector_start:selector_end
                                ].encode("utf-8")
                            ),
                            "boundary_before": (
                                "inherited_poppler_block_start"
                                if within_block == 1
                                else "proposed_source_observed_indent"
                            ),
                            "boundary_after": (
                                "inherited_poppler_block_end"
                                if next_line_start == block.line_count + 1
                                else "proposed_source_observed_indent"
                            ),
                            "split_rule_version": rule["rule_version"],
                            "status": "proposed",
                            "creates_new_text_unit_identity": False,
                            "source_block_unit_replaced": False,
                            "human_review_performed": False,
                            "accepted_paragraph": False,
                            "source_text_included": False,
                            "semantic_promotion": False,
                        }
                    )

    expected = {
        "affected_source_block_count": rule["expected_affected_source_block_count"],
        "split_boundary_count": rule["expected_split_boundary_count"],
        "proposed_segment_count": rule["expected_proposed_segment_count"],
    }
    observed = {
        "affected_source_block_count": affected_blocks,
        "split_boundary_count": split_boundaries,
        "proposed_segment_count": len(segment_rows),
    }
    if observed != expected:
        raise AntonovskyMarkupError(
            "technical screening count surface drifted: "
            + json.dumps({"expected": expected, "observed": observed}, sort_keys=True)
        )
    screening_summary = {
        "schema_version": "tos_zarathustra_antonovsky_technical_screening_summary_v1",
        "screening_id": screening["screening_id"],
        "source_packet_ref": screening["source_packet_ref"],
        "source_file_sha256": screening["source_file_sha256"],
        "source_bbox_sha256": screening["source_bbox_sha256"],
        "private_text_layer_sha256": screening["private_text_layer_sha256"],
        "heading_candidate_count": len(heading_rows),
        "heading_technical_role_counts": dict(sorted(role_counts.items())),
        "heading_screening_coverage": "all_enumerated_candidates",
        "full_witness_heading_recall_claimed": False,
        "raw_in_scope_indent_candidate_block_count": raw_in_scope_blocks,
        "raw_in_scope_indent_boundary_count": raw_in_scope_boundaries,
        "verse_guard_excluded_block_count": excluded_verse_blocks,
        "verse_guard_excluded_boundary_count": excluded_verse_boundaries,
        **observed,
        "unaffected_inherited_source_block_count": (
            plan["expected_counts"]["layout_block_paragraph_candidates"]
            - affected_blocks
        ),
        "effective_proposed_paragraph_candidate_count": (
            plan["expected_counts"]["layout_block_paragraph_candidates"]
            + split_boundaries
        ),
        "source_visible_heading_candidate_count": len(heading_rows),
        "source_visible_split_sample_block_count": rule["source_visible_sample"][
            "sample_block_count"
        ],
        "maker_kind": "model_source_visible",
        "status": "proposed",
        "packet_review_created": False,
        "human_review_performed": False,
        "accepted_sections": False,
        "accepted_paragraphs": False,
        "source_block_units_replaced": False,
        "new_text_unit_identities_issued": False,
        "source_target_alignment_created": False,
        "full_alignment_readiness_claimed": False,
        "semantic_fields_materialized": False,
        "source_text_included": False,
    }
    return heading_rows, segment_rows, screening_summary


def _method(plan: dict[str, Any]) -> dict[str, Any]:
    return {
        "maker_kind": "software",
        "agent_ref": "software:tos-antonovsky-1911-technical-markup-builder",
        "method_name": "exact PDF Poppler bbox-layout technical proposal",
        "method_version": "1",
        "software_refs": [BUILDER_REF, "pdftotext:26.01.0"],
        "model_ref": None,
        "configuration_ref": PLAN_REF,
        "locale": "ru",
        "unicode_version": None,
        "unicode_revision": None,
        "tailoring_ref": None,
        "provenance_event_ref": EVENT_ID,
        "made_at": plan["created_at"],
        "output_posture": "method_result_not_source_or_linguistic_truth",
    }


def _packet(
    repo_root: Path,
    plan: dict[str, Any],
    document: DocumentObservation,
    issuance: dict[str, Any],
    anchors: list[dict[str, Any]],
    units: list[dict[str, Any]],
    citation_digest: str,
) -> dict[str, Any]:
    fixed = issuance["fixed_ids"]
    text_layer_sha256 = _sha256_bytes(document.text_layer.encode("utf-8"))
    method = _method(plan)
    packet = {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json",
        "schema_version": "tos_source_text_unit_packet_v1",
        "packet_id": fixed["packet_id"],
        "packet_version": 1,
        "supersedes_packet_ref": None,
        "content_posture": "source_bound",
        "source_scope": {
            "work_ref": plan["work_ref"],
            "expression_ref": plan["source_item"]["expression_ref"],
            "edition_ref": plan["source_item"]["edition_ref"],
            "item_ref": plan["source_item"]["item_ref"],
            "file_ref": plan["source_item"]["file_ref"],
            "file_sha256": plan["source_item"]["file_sha256"],
        },
        "source_layer": {
            "text_layer_ref": plan["outputs"]["private_text_layer_ref"],
            "text_layer_sha256": text_layer_sha256,
            "language": plan["language"],
            "media_type": "text/plain; charset=utf-8",
            "unicode_form": "source_preserved",
            "position_unit": "unicode_code_point",
            "interval": "half_open",
            "immutable": True,
            "visibility": "local_only",
            "publication_authorized": False,
        },
        "schemes": [
            {
                "scheme_id": fixed["scheme_id"],
                "scheme_version": 1,
                "supersedes_scheme_ref": None,
                "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
                "scheme_name": "Antonovsky 1911 PDF page-panel-layout-block proposal v1",
                "analysis_role": "source_layout",
                "boundary_basis": "rule_based",
                "unit_kinds": ["document", "section", "paragraph"],
                "method": method,
                "policies": {
                    "normalization": "no-text-mutation-separate-successor-layer",
                    "punctuation": "included_in_neighbor",
                    "whitespace": "included_in_neighbor",
                    "line_break": "included_in_neighbor",
                    "hyphenation": "preserve_source",
                    "unreported_gaps_allowed": False,
                    "overlap": "nested_only",
                },
                "authority_limit": {
                    "algorithmic_output_is_source_truth": False,
                    "algorithmic_output_is_linguistic_truth": False,
                    "model_subword_is_lexeme": False,
                    "unit_identity_is_semantic_identity": False,
                    "text_mutation_allowed": False,
                },
            }
        ],
        "anchors": anchors,
        "units": units,
        "segmentations": [
            {
                "segmentation_id": fixed["segmentation_id"],
                "segmentation_version": 1,
                "supersedes_segmentation_ref": None,
                "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
                "scheme_ref": fixed["scheme_id"],
                "ordered_unit_refs": [row["unit_id"] for row in units],
                "coverage": {
                    "scope_anchor_ref": anchors[0]["anchor_ref"],
                    "coverage_posture": "exhaustive_nested",
                    "excluded_anchor_refs": [],
                    "unreported_gaps_allowed": False,
                    "overlap_requires_declaration": True,
                    "source_reconstruction_required": True,
                },
                "status": "proposed",
                "status_reason": "PDF pages are exact, while midpoint panels, Poppler blocks, generated separators, region spans, and heading candidates remain unreviewed method results.",
                "maker": method,
                "competing_segmentation_refs": [],
                "review_refs": [],
                "declared_uses": ["navigation", "source_observation", "interchange"],
                "source_text_authority": False,
                "linguistic_authority": False,
                "semantic_authority": False,
            }
        ],
        "reviews": [],
        "projections": [
            {
                "projection_id": fixed["projection_id"],
                "projection_kind": "other",
                "source_segmentation_refs": [fixed["segmentation_id"]],
                "artifact_ref": plan["outputs"]["citation_spine_ref"],
                "artifact_sha256": citation_digest,
                "admission_posture": "proposal_preserving",
                "preserves_unit_ids": True,
                "preserves_status": True,
                "preserves_source_return": True,
                "runtime_authority": False,
                "source_text_authority": False,
                "linguistic_authority": False,
                "semantic_authority": False,
                "visibility": "local_only",
            }
        ],
        "rights_and_visibility": {
            "source_visibility": "local_only",
            "packet_visibility": "public_metadata_only",
            "effective_visibility": "local_only",
            "rights_record_refs": [plan["source_item"]["rights_ref"]],
            "private_source_used": True,
            "publication_authorized": False,
            "inheritance_policy": "most-restrictive-source-packet-and-destination-wins",
        },
        "authority_boundary": {
            "tree_role": "orientation",
            "graph_role": "relation",
            "source_role": "authority",
            "validators_prove_mechanics_not_truth": True,
            "segmentation_is_method_result_not_source_truth": True,
            "source_unit_is_not_lexeme_sign_or_concept": True,
            "model_token_is_not_linguistic_token": True,
            "projection_is_owner_truth": False,
            "legacy_bulk_migration_authorized": False,
        },
    }
    schema = _load_json(repo_root, SCHEMA_REF)
    errors = sorted(
        Draft202012Validator(schema, format_checker=FormatChecker()).iter_errors(packet),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        rendered = "; ".join(
            f"/{'/'.join(map(str, error.absolute_path))}: {error.message}"
            for error in errors[:20]
        )
        raise AntonovskyMarkupError(f"source-text-unit schema failure: {rendered}")
    return packet


def _summary(
    plan: dict[str, Any],
    document: DocumentObservation,
    regions: list[dict[str, Any]],
    headings: list[dict[str, Any]],
) -> dict[str, Any]:
    counts = _observed_counts(document)
    return {
        "schema_version": "tos_zarathustra_antonovsky_technical_markup_summary_v1",
        "language": "ru",
        "item_ref": plan["source_item"]["item_ref"],
        "source_file_sha256": plan["source_item"]["file_sha256"],
        "bbox_sha256": _sha256_bytes(document.bbox_bytes),
        "pdftotext_version": document.pdftotext_version,
        "warning_count": len(document.warning_lines),
        "private_text_layer_sha256": _sha256_bytes(
            document.text_layer.encode("utf-8")
        ),
        "private_text_layer_code_points": len(document.text_layer),
        "pdf_page_count": counts["pdf_pages"],
        "observed_panel_count": counts["observed_panels"],
        "poppler_flow_count": counts["poppler_flows"],
        "layout_block_paragraph_candidate_count": counts[
            "layout_block_paragraph_candidates"
        ],
        "physical_line_counted_not_unitized": counts[
            "physical_lines_counted_not_unitized"
        ],
        "embedded_word_counted_not_unitized": counts[
            "embedded_words_counted_not_unitized"
        ],
        "tracked_unit_count": counts["tracked_units"],
        "region_candidate_count": len(regions),
        "heading_candidate_count": len(headings),
        "heading_candidate_coverage_posture": plan["heading_candidate_rule"][
            "coverage_posture"
        ],
        "regions": regions,
        "segmentation_status": "proposed",
        "human_review_status": "unreviewed",
        "accepted_russian_text": False,
        "accepted_paragraphs": False,
        "accepted_sections": False,
        "source_target_alignment_created": False,
        "german_numbering_copied": False,
        "semantic_fields_materialized": False,
        "source_text_included": False,
        "publication_authorized": False,
        "authority_boundary": plan["authority_boundary"],
    }


def _provenance(
    repo_root: Path,
    plan: dict[str, Any],
    issuance_ref: str,
    tracked: dict[str, bytes],
    document: DocumentObservation,
) -> dict[str, Any]:
    source = plan["source_item"]
    output_rows = [
        {"ref": ref, "role": "tracked-text-free-technical-markup-artifact", "sha256": _sha256_bytes(payload)}
        for ref, payload in sorted(tracked.items())
        if ref != plan["outputs"]["provenance_ref"]
    ]
    return {
        "schema_version": "tos_provenance_event_v1",
        "event_id": EVENT_ID,
        "event_type": "segmentation",
        "started_at": plan["created_at"],
        "ended_at": plan["created_at"],
        "agent_refs": [
            "software:tos-antonovsky-1911-technical-markup-builder",
            "model:codex",
        ],
        "inputs": [
            {**_binding(repo_root, PLAN_REF), "role": "tracked-technical-markup-plan"},
            {**_binding(repo_root, issuance_ref), "role": "tracked-opaque-identity-issuance"},
            {
                "ref": source["file_ref"],
                "role": "local-fixity-bound-antonovsky-1911-pdf",
                "sha256": source["file_sha256"],
            },
            {
                **_binding(repo_root, source["resource_inventory_ref"]),
                "role": "tracked-text-free-PDF-page-resource-inventory",
            },
            {
                **_binding(repo_root, source["rights_ref"]),
                "role": "tracked-layered-rights-record",
            },
        ],
        "outputs": output_rows,
        "method": {
            "maker_type": "software",
            "name": "exact-antonovsky-1911-poppler-bbox-layout-technical-proposal",
            "version": "1",
            "artifact_digest": _sha256_path(_resolve(repo_root, BUILDER_REF)),
            "runtime": None,
            "device": "cpu",
            "configuration": {
                "pdf_pages": len(document.pages),
                "source_text_tracked": False,
                "opaque_identity_issuance_reused": True,
                "page_boundary_status": "source_attested",
                "panel_block_region_heading_status": "proposed",
                "human_review_status": "unreviewed",
                "source_target_alignment_created": False,
                "semantic_fields_materialized": False,
            },
            "prompt_or_instruction_ref": PLAN_REF,
        },
        "status": "completed_with_warnings",
        "warnings": [
            "Poppler layout blocks are paragraph candidates, not accepted Russian paragraphs.",
            "The typographic heading set is deliberately incomplete and unreviewed.",
            "The six region spans are witness-local page-side candidates and create no German correspondence.",
            "The exact PDF, bbox observation, and sequential embedded-text layer remain local-only mode-0600 files.",
            "No correction, translation alignment, semantics, graph, canon, rights clearance, publication, or server-transfer route is created.",
        ],
        "receipt_refs": [row["ref"] for row in output_rows],
        "rights_basis_ref": source["rights_ref"],
        "event_version": 1,
        "supersedes_event_ref": None,
    }


def _screening_provenance(
    repo_root: Path,
    plan: dict[str, Any],
    screening: dict[str, Any],
    base_outputs: dict[str, bytes],
    screening_outputs: dict[str, bytes],
) -> dict[str, Any]:
    return {
        "schema_version": "tos_provenance_event_v1",
        "event_id": SCREENING_EVENT_ID,
        "event_type": "annotation",
        "started_at": screening["created_at"],
        "ended_at": screening["created_at"],
        "agent_refs": ["model:codex"],
        "inputs": [
            {
                **_binding(repo_root, SCREENING_PLAN_REF),
                "role": "tracked-text-free-technical-screening-plan",
            },
            {
                "ref": plan["outputs"]["heading_candidates_ref"],
                "role": "tracked-text-free-heading-candidate-set",
                "sha256": _sha256_bytes(
                    base_outputs[plan["outputs"]["heading_candidates_ref"]]
                ),
            },
            {
                "ref": plan["outputs"]["citation_spine_ref"],
                "role": "tracked-text-free-source-block-citation-spine",
                "sha256": _sha256_bytes(
                    base_outputs[plan["outputs"]["citation_spine_ref"]]
                ),
            },
            {
                "ref": plan["source_item"]["file_ref"],
                "role": "local-source-visible-exact-PDF",
                "sha256": plan["source_item"]["file_sha256"],
            },
        ],
        "outputs": [
            {
                "ref": ref,
                "role": "tracked-text-free-model-screening-projection",
                "sha256": _sha256_bytes(payload),
            }
            for ref, payload in sorted(screening_outputs.items())
        ],
        "method": {
            "maker_type": "model_source_visible",
            "name": "antonovsky-1911-heading-and-indent-boundary-screening",
            "version": "1",
            "artifact_digest": _sha256_path(_resolve(repo_root, BUILDER_REF)),
            "runtime": None,
            "device": "cpu",
            "configuration": {
                "heading_candidate_coverage": "all_enumerated_candidates",
                "split_sample_block_count": screening[
                    "paragraph_split_screening"
                ]["source_visible_sample"]["sample_block_count"],
                "packet_review_created": False,
                "real_human_reviewer": False,
                "source_target_alignment_created": False,
                "semantic_fields_materialized": False,
            },
            "prompt_or_instruction_ref": SCREENING_PLAN_REF,
        },
        "status": "completed_with_warnings",
        "warnings": [
            "The model source-visible screening is not a real-human text-unit review.",
            "Heading roles remain technical candidates and do not accept section boundaries.",
            "Paragraph spans remain an overlay over unchanged Poppler block units and issue no successor identities.",
            "The 42-block split sample does not accept unsampled boundaries.",
            "No translation alignment, semantics, graph, canon, publication, or transfer authority is created.",
        ],
        "receipt_refs": sorted(screening_outputs),
        "rights_basis_ref": plan["source_item"]["rights_ref"],
        "event_version": 1,
        "supersedes_event_ref": None,
    }


def _build_artifacts(
    repo_root: Path,
    plan: dict[str, Any],
    screening: dict[str, Any],
    document: DocumentObservation,
    issuance: dict[str, Any],
) -> tuple[dict[str, bytes], dict[str, bytes]]:
    anchors, units, citations, regions, headings = _build_views(
        document, plan, issuance
    )
    citation_bytes = _jsonl_bytes(citations)
    region_bytes = _jsonl_bytes(regions)
    heading_bytes = _jsonl_bytes(headings)
    packet = _packet(
        repo_root,
        plan,
        document,
        issuance,
        anchors,
        units,
        _sha256_bytes(citation_bytes),
    )
    summary = _summary(plan, document, regions, headings)
    tracked: dict[str, bytes] = {
        plan["outputs"]["packet_ref"]: _json_bytes(packet),
        plan["outputs"]["citation_spine_ref"]: citation_bytes,
        plan["outputs"]["region_candidates_ref"]: region_bytes,
        plan["outputs"]["heading_candidates_ref"]: heading_bytes,
        plan["outputs"]["summary_ref"]: _json_bytes(summary),
    }
    provenance = _provenance(
        repo_root,
        plan,
        plan["identity_policy"]["issuance_ref"],
        tracked,
        document,
    )
    heading_screening, paragraph_overlay, screening_summary = _screening_views(
        document, plan, screening, citations, headings
    )
    screening_outputs = {
        plan["outputs"]["heading_screening_ref"]: _jsonl_bytes(heading_screening),
        plan["outputs"]["paragraph_segment_overlay_ref"]: _jsonl_bytes(
            paragraph_overlay
        ),
        plan["outputs"]["screening_summary_ref"]: _json_bytes(screening_summary),
    }
    screening_provenance = _screening_provenance(
        repo_root, plan, screening, tracked, screening_outputs
    )
    tracked.update(screening_outputs)
    tracked[plan["outputs"]["provenance_ref"]] = _jsonl_bytes(
        [provenance, screening_provenance]
    )
    private = {
        plan["outputs"]["private_text_layer_ref"]: document.text_layer.encode("utf-8"),
        plan["outputs"]["private_bbox_ref"]: document.bbox_bytes,
    }
    return tracked, private


def _write_artifacts(
    repo_root: Path,
    tracked: dict[str, bytes],
    private: dict[str, bytes],
) -> None:
    for ref, payload in tracked.items():
        path = _resolve(repo_root, ref)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
    for ref, payload in private.items():
        path = _resolve(repo_root, ref)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        os.chmod(path, stat.S_IRUSR | stat.S_IWUSR)


def _check_artifacts(
    repo_root: Path,
    tracked: dict[str, bytes],
    private: dict[str, bytes],
) -> None:
    errors: list[str] = []
    for ref, expected in {**tracked, **private}.items():
        path = _resolve(repo_root, ref)
        if not path.is_file():
            errors.append(f"missing: {ref}")
            continue
        observed = path.read_bytes()
        if observed != expected:
            errors.append(f"drifted: {ref}")
        if ref in private:
            mode = stat.S_IMODE(path.stat().st_mode)
            if mode != 0o600:
                errors.append(f"private mode drifted: {ref} is {oct(mode)}")
    if errors:
        raise AntonovskyMarkupError("artifact parity failed: " + "; ".join(errors))


def _contains_forbidden_key(value: Any) -> bool:
    if isinstance(value, dict):
        for key, child in value.items():
            if key in FORBIDDEN_TRACKED_KEYS or _contains_forbidden_key(child):
                return True
    elif isinstance(value, list):
        return any(_contains_forbidden_key(child) for child in value)
    return False


def _validate_tracked(
    repo_root: Path, plan: dict[str, Any], screening: dict[str, Any]
) -> dict[str, Any]:
    issuance = _load_and_validate_issuance(repo_root, plan, None)
    packet = _load_json(repo_root, plan["outputs"]["packet_ref"])
    schema = _load_json(repo_root, SCHEMA_REF)
    errors = sorted(
        Draft202012Validator(schema, format_checker=FormatChecker()).iter_errors(packet),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        rendered = "; ".join(
            f"/{'/'.join(map(str, error.absolute_path))}: {error.message}"
            for error in errors[:20]
        )
        raise AntonovskyMarkupError(f"tracked packet schema failure: {rendered}")

    citation_path = _resolve(repo_root, plan["outputs"]["citation_spine_ref"])
    region_path = _resolve(repo_root, plan["outputs"]["region_candidates_ref"])
    heading_path = _resolve(repo_root, plan["outputs"]["heading_candidates_ref"])
    heading_screening_path = _resolve(
        repo_root, plan["outputs"]["heading_screening_ref"]
    )
    paragraph_overlay_path = _resolve(
        repo_root, plan["outputs"]["paragraph_segment_overlay_ref"]
    )
    summary = _load_json(repo_root, plan["outputs"]["summary_ref"])
    screening_summary = _load_json(
        repo_root, plan["outputs"]["screening_summary_ref"]
    )
    try:
        citations = [
            json.loads(line)
            for line in citation_path.read_text(encoding="utf-8").splitlines()
        ]
        regions = [
            json.loads(line)
            for line in region_path.read_text(encoding="utf-8").splitlines()
        ]
        headings = [
            json.loads(line)
            for line in heading_path.read_text(encoding="utf-8").splitlines()
        ]
        heading_screening = [
            json.loads(line)
            for line in heading_screening_path.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        paragraph_overlay = [
            json.loads(line)
            for line in paragraph_overlay_path.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
    except (OSError, json.JSONDecodeError) as exc:
        raise AntonovskyMarkupError(f"cannot load tracked projections: {exc}") from exc

    if any(
        _contains_forbidden_key(value)
        for value in (
            packet,
            citations,
            regions,
            headings,
            heading_screening,
            paragraph_overlay,
            summary,
            screening_summary,
        )
    ):
        raise AntonovskyMarkupError("content or semantic key escaped tracked boundary")
    if len(citations) != len({row["unit_id"] for row in citations}):
        raise AntonovskyMarkupError("citation unit identity collision")
    if len(citations) != len({row["display_citation"] for row in citations}):
        raise AntonovskyMarkupError("display citation collision")
    issuance_rows = issuance["units"]
    if [row["source_locator"] for row in citations] != [
        row["source_locator"] for row in issuance_rows
    ]:
        raise AntonovskyMarkupError("citation and issuance locator order drifted")
    if [row["unit_id"] for row in citations] != [
        row["unit_id"] for row in issuance_rows
    ]:
        raise AntonovskyMarkupError("citation and issuance unit identity drifted")
    if [row["anchor_ref"] for row in citations] != [
        row["anchor_ref"] for row in issuance_rows
    ]:
        raise AntonovskyMarkupError("citation and issuance anchor identity drifted")
    if [row["unit_id"] for row in packet["units"]] != [
        row["unit_id"] for row in citations
    ]:
        raise AntonovskyMarkupError("packet and citation unit closure drifted")
    if packet["projections"][0]["artifact_sha256"] != _sha256_path(citation_path):
        raise AntonovskyMarkupError("packet citation projection digest drifted")
    if len(regions) != len(plan["region_candidates"]):
        raise AntonovskyMarkupError("tracked region count drifted")
    if len(headings) != plan["expected_counts"]["heading_candidates"]:
        raise AntonovskyMarkupError("tracked heading candidate count drifted")
    role_by_ordinal = _heading_role_map(screening)
    if len(heading_screening) != len(headings):
        raise AntonovskyMarkupError("tracked heading screening count drifted")
    for candidate, screened in zip(headings, heading_screening, strict=True):
        ordinal = candidate["candidate_ordinal"]
        if screened.get("candidate_ordinal") != ordinal:
            raise AntonovskyMarkupError("heading screening order drifted")
        for key in ("unit_id", "anchor_ref", "source_locator", "display_citation"):
            if screened.get(key) != candidate.get(key):
                raise AntonovskyMarkupError(
                    f"heading screening source binding drifted: {ordinal} {key}"
                )
        if screened.get("technical_role") != role_by_ordinal[ordinal]:
            raise AntonovskyMarkupError(
                f"heading screening technical role drifted: {ordinal}"
            )
        if screened.get("real_human_reviewer") is not False:
            raise AntonovskyMarkupError("model screening became a human review")
        if screened.get("accepted_section") is not False:
            raise AntonovskyMarkupError("model screening accepted a section")

    overlay_by_unit: dict[str, list[dict[str, Any]]] = {}
    citation_by_unit = {row["unit_id"]: row for row in citations}
    for expected_ordinal, row in enumerate(paragraph_overlay, start=1):
        if row.get("overlay_segment_ordinal") != expected_ordinal:
            raise AntonovskyMarkupError("paragraph overlay order drifted")
        unit_id = row.get("source_block_unit_id")
        citation = citation_by_unit.get(unit_id)
        if citation is None or citation.get("unit_kind") != "paragraph":
            raise AntonovskyMarkupError("paragraph overlay source unit is unresolved")
        if row.get("source_locator") != citation.get("source_locator"):
            raise AntonovskyMarkupError("paragraph overlay locator binding drifted")
        if row.get("creates_new_text_unit_identity") is not False:
            raise AntonovskyMarkupError("paragraph overlay issued a new unit identity")
        if row.get("accepted_paragraph") is not False:
            raise AntonovskyMarkupError("paragraph overlay accepted a paragraph")
        overlay_by_unit.setdefault(unit_id, []).append(row)
    for unit_id, rows in overlay_by_unit.items():
        citation = citation_by_unit[unit_id]
        if rows[0]["selector_start"] != citation["selector_start"]:
            raise AntonovskyMarkupError("paragraph overlay start coverage drifted")
        if rows[-1]["selector_end"] != citation["selector_end"]:
            raise AntonovskyMarkupError("paragraph overlay end coverage drifted")
        for within_block, row in enumerate(rows, start=1):
            if row["segment_ordinal_within_source_block"] != within_block:
                raise AntonovskyMarkupError("paragraph overlay local order drifted")
        if any(
            current["selector_start"] != previous["selector_end"]
            for previous, current in zip(rows, rows[1:])
        ):
            raise AntonovskyMarkupError("paragraph overlay has a gap or overlap")
    expected_rule = screening["paragraph_split_screening"]
    if len(overlay_by_unit) != expected_rule["expected_affected_source_block_count"]:
        raise AntonovskyMarkupError("paragraph overlay affected-block count drifted")
    if (
        len(paragraph_overlay) - len(overlay_by_unit)
        != expected_rule["expected_split_boundary_count"]
    ):
        raise AntonovskyMarkupError("paragraph overlay split-boundary count drifted")
    if len(paragraph_overlay) != expected_rule["expected_proposed_segment_count"]:
        raise AntonovskyMarkupError("paragraph overlay segment count drifted")
    if summary.get("tracked_unit_count") != len(citations):
        raise AntonovskyMarkupError("summary tracked unit count drifted")
    if summary.get("layout_block_paragraph_candidate_count") != sum(
        row["unit_kind"] == "paragraph" for row in citations
    ):
        raise AntonovskyMarkupError("summary paragraph candidate count drifted")
    if summary.get("heading_candidate_count") != len(headings):
        raise AntonovskyMarkupError("summary heading candidate count drifted")
    if screening_summary.get("heading_candidate_count") != len(heading_screening):
        raise AntonovskyMarkupError("screening summary heading count drifted")
    if (
        screening_summary.get("affected_source_block_count")
        != len(overlay_by_unit)
    ):
        raise AntonovskyMarkupError("screening summary affected-block count drifted")
    if (
        screening_summary.get("proposed_segment_count")
        != len(paragraph_overlay)
    ):
        raise AntonovskyMarkupError("screening summary segment count drifted")
    if screening_summary.get("human_review_performed") is not False:
        raise AntonovskyMarkupError("screening summary became a human review")
    if summary.get("accepted_russian_text") is not False:
        raise AntonovskyMarkupError("tracked summary widened Russian text authority")
    if summary.get("source_target_alignment_created") is not False:
        raise AntonovskyMarkupError("tracked summary widened alignment authority")
    if summary.get("semantic_fields_materialized") is not False:
        raise AntonovskyMarkupError("tracked summary widened semantic authority")
    if packet["rights_and_visibility"]["publication_authorized"] is not False:
        raise AntonovskyMarkupError("tracked packet widened publication authority")
    if packet.get("reviews") != []:
        raise AntonovskyMarkupError("model screening escaped into packet reviews")
    return summary


def _prepare_local(
    repo_root: Path,
    plan: dict[str, Any],
) -> DocumentObservation:
    source_pdf = _source_pdf(repo_root, plan)
    inventory_pages = _inventory_pages(repo_root, plan)
    bbox_bytes, warning_lines, binary, version = _extract_bbox(source_pdf, plan)
    return _parse_observation(
        bbox_bytes,
        warning_lines,
        binary,
        version,
        inventory_pages,
        plan,
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--build", action="store_true", help="write generated artifacts")
    mode.add_argument("--check", action="store_true", help="check full local parity")
    mode.add_argument(
        "--validate-tracked",
        action="store_true",
        help="validate tracked artifacts without local source payloads",
    )
    parser.add_argument(
        "--issue-identities",
        action="store_true",
        help="one-time opaque identity issuance; valid only with --build",
    )
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    args = parser.parse_args(argv)
    repo_root = args.repo_root.resolve()
    try:
        plan = _load_plan(repo_root)
        screening = _load_screening_plan(repo_root, plan)
        if args.validate_tracked:
            if args.issue_identities:
                raise AntonovskyMarkupError(
                    "--issue-identities is valid only with --build"
                )
            summary = _validate_tracked(repo_root, plan, screening)
            screening_summary = _load_json(
                repo_root, plan["outputs"]["screening_summary_ref"]
            )
            print(
                "antonovsky technical markup tracked validation passed: "
                f"units={summary['tracked_unit_count']} "
                f"blocks={summary['layout_block_paragraph_candidate_count']} "
                f"headings={summary['heading_candidate_count']} "
                f"screened_headings={screening_summary['heading_candidate_count']} "
                f"split_boundaries={screening_summary['split_boundary_count']}"
            )
            return 0

        document = _prepare_local(repo_root, plan)
        if args.issue_identities:
            if not args.build:
                raise AntonovskyMarkupError(
                    "--issue-identities is valid only with --build"
                )
            _issue_identities(repo_root, plan, document)
        issuance = _load_and_validate_issuance(repo_root, plan, document)
        tracked, private = _build_artifacts(
            repo_root, plan, screening, document, issuance
        )
        if args.build:
            _write_artifacts(repo_root, tracked, private)
            action = "built"
        else:
            _check_artifacts(repo_root, tracked, private)
            action = "parity passed"
        summary = json.loads(tracked[plan["outputs"]["summary_ref"]])
        screening_summary = json.loads(
            tracked[plan["outputs"]["screening_summary_ref"]]
        )
        print(
            f"antonovsky technical markup {action}: "
            f"pages={summary['pdf_page_count']} "
            f"panels={summary['observed_panel_count']} "
            f"blocks={summary['layout_block_paragraph_candidate_count']} "
            f"headings={summary['heading_candidate_count']} "
            f"units={summary['tracked_unit_count']} "
            f"screened_headings={screening_summary['heading_candidate_count']} "
            f"split_boundaries={screening_summary['split_boundary_count']}"
        )
        return 0
    except AntonovskyMarkupError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
