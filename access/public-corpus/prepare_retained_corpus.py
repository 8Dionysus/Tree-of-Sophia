#!/usr/bin/env python3
"""Prepare the retained 2026-10-25 Antiquity reading representations.

The old export contains exact retained JSON, not its usually absent raw TEI
payloads. This script keeps those identities separate and emits a bounded,
source-bound reading layer without making transcription or semantic claims.
Run with --plan before --apply. Applying requires the maintained
`tos-native-owner-command` for post-apply file profiles; `--refresh-metadata`
repeats only that native inventory composition and provenance update. The
expected source worktree and target HEAD are deliberately pinned for this migration.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import uuid
from functools import lru_cache
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
import xml.etree.ElementTree as ET

REPO = Path(__file__).resolve().parents[2]
PINNED_HEAD = "e6d4051a05116749bf73f16b027fa8e92d20f3b3"
LEGACY_REPO = Path("/srv/abyss-machine/storage/source-worktrees/tos-research-transfer-20261025")
LEGACY = LEGACY_REPO / "access/web/antiquity/books"
LEGACY_COMMIT = "99e81af653c82765b587c870db3c0f605d794fae"
MAX_INPUT_BYTES = 60_000_000
MAX_OUTPUT_BYTES = 805_306_368
MAX_UNIT_BYTES = 60_000
INVENTORY_ESTIMATE_BYTES = 16_384
MADE_AT = ""
LICENSE = "https://creativecommons.org/licenses/by-sa/4.0/"
CANONICAL_WORKS = {
    "meno": "tos.work.plato.meno",
    "apology": "tos.work.plato.apology",
    "ethics": "tos.work.aristotle.nicomachean-ethics",
}
IDENTITIES: dict[str, str] = {}
MINT_IDENTITIES = False


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def opaque(kind: str, key: str) -> str:
    """Use a minted opaque ID whose persistent assignment lives in a migration map."""
    assignment_key = f"{kind}:{key}"
    value = IDENTITIES.get(assignment_key)
    if value is None:
        value = uuid.uuid4().hex if MINT_IDENTITIES else sha(assignment_key.encode("utf-8"))[:32]
        IDENTITIES[assignment_key] = value
    return f"tos.{kind}.sid-{value}"


def slug(value: str) -> str:
    result = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    if not result or not result[0].isalnum():
        raise ValueError(f"not a usable stable id component: {value!r}")
    return result


def json_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def load(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def text_sha(text: str) -> str:
    return sha(text.encode("utf-8"))


def split_utf8(text: str, limit: int = MAX_UNIT_BYTES) -> list[str]:
    """Split at code-point boundaries, preferring a nearby whitespace seam."""
    chunks: list[str] = []
    start = 0
    while start < len(text):
        end = start
        used = 0
        while end < len(text):
            size = len(text[end].encode("utf-8"))
            if used + size > limit:
                break
            used += size
            end += 1
        if end == start:
            raise ValueError("one code point exceeds the bounded source-unit size")
        if end < len(text):
            seam = max(text.rfind("\n", start, end), text.rfind(" ", start, end))
            if seam > start + limit // 2:
                end = seam + 1
        chunks.append(text[start:end])
        start = end
    return chunks or [""]


def values_from_header(xml_text: str) -> tuple[list[str], list[str], list[str]]:
    try:
        root = ET.fromstring(xml_text)
    except ET.ParseError:
        return ([], [], [])
    def vals(tags: set[str]) -> list[str]:
        found: list[str] = []
        for el in root.iter():
            if el.tag.rsplit("}", 1)[-1].lower() in tags:
                value = " ".join("".join(el.itertext()).split())
                if value and value not in found:
                    found.append(value)
        return found
    return vals({"title"}), vals({"author"}), vals({"editor", "translator"})


def version_label(book: dict[str, Any], version: dict[str, Any], data: dict[str, Any]) -> str:
    pub = version.get("publication") or {}
    titles = pub.get("titles") or []
    dates = pub.get("dates") or []
    head_titles, _, head_credits = values_from_header(data.get("header", ""))
    title = (titles[0] if titles else (head_titles[0] if head_titles else book["title"]))
    credit = head_credits[0] if head_credits else version.get("credit") or ""
    credit = re.sub(r"^(?:translation|translator|edition|editor|издание|перевод)\s*:\s*", "", credit, flags=re.I)
    parts = [str(title)]
    if dates:
        parts.append(str(dates[0]))
    if credit and credit.casefold() not in {str(part).casefold() for part in parts}:
        parts.append(credit)
    return " · ".join(part for part in parts if part)


def source_display_text(data: dict[str, Any]) -> tuple[str, list[dict[str, Any]], list[dict[str, Any]]]:
    """Build the public reader layer from exact block.text values plus marked apparatus."""
    sections = data.get("sections", [])
    pieces: list[str] = []
    spans: list[dict[str, Any]] = []
    char_pos = 0
    note_count = 0
    heading_count = 0
    variant_count = 0
    for section in sections:
        lines: list[str] = []
        group = section.get("group")
        label = section.get("label") or section.get("id") or "Source section"
        for heading in section.get("headings", []):
            lines.append(str(heading))
            heading_count += 1
        blocks = section.get("blocks", [])
        body = [block.get("text", "") for block in blocks if isinstance(block, dict)]
        body = [value for value in body if isinstance(value, str) and value]
        if body:
            lines.extend(body)
        elif section.get("empty") or not blocks:
            lines.append("∅")
        for index, note in enumerate(section.get("notes", []), 1):
            lines.append(f"〔{index}〕 {note}")
            note_count += 1
        for variant in section.get("variants", []):
            if isinstance(variant, dict):
                ordered_keys = [key for key in ("sic", "corr", "orig", "reg") if key in variant]
                ordered_keys.extend(key for key in variant if key not in ordered_keys)
                rendered = "; ".join(f"{key}: {variant[key]}" for key in ordered_keys)
            else:
                rendered = str(variant)
            lines.append(f"⟦{rendered}⟧")
            variant_count += 1
        text = "\n".join(lines)
        if spans:
            text = "\n\n" + text
        start = char_pos
        pieces.append(text)
        char_pos += len(text)
        spans.append({"kind": "section", "section": section, "start": start, "end": char_pos})
    book_notes = data.get("notes", [])
    if book_notes:
        text = "\n\n" + "\n".join(
            f"※ {index}: {note}" for index, note in enumerate(book_notes, 1)
        )
        if pieces:
            start = char_pos
            pieces.append(text)
            char_pos += len(text)
            spans.append({"kind": "book_notes", "start": start, "end": char_pos})
    adjustments = data.get("adjustments", [])
    if adjustments:
        def render_adjustment(value: Any) -> str:
            if isinstance(value, dict):
                return "; ".join(f"{key}: {val}" for key, val in value.items())
            return str(value)
        text = "\n\n" + "\n".join(f"† {render_adjustment(x)}" for x in adjustments)
        if pieces:
            start = char_pos
            pieces.append(text)
            char_pos += len(text)
            spans.append({"kind": "adjustments", "start": start, "end": char_pos})
    full = "".join(pieces)
    # Any global apparatus not attached to a section remains in the exact JSON payload.
    return full, spans, [{"headings": heading_count, "notes": note_count, "variants": variant_count, "bookNotes": len(book_notes), "adjustments": len(adjustments)}]


def event(event_id: str, event_type: str, method: str, inputs: list[dict[str, Any]], outputs: list[dict[str, Any]], note: str) -> dict[str, Any]:
    return {
        "schema_version": "tos_provenance_event_v1", "event_id": event_id, "event_type": event_type, "event_version": 1,
        "started_at": MADE_AT, "ended_at": MADE_AT, "status": "completed",
        "agent_refs": ["model:codex"],
        "method": {"maker_type": "software", "name": method, "version": "1", "configuration": {"generator_ref": "access/public-corpus/prepare_retained_corpus.py", "purpose": note}},
        "inputs": inputs, "outputs": outputs,
    }


def inventory_owner_call(owner: str, request: dict[str, Any]) -> Any:
    wire = json.dumps(request, ensure_ascii=False, separators=(",", ":")) + "\n"
    proc = subprocess.run(
        [owner, "acquisition"], input=wire, text=True, capture_output=True, timeout=660
    )
    try:
        envelope = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"native inventory owner returned invalid JSON: {exc}") from exc
    if proc.returncode != 0 or envelope.get("kind") != "result":
        raise RuntimeError(
            f"native inventory owner failed: {envelope.get('message', proc.stderr.strip())}"
        )
    return envelope["value"]


@lru_cache(maxsize=8)
def inventory_owner_metadata(owner: str) -> dict[str, Any]:
    value = inventory_owner_call(owner, {"family": "inventory", "operation": "metadata"})
    if not isinstance(value, dict) or not value.get("schema_ref") or not value.get("generator_version"):
        raise RuntimeError("native inventory owner metadata is incomplete")
    if not value.get("authority_boundaries", {}).get("2"):
        raise RuntimeError("native inventory owner did not declare authority boundary v2")
    return value


def inventory_file_profile(
    owner: str, repo_root: Path, item_dir_ref: str, payload_entry: dict[str, Any]
) -> dict[str, Any]:
    relative = Path(payload_entry["relative_path"])
    if relative.is_absolute() or ".." in relative.parts:
        raise RuntimeError(f"unsafe Item payload path: {payload_entry['relative_path']}")
    payload_root = (repo_root / "ToS/source-witnesses").resolve(strict=True)
    payload_path = repo_root / item_dir_ref / relative
    if payload_path.is_symlink():
        raise RuntimeError(f"refusing symlink Item payload: {payload_path}")
    payload_path = payload_path.resolve(strict=True)
    if not payload_path.is_relative_to(payload_root) or not payload_path.is_file():
        raise RuntimeError(f"Item payload escaped its source root: {payload_path}")
    request: dict[str, Any] = {
        "family": "inventory",
        "operation": "file",
        "payload_path": str(payload_path),
        "payload_entry": payload_entry,
    }
    if payload_entry.get("media_type") == "text/plain":
        request["plain_text_profile"] = "plain_text_v1"
    value = inventory_owner_call(owner, request)
    for key in ("file_id", "file_sha256", "media_type"):
        if value.get(key) != payload_entry.get("sha256" if key == "file_sha256" else key):
            raise RuntimeError(f"native inventory output disagrees with Item manifest field {key}")
    return value


def compose_native_inventory(
    owner: str,
    repo_root: Path,
    item_manifest_ref: str,
    expected_event_ref: str | None = None,
) -> tuple[dict[str, Any], list[dict[str, Any]], str]:
    manifest_path = repo_root / item_manifest_ref
    manifest_bytes = manifest_path.read_bytes()
    manifest = json.loads(manifest_bytes)
    item_dir_ref = str(Path(item_manifest_ref).parent)
    files = [
        inventory_file_profile(owner, repo_root, item_dir_ref, entry)
        for entry in manifest.get("payload_files", [])
    ]
    if not files or len(files) != len(manifest.get("payload_files", [])):
        raise RuntimeError(f"native inventory returned incomplete files for {item_manifest_ref}")
    inventory_path = repo_root / item_dir_ref / "resource-inventory.json"
    prior: dict[str, Any] = {}
    if inventory_path.exists():
        prior = json.loads(inventory_path.read_bytes())
    event_ref = prior.get("provenance_event_ref") or expected_event_ref
    if not event_ref:
        raise RuntimeError(f"no retained inventory provenance event reference for {item_manifest_ref}")
    metadata = inventory_owner_metadata(owner)
    inventory = {
        "$schema": metadata["schema_ref"],
        "schema_version": "tos_source_resource_inventory_v1",
        "item_id": manifest["item_id"],
        "generated_from_manifest_ref": item_manifest_ref,
        "inventory_authority": "mechanical_metadata_only",
        "source_text_included": False,
        "files": files,
        # This is the schema's historical compatibility identifier. Per-file
        # profiles above come from the maintained native Rust owner operation.
        "generator": {
            "name": "build_source_resource_inventories.py",
            "version": metadata["generator_version"],
        },
        "provenance_event_ref": event_ref,
        "inventory_version": prior.get("inventory_version", 1),
        "supersedes_inventory_ref": prior.get("supersedes_inventory_ref"),
        "authority_boundary": metadata["authority_boundaries"]["2"],
    }
    event_inputs = [{
        "ref": item_manifest_ref,
        "sha256": sha(manifest_bytes),
        "role": "item_manifest",
    }]
    for entry in manifest["payload_files"]:
        event_inputs.append({
            "ref": f"{item_dir_ref}/{entry['relative_path']}",
            "sha256": entry["sha256"],
            "role": "native_inventory_payload",
        })
    event_inputs.sort(key=lambda item: (item["ref"], item["role"]))
    return inventory, event_inputs, item_dir_ref


def write_metadata_refresh(
    owner: str,
    repo_root: Path,
    item_manifest_ref: str,
    expected_event_ref: str | None = None,
) -> None:
    global MADE_AT
    inventory, inventory_inputs, item_dir_ref = compose_native_inventory(
        owner, repo_root, item_manifest_ref, expected_event_ref
    )
    inventory_path = repo_root / item_dir_ref / "resource-inventory.json"
    inventory_bytes = json_bytes(inventory)
    provenance_path = repo_root / item_dir_ref / "provenance.jsonl"
    provenance = [
        json.loads(line)
        for line in provenance_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    metadata_ref = f"{item_dir_ref}/source-metadata-snapshot.json"
    metadata_bytes = (repo_root / metadata_ref).read_bytes()
    rights_events = [row for row in provenance if row.get("event_type") == "rights_assessment"]
    if len(rights_events) != 1:
        raise RuntimeError(f"expected one rights event in {provenance_path}")
    rights_events[0]["inputs"] = [{
        "ref": metadata_ref,
        "sha256": sha(metadata_bytes),
        "role": "rights_source_metadata_snapshot",
    }]
    rights_ref = f"{item_dir_ref}/rights.json"
    rights_sha = sha((repo_root / rights_ref).read_bytes())
    matching_rights_outputs = [
        row for row in rights_events[0].get("outputs", []) if row.get("ref") == rights_ref
    ]
    if len(matching_rights_outputs) != 1:
        raise RuntimeError(f"expected one rights output reference in {provenance_path}")
    matching_rights_outputs[0]["sha256"] = rights_sha

    MADE_AT = datetime.now(timezone.utc).isoformat(timespec="seconds")
    inventory_event_ref = inventory["provenance_event_ref"]
    inventory_event = event(
        inventory_event_ref,
        "forensic_inspection",
        "nativefileprofilescomposedbyretainedpreparation",
        inventory_inputs,
        [{
            "ref": f"{item_dir_ref}/resource-inventory.json",
            "sha256": sha(inventory_bytes),
            "role": "metadata_inventory",
        }],
        "Native operation=file produced each payload profile; retained preparation composed the schema envelope because native operation=item accepts only one plain-text profile for all files. The generator identifier remains the schema's historical compatibility value.",
    )
    replaced = False
    for index, row in enumerate(provenance):
        if row.get("event_id") == inventory_event_ref:
            provenance[index] = inventory_event
            replaced = True
    if not replaced:
        provenance.append(inventory_event)
    provenance_bytes = (
        "\n".join(json.dumps(row, ensure_ascii=False, separators=(",", ":")) for row in provenance)
        + "\n"
    ).encode("utf-8")
    for path, content in ((inventory_path, inventory_bytes), (provenance_path, provenance_bytes)):
        tmp = path.with_name(path.name + ".tmp")
        tmp.write_bytes(content)
        os.replace(tmp, path)


def resolve_inventory_owner(value: str | None) -> str:
    owner = value or os.environ.get("TOS_NATIVE_OWNER_COMMAND") or shutil.which("tos-native-owner-command")
    if not owner:
        raise RuntimeError("provide --native-owner-command or TOS_NATIVE_OWNER_COMMAND")
    path = Path(owner).resolve(strict=True)
    if not path.is_file() or not os.access(path, os.X_OK):
        raise RuntimeError(f"native inventory owner is not an executable file: {path}")
    return str(path)


def refresh_retained_metadata(owner: str) -> int:
    manifests = []
    for path in (REPO / "ToS/source-witnesses").rglob("item.manifest.json"):
        manifest = load(path)
        if str(manifest.get("item_id", "")).startswith("tos.item.retained.antiquity."):
            manifests.append(path)
    manifests.sort()
    if len(manifests) != 96:
        raise RuntimeError(f"expected exactly 96 retained Antiquity Items, found {len(manifests)}")
    for manifest_path in manifests:
        item_manifest_ref = manifest_path.relative_to(REPO).as_posix()
        write_metadata_refresh(owner, REPO, item_manifest_ref)
    return len(manifests)


def new_work_id(book: dict[str, Any]) -> str:
    return CANONICAL_WORKS.get(book["id"], f"tos.work.retained-antiquity.{slug(book['id'])}")


def output_for(book: dict[str, Any], version: dict[str, Any], data: dict[str, Any]) -> tuple[dict[str, bytes], dict[str, Any]]:
    book_id = slug(book["id"])
    legacy_version_id = version["id"]
    version_slug = slug(legacy_version_id)
    version_id = f"retained-{version_slug}"
    work_id = new_work_id(book)
    expr_id = f"tos.expression.retained.antiquity.{book_id}.{version_slug}"
    edition_id = f"tos.edition.retained.antiquity.{book_id}.{version_slug}"
    item_id = f"tos.item.retained.antiquity.{book_id}.{version_slug}"
    # Use a separate expression, edition and item even where the legacy raw TEI digest overlaps.
    work_path_names = {"meno": ("plato", "meno"), "apology": ("plato", "apology"), "ethics": ("aristotle", "nicomachean-ethics")}
    work_author_dir, work_dir_name = work_path_names.get(book["id"], (slug(book.get("author") or "unknown"), book_id))
    work_rel_dir = Path("ToS/source-witnesses/works") / work_author_dir / work_dir_name
    rel_dir = work_rel_dir / "expressions" / f"retained-{version_slug}"
    edition_dir = rel_dir / "editions" / "retained-reading"
    item_dir = edition_dir / "items" / "retained-antiquity-20261027"
    source_json_path = LEGACY / "texts" / f"{legacy_version_id}.json"
    source_json_bytes = source_json_path.read_bytes()
    source_json_sha = sha(source_json_bytes)
    text, section_spans, apparatus_stats = source_display_text(data)
    text_bytes = text.encode("utf-8")
    text_layer_sha = sha(text_bytes)
    json_file_id = f"tos.file.sha256.{source_json_sha}"
    text_file_id = f"tos.file.sha256.{text_layer_sha}"
    source_url = version.get("source") or ""
    if not source_url or version.get("license") != LICENSE:
        raise ValueError(f"missing pinned source URL or CC BY-SA grant for {legacy_version_id}")

    anchors: list[dict[str, Any]] = []
    units: list[dict[str, Any]] = []
    unit_refs: list[str] = []
    structure: list[dict[str, Any]] = []
    book_notes_unit_ids: list[str] = []
    book_notes_anchor_ids: list[str] = []
    adjustment_unit_ids: list[str] = []
    scope_anchor = opaque("anchor", f"scope:{legacy_version_id}")
    anchors.append({
        "anchor_ref": scope_anchor, "ordinal": 1, "text_layer_ref": f"{item_dir}/payload/reading-layer.txt",
        "text_layer_sha256": text_layer_sha,
        "selector": {"type": "text_position", "start": 0, "end": len(text), "position_unit": "unicode_code_point", "interval": "half_open"},
        "exact_sha256": text_layer_sha, "anchor_role": "scope",
        "source_return": {"required": True, "locator_ref": source_url},
    })
    anchor_ord = 2
    unit_intervals: list[tuple[int, int]] = []
    for span_index, span in enumerate(section_spans):
        span_kind = span["kind"]
        section = span.get("section", {})
        section_start, section_end = span["start"], span["end"]
        section_text = text[section_start:section_end]
        chunks = split_utf8(section_text)
        first_unit_id = None
        first_anchor_id = None
        for chunk_index, chunk in enumerate(chunks):
            local_start = sum(len(c) for c in chunks[:chunk_index])
            start, end = section_start + local_start, section_start + local_start + len(chunk)
            source_section_id = section.get("id", span_index) if span_kind == "section" else span_kind
            key = f"{legacy_version_id}:{source_section_id}:chunk:{chunk_index}"
            unit_id = opaque("text-unit", key)
            anchor_id = opaque("anchor", f"{key}:anchor")
            first_unit_id = first_unit_id or unit_id
            first_anchor_id = first_anchor_id or anchor_id
            unit_refs.append(unit_id)
            chunk_sha = text_sha(chunk)
            anchors.append({
                "anchor_ref": anchor_id, "ordinal": anchor_ord, "text_layer_ref": f"{item_dir}/payload/reading-layer.txt",
                "text_layer_sha256": text_layer_sha,
                "selector": {"type": "text_position", "start": start, "end": end, "position_unit": "unicode_code_point", "interval": "half_open"},
                "exact_sha256": chunk_sha, "anchor_role": "content",
                "source_return": {"required": True, "locator_ref": source_url},
            })
            anchor_ord += 1
            units.append({
                "unit_id": unit_id, "unit_version": 1, "supersedes_unit_ref": None,
                "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
                "unit_kind": "section" if span_kind == "section" else "other", "surface_posture": "source_bearing", "continuity": "contiguous",
                "ordered_anchor_refs": [anchor_id], "parent_unit_refs": [], "ordered_child_unit_refs": [],
                "boundary_posture": "method_proposed",
                "certainty": {"value": 1, "meaning": "maker-declared-boundary-confidence-not-truth-probability"},
                "status_reason": "Deterministic retained-JSON section chunk; not a reviewed linguistic boundary." if span_kind == "section" else "Unsectioned retained book-level apparatus chunk; navigation only.",
                "source_text_mutated": False, "semantic_promotion": False,
            })
            unit_intervals.append((start, end))
        if span_kind == "section":
            section_id = str(section.get("id", span_index + 1))
            raw_label = section.get("label") or section_id
            group_label = section.get("group") or ""
            structure.append({
                "id": first_anchor_id,
                "label": f"{group_label + ' · ' if group_label else ''}{raw_label}", "rawLabel": raw_label, "unitId": first_unit_id,
                "level": 1, "parentId": None, "sectionId": section_id,
                "group": group_label, "markers": section.get("markers") or [],
            })
        elif span_kind == "book_notes":
            book_notes_unit_ids.extend(units[-len(chunks):][i]["unit_id"] for i in range(len(chunks)))
            book_notes_anchor_ids.append(first_anchor_id)
        elif span_kind == "adjustments":
            adjustment_unit_ids.extend(units[-len(chunks):][i]["unit_id"] for i in range(len(chunks)))

    # Book-level notes/adjustments follow the source sections and are retained in
    # the same immutable reading layer. The source JSON remains the exact payload.
    if len(unit_refs) == 0:
        raise ValueError(f"no units for {legacy_version_id}")
    if unit_intervals[0][0] != 0 or unit_intervals[-1][1] != len(text) or any(a[1] != b[0] for a, b in zip(unit_intervals, unit_intervals[1:])):
        raise ValueError(f"non-exhaustive reading-layer coverage for {legacy_version_id}")
    if any(len(text[a:b].encode("utf-8")) > MAX_UNIT_BYTES for a, b in unit_intervals):
        raise ValueError(f"oversized source unit for {legacy_version_id}")

    scheme_id = opaque("text-unit-scheme", f"retained:{legacy_version_id}")
    seg_id = opaque("text-segmentation", f"retained:{legacy_version_id}")
    seg_event_id = f"tos.event.retained-segmentation.{version_slug}.20261027"
    acquisition_event_id = f"tos.event.retained-acquisition.{version_slug}.20261027"
    rights_id = f"tos.rights.retained-antiquity.{book_id}.{version_slug}.20261027"
    packet_id = opaque("source-text-unit-packet", f"retained:{legacy_version_id}")
    attribution = version.get("attribution") or f"{book.get('author', 'Unknown')} · retained source record"
    if version.get("coverageLimit"):
        # The Rust projection currently carries attribution into the Native
        # Source inspector but drops unknown version metadata fields. Keep the
        # exact legacy coverage limit visible through that current whitelist.
        attribution = f"{attribution}\n{version['coverageLimit']}"
    credit = version.get("credit") or ""
    pub = version.get("publication") or {}
    terms_refs = [source_url, LICENSE]
    perseus = re.search(r"PerseusDL/(canonical-(?:greek|latin)Lit)/(?:blob/)?([0-9a-f]{40})/", source_url)
    if perseus:
        terms_refs.append(f"https://github.com/PerseusDL/{perseus.group(1)}/blob/{perseus.group(2)}/README.md")
    permissions = [f"Attribution: {attribution}", f"Free public reading and adaptation under {LICENSE}"]
    if "PerseusDL/" in source_url:
        permissions.append("Perseus repository guidance asks users to offer modifications upstream; no upstream submission was made.")
    rights = {
        "schema_version": "tos_rights_record_v1", "rights_id": rights_id,
        "scope_refs": [item_id, json_file_id, text_file_id], "assessment_status": "licensed",
        "jurisdictions_reviewed": ["US"], "source_refs": terms_refs,
        "permissions": permissions,
        "restrictions": ["Give attribution, link the license, and indicate changes.", "Distribute adaptations under the same CC BY-SA 4.0 license.", "Mark the retained JSON to public reading-layer derivation.", "No transcription, linguistic, semantic, canon or human-review claim."],
        "visibility": "public_payload", "redistribution_posture": "authorized_with_conditions",
        "derivative_posture": "allowed_with_conditions", "assessed_by": {"maker_type": "model", "agent_ref": "codex.retained-antiquity-publication.20261027"},
        "assessed_at": MADE_AT, "rationale": "The exact legacy catalog version row carries CC BY-SA 4.0 and is included with its digest in the source metadata snapshot; the fixed source/header record is preserved in the exact retained JSON payload. This item is that retained JSON plus a separately hashed reader layer; the legacy raw-source digest is not represented as the JSON payload digest. Publication carries attribution, source and license routes. The reading layer is not a scholarly or human acceptance.",
        "review_status": "unreviewed", "license_uri": LICENSE, "record_version": 1,
    }
    scheme = {
        "scheme_id": scheme_id, "scheme_version": 1, "supersedes_scheme_ref": None,
        "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
        "scheme_name": "Retained source section and bounded reading chunk",
        "analysis_role": "source_structure", "boundary_basis": "rule_based",
        "unit_kinds": ["section"] + (["other"] if book_notes_unit_ids or adjustment_unit_ids else []),
        "method": {"maker_kind": "software", "agent_ref": "codex.retained-antiquity-publication.20261027", "method_name": "retained-json-reading-layer", "method_version": "1", "software_refs": ["Python 3 stdlib"], "model_ref": None, "configuration_ref": f"{item_dir}/provenance.jsonl", "locale": version.get("language", "en"), "unicode_version": None, "unicode_revision": None, "tailoring_ref": None, "provenance_event_ref": seg_event_id, "made_at": MADE_AT, "output_posture": "method_result_not_source_or_linguistic_truth"},
        "policies": {"normalization": "no-text-mutation-separate-successor-layer", "punctuation": "included_in_neighbor", "whitespace": "included_in_neighbor", "line_break": "included_in_neighbor", "hyphenation": "preserve_source", "unreported_gaps_allowed": False, "overlap": "forbid"},
        "authority_limit": {"algorithmic_output_is_source_truth": False, "algorithmic_output_is_linguistic_truth": False, "model_subword_is_lexeme": False, "unit_identity_is_semantic_identity": False, "text_mutation_allowed": False},
    }
    packet = {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json",
        "schema_version": "tos_source_text_unit_packet_v1", "packet_id": packet_id, "packet_version": 1,
        "supersedes_packet_ref": None, "content_posture": "source_bound",
        "source_scope": {"work_ref": work_id, "expression_ref": expr_id, "edition_ref": edition_id, "item_ref": item_id, "file_ref": json_file_id, "file_sha256": source_json_sha},
        "source_layer": {"text_layer_ref": f"{item_dir}/payload/reading-layer.txt", "text_layer_sha256": text_layer_sha, "language": version.get("language", "en"), "media_type": "text/plain; charset=utf-8", "unicode_form": "source_preserved", "position_unit": "unicode_code_point", "interval": "half_open", "immutable": True, "visibility": "public", "publication_authorized": True},
        "schemes": [scheme], "anchors": anchors, "units": units,
        "segmentations": [{"segmentation_id": seg_id, "segmentation_version": 1, "supersedes_segmentation_ref": None, "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries", "scheme_ref": scheme_id, "ordered_unit_refs": unit_refs, "coverage": {"scope_anchor_ref": scope_anchor, "coverage_posture": "exhaustive_nonoverlapping", "excluded_anchor_refs": [], "unreported_gaps_allowed": False, "overlap_requires_declaration": True, "source_reconstruction_required": True}, "status": "proposed", "status_reason": "Deterministic display and navigation segmentation; no textual, linguistic or semantic acceptance.", "maker": scheme["method"], "competing_segmentation_refs": [], "review_refs": [], "declared_uses": ["navigation", "source_observation"], "source_text_authority": False, "linguistic_authority": False, "semantic_authority": False}],
        "reviews": [], "projections": [],
        "rights_and_visibility": {"source_visibility": "public", "packet_visibility": "public", "effective_visibility": "public", "rights_record_refs": [f"{item_dir}/rights.json"], "private_source_used": False, "publication_authorized": True, "inheritance_policy": "most-restrictive-source-packet-and-destination-wins"},
        "authority_boundary": {"tree_role": "orientation", "graph_role": "relation", "source_role": "authority", "validators_prove_mechanics_not_truth": True, "segmentation_is_method_result_not_source_truth": True, "source_unit_is_not_lexeme_sign_or_concept": True, "model_token_is_not_linguistic_token": True, "projection_is_owner_truth": False, "legacy_bulk_migration_authorized": False},
    }
    packet_bytes = json_bytes(packet)
    packet_sha = sha(packet_bytes)
    item_manifest = {
        "schema_version": "tos_source_item_manifest_v1", "item_id": item_id, "item_kind": "derived_publication",
        "embodiment_ref": edition_id, "storage_posture": "local_gitignored_payload",
        "payload_files": [
            {"file_id": json_file_id, "relative_path": "payload/retained-reading.json", "original_basename": f"{legacy_version_id}.json", "media_type": "application/json", "byte_size": len(source_json_bytes), "sha256": source_json_sha, "fixity_verified_at": MADE_AT},
            {"file_id": text_file_id, "relative_path": "payload/reading-layer.txt", "original_basename": "reading-layer.txt", "media_type": "text/plain", "byte_size": len(text_bytes), "sha256": text_layer_sha, "fixity_verified_at": MADE_AT},
        ],
        "acquisition_event_ref": acquisition_event_id, "rights_ref": f"{item_dir}/rights.json", "provenance_ref": f"{item_dir}/provenance.jsonl", "forensic_report_ref": f"{item_dir}/forensic-report.md", "resource_inventory_ref": f"{item_dir}/resource-inventory.json", "visibility": "public_payload", "manifest_version": 1, "source_record_refs": [f"{item_dir}/source-metadata-snapshot.json"],
    }
    manifest_bytes = json_bytes(item_manifest)
    metadata = {
        "schema_version": "tos_public_source_metadata_snapshot_v1", "record_version": 1, "item_id": item_id, "captured_at": MADE_AT,
        "retained_copy": {"local_record_ref": f"access/web/antiquity/books/texts/{legacy_version_id}.json", "copy_origin": "local retained legacy export", "legacy_json_fetch_time": "unknown", "upstream_source_url_from_catalog": source_url, "file_ref": json_file_id, "byte_size": len(source_json_bytes), "sha256": source_json_sha},
        "legacy_catalog_record": {"record_ref": f"access/web/antiquity/books/catalog.json#version={legacy_version_id}", "record_sha256": sha(json_bytes(version)), "record": version},
        "legacy_source_identity": {"legacy_version_id": legacy_version_id, "legacy_source_sha256_claim": version.get("sha256"), "legacy_representation_sha256_claim": version.get("representationSha256"), "retained_json_payload_sha256": source_json_sha, "raw_source_payload_present": False},
        "source_header": {"format": "retained header field from exact legacy JSON payload", "titles": values_from_header(data.get("header", ""))[0], "authors": values_from_header(data.get("header", ""))[1], "editors_or_translators": values_from_header(data.get("header", ""))[2], "header_retained_in": "payload/retained-reading.json"},
        "bibliography": {"work_ref": work_id, "expression_ref": expr_id, "edition_ref": edition_id, "legacy_book_title": book.get("title"), "legacy_credit": credit, "publication": pub},
        "coverage": {key: version.get(key) for key in ("historicalTranscription", "headerKind", "coverageStatus", "coverageLimit", "proofreadingStatus", "sourceIndex", "pinnedPageRevisions") if key in version},
        "terms_and_public_metadata_refs": terms_refs,
        "retained_json_counts": {"section_count": len(data.get("sections", [])), "block_count": sum(len(s.get("blocks", [])) for s in data.get("sections", [])), "apparatus": apparatus_stats[0]},
        "snapshot_boundary": "Metadata describes the exact retained JSON and its reader-layer derivative. A legacy raw-source digest is a separate claim and no raw TEI payload is asserted present.",
    }
    prov = [
        event(acquisition_event_id, "acquisition", "copy-exact-retained-json", [{"ref": f"access/web/antiquity/books/texts/{legacy_version_id}.json", "sha256": source_json_sha, "role": "retained_json_input"}, {"ref": f"access/web/antiquity/books/catalog.json#version={legacy_version_id}", "sha256": sha(json_bytes(version)), "role": "legacy_catalog_version_row"}], [{"ref": f"{item_dir}/payload/retained-reading.json", "sha256": source_json_sha, "role": "retained_json_payload"}], "Copies the exact old JSON bytes. It is not a claim that the legacy raw TEI bytes are present; upstream source retrieval time is unknown."),
        event(f"tos.event.retained-render.{version_slug}.20261027", "render", "retained-json-to-reading-layer", [{"ref": f"{item_dir}/payload/retained-reading.json", "sha256": source_json_sha, "role": "retained_json_payload"}], [{"ref": f"{item_dir}/payload/reading-layer.txt", "sha256": text_layer_sha, "role": "reader_text_layer"}], "Uses each section/block text value, not HTML. Original headings are kept verbatim, section notes use 〔n〕, both keys and values of each variant use ⟦…⟧, empty sections use ∅, and book notes and retained adjustments are separate unsectioned units marked ※ and †. The exact JSON remains separate."),
        event(seg_event_id, "segmentation", "retained-json-section-navigation", [{"ref": f"{item_dir}/payload/reading-layer.txt", "sha256": text_layer_sha, "role": "reader_text_layer"}], [{"ref": f"{item_dir}/source-text-unit-packet.v1.json", "sha256": packet_sha, "role": "source_structure_packet"}], "Bounded chunks are deterministic navigation units; no linguistic or semantic judgment is made."),
        event(f"tos.event.retained-rights.{version_slug}.20261027", "rights_assessment", "license-attribution-recording", [{"ref": f"{item_dir}/source-metadata-snapshot.json", "sha256": sha(json_bytes(metadata)), "role": "rights_source_metadata_snapshot"}], [{"ref": f"{item_dir}/rights.json", "sha256": sha(json_bytes(rights)), "role": "rights_record"}], "Catalog license row is CC BY-SA 4.0; source/header evidence and its license URL are retained in the exact public metadata snapshot. Rights record is unreviewed."),
    ]
    fixity = f"{source_json_sha}  payload/retained-reading.json\n{text_layer_sha}  payload/reading-layer.txt\n"
    report = (
        f"# Retained-source preparation record\n\n"
        f"- Legacy version: `{legacy_version_id}`\n- Source URL: {source_url}\n- License row: {LICENSE}\n"
        f"- Legacy raw-source SHA claim: `{version.get('sha256')}` (raw payload absent; not used as retained JSON fixity)\n"
        f"- Legacy representation SHA claim: `{version.get('representationSha256')}`\n- Exact retained JSON SHA-256: `{source_json_sha}`\n- Derived reading-layer SHA-256: `{text_layer_sha}`\n"
        f"- Sections: {len(data.get('sections', []))}; reader units: {len(units)}; block count: {sum(len(s.get('blocks', [])) for s in data.get('sections', []))}\n"
        f"- Headers, notes, adjustments and structures remain in `payload/retained-reading.json`; rendered body uses each block's `text` value, not its HTML field.\n"
        f"- Segmentation, extraction and rights are method records, not text, language, semantic, canon or human acceptance.\n"
    )
    credit_role = str(version.get("creditRole", "")).casefold()
    credit_text = str(version.get("credit", "")).casefold()
    is_translation = "translator" in credit_role or credit_text.startswith(("перевод:", "translation:", "translated by:")) or "translator" in credit_text
    expression_role = "translation" if is_translation or version.get("language") not in {"grc", "la"} else "source_language"
    expression = {"schema_version": "tos_corpus_record_v1", "record_type": "expression", "record_id": expr_id, "preferred_label": version_label(book, version, data), "identity_status": "provisional", "source_refs": [source_url], "external_identifiers": [], "same_as_posture": "no_equivalence_claim", "work_ref": work_id, "language": version.get("language", "en"), "expression_role": expression_role, "responsibility_claim_refs": [], "embodiment_claim_refs": [], "record_version": 1, "notes": "A separately retained legacy reading version; no equivalence to another local edition or review is claimed."}
    edition = {"schema_version": "tos_corpus_record_v1", "record_type": "edition", "record_id": edition_id, "preferred_label": version_label(book, version, data), "identity_status": "provisional", "source_refs": [source_url], "external_identifiers": [], "same_as_posture": "no_equivalence_claim", "record_version": 1, "notes": "A retained JSON reading representation and bounded public reader derivative; original raw source payload is not asserted present.", "embodies_expression_refs": [expr_id], "publication_claim_refs": [], "exemplar_claim_refs": []}
    files: dict[str, bytes] = {
        f"{rel_dir}/expression.json": json_bytes(expression),
        f"{edition_dir}/edition.json": json_bytes(edition),
        f"{item_dir}/item.manifest.json": manifest_bytes,
        f"{item_dir}/rights.json": json_bytes(rights),
        f"{item_dir}/source-text-unit-packet.v1.json": packet_bytes,
        f"{item_dir}/source-metadata-snapshot.json": json_bytes(metadata),
        f"{item_dir}/provenance.jsonl": ("\n".join(json.dumps(x, ensure_ascii=False, separators=(",", ":")) for x in prov) + "\n").encode("utf-8"),
        f"{item_dir}/forensic-report.md": report.encode("utf-8"),
        f"{item_dir}/fixity.sha256": fixity.encode("ascii"),
        f"{item_dir}/payload/retained-reading.json": source_json_bytes,
        f"{item_dir}/payload/reading-layer.txt": text_bytes,
    }
    version_record = {
        "id": version_id, "legacyId": legacy_version_id, "label": version_label(book, version, data),
        "language": version.get("language", "en"), "role": expression["expression_role"],
        "packetRef": f"{item_dir}/source-text-unit-packet.v1.json", "packetSha256": packet_sha,
        "segmentationRef": seg_id, "rightsRef": f"{item_dir}/rights.json", "rightsSha256": sha(json_bytes(rights)),
        "sourceUrl": source_url, "attribution": attribution,
        "license": {"label": "Creative Commons Attribution-ShareAlike 4.0 International", "url": LICENSE},
        "structure": [{"id": x["id"], "label": x["label"], "unitId": x["unitId"], "level": 1, "parentId": None} for x in structure],
        "coverageLimit": version.get("coverageLimit"), "coverageStatus": version.get("coverageStatus"), "proofreadingStatus": version.get("proofreadingStatus"), "headerKind": version.get("headerKind"), "historicalTranscription": version.get("historicalTranscription"),
    }
    for key in ("coverageLimit", "coverageStatus", "proofreadingStatus", "headerKind", "historicalTranscription"):
        if version_record.get(key) is None:
            del version_record[key]
    if book_notes_unit_ids:
        version_record["structure"].append({"id": book_notes_anchor_ids[0], "label": "Notes on the work", "unitId": book_notes_unit_ids[0], "level": 1, "parentId": None})
    map_version = {
        "legacyId": legacy_version_id, "versionId": version_id, "language": version.get("language", "en"),
        "revision": text_layer_sha,
        "structure": [{"label": x["rawLabel"], "unitId": x["unitId"], "sectionId": x["sectionId"], "group": x["group"], "markers": x["markers"]} for x in structure],
    }
    if book_notes_unit_ids:
        map_version["bookNotesUnitIds"] = book_notes_unit_ids
    if adjustment_unit_ids:
        map_version["adjustmentUnitIds"] = adjustment_unit_ids
    acq_row = {"version": version_id, "legacyId": legacy_version_id, "packet": version_record["packetRef"], "units": len(units), "textBytes": len(text_bytes), "sourceSha256": source_json_sha, "textLayerSha256": text_layer_sha, "legacyRawSourceSha256": version.get("sha256"), "legacyRepresentationSha256": version.get("representationSha256")}
    header_authors = values_from_header(data.get("header", ""))[1]
    details = {"legacy_id": legacy_version_id, "version_id": version_id, "work_id": work_id, "expr_id": expr_id, "edition_id": edition_id, "item_id": item_id, "item_dir_ref": str(item_dir.relative_to(REPO)), "inventory_event_ref": f"tos.event.retained-inventory.{version_slug}.20261027", "source_json_sha": source_json_sha, "text_sha": text_layer_sha, "units": len(units), "text_bytes": len(text_bytes), "structure": structure, "bookNotesUnitIds": book_notes_unit_ids, "bookNotesAnchorIds": book_notes_anchor_ids, "adjustmentUnitIds": adjustment_unit_ids, "version_record": version_record, "map_version": map_version, "acq_row": acq_row, "book": book, "author_label": header_authors[0] if header_authors else (book.get("author") or "Unknown")}
    # Files are addressed relative to the repository, not the process cwd.
    return files, details


def make_work_record(book: dict[str, Any], version: dict[str, Any], data: dict[str, Any]) -> dict[str, Any]:
    work_id = new_work_id(book)
    head_titles, authors, _ = values_from_header(data.get("header", ""))
    source = version.get("source", "")
    return {"schema_version": "tos_corpus_record_v1", "record_type": "work", "record_id": work_id, "preferred_label": book.get("title") or (head_titles[0] if head_titles else book["id"]), "identity_status": "provisional", "source_refs": [v.get("source", source) for v in book.get("versions", []) if v.get("source")], "external_identifiers": [], "same_as_posture": "no_equivalence_claim", "expression_claim_refs": [], "record_version": 1, "notes": f"Retained Antiquity source work. Header names: {'; '.join(authors) if authors else book.get('author', 'unknown')}. No semantic equivalence or canon claim."}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--plan", action="store_true", help="read-only inventory, transform and size plan")
    mode.add_argument("--apply", action="store_true", help="write prepared source packets and selection inputs")
    mode.add_argument("--refresh-metadata", action="store_true", help="refresh only retained inventory and provenance metadata through the Native owner")
    parser.add_argument("--max-output-bytes", type=int, default=MAX_OUTPUT_BYTES)
    parser.add_argument("--native-owner-command", help="path to tos-native-owner-command (or set TOS_NATIVE_OWNER_COMMAND)")
    args = parser.parse_args()
    if REPO != Path("/srv/abyss-machine/storage/source-worktrees/tos-public-prototypes-20261026"):
        raise SystemExit(f"refusing worktree outside assigned path: {REPO}")
    head = subprocess.check_output(["git", "-C", str(REPO), "rev-parse", "HEAD"], text=True).strip()
    if head != PINNED_HEAD:
        raise SystemExit(f"refusing unpinned worktree HEAD {head}; expected {PINNED_HEAD}")
    legacy_head = subprocess.check_output(["git", "-C", str(LEGACY_REPO), "rev-parse", "HEAD"], text=True).strip()
    if legacy_head != LEGACY_COMMIT:
        raise SystemExit(f"refusing unpinned legacy source HEAD {legacy_head}; expected {LEGACY_COMMIT}")
    legacy_dirty = subprocess.check_output(["git", "-C", str(LEGACY_REPO), "status", "--porcelain", "--untracked-files=all", "--", "access/web/antiquity/books"], text=True)
    if legacy_dirty.strip():
        raise SystemExit("refusing dirty legacy input tree: access/web/antiquity/books")
    inventory_owner = resolve_inventory_owner(args.native_owner_command) if (args.apply or args.refresh_metadata) else None
    if args.refresh_metadata:
        refreshed = refresh_retained_metadata(inventory_owner)
        print(json.dumps({"mode": "refresh-metadata", "items": refreshed, "nativeFileProfileCalls": 2 * refreshed, "touchedFiles": ["resource-inventory.json", "provenance.jsonl"]}, ensure_ascii=False, indent=2))
        return 0
    global IDENTITIES, MINT_IDENTITIES, MADE_AT
    identity_path = REPO / "access/public-corpus/retained-identity-map.v1.json"
    if identity_path.exists():
        identity_doc = load(identity_path)
        if identity_doc.get("schema_version") != "retained_identity_map_v1" or not isinstance(identity_doc.get("assignments"), dict):
            raise SystemExit(f"invalid retained identity map: {identity_path}")
        IDENTITIES = dict(identity_doc["assignments"])
        MADE_AT = identity_doc.get("made_at") or datetime.now(timezone.utc).isoformat(timespec="seconds")
    else:
        IDENTITIES = {}
        MADE_AT = datetime.now(timezone.utc).isoformat(timespec="seconds")
    MINT_IDENTITIES = args.apply
    catalog_path = LEGACY / "catalog.json"
    catalog = load(catalog_path)
    if not isinstance(catalog, list) or len(catalog) != 49:
        raise SystemExit("legacy catalog must contain exactly 49 books")
    text_files = sorted((LEGACY / "texts").glob("*.json"))
    if len(text_files) != 96:
        raise SystemExit(f"expected 96 retained JSON texts, found {len(text_files)}")
    input_bytes = sum(p.stat().st_size for p in text_files) + catalog_path.stat().st_size
    if input_bytes > MAX_INPUT_BYTES:
        raise SystemExit(f"input guard exceeded: {input_bytes} > {MAX_INPUT_BYTES}")
    text_ids = {p.stem for p in text_files}
    rows = [(b, v) for b in catalog for v in b.get("versions", [])]
    if len(rows) != 96 or {v.get("id") for _, v in rows} != text_ids:
        raise SystemExit("catalog/text inventory mismatch")
    if any(v.get("license") != LICENSE for _, v in rows):
        raise SystemExit("every catalog version must carry the pinned CC BY-SA 4.0 grant")

    # Prepare all transformations in memory for a precise output-size guard.
    generated: list[tuple[dict[str, bytes], dict[str, Any], dict[str, Any], dict[str, Any]]] = []
    estimated_output = 0
    section_count = block_count = unit_count = text_bytes_total = 0
    for book, version in rows:
        data = load(LEGACY / "texts" / f"{version['id']}.json")
        files, details = output_for(book, version, data)
        work_record = None
        if book["id"] not in CANONICAL_WORKS:
            work_record = make_work_record(book, version, data)
        section_count += len(data.get("sections", []))
        block_count += sum(len(s.get("blocks", [])) for s in data.get("sections", []))
        unit_count += details["units"]
        text_bytes_total += details["text_bytes"]
        estimated_output += sum(len(content) for content in files.values()) + INVENTORY_ESTIMATE_BYTES
        generated.append((files, details, work_record or {}, data))
    identity_doc = {"schema_version": "retained_identity_map_v1", "made_at": MADE_AT, "policy": "UUID4 values are minted once for this migration and stored by stable legacy section/chunk key; labels, source text, offsets, and generated order are not ID material.", "assignments": dict(sorted(IDENTITIES.items()))}
    identity_bytes = json_bytes(identity_doc)
    estimated_output += len(identity_bytes)
    if estimated_output > args.max_output_bytes:
        raise SystemExit(f"output guard exceeded: {estimated_output} > {args.max_output_bytes}")
    if not args.apply:
        print(json.dumps({"mode": "plan", "head": head, "legacyHead": legacy_head, "sourceRoot": str(LEGACY), "books": len(catalog), "versions": len(rows), "inputBytes": input_bytes, "sections": section_count, "blocks": block_count, "units": unit_count, "readingLayerBytes": text_bytes_total, "estimatedNewSourceBytes": estimated_output, "maxOutputBytes": args.max_output_bytes, "root": str(REPO)}, ensure_ascii=False, indent=2))
        return 0

    # Persist minted identities first so an interrupted apply can be resumed
    # without assigning new opaque IDs to already named source positions.
    if identity_path.exists() and identity_path.read_bytes() != identity_bytes:
        identity_path.write_bytes(identity_bytes)
    elif not identity_path.exists():
        identity_path.write_bytes(identity_bytes)

    selection_path = REPO / "access/public-corpus/selection.v1.json"
    acquisition_path = REPO / "access/public-corpus/acquisition-index.v1.json"
    selection = load(selection_path)
    acquisition = load(acquisition_path)
    existing_docs = {doc["id"]: doc for doc in selection["documents"]}
    additions_by_doc: dict[str, list[dict[str, Any]]] = {}
    map_books: list[dict[str, Any]] = []
    new_acq: list[dict[str, Any]] = []
    mapping: dict[str, str] = {}
    collection = load(REPO / "access/web/src/public-research/antiquity-collection-data.json")
    entity_by_legacy = {e.get("id"): e for e in collection.get("entities", []) if e.get("kind") == "work"}
    author_by_id = {e.get("id"): e for e in collection.get("entities", []) if e.get("kind") == "author"}

    # Emit a single provisional Work record per non-canonical book.
    work_record_written: set[str] = set()
    for files, details, work_record, data in generated:
        for name, content in files.items():
            destination = REPO / name
            if destination.exists():
                if destination.read_bytes() != content:
                    if "ToS/source-witnesses/works/" not in name or "/expressions/retained-" not in name:
                        raise SystemExit(f"refusing to replace non-identical non-generated output: {name}")
                    tmp = destination.with_name(destination.name + ".tmp")
                    tmp.write_bytes(content)
                    os.replace(tmp, destination)
            else:
                destination.parent.mkdir(parents=True, exist_ok=True)
                tmp = destination.with_name(destination.name + ".tmp")
                tmp.write_bytes(content)
                os.replace(tmp, destination)
        write_metadata_refresh(
            inventory_owner,
            REPO,
            f"{details['item_dir_ref']}/item.manifest.json",
            details["inventory_event_ref"],
        )
        if work_record:
            book_id = details["book"]["id"]
            if book_id not in work_record_written:
                path_names = {"meno": ("plato", "meno"), "apology": ("plato", "apology"), "ethics": ("aristotle", "nicomachean-ethics")}
                author_dir, book_dir = path_names.get(book_id, (slug(details["book"].get("author") or "unknown"), slug(book_id)))
                path = REPO / "ToS/source-witnesses/works" / author_dir / book_dir / "work.json"
                content = json_bytes(work_record)
                if path.exists() and path.read_bytes() != content:
                    old = load(path)
                    if old.get("record_id") != work_record["record_id"]:
                        raise SystemExit(f"work record path collision: {path}")
                elif not path.exists():
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(content)
                work_record_written.add(book_id)
        document_id = details["work_id"]
        mapping[details["book"]["id"]] = document_id
        additions_by_doc.setdefault(document_id, []).append(details["version_record"])
        new_acq.append(details["acq_row"])
        map_books.append({"legacyId": details["book"]["id"], "title": details["book"].get("title"), "author": details["book"].get("author"), "documentId": document_id, "versions": [details["map_version"]]})

    # Consolidate map version rows by legacy book while retaining catalog order.
    combined_map: dict[str, dict[str, Any]] = {}
    for row in map_books:
        book_id = row["legacyId"]
        if book_id not in combined_map:
            combined_map[book_id] = {**row, "versions": []}
        combined_map[book_id]["versions"].extend(row["versions"])
    map_books = [combined_map[b["id"]] for b in catalog]

    for book in catalog:
        doc_id = new_work_id(book)
        current = existing_docs.get(doc_id)
        author_entity = author_by_id.get(book.get("author"))
        author_title = (author_entity or {}).get("title") or {}
        author_label = author_title.get("en") or " ".join(part.capitalize() for part in str(book.get("author") or "Unknown").replace("-", " ").split())
        if current is None:
            current = {"id": doc_id, "title": book.get("title") or book["id"], "author": author_label, "versions": []}
            selection["documents"].append(current)
            existing_docs[doc_id] = current
        elif book["id"] not in CANONICAL_WORKS:
            current["author"] = author_label
        by_version_id = {v.get("id"): index for index, v in enumerate(current.get("versions", []))}
        for version in additions_by_doc.get(doc_id, []):
            if version["id"] not in by_version_id:
                current.setdefault("versions", []).append(version)
                by_version_id[version["id"]] = len(current["versions"]) - 1
            else:
                current["versions"][by_version_id[version["id"]]] = version
    all_document_ids = list(dict.fromkeys(doc["id"] for doc in selection["documents"]))
    for profile in selection["profiles"].values():
        profile["documentIds"] = all_document_ids
    selection_bytes = json_bytes(selection)
    selection_path.write_bytes(selection_bytes)

    acq_by_version = {row.get("version"): index for index, row in enumerate(acquisition.get("sources", []))}
    for row in new_acq:
        if row["version"] in acq_by_version:
            acquisition["sources"][acq_by_version[row["version"]]] = row
        else:
            acquisition.setdefault("sources", []).append(row)
            acq_by_version[row["version"]] = len(acquisition["sources"]) - 1
    acquisition_path.write_bytes(json_bytes(acquisition))

    library_map = {"schema_version": "retained_library_map_v1", "sourceCatalog": "legacy-retained-export/access/web/antiquity/books/catalog.json", "books": map_books}
    (REPO / "access/public-corpus/retained-library-map.v1.json").write_bytes(json_bytes(library_map))
    names: dict[str, list[str]] = {}
    for book in catalog:
        entity = entity_by_legacy.get(book["id"])
        if not entity:
            raise SystemExit(f"no work entity for retained legacy id {book['id']}")
        title = entity.get("title") or {}
        names[new_work_id(book)] = [title.get("ru", book.get("title", book["id"])), title.get("en", book.get("title", book["id"]))]
    (REPO / "access/web/src/observatory/retained-work-names.json").write_bytes(json_bytes(names))
    print(json.dumps({"mode": "apply", "books": len(map_books), "versions": len(rows), "workRecords": len(work_record_written), "sourceBytes": estimated_output, "documents": len(selection["documents"]), "selectionVersions": sum(len(x.get("versions", [])) for x in selection["documents"]), "acquisitionRows": len(acquisition["sources"]), "mapping": mapping}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, ValueError, KeyError, json.JSONDecodeError) as exc:
        print(f"prepare_retained_corpus: {exc}", file=sys.stderr)
        raise SystemExit(2)
