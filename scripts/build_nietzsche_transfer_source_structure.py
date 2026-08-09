#!/usr/bin/env python3
"""Build text-free German source structure for frozen transfer candidates.

The builder verifies fixed series-qualified number-label start-page candidates
against exact local German source witnesses. Provider or embedded OCR is used
only as disposable navigation. A small fixed set of OCR gaps is supplied by
bounded model-visible page review. Outputs contain no source or target prose and
establish no accepted German, passage alignment, translation, semantics, rights
clearance, eligibility, target gold, or canon authority.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = Path("ToS/source-witnesses")
SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "hierarchical-source-numbered-unit-page-map.schema.json"
)
PDFTOTEXT_VERSION = "26.01.0"
XHTML_NAMESPACE = "{http://www.w3.org/1999/xhtml}"
AUTHORITY_BOUNDARY = (
    "fixity-bound German source-page start candidates for independently named "
    "series-qualified printed number labels only, verified through disposable "
    "embedded or provider OCR navigation plus a bounded model-visible review "
    "of explicit OCR gaps; all anchors remain proposed whole-page addresses, "
    "the provider OCR remains unaccepted, and no source prose, exact passage "
    "end, accepted German, source-to-target passage alignment, translation "
    "relation, equivalence, quality, textual or edition identity, semantics, "
    "rights clearance, transfer eligibility, target gold, or canon authority "
    "follows"
)
DOES_NOT_ESTABLISH = [
    "source_text",
    "exact_line_boundaries",
    "exact_passage_end_boundaries",
    "accepted_german",
    "provider_ocr_correctness",
    "full_container_page_identity",
    "source_to_target_passage_alignment",
    "translation_correspondence",
    "translation_equivalence",
    "translation_quality",
    "textual_identity",
    "edition_equivalence",
    "semantics",
    "rights_clearance",
    "eligible_target_unit",
    "target_gold",
    "canon_promotion",
]
NUMERIC_NORMALIZATIONS = [
    "remove non-ASCII alphanumeric characters",
    "ASCII casefold",
    "map i l x to 1, o to 0, s to 5, z to 2",
    "join at most two adjacent same-baseline PDF short-line fragments",
]


GENE_ITEM_DIR = (
    SOURCE_ROOT
    / "works/friedrich-nietzsche/zur-genealogie-der-moral/"
    "expressions/de-naumann-1892-second/"
    "editions/leipzig-c-g-naumann-1892-second-edition/items/"
    "wikimedia-commons-unc-scan-pdf"
)
ANTI_ADDRESS_ITEM_DIR = (
    SOURCE_ROOT
    / "collections/friedrich-nietzsche/"
    "nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/"
    "editions/leipzig-c-g-naumann-1906/items/"
    "wikimedia-commons-stanford-scan-djvu"
)
ANTI_NAV_ITEM_DIR = (
    SOURCE_ROOT
    / "collections/friedrich-nietzsche/"
    "nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/"
    "editions/leipzig-c-g-naumann-1906/items/"
    "internet-archive-google-stanford-djvu-xml"
)
ANTI_WORK_BOUNDARY_PATH = (
    SOURCE_ROOT
    / "collections/friedrich-nietzsche/"
    "nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/"
    "structure/work-boundaries/work-boundary-map.json"
)


WORK_CONFIGS: tuple[dict[str, Any], ...] = (
    {
        "slug": "zur-genealogie-der-moral",
        "work_ref": "tos.work.friedrich-nietzsche.zur-genealogie-der-moral",
        "expression_ref": (
            "tos.expression.friedrich-nietzsche.zur-genealogie-der-moral."
            "de-naumann-1892-second"
        ),
        "address_item_dir": GENE_ITEM_DIR,
        "navigation_item_dir": GENE_ITEM_DIR,
        "address_media_type": "application/pdf",
        "navigation_media_type": "application/pdf",
        "address_profile": "pdf_pages_v1",
        "navigation_profile": "pdf_pages_v1",
        "navigation_offset": 0,
        "navigation_relation_scope": "same-fixity-bound-file-page-index",
        "compared_address_pages": [],
        "candidate_profile": (
            "poppler-pdftotext-bbox-short-line-number-candidate-v1"
        ),
        "represented_start_page": 7,
        "represented_end_page": 202,
        "represented_basis": (
            "source-visible numbered preface and three essay title, opening, "
            "number-label, transition, and terminal page inspection"
        ),
        "work_boundary_path": None,
        "map_id": (
            "tos.hierarchical-source-numbered-unit-page-map."
            "friedrich-nietzsche.zur-genealogie-der-moral."
            "de-naumann-1892-second"
        ),
        "event_id": (
            "tos.event.hierarchical-source-numbered-unit-page-map."
            "friedrich-nietzsche.zur-genealogie-der-moral."
            "de-naumann-1892-second.2026-08-08"
        ),
        "output_dir": GENE_ITEM_DIR / "structure",
        "series": (
            {
                "series_key": "preface",
                "series_kind": "numbered-preface",
                "start_page": 7,
                "end_page": 18,
                "start_pages": [7, 8, 9, 11, 13, 14, 15, 17],
                "overrides": {"1": 7},
            },
            {
                "series_key": "essay-1",
                "series_kind": "numbered-essay",
                "start_page": 19,
                "end_page": 58,
                "start_pages": [
                    21, 22, 25, 26, 27, 29, 31, 33, 35,
                    36, 40, 44, 46, 48, 51, 54, 57,
                ],
                "overrides": {"11": 40},
            },
            {
                "series_key": "essay-2",
                "series_kind": "numbered-essay",
                "start_page": 59,
                "end_page": 114,
                "start_pages": [
                    61, 63, 65, 68, 69, 71, 74, 78, 79, 81,
                    82, 86, 90, 92, 94, 96, 99, 101, 102, 105,
                    106, 108, 109, 111, 113,
                ],
                "overrides": {"1": 61, "13": 90, "15": 94, "20": 105},
            },
            {
                "series_key": "essay-3",
                "series_kind": "numbered-essay",
                "start_page": 115,
                "end_page": 202,
                "start_pages": [
                    117, 117, 119, 121, 123, 125, 128, 131, 137, 140,
                    142, 145, 147, 150, 155, 159, 161, 167, 170, 173,
                    177, 179, 182, 186, 190, 195, 198, 201,
                ],
                "overrides": {},
            },
        ),
    },
    {
        "slug": "der-antichrist",
        "work_ref": "tos.work.friedrich-nietzsche.der-antichrist",
        "expression_ref": (
            "tos.expression.friedrich-nietzsche.der-antichrist."
            "de-naumann-1906-volume-8"
        ),
        "address_item_dir": ANTI_ADDRESS_ITEM_DIR,
        "navigation_item_dir": ANTI_NAV_ITEM_DIR,
        "address_media_type": "image/vnd.djvu",
        "navigation_media_type": "application/vnd.djvu+xml",
        "address_profile": "djvu_pages_v1",
        "navigation_profile": "djvu_xml_pages_v1",
        "navigation_offset": -2,
        "navigation_relation_scope": (
            "bounded-source-visible-two-page-offset-only"
        ),
        "compared_address_pages": [
            232, 233, 234, 238, 239, 250, 254, 257,
            261, 264, 280, 288, 327, 329,
        ],
        "candidate_profile": (
            "provider-djvu-xml-page-word-number-candidate-v1"
        ),
        "represented_start_page": 232,
        "represented_end_page": 329,
        "represented_basis": (
            "tracked collection member boundary plus source-visible title, "
            "numbered opening, OCR-gap, late-sequence, and terminal pages"
        ),
        "work_boundary_path": ANTI_WORK_BOUNDARY_PATH,
        "map_id": (
            "tos.hierarchical-source-numbered-unit-page-map."
            "friedrich-nietzsche.der-antichrist.de-naumann-1906-volume-8"
        ),
        "event_id": (
            "tos.event.hierarchical-source-numbered-unit-page-map."
            "friedrich-nietzsche.der-antichrist."
            "de-naumann-1906-volume-8.2026-08-08"
        ),
        "output_dir": ANTI_ADDRESS_ITEM_DIR / "structure",
        "series": (
            {
                "series_key": "main",
                "series_kind": "numbered-main-sequence",
                "start_page": 232,
                "end_page": 329,
                "start_pages": [
                    232, 233, 233, 234, 234, 235, 236, 238, 239, 240,
                    241, 242, 243, 244, 246, 247, 248, 250, 250, 251,
                    253, 254, 255, 257, 259, 261, 264, 266, 266, 268,
                    269, 271, 273, 274, 276, 276, 277, 278, 280, 282,
                    284, 285, 286, 288, 291, 294, 296, 297, 299, 300,
                    302, 304, 306, 308, 310, 313, 314, 319, 322, 324,
                    325, 327,
                ],
                "overrides": {
                    "4": 234,
                    "5": 234,
                    "8": 238,
                    "9": 239,
                    "19": 250,
                    "22": 254,
                    "24": 257,
                    "26": 261,
                    "27": 264,
                    "39": 280,
                    "44": 288,
                },
            },
        ),
    },
)


class SourceStructureBuildError(RuntimeError):
    """Raised when an exact source-structure route cannot be reproduced."""


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
        raise SourceStructureBuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise SourceStructureBuildError(f"JSON root is not an object: {path}")
    return payload


def _render_json(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _render_jsonl(payloads: list[dict[str, Any]]) -> str:
    return "".join(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"
        for payload in payloads
    )


def _payload_entry(
    *,
    repo_root: Path,
    payload_source_root: Path,
    item_dir: Path,
    media_type: str,
) -> tuple[dict[str, Any], dict[str, Any], Path]:
    manifest_path = repo_root / item_dir / "item.manifest.json"
    manifest = _read_json(manifest_path)
    entries = [
        entry
        for entry in manifest.get("payload_files", [])
        if entry.get("media_type") == media_type
    ]
    if len(entries) != 1:
        raise SourceStructureBuildError(
            f"{manifest_path} does not bind exactly one {media_type} payload"
        )
    entry = entries[0]
    relative_item_dir = item_dir.relative_to(SOURCE_ROOT)
    path = payload_source_root / relative_item_dir / entry["relative_path"]
    if not path.is_file():
        raise SourceStructureBuildError(f"local payload is missing: {path}")
    if path.stat().st_size != entry["byte_size"]:
        raise SourceStructureBuildError(f"local payload size drifted: {path}")
    if _sha256_path(path) != entry["sha256"]:
        raise SourceStructureBuildError(f"local payload digest drifted: {path}")
    return manifest, entry, path


def _inventory_binding(
    *,
    repo_root: Path,
    item_dir: Path,
    entry: dict[str, Any],
    profile: str,
) -> tuple[dict[str, Any], dict[str, Any], Path]:
    path = repo_root / item_dir / "resource-inventory.json"
    inventory = _read_json(path)
    files = [
        item
        for item in inventory.get("files", [])
        if item.get("file_id") == entry["file_id"]
    ]
    if len(files) != 1:
        raise SourceStructureBuildError(
            f"{path} does not resolve {entry['file_id']} exactly once"
        )
    file_inventory = files[0]
    if (
        file_inventory.get("file_sha256") != entry["sha256"]
        or file_inventory.get("profile") != profile
    ):
        raise SourceStructureBuildError(f"manifest/inventory drift: {path}")
    return inventory, file_inventory, path


def _rights_binding(repo_root: Path, item_dir: Path) -> tuple[dict[str, Any], Path]:
    path = repo_root / item_dir / "rights.json"
    return _read_json(path), path


def _pdftotext_version() -> str:
    result = subprocess.run(
        ["pdftotext", "-v"],
        check=False,
        capture_output=True,
        text=True,
    )
    match = re.search(
        r"pdftotext version ([0-9.]+)",
        f"{result.stdout}\n{result.stderr}",
    )
    if result.returncode != 0 or match is None:
        raise SourceStructureBuildError("pdftotext is unavailable")
    version = match.group(1)
    if version != PDFTOTEXT_VERSION:
        raise SourceStructureBuildError(
            f"pdftotext version drifted: expected {PDFTOTEXT_VERSION}, got {version}"
        )
    return version


def _numeric_candidate(value: str) -> str:
    translated = (
        re.sub(r"[^0-9A-Za-z]", "", value)
        .lower()
        .translate(str.maketrans({"i": "1", "l": "1", "x": "1", "o": "0", "s": "5", "z": "2"}))
    )
    return translated if translated.isdigit() else ""


def _pdf_bbox_pages(
    pdf_path: Path,
    *,
    start_page: int,
    end_page: int,
) -> dict[int, ET.Element]:
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
        raise SourceStructureBuildError(
            f"pdftotext bbox extraction failed: {diagnostic}"
        )
    try:
        root = ET.fromstring(result.stdout)
    except ET.ParseError as exc:
        raise SourceStructureBuildError(
            f"pdftotext bbox XHTML is invalid: {exc}"
        ) from exc
    pages = root.findall(f".//{XHTML_NAMESPACE}page")
    expected = end_page - start_page + 1
    if len(pages) != expected:
        raise SourceStructureBuildError(
            f"bbox page count drifted: expected {expected}, got {len(pages)}"
        )
    return {
        page_number: page
        for page_number, page in enumerate(pages, start=start_page)
    }


def _pdf_page_has_number(page: ET.Element, expected: str) -> bool:
    fragments: list[tuple[float, float, str]] = []
    for line in page.iter(f"{XHTML_NAMESPACE}line"):
        words = line.findall(f"{XHTML_NAMESPACE}word")
        if not 1 <= len(words) <= 2:
            continue
        x_min = float(line.get("xMin", "-1"))
        y_min = float(line.get("yMin", "-1"))
        if not (150 <= x_min <= 220 and 50 <= y_min <= 520):
            continue
        raw = "".join("".join(word.itertext()) for word in words)
        fragments.append((x_min, y_min, raw))
        if _numeric_candidate(raw) == expected:
            return True
    for left_x, left_y, left_raw in fragments:
        for right_x, right_y, right_raw in fragments:
            if (
                0 < right_x - left_x <= 15
                and abs(right_y - left_y) <= 3
                and _numeric_candidate(left_raw + right_raw) == expected
            ):
                return True
    return False


def _djvu_xml_pages(xml_path: Path) -> list[ET.Element]:
    try:
        root = ET.parse(xml_path).getroot()
    except ET.ParseError as exc:
        raise SourceStructureBuildError(f"DjVuXML is invalid: {exc}") from exc
    if root.tag.rsplit("}", 1)[-1] != "DjVuXML":
        raise SourceStructureBuildError("navigation payload has no DjVuXML root")
    pages = root.findall(".//OBJECT")
    if len(pages) != 525:
        raise SourceStructureBuildError(
            f"DjVuXML page count drifted: expected 525, got {len(pages)}"
        )
    return pages


def _djvu_xml_page_has_number(page: ET.Element, expected: str) -> bool:
    return any(
        _numeric_candidate(word.text or "") == expected
        for word in page.findall(".//WORD")
    )


def _unit_ref(series_key: str, unit_key: str) -> str:
    return f"{series_key}:{unit_key}"


def _anchor_id(config: dict[str, Any], series_key: str, unit_key: str) -> str:
    expression_key = config["expression_ref"].split(".")[-1]
    return (
        f"tos.anchor.friedrich-nietzsche.{config['slug']}.{expression_key}."
        f"{series_key}.unit-{unit_key}.source-start-page"
    )


def _passage_id(config: dict[str, Any], series_key: str, unit_key: str) -> str:
    expression_key = config["expression_ref"].split(".")[-1]
    return (
        f"tos.passage.friedrich-nietzsche.{config['slug']}.{expression_key}."
        f"{series_key}.unit-{unit_key}"
    )


def _output_paths(config: dict[str, Any]) -> dict[str, Path]:
    output_dir = config["output_dir"]
    return {
        "map": output_dir / "hierarchical-numbered-unit-page-map.json",
        "anchors": output_dir / "hierarchical-numbered-unit-anchors.jsonl",
        "provenance": output_dir / "provenance.numbered-unit-page-map.jsonl",
    }


def _witness_binding(
    *,
    repo_root: Path,
    item_dir: Path,
    manifest: dict[str, Any],
    entry: dict[str, Any],
    profile: str,
    inventory_path: Path,
    rights_path: Path,
) -> dict[str, Any]:
    return {
        "item_ref": manifest["item_id"],
        "file_ref": entry["file_id"],
        "file_sha256": entry["sha256"],
        "inventory_profile": profile,
        "inventory": {
            "ref": inventory_path.relative_to(repo_root).as_posix(),
            "sha256": _sha256_path(inventory_path),
        },
        "rights": {
            "ref": rights_path.relative_to(repo_root).as_posix(),
            "sha256": _sha256_path(rights_path),
        },
    }


def _build_one(
    *,
    repo_root: Path,
    payload_source_root: Path,
    config: dict[str, Any],
    event_at: str,
    pdftotext_version: str,
) -> dict[Path, str]:
    address_manifest, address_entry, address_path = _payload_entry(
        repo_root=repo_root,
        payload_source_root=payload_source_root,
        item_dir=config["address_item_dir"],
        media_type=config["address_media_type"],
    )
    navigation_manifest, navigation_entry, navigation_path = _payload_entry(
        repo_root=repo_root,
        payload_source_root=payload_source_root,
        item_dir=config["navigation_item_dir"],
        media_type=config["navigation_media_type"],
    )
    _, address_file_inventory, address_inventory_path = _inventory_binding(
        repo_root=repo_root,
        item_dir=config["address_item_dir"],
        entry=address_entry,
        profile=config["address_profile"],
    )
    _, navigation_file_inventory, navigation_inventory_path = _inventory_binding(
        repo_root=repo_root,
        item_dir=config["navigation_item_dir"],
        entry=navigation_entry,
        profile=config["navigation_profile"],
    )
    _, address_rights_path = _rights_binding(
        repo_root, config["address_item_dir"]
    )
    _, navigation_rights_path = _rights_binding(
        repo_root, config["navigation_item_dir"]
    )
    address_resources = {
        resource.get("resource_id"): resource
        for resource in address_file_inventory.get("resources", [])
        if isinstance(resource, dict)
    }

    pdf_pages: dict[int, ET.Element] | None = None
    djvu_xml_pages: list[ET.Element] | None = None
    if config["candidate_profile"].startswith("poppler-"):
        pdf_pages = _pdf_bbox_pages(
            navigation_path,
            start_page=config["represented_start_page"],
            end_page=config["represented_end_page"],
        )
    else:
        djvu_xml_pages = _djvu_xml_pages(navigation_path)

    outputs = _output_paths(config)
    series_payloads: list[dict[str, Any]] = []
    anchors: list[dict[str, Any]] = []
    trials: list[dict[str, Any]] = []
    override_unit_refs: list[str] = []
    reviewed_pages: set[int] = set()
    total_machine = 0

    for series_sequence, series in enumerate(config["series"], start=1):
        expected_count = len(series["start_pages"])
        override_keys = set(series["overrides"])
        unit_starts: list[dict[str, Any]] = []
        for sequence, source_page in enumerate(series["start_pages"], start=1):
            unit_key = str(sequence)
            if not series["start_page"] <= source_page <= series["end_page"]:
                raise SourceStructureBuildError(
                    f"{config['slug']} {_unit_ref(series['series_key'], unit_key)} "
                    "leaves its series range"
                )
            navigation_page = source_page - config["navigation_offset"]
            is_override = unit_key in override_keys
            if is_override:
                if series["overrides"][unit_key] != source_page:
                    raise SourceStructureBuildError(
                        f"override page drift for {config['slug']} "
                        f"{_unit_ref(series['series_key'], unit_key)}"
                    )
                basis = "source_visible_gap_review"
                override_unit_refs.append(
                    _unit_ref(series["series_key"], unit_key)
                )
                reviewed_pages.add(source_page)
            elif pdf_pages is not None:
                page = pdf_pages.get(navigation_page)
                if page is None or not _pdf_page_has_number(page, unit_key):
                    raise SourceStructureBuildError(
                        f"PDF number candidate disappeared for {config['slug']} "
                        f"{_unit_ref(series['series_key'], unit_key)} on page "
                        f"{navigation_page}"
                    )
                basis = "embedded_pdf_bbox_number_candidate"
                total_machine += 1
            else:
                if djvu_xml_pages is None or not 1 <= navigation_page <= len(djvu_xml_pages):
                    raise SourceStructureBuildError(
                        f"navigation page leaves DjVuXML: {navigation_page}"
                    )
                if not _djvu_xml_page_has_number(
                    djvu_xml_pages[navigation_page - 1], unit_key
                ):
                    raise SourceStructureBuildError(
                        f"DjVuXML number candidate disappeared for {config['slug']} "
                        f"{_unit_ref(series['series_key'], unit_key)} on page "
                        f"{navigation_page}"
                    )
                basis = "provider_djvu_xml_number_candidate"
                total_machine += 1

            resource_prefix = (
                "pdf-page" if config["address_profile"] == "pdf_pages_v1" else "djvu-page"
            )
            resource_id = f"{resource_prefix}-{source_page:04d}"
            resource = address_resources.get(resource_id)
            if (
                resource is None
                or resource.get("locator", {}).get("page_index") != source_page
            ):
                raise SourceStructureBuildError(
                    f"address resource is missing: {resource_id}"
                )
            anchor_ref = _anchor_id(
                config, series["series_key"], unit_key
            )
            unit_starts.append(
                {
                    "sequence": sequence,
                    "unit_key": unit_key,
                    "source_page": source_page,
                    "resource_id": resource_id,
                    "navigation_page": navigation_page,
                    "anchor_ref": anchor_ref,
                    "basis": basis,
                    "status": "proposed",
                    "human_review_performed": False,
                }
            )
            anchors.append(
                {
                    "schema_version": "tos_source_anchor_v1",
                    "anchor_id": anchor_ref,
                    "item_id": address_manifest["item_id"],
                    "file_id": address_entry["file_id"],
                    "file_sha256": address_entry["sha256"],
                    "passage_id": _passage_id(
                        config, series["series_key"], unit_key
                    ),
                    "selectors": [
                        {
                            "type": "structural",
                            "path": [
                                f"work:{config['slug']}",
                                f"expression:{config['expression_ref'].split('.')[-1]}",
                                f"series:{series['series_key']}",
                                f"numbered-unit:{unit_key}",
                            ],
                            "scheme": (
                                "tos-hierarchical-source-numbered-unit-start-v1"
                            ),
                        },
                        {
                            "type": "page_region",
                            "page": source_page,
                            "x": 0,
                            "y": 0,
                            "width": 1,
                            "height": 1,
                            "coordinate_space": "normalized_0_1",
                        },
                    ],
                    "selector_method": {
                        "maker_type": "mixed",
                        "method": (
                            "series-scoped fixed-page number-candidate verification "
                            "plus bounded source-visible OCR-gap review"
                        ),
                        "version": "1",
                        "configuration_ref": (
                            f"{outputs['map'].as_posix()}#"
                            f"{series['series_key']}-{unit_key}"
                        ),
                    },
                    "status": "proposed",
                    "provenance_event_ref": config["event_id"],
                    "anchor_version": 1,
                    "supersedes_anchor_ref": None,
                    "review_ref": None,
                }
            )

        pages = [unit["source_page"] for unit in unit_starts]
        if pages != sorted(pages):
            raise SourceStructureBuildError(
                f"{config['slug']} {series['series_key']} pages are not monotonic"
            )
        series_payloads.append(
            {
                "series_key": series["series_key"],
                "series_sequence": series_sequence,
                "series_kind": series["series_kind"],
                "start_page": series["start_page"],
                "end_page": series["end_page"],
                "expected_unit_count": expected_count,
                "status": "proposed",
                "unit_starts": unit_starts,
            }
        )
        trials.append(
            {
                "series_key": series["series_key"],
                "expected_unit_count": expected_count,
                "machine_number_candidate_count": expected_count - len(override_keys),
                "source_visible_override_unit_keys": sorted(
                    override_keys, key=int
                ),
            }
        )

    expected_total = sum(len(series["start_pages"]) for series in config["series"])
    if total_machine + len(override_unit_refs) != expected_total:
        raise SourceStructureBuildError(
            f"{config['slug']} machine/override accounting drifted"
        )

    work_boundary = None
    work_boundary_input: dict[str, Any] | None = None
    if config["work_boundary_path"] is not None:
        boundary_path = repo_root / config["work_boundary_path"]
        boundary = _read_json(boundary_path)
        members = [
            member
            for member in boundary.get("members", [])
            if member.get("work_ref") == config["work_ref"]
            and member.get("expression_ref") == config["expression_ref"]
        ]
        if len(members) != 1:
            raise SourceStructureBuildError(
                f"{config['slug']} work boundary does not resolve exactly once"
            )
        member = members[0]
        work_boundary = {
            "ref": config["work_boundary_path"].as_posix(),
            "sha256": _sha256_path(boundary_path),
            "member_sequence": member["sequence"],
            "start_page": member["start_page"],
            "end_page": member["end_page"],
            "epistemic_status": member["epistemic_status"],
            "review_status": member["review_status"],
        }
        work_boundary_input = {
            "ref": work_boundary["ref"],
            "role": "tracked-source-work-boundary",
            "sha256": work_boundary["sha256"],
        }

    address_binding = _witness_binding(
        repo_root=repo_root,
        item_dir=config["address_item_dir"],
        manifest=address_manifest,
        entry=address_entry,
        profile=config["address_profile"],
        inventory_path=address_inventory_path,
        rights_path=address_rights_path,
    )
    navigation_binding = _witness_binding(
        repo_root=repo_root,
        item_dir=config["navigation_item_dir"],
        manifest=navigation_manifest,
        entry=navigation_entry,
        profile=config["navigation_profile"],
        inventory_path=navigation_inventory_path,
        rights_path=navigation_rights_path,
    )
    map_payload = {
        "$schema": SCHEMA_REF,
        "schema_version": "tos_hierarchical_source_numbered_unit_page_map_v1",
        "map_id": config["map_id"],
        "work_ref": config["work_ref"],
        "expression_ref": config["expression_ref"],
        "edition_ref": address_manifest["embodiment_ref"],
        "address_witness": address_binding,
        "navigation_witness": navigation_binding,
        "navigation_relation": {
            "relation_scope": config["navigation_relation_scope"],
            "address_page_from_navigation_page_offset": config["navigation_offset"],
            "source_visible_compared_address_pages": config["compared_address_pages"],
            "full_container_page_identity_claimed": False,
            "textual_identity_claimed": False,
        },
        "represented_page_range": {
            "start_page": config["represented_start_page"],
            "end_page": config["represented_end_page"],
            "basis": config["represented_basis"],
        },
        "work_boundary": work_boundary,
        "map_authority": (
            "machine_candidates_with_bounded_model_source_visible_gap_review_only"
        ),
        "source_text_included": False,
        "method": {
            "name": (
                "series-scoped-fixed-page-number-candidate-verification-plus-"
                "source-visible-gap-review"
            ),
            "version": "1",
            "maker_type": "mixed",
            "local_payloads_read": True,
            "candidate_verification_profiles": [config["candidate_profile"]],
            "numeric_candidate_normalizations": NUMERIC_NORMALIZATIONS,
            "series_trials": trials,
            "source_visible_review": {
                "maker_type": "model",
                "human_repeat_performed": False,
                "reviewed_address_pages": sorted(reviewed_pages),
                "override_unit_refs": override_unit_refs,
            },
            "source_to_target_text_compared": False,
            "translation_alignment_inferred": False,
            "semantic_matching_used": False,
            "no_source_text_emitted": True,
        },
        "series": series_payloads,
        "summary": {
            "series_count": len(series_payloads),
            "numbered_unit_count": expected_total,
            "machine_number_candidate_count": total_machine,
            "source_visible_override_unit_count": len(override_unit_refs),
            "source_visible_reviewed_page_count": len(reviewed_pages),
            "exact_start_page_candidates_materialized": expected_total,
            "unresolved_unit_count": 0,
            "start_pages_monotonic_within_series": True,
            "all_anchor_statuses": ["proposed"],
            "human_review_performed": False,
            "accepted_german_unit_count": 0,
        },
        "provenance_ref": outputs["provenance"].as_posix(),
        "provenance_event_ref": config["event_id"],
        "map_version": 1,
        "supersedes_map_ref": None,
        "authority_boundary": AUTHORITY_BOUNDARY,
        "does_not_establish": DOES_NOT_ESTABLISH,
    }
    rendered_map = _render_json(map_payload)
    rendered_anchors = _render_jsonl(anchors)

    provenance_inputs = [
        {
            "ref": address_entry["file_id"],
            "role": "fixity-bound-source-page-address-witness",
            "sha256": address_entry["sha256"],
        },
        {
            "ref": address_binding["inventory"]["ref"],
            "role": "tracked-text-free-address-resource-inventory",
            "sha256": address_binding["inventory"]["sha256"],
        },
        {
            "ref": address_binding["rights"]["ref"],
            "role": "address-witness-rights-basis",
            "sha256": address_binding["rights"]["sha256"],
        },
    ]
    if navigation_entry["file_id"] != address_entry["file_id"]:
        provenance_inputs.extend(
            [
                {
                    "ref": navigation_entry["file_id"],
                    "role": "fixity-bound-navigation-only-provider-ocr",
                    "sha256": navigation_entry["sha256"],
                },
                {
                    "ref": navigation_binding["inventory"]["ref"],
                    "role": "tracked-text-free-navigation-resource-inventory",
                    "sha256": navigation_binding["inventory"]["sha256"],
                },
                {
                    "ref": navigation_binding["rights"]["ref"],
                    "role": "navigation-witness-rights-basis",
                    "sha256": navigation_binding["rights"]["sha256"],
                },
            ]
        )
    if work_boundary_input is not None:
        provenance_inputs.append(work_boundary_input)

    provenance = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": config["event_id"],
        "event_type": "segmentation",
        "started_at": event_at,
        "ended_at": event_at,
        "agent_refs": [
            "model:codex",
            "software:python-standard-library",
            f"software:poppler-{pdftotext_version}",
        ],
        "inputs": provenance_inputs,
        "outputs": [
            {
                "ref": outputs["map"].as_posix(),
                "role": "tracked-text-free-hierarchical-source-numbered-unit-page-map",
                "sha256": _sha256_bytes(rendered_map.encode("utf-8")),
            },
            {
                "ref": outputs["anchors"].as_posix(),
                "role": "tracked-proposed-whole-page-hierarchical-source-anchors",
                "sha256": _sha256_bytes(rendered_anchors.encode("utf-8")),
            },
        ],
        "method": {
            "maker_type": "mixed",
            "name": (
                "series-scoped-fixed-page-number-candidate-verification-plus-"
                "source-visible-gap-review"
            ),
            "version": "1",
            "artifact_digest": None,
            "runtime": (
                f"Python standard-library XML parser; Poppler pdftotext "
                f"{pdftotext_version}"
            ),
            "device": "abyss-machine",
            "configuration": {
                "represented_page_range": [
                    config["represented_start_page"],
                    config["represented_end_page"],
                ],
                "series_count": len(series_payloads),
                "numbered_unit_count": expected_total,
                "machine_number_candidate_count": total_machine,
                "source_visible_override_unit_count": len(override_unit_refs),
                "source_visible_reviewed_page_count": len(reviewed_pages),
                "navigation_offset": config["navigation_offset"],
                "navigation_relation_scope": config["navigation_relation_scope"],
                "human_repeat_performed": False,
                "source_text_included": False,
                "accepted_german_unit_count": 0,
                "source_to_target_text_compared": False,
            },
            "prompt_or_instruction_ref": (
                "ToS/doctrine/CORPUS_FOUNDATION.md#address-law"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            (
                f"All {expected_total} addresses are proposed series-qualified "
                "German source number-label start-page candidates, not exact "
                "line or passage-end boundaries."
            ),
            (
                "Embedded or provider OCR supplied disposable navigation only "
                "and was not accepted as German text."
            ),
            (
                f"The model visibly checked {len(reviewed_pages)} address pages "
                f"for {len(override_unit_refs)} OCR-gap labels; no human repeat exists."
            ),
            (
                "Any navigation/address page relation is bounded to the declared "
                "series evidence and does not establish whole-container page or "
                "textual identity."
            ),
            (
                "No source-to-target passage alignment, translation relation, "
                "accepted German, semantics, eligible target unit, target gold, "
                "publication route, human task, or canon effect was created."
            ),
        ],
        "receipt_refs": [outputs["map"].as_posix(), outputs["anchors"].as_posix()],
        "rights_basis_ref": address_binding["rights"]["ref"],
        "event_version": 1,
        "supersedes_event_ref": None,
    }
    return {
        outputs["map"]: rendered_map,
        outputs["anchors"]: rendered_anchors,
        outputs["provenance"]: _render_jsonl([provenance]),
    }


def build_outputs(
    *,
    repo_root: Path,
    payload_source_root: Path,
    event_at: str,
) -> dict[Path, str]:
    version = _pdftotext_version()
    outputs: dict[Path, str] = {}
    for config in WORK_CONFIGS:
        outputs.update(
            _build_one(
                repo_root=repo_root,
                payload_source_root=payload_source_root,
                config=config,
                event_at=event_at,
                pdftotext_version=version,
            )
        )
    return outputs


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Build text-free German source numbered-unit maps."
    )
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        required=True,
        help="Canonical local source-witness root containing gitignored payloads.",
    )
    parser.add_argument(
        "--event-at",
        default="2026-08-08T21:05:00-06:00",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    payload_source_root = args.payload_source_root.resolve()
    try:
        outputs = build_outputs(
            repo_root=repo_root,
            payload_source_root=payload_source_root,
            event_at=args.event_at,
        )
    except SourceStructureBuildError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    stale: list[str] = []
    for relative_path, rendered in outputs.items():
        path = repo_root / relative_path
        if args.check:
            if not path.is_file() or path.read_text(encoding="utf-8") != rendered:
                stale.append(relative_path.as_posix())
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(rendered, encoding="utf-8")

    if stale:
        for path in stale:
            print(f"[stale] {path}", file=sys.stderr)
        return 1
    if args.check:
        print("[ok] German source numbered-unit maps match exact local witnesses")
    else:
        print("[ok] built German source numbered-unit maps")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
