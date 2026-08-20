#!/usr/bin/env python3
"""Prepare private, ineligible cross-work candidates for the kernel transfer test.

The builder reads one exact local PDF, selects ten digest-ranked and ten
mechanically difficult whole pages before any A/B/C output exists, writes the
extracted page text only to the ignored local-content lane, and emits tracked
anchors, digests, metrics, and provenance. It never creates target gold,
semantic labels, accepted signs, or authorization to execute a variant.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
GOLD_ROOT = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1"
)
PLAN_PATH = GOLD_ROOT / "transfer-samples.json"
ANCHOR_PATH = GOLD_ROOT / "transfer-target-anchors.v1.jsonl"
PROVENANCE_PATH = GOLD_ROOT / "transfer-provenance.jsonl"
LOCAL_CONTENT_ROOT = GOLD_ROOT / "local-content/transfer-targets/v1"
SCHEMA_PATH = Path("ToS/contracts/golden-kernel-transfer-plan.schema.json")
BOUNDARY_MAP_PATH = Path(
    "ToS/source-witnesses/collections/friedrich-nietzsche/"
    "works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries/"
    "work-boundary-map.json"
)
SOURCE_PDF_PATH = Path(
    "ToS/source-witnesses/collections/friedrich-nietzsche/"
    "works-in-two-volumes-volume-2-mysl-1996/editions/"
    "moscow-mysl-1996-volume-2/items/operator-pdf/payload/"
    "Ницше собрание сочинений.pdf"
)
RIGHTS_PATH = Path(
    "ToS/source-witnesses/collections/friedrich-nietzsche/"
    "works-in-two-volumes-volume-2-mysl-1996/editions/"
    "moscow-mysl-1996-volume-2/items/operator-pdf/rights.json"
)
GOLD_ASSURANCE_PATH = GOLD_ROOT / "gold-assurance.v2.json"
BUILDER_PATH = Path("scripts/build_golden_kernel_transfer_candidates.py")
PREPARED_AT = "2026-07-29T16:10:00Z"
EVENT_ID = (
    "tos.event.segmentation.zarathustra-transfer-candidate-freeze-v3."
    "2026-07-29"
)
PREVIOUS_EVENT_ID = (
    "tos.event.annotation.zarathustra-transfer-plan-v2.2026-07-24"
)
EXPECTED_FILE_SHA256 = (
    "ce16b68089dee0fc53a31d9e97723991b292dd8835c43bbbb40a9373c9a436aa"
)
BOUNDARY_TRIM_PAGES = 3
MINIMUM_ALPHABETIC_CHARACTERS = 800
MINIMUM_NONBLANK_LINES = 15
PDFTOTEXT_VERSION = "26.01.0"
PUNCTUATION = set(".,;:!?—–-«»\"()…")
NUMBERED_HEADING = re.compile(r"^(?:[0-9]{1,3}|[IVXLCDM]{1,8})[.)]?$")
WORK_SPECS = (
    {
        "work_ref": "tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese",
        "slug": "jenseits",
        "random": 4,
        "hard": 4,
    },
    {
        "work_ref": "tos.work.friedrich-nietzsche.zur-genealogie-der-moral",
        "slug": "genealogie",
        "random": 3,
        "hard": 3,
    },
    {
        "work_ref": "tos.work.friedrich-nietzsche.der-antichrist",
        "slug": "antichrist",
        "random": 3,
        "hard": 3,
    },
)


class TransferCandidateBuildError(RuntimeError):
    """Raised when the fixed local preparation route cannot be reproduced."""


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise TransferCandidateBuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise TransferCandidateBuildError(f"{path} is not a JSON object")
    return payload


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise TransferCandidateBuildError(f"cannot read {path}: {exc}") from exc
    records: list[dict[str, Any]] = []
    for index, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as exc:
            raise TransferCandidateBuildError(
                f"cannot read {path}:{index}: {exc}"
            ) from exc
        if not isinstance(record, dict):
            raise TransferCandidateBuildError(f"{path}:{index} is not an object")
        records.append(record)
    return records


def _render_json(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _render_jsonl(records: list[dict[str, Any]]) -> str:
    return "".join(
        json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n"
        for record in records
    )


def _pdftotext_version() -> str:
    result = subprocess.run(
        ("pdftotext", "-v"),
        check=False,
        capture_output=True,
        text=True,
    )
    version_output = "\n".join(
        part for part in (result.stdout.strip(), result.stderr.strip()) if part
    )
    match = re.search(r"pdftotext version ([0-9.]+)", version_output)
    if result.returncode != 0 or match is None:
        raise TransferCandidateBuildError(
            f"cannot identify pdftotext version: {version_output!r}"
        )
    version = match.group(1)
    if version != PDFTOTEXT_VERSION:
        raise TransferCandidateBuildError(
            f"pdftotext {version} != frozen {PDFTOTEXT_VERSION}"
        )
    return version


def _extract_pages(
    source_pdf: Path,
    *,
    first_page: int,
    last_page: int,
) -> dict[int, bytes]:
    result = subprocess.run(
        (
            "pdftotext",
            "-f",
            str(first_page),
            "-l",
            str(last_page),
            "-layout",
            "-enc",
            "UTF-8",
            str(source_pdf),
            "-",
        ),
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        raise TransferCandidateBuildError(
            f"pdftotext failed for pages {first_page}-{last_page}"
        )
    segments = result.stdout.split(b"\x0c")
    if segments and segments[-1] == b"":
        segments.pop()
    expected_count = last_page - first_page + 1
    if len(segments) != expected_count:
        raise TransferCandidateBuildError(
            f"pdftotext returned {len(segments)} pages, expected {expected_count}"
        )
    pages: dict[int, bytes] = {}
    for offset, content in enumerate(segments):
        try:
            content.decode("utf-8")
        except UnicodeDecodeError as exc:
            raise TransferCandidateBuildError(
                f"page {first_page + offset} is not UTF-8"
            ) from exc
        pages[first_page + offset] = content
    return pages


def _metrics(content: bytes) -> dict[str, int]:
    text = content.decode("utf-8")
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    substantive_lines = [
        line
        for line in lines
        if not re.fullmatch(r"[0-9]{1,4}", line) and len(line) > 2
    ]
    nonspace = sum(not character.isspace() for character in text)
    alphabetic = sum(character.isalpha() for character in text)
    punctuation = sum(character in PUNCTUATION for character in text)
    line_end_hyphenations = sum(
        line.rstrip().endswith(("-", "—", "–")) for line in text.splitlines()
    )
    numbered_headings = sum(
        NUMBERED_HEADING.fullmatch(line) is not None for line in lines
    )
    edge_signals = 0
    if substantive_lines:
        first = substantive_lines[0]
        last = substantive_lines[-1]
        if first[:1].islower() or first.endswith((",", ";", ":", "-", "—", "–")):
            edge_signals += 1
        if last.endswith((",", ";", ":", "-", "—", "–")):
            edge_signals += 1
    hardness = (
        nonspace
        + punctuation * 6
        + line_end_hyphenations * 160
        + edge_signals * 250
        + numbered_headings * 100
    )
    return {
        "nonspace_characters": nonspace,
        "alphabetic_characters": alphabetic,
        "nonblank_lines": len(lines),
        "punctuation_characters": punctuation,
        "line_end_hyphenations": line_end_hyphenations,
        "page_edge_fragment_signals": edge_signals,
        "numbered_heading_candidates": numbered_headings,
        "mechanical_hardness_score": hardness,
    }


def _difficulty_signals(
    metrics: dict[str, int],
    *,
    stratum: str,
) -> list[str]:
    signals = [
        (
            "digest-random-baseline"
            if stratum == "random"
            else "mechanical-hardness-top-rank"
        )
    ]
    if metrics["nonspace_characters"] >= 3200:
        signals.append("dense-text")
    if metrics["punctuation_characters"] >= 180:
        signals.append("punctuation-rich")
    if metrics["line_end_hyphenations"] > 0:
        signals.append("line-end-hyphenation")
    if metrics["page_edge_fragment_signals"] > 0:
        signals.append("page-edge-fragment-risk")
    if metrics["numbered_heading_candidates"] > 0:
        signals.append("numbered-heading-candidate")
    return signals


def _candidate_limitations(stratum: str) -> list[str]:
    limitations = [
        (
            "pdftotext embedded layer is an automatic extraction candidate, "
            "not a diplomatic transcription"
        ),
        "whole-page scope may split a sentence or numbered unit",
        "work boundary and Russian expression identity remain unreviewed",
        "no human source review, target gold, semantic label, sign match, or relation exists",
        "local source content is not authorized for publication",
    ]
    limitations.append(
        (
            "digest rank controls bounded sampling but does not prove broader "
            "representativeness"
        )
        if stratum == "random"
        else (
            "mechanical score does not establish philosophical or semantic difficulty"
        )
    )
    return limitations


def _select_candidates(
    *,
    plan: dict[str, Any],
    boundary_map: dict[str, Any],
    source_pdf: Path,
) -> tuple[list[dict[str, Any]], dict[str, bytes], str]:
    file_sha256 = _sha256_path(source_pdf)
    if file_sha256 != EXPECTED_FILE_SHA256:
        raise TransferCandidateBuildError(
            f"source PDF sha256 {file_sha256} != {EXPECTED_FILE_SHA256}"
        )
    if plan.get("target_source", {}).get("file_sha256") != file_sha256:
        raise TransferCandidateBuildError("transfer target source digest drifted")
    if boundary_map.get("file_sha256") != file_sha256:
        raise TransferCandidateBuildError("work-boundary source digest drifted")
    members = {
        member.get("work_ref"): member
        for member in boundary_map.get("members", [])
        if isinstance(member, dict)
    }
    seed = _sha256_bytes(
        (
            f"{file_sha256}|{plan.get('transfer_plan_id')}|"
            "candidate-protocol-v1"
        ).encode("utf-8")
    )
    selected: list[dict[str, Any]] = []
    content_by_unit_id: dict[str, bytes] = {}
    for spec in WORK_SPECS:
        member = members.get(spec["work_ref"])
        if not isinstance(member, dict):
            raise TransferCandidateBuildError(
                f"work boundary missing for {spec['work_ref']}"
            )
        first_page = int(member["start_page"]) + BOUNDARY_TRIM_PAGES
        last_page = int(member["end_page"]) - BOUNDARY_TRIM_PAGES
        pages = _extract_pages(
            source_pdf,
            first_page=first_page,
            last_page=last_page,
        )
        eligible: list[dict[str, Any]] = []
        for page, content in pages.items():
            metrics = _metrics(content)
            if (
                metrics["alphabetic_characters"]
                < MINIMUM_ALPHABETIC_CHARACTERS
                or metrics["nonblank_lines"] < MINIMUM_NONBLANK_LINES
            ):
                continue
            random_rank_digest = _sha256_bytes(
                (
                    f"{seed}|{spec['work_ref']}|{page}|"
                    f"{_sha256_bytes(content)}"
                ).encode("utf-8")
            )
            eligible.append(
                {
                    "page": page,
                    "content": content,
                    "metrics": metrics,
                    "random_rank_digest": random_rank_digest,
                }
            )
        required = int(spec["random"]) + int(spec["hard"])
        if len(eligible) < required:
            raise TransferCandidateBuildError(
                f"{spec['work_ref']} has {len(eligible)} eligible pages, needs {required}"
            )
        random_rows = sorted(
            eligible,
            key=lambda row: (row["random_rank_digest"], row["page"]),
        )[: int(spec["random"])]
        random_pages = {row["page"] for row in random_rows}
        hard_rows = sorted(
            (row for row in eligible if row["page"] not in random_pages),
            key=lambda row: (
                -row["metrics"]["mechanical_hardness_score"],
                row["page"],
            ),
        )[: int(spec["hard"])]
        for stratum, rows in (("random", random_rows), ("hard", hard_rows)):
            for rank, row in enumerate(rows, start=1):
                page = int(row["page"])
                unit_id = (
                    f"tos-target-candidate-{spec['slug']}-p{page:04d}-"
                    f"{stratum}"
                )
                local_ref = LOCAL_CONTENT_ROOT / f"{unit_id}.txt"
                candidate = {
                    "unit_id": unit_id,
                    "anchor_ref": (
                        "tos.anchor.zarathustra-foundation-pilot-v1."
                        f"transfer-{spec['slug']}-p{page:04d}"
                    ),
                    "work_ref": spec["work_ref"],
                    "expression_ref": member["expression_ref"],
                    "item_ref": boundary_map["item_ref"],
                    "file_ref": boundary_map["file_id"],
                    "page": page,
                    "page_resource_id": f"pdf-page-{page:04d}",
                    "candidate_scope": "whole-page",
                    "stratum": stratum,
                    "selection_basis": (
                        "deterministic-digest-order"
                        if stratum == "random"
                        else "mechanical-layout-hardness"
                    ),
                    "selection_rank": rank,
                    "selection_metrics": row["metrics"],
                    "provisional_difficulty_signals": _difficulty_signals(
                        row["metrics"],
                        stratum=stratum,
                    ),
                    "source_content_ref": local_ref.as_posix(),
                    "source_content_sha256": _sha256_bytes(row["content"]),
                    "source_content_bytes": len(row["content"]),
                    "source_layer": "embedded-pdf-text-pdftotext-layout",
                    "source_review_status": "model_source_visible",
                    "source_review_scope": "content-bearing-page-and-route-only",
                    "target_gold_status": "not_started",
                    "frozen_before_variant_outputs": True,
                    "eligible_for_variant_execution": False,
                    "limitations": _candidate_limitations(stratum),
                }
                selected.append(candidate)
                content_by_unit_id[unit_id] = row["content"]
    selected.sort(
        key=lambda candidate: (
            0 if candidate["stratum"] == "random" else 1,
            next(
                index
                for index, spec in enumerate(WORK_SPECS)
                if spec["work_ref"] == candidate["work_ref"]
            ),
            candidate["selection_rank"],
        )
    )
    if len(selected) != 20:
        raise TransferCandidateBuildError(
            f"candidate selection produced {len(selected)} records, expected 20"
        )
    return selected, content_by_unit_id, seed


def _build_anchors(
    candidates: list[dict[str, Any]],
    *,
    file_sha256: str,
) -> list[dict[str, Any]]:
    return [
        {
            "schema_version": "tos_source_anchor_v1",
            "anchor_id": candidate["anchor_ref"],
            "item_id": candidate["item_ref"],
            "file_id": candidate["file_ref"],
            "file_sha256": file_sha256,
            "passage_id": None,
            "selectors": [
                {
                    "type": "page_region",
                    "page": candidate["page"],
                    "x": 0,
                    "y": 0,
                    "width": 1,
                    "height": 1,
                    "coordinate_space": "normalized_0_1",
                }
            ],
            "selector_method": {
                "maker_type": "mixed",
                "method": (
                    "deterministic pre-output page selection plus model-visible "
                    "content-bearing confirmation"
                ),
                "version": "1",
                "configuration_ref": PLAN_PATH.as_posix(),
            },
            "status": "proposed",
            "provenance_event_ref": EVENT_ID,
            "anchor_version": 1,
            "supersedes_anchor_ref": None,
            "review_ref": None,
        }
        for candidate in candidates
    ]


def _update_plan(
    *,
    plan: dict[str, Any],
    candidates: list[dict[str, Any]],
    seed: str,
    builder_sha256: str,
    pdftotext_version: str,
) -> dict[str, Any]:
    if plan.get("target_units") != []:
        raise TransferCandidateBuildError(
            "refusing to prepare candidates over non-empty eligible target_units"
        )
    result = json.loads(json.dumps(plan))
    result["schema_version"] = "tos_golden_kernel_transfer_plan_v2"
    result["frozen_at"] = PREPARED_AT
    result["candidate_preparation"] = {
        "protocol_version": 1,
        "prepared_at": PREPARED_AT,
        "builder": {
            "ref": BUILDER_PATH.as_posix(),
            "sha256": builder_sha256,
        },
        "source_extraction": {
            "tool": "pdftotext",
            "version": pdftotext_version,
            "mode": "layout-utf8-whole-page",
            "network_access": False,
            "source_text_tracked": False,
        },
        "deterministic_randomization_key_sha256": seed,
        "work_quotas": [
            {
                "work_ref": spec["work_ref"],
                "random": spec["random"],
                "hard": spec["hard"],
            }
            for spec in WORK_SPECS
        ],
        "content_filter": {
            "boundary_trim_pages": BOUNDARY_TRIM_PAGES,
            "minimum_alphabetic_characters": MINIMUM_ALPHABETIC_CHARACTERS,
            "minimum_nonblank_lines": MINIMUM_NONBLANK_LINES,
            "random_method": "lowest-sha256-rank-per-work-before-variant-outputs",
            "hard_method": (
                "highest-mechanical-layout-score-per-work-before-variant-outputs"
            ),
            "hard_excludes_random_pages": True,
        },
        "variant_outputs_visible": False,
        "human_review_performed": False,
        "model_source_visible_review": {
            "reviewer_ref": "model:codex",
            "candidate_count": 20,
            "scope": (
                "whole-page-is-content-bearing-and-not-title-or-reference-matter"
            ),
            "status": "completed_with_limits",
        },
        "authority": "prepared-local-candidate-soil-only",
    }
    result["candidate_target_units"] = candidates
    result["kernel_evidence_gate"]["blockers"] = [
        "kernel German source acceptance is 0 of 30 translation units",
        "kernel human-double-checked gold is 0 of 15 candidate units",
        "human-accepted Zarathustra sign packets do not exist",
        "human-accepted Zarathustra translation packets do not exist",
        (
            "twenty content-bearing target candidates are frozen locally but "
            "target-text human gold is 0 of 20"
        ),
    ]
    for unit in result.get("scouting_units", []):
        if not isinstance(unit, dict):
            continue
        limitations = [
            limitation
            for limitation in unit.get("limitations", [])
            if limitation
            != "work identity has not yet been promoted into a standalone ToS Work record"
        ]
        replacement = (
            "standalone Work and Expression records now exist, but their "
            "source boundaries and identities remain unreviewed"
        )
        if replacement not in limitations:
            limitations.append(replacement)
        unit["limitations"] = limitations
    result["result"]["conclusion"] = (
        "No A/B/C semantic transfer was run. Twenty private whole-page target "
        "candidates are now selected and digest-frozen before variant outputs, "
        "but they remain machine-prepared, human-unreviewed, without target "
        "gold, and ineligible for execution; therefore no benefit, harm, or "
        "semantic-transfer claim exists."
    )
    result["provenance_event_ref"] = EVENT_ID
    result["plan_version"] = 2
    return result


def _build_event(
    *,
    plan_rendered: str,
    anchor_rendered: str,
    candidates: list[dict[str, Any]],
    builder_sha256: str,
    boundary_map_sha256: str,
    schema_sha256: str,
    gold_assurance_sha256: str,
) -> dict[str, Any]:
    outputs = [
        {
            "ref": PLAN_PATH.as_posix(),
            "role": "blocked-transfer-plan-with-private-ineligible-candidate-soil",
            "sha256": _sha256_bytes(plan_rendered.encode("utf-8")),
        },
        {
            "ref": ANCHOR_PATH.as_posix(),
            "role": "proposed-whole-page-transfer-candidate-anchors",
            "sha256": _sha256_bytes(anchor_rendered.encode("utf-8")),
        },
    ]
    outputs.extend(
        {
            "ref": candidate["source_content_ref"],
            "role": "gitignored-local-only-pdftotext-page-candidate",
            "sha256": candidate["source_content_sha256"],
        }
        for candidate in candidates
    )
    return {
        "schema_version": "tos_provenance_event_v1",
        "event_id": EVENT_ID,
        "event_type": "segmentation",
        "started_at": PREPARED_AT,
        "ended_at": PREPARED_AT,
        "agent_refs": [
            "model:codex",
            "software:pdftotext-26.01.0",
            "software:python-stdlib",
        ],
        "inputs": [
            {
                "ref": PREVIOUS_EVENT_ID,
                "role": "superseded-title-page-only-transfer-plan",
                "sha256": None,
            },
            {
                "ref": (
                    "tos.file.sha256."
                    "ce16b68089dee0fc53a31d9e97723991b292dd8835c43bbbb40a9373c9a436aa"
                ),
                "role": "fixity-verified-private-target-source-pdf",
                "sha256": EXPECTED_FILE_SHA256,
            },
            {
                "ref": BOUNDARY_MAP_PATH.as_posix(),
                "role": "unreviewed-cross-work-page-boundary-map",
                "sha256": boundary_map_sha256,
            },
            {
                "ref": SCHEMA_PATH.as_posix(),
                "role": "candidate-aware-fail-closed-transfer-contract",
                "sha256": schema_sha256,
            },
            {
                "ref": GOLD_ASSURANCE_PATH.as_posix(),
                "role": "solo-human-plus-ai-triggered-review-authority",
                "sha256": gold_assurance_sha256,
            },
        ],
        "outputs": outputs,
        "method": {
            "maker_type": "mixed",
            "name": "pre-output-private-transfer-candidate-freeze",
            "version": "1",
            "artifact_digest": builder_sha256,
            "runtime": "Python standard library plus pdftotext 26.01.0",
            "device": "abyss-machine",
            "configuration": {
                "network_access": False,
                "variant_outputs_visible": False,
                "candidate_count": 20,
                "random_candidate_count": 10,
                "hard_candidate_count": 10,
                "model_source_visible_content_check": True,
                "human_review_performed": False,
                "eligible_target_units_created": 0,
                "target_gold_created": 0,
                "semantic_labels_created": 0,
                "source_text_tracked": False,
            },
            "prompt_or_instruction_ref": (
                "ToS/research-packets/foundation-laboratory-2026-07/"
                "GOLDEN_KERNEL_TRANSFER_REPORT.md"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            (
                "the embedded PDF text is an automatic extraction candidate "
                "and not a diplomatic transcription"
            ),
            (
                "whole-page anchors and cross-work boundaries remain proposed "
                "and human-unreviewed"
            ),
            (
                "mechanical difficulty signals are not semantic or "
                "philosophical classifications"
            ),
            (
                "private source content remains gitignored and is not "
                "authorized for publication"
            ),
            (
                "no human debt, target gold, accepted sign, semantic relation, "
                "variant run, winner, benefit claim, or promotion was created"
            ),
        ],
        "receipt_refs": [],
        "rights_basis_ref": RIGHTS_PATH.as_posix(),
        "event_version": 3,
        "supersedes_event_ref": PREVIOUS_EVENT_ID,
    }


def _write_or_check(path: Path, expected: bytes, *, check: bool) -> bool:
    if check:
        try:
            actual = path.read_bytes()
        except OSError:
            return False
        return actual == expected
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(expected)
    return True


def build_outputs(
    *,
    repo_root: Path,
    payload_source_root: Path,
) -> tuple[
    dict[str, Any],
    list[dict[str, Any]],
    list[dict[str, Any]],
    dict[str, bytes],
]:
    plan = _read_json(repo_root / PLAN_PATH)
    boundary_map = _read_json(repo_root / BOUNDARY_MAP_PATH)
    source_pdf = payload_source_root / SOURCE_PDF_PATH
    if not source_pdf.is_file():
        raise TransferCandidateBuildError(
            f"explicit private source PDF is missing: {source_pdf}"
        )
    pdftotext_version = _pdftotext_version()
    builder_sha256 = _sha256_path(repo_root / BUILDER_PATH)
    candidates, content_by_unit_id, seed = _select_candidates(
        plan=plan,
        boundary_map=boundary_map,
        source_pdf=source_pdf,
    )
    updated_plan = _update_plan(
        plan=plan,
        candidates=candidates,
        seed=seed,
        builder_sha256=builder_sha256,
        pdftotext_version=pdftotext_version,
    )
    anchors = _build_anchors(
        candidates,
        file_sha256=EXPECTED_FILE_SHA256,
    )
    plan_rendered = _render_json(updated_plan)
    anchor_rendered = _render_jsonl(anchors)
    event = _build_event(
        plan_rendered=plan_rendered,
        anchor_rendered=anchor_rendered,
        candidates=candidates,
        builder_sha256=builder_sha256,
        boundary_map_sha256=_sha256_path(repo_root / BOUNDARY_MAP_PATH),
        schema_sha256=_sha256_path(repo_root / SCHEMA_PATH),
        gold_assurance_sha256=_sha256_path(repo_root / GOLD_ASSURANCE_PATH),
    )
    events = [
        record
        for record in _read_jsonl(repo_root / PROVENANCE_PATH)
        if record.get("event_id") != EVENT_ID
    ]
    events.append(event)
    return updated_plan, anchors, events, content_by_unit_id


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=REPO_ROOT,
        help="Tree of Sophia repository root for tracked outputs",
    )
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        required=True,
        help=(
            "explicit Tree of Sophia source root holding private payloads and "
            "ignored local-content outputs"
        ),
    )
    parser.add_argument(
        "--selection-only",
        action="store_true",
        help="print page numbers and non-text metrics without writing",
    )
    parser.add_argument(
        "--confirm-model-source-visible-review",
        action="store_true",
        help=(
            "required for first write after all selected pages were visibly "
            "confirmed as content-bearing"
        ),
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when tracked or private outputs differ without writing",
    )
    args = parser.parse_args(argv)
    repo_root = args.repo_root.resolve()
    payload_source_root = args.payload_source_root.resolve()
    try:
        plan, anchors, events, content_by_unit_id = build_outputs(
            repo_root=repo_root,
            payload_source_root=payload_source_root,
        )
    except TransferCandidateBuildError as exc:
        print(f"transfer-candidate preparation failed: {exc}", file=sys.stderr)
        return 1
    if args.selection_only:
        selection = [
            {
                "unit_id": candidate["unit_id"],
                "work_ref": candidate["work_ref"],
                "page": candidate["page"],
                "stratum": candidate["stratum"],
                "selection_rank": candidate["selection_rank"],
                "selection_metrics": candidate["selection_metrics"],
            }
            for candidate in plan["candidate_target_units"]
        ]
        print(json.dumps(selection, ensure_ascii=False, indent=2))
        return 0
    if not args.check and not args.confirm_model_source_visible_review:
        print(
            "refusing to write before --confirm-model-source-visible-review",
            file=sys.stderr,
        )
        return 1
    outputs: list[tuple[Path, bytes]] = [
        (
            repo_root / PLAN_PATH,
            _render_json(plan).encode("utf-8"),
        ),
        (
            repo_root / ANCHOR_PATH,
            _render_jsonl(anchors).encode("utf-8"),
        ),
        (
            repo_root / PROVENANCE_PATH,
            _render_jsonl(events).encode("utf-8"),
        ),
    ]
    candidate_by_id = {
        candidate["unit_id"]: candidate
        for candidate in plan["candidate_target_units"]
    }
    for unit_id, content in content_by_unit_id.items():
        local_ref = Path(candidate_by_id[unit_id]["source_content_ref"])
        outputs.append((payload_source_root / local_ref, content))
    mismatches = [
        path
        for path, expected in outputs
        if not _write_or_check(path, expected, check=args.check)
    ]
    if mismatches:
        action = "differ" if args.check else "failed to write"
        for path in mismatches:
            print(f"{action}: {path}", file=sys.stderr)
        return 1
    if args.check:
        print("[ok] golden-kernel transfer candidate outputs match")
    else:
        print(
            "[ok] froze 20 private transfer candidates; "
            "0 eligible target units and 0 target-gold units"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
