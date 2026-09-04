#!/usr/bin/env python3
"""Build the text-free technical structure spine for German Zarathustra.

The four exact local DTA TEI payloads are rendered into private mode-0600
machine-transcription layers. Tracked outputs contain only opaque identities,
source locators, digests, hierarchy, counts, citation addresses, review status,
and authority limits. No source prose, sentence/word boundary, translation,
semantic relation, graph fact, or canon authority is emitted.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
PLAN_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/technical-markup/"
    "dta-first-editions-parts-1-4-v1/plan.v1.json"
)
BUILDER_REF = "scripts/build_zarathustra_technical_markup.py"
SCHEMA_REF = "ToS/contracts/source-text-unit-packet-v1.schema.json"
PROVENANCE_SCHEMA_REF = "ToS/contracts/provenance-event.schema.json"
TEI_NS = "http://www.tei-c.org/ns/1.0"
SOURCE_ROOT = Path("ToS/source-witnesses")
FIXED_EVENT_ID = (
    "tos.event.segmentation.zarathustra-dta-technical-markup-v1.2026-09-01"
)
ID_NAMESPACE = "tos-zarathustra-dta-technical-markup-v1-opaque-issuance"
ADDRESSABLE_KINDS = {
    "body": "document",
    "div": "section",
    "cit": "section",
    "list": "section",
    "p": "paragraph",
    "lg": "verse_group",
    "l": "verse_line",
    "head": "other",
    "item": "other",
    "quote": "other",
    "bibl": "other",
    "pb": "milestone",
    "milestone": "milestone",
}
INLINE_EXCLUDED = {"fw", "note"}
SEMANTIC_FORBIDDEN_KEYS = {
    "lemma",
    "lexeme",
    "sense",
    "sign",
    "concept",
    "semantic_relation",
    "translation_alignment",
    "canon_ref",
}


class TechnicalMarkupError(RuntimeError):
    """Raised when the bounded technical route drifts or loses closure."""


@dataclass
class UnitObservation:
    part_order: int
    part_label: str
    locator: str
    source_tag: str
    unit_kind: str
    parent_locator: str | None
    child_locators: list[str]
    start: int
    end: int
    resource_id: str | None
    correspondence_id: str | None
    correspondence_sequence: int | None
    structural_role: str


@dataclass
class PartObservation:
    config: dict[str, Any]
    payload_ref: str
    payload_path: Path
    content: str
    units: list[UnitObservation]
    excluded_resources: list[dict[str, Any]]


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_text(value: str) -> str:
    return _sha256_bytes(value.encode("utf-8"))


def _sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _json_bytes(payload: Any) -> bytes:
    return (
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def _jsonl_bytes(rows: list[dict[str, Any]]) -> bytes:
    return "".join(
        json.dumps(row, ensure_ascii=False, separators=(",", ":")) + "\n"
        for row in rows
    ).encode("utf-8")


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise TechnicalMarkupError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise TechnicalMarkupError(f"JSON root is not an object: {path}")
    return payload


def _resolve(root: Path, ref: str) -> Path:
    candidate = (root / ref).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError as exc:
        raise TechnicalMarkupError(f"repo-relative path escapes root: {ref}") from exc
    return candidate


def _local_name(tag: object) -> str:
    if not isinstance(tag, str):
        return ""
    return tag.rsplit("}", 1)[-1]


def _element_paths(root: ET.Element) -> dict[int, str]:
    paths: dict[int, str] = {id(root): _local_name(root.tag)}

    def visit(parent: ET.Element) -> None:
        counts: Counter[str] = Counter()
        parent_path = paths[id(parent)]
        for child in list(parent):
            name = _local_name(child.tag)
            if not name:
                continue
            counts[name] += 1
            paths[id(child)] = f"{parent_path}/{name}[{counts[name]}]"
            visit(child)

    visit(root)
    return paths


def _select_choice_child(element: ET.Element) -> ET.Element | None:
    children = [child for child in list(element) if _local_name(child.tag)]
    for preferred in ("sic", "orig", "abbr"):
        for child in children:
            if _local_name(child.tag) == preferred:
                return child
    return children[0] if children else None


def _inventory_resources(
    inventory: dict[str, Any], *, file_ref: str, file_sha256: str
) -> dict[str, dict[str, Any]]:
    matching = [
        row
        for row in inventory.get("files", [])
        if isinstance(row, dict)
        and row.get("file_id") == file_ref
        and row.get("file_sha256") == file_sha256
    ]
    if len(matching) != 1 or matching[0].get("profile") != "tei_structure_v1":
        raise TechnicalMarkupError("resource inventory does not bind one TEI file")
    resources: dict[str, dict[str, Any]] = {}
    for resource in matching[0].get("resources", []):
        if not isinstance(resource, dict):
            continue
        locator = resource.get("locator", {})
        path = locator.get("tei_path") if isinstance(locator, dict) else None
        if isinstance(path, str):
            if path in resources:
                raise TechnicalMarkupError(f"duplicate inventory TEI path: {path}")
            resources[path] = resource
    return resources


def _correspondence_map(
    repo_root: Path, plan: dict[str, Any]
) -> dict[tuple[str, str], dict[str, Any]]:
    payload = _load_json(_resolve(repo_root, plan["structure_correspondence_ref"]))
    if payload.get("work_ref") != plan["work_ref"]:
        raise TechnicalMarkupError("structure correspondence work identity drifted")
    result: dict[tuple[str, str], dict[str, Any]] = {}
    for row in payload.get("correspondences", []):
        if not isinstance(row, dict):
            continue
        source = row.get("source", {})
        key = (row.get("part_label"), source.get("resource_id"))
        if not all(isinstance(value, str) for value in key):
            raise TechnicalMarkupError("malformed structure correspondence source")
        if key in result:
            raise TechnicalMarkupError(f"duplicate structure correspondence: {key}")
        result[key] = row
    expected = plan["expected_counts"]["major_structure_candidates"]
    if len(result) != expected:
        raise TechnicalMarkupError(
            f"major structure correspondence drift: expected {expected}, got {len(result)}"
        )
    return result


def _payload_path(
    *, repo_root: Path, manifest_ref: str, payload_relative_path: str
) -> Path:
    manifest_path = Path(manifest_ref)
    try:
        relative_manifest = manifest_path.relative_to(SOURCE_ROOT)
    except ValueError as exc:
        raise TechnicalMarkupError("manifest escapes source-witness owner") from exc
    return _resolve(
        repo_root,
        (SOURCE_ROOT / relative_manifest.parent / payload_relative_path).as_posix(),
    )


def _clean_xml_text(value: str | None, *, after_lb: bool = False) -> str:
    if not value:
        return ""
    if after_lb and value.startswith("\n"):
        value = value[1:]
    if value and value.isspace() and any(mark in value for mark in "\n\r\t"):
        return ""
    return value


def _observe_part(
    *,
    repo_root: Path,
    plan: dict[str, Any],
    config: dict[str, Any],
    major_map: dict[tuple[str, str], dict[str, Any]],
) -> PartObservation:
    manifest_path = _resolve(repo_root, config["manifest_ref"])
    inventory_path = _resolve(repo_root, config["resource_inventory_ref"])
    rights_path = _resolve(repo_root, config["rights_ref"])
    manifest = _load_json(manifest_path)
    inventory = _load_json(inventory_path)
    rights = _load_json(rights_path)
    if manifest.get("item_id") != config["item_ref"]:
        raise TechnicalMarkupError(f"item identity drift: {manifest_path}")
    if manifest.get("embodiment_ref") != config["edition_ref"]:
        raise TechnicalMarkupError(f"edition binding drift: {manifest_path}")
    entries = [
        row
        for row in manifest.get("payload_files", [])
        if isinstance(row, dict)
        and row.get("file_id") == config["file_ref"]
        and row.get("sha256") == config["file_sha256"]
    ]
    if len(entries) != 1 or entries[0].get("media_type") != "application/xml":
        raise TechnicalMarkupError(f"exact DTA TEI payload is absent: {manifest_path}")
    payload_path = _payload_path(
        repo_root=repo_root,
        manifest_ref=config["manifest_ref"],
        payload_relative_path=entries[0]["relative_path"],
    )
    if not payload_path.is_file():
        raise TechnicalMarkupError(f"local DTA payload is absent: {payload_path}")
    if payload_path.stat().st_size != entries[0].get("byte_size"):
        raise TechnicalMarkupError(f"source byte-size drift: {payload_path}")
    if _sha256_path(payload_path) != config["file_sha256"]:
        raise TechnicalMarkupError(f"source fixity drift: {payload_path}")
    if config["item_ref"] not in rights.get("scope_refs", []):
        raise TechnicalMarkupError(f"rights scope omits item: {rights_path}")
    resources = _inventory_resources(
        inventory,
        file_ref=config["file_ref"],
        file_sha256=config["file_sha256"],
    )
    resource_by_id = {
        row.get("resource_id"): (path, row)
        for path, row in resources.items()
        if isinstance(row.get("resource_id"), str)
    }
    excluded_paths: set[str] = set()
    excluded_resources: list[dict[str, Any]] = []
    for resource_id in config.get("excluded_resource_ids", []):
        resolved = resource_by_id.get(resource_id)
        if resolved is None:
            raise TechnicalMarkupError(
                f"excluded source resource is unresolved: {resource_id}"
            )
        path, resource = resolved
        excluded_paths.add(path)
        excluded_resources.append(
            {
                "resource_id": resource_id,
                "resource_kind": resource.get("resource_kind"),
                "tei_path": path,
            }
        )

    try:
        root = ET.parse(payload_path).getroot()
    except (OSError, ET.ParseError) as exc:
        raise TechnicalMarkupError(f"cannot parse exact DTA TEI: {exc}") from exc
    if root.tag != f"{{{TEI_NS}}}TEI":
        raise TechnicalMarkupError(f"unexpected TEI namespace: {payload_path}")
    body = root.find(f".//{{{TEI_NS}}}body")
    if body is None:
        raise TechnicalMarkupError(f"TEI body is absent: {payload_path}")
    paths = _element_paths(root)
    body_path = paths[id(body)]
    if body_path != "TEI/text[1]/body[1]":
        raise TechnicalMarkupError(f"body selector drifted: {body_path}")
    for excluded_path in excluded_paths:
        if excluded_path not in set(paths.values()):
            raise TechnicalMarkupError(
                f"excluded TEI locator is absent from source: {excluded_path}"
            )

    chunks: list[str] = []
    length = 0
    observations: list[UnitObservation] = []

    def append(value: str) -> None:
        nonlocal length
        if value:
            chunks.append(value)
            length += len(value)

    def visit(element: ET.Element, parent_locator: str | None) -> None:
        nonlocal length
        tag = _local_name(element.tag)
        locator = paths[id(element)]
        if locator in excluded_paths or tag in INLINE_EXCLUDED:
            return
        if tag == "choice":
            selected = _select_choice_child(element)
            if selected is not None:
                visit(selected, parent_locator)
            return
        start = length
        addressable = tag in ADDRESSABLE_KINDS
        observation: UnitObservation | None = None
        if addressable:
            resource = resources.get(locator)
            resource_id = resource.get("resource_id") if resource else None
            correspondence = major_map.get((config["part_label"], resource_id))
            structural_role = "source_unit"
            if tag == "body":
                structural_role = "part_scope"
            elif correspondence is not None:
                if correspondence["correspondence_id"] == "structure-i-002":
                    structural_role = "major_wrapper_candidate"
                else:
                    structural_role = "major_reading_unit_candidate"
            elif tag == "div":
                structural_role = "nested_section"
            elif tag in {"cit", "list"}:
                structural_role = "paratext_container"
            observation = UnitObservation(
                part_order=config["part_order"],
                part_label=config["part_label"],
                locator=locator,
                source_tag=tag,
                unit_kind=ADDRESSABLE_KINDS[tag],
                parent_locator=parent_locator,
                child_locators=[],
                start=start,
                end=start,
                resource_id=resource_id,
                correspondence_id=(
                    correspondence["correspondence_id"] if correspondence else None
                ),
                correspondence_sequence=(
                    correspondence["sequence"] if correspondence else None
                ),
                structural_role=structural_role,
            )
            observations.append(observation)
            if parent_locator is not None:
                parent = next(
                    (row for row in reversed(observations) if row.locator == parent_locator),
                    None,
                )
                if parent is None:
                    raise TechnicalMarkupError(
                        f"addressable parent was not observed: {parent_locator}"
                    )
                parent.child_locators.append(locator)
            parent_locator = locator

        append(_clean_xml_text(element.text))
        for child in list(element):
            child_tag = _local_name(child.tag)
            child_path = paths[id(child)]
            if child_path in excluded_paths:
                append(_clean_xml_text(child.tail))
                continue
            if child_tag == "lb":
                append("\n")
                append(_clean_xml_text(child.tail, after_lb=True))
                continue
            visit(child, parent_locator)
            append(_clean_xml_text(child.tail))
        if observation is not None:
            observation.end = length
            if observation.unit_kind != "milestone" and observation.start == observation.end:
                raise TechnicalMarkupError(
                    f"non-milestone unit has an empty source span: {locator}"
                )

    visit(body, None)
    content = "".join(chunks)
    if not content or len(content) != length:
        raise TechnicalMarkupError(f"empty or inconsistent text layer: {payload_path}")
    if observations[0].locator != body_path or observations[0].end != len(content):
        raise TechnicalMarkupError("part document unit does not cover the exact layer")
    locators = [row.locator for row in observations]
    if len(locators) != len(set(locators)):
        raise TechnicalMarkupError("technical unit locator collision")
    return PartObservation(
        config=config,
        payload_ref=(manifest_path.parent / entries[0]["relative_path"]).relative_to(
            repo_root
        ).as_posix(),
        payload_path=payload_path,
        content=content,
        units=observations,
        excluded_resources=sorted(
            excluded_resources, key=lambda row: row["resource_id"]
        ),
    )


def _mint(kind: str, issued_sequence: int) -> str:
    digest = _sha256_text(f"{ID_NAMESPACE}:issued:{issued_sequence}")[:32]
    if kind == "anchor":
        return f"tos.anchor.zarathustra-technical-v1.sid-{digest}"
    return f"tos.{kind}.sid-{digest}"


def _issue_identities(
    *, plan: dict[str, Any], parts: list[PartObservation]
) -> dict[str, Any]:
    issued = 0

    def next_id(kind: str) -> tuple[int, str]:
        nonlocal issued
        issued += 1
        return issued, _mint(kind, issued)

    part_rows: list[dict[str, Any]] = []
    unit_rows: list[dict[str, Any]] = []
    for part in parts:
        fixed: dict[str, Any] = {"part_order": part.config["part_order"]}
        for field, kind in (
            ("packet_id", "source-text-unit-packet"),
            ("scheme_id", "text-unit-scheme"),
            ("segmentation_id", "text-segmentation"),
            ("projection_id", "text-unit-projection"),
        ):
            sequence, value = next_id(kind)
            fixed[field] = value
            fixed[f"{field}_issued_sequence"] = sequence
        part_rows.append(fixed)
        for observation in part.units:
            unit_sequence, unit_id = next_id("text-unit")
            anchor_sequence, anchor_ref = next_id("anchor")
            unit_rows.append(
                {
                    "part_order": observation.part_order,
                    "source_locator": observation.locator,
                    "unit_id": unit_id,
                    "unit_id_issued_sequence": unit_sequence,
                    "anchor_ref": anchor_ref,
                    "anchor_ref_issued_sequence": anchor_sequence,
                }
            )
    return {
        "schema_version": "tos_zarathustra_technical_identity_issuance_v1",
        "issuance_id": "tos.identity-issuance.zarathustra-dta-technical-markup-v1",
        "created_at": plan["created_at"],
        "identity_policy": plan["identity_policy"]["policy"],
        "namespace_seed_sha256": _sha256_text(ID_NAMESPACE),
        "ids_minted_from": "issuance-namespace-and-issued-sequence-only",
        "source_locator_role": "binding-only-not-identity-input",
        "automatic_remint_on_locator_drift": False,
        "part_identities": part_rows,
        "unit_identities": unit_rows,
        "issued_identity_count": issued,
        "authority_boundary": (
            "opaque identity issuance for technical source units only; no textual, "
            "linguistic, translation, semantic, graph, canon, or publication authority"
        ),
    }


def _identity_indexes(
    issuance: dict[str, Any], parts: list[PartObservation]
) -> tuple[dict[int, dict[str, Any]], dict[tuple[int, str], dict[str, Any]]]:
    part_index: dict[int, dict[str, Any]] = {}
    for row in issuance.get("part_identities", []):
        if not isinstance(row, dict) or not isinstance(row.get("part_order"), int):
            raise TechnicalMarkupError("malformed part identity issuance")
        if row["part_order"] in part_index:
            raise TechnicalMarkupError("duplicate part identity issuance")
        part_index[row["part_order"]] = row
    unit_index: dict[tuple[int, str], dict[str, Any]] = {}
    all_ids: set[str] = set()
    for row in issuance.get("unit_identities", []):
        if not isinstance(row, dict):
            raise TechnicalMarkupError("malformed unit identity issuance")
        key = (row.get("part_order"), row.get("source_locator"))
        if not isinstance(key[0], int) or not isinstance(key[1], str):
            raise TechnicalMarkupError("unit identity binding is incomplete")
        if key in unit_index:
            raise TechnicalMarkupError(f"duplicate unit identity binding: {key}")
        for field in ("unit_id", "anchor_ref"):
            value = row.get(field)
            if not isinstance(value, str) or value in all_ids:
                raise TechnicalMarkupError(f"invalid or duplicate issued identity: {value}")
            all_ids.add(value)
        unit_index[key] = row
    expected_parts = {part.config["part_order"] for part in parts}
    if set(part_index) != expected_parts:
        raise TechnicalMarkupError("part identity issuance set drifted")
    expected_units = {
        (part.config["part_order"], unit.locator)
        for part in parts
        for unit in part.units
    }
    if set(unit_index) != expected_units:
        missing = sorted(expected_units - set(unit_index))[:5]
        extra = sorted(set(unit_index) - expected_units)[:5]
        raise TechnicalMarkupError(
            f"unit locator set drifted; missing={missing}, extra={extra}; "
            "explicit successor identity work is required"
        )
    return part_index, unit_index


def _citation_records(
    *,
    work_ref: str,
    parts: list[PartObservation],
    unit_index: dict[tuple[int, str], dict[str, Any]],
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for part in parts:
        by_locator = {row.locator: row for row in part.units}
        citation_by_locator: dict[str, str] = {}
        counters: defaultdict[tuple[str | None, str], int] = defaultdict(int)
        major_kind_counters: defaultdict[tuple[str, str], int] = defaultdict(int)

        def nearest_major_locator(observation: UnitObservation) -> str | None:
            cursor: UnitObservation | None = observation
            while cursor is not None:
                if cursor.structural_role == "major_reading_unit_candidate":
                    return cursor.locator
                cursor = (
                    by_locator.get(cursor.parent_locator)
                    if cursor.parent_locator is not None
                    else None
                )
            return None

        def descendant_count(locator: str, unit_kind: str) -> int:
            total = 0
            for child_locator in by_locator[locator].child_locators:
                child = by_locator[child_locator]
                if child.unit_kind == unit_kind:
                    total += 1
                total += descendant_count(child_locator, unit_kind)
            return total

        for observation in part.units:
            if observation.unit_kind == "document":
                citation = f"Za-DE-{part.config['part_label']}"
                ordinal = 1
            else:
                prefix = {
                    "section": "s",
                    "paragraph": "p",
                    "verse_group": "vg",
                    "verse_line": "v",
                    "milestone": "pb" if observation.source_tag == "pb" else "m",
                    "other": {
                        "head": "h",
                        "item": "item",
                        "quote": "q",
                        "bibl": "b",
                    }.get(observation.source_tag, "u"),
                }[observation.unit_kind]
                counter_key = (observation.parent_locator, prefix)
                counters[counter_key] += 1
                ordinal = counters[counter_key]
                parent_citation = citation_by_locator.get(observation.parent_locator or "")
                if parent_citation is None:
                    raise TechnicalMarkupError(
                        f"citation parent is unresolved: {observation.locator}"
                    )
                citation = f"{parent_citation}.{prefix}{ordinal:03d}"
            citation_by_locator[observation.locator] = citation
            identity = unit_index[(observation.part_order, observation.locator)]
            parent_identity = (
                unit_index[(observation.part_order, observation.parent_locator)]["unit_id"]
                if observation.parent_locator is not None
                else None
            )
            major_locator = nearest_major_locator(observation)
            major_identity = (
                unit_index[(observation.part_order, major_locator)]["unit_id"]
                if major_locator is not None
                else None
            )
            major_correspondence = (
                by_locator[major_locator].correspondence_id
                if major_locator is not None
                else None
            )
            major_ordinal: int | None = None
            if major_locator is not None:
                major_counter_key = (major_locator, observation.unit_kind)
                major_kind_counters[major_counter_key] += 1
                major_ordinal = major_kind_counters[major_counter_key]
            direct_children = [by_locator[child] for child in observation.child_locators]
            rows.append(
                {
                    "schema_version": "tos_zarathustra_technical_citation_v1",
                    "work_ref": work_ref,
                    "part_order": observation.part_order,
                    "part_label": observation.part_label,
                    "unit_id": identity["unit_id"],
                    "anchor_ref": identity["anchor_ref"],
                    "parent_unit_id": parent_identity,
                    "unit_kind": observation.unit_kind,
                    "source_tag": observation.source_tag,
                    "structural_role": observation.structural_role,
                    "display_citation": citation,
                    "ordinal_within_parent_kind": ordinal,
                    "nearest_major_unit_id": major_identity,
                    "nearest_major_correspondence_id": major_correspondence,
                    "ordinal_within_nearest_major_kind": major_ordinal,
                    "direct_child_count": len(direct_children),
                    "direct_paragraph_count": sum(
                        child.unit_kind == "paragraph" for child in direct_children
                    ),
                    "descendant_paragraph_count": descendant_count(
                        observation.locator, "paragraph"
                    ),
                    "direct_verse_line_count": sum(
                        child.unit_kind == "verse_line" for child in direct_children
                    ),
                    "descendant_verse_line_count": descendant_count(
                        observation.locator, "verse_line"
                    ),
                    "source_locator": observation.locator,
                    "source_resource_id": observation.resource_id,
                    "major_correspondence_id": observation.correspondence_id,
                    "major_correspondence_sequence": observation.correspondence_sequence,
                    "boundary_posture": "source_attested",
                    "segmentation_status": "observed_source_structure",
                    "source_text_included": False,
                    "semantic_promotion": False,
                }
            )
        if set(citation_by_locator) != set(by_locator):
            raise TechnicalMarkupError("citation spine does not cover every observed unit")
    citations = [row["display_citation"] for row in rows]
    if len(citations) != len(set(citations)):
        raise TechnicalMarkupError("display citation collision")
    return rows


def _method(plan: dict[str, Any]) -> dict[str, Any]:
    return {
        "maker_kind": "software",
        "agent_ref": "software:tos-zarathustra-technical-markup-builder",
        "method_name": "exact DTA TEI source-structure observation",
        "method_version": "1",
        "software_refs": [BUILDER_REF],
        "model_ref": None,
        "configuration_ref": PLAN_REF,
        "locale": "de",
        "unicode_version": None,
        "unicode_revision": None,
        "tailoring_ref": None,
        "provenance_event_ref": FIXED_EVENT_ID,
        "made_at": plan["created_at"],
        "output_posture": "method_result_not_source_or_linguistic_truth",
    }


def _build_packet(
    *,
    repo_root: Path,
    plan: dict[str, Any],
    part: PartObservation,
    part_ids: dict[str, Any],
    unit_index: dict[tuple[int, str], dict[str, Any]],
    citation_ref: str,
    citation_digest: str,
) -> dict[str, Any]:
    config = part.config
    content_digest = _sha256_text(part.content)
    private_ref = plan["outputs"]["private_layer_pattern"].format(
        part_order=config["part_order"]
    )
    identity_by_locator = {
        unit.locator: unit_index[(unit.part_order, unit.locator)]
        for unit in part.units
    }
    anchors: list[dict[str, Any]] = []
    units: list[dict[str, Any]] = []
    for ordinal, observation in enumerate(part.units, start=1):
        identity = identity_by_locator[observation.locator]
        exact = part.content[observation.start : observation.end]
        role = "scope" if observation.unit_kind == "document" else (
            "milestone" if observation.unit_kind == "milestone" else "content"
        )
        anchors.append(
            {
                "anchor_ref": identity["anchor_ref"],
                "ordinal": ordinal,
                "text_layer_ref": private_ref,
                "text_layer_sha256": content_digest,
                "selector": {
                    "type": "text_position",
                    "start": observation.start,
                    "end": observation.end,
                    "position_unit": "unicode_code_point",
                    "interval": "half_open",
                },
                "exact_sha256": _sha256_text(exact),
                "anchor_role": role,
                "source_return": {
                    "required": True,
                    "locator_ref": f"{part.payload_ref}#{observation.locator}",
                },
            }
        )
        units.append(
            {
                "unit_id": identity["unit_id"],
                "unit_version": 1,
                "supersedes_unit_ref": None,
                "identity_policy": (
                    "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis"
                ),
                "unit_kind": observation.unit_kind,
                "surface_posture": "source_bearing",
                "continuity": "contiguous",
                "ordered_anchor_refs": [identity["anchor_ref"]],
                "parent_unit_refs": (
                    [identity_by_locator[observation.parent_locator]["unit_id"]]
                    if observation.parent_locator is not None
                    else []
                ),
                "ordered_child_unit_refs": [
                    identity_by_locator[child]["unit_id"]
                    for child in observation.child_locators
                ],
                "boundary_posture": "source_attested",
                "certainty": {
                    "value": 1.0,
                    "meaning": (
                        "maker-declared-boundary-confidence-not-truth-probability"
                    ),
                },
                "status_reason": (
                    "The unit records only a boundary explicitly present in the exact "
                    "selected DTA TEI representation; this is not accepted German, "
                    "editorial hierarchy, linguistic analysis, translation, or semantics."
                ),
                "source_text_mutated": False,
                "semantic_promotion": False,
            }
        )
    method = _method(plan)
    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "source-text-unit-packet-v1.schema.json"
        ),
        "schema_version": "tos_source_text_unit_packet_v1",
        "packet_id": part_ids["packet_id"],
        "packet_version": 1,
        "supersedes_packet_ref": None,
        "content_posture": "source_bound",
        "source_scope": {
            "work_ref": plan["work_ref"],
            "expression_ref": config["expression_ref"],
            "edition_ref": config["edition_ref"],
            "item_ref": config["item_ref"],
            "file_ref": config["file_ref"],
            "file_sha256": config["file_sha256"],
        },
        "source_layer": {
            "text_layer_ref": private_ref,
            "text_layer_sha256": content_digest,
            "language": "de",
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
                "scheme_id": part_ids["scheme_id"],
                "scheme_version": 1,
                "supersedes_scheme_ref": None,
                "identity_policy": (
                    "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis"
                ),
                "scheme_name": "DTA TEI non-semantic technical source structure v1",
                "analysis_role": "source_structure",
                "boundary_basis": "source_markup",
                "unit_kinds": [
                    "document",
                    "section",
                    "paragraph",
                    "verse_group",
                    "verse_line",
                    "milestone",
                    "other",
                ],
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
                "segmentation_id": part_ids["segmentation_id"],
                "segmentation_version": 1,
                "supersedes_segmentation_ref": None,
                "identity_policy": (
                    "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries"
                ),
                "scheme_ref": part_ids["scheme_id"],
                "ordered_unit_refs": [row["unit_id"] for row in units],
                "coverage": {
                    "scope_anchor_ref": anchors[0]["anchor_ref"],
                    "coverage_posture": "exhaustive_nested",
                    "excluded_anchor_refs": [],
                    "unreported_gaps_allowed": False,
                    "overlap_requires_declaration": True,
                    "source_reconstruction_required": True,
                },
                "status": "observed_source_structure",
                "status_reason": (
                    "The hierarchy reproduces only selected TEI body element boundaries; "
                    "the separately declared auxiliary Part-IV container subtree is outside "
                    "the work-level source layer."
                ),
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
                "projection_id": part_ids["projection_id"],
                "projection_kind": "other",
                "source_segmentation_refs": [part_ids["segmentation_id"]],
                "artifact_ref": citation_ref,
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
            "rights_record_refs": [config["rights_ref"]],
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


def _validate_packet(
    *, schema: dict[str, Any], packet: dict[str, Any], text: str | None
) -> None:
    errors = sorted(
        Draft202012Validator(schema, format_checker=FormatChecker()).iter_errors(packet),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        rendered = "; ".join(
            f"{'/'.join(map(str, error.absolute_path)) or '<root>'}: {error.message}"
            for error in errors[:12]
        )
        raise TechnicalMarkupError(f"source-text-unit schema failure: {rendered}")
    units = {row["unit_id"]: row for row in packet["units"]}
    anchors = {row["anchor_ref"]: row for row in packet["anchors"]}
    if len(units) != len(packet["units"]) or len(anchors) != len(packet["anchors"]):
        raise TechnicalMarkupError("duplicate packet unit or anchor identity")
    if len(units) != len(anchors):
        raise TechnicalMarkupError("technical packet requires one anchor per unit")
    for anchor in anchors.values():
        start = anchor["selector"]["start"]
        end = anchor["selector"]["end"]
        if not 0 <= start <= end:
            raise TechnicalMarkupError("technical anchor escapes private source layer")
        if text is not None and end > len(text):
            raise TechnicalMarkupError("technical anchor escapes private source layer")
        if text is not None and anchor["exact_sha256"] != _sha256_text(text[start:end]):
            raise TechnicalMarkupError("technical anchor exact digest does not resolve")
    for unit in units.values():
        for parent_ref in unit["parent_unit_refs"]:
            if unit["unit_id"] not in units[parent_ref]["ordered_child_unit_refs"]:
                raise TechnicalMarkupError("parent/child relation is not reciprocal")
        for child_ref in unit["ordered_child_unit_refs"]:
            if unit["unit_id"] not in units[child_ref]["parent_unit_refs"]:
                raise TechnicalMarkupError("child/parent relation is not reciprocal")
    encoded = json.dumps(packet, ensure_ascii=False)
    for forbidden in SEMANTIC_FORBIDDEN_KEYS:
        if f'"{forbidden}"' in encoded:
            raise TechnicalMarkupError(f"semantic field escaped technical packet: {forbidden}")


def _summary(
    *, plan: dict[str, Any], parts: list[PartObservation], citations: list[dict[str, Any]]
) -> dict[str, Any]:
    part_rows: list[dict[str, Any]] = []
    total_kinds: Counter[str] = Counter()
    total_tags: Counter[str] = Counter()
    for part in parts:
        kinds = Counter(row.unit_kind for row in part.units)
        tags = Counter(row.source_tag for row in part.units)
        roles = Counter(row.structural_role for row in part.units)
        total_kinds.update(kinds)
        total_tags.update(tags)
        part_rows.append(
            {
                "part_order": part.config["part_order"],
                "part_label": part.config["part_label"],
                "item_ref": part.config["item_ref"],
                "source_file_sha256": part.config["file_sha256"],
                "private_text_layer_sha256": _sha256_text(part.content),
                "private_text_layer_code_points": len(part.content),
                "unit_count": len(part.units),
                "unit_kind_counts": dict(sorted(kinds.items())),
                "source_tag_counts": dict(sorted(tags.items())),
                "structural_role_counts": dict(sorted(roles.items())),
                "excluded_resources": part.excluded_resources,
                "source_text_included": False,
            }
        )
    result = {
        "schema_version": "tos_zarathustra_dta_technical_markup_summary_v1",
        "work_ref": plan["work_ref"],
        "language": "de",
        "part_count": len(parts),
        "unit_count": sum(len(part.units) for part in parts),
        "citation_count": len(citations),
        "display_citations_unique": len(citations)
        == len({row["display_citation"] for row in citations}),
        "unit_kind_counts": dict(sorted(total_kinds.items())),
        "source_tag_counts": dict(sorted(total_tags.items())),
        "major_structure_candidate_count": sum(
            1 for row in citations if row["major_correspondence_id"] is not None
        ),
        "major_reading_unit_candidate_count": sum(
            1 for row in citations if row["structural_role"] == "major_reading_unit_candidate"
        ),
        "major_wrapper_candidate_count": sum(
            1 for row in citations if row["structural_role"] == "major_wrapper_candidate"
        ),
        "parts": part_rows,
        "source_text_included": False,
        "segmentation_status": "observed_source_structure",
        "human_review_status": "unreviewed",
        "semantic_fields_materialized": False,
        "russian_parallel_structure_materialized": False,
        "authority_boundary": plan["authority_boundary"],
    }
    expected = plan["expected_counts"]
    actual = {
        "parts": result["part_count"],
        "major_structure_candidates": result["major_structure_candidate_count"],
        "reading_units_excluding_wrapper": result[
            "major_reading_unit_candidate_count"
        ],
        "divisions": total_tags["div"],
        "paragraphs": total_kinds["paragraph"],
        "verse_groups": total_kinds["verse_group"],
        "verse_lines": total_kinds["verse_line"],
    }
    for key, value in actual.items():
        if expected[key] != value:
            raise TechnicalMarkupError(
                f"technical count drift for {key}: expected {expected[key]}, got {value}"
            )
    for row in part_rows:
        part_expected = expected["part_counts"][row["part_label"]]
        part_actual = {
            "major_structure_candidates": row["structural_role_counts"].get(
                "major_reading_unit_candidate", 0
            )
            + row["structural_role_counts"].get("major_wrapper_candidate", 0),
            "divisions": row["source_tag_counts"].get("div", 0),
            "paragraphs": row["unit_kind_counts"].get("paragraph", 0),
            "verse_groups": row["unit_kind_counts"].get("verse_group", 0),
            "verse_lines": row["unit_kind_counts"].get("verse_line", 0),
        }
        if part_actual != part_expected:
            raise TechnicalMarkupError(
                f"part {row['part_label']} count drift: expected {part_expected}, "
                f"got {part_actual}"
            )
    return result


def _provenance(
    *,
    repo_root: Path,
    plan: dict[str, Any],
    parts: list[PartObservation],
    tracked_outputs: dict[str, bytes],
    issuance_digest: str,
) -> dict[str, Any]:
    inputs = [
        {"ref": PLAN_REF, "role": "tracked-technical-markup-plan", "sha256": _sha256_path(_resolve(repo_root, PLAN_REF))},
        {
            "ref": plan["identity_policy"]["issuance_ref"],
            "role": "tracked-opaque-identity-issuance",
            "sha256": issuance_digest,
        },
        {
            "ref": plan["structure_correspondence_ref"],
            "role": "tracked-major-structure-candidate-map",
            "sha256": _sha256_path(
                _resolve(repo_root, plan["structure_correspondence_ref"])
            ),
        },
    ]
    for part in parts:
        inputs.extend(
            [
                {
                    "ref": part.config["file_ref"],
                    "role": f"local-fixity-bound-dta-part-{part.config['part_order']}-tei",
                    "sha256": part.config["file_sha256"],
                },
                {
                    "ref": part.config["resource_inventory_ref"],
                    "role": f"tracked-text-free-part-{part.config['part_order']}-resource-inventory",
                    "sha256": _sha256_path(
                        _resolve(repo_root, part.config["resource_inventory_ref"])
                    ),
                },
                {
                    "ref": part.config["rights_ref"],
                    "role": f"tracked-part-{part.config['part_order']}-rights-record",
                    "sha256": _sha256_path(
                        _resolve(repo_root, part.config["rights_ref"])
                    ),
                },
            ]
        )
    outputs = [
        {"ref": ref, "role": "tracked-text-free-technical-markup-artifact", "sha256": _sha256_bytes(payload)}
        for ref, payload in sorted(tracked_outputs.items())
    ]
    event = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": FIXED_EVENT_ID,
        "event_type": "segmentation",
        "started_at": plan["created_at"],
        "ended_at": plan["created_at"],
        "agent_refs": [
            "software:tos-zarathustra-technical-markup-builder",
            "model:codex",
        ],
        "inputs": inputs,
        "outputs": outputs,
        "method": {
            "maker_type": "software",
            "name": "exact-dta-tei-non-semantic-technical-structure",
            "version": "1",
            "artifact_digest": _sha256_path(_resolve(repo_root, BUILDER_REF)),
            "runtime": None,
            "device": "cpu",
            "configuration": {
                "parts": len(parts),
                "source_text_tracked": False,
                "opaque_identity_issuance_reused": True,
                "source_markup_status": "observed_source_structure",
                "human_review_status": "unreviewed",
                "semantic_fields_materialized": False,
                "russian_parallel_structure_materialized": False,
            },
            "prompt_or_instruction_ref": PLAN_REF,
        },
        "status": "completed_with_warnings",
        "warnings": [
            "TEI source markup is observed but not accepted as a final editorial hierarchy.",
            "The sequential German layers remain ignored local-only mode-0600 files.",
            "The Part-IV auxiliary sequence is excluded only from this work-level projection and remains intact in its source container.",
            "No Russian structure, translation alignment, linguistic analysis, semantics, graph, canon, rights clearance, or publication route is created.",
        ],
        "receipt_refs": sorted(tracked_outputs),
        "rights_basis_ref": parts[0].config["rights_ref"],
        "event_version": 1,
        "supersedes_event_ref": None,
    }
    schema = _load_json(_resolve(repo_root, PROVENANCE_SCHEMA_REF))
    errors = list(
        Draft202012Validator(schema, format_checker=FormatChecker()).iter_errors(event)
    )
    if errors:
        raise TechnicalMarkupError(
            "provenance schema failure: " + "; ".join(error.message for error in errors[:8])
        )
    return event


def build_expected(
    repo_root: Path, *, issue_identities: bool = False
) -> tuple[dict[str, bytes], dict[str, bytes], dict[str, Any] | None]:
    plan = _load_json(_resolve(repo_root, PLAN_REF))
    if plan.get("contract_ref") != SCHEMA_REF:
        raise TechnicalMarkupError("technical plan contract binding drifted")
    major_map = _correspondence_map(repo_root, plan)
    parts = [
        _observe_part(
            repo_root=repo_root,
            plan=plan,
            config=config,
            major_map=major_map,
        )
        for config in sorted(plan["source_items"], key=lambda row: row["part_order"])
    ]
    issuance_ref = plan["identity_policy"]["issuance_ref"]
    issuance_path = _resolve(repo_root, issuance_ref)
    new_issuance: dict[str, Any] | None = None
    if issue_identities:
        if issuance_path.exists():
            raise TechnicalMarkupError(
                "identity issuance already exists; automatic remint is forbidden"
            )
        new_issuance = _issue_identities(plan=plan, parts=parts)
        issuance = new_issuance
    else:
        issuance = _load_json(issuance_path)
        if issuance_path.read_bytes() != _json_bytes(issuance):
            raise TechnicalMarkupError("identity issuance is not canonical JSON")
    part_index, unit_index = _identity_indexes(issuance, parts)
    citations = _citation_records(
        work_ref=plan["work_ref"], parts=parts, unit_index=unit_index
    )
    citation_bytes = _jsonl_bytes(citations)
    citation_ref = plan["outputs"]["citation_spine_ref"]
    citation_digest = _sha256_bytes(citation_bytes)
    schema = _load_json(_resolve(repo_root, SCHEMA_REF))
    tracked: dict[str, bytes] = {citation_ref: citation_bytes}
    local: dict[str, bytes] = {}
    for part in parts:
        packet = _build_packet(
            repo_root=repo_root,
            plan=plan,
            part=part,
            part_ids=part_index[part.config["part_order"]],
            unit_index=unit_index,
            citation_ref=citation_ref,
            citation_digest=citation_digest,
        )
        _validate_packet(schema=schema, packet=packet, text=part.content)
        packet_ref = plan["outputs"]["part_packet_pattern"].format(
            part_order=part.config["part_order"]
        )
        private_ref = plan["outputs"]["private_layer_pattern"].format(
            part_order=part.config["part_order"]
        )
        tracked[packet_ref] = _json_bytes(packet)
        local[private_ref] = part.content.encode("utf-8")
    summary = _summary(plan=plan, parts=parts, citations=citations)
    tracked[plan["outputs"]["summary_ref"]] = _json_bytes(summary)
    event = _provenance(
        repo_root=repo_root,
        plan=plan,
        parts=parts,
        tracked_outputs=tracked,
        issuance_digest=_sha256_bytes(_json_bytes(issuance)),
    )
    tracked[plan["outputs"]["provenance_ref"]] = _jsonl_bytes([event])
    if any(any(key in row for key in SEMANTIC_FORBIDDEN_KEYS) for row in citations):
        raise TechnicalMarkupError("semantic field escaped citation spine")
    return tracked, local, new_issuance


def validate_tracked(repo_root: Path) -> dict[str, Any]:
    """Validate the tracked text-free route without local TEI or private layers."""

    plan = _load_json(_resolve(repo_root, PLAN_REF))
    issuance = _load_json(
        _resolve(repo_root, plan["identity_policy"]["issuance_ref"])
    )
    citation_path = _resolve(repo_root, plan["outputs"]["citation_spine_ref"])
    try:
        citations = [
            json.loads(line)
            for line in citation_path.read_text(encoding="utf-8").splitlines()
            if line
        ]
    except (OSError, json.JSONDecodeError) as exc:
        raise TechnicalMarkupError(f"cannot read citation spine: {exc}") from exc
    if not citations or not all(isinstance(row, dict) for row in citations):
        raise TechnicalMarkupError("tracked citation spine is empty or malformed")
    if len(citations) != len({row.get("unit_id") for row in citations}):
        raise TechnicalMarkupError("tracked citation unit identity collision")
    if len(citations) != len({row.get("display_citation") for row in citations}):
        raise TechnicalMarkupError("tracked display citation collision")
    if any(
        row.get("source_text_included") is not False
        or row.get("semantic_promotion") is not False
        for row in citations
    ):
        raise TechnicalMarkupError("tracked citation spine widens the technical boundary")
    issuance_rows = [
        row for row in issuance.get("unit_identities", []) if isinstance(row, dict)
    ]
    citation_bindings = {
        (row["part_order"], row["source_locator"]): (
            row["unit_id"],
            row["anchor_ref"],
        )
        for row in citations
    }
    issuance_bindings = {
        (row.get("part_order"), row.get("source_locator")): (
            row.get("unit_id"),
            row.get("anchor_ref"),
        )
        for row in issuance_rows
    }
    if citation_bindings != issuance_bindings:
        raise TechnicalMarkupError("citation spine and identity issuance drifted")
    part_ids = {
        row["part_order"]: row
        for row in issuance.get("part_identities", [])
        if isinstance(row, dict) and isinstance(row.get("part_order"), int)
    }
    if set(part_ids) != {1, 2, 3, 4}:
        raise TechnicalMarkupError("tracked part identity issuance drifted")
    citation_digest = _sha256_path(citation_path)
    schema = _load_json(_resolve(repo_root, SCHEMA_REF))
    for source in plan["source_items"]:
        part_order = source["part_order"]
        packet_ref = plan["outputs"]["part_packet_pattern"].format(
            part_order=part_order
        )
        packet = _load_json(_resolve(repo_root, packet_ref))
        _validate_packet(schema=schema, packet=packet, text=None)
        fixed = part_ids[part_order]
        if packet["packet_id"] != fixed["packet_id"]:
            raise TechnicalMarkupError(f"part {part_order} packet identity drifted")
        if packet["schemes"][0]["scheme_id"] != fixed["scheme_id"]:
            raise TechnicalMarkupError(f"part {part_order} scheme identity drifted")
        if packet["segmentations"][0]["segmentation_id"] != fixed["segmentation_id"]:
            raise TechnicalMarkupError(
                f"part {part_order} segmentation identity drifted"
            )
        projection = packet["projections"][0]
        if (
            projection["projection_id"] != fixed["projection_id"]
            or projection["artifact_ref"] != plan["outputs"]["citation_spine_ref"]
            or projection["artifact_sha256"] != citation_digest
        ):
            raise TechnicalMarkupError(f"part {part_order} citation projection drifted")
        citation_part = [
            row for row in citations if row["part_order"] == part_order
        ]
        if {row["unit_id"] for row in packet["units"]} != {
            row["unit_id"] for row in citation_part
        }:
            raise TechnicalMarkupError(f"part {part_order} citation unit closure drifted")
        if {row["anchor_ref"] for row in packet["anchors"]} != {
            row["anchor_ref"] for row in citation_part
        }:
            raise TechnicalMarkupError(
                f"part {part_order} citation anchor closure drifted"
            )
    summary = _load_json(_resolve(repo_root, plan["outputs"]["summary_ref"]))
    if summary.get("unit_count") != len(citations):
        raise TechnicalMarkupError("tracked summary unit count drifted")
    expected = plan["expected_counts"]
    observed = {
        "major_structure_candidates": sum(
            row.get("major_correspondence_id") is not None for row in citations
        ),
        "reading_units_excluding_wrapper": sum(
            row.get("structural_role") == "major_reading_unit_candidate"
            for row in citations
        ),
        "divisions": sum(row.get("source_tag") == "div" for row in citations),
        "paragraphs": sum(row.get("unit_kind") == "paragraph" for row in citations),
        "verse_groups": sum(row.get("unit_kind") == "verse_group" for row in citations),
        "verse_lines": sum(row.get("unit_kind") == "verse_line" for row in citations),
    }
    for key, value in observed.items():
        if expected[key] != value:
            raise TechnicalMarkupError(
                f"tracked technical count drift for {key}: expected {expected[key]}, got {value}"
            )
    part_iv_resources = {
        row.get("source_resource_id")
        for row in citations
        if row.get("part_label") == "IV"
    }
    if part_iv_resources.intersection({"tei-pb-0143", "tei-pb-0144", "tei-div-0070"}):
        raise TechnicalMarkupError("Part-IV auxiliary exclusion escaped tracked route")
    provenance_path = _resolve(repo_root, plan["outputs"]["provenance_ref"])
    try:
        provenance_rows = [
            json.loads(line)
            for line in provenance_path.read_text(encoding="utf-8").splitlines()
            if line
        ]
    except (OSError, json.JSONDecodeError) as exc:
        raise TechnicalMarkupError(f"cannot read technical provenance: {exc}") from exc
    if len(provenance_rows) != 1:
        raise TechnicalMarkupError("technical provenance must contain one event")
    provenance_schema = _load_json(_resolve(repo_root, PROVENANCE_SCHEMA_REF))
    provenance_errors = list(
        Draft202012Validator(
            provenance_schema, format_checker=FormatChecker()
        ).iter_errors(provenance_rows[0])
    )
    if provenance_errors:
        raise TechnicalMarkupError(
            "tracked provenance schema failure: "
            + "; ".join(error.message for error in provenance_errors[:8])
        )
    return summary


def _write_exact(path: Path, payload: bytes, *, mode: int | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    if mode is not None:
        path.chmod(mode)


def _check_exact(path: Path, payload: bytes) -> str | None:
    if not path.is_file():
        return f"missing output: {path}"
    actual = path.read_bytes()
    if actual != payload:
        return (
            f"output drift: {path} expected_sha256={_sha256_bytes(payload)} "
            f"actual_sha256={_sha256_bytes(actual)}"
        )
    return None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--build", action="store_true")
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--validate-tracked", action="store_true")
    parser.add_argument(
        "--issue-identities",
        action="store_true",
        help="one-time initial issuance; fails if the tracked issuance already exists",
    )
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    args = parser.parse_args(argv)
    if args.issue_identities and not args.build:
        parser.error("--issue-identities requires --build")
    repo_root = args.repo_root.resolve()
    try:
        if args.validate_tracked:
            summary = validate_tracked(repo_root)
            print(
                "validated tracked Zarathustra technical markup: "
                f"parts={summary['part_count']} units={summary['unit_count']}"
            )
            return 0
        tracked, local, issuance = build_expected(
            repo_root, issue_identities=args.issue_identities
        )
        if args.build:
            if issuance is not None:
                plan = _load_json(_resolve(repo_root, PLAN_REF))
                _write_exact(
                    _resolve(repo_root, plan["identity_policy"]["issuance_ref"]),
                    _json_bytes(issuance),
                )
                # Rebuild after the issuance is on disk so provenance binds its exact bytes.
                tracked, local, _ = build_expected(repo_root)
            for ref, payload in tracked.items():
                _write_exact(_resolve(repo_root, ref), payload)
            for ref, payload in local.items():
                _write_exact(_resolve(repo_root, ref), payload, mode=0o600)
            print(
                "built Zarathustra DTA technical markup: "
                f"tracked={len(tracked)} private_layers={len(local)}"
            )
            return 0
        issues = [
            message
            for ref, payload in {**tracked, **local}.items()
            if (message := _check_exact(_resolve(repo_root, ref), payload)) is not None
        ]
        for ref in local:
            path = _resolve(repo_root, ref)
            if path.is_file() and (path.stat().st_mode & 0o077):
                issues.append(f"private layer permissions are wider than mode-0600: {path}")
        if issues:
            raise TechnicalMarkupError("\n".join(issues))
        summary = _load_json(
            _resolve(repo_root, _load_json(_resolve(repo_root, PLAN_REF))["outputs"]["summary_ref"])
        )
        print(
            "validated Zarathustra DTA technical markup: "
            f"parts={summary['part_count']} units={summary['unit_count']} "
            f"paragraphs={summary['unit_kind_counts']['paragraph']} "
            f"verse_lines={summary['unit_kind_counts']['verse_line']}"
        )
        return 0
    except TechnicalMarkupError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
