#!/usr/bin/env python3
"""Materialize the full DE->RU paragraph-alignment proposal packet set.

Primary and independent challenger outputs are frozen as text-free inputs.
Exact German anchors come from the four tracked DTA text-unit packets. Exact
Russian anchors address mode-0600 ignored paragraph layers rebuilt from the
Antonovsky structural/paragraph v2 source. Machine results remain proposals,
ambiguities, or deferrals and cannot accept translation truth.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import secrets
import stat
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable

from jsonschema import Draft202012Validator, FormatChecker

REPO = Path(__file__).resolve().parents[1]
WORK_REF = "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
SCHEMA_REF = "ToS/contracts/translation-alignment-packet-v1.schema.json"
DE_ROUTE_REF = ("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
                "technical-markup/dta-first-editions-parts-1-4-v1")
RU_ROUTE_REF = ("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
                "technical-markup/antonovsky-1911-structural-paragraph-v2")
ROUTE_REF = ("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
             "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1")
ROUTE = REPO / ROUTE_REF
PRIVATE_ROOT_REF = ("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
                    "gold-sets/foundation-pilot-v1/local-content/translation-alignment-v1")
PRIVATE_ROOT = REPO / PRIVATE_ROOT_REF
RU_BUILDER = REPO / "scripts/build_antonovsky_1911_structural_paragraph_v2.py"
DE_PLAN_REF = f"{DE_ROUTE_REF}/plan.v1.json"
RU_PLAN_REF = ("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
               "technical-markup/antonovsky-1911-pdf-layout-v1/plan.v1.json")
PRIMARY_REF = f"{ROUTE_REF}/primary-proposals-input.v1.jsonl"
PRIMARY_DIAGNOSTICS_REF = f"{ROUTE_REF}/primary-diagnostics-input.v1.jsonl"
PRIMARY_METHOD_REF = f"{ROUTE_REF}/primary-method-input.v1.json"
PRIMARY_RECEIPT_REF = f"{ROUTE_REF}/primary-input-receipt.v1.json"
CHALLENGER_REF = f"{ROUTE_REF}/independent-challenger-input.v1.jsonl"
CHALLENGER_RECEIPT_REF = f"{ROUTE_REF}/independent-challenger-input-receipt.v1.json"
ISSUANCE_REF = f"{ROUTE_REF}/identity-issuance.v1.json"
PART_LABEL = {1: "I", 2: "II", 3: "III", 4: "IV"}
READINGS_BY_PART = {1: 23, 2: 22, 3: 16, 4: 20}
EXPECTED = {"source_paragraphs": 3447, "target_paragraphs": 3569,
            "primary_mappings": 3423, "readings": 81,
            "source_verse_groups": 39, "source_verse_lines": 368,
            "target_verse_groups": 13, "target_verse_lines": 359}

PACKET_REFS = {part: f"{ROUTE_REF}/part-{part}.translation-alignment-packet.v1.json"
               for part in range(1, 5)}
OUTPUT_REFS = {
    "spine": f"{ROUTE_REF}/alignment-spine.v1.jsonl",
    "coverage": f"{ROUTE_REF}/coverage-receipt.v1.json",
    "diagnostics": f"{ROUTE_REF}/per-reading-diagnostics.v1.jsonl",
    "conflicts": f"{ROUTE_REF}/conflict-ledger.v1.jsonl",
    "exclusions": f"{ROUTE_REF}/scope-exclusions.v1.jsonl",
    "challenger_audit": f"{ROUTE_REF}/independent-challenger-audit.v1.jsonl",
    "verification": f"{ROUTE_REF}/agent-verification.v1.json",
    "summary": f"{ROUTE_REF}/summary.v1.json",
    "provenance": f"{ROUTE_REF}/provenance.jsonl",
    "manifest": f"{ROUTE_REF}/manifest.v1.json",
}


class AlignmentBuildError(RuntimeError):
    pass


def sha(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def json_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()


def jsonl_bytes(rows: Iterable[dict[str, Any]]) -> bytes:
    return b"".join((json.dumps(row, ensure_ascii=False, sort_keys=True,
                                separators=(",", ":")) + "\n").encode() for row in rows)


def load_json(ref: str | Path) -> Any:
    path = ref if isinstance(ref, Path) else REPO / ref
    return json.loads(path.read_text(encoding="utf-8"))


def load_jsonl(ref: str | Path) -> list[dict[str, Any]]:
    path = ref if isinstance(ref, Path) else REPO / ref
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def import_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise AlignmentBuildError(f"cannot import {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


FORBIDDEN_TEXT_KEYS = {"text", "source_text", "target_text", "content", "excerpt", "quote"}


def assert_text_free(value: Any, where: str) -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if key.lower() in FORBIDDEN_TEXT_KEYS:
                raise AlignmentBuildError(f"text-bearing key {key!r} in {where}")
            if key in {"source_text_included", "target_text_included"} and child is not False:
                raise AlignmentBuildError(f"text inclusion flag must be false in {where}")
            assert_text_free(child, where)
    elif isinstance(value, list):
        for child in value:
            assert_text_free(child, where)


def import_primary(directory: Path) -> None:
    targets = (REPO / PRIMARY_REF, REPO / PRIMARY_DIAGNOSTICS_REF,
               REPO / PRIMARY_METHOD_REF, REPO / PRIMARY_RECEIPT_REF)
    if any(path.exists() for path in targets):
        raise AlignmentBuildError("primary inputs already exist; refusing replacement")
    files = {
        PRIMARY_REF: directory / "alignment-proposals.jsonl",
        PRIMARY_DIAGNOSTICS_REF: directory / "per-reading-diagnostics.jsonl",
        PRIMARY_METHOD_REF: directory / "method.json",
    }
    for ref, source in files.items():
        parsed = load_jsonl(source) if source.suffix == ".jsonl" else load_json(source)
        assert_text_free(parsed, source.name)
        (REPO / ref).write_bytes(source.read_bytes())
    receipt_sources = ["alignment-proposals.jsonl", "per-reading-diagnostics.jsonl",
                       "anomaly-conflict-ledger.jsonl", "coverage-receipt.json",
                       "method.json", "report.json", "manifest.json", "build_alignment.py"]
    receipt = {
        "schema_version": "tos_zarathustra_de_ru_primary_input_receipt_v1",
        "role": "machine_primary_proposal_input_not_truth",
        "artifact_sha256": {name: sha((directory / name).read_bytes()) for name in receipt_sources},
        "source_text_included": False, "human_acceptance": False,
    }
    (REPO / PRIMARY_RECEIPT_REF).write_bytes(json_bytes(receipt))


def import_challenger(directory: Path) -> None:
    target, receipt_path = REPO / CHALLENGER_REF, REPO / CHALLENGER_RECEIPT_REF
    if target.exists() or receipt_path.exists():
        raise AlignmentBuildError("challenger inputs already exist; refusing replacement")
    source = directory / "mapping.proposal.v1.jsonl"
    rows = load_jsonl(source)
    assert_text_free(rows, source.name)
    target.write_bytes(source.read_bytes())
    names = ["mapping.proposal.v1.jsonl", "chapter-summary.v1.jsonl",
             "low-confidence-anomalies.v1.jsonl", "coverage.v1.json",
             "invariant-audit.v1.json", "configuration.v1.json", "manifest.v1.json",
             "run_challenger.py"]
    receipt = {
        "schema_version": "tos_zarathustra_de_ru_independent_challenger_input_receipt_v1",
        "role": "mixed_prose_verse_machine_challenger_not_truth",
        "artifact_sha256": {name: sha((directory / name).read_bytes()) for name in names},
        "source_text_included": False, "target_text_included": False,
        "accepted_alignment_count": 0,
    }
    receipt_path.write_bytes(json_bytes(receipt))


def reconstruct_material() -> dict[str, Any]:
    primary = load_jsonl(PRIMARY_REF)
    challenger = load_jsonl(CHALLENGER_REF)
    if len(primary) != EXPECTED["primary_mappings"]:
        raise AlignmentBuildError(f"primary mapping census drift: {len(primary)}")
    de_plan = load_json(DE_PLAN_REF)
    ru_plan = load_json(RU_PLAN_REF)

    de_parts: dict[int, dict[str, Any]] = {}
    de_paragraph_anchor: dict[str, dict[str, Any]] = {}
    for part in range(1, 5):
        packet_ref = f"{DE_ROUTE_REF}/part-{part}.source-text-unit.v1.json"
        packet = load_json(packet_ref)
        source_layer_ref = packet["source_layer"]["text_layer_ref"]
        source_layer = REPO / source_layer_ref
        if not source_layer.is_file() \
                or sha(source_layer.read_bytes()) != packet["source_layer"]["text_layer_sha256"]:
            raise AlignmentBuildError(f"German private source layer drift: {source_layer_ref}")
        if stat.S_IMODE(source_layer.stat().st_mode) != 0o600:
            raise AlignmentBuildError(f"German private source layer mode is not 0600: {source_layer_ref}")
        anchors = {row["anchor_ref"]: row for row in packet["anchors"]}
        paragraph_units = [row for row in packet["units"] if row["unit_kind"] == "paragraph"]
        for unit in paragraph_units:
            if len(unit["ordered_anchor_refs"]) != 1:
                raise AlignmentBuildError("German paragraph does not have exactly one anchor")
            de_paragraph_anchor[unit["unit_id"]] = anchors[unit["ordered_anchor_refs"][0]]
        de_parts[part] = {"packet_ref": packet_ref, "packet": packet,
                          "paragraph_units": paragraph_units}

    ru = import_module("antonovsky_alignment_ru_builder", RU_BUILDER)
    model = ru.reconstruct()
    tracked_paragraphs = load_jsonl(f"{RU_ROUTE_REF}/paragraph-spine.v2.jsonl")
    if len(tracked_paragraphs) != EXPECTED["target_paragraphs"] or \
            len(tracked_paragraphs) != len(model["paragraphs"]):
        raise AlignmentBuildError("Russian paragraph census drift")
    row_text = {row.row_ref: row.text for row in model["rows"]}
    ru_by_part: dict[int, list[dict[str, Any]]] = defaultdict(list)
    for raw, tracked in zip(model["paragraphs"], tracked_paragraphs, strict=True):
        text = "\n".join(row_text[ref] for ref in raw["row_refs"])
        if sha(text.encode()) != tracked["exact_sha256"]:
            raise AlignmentBuildError("Russian paragraph private-text binding drift")
        part = int(tracked["part_id"].split("_")[1])
        ru_by_part[part].append({"unit_id": tracked["paragraph_unit_id"],
                                 "citation": tracked["display_citation"],
                                 "reading_unit_ref": tracked["reading_unit_ref"],
                                 "text": text, "exact_sha256": tracked["exact_sha256"]})

    target_layers = {}
    target_records = {}
    for part in range(1, 5):
        position, pieces, records = 0, [], []
        for ordinal, row in enumerate(ru_by_part[part], 1):
            if pieces:
                pieces.append("\n\n"); position += 2
            start = position
            pieces.append(row["text"]); position += len(row["text"])
            records.append({**row, "ordinal": ordinal, "start": start, "end": position})
            target_records[row["unit_id"]] = records[-1]
        payload = "".join(pieces).encode()
        target_layers[part] = {
            "ref": f"{PRIVATE_ROOT_REF}/antonovsky-1911-part-{part}-paragraphs.v1.txt",
            "payload": payload, "sha256": sha(payload), "records": records,
        }

    source_ids = [unit for row in primary for unit in row["source_paragraph_refs"]]
    target_ids = [unit for row in primary for unit in row["target_paragraph_refs"]]
    if len(source_ids) != EXPECTED["source_paragraphs"] or len(set(source_ids)) != len(source_ids) \
            or set(source_ids) != set(de_paragraph_anchor):
        raise AlignmentBuildError("primary German paragraph coverage is not exact")
    if len(target_ids) != EXPECTED["target_paragraphs"] or len(set(target_ids)) != len(target_ids) \
            or set(target_ids) != set(target_records):
        raise AlignmentBuildError("primary Russian paragraph coverage is not exact")
    return {"primary": primary, "challenger": challenger, "de_plan": de_plan,
            "ru_plan": ru_plan, "de_parts": de_parts,
            "de_paragraph_anchor": de_paragraph_anchor,
            "target_layers": target_layers, "target_records": target_records}


def identity_bindings(material: dict[str, Any]) -> dict[str, list[str]]:
    return {
        "packets": [f"part-{part}" for part in range(1, 5)],
        "target_anchors": [row["unit_id"] for part in range(1, 5)
                           for row in material["target_layers"][part]["records"]],
        "alignments": [row["alignment_proposal_ref"] for row in material["primary"]],
        "claims": [row["alignment_proposal_ref"] for row in material["primary"]],
    }


def mint(prefix: str) -> str:
    return f"{prefix}.sid-{secrets.token_hex(16)}"


def issue_identities(material: dict[str, Any]) -> None:
    path = REPO / ISSUANCE_REF
    if path.exists():
        raise AlignmentBuildError("identity issuance already exists; refusing remint")
    prefixes = {"packets": "tos.translation-alignment-packet",
                "target_anchors": "tos.anchor",
                "alignments": "tos.translation-alignment",
                "claims": "tos.translation-alignment-claim"}
    payload = {
        "schema_version": "tos_zarathustra_de_ru_paragraph_alignment_identity_issuance_v1",
        "issuance_id": "tos.identity-issuance.zarathustra-de-ru-paragraph-alignment-v1",
        "issued_on": "2026-09-01", "opaque_identity": True,
        "binding_is_not_identity": True,
        "identities": {kind: [{"binding": binding, "id": mint(prefixes[kind])}
                               for binding in values]
                       for kind, values in identity_bindings(material).items()},
    }
    path.write_bytes(json_bytes(payload))


def load_identities(material: dict[str, Any]) -> dict[str, dict[str, str]]:
    payload = load_json(ISSUANCE_REF)
    expected = identity_bindings(material)
    result, all_ids = {}, []
    for kind, bindings in expected.items():
        records = payload["identities"].get(kind, [])
        if [row["binding"] for row in records] != bindings:
            raise AlignmentBuildError(f"identity binding drift: {kind}")
        result[kind] = {row["binding"]: row["id"] for row in records}
        all_ids.extend(row["id"] for row in records)
    if len(all_ids) != len(set(all_ids)):
        raise AlignmentBuildError("identity collision")
    return result


def challenger_audit(material: dict[str, Any]) -> list[dict[str, Any]]:
    challenger = material["challenger"]
    owner: dict[str, int] = {}
    for index, row in enumerate(challenger):
        for unit in row["ordered_source_unit_refs"] + row["ordered_target_unit_refs"]:
            if unit in owner:
                raise AlignmentBuildError("challenger unit coverage duplicates an identity")
            owner[unit] = index
    audit = []
    for primary in material["primary"]:
        units = primary["source_paragraph_refs"] + primary["target_paragraph_refs"]
        candidate_indexes = sorted({owner[unit] for unit in units if unit in owner})
        candidates = [challenger[index] for index in candidate_indexes]
        source_paragraphs = {
            unit for row in candidates
            for unit, kind in zip(row["ordered_source_unit_refs"], row["source_unit_kinds"], strict=True)
            if kind == "prose_paragraph"
        }
        target_paragraphs = {
            unit for row in candidates
            for unit, kind in zip(row["ordered_target_unit_refs"], row["target_unit_kinds"], strict=True)
            if kind == "prose_paragraph"
        }
        verse_risk = any("verse_group" in row["source_unit_kinds"] + row["target_unit_kinds"]
                         for row in candidates)
        outside = primary["reading_ordinal_within_part"] is None
        agrees = (len(candidates) == 1
                  and source_paragraphs == set(primary["source_paragraph_refs"])
                  and target_paragraphs == set(primary["target_paragraph_refs"])
                  and not verse_risk)
        if outside:
            posture = "outside_challenger_81_reading_scope"
        elif agrees:
            posture = "same_paragraph_group"
        elif verse_risk:
            posture = "mixed_prose_verse_scope_disagreement"
        else:
            posture = "paragraph_group_disagreement"
        audit.append({
            "schema_version": "tos_zarathustra_de_ru_independent_challenger_audit_v1",
            "primary_proposal_ref": primary["alignment_proposal_ref"],
            "part_order": primary["part_order"],
            "reading_ordinal_within_part": primary["reading_ordinal_within_part"],
            "challenger_alignment_refs": [row["alignment_id"] for row in candidates],
            "comparison_posture": posture, "same_paragraph_group": agrees,
            "verse_barrier_risk": verse_risk,
            "requires_source_and_target_visible_review": not agrees,
            "resolution": "none_machine_disagreement_preserved" if not agrees and not outside else
                          "not_applicable_outside_scope" if outside else "agreement_is_not_acceptance",
            "source_text_included": False, "target_text_included": False,
        })
    if len(audit) != EXPECTED["primary_mappings"]:
        raise AlignmentBuildError("challenger audit census drift")
    return audit


def scope_exclusions(material: dict[str, Any]) -> tuple[list[dict[str, Any]], set[tuple[int, int]]]:
    de_rows = load_jsonl(f"{DE_ROUTE_REF}/citation-spine.v1.jsonl")
    reading_ordinal = {}
    for part in range(1, 5):
        majors = [row for row in de_rows if row["part_order"] == part
                  and row["structural_role"] == "major_reading_unit_candidate"]
        if len(majors) != READINGS_BY_PART[part]:
            raise AlignmentBuildError("German reading-unit census drift for exclusions")
        reading_ordinal.update({row["unit_id"]: ordinal for ordinal, row in enumerate(majors, 1)})
    de_lines_by_group: dict[str, list[str]] = defaultdict(list)
    for row in de_rows:
        if row["unit_kind"] == "verse_line":
            de_lines_by_group[row["parent_unit_id"]].append(row["unit_id"])
    exclusions, risks = [], set()
    for group in [row for row in de_rows if row["unit_kind"] == "verse_group"]:
        reading = reading_ordinal[group["nearest_major_unit_id"]]
        risks.add((group["part_order"], reading))
        exclusions.append({
            "schema_version": "tos_zarathustra_de_ru_alignment_scope_exclusion_v1",
            "scope_exclusion_ref": f"de-verse-group-{len(exclusions)+1:03d}",
            "side": "source", "part_order": group["part_order"],
            "reading_ordinal_within_part": reading, "excluded_unit_kind": "verse_group",
            "excluded_group_unit_ref": group["unit_id"],
            "ordered_excluded_verse_line_refs": de_lines_by_group[group["unit_id"]],
            "reason": "paragraph_packet_scope_excludes_source_attested_verse_group",
            "used_as_structural_risk_qualification": True,
            "source_text_included": False, "semantic_equivalence_asserted": False,
        })
    ru_groups = load_jsonl(f"{RU_ROUTE_REF}/verse-group-spine.v2.jsonl")
    ru_lines = load_jsonl(f"{RU_ROUTE_REF}/verse-line-spine.v2.jsonl")
    ru_structure = load_jsonl(f"{RU_ROUTE_REF}/structure-spine.v2.jsonl")
    reading_meta = {row["structure_unit_id"]: (int(row["part_id"].split("_")[1]),
                    int(row["reading_unit_ordinal_within_part"]))
                    for row in ru_structure if row["record_kind"] == "reading_unit_heading"}
    ru_lines_by_group: dict[str, list[str]] = defaultdict(list)
    for row in ru_lines:
        ru_lines_by_group[row["verse_group_unit_ref"]].append(row["verse_line_unit_id"])
    for ordinal, group in enumerate(ru_groups, 1):
        part, reading = reading_meta[group["reading_unit_ref"]]
        risks.add((part, reading))
        exclusions.append({
            "schema_version": "tos_zarathustra_de_ru_alignment_scope_exclusion_v1",
            "scope_exclusion_ref": f"ru-verse-group-{ordinal:03d}", "side": "target",
            "part_order": part, "reading_ordinal_within_part": reading,
            "excluded_unit_kind": "continuous_verse_group",
            "excluded_group_unit_ref": group["verse_group_unit_id"],
            "ordered_excluded_verse_line_refs": ru_lines_by_group[group["verse_group_unit_id"]],
            "reason": "paragraph_packet_scope_excludes_source_visible_continuous_verse_group",
            "used_as_structural_risk_qualification": True,
            "source_text_included": False, "semantic_equivalence_asserted": False,
        })
    if len([row for row in exclusions if row["side"] == "source"]) != EXPECTED["source_verse_groups"] \
            or sum(len(row["ordered_excluded_verse_line_refs"]) for row in exclusions if row["side"] == "source") != EXPECTED["source_verse_lines"] \
            or len([row for row in exclusions if row["side"] == "target"]) != EXPECTED["target_verse_groups"] \
            or sum(len(row["ordered_excluded_verse_line_refs"]) for row in exclusions if row["side"] == "target") != EXPECTED["target_verse_lines"]:
        raise AlignmentBuildError("verse scope-exclusion census drift")
    return exclusions, risks


def anchor_view(anchor: dict[str, Any]) -> dict[str, Any]:
    return {key: anchor[key] for key in ("anchor_ref", "ordinal", "text_layer_ref",
                                          "text_layer_sha256", "selector", "exact_sha256",
                                          "source_return")}


def cross_kind_or_verse_scope_risk(primary: dict[str, Any], audit: dict[str, Any]) -> bool:
    null_side = not primary["source_paragraph_refs"] or not primary["target_paragraph_refs"]
    russian_verse_reading = (
        primary["part_order"] == 3 and primary["reading_ordinal_within_part"] == 15
    )
    return audit["verse_barrier_risk"] or (null_side and russian_verse_reading)


def proposal_posture(primary: dict[str, Any], audit: dict[str, Any]) -> tuple[str, str, str]:
    outside = primary["reading_ordinal_within_part"] is None
    null_side = not primary["source_paragraph_refs"] or not primary["target_paragraph_refs"]
    if outside:
        return ("deferred", "unresolved",
                "source-only in bounded 81-reading technical scope; not evidence of translator omission")
    if null_side:
        reason = ("cross-kind/out-of-paragraph-scope candidate; not evidence of translator omission"
                  if cross_kind_or_verse_scope_risk(primary, audit) else
                  "structural-role mismatch; not evidence of translator omission")
        return "ambiguous", primary["mapping_type"], reason
    if not audit["same_paragraph_group"]:
        return ("ambiguous", primary["mapping_type"],
                "independent mixed-content challenger disagrees; machine ambiguity preserved without resolution")
    if audit["verse_barrier_risk"]:
        return ("ambiguous", primary["mapping_type"],
                "excluded verse boundary qualifies this paragraph grouping; machine ambiguity preserved")
    if primary["resolution_status"].startswith("unresolved") or primary["confidence_band"] == "low":
        return ("ambiguous", primary["mapping_type"],
                "length-profile result is unstable or low-fit; source-and-target-visible review required")
    return ("proposed", primary["mapping_type"],
            "machine grouping agrees across compared technical profiles; proposal is not translation truth")


def build_artifacts(material: dict[str, Any]) -> tuple[dict[str, bytes], dict[str, bytes]]:
    identities = load_identities(material)
    audit_rows = challenger_audit(material)
    audit_by_primary = {row["primary_proposal_ref"]: row for row in audit_rows}
    exclusions, risk_readings = scope_exclusions(material)
    method_sha = sha((REPO / PRIMARY_METHOD_REF).read_bytes())
    target_anchor_id = identities["target_anchors"]
    alignment_id = identities["alignments"]
    claim_id = identities["claims"]
    packet_id = identities["packets"]

    target_anchors = {}
    for part in range(1, 5):
        layer = material["target_layers"][part]
        for row in layer["records"]:
            target_anchors[row["unit_id"]] = {
                "anchor_ref": target_anchor_id[row["unit_id"]], "ordinal": row["ordinal"],
                "text_layer_ref": layer["ref"], "text_layer_sha256": layer["sha256"],
                "selector": {"type": "text_position", "start": row["start"], "end": row["end"],
                             "position_unit": "unicode_code_point", "interval": "half_open"},
                "exact_sha256": row["exact_sha256"],
                "source_return": {"required": True,
                    "locator_ref": f"{RU_ROUTE_REF}/paragraph-spine.v2.jsonl#unit={row['unit_id']}"},
            }

    spine, conflict_rows = [], []
    alignments_by_part: dict[int, list[dict[str, Any]]] = defaultdict(list)
    final_meta = {}
    for primary in material["primary"]:
        ref = primary["alignment_proposal_ref"]
        audit = audit_by_primary[ref]
        status, shape, reason = proposal_posture(primary, audit)
        cross_kind_risk = cross_kind_or_verse_scope_risk(primary, audit)
        source_anchor_refs = [material["de_paragraph_anchor"][unit]["anchor_ref"]
                              for unit in primary["source_paragraph_refs"]]
        target_anchor_refs = [target_anchors[unit]["anchor_ref"]
                              for unit in primary["target_paragraph_refs"]]
        aid, cid = alignment_id[ref], claim_id[ref]
        evidence_role = "method_input" if status == "proposed" else "qualification"
        alignment = {
            "alignment_id": aid, "alignment_version": 1, "supersedes_alignment_ref": None,
            "identity_policy": "opaque-id-independent-of-text-label-translation-and-current-mapping",
            "claim_id": cid, "claim_version": 1, "supersedes_claim_ref": None,
            "direction": "source_to_target", "correspondence_shape": shape,
            "order_posture": "monotonic" if source_anchor_refs and target_anchor_refs else "not_applicable",
            "ordered_source_anchor_refs": source_anchor_refs,
            "ordered_target_anchor_refs": target_anchor_refs,
            "translation_techniques": ["unresolved"],
            "epistemic_status": "inferred" if status == "proposed" else "uncertain",
            "certainty": {"value": 0.5 if status == "proposed" else 0.0,
                          "meaning": "maker_declared_uncertainty_not_truth_probability"},
            "status": status, "status_reason": reason,
            "maker": {"maker_kind": "software", "agent_ref": "codex-internal-agents.alignment-v1",
                      "made_at": "2026-09-01T23:30:00-06:00",
                      "method": "bounded structural anchors plus deterministic length-profile proposals and independent mixed-content challenger",
                      "provenance_event_ref": "tos.event.zarathustra-de-ru-paragraph-alignment-v1.build",
                      "method_output_posture": "proposal_not_truth",
                      "software_ref": "scripts/build_zarathustra_de_ru_paragraph_alignment_v1.py",
                      "software_version": "1", "configuration_sha256": method_sha},
            "evidence": [{"evidence_ref": f"{OUTPUT_REFS['challenger_audit']}#{ref}",
                          "role": evidence_role, "source_anchor_refs": source_anchor_refs,
                          "target_anchor_refs": target_anchor_refs, "description": reason}],
            "competing_alignment_refs": [], "review_refs": [],
        }
        alignments_by_part[primary["part_order"]].append(alignment)
        final_meta[ref] = {"status": status, "shape": shape, "reason": reason,
                           "alignment_id": aid, "claim_id": cid}
        spine.append({
            "schema_version": "tos_zarathustra_de_ru_paragraph_alignment_spine_v1",
            "primary_proposal_ref": ref, "alignment_id": aid, "claim_id": cid,
            "part_order": primary["part_order"],
            "reading_ordinal_within_part": primary["reading_ordinal_within_part"],
            "technical_subpart_ordinal": primary["technical_subpart_ordinal"],
            "correspondence_shape": shape, "ordered_source_anchor_refs": source_anchor_refs,
            "ordered_target_anchor_refs": target_anchor_refs,
            "source_paragraph_unit_refs": primary["source_paragraph_refs"],
            "target_paragraph_unit_refs": primary["target_paragraph_refs"],
            "status": status, "status_reason": reason,
            "primary_profile_disagreement": not primary["primary_challenger_same_group"],
            "independent_challenger_same_paragraph_group": audit["same_paragraph_group"],
            "independent_challenger_refs": audit["challenger_alignment_refs"],
            "verse_barrier_risk": audit["verse_barrier_risk"],
            "cross_kind_or_verse_scope_risk": cross_kind_risk,
            "human_acceptance": False, "source_text_included": False,
            "semantic_equivalence_asserted": False, "canon_effect": False,
        })
        if status != "proposed":
            conflict_rows.append({
                "schema_version": "tos_zarathustra_de_ru_paragraph_alignment_conflict_v1",
                "conflict_ref": f"alignment-conflict-{len(conflict_rows)+1:04d}",
                "alignment_ref": aid, "primary_proposal_ref": ref,
                "part_order": primary["part_order"],
                "reading_ordinal_within_part": primary["reading_ordinal_within_part"],
                "conflict_kinds": (["independent_challenger_group_disagreement"]
                                   if not audit["same_paragraph_group"] else [])
                                  + (["verse_barrier_or_cross_kind_risk"]
                                     if cross_kind_risk else [])
                                  + (["null_side_transition"] if not source_anchor_refs or not target_anchor_refs else [])
                                  + (["primary_length_profile_instability"]
                                     if not primary["primary_challenger_same_group"] else [])
                                  + (["low_length_fit"] if primary["confidence_band"] == "low" else []),
                "preserved_status": status, "reason": reason,
                "machine_resolution": "none", "requires_source_and_target_visible_review": True,
                "source_text_included": False, "semantic_equivalence_asserted": False,
            })

    schema_uri = "https://tree-of-sophia.local/ToS/contracts/translation-alignment-packet-v1.schema.json"
    de_config = {int(row["part_order"]): row for row in material["de_plan"]["source_items"]}
    ru_config = material["ru_plan"]["source_item"]
    packets = {}
    for part in range(1, 5):
        source_packet = material["de_parts"][part]["packet"]
        source_anchor_rows = [anchor_view(material["de_paragraph_anchor"][unit["unit_id"]])
                              for unit in material["de_parts"][part]["paragraph_units"]]
        target_anchor_rows = [target_anchors[row["unit_id"]]
                              for row in material["target_layers"][part]["records"]]
        source_cfg = de_config[part]
        packet = {
            "$schema": schema_uri, "schema_version": "tos_translation_alignment_packet_v1",
            "packet_id": packet_id[f"part-{part}"], "packet_version": 1,
            "supersedes_packet_ref": None, "content_posture": "source_bound",
            "granularity": "paragraph",
            "source_side": {
                "side_role": "source", "work_ref": WORK_REF,
                "expression_ref": source_cfg["expression_ref"], "edition_ref": source_cfg["edition_ref"],
                "item_ref": source_cfg["item_ref"], "file_ref": source_cfg["file_ref"],
                "file_sha256": source_cfg["file_sha256"],
                "text_layer_ref": source_packet["source_layer"]["text_layer_ref"],
                "text_layer_sha256": source_packet["source_layer"]["text_layer_sha256"],
                "language": "de", "segmentation": {
                    "artifact_ref": material["de_parts"][part]["packet_ref"],
                    "sha256": sha((REPO / material["de_parts"][part]["packet_ref"]).read_bytes()),
                    "state": "frozen"}, "tokenization": None, "anchors": source_anchor_rows,
                "rights_refs": [source_cfg["rights_ref"]], "visibility": "local_only",
                "publication_authorized": False,
            },
            "target_side": {
                "side_role": "target", "work_ref": WORK_REF,
                "expression_ref": ru_config["expression_ref"], "edition_ref": ru_config["edition_ref"],
                "item_ref": ru_config["item_ref"], "file_ref": ru_config["file_ref"],
                "file_sha256": ru_config["file_sha256"],
                "text_layer_ref": material["target_layers"][part]["ref"],
                "text_layer_sha256": material["target_layers"][part]["sha256"],
                "language": "ru", "segmentation": {"artifact_ref": f"{RU_ROUTE_REF}/paragraph-spine.v2.jsonl",
                    "sha256": sha((REPO / f"{RU_ROUTE_REF}/paragraph-spine.v2.jsonl").read_bytes()),
                    "state": "frozen"}, "tokenization": None, "anchors": target_anchor_rows,
                "rights_refs": [ru_config["rights_ref"]], "visibility": "local_only",
                "publication_authorized": False,
            },
            "alignments": alignments_by_part[part], "reviews": [], "projections": [],
            "rights_and_visibility": {"source_visibility": "local_only",
                "target_visibility": "local_only", "packet_visibility": "public_metadata_only",
                "effective_visibility": "local_only",
                "rights_record_refs": [source_cfg["rights_ref"], ru_config["rights_ref"]],
                "private_source_used": True, "publication_authorized": False,
                "inheritance_policy": "most_restrictive_side_or_packet_wins"},
            "authority_boundary": {"tree_role": "orientation", "graph_role": "relation",
                "source_role": "authority", "validators_prove_mechanics_not_truth": True,
                "alignment_is_claim_not_translation_truth": True,
                "machine_or_model_may_propose_not_accept": True,
                "exports_are_not_owner_truth": True, "unaligned_members_are_first_class": True,
                "lexical_equivalence_not_inferred": True, "canon_effect": False},
        }
        packets[part] = packet

    schema = load_json(SCHEMA_REF)
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    for part, packet in packets.items():
        errors = sorted(validator.iter_errors(packet), key=lambda error: list(error.path))
        if errors:
            raise AlignmentBuildError(f"part {part} packet schema failure: {errors[0].message}")

    audit_counts = Counter(row["comparison_posture"] for row in audit_rows)
    if audit_counts["same_paragraph_group"] != 3311 or len(audit_rows) - audit_counts["same_paragraph_group"] != 112:
        raise AlignmentBuildError(f"independent comparison denominator drift: {audit_counts}")
    status_counts = Counter(row["status"] for row in spine)
    shape_counts = Counter(row["correspondence_shape"] for row in spine)
    source_refs = [unit for row in spine for unit in row["source_paragraph_unit_refs"]]
    target_refs = [unit for row in spine for unit in row["target_paragraph_unit_refs"]]
    if len(source_refs) != EXPECTED["source_paragraphs"] or len(set(source_refs)) != len(source_refs):
        raise AlignmentBuildError("final source paragraph coverage is not exact")
    if len(target_refs) != EXPECTED["target_paragraphs"] or len(set(target_refs)) != len(target_refs):
        raise AlignmentBuildError("final target paragraph coverage is not exact")
    independent_disagreement_refs = {
        row["primary_proposal_ref"] for row in audit_rows if not row["same_paragraph_group"]
    }
    independent_disagreement_units = {
        unit
        for row in spine
        if row["primary_proposal_ref"] in independent_disagreement_refs
        for unit in row["source_paragraph_unit_refs"] + row["target_paragraph_unit_refs"]
    }
    independent_disagreement_by_part = Counter(
        row["part_order"] for row in spine
        if row["primary_proposal_ref"] in independent_disagreement_refs
    )
    machine_risk_refs = {row["primary_proposal_ref"] for row in spine if row["status"] != "proposed"}
    primary_profile_risk_refs = {
        row["primary_proposal_ref"] for row in spine if row["primary_profile_disagreement"]
    }
    primary_only_risk_refs = machine_risk_refs - independent_disagreement_refs
    if len(independent_disagreement_units) != 263:
        raise AlignmentBuildError(
            f"independent disagreement affected-unit drift: {len(independent_disagreement_units)}"
        )
    if independent_disagreement_by_part != Counter({1: 2, 2: 8, 3: 53, 4: 49}):
        raise AlignmentBuildError(
            f"independent disagreement part census drift: {independent_disagreement_by_part}"
        )
    if (len(primary_profile_risk_refs),
            len(primary_profile_risk_refs & independent_disagreement_refs),
            len(primary_only_risk_refs)) != (20, 12, 8):
        raise AlignmentBuildError(
            "primary-profile risk union drift: "
            f"profile={len(primary_profile_risk_refs)} "
            f"overlap={len(primary_profile_risk_refs & independent_disagreement_refs)} "
            f"primary_only={len(primary_only_risk_refs)}"
        )
    if status_counts["deferred"] != 9 or any(
            row["status"] == "proposed" and (
                not row["independent_challenger_same_paragraph_group"]
                or row["primary_profile_disagreement"]
                or row["verse_barrier_risk"])
            for row in spine):
        raise AlignmentBuildError(f"final risk/status posture drift: {status_counts}")
    if len(conflict_rows) != len(machine_risk_refs):
        raise AlignmentBuildError("machine-risk union is not closed by the conflict ledger")

    diagnostics = []
    for part in range(1, 5):
        for reading in range(1, READINGS_BY_PART[part] + 1):
            rows = [row for row in spine if row["part_order"] == part
                    and row["reading_ordinal_within_part"] == reading]
            diagnostics.append({
                "schema_version": "tos_zarathustra_de_ru_paragraph_alignment_reading_diagnostic_v1",
                "part_order": part, "part_label": PART_LABEL[part],
                "reading_ordinal_within_part": reading, "mapping_count": len(rows),
                "source_paragraph_count": sum(len(row["source_paragraph_unit_refs"]) for row in rows),
                "target_paragraph_count": sum(len(row["target_paragraph_unit_refs"]) for row in rows),
                "status_counts": dict(sorted(Counter(row["status"] for row in rows).items())),
                "correspondence_shape_counts": dict(sorted(Counter(row["correspondence_shape"] for row in rows).items())),
                "independent_challenger_disagreement_count": sum(
                    not row["independent_challenger_same_paragraph_group"] for row in rows),
                "verse_scope_exclusion_present": (part, reading) in risk_readings,
                "verse_barrier_risk_mapping_count": sum(
                    row["cross_kind_or_verse_scope_risk"] for row in rows),
                "order_posture": "monotonic_machine_proposal_within_reading",
                "source_text_included": False, "semantic_equivalence_asserted": False,
            })
    if len(diagnostics) != EXPECTED["readings"]:
        raise AlignmentBuildError("reading diagnostics census drift")

    coverage = {
        "schema_version": "tos_zarathustra_de_ru_paragraph_alignment_coverage_receipt_v1",
        "status": "complete_machine_proposal_coverage",
        "packet_count": 4, "schema_valid_packet_count": 4,
        "reading_pair_count": EXPECTED["readings"],
        "reading_pairs_by_part": {PART_LABEL[p]: n for p, n in READINGS_BY_PART.items()},
        "alignment_count": len(spine), "status_counts": dict(sorted(status_counts.items())),
        "correspondence_shape_counts": dict(sorted(shape_counts.items())),
        "source_paragraph_expected": EXPECTED["source_paragraphs"],
        "source_paragraph_covered": len(source_refs), "source_paragraph_duplicate_count": 0,
        "source_paragraph_uncovered_count": 0,
        "target_paragraph_expected": EXPECTED["target_paragraphs"],
        "target_paragraph_covered": len(target_refs), "target_paragraph_duplicate_count": 0,
        "target_paragraph_uncovered_count": 0,
        "source_only_outside_81_reading_scope_count": 9,
        "internal_null_side_transition_count": 12,
        "cross_kind_out_of_paragraph_scope_candidate_count": 11,
        "structural_role_mismatch_candidate_count": 1,
        "independent_challenger_same_group_count": 3311,
        "independent_challenger_disagreement_count": 112,
        "independent_challenger_disagreement_by_part": {
            PART_LABEL[part]: independent_disagreement_by_part[part] for part in range(1, 5)
        },
        "independent_challenger_disagreement_paragraph_unit_count": len(independent_disagreement_units),
        "primary_profile_disagreement_count": len(primary_profile_risk_refs),
        "primary_profile_and_independent_disagreement_overlap_count": len(
            primary_profile_risk_refs & independent_disagreement_refs),
        "primary_only_machine_risk_count": len(primary_only_risk_refs),
        "machine_risk_union_count": len(machine_risk_refs),
        "conflict_ledger_count": len(conflict_rows),
        "unaccounted_machine_risk_count": 0,
        "preserved_ambiguous_count": status_counts["ambiguous"],
        "preserved_deferred_count": status_counts["deferred"],
        "source_verse_group_exclusion_count": EXPECTED["source_verse_groups"],
        "source_verse_line_exclusion_count": EXPECTED["source_verse_lines"],
        "target_verse_group_exclusion_count": EXPECTED["target_verse_groups"],
        "target_verse_line_exclusion_count": EXPECTED["target_verse_lines"],
        "reviews_count": 0, "projections_count": 0, "accepted_alignment_count": 0,
        "source_text_included": False, "semantic_equivalence_asserted": False,
        "lexical_equivalence_inferred": False, "canon_effect": False,
    }
    verification = {
        "schema_version": "tos_zarathustra_de_ru_paragraph_alignment_agent_verification_v1",
        "status": "complete_machine_proposal_materialization",
        "checks": {"four_packets_schema_valid": True, "all_81_reading_pairs_present": True,
                   "source_paragraphs_exact_once_3447": True,
                   "target_paragraphs_exact_once_3569": True,
                   "outside_reading_source_units_first_class_deferred": True,
                   "internal_null_transitions_not_called_translator_omissions": True,
                   "source_verse_groups_and_lines_explicitly_excluded": True,
                   "target_verse_groups_and_lines_explicitly_excluded": True,
                   "independent_challenger_3311_112_denominator_preserved": True,
                   "all_independent_disagreements_ambiguous_or_deferred": True,
                   "machine_risk_union_fully_accounted_in_conflict_ledger": True,
                   "reviews_empty": True, "projections_empty": True,
                   "accepted_alignment_count_zero": True, "tracked_outputs_text_free": True,
                   "opaque_identity_issuance_closed": True},
        "human_acceptance": False, "translation_truth_verified": False,
        "semantic_equivalence_asserted": False, "lexical_equivalence_inferred": False,
        "graph_projection_created": False, "canon_effect": False,
        "authority_boundary": "mechanical packet and proposal coverage only",
        "source_text_included": False,
    }
    summary = {
        "schema_version": "tos_zarathustra_de_ru_paragraph_alignment_summary_v1",
        "status": "complete_machine_proposal_not_accepted_alignment",
        "packet_count": 4, "reading_pair_count": 81, "alignment_count": len(spine),
        "source_paragraph_count": len(source_refs), "target_paragraph_count": len(target_refs),
        "status_counts": dict(sorted(status_counts.items())),
        "independent_challenger_comparison": {"same_group": 3311, "disagreement": 112,
                                               "affected_paragraph_units": len(independent_disagreement_units),
                                               "disagreement_by_part": {
                                                   PART_LABEL[part]: independent_disagreement_by_part[part]
                                                   for part in range(1, 5)}},
        "machine_risk_union": {"mapping_count": len(machine_risk_refs),
                               "independent_disagreement_count": 112,
                               "primary_profile_disagreement_count": len(primary_profile_risk_refs),
                               "primary_profile_and_independent_overlap_count": len(
                                   primary_profile_risk_refs & independent_disagreement_refs),
                               "primary_only_risk_count": len(primary_only_risk_refs)},
        "scope_exclusions": {"source_verse_groups": 39, "source_verse_lines": 368,
                             "target_verse_groups": 13, "target_verse_lines": 359},
        "reviews_count": 0, "projections_count": 0, "accepted_alignment_count": 0,
        "source_text_included": False, "semantic_equivalence_asserted": False,
        "canon_effect": False,
    }
    provenance = [
        {"event_ref": "tos.event.zarathustra-de-ru-paragraph-alignment-v1.primary-import",
         "event_kind": "primary_machine_proposal_import", "input_ref": PRIMARY_RECEIPT_REF},
        {"event_ref": "tos.event.zarathustra-de-ru-paragraph-alignment-v1.challenger-import",
         "event_kind": "independent_mixed_content_challenger_import", "input_ref": CHALLENGER_RECEIPT_REF},
        {"event_ref": "tos.event.zarathustra-de-ru-paragraph-alignment-v1.target-layer",
         "event_kind": "private_russian_paragraph_layer_derivation", "input_ref": f"{RU_ROUTE_REF}/paragraph-spine.v2.jsonl"},
        {"event_ref": "tos.event.zarathustra-de-ru-paragraph-alignment-v1.build",
         "event_kind": "translation_alignment_packet_proposal_materialization", "input_ref": SCHEMA_REF},
    ]
    for row in provenance:
        row.update({"schema_version": "tos_zarathustra_de_ru_paragraph_alignment_provenance_v1",
                    "event_date": "2026-09-01", "method_output_posture": "proposal_not_truth",
                    "human_review": False, "source_text_included": False,
                    "semantic_equivalence_asserted": False, "canon_effect": False})

    artifacts = {PACKET_REFS[part]: json_bytes(packet) for part, packet in packets.items()}
    artifacts.update({
        OUTPUT_REFS["spine"]: jsonl_bytes(spine),
        OUTPUT_REFS["coverage"]: json_bytes(coverage),
        OUTPUT_REFS["diagnostics"]: jsonl_bytes(diagnostics),
        OUTPUT_REFS["conflicts"]: jsonl_bytes(conflict_rows),
        OUTPUT_REFS["exclusions"]: jsonl_bytes(exclusions),
        OUTPUT_REFS["challenger_audit"]: jsonl_bytes(audit_rows),
        OUTPUT_REFS["verification"]: json_bytes(verification),
        OUTPUT_REFS["summary"]: json_bytes(summary),
        OUTPUT_REFS["provenance"]: jsonl_bytes(provenance),
    })
    static_refs = [PRIMARY_REF, PRIMARY_DIAGNOSTICS_REF, PRIMARY_METHOD_REF,
                   PRIMARY_RECEIPT_REF, CHALLENGER_REF, CHALLENGER_RECEIPT_REF,
                   ISSUANCE_REF, SCHEMA_REF, DE_PLAN_REF, RU_PLAN_REF]
    manifest = {
        "schema_version": "tos_zarathustra_de_ru_paragraph_alignment_manifest_v1",
        "static_inputs": {ref: {"sha256": sha((REPO / ref).read_bytes()),
                                 "byte_size": (REPO / ref).stat().st_size}
                          for ref in static_refs},
        "generated_outputs": {ref: {"sha256": sha(payload), "byte_size": len(payload)}
                              for ref, payload in sorted(artifacts.items())},
        "private_outputs": {material["target_layers"][part]["ref"]: {
            "sha256": material["target_layers"][part]["sha256"],
            "byte_size": len(material["target_layers"][part]["payload"]),
            "required_mode": "0600"} for part in range(1, 5)},
        "source_text_included": False, "accepted_alignment_count": 0,
        "semantic_equivalence_asserted": False, "canon_effect": False,
    }
    artifacts[OUTPUT_REFS["manifest"]] = json_bytes(manifest)
    private = {material["target_layers"][part]["ref"]: material["target_layers"][part]["payload"]
               for part in range(1, 5)}
    return artifacts, private


def write_outputs(artifacts: dict[str, bytes], private: dict[str, bytes]) -> None:
    for ref, payload in artifacts.items():
        path = REPO / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
    for ref, payload in private.items():
        path = REPO / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        os.chmod(path, stat.S_IRUSR | stat.S_IWUSR)


def check_outputs(artifacts: dict[str, bytes], private: dict[str, bytes]) -> None:
    drift = [ref for ref, payload in artifacts.items()
             if not (REPO / ref).is_file() or (REPO / ref).read_bytes() != payload]
    if drift:
        raise AlignmentBuildError("tracked generated drift: " + ", ".join(drift))
    private_drift = []
    for ref, payload in private.items():
        path = REPO / ref
        if not path.is_file() or path.read_bytes() != payload or stat.S_IMODE(path.stat().st_mode) != 0o600:
            private_drift.append(ref)
    if private_drift:
        raise AlignmentBuildError("private layer drift or mode failure: " + ", ".join(private_drift))


def validate_tracked() -> dict[str, Any]:
    manifest = load_json(OUTPUT_REFS["manifest"])
    for ref, receipt in manifest["static_inputs"].items():
        path = REPO / ref
        if not path.is_file() or sha(path.read_bytes()) != receipt["sha256"] \
                or path.stat().st_size != receipt["byte_size"]:
            raise AlignmentBuildError(f"manifest static-input drift: {ref}")
    for ref, receipt in manifest["generated_outputs"].items():
        path = REPO / ref
        if not path.is_file() or sha(path.read_bytes()) != receipt["sha256"] \
                or path.stat().st_size != receipt["byte_size"]:
            raise AlignmentBuildError(f"manifest generated-output drift: {ref}")
    schema = load_json(SCHEMA_REF)
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    packets = [load_json(PACKET_REFS[part]) for part in range(1, 5)]
    for part, packet in enumerate(packets, 1):
        errors = sorted(validator.iter_errors(packet), key=lambda error: list(error.path))
        if errors:
            raise AlignmentBuildError(f"tracked part {part} packet schema failure: {errors[0].message}")
        assert_text_free(packet, PACKET_REFS[part])
        if packet["reviews"] or packet["projections"]:
            raise AlignmentBuildError("reviews and projections must remain empty")
        if any(row["status"] not in {"proposed", "ambiguous", "deferred"}
               for row in packet["alignments"]):
            raise AlignmentBuildError("machine packet contains a decided status")
        anchors = {row["anchor_ref"] for row in packet["source_side"]["anchors"] +
                   packet["target_side"]["anchors"]}
        for row in packet["alignments"]:
            if not set(row["ordered_source_anchor_refs"] + row["ordered_target_anchor_refs"]) <= anchors:
                raise AlignmentBuildError("packet alignment anchor closure failure")
            if row["review_refs"]:
                raise AlignmentBuildError("machine proposal has review refs")
            if row["correspondence_shape"] in {"source_omission", "target_addition", "unresolved"} \
                    and row["status"] == "proposed":
                raise AlignmentBuildError("null or unresolved mapping cannot be proposed")
    spine = load_jsonl(OUTPUT_REFS["spine"])
    audit = load_jsonl(OUTPUT_REFS["challenger_audit"])
    conflicts = load_jsonl(OUTPUT_REFS["conflicts"])
    exclusions = load_jsonl(OUTPUT_REFS["exclusions"])
    diagnostics = load_jsonl(OUTPUT_REFS["diagnostics"])
    coverage = load_json(OUTPUT_REFS["coverage"])
    verification = load_json(OUTPUT_REFS["verification"])
    for ref, value in ((OUTPUT_REFS["spine"], spine), (OUTPUT_REFS["challenger_audit"], audit),
                       (OUTPUT_REFS["conflicts"], conflicts), (OUTPUT_REFS["exclusions"], exclusions),
                       (OUTPUT_REFS["diagnostics"], diagnostics), (OUTPUT_REFS["coverage"], coverage),
                       (OUTPUT_REFS["verification"], verification)):
        assert_text_free(value, ref)
    if len(spine) != EXPECTED["primary_mappings"] or len(audit) != len(spine):
        raise AlignmentBuildError("alignment/audit census drift")
    status_counts = Counter(row["status"] for row in spine)
    if status_counts != Counter(coverage["status_counts"]):
        raise AlignmentBuildError("tracked status census differs from coverage receipt")
    same = sum(row["same_paragraph_group"] for row in audit)
    if (same, len(audit) - same) != (3311, 112):
        raise AlignmentBuildError("tracked independent comparison denominator drift")
    if any(row["status"] == "proposed" for row in spine
           if not row["independent_challenger_same_paragraph_group"]):
        raise AlignmentBuildError("independent disagreement leaked into proposed status")
    machine_risk_refs = {row["primary_proposal_ref"] for row in spine if row["status"] != "proposed"}
    conflict_risk_refs = {row["primary_proposal_ref"] for row in conflicts}
    if conflict_risk_refs != machine_risk_refs \
            or len(conflicts) != coverage["machine_risk_union_count"] \
            or any(row["machine_resolution"] != "none" for row in conflicts):
        raise AlignmentBuildError("conflict ambiguity was not preserved")
    source_refs = [unit for row in spine for unit in row["source_paragraph_unit_refs"]]
    target_refs = [unit for row in spine for unit in row["target_paragraph_unit_refs"]]
    if len(source_refs) != 3447 or len(set(source_refs)) != 3447 \
            or len(target_refs) != 3569 or len(set(target_refs)) != 3569:
        raise AlignmentBuildError("tracked paragraph exact-once coverage failure")
    if len(diagnostics) != 81:
        raise AlignmentBuildError("tracked reading diagnostic census drift")
    if (len([row for row in exclusions if row["side"] == "source"]),
        sum(len(row["ordered_excluded_verse_line_refs"]) for row in exclusions if row["side"] == "source"),
        len([row for row in exclusions if row["side"] == "target"]),
        sum(len(row["ordered_excluded_verse_line_refs"]) for row in exclusions if row["side"] == "target")) != (39, 368, 13, 359):
        raise AlignmentBuildError("tracked verse exclusion closure failure")
    all_alignment_ids = [row["alignment_id"] for packet in packets for row in packet["alignments"]]
    all_claim_ids = [row["claim_id"] for packet in packets for row in packet["alignments"]]
    if len(all_alignment_ids) != len(set(all_alignment_ids)) or len(all_claim_ids) != len(set(all_claim_ids)):
        raise AlignmentBuildError("alignment or claim identity collision")
    if coverage["accepted_alignment_count"] != 0 or verification["human_acceptance"]:
        raise AlignmentBuildError("acceptance boundary drift")
    return {"status": "pass", "packets": 4, "reading_pairs": 81,
            "alignments": len(spine), "source_paragraphs": len(source_refs),
            "target_paragraphs": len(target_refs),
            "proposed": status_counts["proposed"],
            "ambiguous": status_counts["ambiguous"],
            "deferred": status_counts["deferred"], "accepted": 0,
            "machine_risk_union": len(machine_risk_refs),
            "independent_agree": 3311, "independent_disagree": 112}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    actions = parser.add_mutually_exclusive_group(required=True)
    actions.add_argument("--import-primary", type=Path)
    actions.add_argument("--import-challenger", type=Path)
    actions.add_argument("--issue-identities", action="store_true")
    actions.add_argument("--build", action="store_true")
    actions.add_argument("--check", action="store_true")
    actions.add_argument("--validate-tracked", action="store_true")
    args = parser.parse_args(argv)
    if args.import_primary:
        import_primary(args.import_primary)
        print(json.dumps({"status": "imported", "input": "primary"}, indent=2)); return 0
    if args.import_challenger:
        import_challenger(args.import_challenger)
        print(json.dumps({"status": "imported", "input": "independent_challenger"}, indent=2)); return 0
    if args.validate_tracked:
        print(json.dumps(validate_tracked(), indent=2, sort_keys=True)); return 0
    material = reconstruct_material()
    if args.issue_identities:
        issue_identities(material)
        print(json.dumps({"status": "issued", "identity_ref": ISSUANCE_REF}, indent=2)); return 0
    artifacts, private = build_artifacts(material)
    if args.build:
        write_outputs(artifacts, private)
    else:
        check_outputs(artifacts, private)
    print(json.dumps(json.loads(artifacts[OUTPUT_REFS["summary"]]), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except AlignmentBuildError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        raise SystemExit(1)
