#!/usr/bin/env python3
"""Build text-free hierarchical label pairings and source routes.

The builder reads tracked structural maps and target-only candidate crosswalks.
It never reads witness payloads and does not infer passage or translation
alignment from shared numbering.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
TARGET_RIGHTS = Path("ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/rights.json")
CORRESPONDENCE_SCHEMA = "https://tree-of-sophia.local/ToS/contracts/hierarchical-numbered-unit-label-correspondence.schema.json"
ROUTE_SCHEMA = "https://tree-of-sophia.local/ToS/contracts/transfer-candidate-source-structural-route.schema.json"
DOES_NOT_ESTABLISH = [
    "source_text", "target_text", "exact_line_boundaries",
    "exact_passage_end_boundaries", "source_to_target_passage_alignment",
    "translation_correspondence", "translation_equivalence",
    "translation_quality", "textual_identity", "accepted_german",
    "accepted_russian", "target_gold", "eligible_target_unit", "semantics",
    "rights_clearance", "canon_promotion",
]
AUTHORITY = (
    "mechanical pairing and routing of identical series-qualified structural "
    "number-label keys already materialized independently in exact source and "
    "target witness maps; it reads no witness text and establishes neither "
    "exact passage boundaries nor passage or translation alignment, accepted "
    "German or Russian, eligibility, gold, semantics, rights, or canon authority"
)


CONFIGS = (
    {
        "slug": "zur-genealogie-der-moral",
        "pair_slug": "naumann-1892-second-svasyan-mysl-1996",
        "source": Path("ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf/structure/hierarchical-numbered-unit-page-map.json"),
        "target": Path("ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/ru-svasyan-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/hierarchical-numbered-unit-page-map.json"),
        "target_crosswalk": Path("ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/ru-svasyan-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/transfer-candidate-page-crosswalk.v1.json"),
        "expected": 78,
    },
    {
        "slug": "der-antichrist",
        "pair_slug": "naumann-1906-flerova-mysl-1996",
        "source": Path("ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/wikimedia-commons-stanford-scan-djvu/structure/hierarchical-numbered-unit-page-map.json"),
        "target": Path("ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/expressions/ru-flerova-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/hierarchical-numbered-unit-page-map.json"),
        "target_crosswalk": Path("ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/expressions/ru-flerova-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/transfer-candidate-page-crosswalk.v1.json"),
        "expected": 62,
    },
)


class BuildError(RuntimeError):
    pass


def _read(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise BuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise BuildError(f"expected object: {path}")
    return value


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _render(value: dict[str, Any]) -> str:
    return json.dumps(value, ensure_ascii=False, indent=2) + "\n"


def _render_event(value: dict[str, Any]) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n"


def _units(value: dict[str, Any], page_key: str) -> tuple[list[str], dict[str, dict[str, Any]]]:
    order: list[str] = []
    by_key: dict[str, dict[str, Any]] = {}
    for series in value.get("series", []):
        series_key = series["series_key"]
        for unit in series.get("unit_starts", []):
            qualified = f"{series_key}:{unit['unit_key']}"
            if qualified in by_key:
                raise BuildError(f"duplicate qualified key: {qualified}")
            entry = dict(unit)
            entry["series_key"] = series_key
            entry["qualified_unit_key"] = qualified
            entry["page"] = unit[page_key]
            by_key[qualified] = entry
            order.append(qualified)
    return order, by_key


def _witness(value: dict[str, Any], path: Path, *, source: bool) -> dict[str, Any]:
    file_data = value["address_witness"] if source else value["scan_file"]
    return {
        "expression_ref": value["expression_ref"],
        "edition_ref": value["edition_ref"],
        "item_ref": file_data["item_ref"] if source else value["item_ref"],
        "file_ref": file_data["file_ref"],
        "file_sha256": file_data["file_sha256"],
        "numbered_unit_map_ref": path.as_posix(),
        "numbered_unit_map_sha256": _digest(REPO_ROOT / path),
    }


def _rights(source: dict[str, Any]) -> list[dict[str, str]]:
    address = source["address_witness"]["rights"]
    navigation = source["navigation_witness"]["rights"]
    values = [{"role": "source_address", **address}]
    if navigation["ref"] != address["ref"]:
        values.append({"role": "source_navigation", **navigation})
    values.append({"role": "target", "ref": TARGET_RIGHTS.as_posix(), "sha256": _digest(REPO_ROOT / TARGET_RIGHTS)})
    return values


def _event(event_id: str, event_at: str, inputs: list[dict[str, str]], output_ref: Path, output_digest: str, role: str, method: str, config: dict[str, Any], warnings: list[str]) -> dict[str, Any]:
    return {
        "schema_version": "tos_provenance_event_v1", "event_id": event_id,
        "event_type": "alignment", "started_at": event_at, "ended_at": event_at,
        "agent_refs": ["software:python-standard-library"], "inputs": inputs,
        "outputs": [{"ref": output_ref.as_posix(), "role": role, "sha256": output_digest}],
        "method": {"maker_type": "software", "name": method, "version": "1", "artifact_digest": None, "runtime": "Python standard library", "device": None, "configuration": config, "prompt_or_instruction_ref": "ToS/doctrine/CORPUS_FOUNDATION.md#address-law"},
        "status": "completed_with_warnings", "warnings": warnings,
        "receipt_refs": [output_ref.as_posix()], "rights_basis_ref": None,
        "event_version": 1, "supersedes_event_ref": None,
    }


def build_one(config: dict[str, Any], event_at: str) -> dict[Path, str]:
    source_path, target_path, crosswalk_path = config["source"], config["target"], config["target_crosswalk"]
    source, target, crosswalk = _read(REPO_ROOT / source_path), _read(REPO_ROOT / target_path), _read(REPO_ROOT / crosswalk_path)
    if source["work_ref"] != target["work_ref"] or source["work_ref"] != crosswalk["work_ref"]:
        raise BuildError(f"work closure failed for {config['slug']}")
    source_order, source_units = _units(source, "source_page")
    target_order, target_units = _units(target, "pdf_page")
    if len(source_units) != config["expected"] or len(target_units) != config["expected"] or set(source_units) != set(target_units):
        raise BuildError(f"qualified label closure drifted for {config['slug']}")

    alignment_dir = Path("ToS/source-witnesses/works/friedrich-nietzsche") / config["slug"] / "alignments/structure" / config["pair_slug"]
    pair_path = alignment_dir / "hierarchical-numbered-unit-label-correspondence.json"
    pair_prov_path = alignment_dir / "provenance.numbered-unit-label-correspondence.jsonl"
    route_path = alignment_dir / "transfer-candidate-source-structural-route.v1.json"
    route_prov_path = alignment_dir / "provenance.transfer-candidate-source-structural-route.jsonl"
    pair_event_id = f"tos.event.hierarchical-numbered-unit-label-correspondence.friedrich-nietzsche.{config['slug']}.2026-08-08"
    route_event_id = f"tos.event.transfer-candidate-source-structural-route.friedrich-nietzsche.{config['slug']}.2026-08-08"
    rights = _rights(source)
    pairings = []
    for sequence, qualified in enumerate(target_order, 1):
        s, t = source_units[qualified], target_units[qualified]
        pairings.append({"sequence": sequence, "qualified_unit_key": qualified, "series_key": t["series_key"], "unit_key": t["unit_key"], "source_anchor_ref": s["anchor_ref"], "source_page": s["page"], "target_anchor_ref": t["anchor_ref"], "target_pdf_page": t["page"], "basis": "shared_materialized_series_qualified_number_label_key", "status": "proposed", "human_review_performed": False, "translation_alignment_claimed": False})
    pair = {
        "$schema": CORRESPONDENCE_SCHEMA, "schema_version": "tos_hierarchical_numbered_unit_label_correspondence_v1",
        "map_id": f"tos.map.hierarchical-numbered-unit-label-correspondence.friedrich-nietzsche.{config['slug']}",
        "work_ref": source["work_ref"], "source_witness": _witness(source, source_path, source=True), "target_witness": _witness(target, target_path, source=False),
        "rights_basis": rights, "map_authority": "mechanical_shared_series_qualified_number_label_candidate_only",
        "method": {"name": "exact-shared-series-qualified-structural-label-key-intersection", "version": "1", "maker_type": "software", "local_payloads_read": False, "pairing_key": "series_key:unit_key", "requires_materialized_label_in_both_maps": True, "source_to_target_text_compared": False, "translation_alignment_inferred": False, "semantic_matching_used": False, "no_text_emitted": True},
        "pairings": pairings,
        "summary": {"source_numbered_unit_count": len(source_units), "target_numbered_unit_count": len(target_units), "shared_qualified_label_count": len(pairings), "pairing_count": len(pairings), "source_only_qualified_keys": [], "target_only_qualified_keys": [], "all_pairing_statuses": ["proposed"], "human_review_performed": False, "translation_alignment_claimed": False},
        "source_text_included": False, "target_text_included": False, "provenance_ref": pair_prov_path.as_posix(), "provenance_event_ref": pair_event_id,
        "map_version": 1, "supersedes_map_ref": None, "authority_boundary": AUTHORITY, "does_not_establish": DOES_NOT_ESTABLISH,
    }
    pair_rendered = _render(pair)
    pair_digest = hashlib.sha256(pair_rendered.encode()).hexdigest()
    pair_inputs = [{"ref": source_path.as_posix(), "role": "tracked-source-hierarchical-numbered-unit-map", "sha256": _digest(REPO_ROOT / source_path)}, {"ref": target_path.as_posix(), "role": "tracked-target-hierarchical-numbered-unit-map", "sha256": _digest(REPO_ROOT / target_path)}] + [{"ref": item["ref"], "role": f"{item['role']}-rights-basis", "sha256": item["sha256"]} for item in rights]
    pair_event = _event(pair_event_id, event_at, pair_inputs, pair_path, pair_digest, "tracked-text-free-series-qualified-label-pairing-candidates", "exact-shared-series-qualified-structural-label-key-intersection", {"pairing_key": "series_key:unit_key", "expected_pairing_count": len(pairings), "local_payloads_read": False, "source_to_target_text_compared": False, "translation_alignment_inferred": False, "human_review_count": 0}, [f"The {len(pairings)} pairs assert only a shared structural series-qualified number-label key.", "No source or target witness text was read, compared, transcribed, or accepted.", "Shared numbering does not establish passage or translation alignment, equivalence, quality, or semantics."])

    by_pair = {p["qualified_unit_key"]: p for p in pairings}
    candidates = []
    route_count = 0
    for candidate in crosswalk["candidates"]:
        routes = []
        for qualified in candidate["possible_target_unit_refs"]:
            pairing = by_pair.get(qualified)
            if pairing is None:
                raise BuildError(f"candidate route {qualified} lacks pairing")
            routes.append({"qualified_unit_key": qualified, "source_anchor_ref": pairing["source_anchor_ref"], "source_page": pairing["source_page"], "target_anchor_ref": pairing["target_anchor_ref"], "target_pdf_page": pairing["target_pdf_page"], "basis": "series_qualified_number_label_candidate", "status": "proposed", "exact_passage_end_boundary_known": False, "source_to_target_passage_alignment_claimed": False, "translation_alignment_claimed": False})
        route_count += len(routes)
        candidates.append({"candidate_unit_id": candidate["candidate_unit_id"], "candidate_anchor_ref": candidate["candidate_anchor_ref"], "target_pdf_page": candidate["target_pdf_page"], "stratum": candidate["stratum"], "source_route_status": "structurally-routable-ineligible", "possible_source_structural_routes": routes, "eligible_for_variant_execution": False, "target_gold_status": "not_started"})
    route = {
        "$schema": ROUTE_SCHEMA, "schema_version": "tos_transfer_candidate_source_structural_route_v1",
        "route_set_id": f"tos.route-set.transfer-candidate-source-structural.friedrich-nietzsche.{config['slug']}",
        "status": "prepared-structural-source-routes-ineligible", "work_ref": source["work_ref"],
        "inputs": {"target_only_crosswalk": {"ref": crosswalk_path.as_posix(), "sha256": _digest(REPO_ROOT / crosswalk_path)}, "source_numbered_unit_map": {"ref": source_path.as_posix(), "sha256": _digest(REPO_ROOT / source_path)}, "target_numbered_unit_map": {"ref": target_path.as_posix(), "sha256": _digest(REPO_ROOT / target_path)}, "label_correspondence": {"ref": pair_path.as_posix(), "sha256": pair_digest}},
        "method": {"name": "target-candidate-qualified-label-to-source-structural-route", "version": "1", "local_payloads_read": False, "source_or_target_text_read": False, "route_key": "series_key:unit_key", "translation_alignment_inferred": False, "semantic_matching_used": False},
        "summary": {"candidate_page_count": len(candidates), "structurally_source_routable_candidate_page_count": len(candidates), "possible_target_route_count": route_count, "source_structural_route_count": route_count, "human_review_count": 0, "eligible_target_unit_count": 0, "target_gold_count": 0},
        "candidates": candidates, "source_text_included": False, "target_text_included": False,
        "effects": {"candidate_frame_changed": False, "exact_passage_boundary_created": False, "source_text_accepted": False, "target_text_accepted": False, "source_to_target_passage_alignment_created": False, "translation_alignment_created": False, "target_unit_eligible": False, "target_gold_created": False, "semantic_work_opened": False, "human_work_scheduled": False},
        "provenance_event_ref": route_event_id, "does_not_establish": DOES_NOT_ESTABLISH, "authority_boundary": AUTHORITY,
    }
    route_rendered = _render(route)
    route_digest = hashlib.sha256(route_rendered.encode()).hexdigest()
    route_inputs = [{"ref": crosswalk_path.as_posix(), "role": "tracked-target-only-candidate-crosswalk", "sha256": _digest(REPO_ROOT / crosswalk_path)}, {"ref": pair_path.as_posix(), "role": "tracked-series-qualified-label-correspondence", "sha256": pair_digest}, {"ref": source_path.as_posix(), "role": "tracked-source-hierarchical-numbered-unit-map", "sha256": _digest(REPO_ROOT / source_path)}, {"ref": target_path.as_posix(), "role": "tracked-target-hierarchical-numbered-unit-map", "sha256": _digest(REPO_ROOT / target_path)}]
    route_event = _event(route_event_id, event_at, route_inputs, route_path, route_digest, "tracked-text-free-transfer-candidate-source-structural-routes", "target-candidate-qualified-label-to-source-structural-route", {"candidate_page_count": len(candidates), "source_structural_route_count": route_count, "local_payloads_read": False, "source_to_target_text_compared": False, "translation_alignment_inferred": False, "eligible_target_unit_count": 0, "target_gold_count": 0, "human_review_count": 0}, [f"All {len(candidates)} frozen target pages have one or more possible source structural routes ({route_count} total).", "Routes are address candidates only; exact passage ends and passage or translation alignment remain unestablished.", "No text was read or accepted, and no human, semantic, eligibility, or gold work was opened."])
    return {pair_path: pair_rendered, pair_prov_path: _render_event(pair_event), route_path: route_rendered, route_prov_path: _render_event(route_event)}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--event-at", default="2026-08-08T21:15:00-06:00")
    args = parser.parse_args()
    outputs: dict[Path, str] = {}
    for config in CONFIGS:
        outputs.update(build_one(config, args.event_at))
    stale = []
    for relative, rendered in outputs.items():
        path = REPO_ROOT / relative
        if args.check:
            if not path.exists() or path.read_text(encoding="utf-8") != rendered:
                stale.append(relative.as_posix())
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(rendered, encoding="utf-8")
    if stale:
        for item in stale:
            print(f"[stale] {item}", file=sys.stderr)
        return 1
    print("[ok] hierarchical label correspondences and source routes are current")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
