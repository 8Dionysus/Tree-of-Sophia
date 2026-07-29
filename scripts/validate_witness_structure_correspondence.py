#!/usr/bin/env python3
"""Validate the tracked text-free Zarathustra witness-structure map.

This validator checks schema shape, inventory/resource closure, digest-bound
provenance, page-enumeration arithmetic, summaries, and monotonicity. Passing
does not establish textual identity, edition equivalence, accepted German,
translation, semantics, or canon authority.
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
SCHEMA_PATH = Path("ToS/contracts/witness-structure-correspondence.schema.json")
PROVENANCE_SCHEMA_PATH = Path("ToS/contracts/provenance-event.schema.json")
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
    else:
        issues.append((location, "provenance_ref is invalid"))

    return issues


def main() -> int:
    issues = validate_structure_correspondence()
    if issues:
        print("Witness-structure correspondence validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1
    print("[ok] validated text-free Zarathustra witness-structure correspondence")
    print(
        "[boundary] locator candidates only; no textual identity, accepted German, "
        "translation, semantics, or canon promotion"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
