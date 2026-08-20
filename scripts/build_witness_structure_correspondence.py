#!/usr/bin/env python3
"""Build a text-free structural correspondence candidate for Zarathustra witnesses.

The builder reads four local DTA TEIs and the local Naumann 1893 EPUB, then
matches named primary division starts to the 1893 scan-page sequence. It emits
only source-owned locators, one-way fingerprints, and mechanical match metrics.
It does not emit source text or establish textual, linguistic, translation,
semantic, or canon authority.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
import unicodedata
import xml.etree.ElementTree as ET
import zipfile
from collections import Counter
from dataclasses import dataclass
from datetime import datetime
from html.parser import HTMLParser
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = Path("ToS/source-witnesses")
WORK_ROOT = (
    SOURCE_ROOT
    / "works/friedrich-nietzsche/also-sprach-zarathustra"
)
OUTPUT_DIR = WORK_ROOT / "alignments/structure/naumann-1893-dta-parts"
OUTPUT_PATH = OUTPUT_DIR / "structure-correspondence.json"
ANCHOR_SET_PATH = OUTPUT_DIR / "structure-anchor-set.json"
ANCHOR_RECORDS_PATH = OUTPUT_DIR / "structure-anchors.jsonl"
PROVENANCE_PATH = OUTPUT_DIR / "provenance.jsonl"
SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "witness-structure-correspondence.schema.json"
)
ANCHOR_SET_SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "witness-structure-anchor-set.schema.json"
)
MAP_ID = (
    "tos.structure-map.friedrich-nietzsche.also-sprach-zarathustra."
    "dta-parts-to-naumann-1893"
)
ANCHOR_SET_ID = (
    "tos.structure-anchor-set.friedrich-nietzsche.also-sprach-zarathustra."
    "dta-parts-to-naumann-1893"
)
EVENT_ID = (
    "tos.event.structure-correspondence.friedrich-nietzsche."
    "also-sprach-zarathustra.dta-parts-to-naumann-1893.2026-07-28"
)
ANCHOR_EVENT_ID = (
    "tos.event.structure-anchors.friedrich-nietzsche."
    "also-sprach-zarathustra.dta-parts-to-naumann-1893.2026-07-28"
)
WORK_REF = "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
AUTHORITY_BOUNDARY = (
    "named structural starts and locator candidates only; no source text, "
    "textual identity, edition equivalence, accepted German, translation, "
    "semantics, or canon authority"
)
DOES_NOT_ESTABLISH = [
    "source_text",
    "exact_textual_identity",
    "edition_equivalence",
    "accepted_german",
    "translation_correspondence",
    "semantic_correspondence",
    "canon_promotion",
]
ANCHOR_AUTHORITY_BOUNDARY = (
    "stable proposed addresses for named structural-start candidates only; "
    "no source text, exact passage boundary, textual identity, edition "
    "equivalence, accepted German, translation, semantics, rights clearance, "
    "or canon authority"
)
ANCHOR_DOES_NOT_ESTABLISH = [
    "source_text",
    "exact_passage_boundary",
    "exact_textual_identity",
    "edition_equivalence",
    "accepted_german",
    "translation_correspondence",
    "semantic_correspondence",
    "rights_clearance",
    "canon_promotion",
]
NORMALIZATION = "unicode-nfkc-casefold-alpha-token-sequence"
SOURCE_CONTEXT_TOKEN_LIMIT = 160
TARGET_WINDOW_PAGE_COUNT = 2
CONTEXT_WEIGHT = 0.8
HEADING_WEIGHT = 0.2
PAGE_MEMBER_RE = re.compile(r"^EPUB/page_(\d+)\.html$")


class StructureBuildError(RuntimeError):
    """Raised when the fixed witness route cannot be built honestly."""


@dataclass(frozen=True)
class PartSpec:
    part_label: str
    manifest_ref: str
    first_target_page: int
    last_target_page: int
    selection_policy: str


PART_SPECS = (
    PartSpec(
        "I",
        (
            "ToS/source-witnesses/works/friedrich-nietzsche/"
            "also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/"
            "editions/chemnitz-schmeitzner-1883-part-1/items/"
            "dta-sbb-corrected-tei-p5/item.manifest.json"
        ),
        53,
        162,
        (
            "body depth-one named containers plus named depth-two divisions "
            "under body div[2]; contents and numbered prologue subdivisions excluded"
        ),
    ),
    PartSpec(
        "II",
        (
            "ToS/source-witnesses/works/friedrich-nietzsche/"
            "also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-2/"
            "editions/chemnitz-schmeitzner-1883-part-2/items/"
            "dta-sbb-corrected-tei-p5/item.manifest.json"
        ),
        165,
        265,
        "named body depth-one divisions; contents excluded",
    ),
    PartSpec(
        "III",
        (
            "ToS/source-witnesses/works/friedrich-nietzsche/"
            "also-sprach-zarathustra/expressions/de-schmeitzner-1884-part-3/"
            "editions/chemnitz-schmeitzner-1884-part-3/items/"
            "dta-sbb-corrected-tei-p5/item.manifest.json"
        ),
        269,
        385,
        "named body depth-one divisions; contents excluded",
    ),
    PartSpec(
        "IV",
        (
            "ToS/source-witnesses/works/friedrich-nietzsche/"
            "also-sprach-zarathustra/expressions/de-naumann-1891-part-4/"
            "editions/leipzig-naumann-1891-part-4/items/"
            "dta-sub-goettingen-corrected-tei-p5/item.manifest.json"
        ),
        389,
        522,
        (
            "named body depth-one divisions through div[20]; div[21] auxiliary "
            "verse sequence, contents, and back matter excluded"
        ),
    ),
)

TARGET_EPUB_MANIFEST_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/de-naumann-1893/"
    "editions/leipzig-c-g-naumann-1893/items/"
    "internet-archive-cornell-auto-epub/item.manifest.json"
)
TARGET_PDF_MANIFEST_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/de-naumann-1893/"
    "editions/leipzig-c-g-naumann-1893/items/"
    "internet-archive-image-container-pdf/item.manifest.json"
)


class _VisibleTextParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: list[str] = []
        self.hidden_depth = 0

    def handle_starttag(
        self,
        tag: str,
        attrs: list[tuple[str, str | None]],
    ) -> None:
        if tag.lower() in {"script", "style"}:
            self.hidden_depth += 1

    def handle_endtag(self, tag: str) -> None:
        if tag.lower() in {"script", "style"} and self.hidden_depth:
            self.hidden_depth -= 1

    def handle_data(self, data: str) -> None:
        if not self.hidden_depth:
            self.parts.append(data)


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _render_json(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=False) + "\n"


def _render_jsonl(payloads: list[dict[str, Any]]) -> str:
    return "".join(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"
        for payload in payloads
    )


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def _normalize_tokens(text: str) -> list[str]:
    normalized = unicodedata.normalize("NFKC", text).casefold().replace("ſ", "s")
    return re.findall(r"[a-zäöüß]+", normalized)


def _token_fingerprint(tokens: list[str]) -> dict[str, Any]:
    normalized = " ".join(tokens)
    return {
        "algorithm": "sha256",
        "normalization": NORMALIZATION,
        "sha256": _sha256_bytes(normalized.encode("utf-8")),
        "token_count": len(tokens),
    }


def _cosine(left: Counter[str], right: Counter[str]) -> float:
    denominator = math.sqrt(
        sum(value * value for value in left.values())
        * sum(value * value for value in right.values())
    )
    if not denominator:
        return 0.0
    return sum(value * right[token] for token, value in left.items()) / denominator


def _heading_coverage(heading: Counter[str], target: Counter[str]) -> float:
    total = sum(heading.values())
    if not total:
        return 0.0
    return sum(min(value, target[token]) for token, value in heading.items()) / total


def _contains_token_sequence(haystack: list[str], needle: list[str]) -> bool:
    if not needle or len(needle) > len(haystack):
        return False
    width = len(needle)
    return any(haystack[index : index + width] == needle for index in range(len(haystack) - width + 1))


def _read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise StructureBuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise StructureBuildError(f"JSON root is not an object: {path}")
    return payload


def _payload_path(
    *,
    repo_root: Path,
    payload_source_root: Path,
    manifest_path: Path,
    manifest: dict[str, Any],
) -> Path:
    entries = manifest.get("payload_files", [])
    if not isinstance(entries, list) or len(entries) != 1 or not isinstance(entries[0], dict):
        raise StructureBuildError(f"expected one payload file: {manifest_path}")
    relative_item_dir = manifest_path.parent.relative_to(repo_root / SOURCE_ROOT)
    path = payload_source_root / relative_item_dir / entries[0]["relative_path"]
    if not path.is_file():
        raise StructureBuildError(f"local payload is missing: {path}")
    actual = _sha256_path(path)
    if actual != entries[0].get("sha256"):
        raise StructureBuildError(f"local payload digest drifted: {path}")
    return path


def _witness(
    *,
    manifest: dict[str, Any],
    manifest_path: Path,
    repo_root: Path,
    part_label: str | None = None,
) -> dict[str, Any]:
    inventory_path = repo_root / manifest["resource_inventory_ref"]
    inventory = _read_json(inventory_path)
    files = inventory.get("files", [])
    if not isinstance(files, list) or len(files) != 1 or not isinstance(files[0], dict):
        raise StructureBuildError(f"expected one inventory file: {inventory_path}")
    file_inventory = files[0]
    payload_entry = manifest["payload_files"][0]
    if file_inventory.get("file_id") != payload_entry.get("file_id"):
        raise StructureBuildError(f"manifest/inventory file identity drifted: {manifest_path}")
    payload = {
        "item_ref": manifest["item_id"],
        "file_ref": payload_entry["file_id"],
        "file_sha256": payload_entry["sha256"],
        "inventory_ref": manifest["resource_inventory_ref"],
        "profile": file_inventory["profile"],
    }
    if part_label is not None:
        payload = {"part_label": part_label, **payload}
    return payload


def _collect_divisions(tei_path: Path) -> dict[str, dict[str, Any]]:
    root = ET.parse(tei_path).getroot()
    divisions: dict[str, dict[str, Any]] = {}

    def walk(element: ET.Element, element_path: str) -> None:
        counts: Counter[str] = Counter()
        for child in element:
            name = _local_name(child.tag)
            counts[name] += 1
            child_path = f"{element_path}/{name}[{counts[name]}]"
            if name == "div":
                head = next(
                    (candidate for candidate in child if _local_name(candidate.tag) == "head"),
                    None,
                )
                heading_text = "".join(head.itertext()) if head is not None else ""
                divisions[child_path] = {
                    "heading_tokens": _normalize_tokens(heading_text),
                    "content_tokens": _normalize_tokens(" ".join(child.itertext())),
                }
            walk(child, child_path)

    walk(root, "TEI")
    return divisions


def _selected_division(
    *,
    part_label: str,
    resource: dict[str, Any],
    heading_tokens: list[str],
) -> bool:
    if resource.get("resource_kind") != "tei_division":
        return False
    if resource.get("structural_role") != "division":
        return False
    locator = resource.get("locator", {})
    if not isinstance(locator, dict):
        return False
    path = locator.get("tei_path")
    depth = locator.get("tei_depth")
    if not isinstance(path, str) or "/body[1]/" not in path:
        return False
    if not heading_tokens:
        return False
    if part_label == "I":
        return depth == 1 or (
            depth == 2
            and path.startswith("TEI/text[1]/body[1]/div[2]/div[")
        )
    if depth != 1:
        return False
    if part_label == "IV" and path.startswith("TEI/text[1]/body[1]/div[21]"):
        return False
    return True


def _epub_page_texts(
    epub_path: Path,
) -> tuple[dict[int, list[str]], dict[int, str]]:
    page_tokens: dict[int, list[str]] = {}
    member_paths: dict[int, str] = {}
    with zipfile.ZipFile(epub_path) as archive:
        for member in archive.infolist():
            match = PAGE_MEMBER_RE.fullmatch(member.filename)
            if match is None:
                continue
            parser = _VisibleTextParser()
            parser.feed(archive.read(member).decode("utf-8", errors="replace"))
            page_number = int(match.group(1))
            page_tokens[page_number] = _normalize_tokens(" ".join(parser.parts))
            member_paths[page_number] = member.filename
    return page_tokens, member_paths


def _choose_target_page(
    *,
    heading_tokens: list[str],
    source_tokens: list[str],
    target_tokens: dict[int, list[str]],
    first_page: int,
    last_page: int,
) -> dict[str, Any]:
    exact_pages = [
        page
        for page in range(first_page, last_page + 1)
        if _contains_token_sequence(target_tokens.get(page, []), heading_tokens)
    ]
    candidates = exact_pages or list(range(first_page, last_page + 1))
    source_counter = Counter(source_tokens[:SOURCE_CONTEXT_TOKEN_LIMIT])
    heading_counter = Counter(heading_tokens)
    scored: list[tuple[float, int, float, float]] = []
    for page in candidates:
        target_window = target_tokens.get(page, []) + target_tokens.get(page + 1, [])
        target_counter = Counter(target_window)
        context_cosine = _cosine(source_counter, target_counter)
        coverage = _heading_coverage(heading_counter, target_counter)
        score = CONTEXT_WEIGHT * context_cosine + HEADING_WEIGHT * coverage
        scored.append((score, page, context_cosine, coverage))
    scored.sort(key=lambda entry: (-entry[0], entry[1]))
    selected = scored[0]
    runner_up = scored[1] if len(scored) > 1 else None
    if len(exact_pages) == 1:
        mode = "normalized_heading_unique"
    elif exact_pages:
        mode = "normalized_heading_context_disambiguated"
    else:
        mode = "normalized_context_without_heading_sequence"
    return {
        "page": selected[1],
        "mode": mode,
        "normalized_heading_occurrence_count": len(exact_pages),
        "context_cosine": round(selected[2], 6),
        "heading_token_coverage": round(selected[3], 6),
        "candidate_score": round(selected[0], 6),
        "runner_up_score": round(runner_up[0], 6) if runner_up else None,
        "score_margin": (
            round(max(0.0, selected[0] - runner_up[0]), 6)
            if runner_up
            else None
        ),
    }


def _resource_by_id(file_inventory: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {
        resource["resource_id"]: resource
        for resource in file_inventory.get("resources", [])
        if isinstance(resource, dict) and isinstance(resource.get("resource_id"), str)
    }


def _epub_resource_by_member(
    file_inventory: dict[str, Any],
) -> dict[str, dict[str, Any]]:
    resources: dict[str, dict[str, Any]] = {}
    for resource in file_inventory.get("resources", []):
        if not isinstance(resource, dict):
            continue
        locator = resource.get("locator", {})
        member_path = locator.get("member_path") if isinstance(locator, dict) else None
        if isinstance(member_path, str):
            resources[member_path] = resource
    return resources


def build_correspondence(
    *,
    repo_root: Path,
    payload_source_root: Path,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    epub_manifest_path = repo_root / TARGET_EPUB_MANIFEST_REF
    pdf_manifest_path = repo_root / TARGET_PDF_MANIFEST_REF
    epub_manifest = _read_json(epub_manifest_path)
    pdf_manifest = _read_json(pdf_manifest_path)
    epub_payload = _payload_path(
        repo_root=repo_root,
        payload_source_root=payload_source_root,
        manifest_path=epub_manifest_path,
        manifest=epub_manifest,
    )
    epub_inventory = _read_json(repo_root / epub_manifest["resource_inventory_ref"])
    pdf_inventory = _read_json(repo_root / pdf_manifest["resource_inventory_ref"])
    epub_file = epub_inventory["files"][0]
    pdf_file = pdf_inventory["files"][0]
    epub_resources = _epub_resource_by_member(epub_file)
    pdf_resources = _resource_by_id(pdf_file)
    target_tokens, member_paths = _epub_page_texts(epub_payload)

    enumerated_epub_pages = sorted(member_paths)
    enumerated_pdf_pages = sorted(
        resource["locator"]["page_index"]
        for resource in pdf_file["resources"]
        if resource.get("resource_kind") == "pdf_page"
    )
    if enumerated_epub_pages != list(range(0, 529)):
        raise StructureBuildError("Naumann EPUB page-member enumeration drifted")
    if enumerated_pdf_pages != list(range(1, 530)):
        raise StructureBuildError("Naumann PDF page enumeration drifted")

    source_parts: list[dict[str, Any]] = []
    part_routes: list[dict[str, Any]] = []
    correspondences: list[dict[str, Any]] = []
    input_refs: list[dict[str, Any]] = []

    for spec in PART_SPECS:
        manifest_path = repo_root / spec.manifest_ref
        manifest = _read_json(manifest_path)
        tei_payload = _payload_path(
            repo_root=repo_root,
            payload_source_root=payload_source_root,
            manifest_path=manifest_path,
            manifest=manifest,
        )
        inventory = _read_json(repo_root / manifest["resource_inventory_ref"])
        file_inventory = inventory["files"][0]
        divisions = _collect_divisions(tei_payload)
        witness = _witness(
            manifest=manifest,
            manifest_path=manifest_path,
            repo_root=repo_root,
            part_label=spec.part_label,
        )
        if witness["profile"] != "tei_structure_v1":
            raise StructureBuildError(f"source part is not a TEI inventory: {spec.part_label}")
        source_parts.append(witness)
        input_refs.append(
            {
                "ref": witness["file_ref"],
                "role": f"fixity-verified-local-tei-part-{spec.part_label.lower()}",
                "sha256": witness["file_sha256"],
            }
        )
        part_routes.append(
            {
                "part_label": spec.part_label,
                "source_item_ref": witness["item_ref"],
                "selection_policy": spec.selection_policy,
                "target_epub_member_page_range": {
                    "first": spec.first_target_page,
                    "last": spec.last_target_page,
                },
            }
        )

        selected_resources: list[tuple[dict[str, Any], dict[str, Any]]] = []
        for resource in file_inventory["resources"]:
            locator = resource.get("locator", {})
            path = locator.get("tei_path") if isinstance(locator, dict) else None
            division = divisions.get(path) if isinstance(path, str) else None
            if division is None:
                continue
            if _selected_division(
                part_label=spec.part_label,
                resource=resource,
                heading_tokens=division["heading_tokens"],
            ):
                selected_resources.append((resource, division))
        if not selected_resources:
            raise StructureBuildError(f"no named structural divisions selected: {spec.part_label}")

        target_pages: list[int] = []
        for sequence, (resource, division) in enumerate(selected_resources, start=1):
            match = _choose_target_page(
                heading_tokens=division["heading_tokens"],
                source_tokens=division["content_tokens"],
                target_tokens=target_tokens,
                first_page=spec.first_target_page,
                last_page=spec.last_target_page,
            )
            page = match.pop("page")
            target_pages.append(page)
            member_path = member_paths[page]
            epub_resource = epub_resources.get(member_path)
            if epub_resource is None:
                raise StructureBuildError(f"EPUB inventory lacks {member_path}")
            pdf_page_index = page + 1
            pdf_resource_id = f"pdf-page-{pdf_page_index:04d}"
            pdf_resource = pdf_resources.get(pdf_resource_id)
            if pdf_resource is None:
                raise StructureBuildError(f"PDF inventory lacks {pdf_resource_id}")
            locator = resource["locator"]
            label_fingerprint = resource.get("label_fingerprint")
            if not isinstance(label_fingerprint, dict):
                raise StructureBuildError(
                    f"named division lacks label fingerprint: {resource['resource_id']}"
                )
            source_window = division["content_tokens"][:SOURCE_CONTEXT_TOKEN_LIMIT]
            target_window_pages = [
                candidate
                for candidate in range(page, min(page + 1, spec.last_target_page) + 1)
            ]
            correspondences.append(
                {
                    "correspondence_id": (
                        f"structure-{spec.part_label.lower()}-{sequence:03d}"
                    ),
                    "part_label": spec.part_label,
                    "sequence": sequence,
                    "source": {
                        "item_ref": witness["item_ref"],
                        "file_ref": witness["file_ref"],
                        "inventory_ref": witness["inventory_ref"],
                        "resource_id": resource["resource_id"],
                        "tei_path": locator["tei_path"],
                        "tei_depth": locator["tei_depth"],
                        "tei_page_label": locator["tei_page_label"],
                        "label_fingerprint": label_fingerprint,
                        "matching_window_fingerprint": _token_fingerprint(source_window),
                    },
                    "target_epub": {
                        "item_ref": epub_manifest["item_id"],
                        "file_ref": epub_file["file_id"],
                        "inventory_ref": epub_manifest["resource_inventory_ref"],
                        "resource_id": epub_resource["resource_id"],
                        "member_path": member_path,
                        "member_sha256": epub_resource["sha256"],
                        "scan_page_number": page,
                        "spine_index": epub_resource["locator"]["spine_index"],
                        "content_fingerprint": epub_resource["content_fingerprint"],
                    },
                    "target_pdf": {
                        "item_ref": pdf_manifest["item_id"],
                        "file_ref": pdf_file["file_id"],
                        "inventory_ref": pdf_manifest["resource_inventory_ref"],
                        "resource_id": pdf_resource["resource_id"],
                        "page_index": pdf_page_index,
                    },
                    "match": {
                        **match,
                        "search_page_range": {
                            "first": spec.first_target_page,
                            "last": spec.last_target_page,
                        },
                        "target_window_pages": target_window_pages,
                        "status": "machine_corroborated_candidate",
                    },
                }
            )
        if target_pages != sorted(target_pages):
            raise StructureBuildError(
                f"selected target pages are not monotonic in part {spec.part_label}"
            )

    epub_witness = _witness(
        manifest=epub_manifest,
        manifest_path=epub_manifest_path,
        repo_root=repo_root,
    )
    pdf_witness = _witness(
        manifest=pdf_manifest,
        manifest_path=pdf_manifest_path,
        repo_root=repo_root,
    )
    input_refs.extend(
        [
            {
                "ref": epub_witness["file_ref"],
                "role": "fixity-verified-local-naumann-1893-epub",
                "sha256": epub_witness["file_sha256"],
            },
            {
                "ref": pdf_witness["file_ref"],
                "role": "fixity-verified-local-naumann-1893-image-pdf",
                "sha256": pdf_witness["file_sha256"],
            },
        ]
    )
    part_counts = Counter(item["part_label"] for item in correspondences)
    mode_counts = Counter(item["match"]["mode"] for item in correspondences)
    payload = {
        "$schema": SCHEMA_REF,
        "schema_version": "tos_witness_structure_correspondence_v1",
        "correspondence_map_id": MAP_ID,
        "work_ref": WORK_REF,
        "correspondence_authority": "mechanical_candidate_only",
        "source_text_included": False,
        "source_parts": source_parts,
        "target_witnesses": {
            "epub": epub_witness,
            "pdf": pdf_witness,
        },
        "scan_resource_relation": {
            "state": "mechanical_enumeration_candidate",
            "epub_member_page_range": {"first": 0, "last": 528},
            "pdf_page_index_range": {"first": 1, "last": 529},
            "formula": "pdf_page_index = epub_member_page_number + 1",
            "basis": [
                "complete contiguous EPUB page-member enumeration",
                "complete contiguous PDF page-index enumeration",
                "shared Internet Archive source-item lineage",
            ],
            "exact_content_identity_claimed": False,
        },
        "method": {
            "name": "named-division-heading-and-context-correspondence",
            "version": "1",
            "normalization": NORMALIZATION,
            "selection_unit": "named_primary_tei_division_start",
            "source_context_token_limit": SOURCE_CONTEXT_TOKEN_LIMIT,
            "target_window_page_count": TARGET_WINDOW_PAGE_COUNT,
            "score": {
                "context_cosine_weight": CONTEXT_WEIGHT,
                "heading_token_coverage_weight": HEADING_WEIGHT,
            },
            "no_llm": True,
            "no_source_text_emitted": True,
        },
        "part_routes": part_routes,
        "correspondences": correspondences,
        "summary": {
            "correspondence_count": len(correspondences),
            "part_counts": {
                part: part_counts[part] for part in ("I", "II", "III", "IV")
            },
            "match_mode_counts": dict(sorted(mode_counts.items())),
            "monotonic_within_each_part": True,
        },
        "provenance_ref": PROVENANCE_PATH.as_posix(),
        "provenance_event_ref": EVENT_ID,
        "map_version": 1,
        "supersedes_map_ref": None,
        "authority_boundary": AUTHORITY_BOUNDARY,
        "does_not_establish": DOES_NOT_ESTABLISH,
    }
    return payload, input_refs


def _anchor_id(correspondence_id: str, role: str) -> str:
    suffix = correspondence_id.removeprefix("structure-")
    return f"tos.anchor.zarathustra-structure.{role}-{suffix}"


def _anchor_record(
    *,
    anchor_id: str,
    item_id: str,
    file_id: str,
    file_sha256: str,
    selectors: list[dict[str, Any]],
    method: str,
    correspondence_id: str,
) -> dict[str, Any]:
    return {
        "schema_version": "tos_source_anchor_v1",
        "anchor_id": anchor_id,
        "item_id": item_id,
        "file_id": file_id,
        "file_sha256": file_sha256,
        "passage_id": None,
        "selectors": selectors,
        "selector_method": {
            "maker_type": "software",
            "method": method,
            "version": "1",
            "configuration_ref": (
                f"{OUTPUT_PATH.as_posix()}#{correspondence_id}"
            ),
        },
        "status": "proposed",
        "provenance_event_ref": ANCHOR_EVENT_ID,
        "anchor_version": 1,
        "supersedes_anchor_ref": None,
        "review_ref": None,
    }


def build_structure_anchors(
    map_payload: dict[str, Any],
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    source_witnesses = {
        witness["item_ref"]: witness
        for witness in map_payload["source_parts"]
    }
    epub_witness = map_payload["target_witnesses"]["epub"]
    pdf_witness = map_payload["target_witnesses"]["pdf"]
    records: list[dict[str, Any]] = []
    bindings: list[dict[str, Any]] = []

    for correspondence in map_payload["correspondences"]:
        correspondence_id = correspondence["correspondence_id"]
        source = correspondence["source"]
        target_epub = correspondence["target_epub"]
        target_pdf = correspondence["target_pdf"]
        source_witness = source_witnesses[source["item_ref"]]

        source_anchor_id = _anchor_id(correspondence_id, "dta")
        epub_anchor_id = _anchor_id(correspondence_id, "naumann-1893-epub")
        pdf_anchor_id = _anchor_id(correspondence_id, "naumann-1893-pdf")

        records.extend(
            [
                _anchor_record(
                    anchor_id=source_anchor_id,
                    item_id=source["item_ref"],
                    file_id=source["file_ref"],
                    file_sha256=source_witness["file_sha256"],
                    selectors=[
                        {
                            "type": "structural",
                            "path": [source["tei_path"]],
                            "scheme": "tei-xpath-like-inventory-v1",
                        }
                    ],
                    method="resource-inventory TEI division locator",
                    correspondence_id=correspondence_id,
                ),
                _anchor_record(
                    anchor_id=epub_anchor_id,
                    item_id=target_epub["item_ref"],
                    file_id=target_epub["file_ref"],
                    file_sha256=epub_witness["file_sha256"],
                    selectors=[
                        {
                            "type": "container_member",
                            "member_path": target_epub["member_path"],
                            "member_sha256": target_epub["member_sha256"],
                        }
                    ],
                    method="resource-inventory exact EPUB member locator",
                    correspondence_id=correspondence_id,
                ),
                _anchor_record(
                    anchor_id=pdf_anchor_id,
                    item_id=target_pdf["item_ref"],
                    file_id=target_pdf["file_ref"],
                    file_sha256=pdf_witness["file_sha256"],
                    selectors=[
                        {
                            "type": "page_region",
                            "page": target_pdf["page_index"],
                            "x": 0,
                            "y": 0,
                            "width": 1,
                            "height": 1,
                            "coordinate_space": "normalized_0_1",
                        }
                    ],
                    method="resource-inventory whole-page PDF locator",
                    correspondence_id=correspondence_id,
                ),
            ]
        )
        bindings.append(
            {
                "correspondence_id": correspondence_id,
                "part_label": correspondence["part_label"],
                "sequence": correspondence["sequence"],
                "source_tei": {
                    "anchor_ref": source_anchor_id,
                    "item_ref": source["item_ref"],
                    "file_ref": source["file_ref"],
                    "resource_id": source["resource_id"],
                },
                "target_epub": {
                    "anchor_ref": epub_anchor_id,
                    "item_ref": target_epub["item_ref"],
                    "file_ref": target_epub["file_ref"],
                    "resource_id": target_epub["resource_id"],
                },
                "target_pdf": {
                    "anchor_ref": pdf_anchor_id,
                    "item_ref": target_pdf["item_ref"],
                    "file_ref": target_pdf["file_ref"],
                    "resource_id": target_pdf["resource_id"],
                },
                "binding_status": "proposed_cross_witness_locator_candidate",
                "exact_textual_identity_claimed": False,
            }
        )

    rendered_map = _render_json(map_payload)
    rendered_records = _render_jsonl(records)
    count = len(bindings)
    anchor_set = {
        "$schema": ANCHOR_SET_SCHEMA_REF,
        "schema_version": "tos_witness_structure_anchor_set_v1",
        "anchor_set_id": ANCHOR_SET_ID,
        "work_ref": WORK_REF,
        "correspondence_map": {
            "ref": OUTPUT_PATH.as_posix(),
            "sha256": _sha256_bytes(rendered_map.encode("utf-8")),
        },
        "anchor_records": {
            "ref": ANCHOR_RECORDS_PATH.as_posix(),
            "sha256": _sha256_bytes(rendered_records.encode("utf-8")),
        },
        "anchor_authority": "proposed_structural_address_only",
        "source_text_included": False,
        "bindings": bindings,
        "summary": {
            "correspondence_count": count,
            "anchor_count": len(records),
            "anchors_per_correspondence": 3,
            "role_counts": {
                "source_tei": count,
                "target_epub": count,
                "target_pdf": count,
            },
            "all_anchor_statuses": ["proposed"],
        },
        "provenance_ref": PROVENANCE_PATH.as_posix(),
        "provenance_event_ref": ANCHOR_EVENT_ID,
        "anchor_set_version": 1,
        "supersedes_anchor_set_ref": None,
        "authority_boundary": ANCHOR_AUTHORITY_BOUNDARY,
        "does_not_establish": ANCHOR_DOES_NOT_ESTABLISH,
    }
    return anchor_set, records


def _prior_event_time(repo_root: Path, event_id: str) -> str | None:
    path = repo_root / PROVENANCE_PATH
    if not path.is_file():
        return None
    try:
        events = [
            json.loads(line)
            for line in path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
    except (OSError, json.JSONDecodeError):
        return None
    event = next(
        (candidate for candidate in events if candidate.get("event_id") == event_id),
        None,
    )
    value = event.get("started_at") if isinstance(event, dict) else None
    return value if isinstance(value, str) else None


def build_provenance(
    *,
    repo_root: Path,
    map_payload: dict[str, Any],
    input_refs: list[dict[str, Any]],
    event_at: str,
) -> dict[str, Any]:
    rendered_map = _render_json(map_payload)
    map_digest = _sha256_bytes(rendered_map.encode("utf-8"))
    return {
        "schema_version": "tos_provenance_event_v1",
        "event_id": EVENT_ID,
        "event_type": "alignment",
        "started_at": event_at,
        "ended_at": event_at,
        "agent_refs": [
            "model:codex",
            "software:python-standard-library",
        ],
        "inputs": input_refs,
        "outputs": [
            {
                "ref": OUTPUT_PATH.as_posix(),
                "role": "tracked_text_free_structure_correspondence_candidate",
                "sha256": map_digest,
            }
        ],
        "method": {
            "maker_type": "software",
            "name": "named-division-heading-and-context-correspondence",
            "version": "1",
            "artifact_digest": None,
            "runtime": "Python standard library XML, ZIP, and HTML parsers",
            "device": "host-cpu",
            "configuration": {
                "correspondence_count": map_payload["summary"]["correspondence_count"],
                "source_text_included": False,
                "candidate_only": True,
                "normalization": NORMALIZATION,
            },
            "prompt_or_instruction_ref": "ToS/source-witnesses/README.md",
        },
        "status": "completed_with_warnings",
        "warnings": [
            "Normalized heading and context agreement is a locator candidate, not exact textual identity or edition equivalence.",
            "The EPUB-to-PDF page formula is a mechanical enumeration candidate and not a content-identity assertion.",
            "No German correctness, translation, semantic, or canon conclusion was produced.",
        ],
        "receipt_refs": [OUTPUT_PATH.as_posix()],
        "rights_basis_ref": None,
        "event_version": 1,
        "supersedes_event_ref": None,
    }


def build_anchor_provenance(
    *,
    map_payload: dict[str, Any],
    anchor_set: dict[str, Any],
    anchor_records: list[dict[str, Any]],
    event_at: str,
) -> dict[str, Any]:
    rendered_map = _render_json(map_payload)
    rendered_anchor_set = _render_json(anchor_set)
    rendered_anchor_records = _render_jsonl(anchor_records)
    return {
        "schema_version": "tos_provenance_event_v1",
        "event_id": ANCHOR_EVENT_ID,
        "event_type": "segmentation",
        "started_at": event_at,
        "ended_at": event_at,
        "agent_refs": [
            "model:codex",
            "software:python-standard-library",
        ],
        "inputs": [
            {
                "ref": OUTPUT_PATH.as_posix(),
                "role": "tracked_text_free_structure_correspondence_candidate",
                "sha256": _sha256_bytes(rendered_map.encode("utf-8")),
            }
        ],
        "outputs": [
            {
                "ref": ANCHOR_SET_PATH.as_posix(),
                "role": "tracked_proposed_structure_anchor_set",
                "sha256": _sha256_bytes(rendered_anchor_set.encode("utf-8")),
            },
            {
                "ref": ANCHOR_RECORDS_PATH.as_posix(),
                "role": "tracked_proposed_source_anchor_records",
                "sha256": _sha256_bytes(rendered_anchor_records.encode("utf-8")),
            },
        ],
        "method": {
            "maker_type": "software",
            "name": "structure-correspondence-to-stable-source-anchors",
            "version": "1",
            "artifact_digest": None,
            "runtime": "Python standard library",
            "device": "host-cpu",
            "configuration": {
                "correspondence_count": len(map_payload["correspondences"]),
                "anchor_count": len(anchor_records),
                "anchors_per_correspondence": 3,
                "source_text_included": False,
                "candidate_only": True,
            },
            "prompt_or_instruction_ref": "ToS/doctrine/CORPUS_FOUNDATION.md",
        },
        "status": "completed_with_warnings",
        "warnings": [
            "Every emitted anchor remains proposed and identifies only a structural path, exact container member, or whole scan page.",
            "A three-way anchor binding does not establish an exact passage boundary, textual identity, or edition equivalence.",
            "No rights clearance, German correctness, translation, semantic, or canon conclusion was produced.",
        ],
        "receipt_refs": [
            ANCHOR_SET_PATH.as_posix(),
            ANCHOR_RECORDS_PATH.as_posix(),
        ],
        "rights_basis_ref": None,
        "event_version": 1,
        "supersedes_event_ref": None,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        required=True,
        help="Explicit source-witness root containing the fixity-verified local payloads.",
    )
    parser.add_argument(
        "--event-at",
        help=(
            "RFC 3339 event time for first generation; an existing event time is "
            "preserved for deterministic regeneration."
        ),
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    payload_source_root = args.payload_source_root.resolve()
    try:
        map_payload, input_refs = build_correspondence(
            repo_root=repo_root,
            payload_source_root=payload_source_root,
        )
    except StructureBuildError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    generated_at = (
        args.event_at
        or datetime.now().astimezone().isoformat(timespec="seconds")
    )
    map_event_at = _prior_event_time(repo_root, EVENT_ID) or generated_at
    anchor_event_at = _prior_event_time(repo_root, ANCHOR_EVENT_ID) or generated_at
    for event_at in (map_event_at, anchor_event_at):
        try:
            datetime.fromisoformat(event_at)
        except ValueError:
            parser.error("--event-at must be an RFC 3339-compatible timestamp")
    provenance = build_provenance(
        repo_root=repo_root,
        map_payload=map_payload,
        input_refs=input_refs,
        event_at=map_event_at,
    )
    anchor_set, anchor_records = build_structure_anchors(map_payload)
    anchor_provenance = build_anchor_provenance(
        map_payload=map_payload,
        anchor_set=anchor_set,
        anchor_records=anchor_records,
        event_at=anchor_event_at,
    )
    rendered_map = _render_json(map_payload)
    rendered_anchor_set = _render_json(anchor_set)
    rendered_anchor_records = _render_jsonl(anchor_records)
    rendered_provenance = _render_jsonl([provenance, anchor_provenance])
    output_path = repo_root / OUTPUT_PATH
    anchor_set_path = repo_root / ANCHOR_SET_PATH
    anchor_records_path = repo_root / ANCHOR_RECORDS_PATH
    provenance_path = repo_root / PROVENANCE_PATH
    if args.check:
        drift = []
        if not output_path.is_file() or output_path.read_text(encoding="utf-8") != rendered_map:
            drift.append(OUTPUT_PATH.as_posix())
        if (
            not anchor_set_path.is_file()
            or anchor_set_path.read_text(encoding="utf-8") != rendered_anchor_set
        ):
            drift.append(ANCHOR_SET_PATH.as_posix())
        if (
            not anchor_records_path.is_file()
            or anchor_records_path.read_text(encoding="utf-8") != rendered_anchor_records
        ):
            drift.append(ANCHOR_RECORDS_PATH.as_posix())
        if (
            not provenance_path.is_file()
            or provenance_path.read_text(encoding="utf-8") != rendered_provenance
        ):
            drift.append(PROVENANCE_PATH.as_posix())
        if drift:
            for path in drift:
                print(f"[drift] {path}", file=sys.stderr)
            return 1
        verb = "verified"
    else:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(rendered_map, encoding="utf-8")
        anchor_set_path.write_text(rendered_anchor_set, encoding="utf-8")
        anchor_records_path.write_text(rendered_anchor_records, encoding="utf-8")
        provenance_path.write_text(rendered_provenance, encoding="utf-8")
        verb = "wrote"
    print(
        f"[ok] {verb} {len(map_payload['correspondences'])} text-free "
        "named-division correspondence candidates and "
        f"{len(anchor_records)} proposed structural anchors"
    )
    print(
        "[boundary] locator candidates only; no source text, textual identity, "
        "accepted German, translation, semantics, or canon promotion"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
