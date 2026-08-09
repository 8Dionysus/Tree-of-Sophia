#!/usr/bin/env python3
"""Build target-only numbered structure for frozen Mysl transfer candidates.

The exact local PDF's embedded text layer is used only to locate isolated
number labels by page geometry. Model-visible gap review supplies a fixed,
bounded set of missing labels. Tracked outputs contain series-qualified number
labels and page starts only: no source or target prose and no alignment claim.
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
TRANSFER_ROOT = (
    SOURCE_ROOT
    / "works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1"
)
TRANSFER_PLAN_PATH = TRANSFER_ROOT / "transfer-samples.json"
TRANSFER_ANCHOR_PATH = TRANSFER_ROOT / "transfer-target-anchors.v1.jsonl"
MAP_SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "hierarchical-target-numbered-unit-page-map.schema.json"
)
CROSSWALK_SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "transfer-candidate-target-structural-crosswalk.schema.json"
)
PDFTOTEXT_VERSION = "26.01.0"
XHTML_NAMESPACE = "{http://www.w3.org/1999/xhtml}"
MAP_AUTHORITY_BOUNDARY = (
    "machine-derived target-visible series-qualified numbered-label start-page "
    "candidates with bounded model-only review of explicit machine gaps, plus "
    "proposed whole-page addresses for one exact local translation scan only; "
    "repeated numeral labels remain distinct by series identity, and no target "
    "prose, exact line or passage-end boundary, accepted Russian, "
    "source-to-target alignment, translation relation, semantics, rights "
    "clearance, eligible transfer unit, target gold, or canon authority follows"
)
CROSSWALK_AUTHORITY_BOUNDARY = (
    "text-free target-side narrowing of already-frozen whole-page transfer "
    "candidates through one proposed hierarchical target numbered-unit map; "
    "no German parallel unit map is materialized, so the result establishes "
    "neither a source route nor source-to-target passage alignment, accepted "
    "German or Russian, translation correspondence, equivalence or quality, "
    "exact passage ends, eligible target units, target gold, semantic work, "
    "human work, rights clearance, or canon authority"
)
MAP_DOES_NOT_ESTABLISH = [
    "target_text",
    "exact_line_boundaries",
    "exact_passage_end_boundaries",
    "accepted_translation_text",
    "source_to_target_passage_alignment",
    "translation_correspondence",
    "translation_equivalence",
    "translation_quality",
    "textual_identity",
    "semantics",
    "rights_clearance",
    "eligible_target_unit",
    "target_gold",
    "canon_promotion",
]
CROSSWALK_DOES_NOT_ESTABLISH = [
    "source_text",
    "target_text",
    "exact_line_boundaries",
    "exact_passage_end_boundaries",
    "source_parallel_unit_route",
    "source_to_target_passage_alignment",
    "translation_correspondence",
    "translation_equivalence",
    "translation_quality",
    "textual_identity",
    "accepted_german",
    "accepted_russian",
    "target_gold",
    "eligible_target_unit",
    "semantics",
    "rights_clearance",
    "canon_promotion",
]


WORK_CONFIGS: tuple[dict[str, Any], ...] = (
    {
        "slug": "zur-genealogie-der-moral",
        "work_ref": "tos.work.friedrich-nietzsche.zur-genealogie-der-moral",
        "expression_ref": (
            "tos.expression.friedrich-nietzsche.zur-genealogie-der-moral."
            "ru-svasyan-mysl-1996"
        ),
        "member_sequence": 3,
        "start_page": 407,
        "end_page": 524,
        "map_id": (
            "tos.hierarchical-target-numbered-unit-page-map."
            "friedrich-nietzsche.zur-genealogie-der-moral."
            "ru-svasyan-mysl-1996"
        ),
        "map_event_id": (
            "tos.event.hierarchical-target-numbered-unit-page-map."
            "friedrich-nietzsche.zur-genealogie-der-moral."
            "ru-svasyan-mysl-1996.2026-08-08"
        ),
        "crosswalk_id": (
            "tos.transfer-candidate-target-crosswalk.friedrich-nietzsche."
            "zur-genealogie-der-moral.ru-svasyan-mysl-1996-v1"
        ),
        "crosswalk_event_id": (
            "tos.event.transfer-candidate-target-crosswalk."
            "friedrich-nietzsche.zur-genealogie-der-moral.2026-08-08"
        ),
        "output_dir": (
            SOURCE_ROOT
            / "works/friedrich-nietzsche/zur-genealogie-der-moral/"
            "expressions/ru-svasyan-mysl-1996/structure/"
            "mysl-1996-volume-2-operator-pdf"
        ),
        "series": (
            {
                "series_key": "preface",
                "series_kind": "numbered-preface",
                "start_page": 408,
                "end_page": 414,
                "expected_unit_count": 8,
                "overrides": {},
            },
            {
                "series_key": "essay-1",
                "series_kind": "numbered-essay",
                "start_page": 415,
                "end_page": 438,
                "expected_unit_count": 17,
                "overrides": {"5": 419, "8": 423, "9": 424},
            },
            {
                "series_key": "essay-2",
                "series_kind": "numbered-essay",
                "start_page": 439,
                "end_page": 471,
                "expected_unit_count": 25,
                "overrides": {"15": 460},
            },
            {
                "series_key": "essay-3",
                "series_kind": "numbered-essay",
                "start_page": 472,
                "end_page": 524,
                "expected_unit_count": 28,
                "overrides": {"3": 474, "18": 503, "28": 524},
            },
        ),
    },
    {
        "slug": "der-antichrist",
        "work_ref": "tos.work.friedrich-nietzsche.der-antichrist",
        "expression_ref": (
            "tos.expression.friedrich-nietzsche.der-antichrist."
            "ru-flerova-mysl-1996"
        ),
        "member_sequence": 6,
        "start_page": 631,
        "end_page": 692,
        "map_id": (
            "tos.hierarchical-target-numbered-unit-page-map."
            "friedrich-nietzsche.der-antichrist.ru-flerova-mysl-1996"
        ),
        "map_event_id": (
            "tos.event.hierarchical-target-numbered-unit-page-map."
            "friedrich-nietzsche.der-antichrist."
            "ru-flerova-mysl-1996.2026-08-08"
        ),
        "crosswalk_id": (
            "tos.transfer-candidate-target-crosswalk.friedrich-nietzsche."
            "der-antichrist.ru-flerova-mysl-1996-v1"
        ),
        "crosswalk_event_id": (
            "tos.event.transfer-candidate-target-crosswalk."
            "friedrich-nietzsche.der-antichrist.2026-08-08"
        ),
        "output_dir": (
            SOURCE_ROOT
            / "works/friedrich-nietzsche/der-antichrist/expressions/"
            "ru-flerova-mysl-1996/structure/"
            "mysl-1996-volume-2-operator-pdf"
        ),
        "series": (
            {
                "series_key": "main",
                "series_kind": "numbered-main-sequence",
                "start_page": 633,
                "end_page": 691,
                "expected_unit_count": 62,
                "overrides": {
                    "3": 634,
                    "16": 642,
                    "17": 643,
                    "18": 644,
                    "19": 644,
                    "23": 648,
                    "28": 654,
                    "36": 661,
                    "38": 662,
                    "45": 670,
                },
            },
        ),
    },
)


class TargetStructureBuildError(RuntimeError):
    """Raised when the frozen target structure cannot be reproduced."""


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
        raise TargetStructureBuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise TargetStructureBuildError(f"JSON root is not an object: {path}")
    return payload


def _render_json(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _render_jsonl(payloads: list[dict[str, Any]]) -> str:
    return "".join(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"
        for payload in payloads
    )


def _payload_path(
    *,
    repo_root: Path,
    payload_source_root: Path,
    manifest: dict[str, Any],
) -> tuple[dict[str, Any], Path]:
    entries = manifest.get("payload_files", [])
    if len(entries) != 1 or entries[0].get("media_type") != "application/pdf":
        raise TargetStructureBuildError("expected exactly one target PDF")
    entry = entries[0]
    relative_item_dir = (repo_root / MANIFEST_PATH).parent.relative_to(
        repo_root / SOURCE_ROOT
    )
    path = payload_source_root / relative_item_dir / entry["relative_path"]
    if not path.is_file():
        raise TargetStructureBuildError(f"local target PDF is missing: {path}")
    if _sha256_path(path) != entry["sha256"]:
        raise TargetStructureBuildError(f"local target PDF digest drifted: {path}")
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
        raise TargetStructureBuildError(
            "pdftotext is unavailable or did not report its version"
        )
    version = match.group(1)
    if version != PDFTOTEXT_VERSION:
        raise TargetStructureBuildError(
            f"pdftotext version drifted: expected {PDFTOTEXT_VERSION}, got {version}"
        )
    return version


def _bbox_candidates(
    pdf_path: Path,
    *,
    start_page: int,
    end_page: int,
) -> list[dict[str, Any]]:
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
        raise TargetStructureBuildError(
            f"pdftotext bbox extraction failed: {diagnostic}"
        )
    try:
        root = ET.fromstring(result.stdout)
    except ET.ParseError as exc:
        raise TargetStructureBuildError(
            f"pdftotext bbox XHTML is invalid: {exc}"
        ) from exc
    pages = root.findall(f".//{XHTML_NAMESPACE}page")
    expected_page_count = end_page - start_page + 1
    if len(pages) != expected_page_count:
        raise TargetStructureBuildError(
            f"bbox page count drifted: expected {expected_page_count}, got {len(pages)}"
        )

    candidates: list[dict[str, Any]] = []
    for pdf_page, page in enumerate(pages, start=start_page):
        for line in page.iter(f"{XHTML_NAMESPACE}line"):
            words = line.findall(f"{XHTML_NAMESPACE}word")
            if len(words) != 1:
                continue
            raw = "".join(words[0].itertext()).strip()
            normalized = raw[:-1] if raw.endswith(".") else raw
            x_min = float(line.get("xMin", "-1"))
            y_min = float(line.get("yMin", "-1"))
            if not (
                145 <= x_min <= 175
                and 40 <= y_min <= 490
                and len(normalized) <= 3
                and re.fullmatch(r"[1-9][0-9]*", normalized)
            ):
                continue
            candidates.append({"page": pdf_page, "unit_key": normalized})
    return candidates


def _ordered_matches(
    *,
    expected_unit_count: int,
    candidates: list[dict[str, Any]],
) -> tuple[dict[str, int], list[str]]:
    keys = [str(number) for number in range(1, expected_unit_count + 1)]
    positions = {key: index for index, key in enumerate(keys)}
    next_index = 0
    matches: dict[str, int] = {}
    for candidate in candidates:
        key = candidate["unit_key"]
        candidate_index = positions.get(key)
        if candidate_index is None or candidate_index < next_index:
            continue
        matches[key] = candidate["page"]
        next_index = candidate_index + 1
    skipped = [key for key in keys if key not in matches]
    return matches, skipped


def _unit_ref(series_key: str, unit_key: str) -> str:
    return f"{series_key}:{unit_key}"


def _anchor_id(config: dict[str, Any], series_key: str, unit_key: str) -> str:
    return (
        f"tos.anchor.friedrich-nietzsche.{config['slug']}."
        f"{config['expression_ref'].split('.')[-1]}."
        f"{series_key}.unit-{unit_key}.pdf-start-page"
    )


def _passage_id(config: dict[str, Any], series_key: str, unit_key: str) -> str:
    return (
        f"tos.passage.friedrich-nietzsche.{config['slug']}."
        f"{config['expression_ref'].split('.')[-1]}."
        f"{series_key}.unit-{unit_key}"
    )


def _output_paths(config: dict[str, Any]) -> dict[str, Path]:
    output_dir = config["output_dir"]
    return {
        "map": output_dir / "hierarchical-numbered-unit-page-map.json",
        "anchors": output_dir / "hierarchical-numbered-unit-anchors.jsonl",
        "map_provenance": output_dir / "provenance.numbered-unit-page-map.jsonl",
        "crosswalk": output_dir / "transfer-candidate-page-crosswalk.v1.json",
        "crosswalk_provenance": (
            output_dir / "provenance.transfer-candidate-page-crosswalk.jsonl"
        ),
    }


def _build_map_outputs(
    *,
    repo_root: Path,
    config: dict[str, Any],
    pdf_entry: dict[str, Any],
    pdf_path: Path,
    manifest: dict[str, Any],
    inventory: dict[str, Any],
    work_boundary: dict[str, Any],
    version: str,
    event_at: str,
) -> tuple[dict[str, Any], str, str]:
    members = [
        member
        for member in work_boundary.get("members", [])
        if member.get("sequence") == config["member_sequence"]
        and member.get("work_ref") == config["work_ref"]
        and member.get("expression_ref") == config["expression_ref"]
    ]
    if len(members) != 1:
        raise TargetStructureBuildError(
            f"{config['slug']} work boundary does not resolve exactly once"
        )
    member = members[0]
    if (
        member.get("start_page") != config["start_page"]
        or member.get("end_page") != config["end_page"]
    ):
        raise TargetStructureBuildError(
            f"{config['slug']} target work boundary pages drifted"
        )

    inventory_files = inventory.get("files", [])
    if len(inventory_files) != 1:
        raise TargetStructureBuildError("target inventory file set drifted")
    pdf_inventory = inventory_files[0]
    if (
        pdf_inventory.get("profile") != "pdf_pages_v1"
        or pdf_inventory.get("file_id") != pdf_entry["file_id"]
        or pdf_inventory.get("file_sha256") != pdf_entry["sha256"]
        or pdf_inventory.get("summary", {}).get("page_count") != 831
    ):
        raise TargetStructureBuildError(
            "target manifest/inventory PDF binding drifted"
        )
    resources = {
        resource.get("resource_id"): resource
        for resource in pdf_inventory.get("resources", [])
    }
    candidates = _bbox_candidates(
        pdf_path,
        start_page=config["start_page"],
        end_page=config["end_page"],
    )
    output_paths = _output_paths(config)
    series_payloads: list[dict[str, Any]] = []
    anchors: list[dict[str, Any]] = []
    series_trials: list[dict[str, Any]] = []
    override_unit_refs: list[str] = []
    reviewed_pages: set[int] = set()
    total_machine_matches = 0

    for series_sequence, series in enumerate(config["series"], start=1):
        series_candidates = [
            candidate
            for candidate in candidates
            if series["start_page"] <= candidate["page"] <= series["end_page"]
        ]
        machine_matches, skipped = _ordered_matches(
            expected_unit_count=series["expected_unit_count"],
            candidates=series_candidates,
        )
        overrides = series["overrides"]
        if set(skipped) != set(overrides):
            raise TargetStructureBuildError(
                f"{config['slug']} {series['series_key']} bbox gap drifted: "
                f"expected {sorted(overrides)}, got {skipped}"
            )
        pages = {**machine_matches, **overrides}
        expected_keys = {
            str(number) for number in range(1, series["expected_unit_count"] + 1)
        }
        if set(pages) != expected_keys:
            raise TargetStructureBuildError(
                f"{config['slug']} {series['series_key']} coverage is incomplete"
            )
        page_sequence = [
            pages[str(number)]
            for number in range(1, series["expected_unit_count"] + 1)
        ]
        if page_sequence != sorted(page_sequence):
            raise TargetStructureBuildError(
                f"{config['slug']} {series['series_key']} pages are not monotonic"
            )

        unit_starts: list[dict[str, Any]] = []
        for sequence in range(1, series["expected_unit_count"] + 1):
            unit_key = str(sequence)
            page = pages[unit_key]
            resource_id = f"pdf-page-{page:04d}"
            resource = resources.get(resource_id)
            if (
                resource is None
                or resource.get("locator", {}).get("page_index") != page
            ):
                raise TargetStructureBuildError(
                    f"{config['slug']} {_unit_ref(series['series_key'], unit_key)} "
                    "leaves the PDF resource inventory"
                )
            anchor_ref = _anchor_id(config, series["series_key"], unit_key)
            basis = (
                "source_visible_gap_review"
                if unit_key in overrides
                else "embedded_pdf_bbox_order_candidate"
            )
            unit_starts.append(
                {
                    "sequence": sequence,
                    "unit_key": unit_key,
                    "pdf_page": page,
                    "resource_id": resource_id,
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
                    "item_id": manifest["item_id"],
                    "file_id": pdf_entry["file_id"],
                    "file_sha256": pdf_entry["sha256"],
                    "passage_id": _passage_id(
                        config,
                        series["series_key"],
                        unit_key,
                    ),
                    "selectors": [
                        {
                            "type": "structural",
                            "path": [
                                f"work:{config['slug']}",
                                (
                                    "expression:"
                                    f"{config['expression_ref'].split('.')[-1]}"
                                ),
                                f"series:{series['series_key']}",
                                f"numbered-unit:{unit_key}",
                            ],
                            "scheme": (
                                "tos-hierarchical-target-numbered-unit-start-v1"
                            ),
                        },
                        {
                            "type": "page_region",
                            "page": page,
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
                            "series-scoped ordered embedded-PDF bbox numeral "
                            "candidate plus bounded target-visible gap review"
                        ),
                        "version": "1",
                        "configuration_ref": (
                            f"{output_paths['map'].as_posix()}#"
                            f"{series['series_key']}-{unit_key}"
                        ),
                    },
                    "status": "proposed",
                    "provenance_event_ref": config["map_event_id"],
                    "anchor_version": 1,
                    "supersedes_anchor_ref": None,
                    "review_ref": None,
                }
            )

        series_payloads.append(
            {
                "series_key": series["series_key"],
                "series_sequence": series_sequence,
                "series_kind": series["series_kind"],
                "start_page": series["start_page"],
                "end_page": series["end_page"],
                "expected_unit_count": series["expected_unit_count"],
                "status": "proposed",
                "unit_starts": unit_starts,
            }
        )
        series_trials.append(
            {
                "series_key": series["series_key"],
                "expected_unit_count": series["expected_unit_count"],
                "ordered_bbox_candidate_match_count": len(machine_matches),
                "source_visible_override_unit_keys": list(overrides),
            }
        )
        total_machine_matches += len(machine_matches)
        for unit_key, page in overrides.items():
            override_unit_refs.append(_unit_ref(series["series_key"], unit_key))
            reviewed_pages.add(page)

    inventory_digest = _sha256_path(repo_root / INVENTORY_PATH)
    boundary_digest = _sha256_path(repo_root / WORK_BOUNDARY_PATH)
    rights_digest = _sha256_path(repo_root / RIGHTS_PATH)
    total_units = sum(
        series["expected_unit_count"] for series in config["series"]
    )
    map_payload = {
        "$schema": MAP_SCHEMA_REF,
        "schema_version": "tos_hierarchical_target_numbered_unit_page_map_v1",
        "map_id": config["map_id"],
        "work_ref": config["work_ref"],
        "expression_ref": config["expression_ref"],
        "edition_ref": manifest["embodiment_ref"],
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
            "member_sequence": config["member_sequence"],
            "start_page": config["start_page"],
            "end_page": config["end_page"],
            "epistemic_status": member["epistemic_status"],
            "review_status": member["review_status"],
        },
        "map_authority": (
            "machine_candidates_with_bounded_model_gap_review_only"
        ),
        "target_text_included": False,
        "method": {
            "name": (
                "series-scoped-ordered-embedded-pdf-bbox-candidate-plus-"
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
            "candidate_filter": {
                "single_word_line_only": True,
                "x_min_points": {"minimum": 145, "maximum": 175},
                "y_min_points": {"minimum": 40, "maximum": 490},
                "maximum_stripped_character_count": 3,
                "accepted_pattern": "positive integer with optional terminal period",
                "normalizations": [
                    "trim surrounding whitespace",
                    "remove one terminal period",
                ],
            },
            "series_trials": series_trials,
            "source_visible_review": {
                "maker_type": "model",
                "human_repeat_performed": False,
                "reviewed_target_pdf_pages": sorted(reviewed_pages),
                "override_unit_refs": override_unit_refs,
            },
            "cross_lingual_text_matching_used": False,
            "semantic_matching_used": False,
            "no_target_text_emitted": True,
        },
        "series": series_payloads,
        "summary": {
            "series_count": len(series_payloads),
            "numbered_unit_count": total_units,
            "ordered_bbox_candidate_match_count": total_machine_matches,
            "source_visible_override_unit_count": len(override_unit_refs),
            "source_visible_reviewed_page_count": len(reviewed_pages),
            "exact_start_page_candidates_materialized": total_units,
            "unresolved_unit_count": 0,
            "start_pages_monotonic": True,
            "all_anchor_statuses": ["proposed"],
            "human_review_performed": False,
        },
        "provenance_ref": output_paths["map_provenance"].as_posix(),
        "provenance_event_ref": config["map_event_id"],
        "map_version": 1,
        "supersedes_map_ref": None,
        "authority_boundary": MAP_AUTHORITY_BOUNDARY,
        "does_not_establish": MAP_DOES_NOT_ESTABLISH,
    }
    rendered_map = _render_json(map_payload)
    rendered_anchors = _render_jsonl(anchors)
    provenance = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": config["map_event_id"],
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
                "role": "target-visible-local-translation-scan-witness",
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
                "ref": RIGHTS_PATH.as_posix(),
                "role": "target-rights-basis",
                "sha256": rights_digest,
            },
        ],
        "outputs": [
            {
                "ref": output_paths["map"].as_posix(),
                "role": "tracked-text-free-hierarchical-target-numbered-unit-page-map",
                "sha256": _sha256_bytes(rendered_map.encode("utf-8")),
            },
            {
                "ref": output_paths["anchors"].as_posix(),
                "role": "tracked-proposed-whole-page-hierarchical-target-anchors",
                "sha256": _sha256_bytes(rendered_anchors.encode("utf-8")),
            },
        ],
        "method": {
            "maker_type": "mixed",
            "name": (
                "series-scoped-ordered-embedded-pdf-bbox-candidate-plus-"
                "source-visible-gap-review"
            ),
            "version": "1",
            "artifact_digest": None,
            "runtime": (
                f"Python standard library XML parser; Poppler pdftotext {version}"
            ),
            "device": "abyss-machine",
            "configuration": {
                "work_page_range": [config["start_page"], config["end_page"]],
                "series_count": len(series_payloads),
                "numbered_unit_count": total_units,
                "ordered_bbox_candidate_match_count": total_machine_matches,
                "source_visible_override_unit_count": len(override_unit_refs),
                "source_visible_reviewed_page_count": len(reviewed_pages),
                "human_repeat_performed": False,
                "target_text_included": False,
                "cross_lingual_text_matching_used": False,
            },
            "prompt_or_instruction_ref": (
                "ToS/doctrine/CORPUS_FOUNDATION.md#address-law"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            (
                f"All {total_units} addresses are proposed series-qualified "
                "target numbered-label start-page candidates, not exact line "
                "or passage-end boundaries."
            ),
            (
                "The embedded PDF layer supplied disposable navigation only "
                "and was not accepted as Russian text."
            ),
            (
                f"The model visibly checked {len(reviewed_pages)} pages for "
                f"{len(override_unit_refs)} OCR-gap labels; no human repeat exists."
            ),
            (
                "Repeated numeral labels are disambiguated only by proposed "
                "series identity; no German parallel unit map was created."
            ),
            (
                "No source-to-target alignment, translation relation, semantic "
                "claim, eligible target unit, target gold, or publication route "
                "was created."
            ),
        ],
        "receipt_refs": [
            output_paths["map"].as_posix(),
            output_paths["anchors"].as_posix(),
        ],
        "rights_basis_ref": RIGHTS_PATH.as_posix(),
        "event_version": 1,
        "supersedes_event_ref": None,
    }
    return map_payload, rendered_anchors, _render_jsonl([provenance])


def _flatten_starts(map_payload: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {
            "target_unit_ref": _unit_ref(series["series_key"], unit["unit_key"]),
            "pdf_page": unit["pdf_page"],
        }
        for series in map_payload["series"]
        for unit in series["unit_starts"]
    ]


def _build_crosswalk_outputs(
    *,
    repo_root: Path,
    config: dict[str, Any],
    map_payload: dict[str, Any],
    rendered_map: str,
    transfer_plan: dict[str, Any],
    manifest: dict[str, Any],
    event_at: str,
) -> tuple[str, str]:
    starts = _flatten_starts(map_payload)
    candidates = [
        candidate
        for candidate in transfer_plan.get("candidate_target_units", [])
        if candidate.get("work_ref") == config["work_ref"]
    ]
    if len(candidates) != 6:
        raise TargetStructureBuildError(
            f"{config['slug']} frozen transfer quota drifted: {len(candidates)}"
        )
    crosswalk_candidates: list[dict[str, Any]] = []
    possible_route_count = 0
    pages_with_starts = 0
    for candidate in candidates:
        page = candidate["page"]
        prior = [start for start in starts if start["pdf_page"] < page]
        on_page = [start for start in starts if start["pdf_page"] == page]
        following = [start for start in starts if start["pdf_page"] > page]
        if not prior or not following:
            raise TargetStructureBuildError(
                f"{candidate['unit_id']} lacks bounded surrounding starts"
            )
        possible_refs = [prior[-1]["target_unit_ref"]] + [
            start["target_unit_ref"] for start in on_page
        ]
        crosswalk_candidates.append(
            {
                "candidate_unit_id": candidate["unit_id"],
                "candidate_anchor_ref": candidate["anchor_ref"],
                "target_pdf_page": page,
                "stratum": candidate["stratum"],
                "page_relation": (
                    "prior-target-unit-spill-plus-unit-starts"
                    if on_page
                    else "within-one-proposed-target-numbered-unit"
                ),
                "possible_target_unit_refs": possible_refs,
                "starts_on_page_target_unit_refs": [
                    start["target_unit_ref"] for start in on_page
                ],
                "next_proposed_start": {
                    "target_unit_ref": following[0]["target_unit_ref"],
                    "target_pdf_page": following[0]["pdf_page"],
                },
                "source_parallel_route_status": "not_materialized",
                "exact_page_end_boundary_known": False,
                "eligible_for_variant_execution": False,
                "target_gold_status": "not_started",
            }
        )
        possible_route_count += len(possible_refs)
        pages_with_starts += bool(on_page)

    output_paths = _output_paths(config)
    crosswalk_payload = {
        "$schema": CROSSWALK_SCHEMA_REF,
        "schema_version": "tos_transfer_candidate_target_structural_crosswalk_v1",
        "crosswalk_id": config["crosswalk_id"],
        "status": "prepared-target-structural-only-ineligible",
        "work_ref": config["work_ref"],
        "target_expression_ref": config["expression_ref"],
        "target_item_ref": manifest["item_id"],
        "inputs": {
            "transfer_plan": {
                "ref": TRANSFER_PLAN_PATH.as_posix(),
                "sha256": _sha256_path(repo_root / TRANSFER_PLAN_PATH),
            },
            "candidate_anchor_set": {
                "ref": TRANSFER_ANCHOR_PATH.as_posix(),
                "sha256": _sha256_path(repo_root / TRANSFER_ANCHOR_PATH),
            },
            "target_hierarchical_numbered_unit_map": {
                "ref": output_paths["map"].as_posix(),
                "sha256": _sha256_bytes(rendered_map.encode("utf-8")),
            },
            "target_rights": {
                "ref": RIGHTS_PATH.as_posix(),
                "sha256": _sha256_path(repo_root / RIGHTS_PATH),
            },
        },
        "method": {
            "name": (
                "tracked-target-page-to-hierarchical-proposed-unit-start-"
                "crosswalk"
            ),
            "version": "1",
            "selection_frame_changed": False,
            "source_or_target_text_read": False,
            "page_intersection_rule": (
                "preceding proposed target unit start plus every proposed "
                "target unit start on the candidate page"
            ),
            "source_parallel_unit_map_available": False,
        },
        "summary": {
            "candidate_page_count": len(candidates),
            "random_page_count": sum(
                candidate["stratum"] == "random" for candidate in candidates
            ),
            "hard_page_count": sum(
                candidate["stratum"] == "hard" for candidate in candidates
            ),
            "page_with_unit_start_count": int(pages_with_starts),
            "page_without_unit_start_count": (
                len(candidates) - int(pages_with_starts)
            ),
            "possible_target_unit_route_count": possible_route_count,
            "source_parallel_route_count": 0,
            "human_review_count": 0,
            "eligible_target_unit_count": 0,
            "target_gold_count": 0,
        },
        "candidates": crosswalk_candidates,
        "source_text_included": False,
        "target_text_included": False,
        "effects": {
            "candidate_frame_changed": False,
            "exact_passage_boundary_created": False,
            "source_text_accepted": False,
            "target_text_accepted": False,
            "source_to_target_alignment_created": False,
            "translation_alignment_created": False,
            "target_unit_eligible": False,
            "target_gold_created": False,
            "semantic_work_opened": False,
            "human_work_scheduled": False,
        },
        "provenance_event_ref": config["crosswalk_event_id"],
        "does_not_establish": CROSSWALK_DOES_NOT_ESTABLISH,
        "authority_boundary": CROSSWALK_AUTHORITY_BOUNDARY,
    }
    rendered_crosswalk = _render_json(crosswalk_payload)
    provenance = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": config["crosswalk_event_id"],
        "event_type": "alignment",
        "started_at": event_at,
        "ended_at": event_at,
        "agent_refs": ["software:python-standard-library"],
        "inputs": [
            {
                "ref": binding["ref"],
                "role": role,
                "sha256": binding["sha256"],
            }
            for role, binding in (
                (
                    "frozen-golden-kernel-transfer-candidate-plan",
                    crosswalk_payload["inputs"]["transfer_plan"],
                ),
                (
                    "frozen-target-page-anchor-set",
                    crosswalk_payload["inputs"]["candidate_anchor_set"],
                ),
                (
                    "tracked-hierarchical-target-numbered-unit-page-map",
                    crosswalk_payload["inputs"][
                        "target_hierarchical_numbered_unit_map"
                    ],
                ),
                (
                    "target-rights-basis",
                    crosswalk_payload["inputs"]["target_rights"],
                ),
            )
        ],
        "outputs": [
            {
                "ref": output_paths["crosswalk"].as_posix(),
                "role": "tracked-text-free-target-only-transfer-candidate-crosswalk",
                "sha256": _sha256_bytes(rendered_crosswalk.encode("utf-8")),
            }
        ],
        "method": {
            "maker_type": "software",
            "name": "tracked-target-hierarchical-page-crosswalk",
            "version": "1",
            "artifact_digest": None,
            "runtime": "Python standard library",
            "device": None,
            "configuration": {
                "candidate_page_count": len(candidates),
                "possible_target_unit_route_count": possible_route_count,
                "page_with_proposed_unit_starts_count": int(pages_with_starts),
                "page_without_proposed_unit_starts_count": (
                    len(candidates) - int(pages_with_starts)
                ),
                "local_payloads_read": False,
                "source_text_read": False,
                "target_text_read": False,
                "selection_frame_changed": False,
                "source_parallel_unit_map_available": False,
                "source_parallel_route_count": 0,
                "exact_passage_end_boundaries_materialized": False,
                "translation_alignment_inferred": False,
                "eligible_target_unit_count": 0,
                "target_gold_count": 0,
                "human_review_count": 0,
            },
            "prompt_or_instruction_ref": (
                "ToS/doctrine/CORPUS_FOUNDATION.md#address-law"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            (
                f"The crosswalk narrows only six frozen {config['slug']} "
                f"target pages to {possible_route_count} possible target-side "
                "series-qualified unit routes."
            ),
            (
                "No German parallel numbered-unit map exists in this route; "
                "source parallel route count is explicitly zero."
            ),
            (
                "No source or target text was read, compared, transcribed, "
                "aligned, or accepted by the crosswalk event."
            ),
            (
                "Proposed target starts do not establish exact passage ends, "
                "translation correspondence, equivalence, quality, semantics, "
                "rights clearance, eligibility, target gold, or canon authority."
            ),
            "No human work was requested or scheduled by this event.",
        ],
        "receipt_refs": [output_paths["crosswalk"].as_posix()],
        "rights_basis_ref": None,
        "event_version": 1,
        "supersedes_event_ref": None,
    }
    return rendered_crosswalk, _render_jsonl([provenance])


def build_outputs(
    *,
    repo_root: Path,
    payload_source_root: Path,
    event_at: str,
) -> dict[Path, str]:
    manifest = _read_json(repo_root / MANIFEST_PATH)
    inventory = _read_json(repo_root / INVENTORY_PATH)
    work_boundary = _read_json(repo_root / WORK_BOUNDARY_PATH)
    transfer_plan = _read_json(repo_root / TRANSFER_PLAN_PATH)
    pdf_entry, pdf_path = _payload_path(
        repo_root=repo_root,
        payload_source_root=payload_source_root,
        manifest=manifest,
    )
    version = _pdftotext_version()
    outputs: dict[Path, str] = {}
    for config in WORK_CONFIGS:
        map_payload, rendered_anchors, rendered_map_provenance = (
            _build_map_outputs(
                repo_root=repo_root,
                config=config,
                pdf_entry=pdf_entry,
                pdf_path=pdf_path,
                manifest=manifest,
                inventory=inventory,
                work_boundary=work_boundary,
                version=version,
                event_at=event_at,
            )
        )
        output_paths = _output_paths(config)
        rendered_map = _render_json(map_payload)
        rendered_crosswalk, rendered_crosswalk_provenance = (
            _build_crosswalk_outputs(
                repo_root=repo_root,
                config=config,
                map_payload=map_payload,
                rendered_map=rendered_map,
                transfer_plan=transfer_plan,
                manifest=manifest,
                event_at=event_at,
            )
        )
        outputs.update(
            {
                output_paths["map"]: rendered_map,
                output_paths["anchors"]: rendered_anchors,
                output_paths["map_provenance"]: rendered_map_provenance,
                output_paths["crosswalk"]: rendered_crosswalk,
                output_paths["crosswalk_provenance"]: (
                    rendered_crosswalk_provenance
                ),
            }
        )
    return outputs


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Build hierarchical target numbered-unit maps and target-only "
            "crosswalks for the frozen Mysl transfer candidates."
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
        default="2026-08-08T21:10:00-06:00",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    outputs = build_outputs(
        repo_root=repo_root,
        payload_source_root=args.payload_source_root.resolve(),
        event_at=args.event_at,
    )
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
            "[ok] Mysl Genealogie/Antichrist target structure and frozen "
            "candidate crosswalks match local inputs"
        )
        return 0

    for path, rendered in outputs.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(rendered, encoding="utf-8")
    print("[ok] wrote 140 proposed series-qualified target unit starts")
    print("[ok] narrowed 12 frozen target pages to 20 target-only unit routes")
    print(
        "[boundary] no prose, German parallel route, passage alignment, "
        "translation authority, eligible target unit, target gold, semantics, "
        "rights clearance, or human work was emitted"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
