#!/usr/bin/env python3
"""Project text-free co-availability over frozen transfer source/target routes.

The projection joins candidate identities and mechanical statuses only. It
does not read private content and does not create a bilingual passage pair,
alignment, accepted text, eligibility, gold, human work, or semantics.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
GOLD_ROOT = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1"
)
TARGET_PATH = GOLD_ROOT / "transfer-target-passage-candidates.v1.json"
SOURCE_PATH = GOLD_ROOT / "transfer-source-passage-candidates.v1.json"
OUTPUT_PATH = GOLD_ROOT / "transfer-route-readiness.v1.json"
PROVENANCE_PATH = GOLD_ROOT / "transfer-provenance.jsonl"
SCHEMA_PATH = Path("ToS/contracts/transfer-route-readiness-projection.schema.json")
SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "transfer-route-readiness-projection.schema.json"
)
PROJECTION_ID = "tos.transfer-route-readiness.golden-kernel-v1"
EVENT_ID = "tos.event.projection.golden-kernel-transfer-route-readiness-v1.2026-08-09"
EVENT_AT = "2026-08-09T00:45:00-06:00"


class ReadinessBuildError(RuntimeError):
    """Raised when the source/target candidate closure drifts."""


def _sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ReadinessBuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise ReadinessBuildError(f"JSON root is not an object: {path}")
    return payload


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise ReadinessBuildError(f"cannot read {path}: {exc}") from exc
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ReadinessBuildError(
                f"cannot read {path}:{line_number}: {exc}"
            ) from exc
        if not isinstance(record, dict):
            raise ReadinessBuildError(f"{path}:{line_number} is not an object")
        records.append(record)
    return records


def _render_json(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _render_jsonl(payloads: list[dict[str, Any]]) -> str:
    return "".join(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"
        for payload in payloads
    )


def _input_ref(repo_root: Path, path: Path) -> dict[str, str]:
    return {"ref": path.as_posix(), "sha256": _sha256_path(repo_root / path)}


def _route_id(target_id: str) -> str:
    return target_id.replace(
        "tos-target-passage-candidate-", "tos-transfer-route-readiness-"
    )


def build_projection(repo_root: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    target_payload = _read_json(repo_root / TARGET_PATH)
    source_payload = _read_json(repo_root / SOURCE_PATH)
    targets = target_payload.get("passage_candidates", [])
    sources = source_payload.get("passage_candidates", [])
    if len(targets) != 35 or len(sources) != 35:
        raise ReadinessBuildError("candidate sets must each contain 35 routes")
    if (
        target_payload.get("effects", {}).get(
            "source_to_target_passage_alignment_created"
        )
        or source_payload.get("effects", {}).get(
            "source_to_target_passage_alignment_created"
        )
        or target_payload.get("summary", {}).get("eligible_target_unit_count") != 0
        or source_payload.get("summary", {}).get("eligible_target_unit_count") != 0
    ):
        raise ReadinessBuildError("candidate-set authority boundary drifted")
    sources_by_target = {
        source["target_passage_candidate_id"]: source for source in sources
    }
    if len(sources_by_target) != 35 or set(sources_by_target) != {
        target["passage_candidate_id"] for target in targets
    }:
        raise ReadinessBuildError("source candidate set does not close target frame")

    routes: list[dict[str, Any]] = []
    page_rows: dict[str, dict[str, Any]] = {}
    for target in targets:
        source = sources_by_target[target["passage_candidate_id"]]
        if (
            source["status"] != "materialized-layer-exact-candidate"
            or source["qualified_unit_key"] != target["qualified_unit_key"]
            or source["work_ref"] != target["work_ref"]
            or source["frozen_page_candidate_id"] != target["frozen_page_candidate_id"]
            or not source["private_content_ref"]
            or not source["private_content_sha256"]
            or not target["private_content_ref"]
            or not target["private_content_sha256"]
            or source["accepted_source_text"]
            or source["source_to_target_alignment_created"]
            or source["eligible_for_variant_execution"]
            or target["accepted_target_text"]
            or target["eligible_for_variant_execution"]
            or target["target_gold_status"] != "not_started"
            or target["human_review_performed"]
        ):
            raise ReadinessBuildError(
                f"candidate authority or availability drifted: "
                f"{target['passage_candidate_id']}"
            )
        target_status = target["status"]
        if target_status == "proposed-intersecting-layer-exact":
            intersection = True
            readiness_class = "dual-private-layer-candidates-frozen-page-intersecting"
        elif target_status == "rejected-nonintersecting-layer-exact":
            intersection = False
            readiness_class = "dual-private-layer-candidates-target-nonintersecting"
        else:
            raise ReadinessBuildError(f"unsupported target status: {target_status}")
        if target["candidate_page_intersects_passage"] is not intersection:
            raise ReadinessBuildError(
                f"target intersection/status drifted: {target['passage_candidate_id']}"
            )
        route_id = _route_id(target["passage_candidate_id"])
        route = {
            "route_readiness_id": route_id,
            "target_passage_candidate_id": target["passage_candidate_id"],
            "source_passage_candidate_id": source["source_passage_candidate_id"],
            "frozen_page_candidate_id": target["frozen_page_candidate_id"],
            "work_ref": target["work_ref"],
            "qualified_unit_key": target["qualified_unit_key"],
            "source_candidate_status": source["status"],
            "source_boundary_layer": source["boundary_layer"],
            "target_candidate_status": target_status,
            "target_boundary_layer": target["source_layer"],
            "readiness_class": readiness_class,
            "source_candidate_available": True,
            "target_candidate_available": True,
            "frozen_page_intersection": intersection,
            "accepted_source_or_target_text": False,
            "source_to_target_passage_alignment": False,
            "eligible_for_variant_execution": False,
            "target_gold": False,
            "human_review_performed": False,
        }
        routes.append(route)
        page_id = target["frozen_page_candidate_id"]
        page = page_rows.setdefault(
            page_id,
            {
                "frozen_page_candidate_id": page_id,
                "route_readiness_refs": [],
                "dual_intersecting_route_count": 0,
                "dual_target_nonintersecting_route_count": 0,
                "status": "has-dual-private-intersecting-route",
            },
        )
        page["route_readiness_refs"].append(route_id)
        count_field = (
            "dual_intersecting_route_count"
            if intersection
            else "dual_target_nonintersecting_route_count"
        )
        page[count_field] += 1

    frozen_pages = list(page_rows.values())
    if len(frozen_pages) != 20 or any(
        page["dual_intersecting_route_count"] < 1 for page in frozen_pages
    ):
        raise ReadinessBuildError(
            "all twenty frozen pages must retain at least one dual intersecting route"
        )
    intersecting = sum(route["frozen_page_intersection"] for route in routes)
    nonintersecting = len(routes) - intersecting
    if intersecting != 32 or nonintersecting != 3:
        raise ReadinessBuildError(
            f"expected 32 intersecting / 3 nonintersecting, got "
            f"{intersecting} / {nonintersecting}"
        )

    projection = {
        "$schema": SCHEMA_REF,
        "schema_version": "tos_transfer_route_readiness_projection_v1",
        "projection_id": PROJECTION_ID,
        "target_candidate_set": _input_ref(repo_root, TARGET_PATH),
        "source_candidate_set": _input_ref(repo_root, SOURCE_PATH),
        "routes": routes,
        "frozen_pages": frozen_pages,
        "summary": {
            "conservative_route_count": 35,
            "source_candidate_available_route_count": 35,
            "target_candidate_available_route_count": 35,
            "dual_candidate_available_route_count": 35,
            "dual_candidate_frozen_page_intersection_count": intersecting,
            "dual_candidate_target_nonintersection_count": nonintersecting,
            "frozen_page_count": 20,
            "frozen_pages_with_dual_intersecting_route_count": 20,
            "source_to_target_alignment_count": 0,
            "eligible_target_unit_count": 0,
            "target_gold_count": 0,
            "human_review_count": 0,
        },
        "effects": {
            "readiness_projection_created": True,
            "tracked_source_or_target_text_created": False,
            "source_to_target_passage_alignment_created": False,
            "translation_alignment_created": False,
            "target_unit_eligible": False,
            "target_gold_created": False,
            "semantic_work_opened": False,
            "human_work_scheduled": False,
            "publication_authorized": False,
            "canon_effect": False,
        },
        "provenance_event_ref": EVENT_ID,
        "status": "mechanically-prepared-ineligible",
        "authority_boundary": (
            "text-free co-availability over independently materialized source "
            "and target candidates only; shared structural route identity and "
            "frozen-page intersection create no bilingual passage pair, "
            "alignment, accepted text, eligibility, gold, human task, semantic "
            "claim, publication authority, or canon effect"
        ),
        "does_not_establish": [
            "accepted_german",
            "accepted_russian",
            "diplomatic_transcription",
            "source_to_target_passage_alignment",
            "translation_correspondence",
            "translation_equivalence",
            "translation_quality",
            "eligible_target_unit",
            "target_gold",
            "human_review",
            "semantic_relation",
            "publication_permission",
            "canon_promotion",
        ],
    }
    rendered = _render_json(projection).encode("utf-8")
    event_inputs = [
        {**_input_ref(repo_root, TARGET_PATH), "role": "target-candidate-set"},
        {**_input_ref(repo_root, SOURCE_PATH), "role": "source-candidate-set"},
        {**_input_ref(repo_root, SCHEMA_PATH), "role": "readiness-contract"},
        {
            **_input_ref(
                repo_root,
                Path("scripts/build_golden_kernel_transfer_route_readiness.py"),
            ),
            "role": "readiness-builder",
        },
    ]
    event = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": EVENT_ID,
        "event_type": "export",
        "started_at": EVENT_AT,
        "ended_at": EVENT_AT,
        "agent_refs": ["model:codex", "software:python-standard-library"],
        "inputs": event_inputs,
        "outputs": [
            {
                "ref": OUTPUT_PATH.as_posix(),
                "role": "tracked-text-free-transfer-route-readiness-projection",
                "sha256": _sha256_bytes(rendered),
            }
        ],
        "method": {
            "maker_type": "software",
            "name": "candidate-identity-and-mechanical-status-coavailability",
            "version": "1",
            "artifact_digest": _sha256_path(
                repo_root / "scripts/build_golden_kernel_transfer_route_readiness.py"
            ),
            "runtime": "Python standard library",
            "device": "abyss-machine",
            "configuration": projection["summary"],
            "prompt_or_instruction_ref": (
                "ToS/research-packets/foundation-laboratory-2026-07/"
                "GOLDEN_KERNEL_TRANSFER_REPORT.md"
            ),
        },
        "status": "completed_with_warnings",
        "warnings": [
            "co-availability is not source-to-target passage alignment",
            "frozen-page intersection is not accepted target text or target gold",
            "no private source or target content was read or copied",
            "no eligibility, human task, semantics, publication, or canon effect was created",
        ],
        "receipt_refs": [OUTPUT_PATH.as_posix()],
        "rights_basis_ref": None,
        "event_version": 1,
        "supersedes_event_ref": None,
    }
    return projection, event


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)
    repo_root = args.repo_root.resolve()
    try:
        projection, event = build_projection(repo_root)
        expected_output = _render_json(projection)
        events = _read_jsonl(repo_root / PROVENANCE_PATH)
        event_indexes = [
            index
            for index, record in enumerate(events)
            if record.get("event_id") == EVENT_ID
        ]
        if len(event_indexes) > 1:
            raise ReadinessBuildError("readiness provenance event is duplicated")
        if event_indexes:
            events[event_indexes[0]] = event
        else:
            events.append(event)
        expected_provenance = _render_jsonl(events)
        if args.check:
            if (repo_root / OUTPUT_PATH).read_text(
                encoding="utf-8"
            ) != expected_output or (repo_root / PROVENANCE_PATH).read_text(
                encoding="utf-8"
            ) != expected_provenance:
                raise ReadinessBuildError("readiness projection or provenance drifted")
        else:
            (repo_root / OUTPUT_PATH).write_text(expected_output, encoding="utf-8")
            (repo_root / PROVENANCE_PATH).write_text(
                expected_provenance, encoding="utf-8"
            )
    except (OSError, ReadinessBuildError) as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1
    print(
        "[ok] 35 dual private candidate routes projected; 32 intersect frozen "
        "pages, 3 do not; all 20 pages retain an intersecting route"
    )
    print(
        "[boundary] 0 source-target alignments, eligible units, target gold, "
        "human tasks, semantic effects, publication grants, or canon effects"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
