#!/usr/bin/env python3
"""Validate German structural maps and text-free transfer source routes."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from jsonschema import FormatChecker
from jsonschema.validators import validator_for

import build_nietzsche_transfer_source_routes as builder


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_SCHEMA = Path("ToS/contracts/hierarchical-source-numbered-unit-page-map.schema.json")
PAIR_SCHEMA = Path("ToS/contracts/hierarchical-numbered-unit-label-correspondence.schema.json")
ROUTE_SCHEMA = Path("ToS/contracts/transfer-candidate-source-structural-route.schema.json")
ANCHOR_SCHEMA = Path("ToS/contracts/source-anchor.schema.json")
PROVENANCE_SCHEMA = Path("ToS/contracts/provenance-event.schema.json")
PROHIBITED_TEXT_KEYS = {"text", "source_text", "target_text", "quote", "excerpt", "transcription", "translation"}


def _read(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _schema(path: Path):
    value = _read(REPO_ROOT / path)
    cls = validator_for(value)
    cls.check_schema(value)
    return cls(value, format_checker=FormatChecker())


def _text_key_paths(value: Any, prefix: str = "") -> list[str]:
    found: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            path = f"{prefix}.{key}" if prefix else key
            if key in PROHIBITED_TEXT_KEYS:
                found.append(path)
            found.extend(_text_key_paths(child, path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            found.extend(_text_key_paths(child, f"{prefix}[{index}]"))
    return found


def _validate_schema(value: Any, validator: Any, label: str, issues: list[str]) -> None:
    for error in sorted(validator.iter_errors(value), key=lambda item: list(item.absolute_path)):
        at = "".join(f"[{part!r}]" for part in error.absolute_path)
        issues.append(f"{label}{at}: {error.message}")


def _check_ref(ref: dict[str, Any], label: str, issues: list[str]) -> None:
    if ref["ref"].startswith("tos.file.sha256."):
        if ref["ref"].removeprefix("tos.file.sha256.") != ref["sha256"]:
            issues.append(f"{label}: file-id digest closure drifted for {ref['ref']}")
        return
    path = REPO_ROOT / ref["ref"]
    if not path.is_file():
        issues.append(f"{label}: missing {ref['ref']}")
    elif _digest(path) != ref["sha256"]:
        issues.append(f"{label}: digest drift for {ref['ref']}")


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def _event_closure(path: Path, event_id: str, output_path: Path, provenance_validator: Any, issues: list[str]) -> None:
    events = _load_jsonl(REPO_ROOT / path)
    matches = [event for event in events if event.get("event_id") == event_id]
    if len(matches) != 1:
        issues.append(f"{path}: expected exactly one event {event_id}, found {len(matches)}")
        return
    event = matches[0]
    _validate_schema(event, provenance_validator, path.as_posix(), issues)
    for input_ref in event.get("inputs", []):
        _check_ref(input_ref, f"{path}: input", issues)
    outputs = [item for item in event.get("outputs", []) if item.get("ref") == output_path.as_posix()]
    if len(outputs) != 1:
        issues.append(f"{path}: output does not close over {output_path}")
    elif outputs[0].get("sha256") != _digest(REPO_ROOT / output_path):
        issues.append(f"{path}: output digest drift for {output_path}")


def _flatten(value: dict[str, Any], page_key: str) -> tuple[list[str], dict[str, dict[str, Any]]]:
    order: list[str] = []
    units: dict[str, dict[str, Any]] = {}
    for series in value["series"]:
        last_page = 0
        if len(series["unit_starts"]) != series["expected_unit_count"]:
            raise ValueError(f"{series['series_key']} expected count drift")
        for unit in series["unit_starts"]:
            qualified = f"{series['series_key']}:{unit['unit_key']}"
            if qualified in units:
                raise ValueError(f"duplicate {qualified}")
            page = unit[page_key]
            if page < last_page:
                raise ValueError(f"non-monotonic {qualified}")
            last_page = page
            record = dict(unit)
            record["qualified_unit_key"] = qualified
            record["series_key"] = series["series_key"]
            record["page"] = page
            units[qualified] = record
            order.append(qualified)
    return order, units


def validate(repo_root: Path = REPO_ROOT) -> list[str]:
    del repo_root
    issues: list[str] = []
    source_validator, pair_validator, route_validator = _schema(SOURCE_SCHEMA), _schema(PAIR_SCHEMA), _schema(ROUTE_SCHEMA)
    anchor_validator, provenance_validator = _schema(ANCHOR_SCHEMA), _schema(PROVENANCE_SCHEMA)
    total_pairs = total_routes = total_candidates = 0
    for config in builder.CONFIGS:
        source_path, target_path, crosswalk_path = config["source"], config["target"], config["target_crosswalk"]
        source, target, crosswalk = _read(REPO_ROOT / source_path), _read(REPO_ROOT / target_path), _read(REPO_ROOT / crosswalk_path)
        _validate_schema(source, source_validator, source_path.as_posix(), issues)
        prohibited = _text_key_paths(source)
        if prohibited:
            issues.append(f"{source_path}: prohibited text-bearing keys: {prohibited}")
        try:
            source_order, source_units = _flatten(source, "source_page")
            target_order, target_units = _flatten(target, "pdf_page")
        except (KeyError, ValueError) as exc:
            issues.append(f"{source_path}: structural closure failed: {exc}")
            continue
        if len(source_units) != config["expected"] or set(source_units) != set(target_units) or source_order != target_order:
            issues.append(f"{source_path}: source/target qualified-key closure drifted")
        if source["summary"]["numbered_unit_count"] != len(source_units) or source["summary"]["accepted_german_unit_count"] != 0 or source["summary"]["human_review_performed"]:
            issues.append(f"{source_path}: authority summary drifted")
        for witness_name in ("address_witness", "navigation_witness"):
            witness = source[witness_name]
            _check_ref(witness["inventory"], f"{source_path}: {witness_name} inventory", issues)
            _check_ref(witness["rights"], f"{source_path}: {witness_name} rights", issues)
        inventory = _read(REPO_ROOT / source["address_witness"]["inventory"]["ref"])
        resources = {item["resource_id"] for file in inventory["files"] for item in file["resources"]}
        for qualified, unit in source_units.items():
            if unit["resource_id"] not in resources:
                issues.append(f"{source_path}: {qualified} missing resource {unit['resource_id']}")
            offset = source["navigation_relation"]["address_page_from_navigation_page_offset"]
            if unit["navigation_page"] + offset != unit["source_page"]:
                issues.append(f"{source_path}: {qualified} page relation drifted")
        anchor_path = source_path.with_name("hierarchical-numbered-unit-anchors.jsonl")
        anchors = _load_jsonl(REPO_ROOT / anchor_path)
        if len(anchors) != len(source_units):
            issues.append(f"{anchor_path}: anchor count drifted")
        anchor_by_id = {anchor.get("anchor_id"): anchor for anchor in anchors}
        for qualified, unit in source_units.items():
            anchor = anchor_by_id.get(unit["anchor_ref"])
            if anchor is None:
                issues.append(f"{anchor_path}: missing anchor for {qualified}")
                continue
            _validate_schema(anchor, anchor_validator, f"{anchor_path}:{qualified}", issues)
            pages = [selector.get("page") for selector in anchor["selectors"] if selector.get("type") == "page_region"]
            if pages != [unit["source_page"]] or anchor["status"] != "proposed" or anchor["review_ref"] is not None:
                issues.append(f"{anchor_path}: {qualified} authority/page drifted")
        _event_closure(Path(source["provenance_ref"]), source["provenance_event_ref"], source_path, provenance_validator, issues)

        alignment_dir = Path("ToS/source-witnesses/works/friedrich-nietzsche") / config["slug"] / "alignments/structure" / config["pair_slug"]
        pair_path = alignment_dir / "hierarchical-numbered-unit-label-correspondence.json"
        pair_prov = alignment_dir / "provenance.numbered-unit-label-correspondence.jsonl"
        route_path = alignment_dir / "transfer-candidate-source-structural-route.v1.json"
        route_prov = alignment_dir / "provenance.transfer-candidate-source-structural-route.jsonl"
        pair, route = _read(REPO_ROOT / pair_path), _read(REPO_ROOT / route_path)
        _validate_schema(pair, pair_validator, pair_path.as_posix(), issues)
        _validate_schema(route, route_validator, route_path.as_posix(), issues)
        for value, label in ((pair, pair_path), (route, route_path)):
            prohibited = _text_key_paths(value)
            if prohibited:
                issues.append(f"{label}: prohibited text-bearing keys: {prohibited}")
        for ref in pair["rights_basis"]:
            _check_ref(ref, f"{pair_path}: rights", issues)
        actual_pairs = {item["qualified_unit_key"]: item for item in pair["pairings"]}
        if list(actual_pairs) != target_order or len(actual_pairs) != config["expected"]:
            issues.append(f"{pair_path}: pairing key/order closure drifted")
        for qualified, item in actual_pairs.items():
            source_unit, target_unit = source_units[qualified], target_units[qualified]
            expected = (source_unit["anchor_ref"], source_unit["page"], target_unit["anchor_ref"], target_unit["page"])
            actual = (item["source_anchor_ref"], item["source_page"], item["target_anchor_ref"], item["target_pdf_page"])
            if actual != expected or item["translation_alignment_claimed"] or item["human_review_performed"]:
                issues.append(f"{pair_path}: pairing closure drifted for {qualified}")
        total_pairs += len(actual_pairs)
        _event_closure(pair_prov, pair["provenance_event_ref"], pair_path, provenance_validator, issues)

        for input_ref in route["inputs"].values():
            _check_ref(input_ref, f"{route_path}: input", issues)
        target_candidates = {item["candidate_unit_id"]: item for item in crosswalk["candidates"]}
        if len(route["candidates"]) != len(target_candidates):
            issues.append(f"{route_path}: candidate count drifted")
        route_count = 0
        for candidate in route["candidates"]:
            target_candidate = target_candidates.get(candidate["candidate_unit_id"])
            if target_candidate is None:
                issues.append(f"{route_path}: unknown candidate {candidate['candidate_unit_id']}")
                continue
            keys = [item["qualified_unit_key"] for item in candidate["possible_source_structural_routes"]]
            if keys != target_candidate["possible_target_unit_refs"]:
                issues.append(f"{route_path}: possible route closure drifted for {candidate['candidate_unit_id']}")
            for item in candidate["possible_source_structural_routes"]:
                pairing = actual_pairs[item["qualified_unit_key"]]
                expected = (pairing["source_anchor_ref"], pairing["source_page"], pairing["target_anchor_ref"], pairing["target_pdf_page"])
                actual = (item["source_anchor_ref"], item["source_page"], item["target_anchor_ref"], item["target_pdf_page"])
                if actual != expected or item["source_to_target_passage_alignment_claimed"] or item["translation_alignment_claimed"]:
                    issues.append(f"{route_path}: source route closure drifted for {item['qualified_unit_key']}")
            route_count += len(keys)
        if route["summary"]["source_structural_route_count"] != route_count or route["summary"]["structurally_source_routable_candidate_page_count"] != len(target_candidates):
            issues.append(f"{route_path}: route summary drifted")
        if any(route["effects"].values()):
            issues.append(f"{route_path}: forbidden effect became true")
        total_routes += route_count
        total_candidates += len(route["candidates"])
        _event_closure(route_prov, route["provenance_event_ref"], route_path, provenance_validator, issues)
    if (total_pairs, total_routes, total_candidates) != (140, 20, 12):
        issues.append(f"aggregate closure drifted: pairs={total_pairs}, routes={total_routes}, candidates={total_candidates}")
    return issues


def main() -> int:
    issues = validate()
    if issues:
        for issue in issues:
            print(f"[error] {issue}", file=sys.stderr)
        print(f"Nietzsche transfer source route validation failed: {len(issues)} issue(s)", file=sys.stderr)
        return 1
    print("Nietzsche transfer source route validation passed (2 source maps, 140 structural label pairs, 12 candidate pages, 20 source routes; 0 accepted text/alignment/eligibility/gold/human/semantic effects).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
