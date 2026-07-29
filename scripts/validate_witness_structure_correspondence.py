#!/usr/bin/env python3
"""Validate tracked text-free witness-structure maps.

This validator checks schema shape, inventory/resource closure, digest-bound
provenance, page-enumeration arithmetic, summaries, numbered spans, and
monotonicity. Passing does not establish textual identity, edition
equivalence, accepted original-language text, translation, semantics, or
canon authority.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from jsonschema import FormatChecker
from jsonschema.validators import validator_for


REPO_ROOT = Path(__file__).resolve().parents[1]
MAP_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/alignments/structure/"
    "naumann-1893-dta-parts/structure-correspondence.json"
)
ANCHOR_SET_PATH = MAP_PATH.with_name("structure-anchor-set.json")
ANCHOR_RECORDS_PATH = MAP_PATH.with_name("structure-anchors.jsonl")
SCHEMA_PATH = Path("ToS/contracts/witness-structure-correspondence.schema.json")
ANCHOR_SET_SCHEMA_PATH = Path(
    "ToS/contracts/witness-structure-anchor-set.schema.json"
)
ANCHOR_SCHEMA_PATH = Path("ToS/contracts/source-anchor.schema.json")
PROVENANCE_SCHEMA_PATH = Path("ToS/contracts/provenance-event.schema.json")
PARALLEL_MAP_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/alignments/structure/"
    "naumann-1886-polilov-mysl-1996/structure-correspondence.json"
)
PARALLEL_ANCHOR_RECORDS_PATH = PARALLEL_MAP_PATH.with_name(
    "structure-anchors.jsonl"
)
PARALLEL_SCHEMA_PATH = Path(
    "ToS/contracts/parallel-witness-structure-map.schema.json"
)
NUMBERED_UNIT_MAP_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/expressions/de-naumann-1886/"
    "editions/leipzig-c-g-naumann-1886/items/"
    "internet-archive-google-harvard-scan-pdf/structure/"
    "numbered-unit-page-map.json"
)
NUMBERED_UNIT_ANCHOR_RECORDS_PATH = NUMBERED_UNIT_MAP_PATH.with_name(
    "numbered-unit-anchors.jsonl"
)
NUMBERED_UNIT_SCHEMA_PATH = Path(
    "ToS/contracts/numbered-unit-page-map.schema.json"
)
TARGET_NUMBERED_UNIT_MAP_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/expressions/ru-polilov-mysl-1996/"
    "structure/mysl-1996-volume-2-operator-pdf/"
    "numbered-unit-page-map.json"
)
TARGET_NUMBERED_UNIT_ANCHOR_RECORDS_PATH = (
    TARGET_NUMBERED_UNIT_MAP_PATH.with_name("numbered-unit-anchors.jsonl")
)
TARGET_NUMBERED_UNIT_SCHEMA_PATH = Path(
    "ToS/contracts/target-numbered-unit-page-map.schema.json"
)
TARGET_NUMBERED_UNIT_EVENT_ID = (
    "tos.event.target-numbered-unit-page-map.friedrich-nietzsche."
    "jenseits-von-gut-und-boese.ru-polilov-mysl-1996.2026-07-29"
)
NUMBERED_UNIT_LABEL_MAP_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/alignments/structure/"
    "naumann-1886-polilov-mysl-1996/"
    "numbered-unit-label-correspondence.json"
)
NUMBERED_UNIT_LABEL_PROVENANCE_PATH = NUMBERED_UNIT_LABEL_MAP_PATH.with_name(
    "provenance.numbered-unit-label-correspondence.jsonl"
)
NUMBERED_UNIT_LABEL_SCHEMA_PATH = Path(
    "ToS/contracts/parallel-numbered-unit-label-map.schema.json"
)
NUMBERED_UNIT_LABEL_SOURCE_RIGHTS_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/expressions/de-naumann-1886/"
    "editions/leipzig-c-g-naumann-1886/items/"
    "internet-archive-google-harvard-scan-pdf/rights.json"
)
NUMBERED_UNIT_LABEL_TARGET_RIGHTS_PATH = Path(
    "ToS/source-witnesses/collections/friedrich-nietzsche/"
    "works-in-two-volumes-volume-2-mysl-1996/"
    "editions/moscow-mysl-1996-volume-2/items/operator-pdf/rights.json"
)
NUMBERED_UNIT_LABEL_EVENT_ID = (
    "tos.event.parallel-numbered-unit-label-map.friedrich-nietzsche."
    "jenseits-von-gut-und-boese.naumann-1886-to-polilov-mysl-1996."
    "2026-07-29"
)
NUMBERED_UNIT_EVENT_ID = (
    "tos.event.numbered-unit-page-map.friedrich-nietzsche."
    "jenseits-von-gut-und-boese.de-naumann-1886.2026-07-29"
)
PARALLEL_EVENT_ID = (
    "tos.event.structure-correspondence.friedrich-nietzsche."
    "jenseits-von-gut-und-boese.naumann-1886-to-polilov-mysl-1996."
    "source-237a-correction.2026-07-29"
)
ANCHOR_EVENT_ID = (
    "tos.event.structure-anchors.friedrich-nietzsche."
    "also-sprach-zarathustra.dta-parts-to-naumann-1893.2026-07-28"
)
PAGE_MEMBER_RE = re.compile(r"^EPUB/page_(\d+)\.html$")
PROHIBITED_TEXT_KEYS = {
    "text",
    "source_text",
    "heading_text",
    "quote",
    "excerpt",
    "transcription",
}

Issue = tuple[str, str]


def _relative(path: Path, repo_root: Path) -> str:
    try:
        return path.relative_to(repo_root).as_posix()
    except ValueError:
        return path.as_posix()


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _load_json(path: Path, repo_root: Path, issues: list[Issue]) -> dict[str, Any] | None:
    location = _relative(path, repo_root)
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        issues.append((location, "file is missing"))
        return None
    except (OSError, json.JSONDecodeError) as exc:
        issues.append((location, f"cannot read JSON: {exc}"))
        return None
    if not isinstance(payload, dict):
        issues.append((location, "JSON root must be an object"))
        return None
    return payload


def _load_jsonl(
    path: Path,
    repo_root: Path,
    issues: list[Issue],
) -> list[dict[str, Any]]:
    location = _relative(path, repo_root)
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        issues.append((location, f"cannot read JSONL: {exc}"))
        return []
    payloads: list[dict[str, Any]] = []
    for index, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as exc:
            issues.append((f"{location}:{index}", f"invalid JSON: {exc}"))
            continue
        if not isinstance(payload, dict):
            issues.append((f"{location}:{index}", "record must be an object"))
            continue
        payloads.append(payload)
    return payloads


def _validator(schema_path: Path, repo_root: Path):
    schema = json.loads((repo_root / schema_path).read_text(encoding="utf-8"))
    validator_class = validator_for(schema)
    validator_class.check_schema(schema)
    return validator_class(schema, format_checker=FormatChecker())


def _validate_schema(
    payload: object,
    *,
    validator: Any,
    location: str,
    issues: list[Issue],
) -> None:
    for error in sorted(
        validator.iter_errors(payload),
        key=lambda item: list(item.absolute_path),
    ):
        suffix = "".join(f"[{part!r}]" for part in error.absolute_path)
        issues.append((f"{location}{suffix}", error.message))


def _resources(file_inventory: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {
        resource["resource_id"]: resource
        for resource in file_inventory.get("resources", [])
        if isinstance(resource, dict) and isinstance(resource.get("resource_id"), str)
    }


def _prohibited_key_paths(payload: object, prefix: str = "") -> list[str]:
    paths: list[str] = []
    if isinstance(payload, dict):
        for key, value in payload.items():
            path = f"{prefix}.{key}" if prefix else key
            if key in PROHIBITED_TEXT_KEYS:
                paths.append(path)
            paths.extend(_prohibited_key_paths(value, path))
    elif isinstance(payload, list):
        for index, value in enumerate(payload):
            paths.extend(_prohibited_key_paths(value, f"{prefix}[{index}]"))
    return paths


def _load_witness_inventory(
    witness: dict[str, Any],
    *,
    repo_root: Path,
    issues: list[Issue],
    location: str,
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]] | None:
    inventory_ref = witness.get("inventory_ref")
    if not isinstance(inventory_ref, str):
        issues.append((location, "witness inventory_ref is invalid"))
        return None
    inventory = _load_json(repo_root / inventory_ref, repo_root, issues)
    if inventory is None:
        return None
    if inventory.get("item_id") != witness.get("item_ref"):
        issues.append((location, "witness item_ref differs from inventory"))
    files = [
        item
        for item in inventory.get("files", [])
        if isinstance(item, dict) and item.get("file_id") == witness.get("file_ref")
    ]
    if len(files) != 1:
        issues.append((location, "witness file_ref does not resolve exactly once"))
        return None
    file_inventory = files[0]
    if file_inventory.get("file_sha256") != witness.get("file_sha256"):
        issues.append((location, "witness file digest differs from inventory"))
    if file_inventory.get("profile") != witness.get("profile"):
        issues.append((location, "witness profile differs from inventory"))
    return file_inventory, _resources(file_inventory)


def _expected_anchor_id(correspondence_id: str, role: str) -> str:
    suffix = correspondence_id.removeprefix("structure-")
    return f"tos.anchor.zarathustra-structure.{role}-{suffix}"


def validate_structure_correspondence(repo_root: Path = REPO_ROOT) -> list[Issue]:
    repo_root = repo_root.resolve()
    issues: list[Issue] = []
    map_path = repo_root / MAP_PATH
    payload = _load_json(map_path, repo_root, issues)
    if payload is None:
        return issues
    location = MAP_PATH.as_posix()
    _validate_schema(
        payload,
        validator=_validator(SCHEMA_PATH, repo_root),
        location=location,
        issues=issues,
    )
    for path in _prohibited_key_paths(payload):
        issues.append((location, f"source-text-bearing key is forbidden: {path}"))

    source_parts = {
        witness.get("part_label"): witness
        for witness in payload.get("source_parts", [])
        if isinstance(witness, dict)
    }
    if set(source_parts) != {"I", "II", "III", "IV"}:
        issues.append((location, "source_parts must cover I, II, III, and IV exactly"))
    routes = {
        route.get("part_label"): route
        for route in payload.get("part_routes", [])
        if isinstance(route, dict)
    }
    if set(routes) != {"I", "II", "III", "IV"}:
        issues.append((location, "part_routes must cover I, II, III, and IV exactly"))

    witness_resources: dict[str, dict[str, dict[str, Any]]] = {}
    all_witnesses = list(source_parts.values())
    targets = payload.get("target_witnesses", {})
    if isinstance(targets, dict):
        all_witnesses.extend(
            witness
            for witness in (targets.get("epub"), targets.get("pdf"))
            if isinstance(witness, dict)
        )
    for witness in all_witnesses:
        witness_location = f"{location}:{witness.get('item_ref')}"
        result = _load_witness_inventory(
            witness,
            repo_root=repo_root,
            issues=issues,
            location=witness_location,
        )
        if result is not None and isinstance(witness.get("item_ref"), str):
            witness_resources[witness["item_ref"]] = result[1]

    epub_witness = targets.get("epub", {}) if isinstance(targets, dict) else {}
    pdf_witness = targets.get("pdf", {}) if isinstance(targets, dict) else {}
    epub_resources = witness_resources.get(epub_witness.get("item_ref"), {})
    pdf_resources = witness_resources.get(pdf_witness.get("item_ref"), {})
    epub_pages: list[int] = []
    for resource in epub_resources.values():
        locator = resource.get("locator", {})
        member_path = locator.get("member_path") if isinstance(locator, dict) else None
        match = PAGE_MEMBER_RE.fullmatch(member_path) if isinstance(member_path, str) else None
        if match is not None:
            epub_pages.append(int(match.group(1)))
    pdf_pages = [
        resource.get("locator", {}).get("page_index")
        for resource in pdf_resources.values()
        if resource.get("resource_kind") == "pdf_page"
        and isinstance(resource.get("locator"), dict)
    ]
    if sorted(epub_pages) != list(range(0, 529)):
        issues.append((location, "target EPUB page-member enumeration is not 0..528"))
    if sorted(pdf_pages) != list(range(1, 530)):
        issues.append((location, "target PDF page enumeration is not 1..529"))

    correspondence_ids: set[str] = set()
    source_resource_keys: set[tuple[str, str]] = set()
    part_sequences: dict[str, list[int]] = defaultdict(list)
    part_target_pages: dict[str, list[int]] = defaultdict(list)
    mode_counts: Counter[str] = Counter()
    for index, correspondence in enumerate(payload.get("correspondences", []), start=1):
        if not isinstance(correspondence, dict):
            continue
        entry_location = f"{location}:correspondences[{index}]"
        correspondence_id = correspondence.get("correspondence_id")
        if not isinstance(correspondence_id, str) or correspondence_id in correspondence_ids:
            issues.append((entry_location, "correspondence_id is invalid or duplicated"))
        else:
            correspondence_ids.add(correspondence_id)
        part = correspondence.get("part_label")
        sequence = correspondence.get("sequence")
        if isinstance(part, str) and isinstance(sequence, int):
            part_sequences[part].append(sequence)

        source = correspondence.get("source", {})
        source_witness = source_parts.get(part, {})
        if isinstance(source, dict) and isinstance(source_witness, dict):
            for field in ("item_ref", "file_ref", "inventory_ref"):
                if source.get(field) != source_witness.get(field):
                    issues.append((entry_location, f"source {field} differs from part witness"))
            source_key = (str(source.get("item_ref")), str(source.get("resource_id")))
            if source_key in source_resource_keys:
                issues.append((entry_location, "source division resource is duplicated"))
            source_resource_keys.add(source_key)
            source_resource = witness_resources.get(source.get("item_ref"), {}).get(
                source.get("resource_id")
            )
            if source_resource is None:
                issues.append((entry_location, "source division resource is unresolved"))
            else:
                locator = source_resource.get("locator", {})
                expected = {
                    "tei_path": locator.get("tei_path"),
                    "tei_depth": locator.get("tei_depth"),
                    "tei_page_label": locator.get("tei_page_label"),
                    "label_fingerprint": source_resource.get("label_fingerprint"),
                }
                for field, value in expected.items():
                    if source.get(field) != value:
                        issues.append((entry_location, f"source {field} differs from inventory"))

        target_epub = correspondence.get("target_epub", {})
        page_number: int | None = None
        if isinstance(target_epub, dict):
            for field in ("item_ref", "file_ref", "inventory_ref"):
                if target_epub.get(field) != epub_witness.get(field):
                    issues.append((entry_location, f"target_epub {field} differs from witness"))
            epub_resource = epub_resources.get(target_epub.get("resource_id"))
            if epub_resource is None:
                issues.append((entry_location, "target EPUB resource is unresolved"))
            else:
                locator = epub_resource.get("locator", {})
                expected = {
                    "member_path": locator.get("member_path"),
                    "member_sha256": epub_resource.get("sha256"),
                    "spine_index": locator.get("spine_index"),
                    "content_fingerprint": epub_resource.get("content_fingerprint"),
                }
                for field, value in expected.items():
                    if target_epub.get(field) != value:
                        issues.append((entry_location, f"target_epub {field} differs from inventory"))
            member_path = target_epub.get("member_path")
            match = PAGE_MEMBER_RE.fullmatch(member_path) if isinstance(member_path, str) else None
            if match is None:
                issues.append((entry_location, "target EPUB member path is not a scan page"))
            else:
                page_number = int(match.group(1))
                if target_epub.get("scan_page_number") != page_number:
                    issues.append((entry_location, "target EPUB scan page differs from member path"))
                if isinstance(part, str):
                    part_target_pages[part].append(page_number)

        target_pdf = correspondence.get("target_pdf", {})
        if isinstance(target_pdf, dict):
            for field in ("item_ref", "file_ref", "inventory_ref"):
                if target_pdf.get(field) != pdf_witness.get(field):
                    issues.append((entry_location, f"target_pdf {field} differs from witness"))
            pdf_resource = pdf_resources.get(target_pdf.get("resource_id"))
            if pdf_resource is None:
                issues.append((entry_location, "target PDF resource is unresolved"))
            elif target_pdf.get("page_index") != pdf_resource.get("locator", {}).get(
                "page_index"
            ):
                issues.append((entry_location, "target PDF page differs from inventory"))
            if page_number is not None and target_pdf.get("page_index") != page_number + 1:
                issues.append((entry_location, "EPUB-to-PDF page formula drifted"))

        match_evidence = correspondence.get("match", {})
        if isinstance(match_evidence, dict):
            mode = match_evidence.get("mode")
            if isinstance(mode, str):
                mode_counts[mode] += 1
            route = routes.get(part, {})
            if match_evidence.get("search_page_range") != route.get(
                "target_epub_member_page_range"
            ):
                issues.append((entry_location, "match search range differs from part route"))
            target_window = match_evidence.get("target_window_pages", [])
            if page_number is not None and (
                not isinstance(target_window, list)
                or not target_window
                or target_window[0] != page_number
            ):
                issues.append((entry_location, "target window does not start at selected page"))

    for part in ("I", "II", "III", "IV"):
        sequences = part_sequences.get(part, [])
        if sequences != list(range(1, len(sequences) + 1)):
            issues.append((location, f"part {part} sequence is not contiguous"))
        pages = part_target_pages.get(part, [])
        if pages != sorted(pages):
            issues.append((location, f"part {part} target pages are not monotonic"))

    summary = payload.get("summary", {})
    correspondences = [
        item for item in payload.get("correspondences", []) if isinstance(item, dict)
    ]
    if isinstance(summary, dict):
        if summary.get("correspondence_count") != len(correspondences):
            issues.append((location, "summary correspondence_count drifted"))
        expected_part_counts = {
            part: len(part_sequences.get(part, []))
            for part in ("I", "II", "III", "IV")
        }
        if summary.get("part_counts") != expected_part_counts:
            issues.append((location, "summary part_counts drifted"))
        if summary.get("match_mode_counts") != dict(sorted(mode_counts.items())):
            issues.append((location, "summary match_mode_counts drifted"))

    anchor_set_path = repo_root / ANCHOR_SET_PATH
    anchor_records_path = repo_root / ANCHOR_RECORDS_PATH
    anchor_set = _load_json(anchor_set_path, repo_root, issues)
    anchor_records = _load_jsonl(anchor_records_path, repo_root, issues)
    anchors_by_id: dict[str, dict[str, Any]] = {}
    expected_anchor_order: list[str] = []
    if anchor_set is not None:
        anchor_set_location = ANCHOR_SET_PATH.as_posix()
        _validate_schema(
            anchor_set,
            validator=_validator(ANCHOR_SET_SCHEMA_PATH, repo_root),
            location=anchor_set_location,
            issues=issues,
        )
        for path in _prohibited_key_paths(anchor_set):
            issues.append(
                (
                    anchor_set_location,
                    f"source-text-bearing key is forbidden: {path}",
                )
            )
        expected_map_ref = {
            "ref": MAP_PATH.as_posix(),
            "sha256": _sha256(map_path),
        }
        if anchor_set.get("correspondence_map") != expected_map_ref:
            issues.append(
                (
                    anchor_set_location,
                    "correspondence_map ref or digest drifted",
                )
            )
        if anchor_records_path.is_file():
            expected_records_ref = {
                "ref": ANCHOR_RECORDS_PATH.as_posix(),
                "sha256": _sha256(anchor_records_path),
            }
            if anchor_set.get("anchor_records") != expected_records_ref:
                issues.append(
                    (
                        anchor_set_location,
                        "anchor_records ref or digest drifted",
                    )
                )
        if anchor_set.get("work_ref") != payload.get("work_ref"):
            issues.append((anchor_set_location, "work_ref differs from map"))
        if anchor_set.get("provenance_ref") != payload.get("provenance_ref"):
            issues.append(
                (anchor_set_location, "provenance_ref differs from map")
            )

    anchor_validator = _validator(ANCHOR_SCHEMA_PATH, repo_root)
    for index, anchor in enumerate(anchor_records, start=1):
        anchor_location = f"{ANCHOR_RECORDS_PATH.as_posix()}:{index}"
        _validate_schema(
            anchor,
            validator=anchor_validator,
            location=anchor_location,
            issues=issues,
        )
        for path in _prohibited_key_paths(anchor):
            issues.append(
                (
                    anchor_location,
                    f"source-text-bearing key is forbidden: {path}",
                )
            )
        anchor_id = anchor.get("anchor_id")
        if not isinstance(anchor_id, str) or anchor_id in anchors_by_id:
            issues.append((anchor_location, "anchor_id is invalid or duplicated"))
        else:
            anchors_by_id[anchor_id] = anchor
        if anchor.get("status") != "proposed":
            issues.append((anchor_location, "structural anchor must remain proposed"))
        if anchor.get("provenance_event_ref") != ANCHOR_EVENT_ID:
            issues.append((anchor_location, "anchor provenance event drifted"))
        selector_types = {
            selector.get("type")
            for selector in anchor.get("selectors", [])
            if isinstance(selector, dict)
        }
        if selector_types & {"text_quote", "text_position"}:
            issues.append(
                (
                    anchor_location,
                    "text-bearing selectors are forbidden in the structural anchor set",
                )
            )

    correspondences_by_id = {
        item["correspondence_id"]: item
        for item in correspondences
        if isinstance(item.get("correspondence_id"), str)
    }
    bindings = (
        anchor_set.get("bindings", [])
        if isinstance(anchor_set, dict)
        else []
    )
    bindings_by_id: dict[str, dict[str, Any]] = {}
    for index, binding in enumerate(bindings, start=1):
        if not isinstance(binding, dict):
            continue
        binding_location = f"{ANCHOR_SET_PATH.as_posix()}:bindings[{index}]"
        correspondence_id = binding.get("correspondence_id")
        correspondence = correspondences_by_id.get(correspondence_id)
        if correspondence is None or correspondence_id in bindings_by_id:
            issues.append(
                (
                    binding_location,
                    "correspondence binding is unresolved or duplicated",
                )
            )
            continue
        bindings_by_id[correspondence_id] = binding
        if binding.get("part_label") != correspondence.get("part_label"):
            issues.append((binding_location, "part_label differs from map"))
        if binding.get("sequence") != correspondence.get("sequence"):
            issues.append((binding_location, "sequence differs from map"))

        expected_roles = (
            (
                "source_tei",
                "dta",
                correspondence["source"],
                source_parts[correspondence["part_label"]]["file_sha256"],
                [
                    {
                        "type": "structural",
                        "path": [correspondence["source"]["tei_path"]],
                        "scheme": "tei-xpath-like-inventory-v1",
                    }
                ],
                "resource-inventory TEI division locator",
            ),
            (
                "target_epub",
                "naumann-1893-epub",
                correspondence["target_epub"],
                epub_witness.get("file_sha256"),
                [
                    {
                        "type": "container_member",
                        "member_path": correspondence["target_epub"]["member_path"],
                        "member_sha256": correspondence["target_epub"]["member_sha256"],
                    }
                ],
                "resource-inventory exact EPUB member locator",
            ),
            (
                "target_pdf",
                "naumann-1893-pdf",
                correspondence["target_pdf"],
                pdf_witness.get("file_sha256"),
                [
                    {
                        "type": "page_region",
                        "page": correspondence["target_pdf"]["page_index"],
                        "x": 0,
                        "y": 0,
                        "width": 1,
                        "height": 1,
                        "coordinate_space": "normalized_0_1",
                    }
                ],
                "resource-inventory whole-page PDF locator",
            ),
        )
        for (
            role,
            id_role,
            locator,
            file_sha256,
            selectors,
            method,
        ) in expected_roles:
            expected_anchor_id = _expected_anchor_id(
                correspondence_id,
                id_role,
            )
            expected_anchor_order.append(expected_anchor_id)
            expected_binding = {
                "anchor_ref": expected_anchor_id,
                "item_ref": locator["item_ref"],
                "file_ref": locator["file_ref"],
                "resource_id": locator["resource_id"],
            }
            if binding.get(role) != expected_binding:
                issues.append(
                    (
                        binding_location,
                        f"{role} binding differs from correspondence",
                    )
                )
            anchor = anchors_by_id.get(expected_anchor_id)
            if anchor is None:
                issues.append(
                    (
                        binding_location,
                        f"{role} anchor is unresolved: {expected_anchor_id}",
                    )
                )
                continue
            expected_anchor = {
                "item_id": locator["item_ref"],
                "file_id": locator["file_ref"],
                "file_sha256": file_sha256,
                "selectors": selectors,
                "selector_method": {
                    "maker_type": "software",
                    "method": method,
                    "version": "1",
                    "configuration_ref": (
                        f"{MAP_PATH.as_posix()}#{correspondence_id}"
                    ),
                },
                "status": "proposed",
                "provenance_event_ref": ANCHOR_EVENT_ID,
                "anchor_version": 1,
                "supersedes_anchor_ref": None,
                "review_ref": None,
                "passage_id": None,
            }
            for field, expected in expected_anchor.items():
                if anchor.get(field) != expected:
                    issues.append(
                        (
                            binding_location,
                            f"{role} anchor {field} differs from source map",
                        )
                    )

    if set(bindings_by_id) != set(correspondences_by_id):
        issues.append(
            (
                ANCHOR_SET_PATH.as_posix(),
                "anchor bindings do not cover map correspondences exactly",
            )
        )
    actual_anchor_order = [
        anchor.get("anchor_id")
        for anchor in anchor_records
        if isinstance(anchor.get("anchor_id"), str)
    ]
    if actual_anchor_order != expected_anchor_order:
        issues.append(
            (
                ANCHOR_RECORDS_PATH.as_posix(),
                "anchor record order differs from correspondence role order",
            )
        )
    if set(anchors_by_id) != set(expected_anchor_order):
        issues.append(
            (
                ANCHOR_RECORDS_PATH.as_posix(),
                "anchor records do not close over the expected stable ID set",
            )
        )
    if isinstance(anchor_set, dict):
        correspondence_count = len(correspondences_by_id)
        expected_anchor_summary = {
            "correspondence_count": correspondence_count,
            "anchor_count": correspondence_count * 3,
            "anchors_per_correspondence": 3,
            "role_counts": {
                "source_tei": correspondence_count,
                "target_epub": correspondence_count,
                "target_pdf": correspondence_count,
            },
            "all_anchor_statuses": ["proposed"],
        }
        if anchor_set.get("summary") != expected_anchor_summary:
            issues.append(
                (ANCHOR_SET_PATH.as_posix(), "anchor summary drifted")
            )

    provenance_ref = payload.get("provenance_ref")
    if isinstance(provenance_ref, str):
        provenance_path = repo_root / provenance_ref
        try:
            lines = [
                line
                for line in provenance_path.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ]
        except OSError as exc:
            issues.append((provenance_ref, f"cannot read provenance: {exc}"))
            lines = []
        events: list[dict[str, Any]] = []
        provenance_validator = _validator(PROVENANCE_SCHEMA_PATH, repo_root)
        for index, line in enumerate(lines, start=1):
            try:
                event = json.loads(line)
            except json.JSONDecodeError as exc:
                issues.append((f"{provenance_ref}:{index}", f"invalid JSON: {exc}"))
                continue
            if not isinstance(event, dict):
                issues.append((f"{provenance_ref}:{index}", "event must be an object"))
                continue
            _validate_schema(
                event,
                validator=provenance_validator,
                location=f"{provenance_ref}:{index}",
                issues=issues,
            )
            events.append(event)
        event_ref = payload.get("provenance_event_ref")
        matches = [event for event in events if event.get("event_id") == event_ref]
        if len(matches) != 1:
            issues.append((provenance_ref, "provenance_event_ref does not resolve exactly once"))
        else:
            event = matches[0]
            expected_output = {
                "ref": MAP_PATH.as_posix(),
                "role": "tracked_text_free_structure_correspondence_candidate",
                "sha256": _sha256(map_path),
            }
            if expected_output not in event.get("outputs", []):
                issues.append((provenance_ref, "event lacks digest-bound map output"))
            expected_inputs = {
                (witness.get("file_ref"), witness.get("file_sha256"))
                for witness in all_witnesses
            }
            actual_inputs = {
                (entry.get("ref"), entry.get("sha256"))
                for entry in event.get("inputs", [])
                if isinstance(entry, dict)
            }
            if actual_inputs != expected_inputs:
                issues.append((provenance_ref, "event input file/digest set drifted"))

        anchor_event_ref = (
            anchor_set.get("provenance_event_ref")
            if isinstance(anchor_set, dict)
            else None
        )
        anchor_matches = [
            event for event in events if event.get("event_id") == anchor_event_ref
        ]
        if len(anchor_matches) != 1:
            issues.append(
                (
                    provenance_ref,
                    "anchor provenance_event_ref does not resolve exactly once",
                )
            )
        elif anchor_set_path.is_file() and anchor_records_path.is_file():
            anchor_event = anchor_matches[0]
            expected_anchor_inputs = {
                (
                    MAP_PATH.as_posix(),
                    _sha256(map_path),
                )
            }
            actual_anchor_inputs = {
                (entry.get("ref"), entry.get("sha256"))
                for entry in anchor_event.get("inputs", [])
                if isinstance(entry, dict)
            }
            if actual_anchor_inputs != expected_anchor_inputs:
                issues.append(
                    (
                        provenance_ref,
                        "anchor event input map/digest set drifted",
                    )
                )
            expected_anchor_outputs = {
                (
                    ANCHOR_SET_PATH.as_posix(),
                    "tracked_proposed_structure_anchor_set",
                    _sha256(anchor_set_path),
                ),
                (
                    ANCHOR_RECORDS_PATH.as_posix(),
                    "tracked_proposed_source_anchor_records",
                    _sha256(anchor_records_path),
                ),
            }
            actual_anchor_outputs = {
                (
                    entry.get("ref"),
                    entry.get("role"),
                    entry.get("sha256"),
                )
                for entry in anchor_event.get("outputs", [])
                if isinstance(entry, dict)
            }
            if actual_anchor_outputs != expected_anchor_outputs:
                issues.append(
                    (
                        provenance_ref,
                        "anchor event output ref/role/digest set drifted",
                    )
                )
    else:
        issues.append((location, "provenance_ref is invalid"))

    return issues


def _load_parallel_witness(
    witness: dict[str, Any],
    *,
    repo_root: Path,
    issues: list[Issue],
    location: str,
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]] | None:
    inventory_binding = witness.get("inventory")
    if not isinstance(inventory_binding, dict):
        issues.append((location, "witness inventory binding is invalid"))
        return None
    inventory_ref = inventory_binding.get("ref")
    if not isinstance(inventory_ref, str):
        issues.append((location, "witness inventory ref is invalid"))
        return None
    inventory_path = repo_root / inventory_ref
    inventory = _load_json(inventory_path, repo_root, issues)
    if inventory is None:
        return None
    if inventory_binding.get("sha256") != _sha256(inventory_path):
        issues.append((location, "witness inventory digest drifted"))
    if inventory.get("item_id") != witness.get("item_ref"):
        issues.append((location, "witness item_ref differs from inventory"))
    files = [
        item
        for item in inventory.get("files", [])
        if isinstance(item, dict) and item.get("file_id") == witness.get("file_ref")
    ]
    if len(files) != 1:
        issues.append((location, "witness file_ref does not resolve exactly once"))
        return None
    file_inventory = files[0]
    if file_inventory.get("file_sha256") != witness.get("file_sha256"):
        issues.append((location, "witness file digest differs from inventory"))
    if file_inventory.get("profile") != witness.get("profile"):
        issues.append((location, "witness profile differs from inventory"))

    work_boundary = witness.get("work_boundary")
    if isinstance(work_boundary, dict):
        boundary_ref = work_boundary.get("ref")
        if not isinstance(boundary_ref, str):
            issues.append((location, "work-boundary ref is invalid"))
        else:
            boundary_path = repo_root / boundary_ref
            if not boundary_path.is_file():
                issues.append((location, "work-boundary artifact is missing"))
            elif work_boundary.get("sha256") != _sha256(boundary_path):
                issues.append((location, "work-boundary digest drifted"))
    return file_inventory, _resources(file_inventory)


def validate_parallel_structure_correspondence(
    repo_root: Path = REPO_ROOT,
) -> list[Issue]:
    repo_root = repo_root.resolve()
    issues: list[Issue] = []
    map_path = repo_root / PARALLEL_MAP_PATH
    payload = _load_json(map_path, repo_root, issues)
    if payload is None:
        return issues
    location = PARALLEL_MAP_PATH.as_posix()
    _validate_schema(
        payload,
        validator=_validator(PARALLEL_SCHEMA_PATH, repo_root),
        location=location,
        issues=issues,
    )
    for path in _prohibited_key_paths(payload):
        issues.append((location, f"source-text-bearing key is forbidden: {path}"))

    source_witness = payload.get("source_witness", {})
    target_witness = payload.get("target_witness", {})
    witness_resources: dict[str, dict[str, dict[str, Any]]] = {}
    for role, witness in (
        ("source", source_witness),
        ("target", target_witness),
    ):
        if not isinstance(witness, dict):
            issues.append((location, f"{role}_witness is invalid"))
            continue
        result = _load_parallel_witness(
            witness,
            repo_root=repo_root,
            issues=issues,
            location=f"{location}:{role}_witness",
        )
        if result is not None and isinstance(witness.get("item_ref"), str):
            witness_resources[witness["item_ref"]] = result[1]
    if (
        isinstance(source_witness, dict)
        and isinstance(target_witness, dict)
        and source_witness.get("language") == target_witness.get("language")
    ):
        issues.append((location, "parallel witnesses must have distinct languages"))

    method = payload.get("method", {})
    source_unit_binding = (
        method.get("source_numbered_unit_map")
        if isinstance(method, dict)
        else None
    )
    source_unit_map: dict[str, Any] | None = None
    if not isinstance(source_unit_binding, dict):
        issues.append((location, "source numbered-unit map binding is invalid"))
    elif source_unit_binding.get("ref") != NUMBERED_UNIT_MAP_PATH.as_posix():
        issues.append((location, "source numbered-unit map ref drifted"))
    else:
        source_unit_map_path = repo_root / NUMBERED_UNIT_MAP_PATH
        source_unit_map = _load_json(source_unit_map_path, repo_root, issues)
        if (
            source_unit_map is not None
            and source_unit_binding.get("sha256")
            != _sha256(source_unit_map_path)
        ):
            issues.append((location, "source numbered-unit map digest drifted"))
    if source_unit_map is not None and isinstance(source_witness, dict):
        expected_source_identity = {
            "work_ref": payload.get("work_ref"),
            "expression_ref": source_witness.get("expression_ref"),
            "item_ref": source_witness.get("item_ref"),
        }
        for field, expected in expected_source_identity.items():
            if source_unit_map.get(field) != expected:
                issues.append(
                    (
                        location,
                        f"source numbered-unit map {field} differs from witness",
                    )
                )
    supplemental_postures = (
        method.get("supplemental_unit_postures", [])
        if isinstance(method, dict)
        else []
    )
    postures_by_key = {
        posture.get("unit_key"): posture
        for posture in supplemental_postures
        if isinstance(posture, dict)
    }
    expected_target_states = {
        "65a": "not_individually_reviewed",
        "73a": "not_individually_reviewed",
        "237a": "corresponding_prose_present_without_repeated_unit_label",
    }
    if (
        len(postures_by_key) != len(supplemental_postures)
        or set(postures_by_key) != set(expected_target_states)
    ):
        issues.append((location, "supplemental-unit posture set drifted"))
    else:
        for unit_key, target_state in expected_target_states.items():
            if postures_by_key[unit_key].get("target_state") != target_state:
                issues.append(
                    (
                        location,
                        f"supplemental unit {unit_key} target posture drifted",
                    )
                )

    anchor_path = repo_root / PARALLEL_ANCHOR_RECORDS_PATH
    anchors = _load_jsonl(anchor_path, repo_root, issues)
    anchor_validator = _validator(ANCHOR_SCHEMA_PATH, repo_root)
    anchors_by_id: dict[str, dict[str, Any]] = {}
    for index, anchor in enumerate(anchors, start=1):
        anchor_location = f"{PARALLEL_ANCHOR_RECORDS_PATH.as_posix()}:{index}"
        _validate_schema(
            anchor,
            validator=anchor_validator,
            location=anchor_location,
            issues=issues,
        )
        for path in _prohibited_key_paths(anchor):
            issues.append(
                (
                    anchor_location,
                    f"source-text-bearing key is forbidden: {path}",
                )
            )
        anchor_id = anchor.get("anchor_id")
        if not isinstance(anchor_id, str) or anchor_id in anchors_by_id:
            issues.append((anchor_location, "anchor_id is invalid or duplicated"))
        else:
            anchors_by_id[anchor_id] = anchor

    divisions = [
        division
        for division in payload.get("divisions", [])
        if isinstance(division, dict)
    ]
    sequences: list[int] = []
    source_pages: list[int] = []
    target_pages: list[int] = []
    expected_anchor_ids: set[str] = set()
    numbered_spans: list[tuple[int, int]] = []
    supplemental_units: list[str] = []
    for index, division in enumerate(divisions, start=1):
        entry_location = f"{location}:divisions[{index}]"
        sequence = division.get("sequence")
        if isinstance(sequence, int):
            sequences.append(sequence)
        span = division.get("numbered_unit_span")
        kind = division.get("division_kind")
        if kind == "numbered_main_division":
            if not isinstance(span, dict):
                issues.append((entry_location, "numbered division lacks a span"))
            else:
                first = span.get("first")
                last = span.get("last")
                if isinstance(first, int) and isinstance(last, int):
                    if first > last:
                        issues.append((entry_location, "numbered span is reversed"))
                    numbered_spans.append((first, last))
            if "numbered_unit_boundary" not in division.get(
                "correspondence_basis", []
            ):
                issues.append(
                    (entry_location, "numbered division lacks boundary basis")
                )
        elif span is not None:
            issues.append((entry_location, "non-numbered division carries a span"))
        supplemental_units.extend(division.get("supplemental_numbered_units", []))

        for role, witness, page_accumulator in (
            ("source", source_witness, source_pages),
            ("target", target_witness, target_pages),
        ):
            locator = division.get(role, {})
            if not isinstance(locator, dict) or not isinstance(witness, dict):
                issues.append((entry_location, f"{role} locator is invalid"))
                continue
            for locator_field, witness_field in (
                ("item_ref", "item_ref"),
                ("file_ref", "file_ref"),
            ):
                if locator.get(locator_field) != witness.get(witness_field):
                    issues.append(
                        (
                            entry_location,
                            f"{role} {locator_field} differs from witness",
                        )
                    )
            inventory_binding = witness.get("inventory", {})
            if locator.get("inventory_ref") != inventory_binding.get("ref"):
                issues.append(
                    (entry_location, f"{role} inventory_ref differs from witness")
                )
            resource = witness_resources.get(locator.get("item_ref"), {}).get(
                locator.get("resource_id")
            )
            if resource is None:
                issues.append((entry_location, f"{role} resource is unresolved"))
            elif (
                resource.get("resource_kind") != "pdf_page"
                or resource.get("locator", {}).get("page_index")
                != locator.get("pdf_page")
            ):
                issues.append(
                    (entry_location, f"{role} page differs from inventory")
                )
            page = locator.get("pdf_page")
            if isinstance(page, int):
                page_accumulator.append(page)

            anchor_ref = locator.get("anchor_ref")
            if isinstance(anchor_ref, str):
                expected_anchor_ids.add(anchor_ref)
            anchor = anchors_by_id.get(anchor_ref)
            if anchor is None:
                issues.append((entry_location, f"{role} anchor is unresolved"))
                continue
            expected_file_digest = witness.get("file_sha256")
            if (
                anchor.get("item_id") != witness.get("item_ref")
                or anchor.get("file_id") != witness.get("file_ref")
                or anchor.get("file_sha256") != expected_file_digest
            ):
                issues.append(
                    (entry_location, f"{role} anchor crosses its witness")
                )
            if anchor.get("status") != "proposed":
                issues.append((entry_location, f"{role} anchor is not proposed"))
            if anchor.get("provenance_event_ref") != payload.get(
                "provenance_event_ref"
            ):
                issues.append(
                    (entry_location, f"{role} anchor provenance differs from map")
                )
            selectors = anchor.get("selectors", [])
            if len(selectors) != 1 or not isinstance(selectors[0], dict):
                issues.append(
                    (entry_location, f"{role} anchor must have one selector")
                )
            else:
                selector = selectors[0]
                expected_selector = {
                    "type": "page_region",
                    "page": page,
                    "x": 0,
                    "y": 0,
                    "width": 1,
                    "height": 1,
                    "coordinate_space": "normalized_0_1",
                }
                if selector != expected_selector:
                    issues.append(
                        (
                            entry_location,
                            f"{role} anchor is not the declared whole page",
                        )
                    )

    if sequences != list(range(1, len(divisions) + 1)):
        issues.append((location, "division sequence is not contiguous"))
    if source_pages != sorted(source_pages) or len(set(source_pages)) != len(
        source_pages
    ):
        issues.append((location, "source division pages are not strictly monotonic"))
    if target_pages != sorted(target_pages) or len(set(target_pages)) != len(
        target_pages
    ):
        issues.append((location, "target division pages are not strictly monotonic"))
    if set(anchors_by_id) != expected_anchor_ids:
        issues.append((location, "anchor records differ from division bindings"))

    flattened_units = [
        number
        for first, last in numbered_spans
        for number in range(first, last + 1)
    ]
    if flattened_units != list(range(1, 297)):
        issues.append(
            (
                location,
                "numbered division spans do not cover integers 1 through 296 once",
            )
        )

    boundary_binding = (
        target_witness.get("work_boundary")
        if isinstance(target_witness, dict)
        else None
    )
    if isinstance(boundary_binding, dict):
        boundary_path = repo_root / str(boundary_binding.get("ref"))
        boundary = _load_json(boundary_path, repo_root, issues)
        if boundary is not None:
            members = [
                member
                for member in boundary.get("members", [])
                if isinstance(member, dict)
                and member.get("work_ref") == payload.get("work_ref")
                and member.get("expression_ref")
                == target_witness.get("expression_ref")
            ]
            if len(members) != 1:
                issues.append(
                    (location, "target work boundary does not resolve exactly once")
                )
            elif target_pages and not all(
                members[0].get("start_page") <= page <= members[0].get("end_page")
                for page in target_pages
            ):
                issues.append((location, "target division page leaves work boundary"))

    summary = payload.get("summary", {})
    if isinstance(summary, dict):
        expected_summary = {
            "division_correspondence_count": len(divisions),
            "anchor_count": len(anchors),
            "numbered_division_count": len(numbered_spans),
            "integer_numbered_units_covered": len(flattened_units),
            "supplemental_numbered_units": supplemental_units,
        }
        for field, value in expected_summary.items():
            if summary.get(field) != value:
                issues.append((location, f"summary {field} drifted"))

    provenance_ref = payload.get("provenance_ref")
    if not isinstance(provenance_ref, str):
        issues.append((location, "provenance_ref is invalid"))
        return issues
    provenance_path = repo_root / provenance_ref
    events = _load_jsonl(provenance_path, repo_root, issues)
    provenance_validator = _validator(PROVENANCE_SCHEMA_PATH, repo_root)
    for index, event in enumerate(events, start=1):
        _validate_schema(
            event,
            validator=provenance_validator,
            location=f"{provenance_ref}:{index}",
            issues=issues,
        )
    event_matches = [
        event
        for event in events
        if event.get("event_id") == payload.get("provenance_event_ref")
    ]
    if len(event_matches) != 1 or payload.get("provenance_event_ref") != PARALLEL_EVENT_ID:
        issues.append((provenance_ref, "parallel provenance event does not resolve"))
    else:
        event = event_matches[0]
        expected_outputs = {
            (
                PARALLEL_MAP_PATH.as_posix(),
                "tracked_text_free_parallel_structure_candidate",
                _sha256(map_path),
            ),
            (
                PARALLEL_ANCHOR_RECORDS_PATH.as_posix(),
                "tracked_proposed_parallel_page_anchors",
                _sha256(anchor_path),
            ),
        }
        actual_outputs = {
            (output.get("ref"), output.get("role"), output.get("sha256"))
            for output in event.get("outputs", [])
            if isinstance(output, dict)
        }
        if actual_outputs != expected_outputs:
            issues.append((provenance_ref, "parallel event outputs drifted"))

        expected_inputs: set[tuple[object, object]] = set()
        for witness in (source_witness, target_witness):
            if not isinstance(witness, dict):
                continue
            expected_inputs.add((witness.get("file_ref"), witness.get("file_sha256")))
            inventory_binding = witness.get("inventory", {})
            if isinstance(inventory_binding, dict):
                expected_inputs.add(
                    (inventory_binding.get("ref"), inventory_binding.get("sha256"))
                )
            boundary_binding = witness.get("work_boundary")
            if isinstance(boundary_binding, dict):
                expected_inputs.add(
                    (boundary_binding.get("ref"), boundary_binding.get("sha256"))
                )
        method = payload.get("method", {})
        source_unit_map = (
            method.get("source_numbered_unit_map")
            if isinstance(method, dict)
            else None
        )
        if isinstance(source_unit_map, dict):
            expected_inputs.add(
                (source_unit_map.get("ref"), source_unit_map.get("sha256"))
            )
        actual_inputs = {
            (input_entry.get("ref"), input_entry.get("sha256"))
            for input_entry in event.get("inputs", [])
            if isinstance(input_entry, dict)
        }
        if actual_inputs != expected_inputs:
            issues.append((provenance_ref, "parallel event inputs drifted"))

    return issues


def _numbered_unit_keys() -> list[str]:
    keys: list[str] = []
    for number in range(1, 297):
        keys.append(str(number))
        if number in {65, 73, 237}:
            keys.append(f"{number}a")
    return keys


def validate_numbered_unit_page_map(
    repo_root: Path = REPO_ROOT,
) -> list[Issue]:
    repo_root = repo_root.resolve()
    issues: list[Issue] = []
    map_path = repo_root / NUMBERED_UNIT_MAP_PATH
    payload = _load_json(map_path, repo_root, issues)
    if payload is None:
        return issues
    location = NUMBERED_UNIT_MAP_PATH.as_posix()
    _validate_schema(
        payload,
        validator=_validator(NUMBERED_UNIT_SCHEMA_PATH, repo_root),
        location=location,
        issues=issues,
    )
    for path in _prohibited_key_paths(payload):
        issues.append((location, f"source-text-bearing key is forbidden: {path}"))

    inventory_binding = payload.get("inventory", {})
    inventory_ref = (
        inventory_binding.get("ref")
        if isinstance(inventory_binding, dict)
        else None
    )
    inventory_path = repo_root / str(inventory_ref)
    inventory = _load_json(inventory_path, repo_root, issues)
    if not isinstance(inventory_ref, str) or inventory is None:
        issues.append((location, "resource inventory binding is invalid"))
        return issues
    if inventory_binding.get("sha256") != _sha256(inventory_path):
        issues.append((location, "resource inventory digest drifted"))

    inventory_files = {
        file_inventory.get("profile"): file_inventory
        for file_inventory in inventory.get("files", [])
        if isinstance(file_inventory, dict)
    }
    expected_profiles = {
        "pdf_pages_v1",
        "djvu_xml_pages_v1",
        "abbyy_xml_pages_v1",
    }
    if set(inventory_files) != expected_profiles:
        issues.append((location, "resource inventory profile set drifted"))
    for profile in expected_profiles:
        file_inventory = inventory_files.get(profile, {})
        if file_inventory.get("summary", {}).get("page_count") != 274:
            issues.append((location, f"{profile} page count drifted"))

    scan_file = payload.get("scan_file", {})
    pdf_inventory = inventory_files.get("pdf_pages_v1", {})
    if (
        scan_file.get("file_ref") != pdf_inventory.get("file_id")
        or scan_file.get("file_sha256") != pdf_inventory.get("file_sha256")
    ):
        issues.append((location, "scan file differs from PDF inventory"))
    navigation_bindings = {
        entry.get("inventory_profile"): entry
        for entry in payload.get("navigation_files", [])
        if isinstance(entry, dict)
    }
    expected_navigation_roles = {
        "djvu_xml_pages_v1": "djvu_xml",
        "abbyy_xml_pages_v1": "abbyy_xml_gzip",
    }
    if set(navigation_bindings) != set(expected_navigation_roles):
        issues.append((location, "navigation profile set drifted"))
    for profile, role in expected_navigation_roles.items():
        binding = navigation_bindings.get(profile, {})
        file_inventory = inventory_files.get(profile, {})
        if (
            binding.get("role") != role
            or binding.get("file_ref") != file_inventory.get("file_id")
            or binding.get("file_sha256") != file_inventory.get("file_sha256")
        ):
            issues.append((location, f"{profile} binding differs from inventory"))

    pdf_resources = _resources(pdf_inventory)
    expected_keys = _numbered_unit_keys()
    unit_starts = [
        unit
        for unit in payload.get("unit_starts", [])
        if isinstance(unit, dict)
    ]
    unit_keys = [unit.get("unit_key") for unit in unit_starts]
    sequences = [unit.get("sequence") for unit in unit_starts]
    pages = [unit.get("pdf_page") for unit in unit_starts]
    if unit_keys != expected_keys:
        issues.append((location, "numbered-unit sequence drifted"))
    if sequences != list(range(1, 300)):
        issues.append((location, "numbered-unit ordinal sequence drifted"))
    if pages != sorted(pages):
        issues.append((location, "numbered-unit pages are not monotonic"))

    anchor_path = repo_root / NUMBERED_UNIT_ANCHOR_RECORDS_PATH
    anchors = _load_jsonl(anchor_path, repo_root, issues)
    anchor_validator = _validator(ANCHOR_SCHEMA_PATH, repo_root)
    anchors_by_id: dict[str, dict[str, Any]] = {}
    for index, anchor in enumerate(anchors, start=1):
        anchor_location = (
            f"{NUMBERED_UNIT_ANCHOR_RECORDS_PATH.as_posix()}:{index}"
        )
        _validate_schema(
            anchor,
            validator=anchor_validator,
            location=anchor_location,
            issues=issues,
        )
        for path in _prohibited_key_paths(anchor):
            issues.append(
                (
                    anchor_location,
                    f"source-text-bearing key is forbidden: {path}",
                )
            )
        anchor_id = anchor.get("anchor_id")
        if not isinstance(anchor_id, str) or anchor_id in anchors_by_id:
            issues.append((anchor_location, "anchor_id is invalid or duplicated"))
        else:
            anchors_by_id[anchor_id] = anchor

    expected_anchor_ids: set[str] = set()
    basis_counts: Counter[str] = Counter()
    for unit in unit_starts:
        unit_key = unit.get("unit_key")
        page = unit.get("pdf_page")
        resource_id = unit.get("resource_id")
        anchor_ref = unit.get("anchor_ref")
        if isinstance(anchor_ref, str):
            expected_anchor_ids.add(anchor_ref)
        basis = unit.get("basis")
        if isinstance(basis, str):
            basis_counts[basis] += 1
        resource = pdf_resources.get(resource_id)
        if (
            resource is None
            or resource.get("resource_kind") != "pdf_page"
            or resource.get("locator", {}).get("page_index") != page
        ):
            issues.append(
                (location, f"numbered unit {unit_key} leaves the PDF inventory")
            )
        anchor = anchors_by_id.get(anchor_ref)
        if anchor is None:
            issues.append((location, f"numbered unit {unit_key} has no anchor"))
            continue
        expected_passage = (
            "tos.passage.friedrich-nietzsche.jenseits-von-gut-und-boese."
            f"de-naumann-1886.unit-{unit_key}"
        )
        if (
            anchor.get("item_id") != payload.get("item_ref")
            or anchor.get("file_id") != scan_file.get("file_ref")
            or anchor.get("file_sha256") != scan_file.get("file_sha256")
            or anchor.get("passage_id") != expected_passage
            or anchor.get("status") != "proposed"
            or anchor.get("provenance_event_ref")
            != payload.get("provenance_event_ref")
        ):
            issues.append(
                (location, f"numbered unit {unit_key} anchor binding drifted")
            )
        selectors = anchor.get("selectors", [])
        selector_types = [
            selector.get("type")
            for selector in selectors
            if isinstance(selector, dict)
        ]
        if selector_types != ["structural", "page_region"]:
            issues.append(
                (location, f"numbered unit {unit_key} selector bundle drifted")
            )
            continue
        expected_structural_selector = {
            "type": "structural",
            "path": [
                "work:jenseits-von-gut-und-boese",
                "expression:de-naumann-1886",
                f"numbered-unit:{unit_key}",
            ],
            "scheme": "jgb-numbered-unit-start-v1",
        }
        if selectors[0] != expected_structural_selector:
            issues.append(
                (location, f"numbered unit {unit_key} structural selector drifted")
            )
        page_selector = selectors[1]
        expected_page_selector = {
            "type": "page_region",
            "page": page,
            "x": 0,
            "y": 0,
            "width": 1,
            "height": 1,
            "coordinate_space": "normalized_0_1",
        }
        if page_selector != expected_page_selector:
            issues.append(
                (location, f"numbered unit {unit_key} page selector drifted")
            )
    if set(anchors_by_id) != expected_anchor_ids or len(anchors) != 299:
        issues.append((location, "numbered-unit anchor set differs from map"))
    expected_basis_counts = {
        "ocr_order_candidate": 266,
        "source_visible_gap_review": 22,
        "source_visible_ocr_disambiguation": 10,
        "source_visible_repeated_number_review": 1,
    }
    if dict(basis_counts) != expected_basis_counts:
        issues.append((location, "numbered-unit basis counts drifted"))

    method = payload.get("method", {})
    review = method.get("source_visible_review", {}) if isinstance(method, dict) else {}
    if isinstance(review, dict):
        declared_basis_keys = {
            "source_visible_gap_review": set(
                review.get("gap_review_unit_keys", [])
            ),
            "source_visible_ocr_disambiguation": set(
                review.get("ocr_disambiguation_unit_keys", [])
            ),
            "source_visible_repeated_number_review": set(
                review.get("repeated_number_unit_keys", [])
            ),
        }
        declared_review_keys: set[object] = set()
        for basis, keys in declared_basis_keys.items():
            if declared_review_keys & keys:
                issues.append(
                    (location, f"declared source-visible {basis} set overlaps")
                )
            declared_review_keys.update(keys)
        materialized_review_keys = {
            unit.get("unit_key")
            for unit in unit_starts
            if unit.get("basis") != "ocr_order_candidate"
        }
        if declared_review_keys != materialized_review_keys:
            issues.append((location, "declared source-visible review set drifted"))
        for unit in unit_starts:
            unit_key = unit.get("unit_key")
            expected_basis = "ocr_order_candidate"
            for basis, keys in declared_basis_keys.items():
                if unit_key in keys:
                    expected_basis = basis
                    break
            if unit.get("basis") != expected_basis:
                issues.append(
                    (location, f"numbered unit {unit_key} review basis drifted")
                )

    summary = payload.get("summary", {})
    if isinstance(summary, dict):
        if summary.get("numbered_unit_count") != len(unit_starts):
            issues.append((location, "numbered-unit summary count drifted"))
        if summary.get("exact_start_page_candidates_materialized") != len(
            unit_starts
        ):
            issues.append((location, "start-page candidate summary drifted"))

    provenance_ref = payload.get("provenance_ref")
    if not isinstance(provenance_ref, str):
        issues.append((location, "provenance_ref is invalid"))
        return issues
    provenance_path = repo_root / provenance_ref
    events = _load_jsonl(provenance_path, repo_root, issues)
    provenance_validator = _validator(PROVENANCE_SCHEMA_PATH, repo_root)
    for index, event in enumerate(events, start=1):
        _validate_schema(
            event,
            validator=provenance_validator,
            location=f"{provenance_ref}:{index}",
            issues=issues,
        )
    event_matches = [
        event
        for event in events
        if event.get("event_id") == payload.get("provenance_event_ref")
    ]
    if (
        len(event_matches) != 1
        or payload.get("provenance_event_ref") != NUMBERED_UNIT_EVENT_ID
    ):
        issues.append((provenance_ref, "numbered-unit provenance event does not resolve"))
    else:
        event = event_matches[0]
        expected_outputs = {
            (
                NUMBERED_UNIT_MAP_PATH.as_posix(),
                "tracked-text-free-numbered-unit-page-map",
                _sha256(map_path),
            ),
            (
                NUMBERED_UNIT_ANCHOR_RECORDS_PATH.as_posix(),
                "tracked-proposed-whole-page-source-anchors",
                _sha256(anchor_path),
            ),
        }
        actual_outputs = {
            (output.get("ref"), output.get("role"), output.get("sha256"))
            for output in event.get("outputs", [])
            if isinstance(output, dict)
        }
        if actual_outputs != expected_outputs:
            issues.append((provenance_ref, "numbered-unit event outputs drifted"))
        expected_inputs = {
            (scan_file.get("file_ref"), scan_file.get("file_sha256")),
            (inventory_ref, inventory_binding.get("sha256")),
            *{
                (entry.get("file_ref"), entry.get("file_sha256"))
                for entry in payload.get("navigation_files", [])
                if isinstance(entry, dict)
            },
        }
        actual_inputs = {
            (input_entry.get("ref"), input_entry.get("sha256"))
            for input_entry in event.get("inputs", [])
            if isinstance(input_entry, dict)
        }
        if actual_inputs != expected_inputs:
            issues.append((provenance_ref, "numbered-unit event inputs drifted"))

    return issues


def _target_numbered_unit_keys() -> list[str]:
    keys: list[str] = []
    for number in range(1, 297):
        keys.append(str(number))
        if number in {65, 73}:
            keys.append(f"{number}a")
    return keys


def validate_target_numbered_unit_page_map(
    repo_root: Path = REPO_ROOT,
) -> list[Issue]:
    """Close the target-only numbered-label map over tracked evidence.

    This establishes reference and arithmetic closure only. It deliberately
    does not compare source and target text or infer translation equivalence.
    """

    repo_root = repo_root.resolve()
    issues: list[Issue] = []
    map_path = repo_root / TARGET_NUMBERED_UNIT_MAP_PATH
    payload = _load_json(map_path, repo_root, issues)
    if payload is None:
        return issues
    location = TARGET_NUMBERED_UNIT_MAP_PATH.as_posix()
    _validate_schema(
        payload,
        validator=_validator(TARGET_NUMBERED_UNIT_SCHEMA_PATH, repo_root),
        location=location,
        issues=issues,
    )
    for path in _prohibited_key_paths(payload):
        issues.append((location, f"source-text-bearing key is forbidden: {path}"))

    inventory_binding = payload.get("inventory", {})
    inventory_ref = (
        inventory_binding.get("ref")
        if isinstance(inventory_binding, dict)
        else None
    )
    if not isinstance(inventory_ref, str):
        issues.append((location, "target resource inventory binding is invalid"))
        return issues
    inventory_path = repo_root / inventory_ref
    inventory = _load_json(inventory_path, repo_root, issues)
    if inventory is None:
        return issues
    if inventory_binding.get("sha256") != _sha256(inventory_path):
        issues.append((location, "target resource inventory digest drifted"))

    inventory_files = [
        file_inventory
        for file_inventory in inventory.get("files", [])
        if isinstance(file_inventory, dict)
    ]
    if len(inventory_files) != 1:
        issues.append((location, "target resource inventory file set drifted"))
        return issues
    pdf_inventory = inventory_files[0]
    if (
        pdf_inventory.get("profile") != "pdf_pages_v1"
        or pdf_inventory.get("summary", {}).get("page_count") != 831
    ):
        issues.append((location, "target PDF inventory profile or count drifted"))

    scan_file = payload.get("scan_file", {})
    if (
        not isinstance(scan_file, dict)
        or scan_file.get("file_ref") != pdf_inventory.get("file_id")
        or scan_file.get("file_sha256") != pdf_inventory.get("file_sha256")
        or scan_file.get("inventory_profile") != "pdf_pages_v1"
        or inventory.get("item_id") != payload.get("item_ref")
    ):
        issues.append((location, "target scan file differs from inventory"))

    boundary_binding = payload.get("work_boundary", {})
    boundary_ref = (
        boundary_binding.get("ref")
        if isinstance(boundary_binding, dict)
        else None
    )
    if not isinstance(boundary_ref, str):
        issues.append((location, "target work-boundary binding is invalid"))
        return issues
    boundary_path = repo_root / boundary_ref
    boundary = _load_json(boundary_path, repo_root, issues)
    if boundary is None:
        return issues
    if boundary_binding.get("sha256") != _sha256(boundary_path):
        issues.append((location, "target work-boundary digest drifted"))
    members = [
        member
        for member in boundary.get("members", [])
        if isinstance(member, dict)
        and member.get("sequence") == boundary_binding.get("member_sequence")
        and member.get("work_ref") == payload.get("work_ref")
        and member.get("expression_ref") == payload.get("expression_ref")
    ]
    if len(members) != 1:
        issues.append((location, "target work boundary does not resolve exactly once"))
        member: dict[str, Any] = {}
    else:
        member = members[0]
        expected_boundary_fields = {
            "start_page": member.get("start_page"),
            "end_page": member.get("end_page"),
            "epistemic_status": member.get("epistemic_status"),
            "review_status": member.get("review_status"),
        }
        for field, value in expected_boundary_fields.items():
            if boundary_binding.get(field) != value:
                issues.append((location, f"target work-boundary {field} drifted"))

    resources = _resources(pdf_inventory)
    expected_keys = _target_numbered_unit_keys()
    unit_starts = [
        unit
        for unit in payload.get("unit_starts", [])
        if isinstance(unit, dict)
    ]
    unit_keys = [unit.get("unit_key") for unit in unit_starts]
    sequences = [unit.get("sequence") for unit in unit_starts]
    pages = [unit.get("pdf_page") for unit in unit_starts]
    if unit_keys != expected_keys:
        issues.append((location, "target numbered-unit sequence drifted"))
    if "237a" in unit_keys:
        issues.append((location, "source-only 237a was materialized in target"))
    if sequences != list(range(1, 299)):
        issues.append((location, "target numbered-unit ordinal sequence drifted"))
    if pages != sorted(pages):
        issues.append((location, "target numbered-unit pages are not monotonic"))
    if member and not all(
        member.get("start_page") <= page <= member.get("end_page")
        for page in pages
        if isinstance(page, int)
    ):
        issues.append((location, "target numbered-unit page leaves work boundary"))

    anchor_path = repo_root / TARGET_NUMBERED_UNIT_ANCHOR_RECORDS_PATH
    anchors = _load_jsonl(anchor_path, repo_root, issues)
    anchor_validator = _validator(ANCHOR_SCHEMA_PATH, repo_root)
    anchors_by_id: dict[str, dict[str, Any]] = {}
    for index, anchor in enumerate(anchors, start=1):
        anchor_location = (
            f"{TARGET_NUMBERED_UNIT_ANCHOR_RECORDS_PATH.as_posix()}:{index}"
        )
        _validate_schema(
            anchor,
            validator=anchor_validator,
            location=anchor_location,
            issues=issues,
        )
        for path in _prohibited_key_paths(anchor):
            issues.append(
                (
                    anchor_location,
                    f"source-text-bearing key is forbidden: {path}",
                )
            )
        anchor_id = anchor.get("anchor_id")
        if not isinstance(anchor_id, str) or anchor_id in anchors_by_id:
            issues.append((anchor_location, "anchor_id is invalid or duplicated"))
        else:
            anchors_by_id[anchor_id] = anchor

    expected_anchor_ids: set[str] = set()
    basis_counts: Counter[str] = Counter()
    basis_keys: defaultdict[str, set[object]] = defaultdict(set)
    for unit in unit_starts:
        unit_key = unit.get("unit_key")
        page = unit.get("pdf_page")
        resource_id = unit.get("resource_id")
        anchor_ref = unit.get("anchor_ref")
        if isinstance(anchor_ref, str):
            expected_anchor_ids.add(anchor_ref)
        basis = unit.get("basis")
        if isinstance(basis, str):
            basis_counts[basis] += 1
            basis_keys[basis].add(unit_key)
        resource = resources.get(resource_id)
        if (
            resource is None
            or resource.get("resource_kind") != "pdf_page"
            or resource.get("locator", {}).get("page_index") != page
        ):
            issues.append(
                (location, f"target numbered unit {unit_key} leaves inventory")
            )
        anchor = anchors_by_id.get(anchor_ref)
        if anchor is None:
            issues.append((location, f"target numbered unit {unit_key} has no anchor"))
            continue
        expected_passage = (
            "tos.passage.friedrich-nietzsche.jenseits-von-gut-und-boese."
            f"ru-polilov-mysl-1996.unit-{unit_key}"
        )
        if (
            anchor.get("item_id") != payload.get("item_ref")
            or anchor.get("file_id") != scan_file.get("file_ref")
            or anchor.get("file_sha256") != scan_file.get("file_sha256")
            or anchor.get("passage_id") != expected_passage
            or anchor.get("status") != "proposed"
            or anchor.get("provenance_event_ref")
            != payload.get("provenance_event_ref")
        ):
            issues.append(
                (location, f"target numbered unit {unit_key} anchor binding drifted")
            )
        selectors = anchor.get("selectors", [])
        selector_types = [
            selector.get("type")
            for selector in selectors
            if isinstance(selector, dict)
        ]
        if selector_types != ["structural", "page_region"]:
            issues.append(
                (location, f"target numbered unit {unit_key} selectors drifted")
            )
            continue
        expected_structural_selector = {
            "type": "structural",
            "path": [
                "work:jenseits-von-gut-und-boese",
                "expression:ru-polilov-mysl-1996",
                f"numbered-unit:{unit_key}",
            ],
            "scheme": "jgb-target-numbered-unit-start-v1",
        }
        if selectors[0] != expected_structural_selector:
            issues.append(
                (
                    location,
                    f"target numbered unit {unit_key} structural selector drifted",
                )
            )
        expected_page_selector = {
            "type": "page_region",
            "page": page,
            "x": 0,
            "y": 0,
            "width": 1,
            "height": 1,
            "coordinate_space": "normalized_0_1",
        }
        if selectors[1] != expected_page_selector:
            issues.append(
                (location, f"target numbered unit {unit_key} page selector drifted")
            )
    if set(anchors_by_id) != expected_anchor_ids or len(anchors) != 298:
        issues.append((location, "target numbered-unit anchor set differs from map"))
    expected_basis_counts = {
        "embedded_pdf_bbox_order_candidate": 264,
        "source_visible_gap_review": 33,
        "source_visible_ocr_disambiguation": 1,
    }
    if dict(basis_counts) != expected_basis_counts:
        issues.append((location, "target numbered-unit basis counts drifted"))

    method = payload.get("method", {})
    review = (
        method.get("source_visible_review", {})
        if isinstance(method, dict)
        else {}
    )
    if isinstance(review, dict):
        declared_gap_keys = set(review.get("gap_review_unit_keys", []))
        declared_disambiguation_keys = set(
            review.get("ocr_disambiguation_unit_keys", [])
        )
        if declared_gap_keys != basis_keys["source_visible_gap_review"]:
            issues.append((location, "target declared gap-review set drifted"))
        if (
            declared_disambiguation_keys
            != basis_keys["source_visible_ocr_disambiguation"]
        ):
            issues.append(
                (location, "target declared OCR-disambiguation set drifted")
            )
        if review.get("supplemental_label_unit_keys") != ["65a", "73a"]:
            issues.append((location, "target supplemental-label review drifted"))

    asymmetries = payload.get("numbering_asymmetries", [])
    if len(asymmetries) != 1 or not isinstance(asymmetries[0], dict):
        issues.append((location, "target numbering asymmetry set drifted"))
        asymmetry: dict[str, Any] = {}
    else:
        asymmetry = asymmetries[0]
    source_map_ref = asymmetry.get("source_map_ref")
    if not isinstance(source_map_ref, str):
        issues.append((location, "target source-map asymmetry ref is invalid"))
    else:
        source_map_path = repo_root / source_map_ref
        source_map = _load_json(source_map_path, repo_root, issues)
        if source_map is not None:
            if asymmetry.get("source_map_sha256") != _sha256(source_map_path):
                issues.append((location, "target source-map asymmetry digest drifted"))
            source_keys = {
                unit.get("unit_key")
                for unit in source_map.get("unit_starts", [])
                if isinstance(unit, dict)
            }
            if "237a" not in source_keys:
                issues.append((location, "bound source map does not contain 237a"))
    if (
        asymmetry.get("source_unit_key") != "237a"
        or asymmetry.get("target_numbered_unit_materialized") is not False
        or asymmetry.get("exact_translation_alignment_claimed") is not False
    ):
        issues.append((location, "target 237a asymmetry posture drifted"))

    summary = payload.get("summary", {})
    if isinstance(summary, dict):
        expected_summary = {
            "integer_numbered_unit_count": 296,
            "supplemental_numbered_units": ["65a", "73a"],
            "source_only_nonmaterialized_numbered_units": ["237a"],
            "numbered_unit_count": len(unit_starts),
            "exact_start_page_candidates_materialized": len(unit_starts),
            "unresolved_unit_count": 0,
            "start_pages_monotonic": pages == sorted(pages),
            "all_anchor_statuses": ["proposed"],
            "human_review_performed": False,
        }
        for field, value in expected_summary.items():
            if summary.get(field) != value:
                issues.append((location, f"target summary {field} drifted"))

    provenance_ref = payload.get("provenance_ref")
    if not isinstance(provenance_ref, str):
        issues.append((location, "target provenance_ref is invalid"))
        return issues
    provenance_path = repo_root / provenance_ref
    events = _load_jsonl(provenance_path, repo_root, issues)
    provenance_validator = _validator(PROVENANCE_SCHEMA_PATH, repo_root)
    for index, event in enumerate(events, start=1):
        _validate_schema(
            event,
            validator=provenance_validator,
            location=f"{provenance_ref}:{index}",
            issues=issues,
        )
    event_matches = [
        event
        for event in events
        if event.get("event_id") == payload.get("provenance_event_ref")
    ]
    if (
        len(event_matches) != 1
        or payload.get("provenance_event_ref") != TARGET_NUMBERED_UNIT_EVENT_ID
    ):
        issues.append(
            (provenance_ref, "target numbered-unit provenance event does not resolve")
        )
    else:
        event = event_matches[0]
        expected_outputs = {
            (
                TARGET_NUMBERED_UNIT_MAP_PATH.as_posix(),
                "tracked-text-free-target-numbered-unit-page-map",
                _sha256(map_path),
            ),
            (
                TARGET_NUMBERED_UNIT_ANCHOR_RECORDS_PATH.as_posix(),
                "tracked-proposed-whole-page-target-anchors",
                _sha256(anchor_path),
            ),
        }
        actual_outputs = {
            (output.get("ref"), output.get("role"), output.get("sha256"))
            for output in event.get("outputs", [])
            if isinstance(output, dict)
        }
        if actual_outputs != expected_outputs:
            issues.append((provenance_ref, "target event outputs drifted"))
        expected_inputs = {
            (scan_file.get("file_ref"), scan_file.get("file_sha256")),
            (inventory_ref, inventory_binding.get("sha256")),
            (boundary_ref, boundary_binding.get("sha256")),
            (source_map_ref, asymmetry.get("source_map_sha256")),
        }
        actual_inputs = {
            (input_entry.get("ref"), input_entry.get("sha256"))
            for input_entry in event.get("inputs", [])
            if isinstance(input_entry, dict)
        }
        if actual_inputs != expected_inputs:
            issues.append((provenance_ref, "target event inputs drifted"))

    return issues


def validate_numbered_unit_label_correspondence(
    repo_root: Path = REPO_ROOT,
) -> list[Issue]:
    """Close shared structural labels without treating them as translations."""

    repo_root = repo_root.resolve()
    issues: list[Issue] = []
    map_path = repo_root / NUMBERED_UNIT_LABEL_MAP_PATH
    payload = _load_json(map_path, repo_root, issues)
    if payload is None:
        return issues
    location = NUMBERED_UNIT_LABEL_MAP_PATH.as_posix()
    _validate_schema(
        payload,
        validator=_validator(NUMBERED_UNIT_LABEL_SCHEMA_PATH, repo_root),
        location=location,
        issues=issues,
    )
    for path in _prohibited_key_paths(payload):
        issues.append((location, f"source-text-bearing key is forbidden: {path}"))

    source_witness = payload.get("source_witness", {})
    target_witness = payload.get("target_witness", {})
    if not isinstance(source_witness, dict) or not isinstance(target_witness, dict):
        issues.append((location, "numbered-label witness bindings are invalid"))
        return issues
    witness_inputs = (
        (source_witness, NUMBERED_UNIT_MAP_PATH, "source"),
        (target_witness, TARGET_NUMBERED_UNIT_MAP_PATH, "target"),
    )
    unit_maps: dict[str, dict[str, dict[str, Any]]] = {}
    for witness, expected_path, role in witness_inputs:
        map_ref = witness.get("numbered_unit_map_ref")
        if map_ref != expected_path.as_posix():
            issues.append((location, f"{role} numbered-unit map ref drifted"))
            continue
        input_path = repo_root / expected_path
        input_payload = _load_json(input_path, repo_root, issues)
        if input_payload is None:
            continue
        if witness.get("numbered_unit_map_sha256") != _sha256(input_path):
            issues.append((location, f"{role} numbered-unit map digest drifted"))
        scan_file = input_payload.get("scan_file", {})
        expected_binding = {
            "expression_ref": input_payload.get("expression_ref"),
            "edition_ref": input_payload.get("edition_ref"),
            "item_ref": input_payload.get("item_ref"),
            "file_ref": (
                scan_file.get("file_ref")
                if isinstance(scan_file, dict)
                else None
            ),
            "file_sha256": (
                scan_file.get("file_sha256")
                if isinstance(scan_file, dict)
                else None
            ),
            "numbered_unit_map_ref": expected_path.as_posix(),
            "numbered_unit_map_sha256": _sha256(input_path),
        }
        if witness != expected_binding:
            issues.append((location, f"{role} witness binding drifted"))
        unit_maps[role] = {
            unit["unit_key"]: unit
            for unit in input_payload.get("unit_starts", [])
            if isinstance(unit, dict) and isinstance(unit.get("unit_key"), str)
        }

    source_units = unit_maps.get("source", {})
    target_units = unit_maps.get("target", {})
    expected_keys = _target_numbered_unit_keys()
    if set(source_units) - set(target_units) != {"237a"}:
        issues.append((location, "source-only numbered-label set drifted"))
    if set(target_units) - set(source_units):
        issues.append((location, "unexpected target-only numbered labels appeared"))

    source_anchors = {
        anchor.get("anchor_id"): anchor
        for anchor in _load_jsonl(
            repo_root / NUMBERED_UNIT_ANCHOR_RECORDS_PATH,
            repo_root,
            issues,
        )
        if isinstance(anchor.get("anchor_id"), str)
    }
    target_anchors = {
        anchor.get("anchor_id"): anchor
        for anchor in _load_jsonl(
            repo_root / TARGET_NUMBERED_UNIT_ANCHOR_RECORDS_PATH,
            repo_root,
            issues,
        )
        if isinstance(anchor.get("anchor_id"), str)
    }
    pairings = [
        pairing
        for pairing in payload.get("pairings", [])
        if isinstance(pairing, dict)
    ]
    pairing_keys = [pairing.get("unit_key") for pairing in pairings]
    sequences = [pairing.get("sequence") for pairing in pairings]
    if pairing_keys != expected_keys:
        issues.append((location, "shared numbered-label pairing sequence drifted"))
    if sequences != list(range(1, 299)):
        issues.append((location, "shared numbered-label ordinal sequence drifted"))
    for pairing in pairings:
        unit_key = pairing.get("unit_key")
        source_unit = source_units.get(unit_key)
        target_unit = target_units.get(unit_key)
        if source_unit is None or target_unit is None:
            issues.append(
                (location, f"pairing {unit_key} does not resolve in both maps")
            )
            continue
        expected_pairing_fields = {
            "source_anchor_ref": source_unit.get("anchor_ref"),
            "source_pdf_page": source_unit.get("pdf_page"),
            "target_anchor_ref": target_unit.get("anchor_ref"),
            "target_pdf_page": target_unit.get("pdf_page"),
            "basis": "shared_materialized_number_label_key",
            "status": "proposed",
            "human_review_performed": False,
            "translation_alignment_claimed": False,
        }
        for field, value in expected_pairing_fields.items():
            if pairing.get(field) != value:
                issues.append(
                    (location, f"pairing {unit_key} field {field} drifted")
                )
        source_anchor = source_anchors.get(pairing.get("source_anchor_ref"))
        target_anchor = target_anchors.get(pairing.get("target_anchor_ref"))
        if (
            source_anchor is None
            or source_anchor.get("passage_id")
            != (
                "tos.passage.friedrich-nietzsche."
                "jenseits-von-gut-und-boese.de-naumann-1886."
                f"unit-{unit_key}"
            )
        ):
            issues.append((location, f"pairing {unit_key} source anchor drifted"))
        if (
            target_anchor is None
            or target_anchor.get("passage_id")
            != (
                "tos.passage.friedrich-nietzsche."
                "jenseits-von-gut-und-boese.ru-polilov-mysl-1996."
                f"unit-{unit_key}"
            )
        ):
            issues.append((location, f"pairing {unit_key} target anchor drifted"))

    unpaired = payload.get("unpaired_units", [])
    if (
        len(unpaired) != 1
        or not isinstance(unpaired[0], dict)
        or unpaired[0].get("unit_key") != "237a"
        or unpaired[0].get("target_unit_materialized") is not False
        or unpaired[0].get("translation_alignment_claimed") is not False
        or "237a" not in source_units
        or "237a" in target_units
    ):
        issues.append((location, "source-only 237a pairing posture drifted"))

    rights_entries = [
        entry
        for entry in payload.get("rights_basis", [])
        if isinstance(entry, dict)
    ]
    rights_roles = {entry.get("role") for entry in rights_entries}
    if rights_roles != {"source", "target"}:
        issues.append((location, "numbered-label rights basis roles drifted"))
    expected_rights_paths = {
        "source": NUMBERED_UNIT_LABEL_SOURCE_RIGHTS_PATH,
        "target": NUMBERED_UNIT_LABEL_TARGET_RIGHTS_PATH,
    }
    for entry in rights_entries:
        role = entry.get("role")
        ref = entry.get("ref")
        if not isinstance(ref, str):
            issues.append((location, "numbered-label rights ref is invalid"))
            continue
        expected_rights_path = expected_rights_paths.get(role)
        if expected_rights_path is None or ref != expected_rights_path.as_posix():
            issues.append((location, f"{role} rights basis ref drifted"))
            continue
        rights_path = repo_root / ref
        if _load_json(rights_path, repo_root, issues) is None:
            continue
        if entry.get("sha256") != _sha256(rights_path):
            issues.append((location, f"rights basis digest drifted for {ref}"))

    summary = payload.get("summary", {})
    if isinstance(summary, dict):
        expected_summary = {
            "source_numbered_unit_count": len(source_units),
            "target_numbered_unit_count": len(target_units),
            "shared_materialized_label_count": len(set(source_units) & set(target_units)),
            "pairing_count": len(pairings),
            "source_only_unit_keys": ["237a"],
            "target_only_unit_keys": [],
            "all_pairing_statuses": ["proposed"],
            "human_review_performed": False,
            "translation_alignment_claimed": False,
        }
        for field, value in expected_summary.items():
            if summary.get(field) != value:
                issues.append((location, f"numbered-label summary {field} drifted"))

    provenance_ref = payload.get("provenance_ref")
    if (
        not isinstance(provenance_ref, str)
        or provenance_ref != NUMBERED_UNIT_LABEL_PROVENANCE_PATH.as_posix()
    ):
        issues.append((location, "numbered-label provenance_ref drifted"))
        return issues
    events = _load_jsonl(repo_root / provenance_ref, repo_root, issues)
    provenance_validator = _validator(PROVENANCE_SCHEMA_PATH, repo_root)
    for index, event in enumerate(events, start=1):
        _validate_schema(
            event,
            validator=provenance_validator,
            location=f"{provenance_ref}:{index}",
            issues=issues,
        )
    event_matches = [
        event
        for event in events
        if event.get("event_id") == payload.get("provenance_event_ref")
    ]
    if (
        len(event_matches) != 1
        or payload.get("provenance_event_ref") != NUMBERED_UNIT_LABEL_EVENT_ID
    ):
        issues.append(
            (provenance_ref, "numbered-label provenance event does not resolve")
        )
    else:
        event = event_matches[0]
        expected_outputs = {
            (
                NUMBERED_UNIT_LABEL_MAP_PATH.as_posix(),
                "tracked-text-free-shared-number-label-pairing-candidates",
                _sha256(map_path),
            )
        }
        actual_outputs = {
            (output.get("ref"), output.get("role"), output.get("sha256"))
            for output in event.get("outputs", [])
            if isinstance(output, dict)
        }
        if actual_outputs != expected_outputs:
            issues.append((provenance_ref, "numbered-label event outputs drifted"))
        expected_inputs = {
            (
                witness.get("numbered_unit_map_ref"),
                witness.get("numbered_unit_map_sha256"),
            )
            for witness in (source_witness, target_witness)
        }
        expected_inputs.update(
            (entry.get("ref"), entry.get("sha256"))
            for entry in rights_entries
        )
        actual_inputs = {
            (input_entry.get("ref"), input_entry.get("sha256"))
            for input_entry in event.get("inputs", [])
            if isinstance(input_entry, dict)
        }
        if actual_inputs != expected_inputs:
            issues.append((provenance_ref, "numbered-label event inputs drifted"))

    return issues


def main() -> int:
    issues = [
        *validate_structure_correspondence(),
        *validate_parallel_structure_correspondence(),
        *validate_numbered_unit_page_map(),
        *validate_target_numbered_unit_page_map(),
        *validate_numbered_unit_label_correspondence(),
    ]
    if issues:
        print("Witness-structure correspondence validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1
    print("[ok] validated text-free witness-structure correspondences")
    print(
        "[boundary] locator candidates only; no textual identity, accepted "
        "original-language text, translation, semantics, or canon promotion"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
