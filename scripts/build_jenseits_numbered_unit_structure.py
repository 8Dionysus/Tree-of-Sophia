#!/usr/bin/env python3
"""Build the text-free numbered-unit page map for the Naumann 1886 scan.

The builder uses the exact provider ABBYY XML only to propose an ordered
number sequence, then applies a bounded, explicit set of model-visible scan
review results for OCR gaps and disambiguations. It emits page starts and
whole-page proposed anchors, never OCR strings or accepted German text.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = Path("ToS/source-witnesses")
ITEM_DIR = (
    SOURCE_ROOT
    / "works/friedrich-nietzsche/jenseits-von-gut-und-boese/"
    "expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/"
    "items/internet-archive-google-harvard-scan-pdf"
)
MANIFEST_PATH = ITEM_DIR / "item.manifest.json"
INVENTORY_PATH = ITEM_DIR / "resource-inventory.json"
OUTPUT_DIR = ITEM_DIR / "structure"
MAP_PATH = OUTPUT_DIR / "numbered-unit-page-map.json"
ANCHOR_PATH = OUTPUT_DIR / "numbered-unit-anchors.jsonl"
PROVENANCE_PATH = OUTPUT_DIR / "provenance.jsonl"
SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "numbered-unit-page-map.schema.json"
)
WORK_REF = "tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese"
EXPRESSION_REF = (
    "tos.expression.friedrich-nietzsche.jenseits-von-gut-und-boese."
    "de-naumann-1886"
)
EDITION_REF = (
    "tos.edition.friedrich-nietzsche.jenseits-von-gut-und-boese."
    "leipzig-c-g-naumann-1886"
)
MAP_ID = (
    "tos.numbered-unit-page-map.friedrich-nietzsche."
    "jenseits-von-gut-und-boese.de-naumann-1886"
)
EVENT_ID = (
    "tos.event.numbered-unit-page-map.friedrich-nietzsche."
    "jenseits-von-gut-und-boese.de-naumann-1886.2026-07-29"
)
RIGHTS_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/expressions/de-naumann-1886/"
    "editions/leipzig-c-g-naumann-1886/items/"
    "internet-archive-google-harvard-scan-pdf/rights.json"
)
AUTHORITY_BOUNDARY = (
    "model-reviewed source-visible numbered-unit start-page candidates and "
    "proposed whole-page addresses for one exact scan only; no source text, "
    "exact line boundary, textual acceptance, critical equivalence, "
    "translation correspondence, semantics, rights clearance, or canon authority"
)
DOES_NOT_ESTABLISH = [
    "source_text",
    "exact_line_boundaries",
    "exact_passage_end_boundaries",
    "accepted_original_language_text",
    "critical_edition_equivalence",
    "translation_correspondence",
    "translation_equivalence",
    "translation_quality",
    "semantics",
    "rights_clearance",
    "canon_promotion",
]
ABBYY_NS = (
    "{http://www.abbyy.com/FineReader_xml/FineReader6-schema-v1.xml}"
)
OCR_ALLOWED_CHARACTERS = set(
    "0123456789IiLlOoZzAaSsGgQqÖöÜüTt»«^*.,:;-—– •~/\\"
)
OCR_SUBSTITUTIONS = {
    "0": set("0oaöü"),
    "1": set("1ilt»"),
    "2": set("2z"),
    "3": set("3"),
    "4": set("4"),
    "5": set("5s"),
    "6": set("6"),
    "7": set("7"),
    "8": set("8ös"),
    "9": set("9gq"),
}

# These pages are judgments from the bounded model-visible review of the exact
# scan, not values inferred by making the alignment algorithm more permissive.
PAGE_OVERRIDES = {
    "15": 27,
    "18": 30,
    "23": 37,
    "24": 42,
    "30": 50,
    "32": 52,
    "108": 98,
    "110": 98,
    "131": 101,
    "151": 103,
    "199": 125,
    "202": 131,
    "227": 176,
    "233": 187,
    "234": 187,
    "245": 205,
    "247": 208,
    "251": 211,
    "253": 216,
    "254": 217,
    "258": 228,
    "262": 238,
    "270": 250,
    "276": 254,
    "277": 254,
    "291": 261,
    "296": 266,
}
GAP_REVIEW_KEYS = {
    "15",
    "18",
    "23",
    "24",
    "30",
    "32",
    "108",
    "110",
    "131",
    "151",
    "160",
    "188",
    "202",
    "227",
    "233",
    "234",
    "245",
    "251",
    "254",
    "276",
    "277",
    "291",
}
OCR_DISAMBIGUATION_KEYS = {
    "195",
    "199",
    "224",
    "247",
    "253",
    "258",
    "262",
    "270",
    "283",
    "296",
}


class NumberedUnitBuildError(RuntimeError):
    """Raised when the fixed source route cannot be reproduced honestly."""


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
        raise NumberedUnitBuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise NumberedUnitBuildError(f"JSON root is not an object: {path}")
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
        if number in {65, 73, 237}:
            keys.append(f"{number}a")
    return keys


def _substitution_cost(expected: str, observed: str) -> float:
    if expected == observed:
        return 0.0
    if observed in OCR_SUBSTITUTIONS.get(expected, set()):
        return 0.22
    return 1.0


def _weighted_distance(expected: str, observed: str) -> float:
    previous = list(range(len(observed) + 1))
    for row, expected_character in enumerate(expected, start=1):
        current = [row]
        for column, observed_character in enumerate(observed, start=1):
            current.append(
                min(
                    previous[column] + 1,
                    current[-1] + 1,
                    previous[column - 1]
                    + _substitution_cost(expected_character, observed_character),
                )
            )
        previous = current
    return previous[-1]


def _abbyy_candidates(payload_path: Path) -> list[dict[str, Any]]:
    candidates: list[dict[str, Any]] = []
    page_index = 0
    try:
        with gzip.open(payload_path, "rb") as source:
            for _event, page in ET.iterparse(source, events=("end",)):
                if page.tag != f"{ABBYY_NS}page":
                    continue
                page_index += 1
                if not 10 <= page_index <= 266:
                    page.clear()
                    continue
                for paragraph in page.iter(f"{ABBYY_NS}par"):
                    raw = "".join(
                        character.text or ""
                        for character in paragraph.iter(f"{ABBYY_NS}charParams")
                    ).strip()
                    tops = [
                        int(line.get("t", "99999"))
                        for line in paragraph.iter(f"{ABBYY_NS}line")
                    ]
                    top = min(tops, default=-1)
                    if not (
                        400 <= top <= 3500
                        and 1 <= len(raw) <= 14
                        and set(raw) <= OCR_ALLOWED_CHARACTERS
                        and any(
                            character.isdigit() or character in "IiLl"
                            for character in raw
                        )
                    ):
                        continue
                    compact = "".join(
                        character.lower()
                        for character in raw
                        if character.isalnum() or character == "»"
                    )
                    if compact:
                        candidates.append(
                            {
                                "page": page_index,
                                "top": top,
                                "compact": compact,
                            }
                        )
                page.clear()
    except (OSError, EOFError, ET.ParseError) as exc:
        raise NumberedUnitBuildError(f"ABBYY XML gzip is unreadable: {exc}") from exc
    if page_index != 274:
        raise NumberedUnitBuildError(
            f"ABBYY page count drifted: expected 274, got {page_index}"
        )
    return candidates


def _ordered_candidate_matches(
    keys: list[str],
    candidates: list[dict[str, Any]],
) -> tuple[dict[str, int], list[str]]:
    expected_count = len(keys)
    candidate_count = len(candidates)
    infinity = float("inf")
    scores = [
        [infinity] * (candidate_count + 1)
        for _ in range(expected_count + 1)
    ]
    route: list[list[tuple[str, int, int] | None]] = [
        [None] * (candidate_count + 1)
        for _ in range(expected_count + 1)
    ]
    scores[0][0] = 0.0
    for expected_index in range(expected_count + 1):
        for candidate_index in range(candidate_count + 1):
            score = scores[expected_index][candidate_index]
            if expected_index < expected_count:
                proposed = score + 0.92
                if proposed < scores[expected_index + 1][candidate_index]:
                    scores[expected_index + 1][candidate_index] = proposed
                    route[expected_index + 1][candidate_index] = (
                        "skip_expected",
                        expected_index,
                        candidate_index,
                    )
            if candidate_index < candidate_count:
                proposed = score + 0.04
                if proposed < scores[expected_index][candidate_index + 1]:
                    scores[expected_index][candidate_index + 1] = proposed
                    route[expected_index][candidate_index + 1] = (
                        "skip_candidate",
                        expected_index,
                        candidate_index,
                    )
            if expected_index < expected_count and candidate_index < candidate_count:
                expected = keys[expected_index].removesuffix("a")
                cost = _weighted_distance(
                    expected,
                    candidates[candidate_index]["compact"],
                )
                proposed = score + cost
                if (
                    cost <= 1.55
                    and proposed
                    < scores[expected_index + 1][candidate_index + 1]
                ):
                    scores[expected_index + 1][candidate_index + 1] = proposed
                    route[expected_index + 1][candidate_index + 1] = (
                        "match",
                        expected_index,
                        candidate_index,
                    )

    expected_index = expected_count
    candidate_index = candidate_count
    actions: list[tuple[str, int, int]] = []
    while expected_index or candidate_index:
        action = route[expected_index][candidate_index]
        if action is None:
            raise NumberedUnitBuildError("ordered ABBYY alignment route is incomplete")
        actions.append(action)
        _kind, expected_index, candidate_index = action
    actions.reverse()

    matches: dict[str, int] = {}
    skipped: list[str] = []
    for kind, expected_index, candidate_index in actions:
        if kind == "match":
            matches[keys[expected_index]] = candidates[candidate_index]["page"]
        elif kind == "skip_expected":
            skipped.append(keys[expected_index])
    return matches, skipped


def _payload_entries(
    *,
    repo_root: Path,
    payload_source_root: Path,
    manifest: dict[str, Any],
) -> tuple[dict[str, dict[str, Any]], dict[str, Path]]:
    relative_item_dir = (repo_root / MANIFEST_PATH).parent.relative_to(
        repo_root / SOURCE_ROOT
    )
    payload_dir = payload_source_root / relative_item_dir
    entries: dict[str, dict[str, Any]] = {}
    paths: dict[str, Path] = {}
    for entry in manifest["payload_files"]:
        media_type = entry["media_type"]
        if media_type == "application/pdf":
            role = "pdf"
        elif media_type == "application/vnd.djvu+xml":
            role = "djvu"
        elif (
            media_type == "application/gzip"
            and entry["relative_path"].endswith(".abbyy.xml.gz")
        ):
            role = "abbyy"
        else:
            raise NumberedUnitBuildError(
                f"unexpected payload media type: {media_type}"
            )
        path = payload_dir / entry["relative_path"]
        if not path.is_file():
            raise NumberedUnitBuildError(f"local payload is missing: {path}")
        if _sha256_path(path) != entry["sha256"]:
            raise NumberedUnitBuildError(f"local payload digest drifted: {path}")
        entries[role] = entry
        paths[role] = path
    if set(entries) != {"pdf", "djvu", "abbyy"}:
        raise NumberedUnitBuildError("expected PDF, DjVu XML, and ABBYY XML gzip")
    return entries, paths


def _inventory_files(inventory: dict[str, Any]) -> dict[str, dict[str, Any]]:
    files = {
        file_inventory["profile"]: file_inventory
        for file_inventory in inventory["files"]
    }
    expected = {
        "pdf_pages_v1",
        "djvu_xml_pages_v1",
        "abbyy_xml_pages_v1",
    }
    if set(files) != expected:
        raise NumberedUnitBuildError(
            f"resource inventory profiles drifted: {sorted(files)}"
        )
    for file_inventory in files.values():
        if file_inventory["summary"]["page_count"] != 274:
            raise NumberedUnitBuildError("resource inventory page count drifted")
    return files


def _anchor_id(unit_key: str) -> str:
    return (
        "tos.anchor.friedrich-nietzsche.jenseits-von-gut-und-boese."
        f"de-naumann-1886.unit-{unit_key}.pdf-start-page"
    )


def _passage_id(unit_key: str) -> str:
    return (
        "tos.passage.friedrich-nietzsche.jenseits-von-gut-und-boese."
        f"de-naumann-1886.unit-{unit_key}"
    )


def _basis(unit_key: str) -> str:
    if unit_key == "237a":
        return "source_visible_repeated_number_review"
    if unit_key in GAP_REVIEW_KEYS:
        return "source_visible_gap_review"
    if unit_key in OCR_DISAMBIGUATION_KEYS:
        return "source_visible_ocr_disambiguation"
    return "ocr_order_candidate"


def build_outputs(
    *,
    repo_root: Path,
    payload_source_root: Path,
    event_at: str,
) -> tuple[str, str, str]:
    manifest = _read_json(repo_root / MANIFEST_PATH)
    inventory = _read_json(repo_root / INVENTORY_PATH)
    entries, payload_paths = _payload_entries(
        repo_root=repo_root,
        payload_source_root=payload_source_root,
        manifest=manifest,
    )
    inventories = _inventory_files(inventory)
    for role, profile in {
        "pdf": "pdf_pages_v1",
        "djvu": "djvu_xml_pages_v1",
        "abbyy": "abbyy_xml_pages_v1",
    }.items():
        if inventories[profile]["file_id"] != entries[role]["file_id"]:
            raise NumberedUnitBuildError(
                f"manifest/inventory identity drifted for {role}"
            )

    keys = _unit_keys()
    candidates = _abbyy_candidates(payload_paths["abbyy"])
    machine_matches, skipped = _ordered_candidate_matches(keys, candidates)
    if set(skipped) != set(PAGE_OVERRIDES):
        raise NumberedUnitBuildError(
            "ordered ABBYY gap set drifted: "
            f"expected {sorted(PAGE_OVERRIDES)}, got {skipped}"
        )
    pages = {**machine_matches, **PAGE_OVERRIDES}
    if set(pages) != set(keys):
        raise NumberedUnitBuildError("numbered-unit coverage is incomplete")
    page_sequence = [pages[key] for key in keys]
    if page_sequence != sorted(page_sequence):
        raise NumberedUnitBuildError("numbered-unit pages are not monotonic")

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
    map_payload = {
        "$schema": SCHEMA_REF,
        "schema_version": "tos_numbered_unit_page_map_v1",
        "map_id": MAP_ID,
        "work_ref": WORK_REF,
        "expression_ref": EXPRESSION_REF,
        "edition_ref": EDITION_REF,
        "item_ref": manifest["item_id"],
        "scan_file": {
            "file_ref": entries["pdf"]["file_id"],
            "file_sha256": entries["pdf"]["sha256"],
            "inventory_profile": "pdf_pages_v1",
        },
        "navigation_files": [
            {
                "role": "djvu_xml",
                "file_ref": entries["djvu"]["file_id"],
                "file_sha256": entries["djvu"]["sha256"],
                "inventory_profile": "djvu_xml_pages_v1",
            },
            {
                "role": "abbyy_xml_gzip",
                "file_ref": entries["abbyy"]["file_id"],
                "file_sha256": entries["abbyy"]["sha256"],
                "inventory_profile": "abbyy_xml_pages_v1",
            },
        ],
        "inventory": {
            "ref": INVENTORY_PATH.as_posix(),
            "sha256": inventory_digest,
        },
        "map_authority": "model_reviewed_source_structure_candidate_only",
        "source_text_included": False,
        "method": {
            "name": "ordered-ocr-candidate-plus-source-visible-gap-review",
            "version": "1",
            "maker_type": "mixed",
            "local_payloads_read": True,
            "layer_trials": [
                {
                    "layer": "embedded_pdf_text",
                    "result": (
                        "substantial numeral loss; insufficient for a complete "
                        "numbered-unit map"
                    ),
                    "selected_role": "rejected_as_complete_map_source",
                },
                {
                    "layer": "djvu_xml",
                    "result": (
                        "274-page word-coordinate navigation retained as an "
                        "independent provider OCR comparison"
                    ),
                    "selected_role": "secondary_coordinate_navigation",
                },
                {
                    "layer": "abbyy_xml_gzip",
                    "result": (
                        "274-page paragraph, line, word, and character geometry "
                        "selected for ordered numeral candidates"
                    ),
                    "selected_role": "primary_machine_candidate_navigation",
                },
            ],
            "ordered_abbyy_candidate_matches": len(machine_matches),
            "source_visible_review": {
                "maker_type": "model",
                "human_repeat_performed": False,
                "gap_review_unit_keys": sorted(
                    GAP_REVIEW_KEYS,
                    key=lambda value: int(value.rstrip("a")),
                ),
                "ocr_disambiguation_unit_keys": sorted(
                    OCR_DISAMBIGUATION_KEYS,
                    key=lambda value: int(value.rstrip("a")),
                ),
                "repeated_number_unit_keys": ["237a"],
            },
            "cross_lingual_text_matching_used": False,
            "semantic_matching_used": False,
            "no_source_text_emitted": True,
        },
        "unit_starts": unit_starts,
        "summary": {
            "integer_numbered_unit_count": 296,
            "supplemental_numbered_units": ["65a", "73a", "237a"],
            "numbered_unit_count": 299,
            "exact_start_page_candidates_materialized": 299,
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
            "file_id": entries["pdf"]["file_id"],
            "file_sha256": entries["pdf"]["sha256"],
            "passage_id": _passage_id(unit["unit_key"]),
            "selectors": [
                {
                    "type": "structural",
                    "path": [
                        "work:jenseits-von-gut-und-boese",
                        "expression:de-naumann-1886",
                        f"numbered-unit:{unit['unit_key']}",
                    ],
                    "scheme": "jgb-numbered-unit-start-v1",
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
                    "ordered ABBYY numeral candidate plus bounded "
                    "source-visible scan review"
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
            "software:poppler-26.01.0",
        ],
        "inputs": [
            {
                "ref": entries["pdf"]["file_id"],
                "role": "source-visible-scan-witness",
                "sha256": entries["pdf"]["sha256"],
            },
            {
                "ref": entries["djvu"]["file_id"],
                "role": "secondary-provider-ocr-coordinate-layer",
                "sha256": entries["djvu"]["sha256"],
            },
            {
                "ref": entries["abbyy"]["file_id"],
                "role": "primary-machine-candidate-coordinate-layer",
                "sha256": entries["abbyy"]["sha256"],
            },
            {
                "ref": INVENTORY_PATH.as_posix(),
                "role": "tracked-text-free-resource-inventory",
                "sha256": inventory_digest,
            },
        ],
        "outputs": [
            {
                "ref": MAP_PATH.as_posix(),
                "role": "tracked-text-free-numbered-unit-page-map",
                "sha256": _sha256_bytes(rendered_map.encode("utf-8")),
            },
            {
                "ref": ANCHOR_PATH.as_posix(),
                "role": "tracked-proposed-whole-page-source-anchors",
                "sha256": _sha256_bytes(rendered_anchors.encode("utf-8")),
            },
        ],
        "method": {
            "maker_type": "mixed",
            "name": "ordered-ocr-candidate-plus-source-visible-gap-review",
            "version": "1",
            "artifact_digest": None,
            "runtime": (
                "Python standard library gzip/XML parser; Poppler-rendered "
                "exact scan pages"
            ),
            "device": "abyss-machine",
            "configuration": {
                "expected_integer_units": 296,
                "supplemental_numbered_units": ["65a", "73a", "237a"],
                "ordered_abbyy_candidate_matches": len(machine_matches),
                "source_visible_gap_review_count": len(GAP_REVIEW_KEYS),
                "source_visible_ocr_disambiguation_count": len(
                    OCR_DISAMBIGUATION_KEYS
                ),
                "human_repeat_performed": False,
                "source_text_included": False,
            },
            "prompt_or_instruction_ref": (
                "ToS/doctrine/CORPUS_FOUNDATION.md#address-law"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            (
                "All 299 addresses are proposed start-page candidates for one "
                "scan; they are not exact line or passage-end boundaries."
            ),
            (
                "OCR layers supplied navigation candidates only and were not "
                "accepted as German source text."
            ),
            (
                "The scan-visible review was model-performed and has not been "
                "independently repeated by a human."
            ),
            (
                "The source-visible repeated 237 is retained as 237a; this "
                "local structural key does not claim critical-edition equivalence."
            ),
        ],
        "receipt_refs": [
            MAP_PATH.as_posix(),
            ANCHOR_PATH.as_posix(),
        ],
        "rights_basis_ref": RIGHTS_REF,
        "event_version": 1,
        "supersedes_event_ref": None,
    }
    return rendered_map, rendered_anchors, _render_jsonl([provenance])


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Build the text-free Jenseits Naumann 1886 numbered-unit page map."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        required=True,
        help="Source-witness root containing the exact local payloads.",
    )
    parser.add_argument(
        "--event-at",
        default="2026-07-29T00:05:44-06:00",
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
        print("[ok] Jenseits numbered-unit page map and anchors match local inputs")
        return 0

    for path, rendered in outputs.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(rendered, encoding="utf-8")
    print("[ok] wrote 299 proposed Jenseits numbered-unit start-page anchors")
    print(
        "[boundary] no OCR text, accepted German, exact line boundaries, "
        "translation, or semantics were emitted"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
