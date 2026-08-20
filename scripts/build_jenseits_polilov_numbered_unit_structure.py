#!/usr/bin/env python3
"""Build a text-free numbered-unit page map for the Mysl 1996 translation.

The exact local PDF's embedded ABBYY text layer is used only as disposable
navigation. Ordered numeral-label candidates are combined with an explicit
bounded set of model-visible page-review results. The tracked outputs contain
page starts and proposed whole-page anchors, never translation text or a
source-to-translation equivalence claim.
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
TARGET_ITEM_DIR = (
    SOURCE_ROOT
    / "collections/friedrich-nietzsche/"
    "works-in-two-volumes-volume-2-mysl-1996/"
    "editions/moscow-mysl-1996-volume-2/items/operator-pdf"
)
MANIFEST_PATH = TARGET_ITEM_DIR / "item.manifest.json"
INVENTORY_PATH = TARGET_ITEM_DIR / "resource-inventory.json"
RIGHTS_PATH = TARGET_ITEM_DIR / "rights.json"
WORK_BOUNDARY_PATH = (
    SOURCE_ROOT
    / "collections/friedrich-nietzsche/"
    "works-in-two-volumes-volume-2-mysl-1996/"
    "structure/work-boundaries/work-boundary-map.json"
)
SOURCE_MAP_PATH = (
    SOURCE_ROOT
    / "works/friedrich-nietzsche/jenseits-von-gut-und-boese/"
    "expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/"
    "items/internet-archive-google-harvard-scan-pdf/structure/"
    "numbered-unit-page-map.json"
)
OUTPUT_DIR = (
    SOURCE_ROOT
    / "works/friedrich-nietzsche/jenseits-von-gut-und-boese/"
    "expressions/ru-polilov-mysl-1996/structure/"
    "mysl-1996-volume-2-operator-pdf"
)
MAP_PATH = OUTPUT_DIR / "numbered-unit-page-map.json"
ANCHOR_PATH = OUTPUT_DIR / "numbered-unit-anchors.jsonl"
PROVENANCE_PATH = OUTPUT_DIR / "provenance.jsonl"
SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "target-numbered-unit-page-map.schema.json"
)
WORK_REF = "tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese"
EXPRESSION_REF = (
    "tos.expression.friedrich-nietzsche.jenseits-von-gut-und-boese."
    "ru-polilov-mysl-1996"
)
EDITION_REF = (
    "tos.edition.friedrich-nietzsche.works-in-two-volumes."
    "moscow-mysl-1996-volume-2"
)
MAP_ID = (
    "tos.target-numbered-unit-page-map.friedrich-nietzsche."
    "jenseits-von-gut-und-boese.ru-polilov-mysl-1996"
)
EVENT_ID = (
    "tos.event.target-numbered-unit-page-map.friedrich-nietzsche."
    "jenseits-von-gut-und-boese.ru-polilov-mysl-1996.2026-07-29"
)
SOURCE_MAP_SHA256 = (
    "fb7c15005968bb5795388714ab2b725593f5f29f31cb29c71cece77f751b2b90"
)
PDFTOTEXT_VERSION = "26.01.0"
WORK_START_PAGE = 238
WORK_END_PAGE = 406
AUTHORITY_BOUNDARY = (
    "model-reviewed target-visible numbered-label start-page candidates and "
    "proposed whole-page addresses for one exact translation scan only; no "
    "target text, exact line boundary, translation alignment, translation "
    "equivalence or quality, textual identity, semantics, rights clearance, "
    "or canon authority"
)
DOES_NOT_ESTABLISH = [
    "target_text",
    "exact_line_boundaries",
    "exact_passage_end_boundaries",
    "accepted_translation_text",
    "translation_correspondence",
    "translation_equivalence",
    "translation_quality",
    "textual_identity",
    "semantics",
    "rights_clearance",
    "canon_promotion",
]

# These pages are bounded judgments from model-visible review of the exact
# fixity-bound scan. They are not inferred by loosening the OCR filter.
PAGE_OVERRIDES = {
    "8": 246,
    "29": 265,
    "33": 268,
    "38": 271,
    "41": 273,
    "46": 278,
    "47": 279,
    "49": 281,
    "52": 282,
    "69": 292,
    "77": 293,
    "85": 294,
    "93": 295,
    "102": 296,
    "111": 297,
    "120": 298,
    "129": 299,
    "134": 299,
    "137": 300,
    "145": 301,
    "150": 301,
    "153": 302,
    "162": 303,
    "171": 304,
    "180": 305,
    "195": 315,
    "203": 322,
    "207": 328,
    "225": 346,
    "250": 369,
    "263": 386,
    "264": 389,
    "285": 399,
}
OCR_DISAMBIGUATION_KEYS = {"6"}
SUPPLEMENTAL_LABEL_KEYS = ["65a", "73a"]
XHTML_NAMESPACE = "{http://www.w3.org/1999/xhtml}"


class TargetNumberedUnitBuildError(RuntimeError):
    """Raised when the fixed local target route cannot be reproduced."""


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
        raise TargetNumberedUnitBuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise TargetNumberedUnitBuildError(f"JSON root is not an object: {path}")
    return payload


def _render_json(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _render_jsonl(payloads: list[dict[str, Any]]) -> str:
    return "".join(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"
        for payload in payloads
    )


def _unit_keys() -> list[str]:
    keys: list[str] = []
    for number in range(1, 297):
        keys.append(str(number))
        if number in {65, 73}:
            keys.append(f"{number}a")
    return keys


def _payload_path(
    *,
    repo_root: Path,
    payload_source_root: Path,
    manifest: dict[str, Any],
) -> tuple[dict[str, Any], Path]:
    entries = manifest.get("payload_files", [])
    if len(entries) != 1 or entries[0].get("media_type") != "application/pdf":
        raise TargetNumberedUnitBuildError("expected exactly one target PDF")
    entry = entries[0]
    relative_item_dir = (repo_root / MANIFEST_PATH).parent.relative_to(
        repo_root / SOURCE_ROOT
    )
    path = payload_source_root / relative_item_dir / entry["relative_path"]
    if not path.is_file():
        raise TargetNumberedUnitBuildError(f"local target PDF is missing: {path}")
    if _sha256_path(path) != entry["sha256"]:
        raise TargetNumberedUnitBuildError(
            f"local target PDF digest drifted: {path}"
        )
    return entry, path


def _pdftotext_version() -> str:
    result = subprocess.run(
        ["pdftotext", "-v"],
        check=False,
        capture_output=True,
        text=True,
    )
    output = f"{result.stdout}\n{result.stderr}"
    match = re.search(r"pdftotext version ([0-9.]+)", output)
    if result.returncode != 0 or match is None:
        raise TargetNumberedUnitBuildError(
            "pdftotext is unavailable or did not report its version"
        )
    version = match.group(1)
    if version != PDFTOTEXT_VERSION:
        raise TargetNumberedUnitBuildError(
            f"pdftotext version drifted: expected {PDFTOTEXT_VERSION}, got {version}"
        )
    return version


def _bbox_xhtml(pdf_path: Path) -> bytes:
    result = subprocess.run(
        [
            "pdftotext",
            "-f",
            str(WORK_START_PAGE),
            "-l",
            str(WORK_END_PAGE),
            "-bbox-layout",
            str(pdf_path),
            "-",
        ],
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        diagnostic = result.stderr.decode("utf-8", errors="replace").strip()
        raise TargetNumberedUnitBuildError(
            f"pdftotext bbox extraction failed: {diagnostic}"
        )
    if not result.stdout:
        raise TargetNumberedUnitBuildError("pdftotext emitted no bbox XHTML")
    return result.stdout


def _normalize_candidate(raw: str) -> str:
    compact = raw.strip().strip(".").strip()
    return compact.translate(
        str.maketrans(
            {
                "а": "a",
                "А": "a",
                "б": "6",
            }
        )
    )


def _bbox_candidates(xhtml: bytes) -> list[dict[str, Any]]:
    try:
        root = ET.fromstring(xhtml)
    except ET.ParseError as exc:
        raise TargetNumberedUnitBuildError(
            f"pdftotext bbox XHTML is invalid: {exc}"
        ) from exc
    pages = root.findall(f".//{XHTML_NAMESPACE}page")
    expected_page_count = WORK_END_PAGE - WORK_START_PAGE + 1
    if len(pages) != expected_page_count:
        raise TargetNumberedUnitBuildError(
            f"bbox page count drifted: expected {expected_page_count}, got {len(pages)}"
        )

    candidates: list[dict[str, Any]] = []
    for pdf_page, page in enumerate(pages, start=WORK_START_PAGE):
        for line in page.iter(f"{XHTML_NAMESPACE}line"):
            words = line.findall(f"{XHTML_NAMESPACE}word")
            if len(words) != 1:
                continue
            raw = "".join(words[0].itertext())
            stripped = raw.strip().strip(".").strip()
            x_min = float(line.get("xMin", "-1"))
            y_min = float(line.get("yMin", "-1"))
            if not (
                145 <= x_min <= 175
                and 40 <= y_min <= 490
                and 1 <= len(stripped) <= 5
            ):
                continue
            normalized = _normalize_candidate(raw)
            if re.fullmatch(r"(?:[1-9]|[1-9][0-9]|[12][0-9]{2})(?:a)?", normalized):
                candidates.append(
                    {
                        "page": pdf_page,
                        "normalized": normalized,
                        "raw": raw,
                    }
                )
    return candidates


def _ordered_candidate_matches(
    keys: list[str],
    candidates: list[dict[str, Any]],
) -> tuple[dict[str, int], list[str], dict[str, str]]:
    positions = {key: index for index, key in enumerate(keys)}
    next_index = 0
    matches: dict[str, int] = {}
    raw_matches: dict[str, str] = {}
    for candidate in candidates:
        normalized = candidate["normalized"]
        candidate_index = positions.get(normalized)
        if candidate_index is None or candidate_index < next_index:
            continue
        key = keys[candidate_index]
        matches[key] = candidate["page"]
        raw_matches[key] = candidate["raw"]
        next_index = candidate_index + 1
    skipped = [key for key in keys if key not in matches]
    return matches, skipped, raw_matches


def _anchor_id(unit_key: str) -> str:
    return (
        "tos.anchor.friedrich-nietzsche.jenseits-von-gut-und-boese."
        f"ru-polilov-mysl-1996.unit-{unit_key}.pdf-start-page"
    )


def _passage_id(unit_key: str) -> str:
    return (
        "tos.passage.friedrich-nietzsche.jenseits-von-gut-und-boese."
        f"ru-polilov-mysl-1996.unit-{unit_key}"
    )


def _basis(unit_key: str) -> str:
    if unit_key in PAGE_OVERRIDES:
        return "source_visible_gap_review"
    if unit_key in OCR_DISAMBIGUATION_KEYS:
        return "source_visible_ocr_disambiguation"
    return "embedded_pdf_bbox_order_candidate"


def build_outputs(
    *,
    repo_root: Path,
    payload_source_root: Path,
    event_at: str,
) -> tuple[str, str, str]:
    manifest = _read_json(repo_root / MANIFEST_PATH)
    inventory = _read_json(repo_root / INVENTORY_PATH)
    work_boundary = _read_json(repo_root / WORK_BOUNDARY_PATH)
    source_map = _read_json(repo_root / SOURCE_MAP_PATH)
    pdf_entry, pdf_path = _payload_path(
        repo_root=repo_root,
        payload_source_root=payload_source_root,
        manifest=manifest,
    )

    inventory_files = inventory.get("files", [])
    if len(inventory_files) != 1:
        raise TargetNumberedUnitBuildError("target inventory file set drifted")
    pdf_inventory = inventory_files[0]
    if (
        pdf_inventory.get("profile") != "pdf_pages_v1"
        or pdf_inventory.get("file_id") != pdf_entry["file_id"]
        or pdf_inventory.get("file_sha256") != pdf_entry["sha256"]
        or pdf_inventory.get("summary", {}).get("page_count") != 831
    ):
        raise TargetNumberedUnitBuildError(
            "target manifest/inventory PDF binding drifted"
        )

    members = [
        member
        for member in work_boundary.get("members", [])
        if member.get("sequence") == 2
        and member.get("work_ref") == WORK_REF
        and member.get("expression_ref") == EXPRESSION_REF
    ]
    if len(members) != 1:
        raise TargetNumberedUnitBuildError(
            "target work boundary does not resolve exactly once"
        )
    member = members[0]
    if (
        member.get("start_page") != WORK_START_PAGE
        or member.get("end_page") != WORK_END_PAGE
    ):
        raise TargetNumberedUnitBuildError("target work boundary pages drifted")

    source_map_digest = _sha256_path(repo_root / SOURCE_MAP_PATH)
    if source_map_digest != SOURCE_MAP_SHA256:
        raise TargetNumberedUnitBuildError("source numbered-unit map drifted")
    source_keys = {
        unit.get("unit_key") for unit in source_map.get("unit_starts", [])
    }
    if "237a" not in source_keys:
        raise TargetNumberedUnitBuildError(
            "source numbered-unit map no longer contains 237a"
        )

    version = _pdftotext_version()
    candidates = _bbox_candidates(_bbox_xhtml(pdf_path))
    keys = _unit_keys()
    machine_matches, skipped, raw_matches = _ordered_candidate_matches(
        keys,
        candidates,
    )
    if len(machine_matches) != 265 or set(skipped) != set(PAGE_OVERRIDES):
        raise TargetNumberedUnitBuildError(
            "ordered bbox route drifted: "
            f"expected 265 matches and {sorted(PAGE_OVERRIDES)}, "
            f"got {len(machine_matches)} matches and {skipped}"
        )
    if machine_matches.get("6") != 244 or raw_matches.get("6") != "б":
        raise TargetNumberedUnitBuildError(
            "the source-visible OCR disambiguation for unit 6 drifted"
        )
    if not all(key in machine_matches for key in SUPPLEMENTAL_LABEL_KEYS):
        raise TargetNumberedUnitBuildError(
            "target supplemental labels are absent from the ordered bbox route"
        )

    pages = {**machine_matches, **PAGE_OVERRIDES}
    if set(pages) != set(keys):
        raise TargetNumberedUnitBuildError("target numbered-unit coverage is incomplete")
    page_sequence = [pages[key] for key in keys]
    if page_sequence != sorted(page_sequence):
        raise TargetNumberedUnitBuildError(
            "target numbered-unit pages are not monotonic"
        )
    resources = {
        resource.get("resource_id"): resource
        for resource in pdf_inventory.get("resources", [])
    }
    for key, page in pages.items():
        resource = resources.get(f"pdf-page-{page:04d}")
        if resource is None or resource.get("locator", {}).get("page_index") != page:
            raise TargetNumberedUnitBuildError(
                f"target unit {key} leaves the PDF resource inventory"
            )

    unit_starts = [
        {
            "sequence": sequence,
            "unit_key": unit_key,
            "pdf_page": pages[unit_key],
            "resource_id": f"pdf-page-{pages[unit_key]:04d}",
            "anchor_ref": _anchor_id(unit_key),
            "basis": _basis(unit_key),
            "status": "proposed",
            "human_review_performed": False,
        }
        for sequence, unit_key in enumerate(keys, start=1)
    ]
    inventory_digest = _sha256_path(repo_root / INVENTORY_PATH)
    boundary_digest = _sha256_path(repo_root / WORK_BOUNDARY_PATH)
    map_payload = {
        "$schema": SCHEMA_REF,
        "schema_version": "tos_target_numbered_unit_page_map_v1",
        "map_id": MAP_ID,
        "work_ref": WORK_REF,
        "expression_ref": EXPRESSION_REF,
        "edition_ref": EDITION_REF,
        "item_ref": manifest["item_id"],
        "scan_file": {
            "file_ref": pdf_entry["file_id"],
            "file_sha256": pdf_entry["sha256"],
            "inventory_profile": "pdf_pages_v1",
        },
        "inventory": {
            "ref": INVENTORY_PATH.as_posix(),
            "sha256": inventory_digest,
        },
        "work_boundary": {
            "ref": WORK_BOUNDARY_PATH.as_posix(),
            "sha256": boundary_digest,
            "member_sequence": 2,
            "start_page": WORK_START_PAGE,
            "end_page": WORK_END_PAGE,
            "epistemic_status": member["epistemic_status"],
            "review_status": member["review_status"],
        },
        "map_authority": "model_reviewed_target_structure_candidate_only",
        "source_text_included": False,
        "method": {
            "name": (
                "ordered-embedded-pdf-bbox-candidate-plus-"
                "source-visible-gap-review"
            ),
            "version": "1",
            "maker_type": "mixed",
            "local_payloads_read": True,
            "navigation_tool": {
                "name": "pdftotext",
                "version": version,
                "mode": "bbox-layout",
            },
            "layer_trials": [
                {
                    "layer": "embedded_plain_text",
                    "result": (
                        "reading order, running heads, and damaged numeral "
                        "labels prevent a complete deterministic map"
                    ),
                    "selected_role": "rejected_as_complete_map_source",
                },
                {
                    "layer": "embedded_bbox_unordered_candidates",
                    "result": (
                        "page geometry narrows numeral candidates but admits "
                        "unrelated numbers without an order constraint"
                    ),
                    "selected_role": "rejected_without_order_constraint",
                },
                {
                    "layer": (
                        "embedded_bbox_ordered_candidates_plus_"
                        "source_visible_gap_review"
                    ),
                    "result": (
                        "265 ordered candidates plus 33 bounded visible "
                        "page judgments close the target label sequence"
                    ),
                    "selected_role": "selected_bounded_structure_route",
                },
            ],
            "candidate_filter": {
                "single_word_line_only": True,
                "x_min_points": {"minimum": 145, "maximum": 175},
                "y_min_points": {"minimum": 40, "maximum": 490},
                "maximum_stripped_character_count": 5,
                "normalizations": [
                    "trim surrounding whitespace and terminal periods",
                    "map Cyrillic а/А to structural suffix a",
                    "map the isolated OCR glyph Cyrillic б to candidate 6",
                ],
            },
            "ordered_bbox_candidate_matches": len(machine_matches),
            "source_visible_review": {
                "maker_type": "model",
                "human_repeat_performed": False,
                "gap_review_unit_keys": list(PAGE_OVERRIDES),
                "ocr_disambiguation_unit_keys": ["6"],
                "supplemental_label_unit_keys": SUPPLEMENTAL_LABEL_KEYS,
            },
            "cross_lingual_text_matching_used": False,
            "semantic_matching_used": False,
            "no_source_text_emitted": True,
        },
        "unit_starts": unit_starts,
        "numbering_asymmetries": [
            {
                "source_unit_key": "237a",
                "source_map_ref": SOURCE_MAP_PATH.as_posix(),
                "source_map_sha256": source_map_digest,
                "target_state": (
                    "corresponding_prose_present_without_repeated_unit_label"
                ),
                "target_numbered_unit_materialized": False,
                "exact_translation_alignment_claimed": False,
            }
        ],
        "summary": {
            "integer_numbered_unit_count": 296,
            "supplemental_numbered_units": SUPPLEMENTAL_LABEL_KEYS,
            "source_only_nonmaterialized_numbered_units": ["237a"],
            "numbered_unit_count": 298,
            "exact_start_page_candidates_materialized": 298,
            "unresolved_unit_count": 0,
            "start_pages_monotonic": True,
            "all_anchor_statuses": ["proposed"],
            "human_review_performed": False,
        },
        "provenance_ref": PROVENANCE_PATH.as_posix(),
        "provenance_event_ref": EVENT_ID,
        "map_version": 1,
        "supersedes_map_ref": None,
        "authority_boundary": AUTHORITY_BOUNDARY,
        "does_not_establish": DOES_NOT_ESTABLISH,
    }
    rendered_map = _render_json(map_payload)

    anchors = [
        {
            "schema_version": "tos_source_anchor_v1",
            "anchor_id": unit["anchor_ref"],
            "item_id": manifest["item_id"],
            "file_id": pdf_entry["file_id"],
            "file_sha256": pdf_entry["sha256"],
            "passage_id": _passage_id(unit["unit_key"]),
            "selectors": [
                {
                    "type": "structural",
                    "path": [
                        "work:jenseits-von-gut-und-boese",
                        "expression:ru-polilov-mysl-1996",
                        f"numbered-unit:{unit['unit_key']}",
                    ],
                    "scheme": "jgb-target-numbered-unit-start-v1",
                },
                {
                    "type": "page_region",
                    "page": unit["pdf_page"],
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
                    "ordered embedded-PDF bbox numeral candidate plus "
                    "bounded target-visible page review"
                ),
                "version": "1",
                "configuration_ref": (
                    f"{MAP_PATH.as_posix()}#unit-{unit['unit_key']}"
                ),
            },
            "status": "proposed",
            "provenance_event_ref": EVENT_ID,
            "anchor_version": 1,
            "supersedes_anchor_ref": None,
            "review_ref": None,
        }
        for unit in unit_starts
    ]
    rendered_anchors = _render_jsonl(anchors)
    provenance = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": EVENT_ID,
        "event_type": "segmentation",
        "started_at": event_at,
        "ended_at": event_at,
        "agent_refs": [
            "model:codex",
            "software:python-standard-library",
            f"software:poppler-{version}",
        ],
        "inputs": [
            {
                "ref": pdf_entry["file_id"],
                "role": "target-visible-translation-scan-witness",
                "sha256": pdf_entry["sha256"],
            },
            {
                "ref": INVENTORY_PATH.as_posix(),
                "role": "tracked-text-free-target-resource-inventory",
                "sha256": inventory_digest,
            },
            {
                "ref": WORK_BOUNDARY_PATH.as_posix(),
                "role": "tracked-target-work-boundary",
                "sha256": boundary_digest,
            },
            {
                "ref": SOURCE_MAP_PATH.as_posix(),
                "role": "numbering-asymmetry-reference-only",
                "sha256": source_map_digest,
            },
        ],
        "outputs": [
            {
                "ref": MAP_PATH.as_posix(),
                "role": "tracked-text-free-target-numbered-unit-page-map",
                "sha256": _sha256_bytes(rendered_map.encode("utf-8")),
            },
            {
                "ref": ANCHOR_PATH.as_posix(),
                "role": "tracked-proposed-whole-page-target-anchors",
                "sha256": _sha256_bytes(rendered_anchors.encode("utf-8")),
            },
        ],
        "method": {
            "maker_type": "mixed",
            "name": (
                "ordered-embedded-pdf-bbox-candidate-plus-"
                "source-visible-gap-review"
            ),
            "version": "1",
            "artifact_digest": None,
            "runtime": (
                f"Python standard library XML parser; Poppler pdftotext {version}"
            ),
            "device": "abyss-machine",
            "configuration": {
                "work_page_range": [WORK_START_PAGE, WORK_END_PAGE],
                "expected_integer_units": 296,
                "target_supplemental_numbered_units": SUPPLEMENTAL_LABEL_KEYS,
                "source_only_nonmaterialized_numbered_units": ["237a"],
                "ordered_bbox_candidate_matches": len(machine_matches),
                "source_visible_gap_review_count": len(PAGE_OVERRIDES),
                "source_visible_ocr_disambiguation_count": 1,
                "human_repeat_performed": False,
                "source_text_included": False,
                "cross_lingual_text_matching_used": False,
            },
            "prompt_or_instruction_ref": (
                "ToS/doctrine/CORPUS_FOUNDATION.md#address-law"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            (
                "All 298 addresses are proposed target numbered-label "
                "start-page candidates, not exact line or passage-end boundaries."
            ),
            (
                "The embedded PDF text layer supplied disposable navigation "
                "candidates only and was not accepted as Russian text."
            ),
            (
                "The scan-visible review was model-performed and has not been "
                "independently repeated by a human."
            ),
            (
                "The target visibly materializes labels 1-296, 65a, and 73a; "
                "source-only 237a remains an explicit nonmaterialized asymmetry."
            ),
            (
                "No source-to-target unit alignment, translation equivalence, "
                "translation quality, or semantic claim was made."
            ),
        ],
        "receipt_refs": [
            MAP_PATH.as_posix(),
            ANCHOR_PATH.as_posix(),
        ],
        "rights_basis_ref": RIGHTS_PATH.as_posix(),
        "event_version": 1,
        "supersedes_event_ref": None,
    }
    return rendered_map, rendered_anchors, _render_jsonl([provenance])


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Build the text-free Jenseits Polilov/Mysl 1996 "
            "numbered-unit page map."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        required=True,
        help="Source-witness root containing the exact local target PDF.",
    )
    parser.add_argument(
        "--event-at",
        default="2026-07-29T01:36:00-06:00",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    rendered_map, rendered_anchors, rendered_provenance = build_outputs(
        repo_root=repo_root,
        payload_source_root=args.payload_source_root.resolve(),
        event_at=args.event_at,
    )
    outputs = {
        repo_root / MAP_PATH: rendered_map,
        repo_root / ANCHOR_PATH: rendered_anchors,
        repo_root / PROVENANCE_PATH: rendered_provenance,
    }
    if args.check:
        drift = [
            path.relative_to(repo_root).as_posix()
            for path, rendered in outputs.items()
            if not path.is_file()
            or path.read_text(encoding="utf-8") != rendered
        ]
        if drift:
            for path in drift:
                print(f"[drift] {path}", file=sys.stderr)
            return 1
        print(
            "[ok] Jenseits Polilov/Mysl target numbered-unit map "
            "matches local inputs"
        )
        return 0

    for path, rendered in outputs.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(rendered, encoding="utf-8")
    print("[ok] wrote 298 proposed target numbered-unit start-page anchors")
    print(
        "[boundary] no target text, exact line boundaries, translation "
        "alignment, equivalence, quality, or semantics were emitted"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
