#!/usr/bin/env python3
"""Build one text-free German source-triangulation packet.

The builder compares a local-only eKGWB response, the exact DTA TEI payload,
and two members of the Naumann automatic-OCR EPUB. It emits only selectors,
fixity, aggregate counts, and one-way fingerprints. It does not fetch the
network, publish source text, accept German, admit a critical edition, or open
translation and semantic gates.
"""

from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import re
import sys
import unicodedata
import zipfile
from html.parser import HTMLParser
from pathlib import Path
from typing import Any
from xml.etree import ElementTree


REPO_ROOT = Path(__file__).resolve().parents[1]
GOLD_ROOT = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1"
)
PACKET_PATH = GOLD_ROOT / (
    "german-source-triangulation."
    "ekgwb-dta-naumann.za-i-vorrede-1.v1.json"
)
PROVENANCE_PATH = GOLD_ROOT / "provenance.german-source-triangulation.jsonl"
DISCOVERY_PATH = Path(
    "ToS/source-witnesses/discovery/runs/"
    "ekgwb-za-i-vorrede-1-http-fallback.2026-07-29.v2.json"
)
PRIOR_DISCOVERY_PATH = Path(
    "ToS/source-witnesses/discovery/runs/"
    "ekgwb-za-i-vorrede-1.2026-07-28.v1.json"
)
ASSISTED_REVIEW_PATH = GOLD_ROOT / "german-assisted-source-review.v1.json"
SOURCE_REVIEW_PATH = GOLD_ROOT / "translation-source-review-plan.v2.json"
CRITICAL_WITNESS_PATH = (
    GOLD_ROOT / "critical-edition-witness.ekgwb.za-i-vorrede-1.v1.json"
)

EKGWB_LOCAL_PATH = Path(
    "works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1/local-content/translation/source-review/"
    "critical-edition-candidates/ekgwb/za-i/"
    "static-html-include.response.html"
)
DTA_ITEM_ROOT = Path(
    "works/friedrich-nietzsche/also-sprach-zarathustra/"
    "expressions/de-schmeitzner-1883-part-1/"
    "editions/chemnitz-schmeitzner-1883-part-1/items/"
    "dta-sbb-corrected-tei-p5"
)
DTA_PAYLOAD_PATH = (
    DTA_ITEM_ROOT / "payload/nietzsche_zarathustra01_1883.tei.xml"
)
NAUMANN_ITEM_ROOT = Path(
    "works/friedrich-nietzsche/also-sprach-zarathustra/"
    "expressions/de-naumann-1893/editions/leipzig-c-g-naumann-1893/"
    "items/internet-archive-cornell-auto-epub"
)
NAUMANN_PAYLOAD_PATH = (
    NAUMANN_ITEM_ROOT / "payload/Nietzsche_Also_sprach_Zarathustra_1893.epub"
)
NAUMANN_MEMBERS = ("EPUB/page_55.html", "EPUB/page_56.html")

SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "german-source-triangulation.schema.json"
)
PACKET_ID = (
    "tos.german-source-triangulation."
    "ekgwb-dta-naumann.za-i-vorrede-1.v1"
)
EVENT_ID = (
    "tos.event.alignment.zarathustra-german-source-triangulation."
    "za-i-vorrede-1.2026-07-29"
)
DISCOVERY_EVENT_ID = (
    "tos.event.discovery.ekgwb-za-i-vorrede-1-http-fallback.2026-07-29"
)
ACQUISITION_EVENT_ID = (
    "tos.event.acquisition.ekgwb-za-i-static-html.2026-07-29"
)
ACQUIRED_AT = "2026-07-29T08:02:23Z"
PREPARED_AT = "2026-07-29T08:02:25Z"
DISCOVERY_STARTED_AT = "2026-07-29T08:02:21Z"
DISCOVERY_ENDED_AT = "2026-07-29T08:02:25Z"
REFERENCE_DIGEST = (
    "6d3deb76b489989f2a8ed3782f3ae9c12914a0ab6baea1b9f21432bcc8749e16"
)
EKGWB_DIGEST = (
    "3ad9b3f601eccf45446c71f2758d1aa5fd70fb01cc67c81453526f1b7eeb7061"
)
DTA_DIGEST = (
    "d3fa5f8af39d87c8f0ede7a0b52b26da0c57e73125d3b0bd2a836b80eae24a4b"
)
NAUMANN_DIGEST = (
    "adc7eeae2d5a5cf2b225ef81170b595c77664167a539f0cb1a478184aca8e9de"
)
EXPECTED_MEMBER_DIGESTS = {
    "EPUB/page_55.html": (
        "0e29c6e4e9c29756a1014530c1f5d4a9140a6fbb3ca613a296a174f3e296fa5c"
    ),
    "EPUB/page_56.html": (
        "0aec8864adf973d300fa4e1a4e923c492199ca9d8ccc0726cafd0200992d62c7"
    ),
}
AUTHORITY_BOUNDARY = (
    "machine-only text-free triangulation evidence over one local "
    "critical-edition candidate, one DTA TEI witness, and one automatic-OCR "
    "EPUB witness; not accepted German, critical-edition admission, "
    "translation evidence, semantics, rights clearance, or canon authority"
)


class TriangulationBuildError(RuntimeError):
    """Raised when an exact input or expected comparison shape drifts."""


class _ParagraphDivParser(HTMLParser):
    """Collect text from div.p elements in a bounded HTML fragment."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self._div_depth = 0
        self._capture_depth: int | None = None
        self._parts: list[str] = []
        self.paragraphs: list[str] = []

    def handle_starttag(
        self,
        tag: str,
        attrs: list[tuple[str, str | None]],
    ) -> None:
        if tag.lower() != "div":
            return
        self._div_depth += 1
        classes = dict(attrs).get("class", "").split()
        if self._capture_depth is None and "p" in classes:
            self._capture_depth = self._div_depth
            self._parts = []

    def handle_endtag(self, tag: str) -> None:
        if tag.lower() != "div":
            return
        if (
            self._capture_depth is not None
            and self._div_depth == self._capture_depth
        ):
            self.paragraphs.append("".join(self._parts))
            self._capture_depth = None
            self._parts = []
        self._div_depth = max(0, self._div_depth - 1)

    def handle_data(self, data: str) -> None:
        if self._capture_depth is not None:
            self._parts.append(data)


class _BodyTextParser(HTMLParser):
    """Collect visible body data from one EPUB XHTML member."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self._body_depth = 0
        self._suppressed_depth = 0
        self.parts: list[str] = []

    def handle_starttag(
        self,
        tag: str,
        attrs: list[tuple[str, str | None]],
    ) -> None:
        del attrs
        lowered = tag.lower()
        if lowered == "body":
            self._body_depth += 1
        elif self._body_depth and lowered in {"script", "style"}:
            self._suppressed_depth += 1

    def handle_endtag(self, tag: str) -> None:
        lowered = tag.lower()
        if self._body_depth and lowered in {"script", "style"}:
            self._suppressed_depth = max(0, self._suppressed_depth - 1)
        elif lowered == "body":
            self._body_depth = max(0, self._body_depth - 1)

    def handle_data(self, data: str) -> None:
        if self._body_depth and not self._suppressed_depth:
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
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _render_jsonl(payloads: list[dict[str, Any]]) -> str:
    return "".join(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"
        for payload in payloads
    )


def _alpha_tokens(text: str) -> list[str]:
    normalized = unicodedata.normalize("NFKC", text).casefold()
    tokens: list[str] = []
    current: list[str] = []
    for character in normalized:
        if character.isalpha():
            current.append(character)
        elif current:
            tokens.append("".join(current))
            current = []
    if current:
        tokens.append("".join(current))
    return tokens


def _sequence_digest(tokens: list[str]) -> str:
    return _sha256_bytes(" ".join(tokens).encode("utf-8"))


def _extract_ekgwb_paragraphs(payload: bytes) -> list[str]:
    text = payload.decode("utf-8")
    start_marker = 'id="eKGWB/Za-I-Vorrede-1"'
    next_marker = '<a name="eKGWB/Za-I-Vorrede-2"'
    start = text.find(start_marker)
    if start < 0:
        raise TriangulationBuildError("eKGWB target element is absent")
    end = text.find(next_marker, start)
    if end < 0:
        raise TriangulationBuildError("eKGWB next-section boundary is absent")
    parser = _ParagraphDivParser()
    parser.feed(text[start:end])
    paragraphs = [
        paragraph
        for paragraph in parser.paragraphs
        if len(_alpha_tokens(paragraph)) > 1
    ]
    if len(paragraphs) != 12:
        raise TriangulationBuildError(
            f"eKGWB paragraph count drifted: {len(paragraphs)}"
        )
    return paragraphs


def _extract_dta_paragraphs(payload: bytes) -> tuple[list[str], list[str]]:
    try:
        root = ElementTree.fromstring(payload)
    except ElementTree.ParseError as exc:
        raise TriangulationBuildError(f"cannot parse DTA TEI: {exc}") from exc
    namespace = {"tei": "http://www.tei-c.org/ns/1.0"}
    body = root.find("./tei:text/tei:body", namespace)
    if body is None:
        raise TriangulationBuildError("DTA TEI body is absent")
    first_div = body.find("./tei:div", namespace)
    section = (
        first_div.find("./tei:div", namespace)
        if first_div is not None
        else None
    )
    if section is None:
        raise TriangulationBuildError("DTA target section is absent")
    raw_paragraphs = [
        "".join(paragraph.itertext())
        for paragraph in section.findall("./tei:p", namespace)
    ]
    paragraphs = [
        re.sub(r"¬\s*", "", paragraph)
        for paragraph in raw_paragraphs
        if len(_alpha_tokens(re.sub(r"¬\s*", "", paragraph))) > 1
    ]
    if len(paragraphs) != 12:
        raise TriangulationBuildError(
            f"DTA paragraph count drifted: {len(paragraphs)}"
        )
    return raw_paragraphs, paragraphs


def _extract_epub_tokens(
    payload_path: Path,
) -> tuple[list[str], list[dict[str, str]]]:
    tokens: list[str] = []
    member_refs: list[dict[str, str]] = []
    try:
        with zipfile.ZipFile(payload_path) as archive:
            for member in NAUMANN_MEMBERS:
                payload = archive.read(member)
                member_digest = _sha256_bytes(payload)
                if member_digest != EXPECTED_MEMBER_DIGESTS[member]:
                    raise TriangulationBuildError(
                        f"Naumann member digest drifted: {member}"
                    )
                parser = _BodyTextParser()
                parser.feed(payload.decode("utf-8"))
                tokens.extend(_alpha_tokens(" ".join(parser.parts)))
                member_refs.append(
                    {"path": member, "sha256": member_digest}
                )
    except (OSError, KeyError, UnicodeDecodeError, zipfile.BadZipFile) as exc:
        raise TriangulationBuildError(
            f"cannot inspect Naumann EPUB: {exc}"
        ) from exc
    return tokens, member_refs


def _tracked_witness(
    *,
    repo_root: Path,
    item_root: Path,
    payload_path: Path,
    expected_payload_digest: str,
) -> dict[str, Any]:
    manifest_path = (
        Path("ToS/source-witnesses") / item_root / "item.manifest.json"
    )
    rights_path = Path("ToS/source-witnesses") / item_root / "rights.json"
    try:
        manifest = json.loads(
            (repo_root / manifest_path).read_text(encoding="utf-8")
        )
    except (OSError, json.JSONDecodeError) as exc:
        raise TriangulationBuildError(
            f"cannot read manifest {manifest_path}: {exc}"
        ) from exc
    payload_rows = manifest.get("payload_files", [])
    row = next(
        (
            candidate
            for candidate in payload_rows
            if isinstance(candidate, dict)
            and candidate.get("relative_path")
            == payload_path.relative_to(item_root).as_posix()
        ),
        None,
    )
    if not isinstance(row, dict):
        raise TriangulationBuildError(
            f"payload is absent from manifest {manifest_path}"
        )
    if row.get("sha256") != expected_payload_digest:
        raise TriangulationBuildError(
            f"manifest payload digest drifted: {manifest_path}"
        )
    return {
        "item_ref": manifest["item_id"],
        "file_ref": row["file_id"],
        "file_sha256": row["sha256"],
        "payload_relative_path": payload_path.as_posix(),
        "item_manifest": {
            "ref": manifest_path.as_posix(),
            "sha256": _sha256_path(repo_root / manifest_path),
        },
        "rights_record": {
            "ref": rights_path.as_posix(),
            "sha256": _sha256_path(repo_root / rights_path),
        },
        "source_text_tracked": False,
    }


def _digest_bound(repo_root: Path, path: Path) -> dict[str, str]:
    return {
        "ref": path.as_posix(),
        "sha256": _sha256_path(repo_root / path),
    }


def _comparison_metrics(
    reference_tokens: list[str],
    candidate_tokens: list[str],
) -> dict[str, Any]:
    matcher = difflib.SequenceMatcher(
        None,
        reference_tokens,
        candidate_tokens,
        autojunk=False,
    )
    opcodes = matcher.get_opcodes()
    expected_shapes = [
        ("equal", 0, 111, 0, 111),
        ("replace", 111, 112, 111, 112),
        ("equal", 112, 165, 112, 165),
        ("insert", 165, 165, 165, 180),
        ("equal", 165, 261, 180, 276),
        ("insert", 261, 261, 276, 392),
    ]
    if opcodes != expected_shapes:
        raise TriangulationBuildError(
            f"Naumann comparison shape drifted: {opcodes}"
        )
    return {
        "candidate_tokens": len(candidate_tokens),
        "equal_reference_tokens": sum(
            a2 - a1
            for tag, a1, a2, _b1, _b2 in opcodes
            if tag == "equal"
        ),
        "single_token_replacements": sum(
            1
            for tag, a1, a2, b1, b2 in opcodes
            if tag == "replace" and a2 - a1 == 1 and b2 - b1 == 1
        ),
        "page_furniture_insertions": 15,
        "trailing_next_section_tokens": 116,
        "equal_run_lengths": [
            a2 - a1
            for tag, a1, a2, _b1, _b2 in opcodes
            if tag == "equal"
        ],
        "exact_textual_identity": False,
        "translation_claimed": False,
    }


def build_outputs(
    *,
    repo_root: Path,
    payload_source_root: Path,
    prepared_at: str = PREPARED_AT,
) -> tuple[str, str, str]:
    ekgwb_path = payload_source_root / EKGWB_LOCAL_PATH
    dta_path = payload_source_root / DTA_PAYLOAD_PATH
    naumann_path = payload_source_root / NAUMANN_PAYLOAD_PATH
    for path, expected_digest in (
        (ekgwb_path, EKGWB_DIGEST),
        (dta_path, DTA_DIGEST),
        (naumann_path, NAUMANN_DIGEST),
    ):
        if not path.is_file():
            raise TriangulationBuildError(f"local payload is absent: {path}")
        actual_digest = _sha256_path(path)
        if actual_digest != expected_digest:
            raise TriangulationBuildError(
                f"local payload digest drifted: {path}"
            )
    if ekgwb_path.stat().st_size != 180138:
        raise TriangulationBuildError("eKGWB response byte size drifted")

    ekgwb_payload = ekgwb_path.read_bytes()
    ekgwb_paragraphs = _extract_ekgwb_paragraphs(ekgwb_payload)
    ekgwb_paragraph_tokens = [
        _alpha_tokens(paragraph) for paragraph in ekgwb_paragraphs
    ]
    reference_tokens = [
        token
        for paragraph_tokens in ekgwb_paragraph_tokens
        for token in paragraph_tokens
    ]
    if (
        len(reference_tokens) != 261
        or _sequence_digest(reference_tokens) != REFERENCE_DIGEST
    ):
        raise TriangulationBuildError("eKGWB normalized sequence drifted")

    raw_dta_paragraphs, dta_paragraphs = _extract_dta_paragraphs(
        dta_path.read_bytes()
    )
    dta_paragraph_tokens = [
        _alpha_tokens(paragraph) for paragraph in dta_paragraphs
    ]
    dta_tokens = [
        token
        for paragraph_tokens in dta_paragraph_tokens
        for token in paragraph_tokens
    ]
    equal_paragraphs = sum(
        source == target
        for source, target in zip(
            ekgwb_paragraph_tokens,
            dta_paragraph_tokens,
            strict=True,
        )
    )
    if (
        len(dta_tokens) != 261
        or _sequence_digest(dta_tokens) != REFERENCE_DIGEST
        or equal_paragraphs != 12
        or dta_tokens != reference_tokens
    ):
        raise TriangulationBuildError(
            "DTA source-aware normalized sequence no longer matches"
        )
    raw_dta_tokens = [
        token
        for paragraph in raw_dta_paragraphs
        for token in _alpha_tokens(paragraph)
    ]
    naive_false_splits = len(raw_dta_tokens) - len(dta_tokens)
    if naive_false_splits != 3:
        raise TriangulationBuildError(
            "DTA naive-normalization control no longer yields three false splits"
        )

    naumann_tokens, member_refs = _extract_epub_tokens(naumann_path)
    naumann_metrics = _comparison_metrics(reference_tokens, naumann_tokens)
    if (
        naumann_metrics["candidate_tokens"] != 392
        or naumann_metrics["equal_reference_tokens"] != 260
    ):
        raise TriangulationBuildError("Naumann aggregate metrics drifted")

    dta_witness = _tracked_witness(
        repo_root=repo_root,
        item_root=DTA_ITEM_ROOT,
        payload_path=DTA_PAYLOAD_PATH,
        expected_payload_digest=DTA_DIGEST,
    )
    naumann_witness = _tracked_witness(
        repo_root=repo_root,
        item_root=NAUMANN_ITEM_ROOT,
        payload_path=NAUMANN_PAYLOAD_PATH,
        expected_payload_digest=NAUMANN_DIGEST,
    )
    packet = {
        "$schema": SCHEMA_REF,
        "schema_version": "tos_german_source_triangulation_v1",
        "packet_id": PACKET_ID,
        "status": "machine_triangulated_candidate",
        "prepared_at": prepared_at,
        "work_ref": (
            "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
        ),
        "target": {
            "review_unit_id": "tos-translation-source-review-v2-001",
            "context_anchor_ref": (
                "tos.anchor.zarathustra-translation-pilot-v1.t001"
            ),
            "critical_locator_siglum": "Za-I-Vorrede-1",
            "critical_locator_scope": "Zarathustra's Vorrede, section 1",
        },
        "bindings": {
            "assisted_review_plan": _digest_bound(
                repo_root,
                ASSISTED_REVIEW_PATH,
            ),
            "source_review_plan": _digest_bound(
                repo_root,
                SOURCE_REVIEW_PATH,
            ),
            "metadata_critical_witness_packet": _digest_bound(
                repo_root,
                CRITICAL_WITNESS_PATH,
            ),
        },
        "inputs": {
            "ekgwb": {
                "source_role": "critical_edition_candidate_local_copy",
                "local_payload_ref": (
                    Path("ToS/source-witnesses") / EKGWB_LOCAL_PATH
                ).as_posix(),
                "payload_sha256": EKGWB_DIGEST,
                "byte_size": 180138,
                "retrieval": {
                    "official_endpoint": (
                        "http://www.nietzschesource.org/resources/scripts/"
                        "static_html_include.php?book=%23eKGWB%2FZa-I"
                    ),
                    "fetched_at": prepared_at,
                    "http_status": 200,
                    "media_type": "text/html; charset=UTF-8",
                    "transport": "unencrypted_http",
                    "tls_authenticity_established": False,
                    "response_persisted_local_only": True,
                    "bounded_repeat_fetches": 2,
                    "repeat_fetches_byte_identical": True,
                },
                "selector": {
                    "kind": "html_element_id",
                    "value": "eKGWB/Za-I-Vorrede-1",
                },
                "rights": {
                    "declared_license": "CC-BY-NC-ND-4.0",
                    "license_uri": (
                        "https://creativecommons.org/licenses/"
                        "by-nc-nd/4.0/"
                    ),
                    "official_rights_ref": (
                        "https://doc.nietzschesource.org/en/rights"
                    ),
                    "human_rights_review": False,
                    "local_research_only": True,
                    "redistribution_authorized": False,
                    "derivative_publication_authorized": False,
                },
                "source_text_tracked": False,
            },
            "dta_tei": dta_witness,
            "naumann_auto_epub": naumann_witness,
        },
        "method": {
            "implementation": "python-standard-library",
            "normalization": [
                "unicode_nfkc",
                "unicode_casefold",
                "unicode_alpha_token_sequence",
            ],
            "dta_dehyphenation": (
                "remove_dta_printed_hyphen_marker_u00ac_and_following_"
                "whitespace_before_tokenization"
            ),
            "comparison": (
                "python_stdlib_difflib_sequence_matcher_autojunk_false"
            ),
            "model_used_for_text_decision": False,
            "translation_performed": False,
            "source_text_emitted": False,
        },
        "results": {
            "ekgwb_reference": {
                "paragraphs": 12,
                "normalized_tokens": 261,
                "normalized_sequence_sha256": REFERENCE_DIGEST,
            },
            "dta_exact_comparison": {
                "section_selector": "TEI/text[1]/body[1]/div[1]/div[1]",
                "paragraphs": 12,
                "normalized_tokens": 261,
                "normalized_sequence_sha256": REFERENCE_DIGEST,
                "equal_paragraphs": equal_paragraphs,
                "equal_tokens": 261,
                "exact_after_source_aware_normalization": True,
            },
            "normalization_failure_control": {
                "naive_generic_whitespace_join_false_token_splits": 3,
                "source_aware_dehyphenation_required": True,
                "false_discrepancy_preserved": True,
            },
            "naumann_ocr_comparison": {
                "epub_members": member_refs,
                **naumann_metrics,
            },
        },
        "source_posture": "machine_triangulated_candidate",
        "rights_and_transport_boundary": {
            "unencrypted_transport_is_authenticity_risk": True,
            "all_payloads_remain_local": True,
            "tracked_packet_contains_source_text": False,
            "rights_clearance_claimed": False,
            "critical_edition_admission_authorized": False,
        },
        "gate_effects": {
            "machine_triangulated_units": 1,
            "human_task_created": False,
            "human_debt_units": 0,
            "source_unit_selected_for_human_review": False,
            "accepted_german_units": 0,
            "critical_edition_units_admitted": 0,
            "translation_lanes_opened": [],
            "semantic_tasks_opened": 0,
            "promotion_authorized": False,
        },
        "authority_boundary": AUTHORITY_BOUNDARY,
        "does_not_establish": [
            "source_authenticity_over_http",
            "bibliographic_acceptance",
            "rights_clearance",
            "accepted_german_text",
            "german_orthographic_correctness",
            "german_grammatical_correctness",
            "german_semantics",
            "critical_edition_admission",
            "translation_correspondence",
            "translation_equivalence",
            "translation_quality",
            "semantic_annotation",
            "canon_promotion",
        ],
    }
    packet_rendered = _render_json(packet)
    packet_digest = _sha256_bytes(packet_rendered.encode("utf-8"))
    discovery = {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "material-discovery-record.schema.json"
        ),
        "schema_version": "tos_material_discovery_record_v1",
        "discovery_id": (
            "tos.discovery.ekgwb-za-i-vorrede-1-http-fallback.2026-07-29"
        ),
        "protocol_ref": "ToS/source-witnesses/discovery/DISCOVERY_PROTOCOL.md",
        "target": {
            "target_kind": "critical-edition",
            "known_tos_refs": [
                "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
                (
                    "tos.expression.friedrich-nietzsche."
                    "also-sprach-zarathustra.de-naumann-1893"
                ),
            ],
            "description": (
                "A directly inspectable official Nietzsche Source response "
                "containing eKGWB Za-I-Vorrede-1, with transport and rights "
                "limits kept explicit."
            ),
            "required_properties": [
                "critical-edition identity",
                "exact stable section locator",
                "directly inspectable current passage",
                "originating scholarly record",
                (
                    "declared rights evidence separated from ToS rights "
                    "judgment"
                ),
            ],
            "acceptable_substitutions": [
                (
                    "an authenticated HTTPS or institutionally supplied "
                    "copy of the same exact eKGWB section"
                ),
                (
                    "another scholarly critical edition with exact "
                    "passage-level addressability and a reviewable rights route"
                ),
            ],
            "languages": ["de"],
            "formats": ["metadata", "HTML", "TEI"],
            "purpose_ref": (
                "ToS/research-packets/foundation-laboratory-2026-07/"
                "GERMAN_ASSISTED_REVIEW_RESEARCH.md"
            ),
        },
        "channels": [
            {
                "channel_id": "channel-nietzsche-source-https",
                "sequence": 1,
                "channel_type": "specialized-scholarly-project",
                "role": "digital-object-record",
                "source_name": "Nietzsche Source",
                "endpoint_url": (
                    "https://www.nietzschesource.org/resources/scripts/"
                    "static_html_include.php?book=%23eKGWB%2FZa-I"
                ),
                "interface_type": "web",
                "interface_version": None,
                "exact_query": (
                    "Two bounded curl 8.18.0 GET requests with redirects and "
                    "20-second timeout to the exact HTTPS static include "
                    "endpoint; both failed to connect to port 443 before any "
                    "HTTP response."
                ),
                "queried_at": "2026-07-29T08:02:21Z",
                "elapsed_seconds": 0.903357,
                "result_order_preserved": True,
                "results": [],
            },
            {
                "channel_id": "channel-nietzsche-source-http-spa",
                "sequence": 2,
                "channel_type": "specialized-scholarly-project",
                "role": "digital-object-record",
                "source_name": "Nietzsche Source",
                "endpoint_url": (
                    "http://www.nietzschesource.org/resources/scripts/"
                    "static_html_include.php?book=%23eKGWB%2FZa-I"
                ),
                "interface_type": "web",
                "interface_version": (
                    "public SPA static include route observed 2026-07-29"
                ),
                "exact_query": (
                    "Two bounded curl 8.18.0 GET requests with redirects and "
                    "20-second timeout to the exact HTTP static include "
                    "endpoint discovered from the public Nietzsche Source SPA."
                ),
                "queried_at": "2026-07-29T08:02:23Z",
                "elapsed_seconds": 1.943764,
                "result_order_preserved": True,
                "results": [
                    {
                        "result_id": (
                            "tos-discovery-result."
                            "ekgwb-official-http-static-za-i"
                        ),
                        "rank": 1,
                        "title_as_displayed": (
                            "Nietzsche Source eKGWB, Also sprach Zarathustra I"
                        ),
                        "result_url": (
                            "http://www.nietzschesource.org/resources/scripts/"
                            "static_html_include.php?book=%23eKGWB%2FZa-I"
                        ),
                        "originating_record_url": (
                            "http://www.nietzschesource.org/eKGWB/"
                            "Za-I-Vorrede-1"
                        ),
                        "identifiers": [
                            {
                                "scheme": "eKGWB",
                                "value": "Za-I-Vorrede-1",
                            }
                        ],
                        "available_formats": ["HTML"],
                        "declared_rights": {
                            "statement": "CC BY-NC-ND 4.0",
                            "scope": "content",
                            "evidence_url": (
                                "https://doc.nietzschesource.org/en/rights"
                            ),
                            "tos_conclusion": (
                                "evidence-only-not-a-rights-conclusion"
                            ),
                        },
                        "availability": "open-view",
                        "machine_interface": "html",
                        "decision": "select",
                        "rationale": (
                            "The official public SPA route returned the exact "
                            "part containing the named section twice with "
                            "identical bytes. Selection is local research "
                            "evidence only: unencrypted transport prevents an "
                            "authenticity claim, and rights remain unreviewed."
                        ),
                        "acquisition": {
                            "downloaded": True,
                            "acquired_at": ACQUIRED_AT,
                            "byte_size": 180138,
                            "sha256": EKGWB_DIGEST,
                            "event_ref": ACQUISITION_EVENT_ID,
                        },
                        "snapshot": {
                            "state": "captured",
                            "format": "static-snapshot",
                            "sha256": EKGWB_DIGEST,
                            "reason": (
                                "The exact response is preserved only in the "
                                "gitignored local-content lane and is not a "
                                "publication payload."
                            ),
                        },
                    }
                ],
            },
        ],
        "channel_comparison": [
            {
                "channel_id": "channel-nietzsche-source-https",
                "completeness": "unknown",
                "metadata_precision": "unknown",
                "rights_clarity": "limited",
                "machine_interface_quality": "poor",
                "human_minutes": 0,
                "machine_seconds": 0.903357,
                "notes": (
                    "Both HTTPS requests failed before an HTTP response; this "
                    "does not show that the resource is absent."
                ),
            },
            {
                "channel_id": "channel-nietzsche-source-http-spa",
                "completeness": "strong",
                "metadata_precision": "strong",
                "rights_clarity": "limited",
                "machine_interface_quality": "adequate",
                "human_minutes": 0,
                "machine_seconds": 1.943764,
                "notes": (
                    "Both public-route responses were 180138 bytes with the "
                    "same SHA-256. HTTP is unencrypted, and the declared "
                    "BY-NC-ND posture is evidence rather than ToS clearance."
                ),
            },
        ],
        "selected_result_ids": [
            "tos-discovery-result.ekgwb-official-http-static-za-i"
        ],
        "rejected_result_ids": [],
        "rights_inference_from_availability_prohibited": True,
        "general_web_search_is_last_resort": True,
        "technical_access_bypass_used": False,
        "maker": {
            "maker_type": "model",
            "agent_ref": "model:codex",
        },
        "started_at": DISCOVERY_STARTED_AT,
        "ended_at": DISCOVERY_ENDED_AT,
        "status": "reconciled",
        "provenance_event_refs": [
            DISCOVERY_EVENT_ID,
            ACQUISITION_EVENT_ID,
        ],
        "record_version": 2,
        "supersedes_discovery_ref": (
            "tos.discovery.ekgwb-za-i-vorrede-1.2026-07-28"
        ),
    }
    discovery_rendered = _render_json(discovery)
    discovery_digest = _sha256_bytes(discovery_rendered.encode("utf-8"))
    discovery_event = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": DISCOVERY_EVENT_ID,
        "event_type": "discovery",
        "started_at": DISCOVERY_STARTED_AT,
        "ended_at": DISCOVERY_ENDED_AT,
        "agent_refs": ["model:codex", "software:curl-8.18.0"],
        "inputs": [
            {
                "ref": PRIOR_DISCOVERY_PATH.as_posix(),
                "role": "prior-incomplete-source-ordered-discovery-run",
                "sha256": _sha256_path(repo_root / PRIOR_DISCOVERY_PATH),
            }
        ],
        "outputs": [
            {
                "ref": DISCOVERY_PATH.as_posix(),
                "role": "reconciled-official-http-fallback-discovery-run",
                "sha256": discovery_digest,
            }
        ],
        "method": {
            "maker_type": "mixed",
            "name": "official-spa-route-reconciliation",
            "version": "1",
            "artifact_digest": None,
            "runtime": "curl 8.18.0",
            "device": "abyss-machine",
            "configuration": {
                "https_attempts": 2,
                "http_attempts": 2,
                "timeout_seconds": 20,
                "redirects_followed": True,
                "general_web_search_used": False,
                "technical_access_bypass_used": False,
            },
            "prompt_or_instruction_ref": (
                "ToS/source-witnesses/discovery/DISCOVERY_PROTOCOL.md"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            "the authenticated HTTPS route did not connect",
            (
                "the successful official SPA endpoint used unencrypted HTTP "
                "and does not establish transport authenticity"
            ),
            (
                "declared rights were recorded as evidence and not promoted "
                "to ToS rights clearance"
            ),
        ],
        "receipt_refs": [DISCOVERY_PATH.as_posix()],
        "rights_basis_ref": CRITICAL_WITNESS_PATH.as_posix(),
        "event_version": 1,
        "supersedes_event_ref": (
            "tos.event.discovery.ekgwb-za-i-vorrede-1.2026-07-28"
        ),
    }
    acquisition_event = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": ACQUISITION_EVENT_ID,
        "event_type": "acquisition",
        "started_at": ACQUIRED_AT,
        "ended_at": ACQUIRED_AT,
        "agent_refs": ["model:codex", "software:curl-8.18.0"],
        "inputs": [
            {
                "ref": DISCOVERY_PATH.as_posix(),
                "role": "official-route-discovery-record",
                "sha256": discovery_digest,
            }
        ],
        "outputs": [
            {
                "ref": (
                    Path("ToS/source-witnesses") / EKGWB_LOCAL_PATH
                ).as_posix(),
                "role": "gitignored-local-only-http-response",
                "sha256": EKGWB_DIGEST,
            }
        ],
        "method": {
            "maker_type": "mixed",
            "name": "bounded-local-critical-candidate-acquisition",
            "version": "1",
            "artifact_digest": None,
            "runtime": "curl 8.18.0",
            "device": "abyss-machine",
            "configuration": {
                "http_status": 200,
                "byte_size": 180138,
                "transport": "unencrypted_http",
                "storage": "gitignored-local-content",
                "publication_authorized": False,
            },
            "prompt_or_instruction_ref": (
                "ToS/source-witnesses/LOCAL_STORAGE_BOUNDARY.md"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            "transport authenticity is not established",
            "the response is local research material and is not publishable",
        ],
        "receipt_refs": [DISCOVERY_PATH.as_posix()],
        "rights_basis_ref": CRITICAL_WITNESS_PATH.as_posix(),
        "event_version": 1,
        "supersedes_event_ref": None,
    }
    alignment_event = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": EVENT_ID,
        "event_type": "alignment",
        "started_at": prepared_at,
        "ended_at": prepared_at,
        "agent_refs": ["model:codex", "software:python-stdlib"],
        "inputs": [
            {
                "ref": (
                    Path("ToS/source-witnesses") / EKGWB_LOCAL_PATH
                ).as_posix(),
                "role": "local-only-unencrypted-http-critical-candidate",
                "sha256": EKGWB_DIGEST,
            },
            {
                "ref": dta_witness["file_ref"],
                "role": "fixity-verified-dta-tei-witness",
                "sha256": DTA_DIGEST,
            },
            {
                "ref": naumann_witness["file_ref"],
                "role": "fixity-verified-automatic-ocr-epub-witness",
                "sha256": NAUMANN_DIGEST,
            },
            {
                "ref": ASSISTED_REVIEW_PATH.as_posix(),
                "role": "solo-human-plus-ai-competence-boundary",
                "sha256": packet["bindings"]["assisted_review_plan"]["sha256"],
            },
        ],
        "outputs": [
            {
                "ref": PACKET_PATH.as_posix(),
                "role": "text-free-machine-triangulation-candidate",
                "sha256": packet_digest,
            }
        ],
        "method": {
            "maker_type": "mixed",
            "name": "source-aware-german-witness-triangulation",
            "version": "1",
            "artifact_digest": _sha256_path(Path(__file__)),
            "runtime": "Python standard library",
            "device": "abyss-machine",
            "configuration": {
                "network_fetch_performed_by_builder": False,
                "source_text_emitted": False,
                "source_aware_dta_dehyphenation": True,
                "comparison": (
                    "difflib.SequenceMatcher(autojunk=False)"
                ),
                "human_review_performed": False,
                "accepted_german_units": 0,
                "translation_lanes_opened": 0,
            },
            "prompt_or_instruction_ref": (
                "ToS/research-packets/foundation-laboratory-2026-07/"
                "GERMAN_ASSISTED_REVIEW_RESEARCH.md"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            (
                "the Nietzsche Source response was acquired over unencrypted "
                "HTTP, so transport authenticity is not established"
            ),
            (
                "naive generic normalization created three false DTA token "
                "splits before source-aware dehyphenation"
            ),
            (
                "the Naumann EPUB is automatic OCR and differs by one token "
                "plus page furniture in the bounded comparison"
            ),
            (
                "human German-language, bibliographic, and rights review "
                "remain absent"
            ),
            (
                "no critical edition, German source unit, translation lane, "
                "semantic task, or promotion was admitted"
            ),
        ],
        "receipt_refs": [PACKET_PATH.as_posix()],
        "rights_basis_ref": CRITICAL_WITNESS_PATH.as_posix(),
        "event_version": 1,
        "supersedes_event_ref": None,
    }
    return (
        packet_rendered,
        discovery_rendered,
        _render_jsonl(
            [discovery_event, acquisition_event, alignment_event]
        ),
    )


def _write_or_check(
    path: Path,
    expected: str,
    *,
    check: bool,
) -> bool:
    if check:
        try:
            actual = path.read_text(encoding="utf-8")
        except OSError:
            return False
        return actual == expected
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(expected, encoding="utf-8")
    return True


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=REPO_ROOT,
        help="Tree of Sophia repository root",
    )
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        required=True,
        help=(
            "explicit local source-witness root containing gitignored payloads; "
            "never inferred from the tracked checkout"
        ),
    )
    parser.add_argument(
        "--prepared-at",
        default=PREPARED_AT,
        help="fixed provenance timestamp used for deterministic regeneration",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when tracked outputs differ without writing",
    )
    args = parser.parse_args(argv)
    repo_root = args.repo_root.resolve()
    payload_source_root = args.payload_source_root.resolve()
    try:
        packet, discovery, provenance = build_outputs(
            repo_root=repo_root,
            payload_source_root=payload_source_root,
            prepared_at=args.prepared_at,
        )
    except TriangulationBuildError as exc:
        print(f"German source triangulation failed: {exc}", file=sys.stderr)
        return 1
    results = [
        _write_or_check(
            repo_root / PACKET_PATH,
            packet,
            check=args.check,
        ),
        _write_or_check(
            repo_root / DISCOVERY_PATH,
            discovery,
            check=args.check,
        ),
        _write_or_check(
            repo_root / PROVENANCE_PATH,
            provenance,
            check=args.check,
        ),
    ]
    if not all(results):
        print(
            "German source triangulation outputs differ from deterministic build",
            file=sys.stderr,
        )
        return 1
    verb = "checked" if args.check else "wrote"
    print(
        f"German source triangulation {verb}: "
        f"{PACKET_PATH.as_posix()}, {DISCOVERY_PATH.as_posix()}, "
        f"{PROVENANCE_PATH.as_posix()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
