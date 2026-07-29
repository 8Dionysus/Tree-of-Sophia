#!/usr/bin/env python3
"""Build the source-gated Zarathustra lexical observation index.

The local SQLite projection contains source-bearing token order and page text
and must stay below the ignored local-content boundary. The tracked projection
contains only form hashes, aggregate counts, and text-free TEI resource
references. Neither output accepts German, creates a lexeme/lemma/sign, or
opens semantic work.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
import tempfile
import unicodedata
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = Path("ToS/source-witnesses")
DEFAULT_PLAN = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/lexical-indexes/"
    "dta-first-editions-parts-1-4-v1/index-plan.v1.json"
)
PLAN_SCHEMA = Path("ToS/contracts/lexical-index-plan.schema.json")
PROJECTION_SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "lexical-index-projection.schema.json"
)
GENERATOR_REF = "scripts/build_zarathustra_lexical_index.py"
AUTHORITY_BOUNDARY = (
    "mechanical source-observation and rebuildable local search only; no "
    "accepted German, rights clearance, lexeme, lemma, translation, sign, "
    "concept, claim, relation, graph, canon, or publication authority"
)
TEI_NS = "http://www.tei-c.org/ns/1.0"


class LexicalIndexBuildError(RuntimeError):
    """Raised when exact inputs cannot produce the bounded projection."""


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise LexicalIndexBuildError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise LexicalIndexBuildError(f"{path} must contain a JSON object")
    return payload


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as exc:
        raise LexicalIndexBuildError(f"cannot hash {path}: {exc}") from exc
    return digest.hexdigest()


def _canonical_json_bytes(payload: object) -> bytes:
    return (
        json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("utf-8")


def _resolve_repo_path(repo_root: Path, relative: str) -> Path:
    candidate = (repo_root / relative).resolve()
    try:
        candidate.relative_to(repo_root.resolve())
    except ValueError as exc:
        raise LexicalIndexBuildError(
            f"repo-relative path escapes root: {relative}"
        ) from exc
    return candidate


def _local_name(tag: object) -> str:
    if not isinstance(tag, str):
        return ""
    return tag.rsplit("}", 1)[-1]


def _element_paths(root: ET.Element) -> dict[int, str]:
    root_name = _local_name(root.tag)
    paths: dict[int, str] = {id(root): root_name}

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


def _is_letter_or_mark(character: str) -> bool:
    return unicodedata.category(character).startswith(("L", "M"))


def iter_word_spans(text: str, joiners: set[str]) -> Iterable[tuple[int, int, str]]:
    """Yield exact Unicode word spans without mutating the source string."""

    start: int | None = None
    index = 0
    while index < len(text):
        character = text[index]
        if _is_letter_or_mark(character):
            if start is None:
                start = index
            index += 1
            continue
        if (
            character in joiners
            and start is not None
            and index + 1 < len(text)
            and _is_letter_or_mark(text[index + 1])
        ):
            index += 1
            continue
        if start is not None:
            yield start, index, text[start:index]
            start = None
        index += 1
    if start is not None:
        yield start, len(text), text[start:]


def normalize_form(exact_form: str) -> str:
    """Return the declared search-only normalization."""

    return unicodedata.normalize("NFC", exact_form).casefold()


def _select_choice_child(element: ET.Element) -> ET.Element | None:
    children = [child for child in list(element) if _local_name(child.tag)]
    for preferred in ("sic", "orig", "abbr"):
        for child in children:
            if _local_name(child.tag) == preferred:
                return child
    return children[0] if children else None


def _inventory_maps(
    inventory: dict[str, Any],
    *,
    file_id: str,
    file_sha256: str,
) -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, Any]]]:
    matching = [
        entry
        for entry in inventory.get("files", [])
        if isinstance(entry, dict)
        and entry.get("file_id") == file_id
        and entry.get("file_sha256") == file_sha256
    ]
    if len(matching) != 1:
        raise LexicalIndexBuildError(
            "resource inventory does not resolve one exact source file"
        )
    if matching[0].get("profile") != "tei_structure_v1":
        raise LexicalIndexBuildError("source file lacks a TEI structure inventory")
    pages: dict[str, dict[str, Any]] = {}
    sections: dict[str, dict[str, Any]] = {}
    for resource in matching[0].get("resources", []):
        if not isinstance(resource, dict):
            continue
        locator = resource.get("locator")
        if not isinstance(locator, dict) or not isinstance(locator.get("tei_path"), str):
            continue
        if resource.get("resource_kind") == "tei_page_break":
            pages[locator["tei_path"]] = resource
        elif resource.get("resource_kind") == "tei_division":
            sections[locator["tei_path"]] = resource
    if not pages or not sections:
        raise LexicalIndexBuildError("TEI inventory has no page or division resources")
    return pages, sections


def _payload_path(
    *,
    manifest_ref: str,
    payload_relative_path: str,
    payload_source_root: Path,
) -> Path:
    manifest_path = Path(manifest_ref)
    try:
        relative_manifest = manifest_path.relative_to(SOURCE_ROOT)
    except ValueError as exc:
        raise LexicalIndexBuildError(
            f"manifest is outside source-witnesses: {manifest_ref}"
        ) from exc
    candidate = (
        payload_source_root / relative_manifest.parent / payload_relative_path
    ).resolve()
    try:
        candidate.relative_to(payload_source_root.resolve())
    except ValueError as exc:
        raise LexicalIndexBuildError("payload path escapes explicit source root") from exc
    return candidate


def _parse_source_item(
    plan_item: dict[str, Any],
    *,
    repo_root: Path,
    payload_source_root: Path,
    excluded_elements: set[str],
    joiners: set[str],
) -> dict[str, Any]:
    manifest_path = _resolve_repo_path(repo_root, plan_item["manifest_ref"])
    inventory_path = _resolve_repo_path(
        repo_root, plan_item["resource_inventory_ref"]
    )
    rights_path = _resolve_repo_path(repo_root, plan_item["rights_ref"])
    manifest = _load_json(manifest_path)
    inventory = _load_json(inventory_path)
    rights = _load_json(rights_path)

    if manifest.get("item_id") != plan_item["item_ref"]:
        raise LexicalIndexBuildError(f"item identity drift: {manifest_path}")
    payload_entries = [
        entry
        for entry in manifest.get("payload_files", [])
        if isinstance(entry, dict)
        and entry.get("file_id") == plan_item["file_id"]
        and entry.get("sha256") == plan_item["file_sha256"]
    ]
    if len(payload_entries) != 1:
        raise LexicalIndexBuildError(f"exact file is absent from {manifest_path}")
    payload_entry = payload_entries[0]
    if payload_entry.get("media_type") != "application/xml":
        raise LexicalIndexBuildError("lexical v1 accepts exact XML payloads only")
    payload_path = _payload_path(
        manifest_ref=plan_item["manifest_ref"],
        payload_relative_path=payload_entry["relative_path"],
        payload_source_root=payload_source_root,
    )
    if not payload_path.is_file():
        raise LexicalIndexBuildError(f"local payload is absent: {payload_path}")
    if payload_path.stat().st_size != payload_entry.get("byte_size"):
        raise LexicalIndexBuildError(f"payload byte-size drift: {payload_path}")
    if _sha256_file(payload_path) != plan_item["file_sha256"]:
        raise LexicalIndexBuildError(f"payload digest drift: {payload_path}")
    if inventory.get("item_id") != plan_item["item_ref"]:
        raise LexicalIndexBuildError(f"inventory item drift: {inventory_path}")
    if plan_item["item_ref"] not in rights.get("scope_refs", []):
        raise LexicalIndexBuildError(f"rights scope omits item: {rights_path}")

    page_resources, section_resources = _inventory_maps(
        inventory,
        file_id=plan_item["file_id"],
        file_sha256=plan_item["file_sha256"],
    )

    try:
        root = ET.parse(payload_path).getroot()
    except (ET.ParseError, OSError) as exc:
        raise LexicalIndexBuildError(f"cannot parse {payload_path}: {exc}") from exc
    if root.tag != f"{{{TEI_NS}}}TEI":
        raise LexicalIndexBuildError(f"unsupported TEI namespace: {payload_path}")
    body = root.find(f".//{{{TEI_NS}}}body")
    if body is None:
        raise LexicalIndexBuildError(f"TEI body is absent: {payload_path}")
    paths = _element_paths(root)
    if id(body) not in paths:
        raise LexicalIndexBuildError("TEI body path could not be resolved")

    occurrences: list[dict[str, Any]] = []
    body_pages: dict[str, dict[str, Any]] = {}
    body_sections: dict[str, dict[str, Any]] = {}
    page_tokens: defaultdict[str, list[dict[str, str]]] = defaultdict(list)
    page_section_refs: defaultdict[str, set[str]] = defaultdict(set)
    occurrence_ids: set[str] = set()
    token_ordinal = 0

    def add_text(
        text: str | None,
        *,
        node_path: str,
        page_resource_id: str | None,
        section_stack: tuple[str, ...],
        editorial_status: str,
    ) -> None:
        nonlocal token_ordinal
        if not text:
            return
        for start, end, exact_form in iter_word_spans(text, joiners):
            if page_resource_id is None:
                raise LexicalIndexBuildError(
                    f"indexed body token has no preceding page anchor: {node_path}"
                )
            token_ordinal += 1
            normalized_form = normalize_form(exact_form)
            locator_digest = _sha256_bytes(
                "\x1f".join(
                    [
                        plan_item["file_sha256"],
                        node_path,
                        str(start),
                        str(end),
                        exact_form,
                    ]
                ).encode("utf-8")
            )
            occurrence_id = f"tos.occurrence.lexical-zarathustra-dta-v1.{locator_digest}"
            if occurrence_id in occurrence_ids:
                raise LexicalIndexBuildError("occurrence locator collision")
            occurrence_ids.add(occurrence_id)
            section_resource_id = section_stack[-1] if section_stack else None
            occurrence = {
                "occurrence_id": occurrence_id,
                "item_ref": plan_item["item_ref"],
                "file_id": plan_item["file_id"],
                "file_sha256": plan_item["file_sha256"],
                "token_ordinal": token_ordinal,
                "exact_form": exact_form,
                "normalized_form": normalized_form,
                "exact_form_sha256": _sha256_bytes(exact_form.encode("utf-8")),
                "normalized_form_sha256": _sha256_bytes(
                    normalized_form.encode("utf-8")
                ),
                "page_resource_id": page_resource_id,
                "section_resource_id": section_resource_id,
                "text_node_path": node_path,
                "start_offset": start,
                "end_offset": end,
                "editorial_status": editorial_status,
            }
            occurrences.append(occurrence)
            page_tokens[page_resource_id].append(
                {"exact": exact_form, "normalized": normalized_form}
            )
            page_section_refs[page_resource_id].update(section_stack)

    def visit(
        element: ET.Element,
        *,
        current_page: str | None,
        section_stack: tuple[str, ...],
        editorial_status: str,
    ) -> str | None:
        tag = _local_name(element.tag)
        path = paths[id(element)]
        if tag == "pb":
            resource = page_resources.get(path)
            if resource is None:
                raise LexicalIndexBuildError(
                    f"TEI page break is absent from tracked inventory: {path}"
                )
            current_page = resource["resource_id"]
            body_pages[current_page] = resource
        if tag == "div":
            resource = section_resources.get(path)
            if resource is None:
                raise LexicalIndexBuildError(
                    f"TEI division is absent from tracked inventory: {path}"
                )
            body_sections[resource["resource_id"]] = resource
            section_stack = (*section_stack, resource["resource_id"])
        if tag in excluded_elements:
            return current_page
        if tag == "choice":
            selected = _select_choice_child(element)
            if selected is not None:
                selected_status = editorial_status
                selected_tag = _local_name(selected.tag)
                if selected_tag == "sic":
                    selected_status = "source-marked-sic"
                elif selected_tag in {"orig", "abbr"}:
                    selected_status = f"source-marked-{selected_tag}"
                current_page = visit(
                    selected,
                    current_page=current_page,
                    section_stack=section_stack,
                    editorial_status=selected_status,
                )
            return current_page
        if tag == "supplied":
            editorial_status = "source-editorial-supplied"
        elif tag == "corr":
            editorial_status = "source-editorial-corr"
        elif tag == "sic":
            editorial_status = "source-marked-sic"

        add_text(
            element.text,
            node_path=f"{path}/text()[1]",
            page_resource_id=current_page,
            section_stack=section_stack,
            editorial_status=editorial_status,
        )
        for child in list(element):
            child_name = _local_name(child.tag)
            if not child_name:
                continue
            current_page = visit(
                child,
                current_page=current_page,
                section_stack=section_stack,
                editorial_status=editorial_status,
            )
            add_text(
                child.tail,
                node_path=f"{paths[id(child)]}/tail()[1]",
                page_resource_id=current_page,
                section_stack=section_stack,
                editorial_status=editorial_status,
            )
        return current_page

    visit(
        body,
        current_page=None,
        section_stack=(),
        editorial_status="witness-text",
    )
    if not occurrences:
        raise LexicalIndexBuildError(f"TEI body produced no word tokens: {payload_path}")
    if not body_pages or not body_sections:
        raise LexicalIndexBuildError("indexed TEI body lacks pages or divisions")
    if any(
        occurrence["page_resource_id"] not in body_pages
        for occurrence in occurrences
    ):
        raise LexicalIndexBuildError("occurrence loses its page resource")

    pages: list[dict[str, Any]] = []
    for resource_id, resource in sorted(body_pages.items()):
        locator = resource["locator"]
        tokens = page_tokens.get(resource_id, [])
        pages.append(
            {
                "item_ref": plan_item["item_ref"],
                "resource_id": resource_id,
                "tei_path": locator["tei_path"],
                "facs_ref": locator.get("tei_facs_ref"),
                "page_label": locator.get("tei_page_label"),
                "exact_text": " ".join(token["exact"] for token in tokens),
                "normalized_text": " ".join(
                    token["normalized"] for token in tokens
                ),
                "section_refs": sorted(page_section_refs.get(resource_id, set())),
            }
        )
    sections: list[dict[str, Any]] = []
    for resource_id, resource in sorted(body_sections.items()):
        locator = resource["locator"]
        sections.append(
            {
                "item_ref": plan_item["item_ref"],
                "resource_id": resource_id,
                "tei_path": locator["tei_path"],
                "tei_depth": locator.get("tei_depth"),
                "page_label": locator.get("tei_page_label"),
                "tei_n": locator.get("tei_n"),
                "tei_type": locator.get("tei_type"),
                "parent_resource_id": locator.get("parent_resource_id"),
            }
        )

    receipt = {
        "part_order": plan_item["part_order"],
        "item_ref": plan_item["item_ref"],
        "file_id": plan_item["file_id"],
        "file_sha256": plan_item["file_sha256"],
        "manifest_ref": plan_item["manifest_ref"],
        "resource_inventory_ref": plan_item["resource_inventory_ref"],
        "rights_ref": plan_item["rights_ref"],
        "rights_assessment_status": rights.get("assessment_status"),
        "rights_review_status": rights.get("review_status"),
        "language": plan_item["language"],
        "edition_ref": manifest.get("embodiment_ref"),
        "body_page_count": len(pages),
        "section_count": len(sections),
        "token_occurrence_count": len(occurrences),
    }
    if not isinstance(receipt["edition_ref"], str):
        raise LexicalIndexBuildError(f"item has no embodiment ref: {manifest_path}")
    if not isinstance(receipt["rights_assessment_status"], str) or not isinstance(
        receipt["rights_review_status"], str
    ):
        raise LexicalIndexBuildError(f"rights state is incomplete: {rights_path}")
    return {
        "receipt": receipt,
        "occurrences": occurrences,
        "pages": pages,
        "sections": sections,
    }


def _create_local_database(
    database_path: Path,
    source_results: list[dict[str, Any]],
    *,
    plan_id: str,
    plan_sha256: str,
) -> dict[str, Any]:
    connection = sqlite3.connect(database_path)
    try:
        connection.execute("PRAGMA page_size=4096")
        connection.execute("PRAGMA journal_mode=OFF")
        connection.execute("PRAGMA synchronous=OFF")
        connection.execute("PRAGMA temp_store=MEMORY")
        connection.execute(
            """
            CREATE TABLE metadata(
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL
            ) WITHOUT ROWID
            """
        )
        connection.execute(
            """
            CREATE TABLE source_items(
              item_ref TEXT PRIMARY KEY,
              part_order INTEGER NOT NULL,
              file_id TEXT NOT NULL,
              file_sha256 TEXT NOT NULL,
              language TEXT NOT NULL,
              edition_ref TEXT NOT NULL,
              manifest_ref TEXT NOT NULL,
              resource_inventory_ref TEXT NOT NULL,
              rights_ref TEXT NOT NULL
            ) WITHOUT ROWID
            """
        )
        connection.execute(
            """
            CREATE TABLE pages(
              item_ref TEXT NOT NULL,
              resource_id TEXT NOT NULL,
              tei_path TEXT NOT NULL,
              facs_ref TEXT,
              page_label TEXT,
              PRIMARY KEY(item_ref, resource_id)
            ) WITHOUT ROWID
            """
        )
        connection.execute(
            """
            CREATE TABLE sections(
              item_ref TEXT NOT NULL,
              resource_id TEXT NOT NULL,
              tei_path TEXT NOT NULL,
              tei_depth INTEGER,
              page_label TEXT,
              tei_n TEXT,
              tei_type TEXT,
              parent_resource_id TEXT,
              PRIMARY KEY(item_ref, resource_id)
            ) WITHOUT ROWID
            """
        )
        connection.execute(
            """
            CREATE TABLE forms(
              form_key TEXT PRIMARY KEY,
              exact_form TEXT NOT NULL,
              normalized_form TEXT NOT NULL,
              exact_form_sha256 TEXT NOT NULL,
              normalized_form_sha256 TEXT NOT NULL,
              occurrence_count INTEGER NOT NULL
            ) WITHOUT ROWID
            """
        )
        connection.execute(
            """
            CREATE TABLE occurrences(
              occurrence_id TEXT PRIMARY KEY,
              item_ref TEXT NOT NULL,
              token_ordinal INTEGER NOT NULL,
              form_key TEXT NOT NULL,
              exact_form TEXT NOT NULL,
              normalized_form TEXT NOT NULL,
              exact_form_sha256 TEXT NOT NULL,
              normalized_form_sha256 TEXT NOT NULL,
              page_resource_id TEXT NOT NULL,
              section_resource_id TEXT,
              text_node_path TEXT NOT NULL,
              start_offset INTEGER NOT NULL,
              end_offset INTEGER NOT NULL,
              editorial_status TEXT NOT NULL
            ) WITHOUT ROWID
            """
        )
        connection.execute(
            "CREATE INDEX occurrences_exact_idx ON occurrences(exact_form)"
        )
        connection.execute(
            "CREATE INDEX occurrences_normalized_idx ON occurrences(normalized_form)"
        )
        connection.execute(
            "CREATE INDEX occurrences_page_idx ON occurrences(item_ref, page_resource_id)"
        )
        connection.execute(
            "CREATE INDEX occurrences_section_idx ON occurrences(item_ref, section_resource_id)"
        )
        connection.execute(
            """
            CREATE VIRTUAL TABLE page_fts USING fts5(
              item_ref UNINDEXED,
              page_resource_id UNINDEXED,
              section_refs UNINDEXED,
              exact_text,
              normalized_text,
              lemma,
              phrase,
              prefix,
              section,
              page,
              language,
              edition,
              translation,
              sign_candidate,
              tokenize='unicode61 remove_diacritics 0'
            )
            """
        )
        connection.executemany(
            "INSERT INTO metadata(key, value) VALUES (?, ?)",
            [
                ("plan_id", plan_id),
                ("plan_sha256", plan_sha256),
                ("authority_boundary", AUTHORITY_BOUNDARY),
            ],
        )

        all_occurrences: list[dict[str, Any]] = []
        for result in sorted(
            source_results, key=lambda entry: entry["receipt"]["part_order"]
        ):
            receipt = result["receipt"]
            connection.execute(
                """
                INSERT INTO source_items(
                  item_ref, part_order, file_id, file_sha256, language,
                  edition_ref, manifest_ref, resource_inventory_ref, rights_ref
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    receipt["item_ref"],
                    receipt["part_order"],
                    receipt["file_id"],
                    receipt["file_sha256"],
                    receipt["language"],
                    receipt["edition_ref"],
                    receipt["manifest_ref"],
                    receipt["resource_inventory_ref"],
                    receipt["rights_ref"],
                ),
            )
            for page in result["pages"]:
                connection.execute(
                    """
                    INSERT INTO pages(
                      item_ref, resource_id, tei_path, facs_ref, page_label
                    ) VALUES (?, ?, ?, ?, ?)
                    """,
                    (
                        page["item_ref"],
                        page["resource_id"],
                        page["tei_path"],
                        page["facs_ref"],
                        page["page_label"],
                    ),
                )
                connection.execute(
                    """
                    INSERT INTO page_fts(
                      item_ref, page_resource_id, section_refs,
                      exact_text, normalized_text, lemma, phrase, prefix,
                      section, page, language, edition, translation,
                      sign_candidate
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        page["item_ref"],
                        page["resource_id"],
                        " ".join(page["section_refs"]),
                        page["exact_text"],
                        page["normalized_text"],
                        "",
                        page["normalized_text"],
                        page["normalized_text"],
                        " ".join(page["section_refs"]),
                        page["page_label"] or page["resource_id"],
                        receipt["language"],
                        receipt["edition_ref"],
                        "",
                        "",
                    ),
                )
            for section in result["sections"]:
                connection.execute(
                    """
                    INSERT INTO sections(
                      item_ref, resource_id, tei_path, tei_depth, page_label,
                      tei_n, tei_type, parent_resource_id
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        section["item_ref"],
                        section["resource_id"],
                        section["tei_path"],
                        section["tei_depth"],
                        section["page_label"],
                        section["tei_n"],
                        section["tei_type"],
                        section["parent_resource_id"],
                    ),
                )
            all_occurrences.extend(result["occurrences"])

        form_counts = Counter(
            occurrence["exact_form"] for occurrence in all_occurrences
        )
        exact_hashes: dict[str, str] = {}
        for exact_form in sorted(form_counts):
            exact_hash = _sha256_bytes(exact_form.encode("utf-8"))
            previous = exact_hashes.get(exact_hash)
            if previous is not None and previous != exact_form:
                raise LexicalIndexBuildError("exact-form SHA-256 collision")
            exact_hashes[exact_hash] = exact_form
            normalized = normalize_form(exact_form)
            connection.execute(
                """
                INSERT INTO forms(
                  form_key, exact_form, normalized_form, exact_form_sha256,
                  normalized_form_sha256, occurrence_count
                ) VALUES (?, ?, ?, ?, ?, ?)
                """,
                (
                    f"lexical-form:sha256:{exact_hash}",
                    exact_form,
                    normalized,
                    exact_hash,
                    _sha256_bytes(normalized.encode("utf-8")),
                    form_counts[exact_form],
                ),
            )
        for occurrence in sorted(
            all_occurrences,
            key=lambda entry: (entry["item_ref"], entry["token_ordinal"]),
        ):
            connection.execute(
                """
                INSERT INTO occurrences(
                  occurrence_id, item_ref, token_ordinal, form_key, exact_form,
                  normalized_form, exact_form_sha256, normalized_form_sha256,
                  page_resource_id, section_resource_id, text_node_path,
                  start_offset, end_offset, editorial_status
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    occurrence["occurrence_id"],
                    occurrence["item_ref"],
                    occurrence["token_ordinal"],
                    f"lexical-form:sha256:{occurrence['exact_form_sha256']}",
                    occurrence["exact_form"],
                    occurrence["normalized_form"],
                    occurrence["exact_form_sha256"],
                    occurrence["normalized_form_sha256"],
                    occurrence["page_resource_id"],
                    occurrence["section_resource_id"],
                    occurrence["text_node_path"],
                    occurrence["start_offset"],
                    occurrence["end_offset"],
                    occurrence["editorial_status"],
                ),
            )
        connection.commit()

        first = connection.execute(
            """
            SELECT exact_form, normalized_form, item_ref, page_resource_id,
                   section_resource_id
              FROM occurrences
             ORDER BY item_ref, token_ordinal
             LIMIT 1
            """
        ).fetchone()
        if first is None:
            raise LexicalIndexBuildError("local occurrence table is empty")
        exact_count = connection.execute(
            "SELECT count(*) FROM occurrences WHERE exact_form = ?", (first[0],)
        ).fetchone()[0]
        normalized_count = connection.execute(
            "SELECT count(*) FROM occurrences WHERE normalized_form = ?",
            (first[1],),
        ).fetchone()[0]
        prefix_length = min(3, len(first[1]))
        prefix_count = connection.execute(
            "SELECT count(*) FROM occurrences WHERE normalized_form LIKE ?",
            (first[1][:prefix_length] + "%",),
        ).fetchone()[0]
        section_probe = connection.execute(
            "SELECT section_resource_id FROM occurrences "
            "WHERE section_resource_id IS NOT NULL LIMIT 1"
        ).fetchone()
        if section_probe is None:
            raise LexicalIndexBuildError("local index has no section-bound token")
        section_count = connection.execute(
            "SELECT count(*) FROM occurrences WHERE section_resource_id = ?",
            (section_probe[0],),
        ).fetchone()[0]
        page_count = connection.execute(
            """
            SELECT count(*) FROM occurrences
             WHERE item_ref = ? AND page_resource_id = ?
            """,
            (first[2], first[3]),
        ).fetchone()[0]
        language_count = connection.execute(
            "SELECT count(*) FROM occurrences o "
            "JOIN source_items s USING(item_ref) WHERE s.language = 'de'"
        ).fetchone()[0]
        edition_count = connection.execute(
            "SELECT count(*) FROM occurrences WHERE item_ref = ?", (first[2],)
        ).fetchone()[0]
        phrase_row = connection.execute(
            """
            SELECT normalized_text
              FROM page_fts
             WHERE length(normalized_text) > 0
             ORDER BY item_ref, page_resource_id
            """
        ).fetchone()
        if phrase_row is None or len(phrase_row[0].split()) < 2:
            raise LexicalIndexBuildError("local index has no two-token page phrase")
        phrase_tokens = phrase_row[0].split()[:2]
        phrase = " ".join(token.replace('"', '""') for token in phrase_tokens)
        phrase_count = connection.execute(
            "SELECT count(*) FROM page_fts "
            "WHERE page_fts MATCH ?",
            (f'normalized_text : "{phrase}"',),
        ).fetchone()[0]
        probes = {
            "exact_form": {"status": "passed", "result_count": exact_count},
            "normalized_form": {
                "status": "passed",
                "result_count": normalized_count,
            },
            "prefix": {"status": "passed", "result_count": prefix_count},
            "phrase": {"status": "passed", "result_count": phrase_count},
            "section": {"status": "passed", "result_count": section_count},
            "page": {"status": "passed", "result_count": page_count},
            "language": {"status": "passed", "result_count": language_count},
            "edition": {"status": "passed", "result_count": edition_count},
            "lemma": {
                "status": "blocked-not-materialized",
                "result_count": 0,
                "blocker_refs": [
                    "german-language-competence-not-attested",
                    "morphology-method-not-compared",
                ],
            },
            "translation": {"status": "not-applicable", "result_count": 0},
            "sign_candidate": {
                "status": "blocked-not-materialized",
                "result_count": 0,
                "blocker_refs": [
                    "task-specific-source-gate-not-satisfied",
                    "human-sign-review-not-triggered",
                ],
            },
        }
        if any(
            probe["status"] == "passed" and not probe["result_count"]
            for probe in probes.values()
        ):
            raise LexicalIndexBuildError("one or more local query probes returned zero")

        table_counts = {
            table: connection.execute(f"SELECT count(*) FROM {table}").fetchone()[0]
            for table in ("source_items", "pages", "sections", "occurrences", "forms")
        }
        fts5_enabled = any(
            "ENABLE_FTS5" in row[0]
            for row in connection.execute("PRAGMA compile_options").fetchall()
        )
        if not fts5_enabled:
            raise LexicalIndexBuildError("SQLite runtime does not report FTS5")
        connection.execute("VACUUM")
        return {
            "sqlite_version": sqlite3.sqlite_version,
            "fts5_enabled": True,
            "table_counts": table_counts,
            "query_probes": probes,
        }
    except sqlite3.Error as exc:
        raise LexicalIndexBuildError(f"SQLite build failed: {exc}") from exc
    finally:
        connection.close()


def _build_form_rows(source_results: list[dict[str, Any]]) -> list[dict[str, Any]]:
    forms: dict[str, dict[str, Any]] = {}
    exact_hash_values: dict[str, str] = {}
    part_order = {
        result["receipt"]["item_ref"]: result["receipt"]["part_order"]
        for result in source_results
    }
    for result in source_results:
        for occurrence in result["occurrences"]:
            exact_form = occurrence["exact_form"]
            exact_hash = occurrence["exact_form_sha256"]
            previous = exact_hash_values.get(exact_hash)
            if previous is not None and previous != exact_form:
                raise LexicalIndexBuildError("tracked exact-form hash collision")
            exact_hash_values[exact_hash] = exact_form
            aggregate = forms.setdefault(
                exact_form,
                {
                    "form_key": f"lexical-form:sha256:{exact_hash}",
                    "exact_form_sha256": exact_hash,
                    "normalized_form_sha256": occurrence[
                        "normalized_form_sha256"
                    ],
                    "occurrence_count": 0,
                    "source_editorial_occurrence_count": 0,
                    "unsectioned_occurrence_count": 0,
                    "items": defaultdict(
                        lambda: {
                            "occurrence_count": 0,
                            "pages": Counter(),
                            "sections": Counter(),
                        }
                    ),
                },
            )
            if (
                aggregate["normalized_form_sha256"]
                != occurrence["normalized_form_sha256"]
            ):
                raise LexicalIndexBuildError(
                    "one exact form produced multiple normalized hashes"
                )
            aggregate["occurrence_count"] += 1
            if occurrence["editorial_status"] != "witness-text":
                aggregate["source_editorial_occurrence_count"] += 1
            if occurrence["section_resource_id"] is None:
                aggregate["unsectioned_occurrence_count"] += 1
            item = aggregate["items"][occurrence["item_ref"]]
            item["occurrence_count"] += 1
            item["pages"][occurrence["page_resource_id"]] += 1
            if occurrence["section_resource_id"] is not None:
                item["sections"][occurrence["section_resource_id"]] += 1

    rows: list[dict[str, Any]] = []
    for exact_form, aggregate in forms.items():
        item_rows = []
        for item_ref, item in sorted(
            aggregate["items"].items(),
            key=lambda pair: part_order[pair[0]],
        ):
            item_rows.append(
                {
                    "item_ref": item_ref,
                    "occurrence_count": item["occurrence_count"],
                    "page_hits": [
                        {
                            "resource_id": resource_id,
                            "occurrence_count": count,
                        }
                        for resource_id, count in sorted(item["pages"].items())
                    ],
                    "section_hits": [
                        {
                            "resource_id": resource_id,
                            "occurrence_count": count,
                        }
                        for resource_id, count in sorted(
                            item["sections"].items()
                        )
                    ],
                }
            )
        rows.append(
            {
                "form_key": aggregate["form_key"],
                "exact_form_sha256": aggregate["exact_form_sha256"],
                "normalized_form_sha256": aggregate["normalized_form_sha256"],
                "occurrence_count": aggregate["occurrence_count"],
                "source_editorial_occurrence_count": aggregate[
                    "source_editorial_occurrence_count"
                ],
                "unsectioned_occurrence_count": aggregate[
                    "unsectioned_occurrence_count"
                ],
                "source_items": item_rows,
            }
        )
    rows.sort(key=lambda row: row["exact_form_sha256"])
    return rows


def build_projection(
    *,
    repo_root: Path,
    plan_path: Path,
    payload_source_root: Path,
    local_database_path: Path,
) -> tuple[dict[str, Any], Path]:
    plan = _load_json(plan_path)
    plan_schema = _load_json(repo_root / PLAN_SCHEMA)
    errors = sorted(
        Draft202012Validator(plan_schema).iter_errors(plan),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        raise LexicalIndexBuildError(
            "plan schema validation failed: "
            + "; ".join(error.message for error in errors[:5])
        )
    if plan["authority_boundary"] != AUTHORITY_BOUNDARY:
        raise LexicalIndexBuildError("plan authority boundary drift")
    if sorted(
        item["part_order"] for item in plan["source_items"]
    ) != list(range(1, len(plan["source_items"]) + 1)):
        raise LexicalIndexBuildError("source part order must be contiguous")

    source_results = [
        _parse_source_item(
            item,
            repo_root=repo_root,
            payload_source_root=payload_source_root,
            excluded_elements=set(plan["scope"]["excluded_elements"]),
            joiners=set(plan["tokenization"]["internal_joiners"]),
        )
        for item in sorted(plan["source_items"], key=lambda entry: entry["part_order"])
    ]
    plan_sha256 = _sha256_file(plan_path)
    local_database_path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix="tos-lexical-index-",
        suffix=".sqlite3",
        dir=local_database_path.parent,
    )
    os.close(descriptor)
    temporary_path = Path(temporary_name)
    try:
        local_receipt = _create_local_database(
            temporary_path,
            source_results,
            plan_id=plan["plan_id"],
            plan_sha256=plan_sha256,
        )
        local_receipt["relative_path"] = plan["local_projection"]["relative_path"]
        local_receipt["database_sha256"] = _sha256_file(temporary_path)
        local_receipt["database_bytes"] = temporary_path.stat().st_size

        form_rows = _build_form_rows(source_results)
        all_occurrences = [
            occurrence
            for result in source_results
            for occurrence in result["occurrences"]
        ]
        projection = {
            "$schema": PROJECTION_SCHEMA_REF,
            "schema_version": "tos_lexical_index_projection_v1",
            "generated_or_authored": "generated_from_source",
            "projection_id": (
                "lexical-index-projection:"
                "zarathustra-dta-first-editions-parts-1-4-v1"
            ),
            "plan_id": plan["plan_id"],
            "plan_ref": plan_path.relative_to(repo_root).as_posix(),
            "plan_sha256": plan_sha256,
            "generator_ref": GENERATOR_REF,
            "generator_sha256": _sha256_file(repo_root / GENERATOR_REF),
            "builder": {"surface": GENERATOR_REF},
            "work_ref": plan["work_ref"],
            "source_items": [
                result["receipt"]
                for result in sorted(
                    source_results,
                    key=lambda entry: entry["receipt"]["part_order"],
                )
            ],
            "summary": {
                "source_item_count": len(source_results),
                "body_page_count": sum(
                    len(result["pages"]) for result in source_results
                ),
                "section_count": sum(
                    len(result["sections"]) for result in source_results
                ),
                "token_occurrence_count": len(all_occurrences),
                "exact_form_row_count": len(form_rows),
                "normalized_form_hash_count": len(
                    {
                        occurrence["normalized_form_sha256"]
                        for occurrence in all_occurrences
                    }
                ),
                "source_editorial_occurrence_count": sum(
                    occurrence["editorial_status"] != "witness-text"
                    for occurrence in all_occurrences
                ),
                "unsectioned_occurrence_count": sum(
                    occurrence["section_resource_id"] is None
                    for occurrence in all_occurrences
                ),
                "semantic_fields_populated": 0,
            },
            "hash_profiles": {
                "exact_form": {
                    "algorithm": "sha256",
                    "input": "UTF-8 exact XML character sequence",
                    "normalization": "none",
                    "confidentiality": (
                        "none-low-entropy-dictionary-recovery-possible"
                    ),
                },
                "normalized_form": {
                    "algorithm": "sha256",
                    "input": "UTF-8 derived search key",
                    "normalization": "unicode-nfc-casefold",
                    "confidentiality": (
                        "none-low-entropy-dictionary-recovery-possible"
                    ),
                },
            },
            "field_posture": plan["field_posture"],
            "form_rows": form_rows,
            "local_projection_receipt": local_receipt,
            "content_exposure": {
                "tracked_exact_strings": False,
                "tracked_sequence": False,
                "tracked_context": False,
                "tracked_occurrence_positions": False,
                "tracked_form_hashes": True,
                "dictionary_recovery_possible": True,
                "confidentiality_claimed": False,
            },
            "rights_and_visibility": plan["rights_and_visibility"],
            "semantic_boundary": plan["semantic_boundary"],
            "authority_boundary": AUTHORITY_BOUNDARY,
        }
        return projection, temporary_path
    except Exception:
        temporary_path.unlink(missing_ok=True)
        raise


def _validate_projection_schema(repo_root: Path, projection: dict[str, Any]) -> None:
    schema = _load_json(repo_root / "ToS/contracts/lexical-index-projection.schema.json")
    errors = sorted(
        Draft202012Validator(schema).iter_errors(projection),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        raise LexicalIndexBuildError(
            "projection schema validation failed: "
            + "; ".join(error.message for error in errors[:8])
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--plan",
        type=Path,
        default=REPO_ROOT / DEFAULT_PLAN,
        help="tracked lexical index plan",
    )
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        required=True,
        help="explicit local ToS/source-witnesses root holding ignored payloads",
    )
    parser.add_argument(
        "--local-output-root",
        type=Path,
        required=True,
        help="explicit local repository root owning the ignored SQLite output",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="tracked projection path; defaults to the plan's tracked path",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare rebuilt projection and local database with current artifacts",
    )
    args = parser.parse_args()

    repo_root = REPO_ROOT.resolve()
    plan_path = args.plan.resolve()
    plan = _load_json(plan_path)
    output_path = (
        args.output.resolve()
        if args.output
        else _resolve_repo_path(repo_root, plan["tracked_projection"]["relative_path"])
    )
    expected_output = _resolve_repo_path(
        repo_root, plan["tracked_projection"]["relative_path"]
    )
    if output_path != expected_output:
        raise LexicalIndexBuildError(
            "output must remain at the tracked path declared by the plan"
        )
    local_output_root = args.local_output_root.resolve()
    local_database_path = _resolve_repo_path(
        local_output_root, plan["local_projection"]["relative_path"]
    )
    if "local-content" not in local_database_path.parts:
        raise LexicalIndexBuildError(
            "source-bearing SQLite output must remain below local-content"
        )

    projection, temporary_database = build_projection(
        repo_root=repo_root,
        plan_path=plan_path,
        payload_source_root=args.payload_source_root.resolve(),
        local_database_path=local_database_path,
    )
    try:
        _validate_projection_schema(repo_root, projection)
        expected_bytes = _canonical_json_bytes(projection)
        if args.check:
            if not output_path.is_file():
                raise LexicalIndexBuildError(
                    f"tracked projection is absent: {output_path}"
                )
            if output_path.read_bytes() != expected_bytes:
                raise LexicalIndexBuildError(
                    "tracked lexical projection differs from deterministic rebuild"
                )
            if not local_database_path.is_file():
                raise LexicalIndexBuildError(
                    f"local lexical database is absent: {local_database_path}"
                )
            if _sha256_file(local_database_path) != _sha256_file(temporary_database):
                raise LexicalIndexBuildError(
                    "local lexical database differs from deterministic rebuild"
                )
            print(
                json.dumps(
                    {
                        "status": "ok",
                        "mode": "check",
                        "projection": output_path.relative_to(repo_root).as_posix(),
                        "local_database_sha256": _sha256_file(local_database_path),
                        "summary": projection["summary"],
                        "authority_boundary": AUTHORITY_BOUNDARY,
                    },
                    ensure_ascii=False,
                    sort_keys=True,
                )
            )
            return 0

        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_bytes(expected_bytes)
        local_database_path.parent.mkdir(parents=True, exist_ok=True)
        os.replace(temporary_database, local_database_path)
        print(
            json.dumps(
                {
                    "status": "written",
                    "projection": output_path.relative_to(repo_root).as_posix(),
                    "local_database": plan["local_projection"]["relative_path"],
                    "local_database_sha256": _sha256_file(local_database_path),
                    "summary": projection["summary"],
                    "authority_boundary": AUTHORITY_BOUNDARY,
                },
                ensure_ascii=False,
                sort_keys=True,
            )
        )
        return 0
    finally:
        temporary_database.unlink(missing_ok=True)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except LexicalIndexBuildError as exc:
        raise SystemExit(f"lexical index build failed: {exc}") from exc
