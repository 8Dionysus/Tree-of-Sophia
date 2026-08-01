#!/usr/bin/env python3
"""Pair independently materialized Jenseits number labels without text.

This release-safe builder intersects the stable structural unit keys in the
Naumann 1886 source map and Polilov/Mysl 1996 target map. It does not read
either local payload, compare passages, or infer translation alignment.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
ALIGNMENT_DIR = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/alignments/structure/"
    "naumann-1886-polilov-mysl-1996"
)
SOURCE_MAP_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/expressions/de-naumann-1886/"
    "editions/leipzig-c-g-naumann-1886/items/"
    "internet-archive-google-harvard-scan-pdf/structure/"
    "numbered-unit-page-map.json"
)
TARGET_MAP_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/expressions/ru-polilov-mysl-1996/"
    "structure/mysl-1996-volume-2-operator-pdf/"
    "numbered-unit-page-map.json"
)
SOURCE_RIGHTS_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/expressions/de-naumann-1886/"
    "editions/leipzig-c-g-naumann-1886/items/"
    "internet-archive-google-harvard-scan-pdf/rights.json"
)
TARGET_RIGHTS_PATH = Path(
    "ToS/source-witnesses/collections/friedrich-nietzsche/"
    "works-in-two-volumes-volume-2-mysl-1996/"
    "editions/moscow-mysl-1996-volume-2/items/operator-pdf/rights.json"
)
MAP_PATH = ALIGNMENT_DIR / "numbered-unit-label-correspondence.json"
PROVENANCE_PATH = (
    ALIGNMENT_DIR / "provenance.numbered-unit-label-correspondence.jsonl"
)
SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "parallel-numbered-unit-label-map.schema.json"
)
MAP_ID = (
    "tos.parallel-numbered-unit-label-map.friedrich-nietzsche."
    "jenseits-von-gut-und-boese.naumann-1886-to-polilov-mysl-1996"
)
EVENT_ID = (
    "tos.event.parallel-numbered-unit-label-map.friedrich-nietzsche."
    "jenseits-von-gut-und-boese.rights-refresh.2026-08-01"
)
SUPERSEDED_EVENT_ID = (
    "tos.event.parallel-numbered-unit-label-map.friedrich-nietzsche."
    "jenseits-von-gut-und-boese.naumann-1886-to-polilov-mysl-1996."
    "2026-07-29"
)
AUTHORITY_BOUNDARY = (
    "mechanical pairing of identical structural number-label keys already "
    "materialized independently in two exact witness maps; no text comparison, "
    "exact passage boundary, source-to-target passage alignment, translation "
    "correspondence, equivalence or quality, semantics, rights clearance, or "
    "canon authority"
)
DOES_NOT_ESTABLISH = [
    "source_text",
    "target_text",
    "exact_line_boundaries",
    "exact_passage_end_boundaries",
    "source_to_target_passage_alignment",
    "translation_correspondence",
    "translation_equivalence",
    "translation_quality",
    "textual_identity",
    "semantics",
    "rights_clearance",
    "canon_promotion",
]


class LabelCorrespondenceBuildError(RuntimeError):
    """Raised when the two tracked input maps no longer close exactly."""


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
        raise LabelCorrespondenceBuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise LabelCorrespondenceBuildError(
            f"JSON root is not an object: {path}"
        )
    return payload


def _render_json(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _render_jsonl(payloads: list[dict[str, Any]]) -> str:
    return "".join(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"
        for payload in payloads
    )


def _units_by_key(payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    units = [
        unit
        for unit in payload.get("unit_starts", [])
        if isinstance(unit, dict) and isinstance(unit.get("unit_key"), str)
    ]
    by_key = {unit["unit_key"]: unit for unit in units}
    if len(by_key) != len(units):
        raise LabelCorrespondenceBuildError("numbered-unit keys are duplicated")
    return by_key


def _witness_binding(
    payload: dict[str, Any],
    *,
    map_path: Path,
    map_digest: str,
) -> dict[str, Any]:
    scan_file = payload.get("scan_file", {})
    if not isinstance(scan_file, dict):
        raise LabelCorrespondenceBuildError(
            f"scan_file is invalid in {map_path}"
        )
    return {
        "expression_ref": payload["expression_ref"],
        "edition_ref": payload["edition_ref"],
        "item_ref": payload["item_ref"],
        "file_ref": scan_file["file_ref"],
        "file_sha256": scan_file["file_sha256"],
        "numbered_unit_map_ref": map_path.as_posix(),
        "numbered_unit_map_sha256": map_digest,
    }


def build_outputs(
    *,
    repo_root: Path,
    event_at: str,
) -> tuple[str, str]:
    source_map = _read_json(repo_root / SOURCE_MAP_PATH)
    target_map = _read_json(repo_root / TARGET_MAP_PATH)
    source_units = _units_by_key(source_map)
    target_units = _units_by_key(target_map)

    if source_map.get("work_ref") != target_map.get("work_ref"):
        raise LabelCorrespondenceBuildError("source and target work refs differ")
    shared_keys = [
        unit["unit_key"]
        for unit in target_map.get("unit_starts", [])
        if isinstance(unit, dict) and unit.get("unit_key") in source_units
    ]
    source_only = set(source_units) - set(target_units)
    target_only = set(target_units) - set(source_units)
    if (
        len(source_units) != 299
        or len(target_units) != 298
        or len(shared_keys) != 298
        or source_only != {"237a"}
        or target_only
    ):
        raise LabelCorrespondenceBuildError(
            "numbered-unit intersection drifted: "
            f"source={len(source_units)}, target={len(target_units)}, "
            f"shared={len(shared_keys)}, source_only={sorted(source_only)}, "
            f"target_only={sorted(target_only)}"
        )

    pairings = [
        {
            "sequence": sequence,
            "unit_key": unit_key,
            "source_anchor_ref": source_units[unit_key]["anchor_ref"],
            "source_pdf_page": source_units[unit_key]["pdf_page"],
            "target_anchor_ref": target_units[unit_key]["anchor_ref"],
            "target_pdf_page": target_units[unit_key]["pdf_page"],
            "basis": "shared_materialized_number_label_key",
            "status": "proposed",
            "human_review_performed": False,
            "translation_alignment_claimed": False,
        }
        for sequence, unit_key in enumerate(shared_keys, start=1)
    ]
    source_digest = _sha256_path(repo_root / SOURCE_MAP_PATH)
    target_digest = _sha256_path(repo_root / TARGET_MAP_PATH)
    source_rights_digest = _sha256_path(repo_root / SOURCE_RIGHTS_PATH)
    target_rights_digest = _sha256_path(repo_root / TARGET_RIGHTS_PATH)
    map_payload = {
        "$schema": SCHEMA_REF,
        "schema_version": "tos_parallel_numbered_unit_label_map_v1",
        "map_id": MAP_ID,
        "work_ref": source_map["work_ref"],
        "source_witness": _witness_binding(
            source_map,
            map_path=SOURCE_MAP_PATH,
            map_digest=source_digest,
        ),
        "target_witness": _witness_binding(
            target_map,
            map_path=TARGET_MAP_PATH,
            map_digest=target_digest,
        ),
        "rights_basis": [
            {
                "role": "source",
                "ref": SOURCE_RIGHTS_PATH.as_posix(),
                "sha256": source_rights_digest,
            },
            {
                "role": "target",
                "ref": TARGET_RIGHTS_PATH.as_posix(),
                "sha256": target_rights_digest,
            },
        ],
        "map_authority": "mechanical_shared_number_label_candidate_only",
        "source_text_included": False,
        "method": {
            "name": "exact-shared-structural-label-key-intersection",
            "version": "1",
            "maker_type": "software",
            "local_payloads_read": False,
            "pairing_key": "unit_key",
            "requires_materialized_label_in_both_maps": True,
            "source_to_target_text_compared": False,
            "translation_alignment_inferred": False,
            "semantic_matching_used": False,
            "no_source_text_emitted": True,
        },
        "pairings": pairings,
        "unpaired_units": [
            {
                "side": "source",
                "unit_key": "237a",
                "reason": (
                    "target_witness_has_corresponding_prose_without_"
                    "repeated_number_label"
                ),
                "target_unit_materialized": False,
                "translation_alignment_claimed": False,
                "status": "proposed",
            }
        ],
        "summary": {
            "source_numbered_unit_count": len(source_units),
            "target_numbered_unit_count": len(target_units),
            "shared_materialized_label_count": len(shared_keys),
            "pairing_count": len(pairings),
            "source_only_unit_keys": ["237a"],
            "target_only_unit_keys": [],
            "all_pairing_statuses": ["proposed"],
            "human_review_performed": False,
            "translation_alignment_claimed": False,
        },
        "provenance_ref": PROVENANCE_PATH.as_posix(),
        "provenance_event_ref": EVENT_ID,
        "map_version": 2,
        "supersedes_map_ref": None,
        "authority_boundary": AUTHORITY_BOUNDARY,
        "does_not_establish": DOES_NOT_ESTABLISH,
    }
    rendered_map = _render_json(map_payload)
    provenance = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": EVENT_ID,
        "event_type": "alignment",
        "started_at": event_at,
        "ended_at": event_at,
        "agent_refs": ["software:python-standard-library"],
        "inputs": [
            {
                "ref": SOURCE_MAP_PATH.as_posix(),
                "role": "tracked-source-numbered-label-map",
                "sha256": source_digest,
            },
            {
                "ref": TARGET_MAP_PATH.as_posix(),
                "role": "tracked-target-numbered-label-map",
                "sha256": target_digest,
            },
            {
                "ref": SOURCE_RIGHTS_PATH.as_posix(),
                "role": "source-rights-basis",
                "sha256": source_rights_digest,
            },
            {
                "ref": TARGET_RIGHTS_PATH.as_posix(),
                "role": "target-rights-basis",
                "sha256": target_rights_digest,
            },
        ],
        "outputs": [
            {
                "ref": MAP_PATH.as_posix(),
                "role": "tracked-text-free-shared-number-label-pairing-candidates",
                "sha256": _sha256_bytes(rendered_map.encode("utf-8")),
            }
        ],
        "method": {
            "maker_type": "software",
            "name": "exact-shared-structural-label-key-intersection",
            "version": "1",
            "artifact_digest": None,
            "runtime": "Python standard library",
            "device": None,
            "configuration": {
                "pairing_key": "unit_key",
                "expected_pairing_count": 298,
                "source_only_unit_keys": ["237a"],
                "local_payloads_read": False,
                "source_to_target_text_compared": False,
                "translation_alignment_inferred": False,
            },
            "prompt_or_instruction_ref": (
                "ToS/doctrine/CORPUS_FOUNDATION.md#address-law"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            (
                "The 298 pairs assert only that the same structural number-label "
                "key is independently materialized in both witness maps."
            ),
            (
                "No source or target text was read, compared, transcribed, or "
                "accepted by this pairing route."
            ),
            (
                "Shared numbering does not establish exact passage alignment, "
                "translation correspondence, equivalence, quality, or semantics."
            ),
            (
                "Source-only 237a remains unpaired because the target does not "
                "materialize a repeated 237/237a label."
            ),
        ],
        "receipt_refs": [MAP_PATH.as_posix()],
        "rights_basis_ref": None,
        "event_version": 1,
        "supersedes_event_ref": SUPERSEDED_EVENT_ID,
    }
    return rendered_map, _render_jsonl([provenance])


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Build text-free shared-number-label pairing candidates for "
            "the Jenseits source and target maps."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument(
        "--event-at",
        default="2026-08-01T06:22:00-06:00",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    rendered_map, rendered_provenance = build_outputs(
        repo_root=repo_root,
        event_at=args.event_at,
    )
    outputs = {
        repo_root / MAP_PATH: rendered_map,
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
        print("[ok] Jenseits numbered-label correspondence matches tracked maps")
        return 0

    for path, rendered in outputs.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(rendered, encoding="utf-8")
    print("[ok] wrote 298 proposed shared-number-label pairing candidates")
    print(
        "[boundary] no payload, text comparison, passage alignment, "
        "translation equivalence, quality, or semantics"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
