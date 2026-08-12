#!/usr/bin/env python3
"""Build one local-only, non-promoting Antonovsky 2007/1911 collation proposal.

The 2007 input is an already-preserved, unattested human Workbench observation.
It is useful evidence, but not a pass receipt, gold, accepted source text, or a
review.  The builder copies the exact observation into ignored owner-local
custody, proposes one sentence boundary, compares it with the existing 1911
sentence proposal, and emits text-free tracked records only.
"""

from __future__ import annotations

import argparse
import copy
import difflib
import hashlib
import json
import os
import platform
import subprocess
import sys
import unicodedata
from pathlib import Path
from typing import Any, Callable

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OWNER_ROOT = Path("/srv/AbyssOS/Tree-of-Sophia")
DEFAULT_ARTIFACT_ROOT = Path("/srv/abyss-machine/storage/artifacts")
PLAN_REF = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "antonovsky-2007-1911-opening-sentence-collation.plan.v1.json"
)
BUILDER_REF = Path("scripts/build_antonovsky_2007_1911_collation.py")
ANCHOR_SCHEMA_REF = Path("ToS/contracts/source-anchor-v2.schema.json")
LAYER_SCHEMA_REF = Path("ToS/contracts/source-text-layer.schema.json")
UNIT_SCHEMA_REF = Path("ToS/contracts/source-text-unit-packet-v1.schema.json")
COLLATION_SCHEMA_REF = Path(
    "ToS/contracts/witness-text-collation-packet-v1.schema.json"
)
PROVENANCE_SCHEMA_REF = Path("ToS/contracts/provenance-event-v2.schema.json")
EVENT_ID = (
    "tos.event.alignment.antonovsky-2007-1911-opening-sentence-collation."
    "2026-08-12"
)
SOFTWARE_AGENT = "software:tos-antonovsky-2007-1911-collation-builder"


class AntonovskyCollationError(RuntimeError):
    """Raised when exact local inputs or non-promotion closure drift."""


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _json_bytes(payload: Any) -> bytes:
    return (
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def _canonical_value_sha256(payload: Any) -> str:
    return _sha256_bytes(
        json.dumps(
            payload,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    )


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise AntonovskyCollationError(f"cannot load JSON {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise AntonovskyCollationError(f"JSON root is not an object: {path}")
    return payload


def _checked_path(root: Path, ref: str, *, label: str) -> Path:
    path = root / ref
    try:
        resolved = path.resolve(strict=True)
    except OSError as exc:
        raise AntonovskyCollationError(f"{label} is absent: {ref}") from exc
    root_resolved = root.resolve()
    if not resolved.is_relative_to(root_resolved):
        raise AntonovskyCollationError(f"{label} escapes its owner root: {ref}")
    if path.is_symlink() or not path.is_file():
        raise AntonovskyCollationError(f"{label} is not a regular file: {ref}")
    return path


def _verify_digest(path: Path, expected: str, *, label: str) -> None:
    actual = _sha256(path)
    if actual != expected:
        raise AntonovskyCollationError(
            f"{label} digest drifted: expected {expected}, got {actual}"
        )


def _is_ignored(repo_root: Path, ref: str) -> bool:
    process = subprocess.run(
        ["git", "check-ignore", "-q", "--", ref],
        cwd=repo_root,
        check=False,
    )
    return process.returncode == 0


def _artifact_ref(relative_path: str) -> str:
    return f"abyss-stack:artifact/{relative_path}"


def _find_observation_row(
    autosave: dict[str, Any], sample_id: str
) -> dict[str, Any]:
    rows = autosave.get("rows")
    if not isinstance(rows, list):
        raise AntonovskyCollationError("autosave rows are absent")
    matches = [
        row
        for row in rows
        if isinstance(row, dict) and row.get("unit_id") == sample_id
    ]
    if len(matches) != 1:
        raise AntonovskyCollationError(
            f"expected one autosave row for {sample_id}, found {len(matches)}"
        )
    return matches[0]


def _read_human_observation(
    *, artifact_root: Path, plan: dict[str, Any]
) -> tuple[str, dict[str, Any], dict[str, Path]]:
    expected = plan["human_observation"]
    paths = {
        "autosave": _checked_path(
            artifact_root,
            expected["autosave_relative_path"],
            label="source autosave",
        ),
        "closure": _checked_path(
            artifact_root,
            expected["closure_relative_path"],
            label="sparse-calibration closure",
        ),
        "closure_receipt": _checked_path(
            artifact_root,
            expected["closure_receipt_relative_path"],
            label="sparse-calibration closure receipt",
        ),
    }
    for key, digest_key in (
        ("autosave", "autosave_sha256"),
        ("closure", "closure_sha256"),
        ("closure_receipt", "closure_receipt_sha256"),
    ):
        _verify_digest(paths[key], expected[digest_key], label=key)

    autosave = _load_json(paths["autosave"])
    closure = _load_json(paths["closure"])
    receipt = _load_json(paths["closure_receipt"])
    if autosave.get("schema_version") != "tos_human_review_workbench_state_v1":
        raise AntonovskyCollationError("unexpected Workbench autosave schema")
    if autosave.get("protocol_id") != expected["expected_protocol_id"]:
        raise AntonovskyCollationError("Workbench protocol drifted")
    if autosave.get("status") != expected["expected_state_status"]:
        raise AntonovskyCollationError("Workbench status drifted")
    if autosave.get("submitted_at_utc") is not None:
        raise AntonovskyCollationError(
            "Workbench autosave now claims a submission not admitted by this plan"
        )
    if autosave.get("reviewer_ref") != expected["reviewer_ref"]:
        raise AntonovskyCollationError("Workbench reviewer binding drifted")

    if closure.get("schema_version") != "tos_sparse_calibration_closure_v1":
        raise AntonovskyCollationError("unexpected sparse-calibration closure schema")
    if closure.get("status") != "closed-no-human-debt":
        raise AntonovskyCollationError("sparse-calibration closure status drifted")
    if closure.get("attestation_status") != "not-collected":
        raise AntonovskyCollationError("closure unexpectedly claims attestation")
    if closure.get("promotion_authorized") is not False:
        raise AntonovskyCollationError("closure unexpectedly authorizes promotion")
    if closure.get("source_autosave_sha256") != expected["autosave_sha256"]:
        raise AntonovskyCollationError("closure autosave binding drifted")
    counts = closure.get("counts", {})
    if not isinstance(counts, dict) or counts.get("human_debt_units") != 0:
        raise AntonovskyCollationError("closure human-debt boundary drifted")
    selected = closure.get("selected_calibration_units")
    if not isinstance(selected, list) or len(selected) != 1:
        raise AntonovskyCollationError("closure selected-unit cardinality drifted")
    selected_row = selected[0]
    if not isinstance(selected_row, dict):
        raise AntonovskyCollationError("closure selected-unit row is invalid")
    if selected_row.get("unit_id") != expected["sample_id"]:
        raise AntonovskyCollationError("closure selected-unit identity drifted")
    if selected_row.get("classification") != "rare-independent-transcription-calibration":
        raise AntonovskyCollationError("closure observation classification drifted")
    if selected_row.get("attestation_status") != "not-collected":
        raise AntonovskyCollationError("selected observation unexpectedly attested")
    if selected_row.get("promotion_authorized") is not False:
        raise AntonovskyCollationError("selected observation unexpectedly promoted")
    if float(selected_row.get("active_seconds", -1)) != float(
        expected["expected_active_seconds"]
    ):
        raise AntonovskyCollationError("selected observation active time drifted")

    if receipt.get("schema_version") != "tos_sparse_calibration_closure_receipt_v1":
        raise AntonovskyCollationError("unexpected closure receipt schema")
    if receipt.get("status") != "closure-fixed":
        raise AntonovskyCollationError("closure receipt status drifted")
    if receipt.get("closure_sha256") != expected["closure_sha256"]:
        raise AntonovskyCollationError("closure receipt digest binding drifted")
    if receipt.get("source_autosave_sha256") != expected["autosave_sha256"]:
        raise AntonovskyCollationError("receipt autosave digest binding drifted")

    row = _find_observation_row(autosave, expected["sample_id"])
    values = row.get("values")
    if not isinstance(values, dict):
        raise AntonovskyCollationError("observation values are absent")
    transcription = values.get("diplomatic_transcription")
    if not isinstance(transcription, str) or not transcription:
        raise AntonovskyCollationError("observation transcription is absent")
    if values.get("decision") != expected["expected_decision"]:
        raise AntonovskyCollationError("observation decision drifted")
    if float(row.get("active_seconds", -1)) != float(
        expected["expected_active_seconds"]
    ):
        raise AntonovskyCollationError("autosave active time drifted")
    if selected_row.get("calibration_transcription_sha256") != _sha256_bytes(
        transcription.encode("utf-8")
    ):
        raise AntonovskyCollationError("closure transcription digest drifted")
    required_observed_fields = {
        "decision",
        "diplomatic_transcription",
        "layout_and_reading_order",
        "page_and_region_resolved",
        "source_damage_or_ambiguity",
        "source_legibility",
    }
    if set(selected_row.get("observed_fields", [])) != required_observed_fields:
        raise AntonovskyCollationError("closure observed-field set drifted")
    if not isinstance(values.get("source_damage_or_ambiguity"), str) or not values[
        "source_damage_or_ambiguity"
    ]:
        raise AntonovskyCollationError("human ambiguity observation is absent")
    return transcription, copy.deepcopy(values), paths


def _verify_2007_source(
    *, repo_root: Path, owner_root: Path, plan: dict[str, Any]
) -> dict[str, Path]:
    source = plan["witness_2007"]
    paths = {
        "pdf": _checked_path(
            owner_root, source["source_relative_ref"], label="2007 source PDF"
        ),
        "manifest": _checked_path(
            repo_root, source["item_manifest_ref"], label="2007 item manifest"
        ),
        "inventory": _checked_path(
            repo_root,
            source["resource_inventory_ref"],
            label="2007 resource inventory",
        ),
        "rights": _checked_path(
            repo_root, source["rights_ref"], label="2007 rights record"
        ),
    }
    _verify_digest(paths["pdf"], source["file_sha256"], label="2007 source PDF")
    _verify_digest(paths["rights"], source["rights_sha256"], label="2007 rights")
    manifest = _load_json(paths["manifest"])
    if manifest.get("item_id") != source["item_ref"]:
        raise AntonovskyCollationError("2007 item manifest identity drifted")
    payloads = manifest.get("payload_files", [])
    if not isinstance(payloads, list) or len(payloads) != 1:
        raise AntonovskyCollationError("2007 manifest payload cardinality drifted")
    payload = payloads[0]
    if not isinstance(payload, dict) or payload.get("file_id") != source["file_ref"]:
        raise AntonovskyCollationError("2007 source file identity drifted")
    if payload.get("sha256") != source["file_sha256"]:
        raise AntonovskyCollationError("2007 manifest fixity drifted")
    inventory = _load_json(paths["inventory"])
    resources: list[dict[str, Any]] = []
    for file_row in inventory.get("files", []):
        if not isinstance(file_row, dict) or file_row.get("file_id") != source["file_ref"]:
            continue
        resources.extend(
            row for row in file_row.get("resources", []) if isinstance(row, dict)
        )
    matches = [
        row for row in resources if row.get("resource_id") == source["page_resource_id"]
    ]
    if len(matches) != 1:
        raise AntonovskyCollationError("2007 page resource binding drifted")
    locator = matches[0].get("locator", {})
    expected_locator = {
        "page_index": source["page_number"],
        "width_points": source["page_width_points"],
        "height_points": source["page_height_points"],
        "rotation_degrees": source["page_rotation_degrees"],
    }
    if locator != expected_locator:
        raise AntonovskyCollationError("2007 page geometry drifted")
    rights = _load_json(paths["rights"])
    if rights.get("visibility") != "local_only":
        raise AntonovskyCollationError("2007 rights visibility widened")
    if rights.get("redistribution_posture") != "not_authorized":
        raise AntonovskyCollationError("2007 redistribution posture widened")
    if rights.get("derivative_posture") != "local_research_only":
        raise AntonovskyCollationError("2007 derivative posture widened")
    if rights.get("review_status") != "unreviewed":
        raise AntonovskyCollationError("2007 rights review posture drifted")
    return paths


def _verify_1911_target(
    *, repo_root: Path, owner_root: Path, plan: dict[str, Any]
) -> tuple[str, dict[str, Path]]:
    target = plan["witness_1911"]
    paths = {
        "text": _checked_path(
            owner_root, target["text_layer_ref"], label="1911 private text layer"
        ),
        "layer": _checked_path(
            repo_root,
            target["text_layer_record_ref"],
            label="1911 text-layer record",
        ),
        "unit": _checked_path(
            repo_root,
            target["text_unit_packet_ref"],
            label="1911 sentence-unit packet",
        ),
        "rights": _checked_path(
            repo_root, target["rights_ref"], label="1911 rights record"
        ),
    }
    for key, expected in (
        ("text", target["text_layer_sha256"]),
        ("layer", target["text_layer_record_sha256"]),
        ("unit", target["text_unit_packet_sha256"]),
        ("rights", target["rights_sha256"]),
    ):
        _verify_digest(paths[key], expected, label=f"1911 {key}")
    text = paths["text"].read_text(encoding="utf-8")
    start = target["sentence_start"]
    end = target["sentence_end"]
    if not (start == 0 < end < len(text)):
        raise AntonovskyCollationError("1911 sentence selector is invalid")
    if text.find(".") + 1 != end:
        raise AntonovskyCollationError("1911 first-full-stop guard drifted")
    sentence = text[start:end]
    if len(sentence) != target["sentence_codepoints"]:
        raise AntonovskyCollationError("1911 sentence length drifted")
    if _sha256_bytes(sentence.encode("utf-8")) != target["sentence_sha256"]:
        raise AntonovskyCollationError("1911 sentence digest drifted")
    packet = _load_json(paths["unit"])
    anchors = {
        row.get("anchor_ref"): row
        for row in packet.get("anchors", [])
        if isinstance(row, dict)
    }
    anchor = anchors.get(target["anchor_ref"])
    if not isinstance(anchor, dict):
        raise AntonovskyCollationError("1911 sentence anchor is absent")
    if anchor.get("exact_sha256") != target["sentence_sha256"]:
        raise AntonovskyCollationError("1911 sentence anchor digest drifted")
    if anchor.get("selector", {}).get("start") != start or anchor.get(
        "selector", {}
    ).get("end") != end:
        raise AntonovskyCollationError("1911 sentence anchor selector drifted")
    return sentence, paths


def _select_2007_sentence(
    text: str, plan: dict[str, Any]
) -> tuple[str, str, str, str]:
    source = plan["witness_2007"]
    encoded = text.encode("utf-8")
    if len(text) != source["text_layer_codepoints"]:
        raise AntonovskyCollationError("2007 observation code-point count drifted")
    if len(encoded) != source["text_layer_bytes"]:
        raise AntonovskyCollationError("2007 observation byte count drifted")
    if _sha256_bytes(encoded) != source["text_layer_sha256"]:
        raise AntonovskyCollationError("2007 observation content digest drifted")
    dots = [index for index, value in enumerate(text) if value == "."]
    if len(dots) < 2:
        raise AntonovskyCollationError("2007 observation lacks two U+002E guards")
    heading_end = dots[0] + 1
    sentence_end = dots[1] + 1
    sentence_start = heading_end
    while sentence_start < sentence_end and text[sentence_start].isspace():
        sentence_start += 1
    if (
        heading_end != source["heading_end"]
        or sentence_start != source["sentence_start"]
        or sentence_end != source["sentence_end"]
    ):
        raise AntonovskyCollationError("2007 sentence selection law drifted")
    heading = text[source["heading_start"] : source["heading_end"]]
    interstitial = text[
        source["interstitial_start"] : source["interstitial_end"]
    ]
    sentence = text[source["sentence_start"] : source["sentence_end"]]
    remainder = text[source["remainder_start"] : source["remainder_end"]]
    if not interstitial or not all(value.isspace() for value in interstitial):
        raise AntonovskyCollationError("2007 interstitial whitespace guard drifted")
    checks = (
        (heading, source["heading_sha256"], "heading"),
        (sentence, source["sentence_sha256"], "sentence"),
        (remainder, source["remainder_sha256"], "remainder"),
    )
    for value, expected, label in checks:
        if _sha256_bytes(value.encode("utf-8")) != expected:
            raise AntonovskyCollationError(f"2007 {label} digest drifted")
    if len(sentence) != source["sentence_codepoints"] or len(
        sentence.encode("utf-8")
    ) != source["sentence_bytes"]:
        raise AntonovskyCollationError("2007 sentence size drifted")
    if sentence[0].isspace() or not sentence.endswith("."):
        raise AntonovskyCollationError("2007 selected sentence boundary is malformed")
    if source["remainder_end"] != len(text):
        raise AntonovskyCollationError("2007 declared scope does not close over layer")
    return heading, interstitial, sentence, remainder


def _comparison_views(
    source_sentence: str,
    target_sentence: str,
    plan: dict[str, Any],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    transforms: dict[str, Callable[[str], str]] = {
        "exact": lambda value: value,
        "unicode-nfc": lambda value: unicodedata.normalize("NFC", value),
        "whitespace-collapsed": lambda value: " ".join(value.split()),
        "alphanumeric-casefold": lambda value: "".join(
            character.casefold() for character in value if character.isalnum()
        ),
    }
    expected_rows = plan["comparison_method"]["views"]
    if [row.get("view_id") for row in expected_rows] != list(transforms):
        raise AntonovskyCollationError("comparison view order or identity drifted")
    public_rows: list[dict[str, Any]] = []
    private_rows: list[dict[str, Any]] = []
    for expected in expected_rows:
        view_id = expected["view_id"]
        left = transforms[view_id](source_sentence)
        right = transforms[view_id](target_sentence)
        matcher = difflib.SequenceMatcher(a=left, b=right, autojunk=False)
        opcodes = matcher.get_opcodes()
        script = ";".join(
            f"{tag}:{a_start}:{a_end}:{b_start}:{b_end}"
            for tag, a_start, a_end, b_start, b_end in opcodes
        )
        tag_counts = {
            tag: sum(1 for row in opcodes if row[0] == tag)
            for tag in ("equal", "replace", "delete", "insert")
        }
        extent_counts = {
            tag: sum(
                max(a_end - a_start, b_end - b_start)
                for row_tag, a_start, a_end, b_start, b_end in opcodes
                if row_tag == tag
            )
            for tag in ("equal", "replace", "delete", "insert")
        }
        actual = {
            "view_id": view_id,
            "normalization": expected["normalization"],
            "normalization_version": expected["normalization_version"],
            "witness_a_codepoints": len(left),
            "witness_b_codepoints": len(right),
            "exact_equal": left == right,
            "similarity_metric": "sequence_matcher_ratio",
            "similarity_score_ppm": round(matcher.ratio() * 1_000_000),
            "opcode_count": len(opcodes),
            "opcode_tag_counts": tag_counts,
            "opcode_extent_counts": extent_counts,
            "private_edit_script_sha256": _sha256_bytes(script.encode("utf-8")),
        }
        expected_actual = dict(expected)
        expected_actual["similarity_metric"] = "sequence_matcher_ratio"
        if actual != expected_actual:
            raise AntonovskyCollationError(
                f"comparison view {view_id} drifted from frozen plan"
            )
        public_rows.append(actual)
        private_rows.append(
            {
                **actual,
                "opcodes": [
                    {
                        "tag": tag,
                        "witness_a_start": a_start,
                        "witness_a_end": a_end,
                        "witness_b_start": b_start,
                        "witness_b_end": b_end,
                    }
                    for tag, a_start, a_end, b_start, b_end in opcodes
                ],
            }
        )
    return public_rows, private_rows


def _source_anchor(plan: dict[str, Any], plan_digest: str) -> dict[str, Any]:
    source = plan["witness_2007"]
    ids = plan["opaque_ids"]
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-anchor-v2.schema.json",
        "schema_version": "tos_source_anchor_v2",
        "anchor_id": ids["source_anchor_id"],
        "anchor_version": 1,
        "supersedes_anchor_ref": None,
        "passage_id": ids["source_passage_id"],
        "target": {
            "item_id": source["item_ref"],
            "file_id": source["file_ref"],
            "file_sha256": source["file_sha256"],
            "media_type": "application/pdf",
        },
        "selector_payload": {
            "kind": "selector_expression",
            "expression": {
                "mode": "single",
                "selector": {
                    "state": {
                        "state_type": "digest_state",
                        "representation_ref": source["source_relative_ref"],
                        "representation_sha256": source["file_sha256"],
                        "media_type": "application/pdf",
                        "character_normalization": "none",
                    },
                    "selector": {
                        "type": "page_region",
                        "page_identity": {"page_number": source["page_number"]},
                        "x": 0.0,
                        "y": 0.0,
                        "width": source["page_width_points"],
                        "height": source["page_height_points"],
                        "coordinate_space": "points",
                        "source_width": source["page_width_points"],
                        "source_height": source["page_height_points"],
                    },
                },
            },
        },
        "publication_boundary": {
            "record_storage": "tracked",
            "source_content_visibility": "local_only",
            "source_text_in_record": False,
            "public_payload_expected": False,
        },
        "selector_method": {
            "maker_type": "mixed",
            "method": "frozen-sample-whole-page-region-binding",
            "version": "1",
            "configuration_ref": PLAN_REF.as_posix(),
            "configuration_digest": plan_digest,
        },
        "resolution_status": "mechanically_resolved",
        "review_status": "unreviewed",
        "review_ref": None,
        "provenance_event_ref": EVENT_ID,
    }


def _source_layer(
    *,
    plan: dict[str, Any],
    plan_digest: str,
    anchor_digest: str,
    observation_values: dict[str, Any],
) -> dict[str, Any]:
    source = plan["witness_2007"]
    ids = plan["opaque_ids"]
    ambiguity = observation_values["source_damage_or_ambiguity"]
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-layer.schema.json",
        "schema_version": "tos_source_text_layer_v1",
        "layer_id": ids["source_text_layer_id"],
        "layer_role": "diplomatic_transcription",
        "source_binding": {
            "work_ref": source["work_ref"],
            "expression_ref": source["expression_ref"],
            "edition_ref": source["edition_ref"],
            "item_ref": source["item_ref"],
            "source_file_ref": source["file_ref"],
            "source_file_sha256": source["file_sha256"],
            "anchor_contract": "tos_source_anchor_v2",
            "anchors": [
                {
                    "anchor_id": ids["source_anchor_id"],
                    "anchor_record_ref": plan["outputs"]["source_anchor_ref"],
                    "anchor_record_sha256": anchor_digest,
                }
            ],
        },
        "representation": {
            "content_file_id": f"tos.file.sha256.{source['text_layer_sha256']}",
            "content_ref": plan["outputs"]["private_text_ref"],
            "content_sha256": source["text_layer_sha256"],
            "media_type": "text/plain",
            "charset": "UTF-8",
            "language": source["language"],
            "text_scope": {
                "start": 0,
                "end": source["text_layer_codepoints"],
                "position_unit": "unicode_code_point",
                "interval": "half_open",
            },
            "character_normalization": "none",
            "line_break_posture": "logical_reflow",
            "storage": "ignored_local",
            "content_visibility": "local_only",
            "tracked_content": False,
            "publication_authorized": False,
            "rights_record_refs": [
                {"ref": source["rights_ref"], "sha256": source["rights_sha256"]}
            ],
            "publication_authority_refs": [],
        },
        "derivation": {
            "method": "manual_transcription",
            "input_layers": [],
            "maker": {
                "maker_type": "human",
                "agent_ref": "human:dionysus",
                "method": "blind-source-visible-workbench-transcription-observation",
                "version": "1",
                "configuration_ref": PLAN_REF.as_posix(),
                "configuration_digest": plan_digest,
            },
            "preservation_goal": "source_near",
            "loss_posture": "preservation_intended",
            "silent_changes_allowed": False,
            "change_payload": {"kind": "none"},
        },
        "editorial_policy": {
            "policy_ref": PLAN_REF.as_posix(),
            "policy_sha256": plan_digest,
            "transcription_goal": "diplomatic",
            "historical_language_preserved": True,
            "printing_errors_silently_corrected": False,
            "typography_posture": "preserve",
            "layout_posture": "logical_reflow",
            "unicode_normalization": "none",
            "uncertainty_representation": "explicit-never-silent",
            "method_declared": True,
        },
        "uncertainty": {
            "status": "unresolved",
            "annotations": [
                {
                    "annotation_id": (
                        "tos.annotation.antonovsky-2007-p011-human-ambiguity."
                        "sid-8846af42fb4d0b3b066b42e209f99e03"
                    ),
                    "anchor_ref": ids["source_anchor_id"],
                    "kind": "ambiguous_reading",
                    "alternatives": [
                        {
                            "value_sha256": _sha256_bytes(ambiguity.encode("utf-8")),
                            "value_in_record": False,
                            "value": None,
                            "status": "possible",
                        }
                    ],
                    "resolution": "unresolved",
                }
            ],
        },
        "admission": {
            "mechanical_status": "fixity_verified",
            "review_status": "unreviewed",
            "review_ref": None,
            "human_review_performed": False,
            "human_language_competence": "not_assessed",
            "language_competence_evidence_refs": [],
            "accepted_uses": [],
            "automatic_validation_complete": True,
            "model_output_is_ground_truth": False,
            "validator_proves_content_truth": False,
            "routine_human_task_created": False,
            "promotion_authorized": False,
        },
        "provenance_event_ref": EVENT_ID,
        "authority_boundary": (
            "a source text layer is one immutable, source-returnable representation "
            "with explicit derivation, uncertainty, review, competence, rights, and "
            "use scope; mechanical validation, model output, normalization, or "
            "agreement with another layer does not make it accepted source text, "
            "translation evidence, linguistic truth, semantic evidence, graph truth, "
            "canon authority, or publication permission"
        ),
        "layer_version": 1,
        "supersedes_layer_ref": None,
    }


def _unit_method(plan: dict[str, Any], plan_digest: str) -> dict[str, Any]:
    return {
        "maker_kind": "software",
        "agent_ref": SOFTWARE_AGENT,
        "method_name": (
            "first-non-whitespace-after-first-full-stop-through-second-full-stop"
        ),
        "method_version": "1",
        "software_refs": [BUILDER_REF.as_posix()],
        "model_ref": None,
        "configuration_ref": PLAN_REF.as_posix(),
        "locale": "ru",
        "unicode_version": unicodedata.unidata_version,
        "unicode_revision": None,
        "tailoring_ref": None,
        "provenance_event_ref": EVENT_ID,
        "made_at": plan["created_at"],
        "output_posture": "method_result_not_source_or_linguistic_truth",
    }


def _source_unit_packet(
    *,
    plan: dict[str, Any],
    plan_digest: str,
    text: str,
    heading: str,
    interstitial: str,
    sentence: str,
    remainder: str,
) -> dict[str, Any]:
    source = plan["witness_2007"]
    ids = plan["opaque_ids"]
    method = _unit_method(plan, plan_digest)
    private_ref = plan["outputs"]["private_text_ref"]

    def anchor(
        ref: str, ordinal: int, role: str, start: int, end: int, value: str
    ) -> dict[str, Any]:
        return {
            "anchor_ref": ref,
            "ordinal": ordinal,
            "text_layer_ref": private_ref,
            "text_layer_sha256": source["text_layer_sha256"],
            "selector": {
                "type": "text_position",
                "start": start,
                "end": end,
                "position_unit": "unicode_code_point",
                "interval": "half_open",
            },
            "exact_sha256": _sha256_bytes(value.encode("utf-8")),
            "anchor_role": role,
            "source_return": {"required": True, "locator_ref": private_ref},
        }

    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json",
        "schema_version": "tos_source_text_unit_packet_v1",
        "packet_id": ids["source_unit_packet_id"],
        "packet_version": 1,
        "supersedes_packet_ref": None,
        "content_posture": "source_bound",
        "source_scope": {
            key: source[key]
            for key in (
                "work_ref",
                "expression_ref",
                "edition_ref",
                "item_ref",
                "file_ref",
                "file_sha256",
            )
        },
        "source_layer": {
            "text_layer_ref": private_ref,
            "text_layer_sha256": source["text_layer_sha256"],
            "language": source["language"],
            "media_type": "text/plain; charset=utf-8",
            "unicode_form": "source_preserved",
            "position_unit": "unicode_code_point",
            "interval": "half_open",
            "immutable": True,
            "visibility": "local_only",
            "publication_authorized": False,
        },
        "schemes": [
            {
                "scheme_id": ids["source_unit_scheme_id"],
                "scheme_version": 1,
                "supersedes_scheme_ref": None,
                "identity_policy": (
                    "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis"
                ),
                "scheme_name": "ru first-prose-sentence boundary proposal",
                "analysis_role": "orthographic",
                "boundary_basis": "rule_based",
                "unit_kinds": ["sentence"],
                "method": method,
                "policies": {
                    "normalization": "no-text-mutation-separate-successor-layer",
                    "punctuation": "included_in_neighbor",
                    "whitespace": "declared_excluded",
                    "line_break": "declared_excluded",
                    "hyphenation": "preserve_source",
                    "unreported_gaps_allowed": False,
                    "overlap": "forbid",
                },
                "authority_limit": {
                    "algorithmic_output_is_source_truth": False,
                    "algorithmic_output_is_linguistic_truth": False,
                    "model_subword_is_lexeme": False,
                    "unit_identity_is_semantic_identity": False,
                    "text_mutation_allowed": False,
                },
            }
        ],
        "anchors": [
            anchor(ids["source_scope_anchor_id"], 1, "scope", 0, len(text), text),
            anchor(
                ids["source_heading_anchor_id"],
                2,
                "gap",
                source["heading_start"],
                source["heading_end"],
                heading,
            ),
            anchor(
                ids["source_interstitial_anchor_id"],
                3,
                "whitespace",
                source["interstitial_start"],
                source["interstitial_end"],
                interstitial,
            ),
            anchor(
                ids["source_sentence_anchor_id"],
                4,
                "content",
                source["sentence_start"],
                source["sentence_end"],
                sentence,
            ),
            anchor(
                ids["source_remainder_anchor_id"],
                5,
                "gap",
                source["remainder_start"],
                source["remainder_end"],
                remainder,
            ),
        ],
        "units": [
            {
                "unit_id": ids["source_sentence_unit_id"],
                "unit_version": 1,
                "supersedes_unit_ref": None,
                "identity_policy": (
                    "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis"
                ),
                "unit_kind": "sentence",
                "surface_posture": "source_bearing",
                "continuity": "contiguous",
                "ordered_anchor_refs": [ids["source_sentence_anchor_id"]],
                "parent_unit_refs": [],
                "ordered_child_unit_refs": [],
                "boundary_posture": "method_proposed",
                "certainty": {
                    "value": 0.5,
                    "meaning": "maker-declared-boundary-confidence-not-truth-probability",
                },
                "status_reason": (
                    "A deterministic punctuation-and-whitespace rule selects the first "
                    "prose span after the page heading and section marker. The boundary "
                    "and underlying unattested observation remain unreviewed."
                ),
                "source_text_mutated": False,
                "semantic_promotion": False,
            }
        ],
        "segmentations": [
            {
                "segmentation_id": ids["source_segmentation_id"],
                "segmentation_version": 1,
                "supersedes_segmentation_ref": None,
                "identity_policy": (
                    "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries"
                ),
                "scheme_ref": ids["source_unit_scheme_id"],
                "ordered_unit_refs": [ids["source_sentence_unit_id"]],
                "coverage": {
                    "scope_anchor_ref": ids["source_scope_anchor_id"],
                    "coverage_posture": "declared_partial",
                    "excluded_anchor_refs": [
                        ids["source_heading_anchor_id"],
                        ids["source_interstitial_anchor_id"],
                        ids["source_remainder_anchor_id"],
                    ],
                    "unreported_gaps_allowed": False,
                    "overlap_requires_declaration": True,
                    "source_reconstruction_required": True,
                },
                "status": "proposed",
                "status_reason": (
                    "Only one first-prose-sentence candidate is selected; heading, "
                    "interstitial whitespace, and exact remainder are digest-bound as "
                    "excluded coverage. No completion attestation or linguistic review exists."
                ),
                "maker": method,
                "competing_segmentation_refs": [],
                "review_refs": [],
                "declared_uses": ["source_observation"],
                "source_text_authority": False,
                "linguistic_authority": False,
                "semantic_authority": False,
            }
        ],
        "reviews": [],
        "projections": [],
        "rights_and_visibility": {
            "source_visibility": "local_only",
            "packet_visibility": "public_metadata_only",
            "effective_visibility": "local_only",
            "rights_record_refs": [source["rights_ref"]],
            "private_source_used": True,
            "publication_authorized": False,
            "inheritance_policy": "most-restrictive-source-packet-and-destination-wins",
        },
        "authority_boundary": {
            "tree_role": "orientation",
            "graph_role": "relation",
            "source_role": "authority",
            "validators_prove_mechanics_not_truth": True,
            "segmentation_is_method_result_not_source_truth": True,
            "source_unit_is_not_lexeme_sign_or_concept": True,
            "model_token_is_not_linguistic_token": True,
            "projection_is_owner_truth": False,
            "legacy_bulk_migration_authorized": False,
        },
    }


def _collation_packet(
    *,
    plan: dict[str, Any],
    plan_digest: str,
    source_unit_digest: str,
    private_detail_digest: str,
    public_views: list[dict[str, Any]],
) -> dict[str, Any]:
    source = plan["witness_2007"]
    target = plan["witness_1911"]
    ids = plan["opaque_ids"]
    source_witness_id = ids["witness_2007_id"]
    target_witness_id = ids["witness_1911_id"]
    source_private_ref = plan["outputs"]["private_text_ref"]
    source_unit_ref = plan["outputs"]["source_text_unit_packet_ref"]
    detail_ref = plan["outputs"]["private_detail_ref"]

    def witness(
        *,
        witness_id: str,
        ordinal: int,
        row: dict[str, Any],
        text_layer_ref: str,
        text_layer_sha256: str,
        unit_ref: str,
        unit_sha256: str,
        anchor_ref: str,
        start: int,
        end: int,
        exact_sha256: str,
        source_return_ref: str,
    ) -> dict[str, Any]:
        return {
            "witness_id": witness_id,
            "ordinal": ordinal,
            "work_ref": row["work_ref"],
            "expression_ref": row["expression_ref"],
            "edition_ref": row["edition_ref"],
            "item_ref": row["item_ref"],
            "file_ref": row["file_ref"],
            "file_sha256": row["file_sha256"],
            "text_layer_ref": text_layer_ref,
            "text_layer_sha256": text_layer_sha256,
            "text_unit_packet": {"ref": unit_ref, "sha256": unit_sha256},
            "language": row["language"],
            "anchor_ref": anchor_ref,
            "selector": {
                "type": "text_position",
                "start": start,
                "end": end,
                "position_unit": "unicode_code_point",
                "interval": "half_open",
            },
            "exact_sha256": exact_sha256,
            "source_return_ref": source_return_ref,
            "source_text_in_record": False,
            "rights_refs": [row["rights_ref"]],
            "visibility": "local_only",
            "publication_authorized": False,
        }

    artifact = plan["human_observation"]
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/witness-text-collation-packet-v1.schema.json",
        "schema_version": "tos_witness_text_collation_packet_v1",
        "packet_id": ids["collation_packet_id"],
        "packet_version": 1,
        "supersedes_packet_ref": None,
        "content_posture": "source_bound",
        "granularity": "sentence",
        "witnesses": [
            witness(
                witness_id=source_witness_id,
                ordinal=1,
                row=source,
                text_layer_ref=source_private_ref,
                text_layer_sha256=source["text_layer_sha256"],
                unit_ref=source_unit_ref,
                unit_sha256=source_unit_digest,
                anchor_ref=ids["source_sentence_anchor_id"],
                start=source["sentence_start"],
                end=source["sentence_end"],
                exact_sha256=source["sentence_sha256"],
                source_return_ref=source_private_ref,
            ),
            witness(
                witness_id=target_witness_id,
                ordinal=2,
                row=target,
                text_layer_ref=target["text_layer_ref"],
                text_layer_sha256=target["text_layer_sha256"],
                unit_ref=target["text_unit_packet_ref"],
                unit_sha256=target["text_unit_packet_sha256"],
                anchor_ref=target["anchor_ref"],
                start=target["sentence_start"],
                end=target["sentence_end"],
                exact_sha256=target["sentence_sha256"],
                source_return_ref=target["text_layer_ref"],
            ),
        ],
        "collations": [
            {
                "collation_id": ids["collation_id"],
                "collation_version": 1,
                "supersedes_collation_ref": None,
                "identity_policy": (
                    "opaque-id-independent-of-text-label-offset-score-algorithm-and-current-correspondence"
                ),
                "claim_id": ids["collation_claim_id"],
                "claim_version": 1,
                "supersedes_claim_ref": None,
                "direction": "symmetric_comparison",
                "ordered_witness_refs": [source_witness_id, target_witness_id],
                "relation_scope": "same_work",
                "correspondence_shape": "one_to_one",
                "correspondence_basis": "ordinal_and_surface_similarity",
                "status": "proposed",
                "status_reason": (
                    "The first prose sentence after the shared opening section marker "
                    "has high character-level surface similarity in four declared views. "
                    "The 2007 observation is unattested, both boundaries are unreviewed, "
                    "and no textual equivalence or edition derivation is established."
                ),
                "comparison_method": {
                    "algorithm": plan["comparison_method"]["name"],
                    "algorithm_version": plan["comparison_method"]["version"],
                    "tokenization": "unicode_code_point",
                    "autojunk": False,
                    "source_text_mutated": False,
                    "base_or_lemma_selected": False,
                },
                "comparison_views": public_views,
                "private_detail": {
                    "ref": detail_ref,
                    "sha256": private_detail_digest,
                    "tracked": False,
                    "visibility": "local_only",
                    "reconstructive_detail": True,
                },
                "maker": {
                    "maker_kind": "software",
                    "agent_ref": SOFTWARE_AGENT,
                    "made_at": plan["created_at"],
                    "method": plan["comparison_method"]["name"],
                    "method_version": plan["comparison_method"]["version"],
                    "software_ref": BUILDER_REF.as_posix(),
                    "configuration_sha256": plan_digest,
                    "provenance_event_ref": EVENT_ID,
                    "output_posture": "proposal_not_textual_truth",
                },
                "evidence": [
                    {
                        "evidence_ref": PLAN_REF.as_posix(),
                        "sha256": plan_digest,
                        "role": "method_input",
                        "witness_refs": [source_witness_id, target_witness_id],
                        "authority_effect": "supports_proposal",
                    },
                    {
                        "evidence_ref": _artifact_ref(
                            artifact["autosave_relative_path"]
                        ),
                        "sha256": artifact["autosave_sha256"],
                        "role": "source_observation",
                        "witness_refs": [source_witness_id],
                        "authority_effect": "none",
                    },
                    {
                        "evidence_ref": _artifact_ref(
                            artifact["closure_relative_path"]
                        ),
                        "sha256": artifact["closure_sha256"],
                        "role": "qualification",
                        "witness_refs": [source_witness_id],
                        "authority_effect": "none",
                    },
                    {
                        "evidence_ref": target["text_unit_packet_ref"],
                        "sha256": target["text_unit_packet_sha256"],
                        "role": "support",
                        "witness_refs": [target_witness_id],
                        "authority_effect": "supports_proposal",
                    },
                    {
                        "evidence_ref": source["rights_ref"],
                        "sha256": source["rights_sha256"],
                        "role": "rights",
                        "witness_refs": [source_witness_id],
                        "authority_effect": "none",
                    },
                    {
                        "evidence_ref": target["rights_ref"],
                        "sha256": target["rights_sha256"],
                        "role": "rights",
                        "witness_refs": [target_witness_id],
                        "authority_effect": "none",
                    },
                ],
                "competing_collation_refs": [],
                "review_refs": [],
                "interpretive_boundary": {
                    "preferred_reading_selected": False,
                    "textual_equivalence_established": False,
                    "expression_derivation_established": False,
                    "translation_relation_inferred": False,
                    "lexical_equivalence_inferred": False,
                    "semantic_relation_inferred": False,
                },
            }
        ],
        "reviews": [],
        "projections": [],
        "rights_and_visibility": {
            "witness_visibility": "local_only",
            "packet_visibility": "public_metadata_only",
            "effective_visibility": "local_only",
            "rights_record_refs": [source["rights_ref"], target["rights_ref"]],
            "private_source_used": True,
            "publication_authorized": False,
            "inheritance_policy": (
                "most-restrictive-witness-packet-detail-and-destination-wins"
            ),
        },
        "authority_boundary": {
            "tree_role": "orientation",
            "graph_role": "relation",
            "source_role": "authority",
            "validators_prove_mechanics_not_truth": True,
            "collation_is_claim_not_textual_truth": True,
            "machine_or_model_may_propose_not_accept": True,
            "human_observation_is_not_human_review": True,
            "projection_is_not_owner_truth": True,
            "preferred_reading_not_inferred": True,
            "equivalence_not_inferred": True,
            "derivation_not_inferred": True,
            "translation_not_inferred": True,
            "semantics_not_inferred": True,
            "canon_effect": False,
        },
    }


def _entity(
    *,
    ref: str,
    role: str,
    digest: str,
    size: int,
    media_type: str,
    availability: str,
    disclosure: str,
    at: str,
) -> dict[str, Any]:
    return {
        "entity_ref": ref,
        "role": role,
        "sha256": digest,
        "size_bytes": size,
        "media_type": media_type,
        "availability": availability,
        "content_disclosure": disclosure,
        "fixity_verified": True,
        "fixity_verified_at": at,
    }


def _provenance_event(
    *,
    repo_root: Path,
    owner_root: Path,
    artifact_paths: dict[str, Path],
    source_paths: dict[str, Path],
    target_paths: dict[str, Path],
    plan: dict[str, Any],
    plan_digest: str,
    tracked_records: dict[str, bytes],
    private_outputs: dict[str, bytes],
) -> dict[str, Any]:
    created_at = plan["created_at"]
    builder_path = repo_root / BUILDER_REF
    builder_digest = _sha256(builder_path)
    executable = Path(sys.executable).resolve()
    argv = [
        "python",
        BUILDER_REF.as_posix(),
        "--build",
        "--owner-root",
        DEFAULT_OWNER_ROOT.as_posix(),
        "--artifact-root",
        DEFAULT_ARTIFACT_ROOT.as_posix(),
    ]
    observation = plan["human_observation"]
    source = plan["witness_2007"]
    target = plan["witness_1911"]

    inputs = [
        _entity(
            ref=_artifact_ref(observation["autosave_relative_path"]),
            role="unattested-human-workbench-observation-autosave",
            digest=observation["autosave_sha256"],
            size=artifact_paths["autosave"].stat().st_size,
            media_type="application/json",
            availability="owner_local",
            disclosure="private_content",
            at=created_at,
        ),
        _entity(
            ref=_artifact_ref(observation["closure_relative_path"]),
            role="no-gold-no-attestation-sparse-calibration-closure",
            digest=observation["closure_sha256"],
            size=artifact_paths["closure"].stat().st_size,
            media_type="application/json",
            availability="owner_local",
            disclosure="private_content",
            at=created_at,
        ),
        _entity(
            ref=_artifact_ref(observation["closure_receipt_relative_path"]),
            role="sparse-calibration-closure-receipt",
            digest=observation["closure_receipt_sha256"],
            size=artifact_paths["closure_receipt"].stat().st_size,
            media_type="application/json",
            availability="owner_local",
            disclosure="private_content",
            at=created_at,
        ),
        _entity(
            ref=source["source_relative_ref"],
            role="exact-2007-local-pdf-witness",
            digest=source["file_sha256"],
            size=source_paths["pdf"].stat().st_size,
            media_type="application/pdf",
            availability="ignored_local",
            disclosure="private_content",
            at=created_at,
        ),
        _entity(
            ref=target["text_layer_ref"],
            role="exact-1911-private-raw-text-layer",
            digest=target["text_layer_sha256"],
            size=target_paths["text"].stat().st_size,
            media_type="text/plain; charset=utf-8",
            availability="ignored_local",
            disclosure="private_content",
            at=created_at,
        ),
    ]
    tracked_inputs = [
        (PLAN_REF.as_posix(), plan_digest, "tracked-text-free-collation-plan"),
        (
            plan["research_ref"],
            _sha256(repo_root / plan["research_ref"]),
            "ordered-witness-text-collation-research",
        ),
        (
            plan["contract_ref"],
            _sha256(repo_root / plan["contract_ref"]),
            "witness-text-collation-contract",
        ),
        (
            observation["assurance_ref"],
            observation["assurance_sha256"],
            "solo-human-plus-ai-assurance-boundary",
        ),
        (
            observation["method_research_ref"],
            observation["method_research_sha256"],
            "human-assurance-method-research",
        ),
        (
            source["item_manifest_ref"],
            _sha256(source_paths["manifest"]),
            "exact-2007-item-manifest",
        ),
        (
            source["resource_inventory_ref"],
            _sha256(source_paths["inventory"]),
            "exact-2007-page-resource-inventory",
        ),
        (source["rights_ref"], source["rights_sha256"], "2007-layered-rights"),
        (
            target["text_layer_record_ref"],
            target["text_layer_record_sha256"],
            "1911-private-layer-record",
        ),
        (
            target["text_unit_packet_ref"],
            target["text_unit_packet_sha256"],
            "1911-sentence-unit-proposal",
        ),
        (target["rights_ref"], target["rights_sha256"], "1911-layered-rights"),
    ]
    for ref, digest, role in tracked_inputs:
        path = _checked_path(repo_root, ref, label=role)
        _verify_digest(path, digest, label=role)
        inputs.append(
            _entity(
                ref=ref,
                role=role,
                digest=digest,
                size=path.stat().st_size,
                media_type="application/json"
                if path.suffix in {".json", ".jsonl"}
                else "text/markdown; charset=utf-8",
                availability="tracked",
                disclosure="public_metadata_only",
                at=created_at,
            )
        )

    outputs = [
        _entity(
            ref=ref,
            role="tracked-text-free-anchor-layer-unit-or-collation-record",
            digest=_sha256_bytes(payload),
            size=len(payload),
            media_type="application/json",
            availability="tracked",
            disclosure="public_metadata_only",
            at=created_at,
        )
        for ref, payload in tracked_records.items()
    ]
    byproducts = [
        _entity(
            ref=ref,
            role=(
                "ignored-local-human-observation-text-layer"
                if ref == plan["outputs"]["private_text_ref"]
                else "ignored-local-reconstructive-collation-detail"
            ),
            digest=_sha256_bytes(payload),
            size=len(payload),
            media_type=(
                "text/plain; charset=utf-8"
                if ref == plan["outputs"]["private_text_ref"]
                else "application/json"
            ),
            availability="ignored_local",
            disclosure="private_content",
            at=created_at,
        )
        for ref, payload in private_outputs.items()
    ]
    collation_ref = plan["outputs"]["collation_packet_ref"]
    derivations = [
        {
            "derivation_id": "tos.derivation.antonovsky-2007-source-anchor.1",
            "input_entity_ref": source["source_relative_ref"],
            "output_entity_ref": plan["outputs"]["source_anchor_ref"],
            "relation": "selection_from",
            "influence_asserted": True,
            "description": "The tracked text-free anchor identifies the exact page region of the fixed local 2007 PDF witness.",
        },
        {
            "derivation_id": "tos.derivation.antonovsky-2007-source-layer.1",
            "input_entity_ref": _artifact_ref(observation["autosave_relative_path"]),
            "output_entity_ref": plan["outputs"]["source_text_layer_ref"],
            "relation": "aggregation_from",
            "influence_asserted": True,
            "description": "The tracked text-free layer record binds the private observation bytes, exact PDF page, uncertainty, rights, and no-review posture.",
        },
        {
            "derivation_id": "tos.derivation.antonovsky-2007-sentence-unit.1",
            "input_entity_ref": _artifact_ref(observation["autosave_relative_path"]),
            "output_entity_ref": plan["outputs"]["source_text_unit_packet_ref"],
            "relation": "selection_from",
            "influence_asserted": True,
            "description": "The text-free unit packet selects one exact private span and digest without accepting its boundary or content.",
        },
        {
            "derivation_id": "tos.derivation.antonovsky-2007-1911-collation.1",
            "input_entity_ref": _artifact_ref(observation["autosave_relative_path"]),
            "output_entity_ref": collation_ref,
            "relation": "aggregation_from",
            "influence_asserted": True,
            "description": "The 2007 private sentence contributes only its source-bound selector, digest, and comparison metrics to the proposal.",
        },
        {
            "derivation_id": "tos.derivation.antonovsky-2007-1911-collation.2",
            "input_entity_ref": target["text_layer_ref"],
            "output_entity_ref": collation_ref,
            "relation": "aggregation_from",
            "influence_asserted": True,
            "description": "The existing 1911 private sentence proposal contributes only its source-bound selector, digest, and comparison metrics.",
        },
    ]
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json",
        "schema_version": "tos_provenance_event_v2",
        "event_id": EVENT_ID,
        "event_version": 1,
        "supersedes_event_ref": None,
        "record_binding": {
            "manifest_ref": PLAN_REF.as_posix(),
            "digest_algorithm": "sha256",
            "digest_scope": "exact_event_record_bytes",
        },
        "activity": {
            "event_type": "alignment",
            "started_at": created_at,
            "ended_at": created_at,
            "status": "completed_with_warnings",
            "terminal_reason": None,
            "exit_code": 0,
            "warnings": [
                "The 2007 Workbench observation has no completion attestation and creates no gold or human review.",
                "Both sentence boundaries and the cross-edition correspondence remain proposed.",
                "Surface similarity establishes no source fidelity, preferred reading, textual equivalence, Expression derivation, translation judgment, or semantics.",
                "The exact 2007 PDF and all source-bearing or reconstructive derivatives remain local-only and publication-unauthorized.",
                "The unsigned self-recorded event proves mechanics and closure, not execution or content truth.",
            ],
        },
        "entities": {"inputs": inputs, "outputs": outputs, "byproducts": byproducts},
        "derivations": derivations,
        "responsibility": [
            {
                "agent_ref": "human:dionysus",
                "agent_kind": "human",
                "role": "operator",
                "responsibility_posture": "performed",
                "evidence_binding": None,
                "human_evidence_status": "unattested",
            },
            {
                "agent_ref": SOFTWARE_AGENT,
                "agent_kind": "software",
                "role": "executor",
                "responsibility_posture": "performed",
                "evidence_binding": {
                    "ref": BUILDER_REF.as_posix(),
                    "sha256": builder_digest,
                },
                "human_evidence_status": "not_applicable",
            },
            {
                "agent_ref": "model:openai-codex",
                "agent_kind": "model",
                "role": "observer",
                "responsibility_posture": "observed",
                "evidence_binding": {
                    "ref": PLAN_REF.as_posix(),
                    "sha256": plan_digest,
                },
                "human_evidence_status": "not_applicable",
            },
        ],
        "method": {
            "procedure": {
                "name": plan["comparison_method"]["name"],
                "version": plan["comparison_method"]["version"],
                "purpose": plan["question"],
            },
            "command_capture": {
                "disclosure": "inline",
                "argv": argv,
                "argv_sha256": _canonical_value_sha256(argv),
                "withholding_reason": None,
            },
            "configuration_binding": {
                "ref": PLAN_REF.as_posix(),
                "sha256": plan_digest,
            },
            "software_components": [
                {
                    "name": "Tree of Sophia Antonovsky witness collation builder",
                    "version": plan["comparison_method"]["version"],
                    "role": "observation-materialization-and-collation-proposal-builder",
                    "artifact_ref": BUILDER_REF.as_posix(),
                    "artifact_sha256": builder_digest,
                    "verification_status": "verified",
                },
                {
                    "name": platform.python_implementation(),
                    "version": platform.python_version(),
                    "role": "language-runtime",
                    "artifact_ref": "runtime:cpython-executable",
                    "artifact_sha256": _sha256(executable),
                    "verification_status": "verified",
                },
            ],
            "model_invocations": [],
            "environment": {
                "runtime": platform.python_implementation(),
                "runtime_version": platform.python_version(),
                "runtime_artifact_sha256": _sha256(executable),
                "backend": "python-standard-library-difflib-sequence-matcher",
                "hardware_target": "cpu",
                "unicode_version": unicodedata.unidata_version,
                "environment_profile_binding": {
                    "ref": PLAN_REF.as_posix(),
                    "sha256": plan_digest,
                },
            },
        },
        "manual_changes": {
            "status": "none_declared",
            "change_receipts": [],
            "statement": "The builder copies the preserved human observation exactly and applies no manual correction; that prior unattested observation is an input, not a manual edit to this run.",
        },
        "measurements": [
            {
                "metric": "input_bytes",
                "status": "measured",
                "value": sum(row["size_bytes"] for row in inputs),
                "unit": "bytes",
                "method": "sum of exact fixity-bound input entity byte counts",
                "evidence_binding": None,
            },
            {
                "metric": "output_bytes",
                "status": "measured",
                "value": sum(row["size_bytes"] for row in outputs + byproducts),
                "unit": "bytes",
                "method": "sum of exact serialized tracked and ignored-local output byte counts",
                "evidence_binding": None,
            },
            {
                "metric": "human_active_seconds",
                "status": "measured",
                "value": observation["expected_active_seconds"],
                "unit": "seconds",
                "method": "exact Workbench active_seconds preserved by autosave and sparse-calibration closure",
                "evidence_binding": None,
            },
        ],
        "evidence_authentication": {
            "capture_posture": "tool_captured",
            "signature_status": "unsigned",
            "signature_bindings": [],
            "verification_status": "mechanically_verified",
            "producer_control_boundary": "The autosave, closure, private sources, and builder are operator-local. Exact digests support replay but do not authenticate the human observation as a completed review or prove textual truth.",
        },
        "rights_and_visibility": {
            "rights_record_bindings": [
                {"ref": source["rights_ref"], "sha256": source["rights_sha256"]},
                {"ref": target["rights_ref"], "sha256": target["rights_sha256"]},
            ],
            "intended_uses": ["local_research", "indexing"],
            "content_visibility": "local_only",
            "publication_authorized": False,
            "publication_authority_bindings": [],
        },
        "review_and_authority": {
            "mechanical_validation": "passed",
            "human_review_status": "not_performed",
            "review_bindings": [],
            "accepted_uses": [],
            "promotion_authorized": False,
            "competence_evidence_bindings": [],
        },
        "reproducibility": {
            "classification": "replay_ready",
            "known_gaps": [
                "The human observation has no completion attestation or pass receipt.",
                "The 2007 source layer and both sentence boundaries are unreviewed.",
                "The comparison algorithm is character-level screening, not philological adjudication.",
                "The private source and reconstructive detail are intentionally absent from Git.",
                "The receipt is unsigned and self-recorded by the local builder.",
            ],
            "replay_scope": "Exact artifact, PDF, layer, unit, rights, selector, digest, comparison-view, private-detail, runtime, and non-promotion closure.",
        },
        "authority_boundary": {
            "validator_role": "mechanics_and_closure_only_not_truth",
            "claims_not_established": [
                "execution_truth",
                "content_truth",
                "source_fidelity",
                "translation_quality",
                "semantic_correctness",
                "rights_clearance",
                "human_review",
                "publication_authority",
                "canon_authority",
            ],
        },
    }


def _validate_schema(repo_root: Path, schema_ref: Path, payload: dict[str, Any]) -> None:
    schema = _load_json(repo_root / schema_ref)
    errors = sorted(
        Draft202012Validator(schema, format_checker=FormatChecker()).iter_errors(
            payload
        ),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        details = "; ".join(
            f"{'/'.join(str(part) for part in error.absolute_path) or '<root>'}: {error.message}"
            for error in errors[:16]
        )
        raise AntonovskyCollationError(
            f"{schema_ref.as_posix()} validation failed: {details}"
        )


def _collation_semantic_issues(packet: dict[str, Any]) -> list[str]:
    messages: list[str] = []
    witnesses = [row for row in packet.get("witnesses", []) if isinstance(row, dict)]
    collations = [row for row in packet.get("collations", []) if isinstance(row, dict)]
    witness_ids = [row.get("witness_id") for row in witnesses]
    if len(witness_ids) != len(set(witness_ids)):
        messages.append("duplicate witness identity")
    if len({row.get("ordinal") for row in witnesses}) != len(witnesses):
        messages.append("duplicate witness ordinal")
    if len({row.get("expression_ref") for row in witnesses}) != len(witnesses):
        messages.append("collation witnesses must retain distinct Expression identities")
    known = set(witness_ids)
    decided = {"accepted", "accepted_with_limits", "rejected", "superseded"}
    for row in collations:
        refs = row.get("ordered_witness_refs", [])
        if any(ref not in known for ref in refs):
            messages.append("collation references an unknown witness")
        if row.get("status") in decided and not row.get("review_refs"):
            messages.append("decided collation lacks human review")
        if row.get("status") in {"accepted", "accepted_with_limits"} and row.get(
            "maker", {}
        ).get("maker_kind") != "human":
            messages.append("non-human collation acceptance")
        if row.get("collation_id") == row.get("supersedes_collation_ref"):
            messages.append("collation cannot supersede itself")
        if row.get("claim_id") == row.get("supersedes_claim_ref"):
            messages.append("collation claim cannot supersede itself")
        if any(
            value is not False for value in row.get("interpretive_boundary", {}).values()
        ):
            messages.append("collation interpretive boundary widened")
    accepted = {
        row.get("collation_id")
        for row in collations
        if row.get("status") in {"accepted", "accepted_with_limits"}
    }
    for projection in packet.get("projections", []):
        if not isinstance(projection, dict):
            continue
        if any(ref not in accepted for ref in projection.get("source_collation_refs", [])):
            messages.append("projection uses a collation that is not accepted")
    rights = packet.get("rights_and_visibility", {})
    if rights.get("private_source_used") and rights.get("publication_authorized"):
        messages.append("private-source collation authorizes publication")
    return messages


def _semantic_checks(
    *,
    repo_root: Path,
    layer: dict[str, Any],
    unit: dict[str, Any],
    collation: dict[str, Any],
    event: dict[str, Any],
    source_text: str,
) -> None:
    sys.path.insert(0, str(repo_root / "scripts"))
    import validate_source_witness_foundation as foundation  # noqa: PLC0415

    checks = [
        ("source layer", foundation._source_text_layer_semantic_issues(layer)),
        ("source units", foundation._source_text_unit_v1_issues(unit, text=source_text)),
        ("collation", _collation_semantic_issues(collation)),
        ("provenance", foundation._provenance_v2_semantic_issues(event)),
    ]
    failures = [f"{label}: {row}" for label, rows in checks for row in rows]
    if failures:
        raise AntonovskyCollationError("; ".join(failures[:32]))


def _expected(
    *, repo_root: Path, owner_root: Path, artifact_root: Path
) -> tuple[dict[str, bytes], dict[str, bytes], dict[str, Any]]:
    plan_path = repo_root / PLAN_REF
    plan = _load_json(plan_path)
    plan_digest = _sha256(plan_path)
    if plan.get("schema_version") != (
        "tos_antonovsky_2007_1911_opening_sentence_collation_plan_v1"
    ):
        raise AntonovskyCollationError("unexpected collation plan version")
    expected_boundary = {
        "human_observation_preserved": True,
        "human_attestation_collected": False,
        "human_review_performed": False,
        "gold_created": False,
        "source_text_accepted": False,
        "sentence_boundary_accepted": False,
        "collation_status": "proposed",
        "preferred_reading_selected": False,
        "textual_equivalence_established": False,
        "expression_derivation_established": False,
        "translation_relation_created": False,
        "lexical_or_semantic_relation_created": False,
        "projection_created": False,
        "graph_or_canon_effect": False,
        "routine_human_task_created": False,
        "publication_authorized": False,
    }
    if plan.get("authority_boundary") != expected_boundary:
        raise AntonovskyCollationError("plan authority boundary widened")
    for ref in (
        plan["research_ref"],
        plan["contract_ref"],
        plan["human_observation"]["assurance_ref"],
        plan["human_observation"]["method_research_ref"],
    ):
        _checked_path(repo_root, ref, label="tracked plan dependency")
    for ref in plan["outputs"].values():
        if not isinstance(ref, str) or ref.startswith("/") or ".." in Path(ref).parts:
            raise AntonovskyCollationError(f"unsafe output ref: {ref!r}")
    for ref in (
        plan["outputs"]["private_text_ref"],
        plan["outputs"]["private_detail_ref"],
    ):
        if not _is_ignored(repo_root, ref):
            raise AntonovskyCollationError(f"private output is not Git-ignored: {ref}")

    source_paths = _verify_2007_source(
        repo_root=repo_root, owner_root=owner_root, plan=plan
    )
    source_text, observation_values, artifact_paths = _read_human_observation(
        artifact_root=artifact_root, plan=plan
    )
    heading, interstitial, source_sentence, remainder = _select_2007_sentence(
        source_text, plan
    )
    target_sentence, target_paths = _verify_1911_target(
        repo_root=repo_root, owner_root=owner_root, plan=plan
    )
    public_views, private_views = _comparison_views(
        source_sentence, target_sentence, plan
    )

    private_text_bytes = source_text.encode("utf-8")
    private_detail = {
        "schema_version": "tos_private_witness_text_collation_detail_v1",
        "visibility": "local_only",
        "publication_authorized": False,
        "sample_id": plan["human_observation"]["sample_id"],
        "human_observation": {
            "reviewer_ref": plan["human_observation"]["reviewer_ref"],
            "active_seconds": plan["human_observation"]["expected_active_seconds"],
            "attestation_status": "not_collected",
            "pass_receipt_collected": False,
            "promotion_authorized": False,
            "values": observation_values,
        },
        "witnesses": [
            {
                "ordinal": 1,
                "text_layer_sha256": plan["witness_2007"]["text_layer_sha256"],
                "selector": [
                    plan["witness_2007"]["sentence_start"],
                    plan["witness_2007"]["sentence_end"],
                ],
                "sentence_sha256": plan["witness_2007"]["sentence_sha256"],
            },
            {
                "ordinal": 2,
                "text_layer_sha256": plan["witness_1911"]["text_layer_sha256"],
                "selector": [
                    plan["witness_1911"]["sentence_start"],
                    plan["witness_1911"]["sentence_end"],
                ],
                "sentence_sha256": plan["witness_1911"]["sentence_sha256"],
            },
        ],
        "comparison_views": private_views,
        "authority_boundary": (
            "local reconstructive detail for an unattested observation and proposed "
            "collation; not gold, review, source acceptance, textual equivalence, "
            "Expression derivation, translation judgment, semantics, or publication"
        ),
    }
    private_detail_bytes = _json_bytes(private_detail)
    private_outputs = {
        plan["outputs"]["private_text_ref"]: private_text_bytes,
        plan["outputs"]["private_detail_ref"]: private_detail_bytes,
    }

    anchor = _source_anchor(plan, plan_digest)
    anchor_bytes = _json_bytes(anchor)
    layer = _source_layer(
        plan=plan,
        plan_digest=plan_digest,
        anchor_digest=_sha256_bytes(anchor_bytes),
        observation_values=observation_values,
    )
    layer_bytes = _json_bytes(layer)
    unit = _source_unit_packet(
        plan=plan,
        plan_digest=plan_digest,
        text=source_text,
        heading=heading,
        interstitial=interstitial,
        sentence=source_sentence,
        remainder=remainder,
    )
    unit_bytes = _json_bytes(unit)
    collation = _collation_packet(
        plan=plan,
        plan_digest=plan_digest,
        source_unit_digest=_sha256_bytes(unit_bytes),
        private_detail_digest=_sha256_bytes(private_detail_bytes),
        public_views=public_views,
    )
    collation_bytes = _json_bytes(collation)
    tracked_records = {
        plan["outputs"]["source_anchor_ref"]: anchor_bytes,
        plan["outputs"]["source_text_layer_ref"]: layer_bytes,
        plan["outputs"]["source_text_unit_packet_ref"]: unit_bytes,
        plan["outputs"]["collation_packet_ref"]: collation_bytes,
    }
    event = _provenance_event(
        repo_root=repo_root,
        owner_root=owner_root,
        artifact_paths=artifact_paths,
        source_paths=source_paths,
        target_paths=target_paths,
        plan=plan,
        plan_digest=plan_digest,
        tracked_records=tracked_records,
        private_outputs=private_outputs,
    )
    event_bytes = _json_bytes(event)
    tracked_records[plan["outputs"]["provenance_event_ref"]] = event_bytes

    for schema_ref, payload in (
        (ANCHOR_SCHEMA_REF, anchor),
        (LAYER_SCHEMA_REF, layer),
        (UNIT_SCHEMA_REF, unit),
        (COLLATION_SCHEMA_REF, collation),
        (PROVENANCE_SCHEMA_REF, event),
    ):
        _validate_schema(repo_root, schema_ref, payload)
    _semantic_checks(
        repo_root=repo_root,
        layer=layer,
        unit=unit,
        collation=collation,
        event=event,
        source_text=source_text,
    )

    tracked_blob = b"\n".join(tracked_records.values()) + plan_path.read_bytes()
    protected_values = [source_text, source_sentence, target_sentence]
    protected_values.extend(
        value
        for key, value in observation_values.items()
        if isinstance(value, str)
        and key
        in {
            "diplomatic_transcription",
            "layout_and_reading_order",
            "source_damage_or_ambiguity",
            "notes",
        }
        and len(value) >= 32
    )
    for value in protected_values:
        if value.encode("utf-8") in tracked_blob:
            raise AntonovskyCollationError(
                "tracked plan or output exposes private source or human-observation text"
            )
    return tracked_records, private_outputs, plan


def _write_private(path: Path, payload: bytes) -> None:
    if path.exists() and path.is_symlink():
        raise AntonovskyCollationError(f"private output path is symlinked: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    os.chmod(path, 0o600)


def run(
    *,
    repo_root: Path,
    owner_root: Path,
    artifact_root: Path,
    build: bool,
) -> dict[str, Any]:
    tracked_records, private_outputs, plan = _expected(
        repo_root=repo_root,
        owner_root=owner_root,
        artifact_root=artifact_root,
    )
    if build:
        for ref, payload in private_outputs.items():
            _write_private(owner_root / ref, payload)
        for ref, payload in tracked_records.items():
            path = repo_root / ref
            if path.exists() and path.is_symlink():
                raise AntonovskyCollationError(
                    f"tracked output path is symlinked: {ref}"
                )
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(payload)
    for ref, payload in private_outputs.items():
        path = owner_root / ref
        if path.is_symlink() or not path.is_file() or path.read_bytes() != payload:
            raise AntonovskyCollationError(
                f"private output is absent or stale: {ref}"
            )
        if path.stat().st_mode & 0o777 != 0o600:
            raise AntonovskyCollationError(f"private output mode is not 0600: {ref}")
    for ref, payload in tracked_records.items():
        path = repo_root / ref
        if path.is_symlink() or not path.is_file() or path.read_bytes() != payload:
            raise AntonovskyCollationError(
                f"tracked output is absent or stale: {ref}"
            )
    return {
        "status": "passed",
        "question": plan["question"],
        "human_observation_status": "unattested_preserved",
        "human_active_seconds": plan["human_observation"]["expected_active_seconds"],
        "source_sentence_selector": [
            plan["witness_2007"]["sentence_start"],
            plan["witness_2007"]["sentence_end"],
        ],
        "source_sentence_sha256": plan["witness_2007"]["sentence_sha256"],
        "target_sentence_selector": [
            plan["witness_1911"]["sentence_start"],
            plan["witness_1911"]["sentence_end"],
        ],
        "target_sentence_sha256": plan["witness_1911"]["sentence_sha256"],
        "comparison_view_count": len(plan["comparison_method"]["views"]),
        "tracked_record_count": len(tracked_records),
        "private_output_count": len(private_outputs),
        "collation_status": "proposed",
        "human_review_performed": False,
        "gold_created": False,
        "textual_equivalence_established": False,
        "expression_derivation_established": False,
        "translation_or_semantic_relation_created": False,
        "graph_or_canon_effect": False,
        "publication_authorized": False,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--owner-root", type=Path, default=DEFAULT_OWNER_ROOT)
    parser.add_argument("--artifact-root", type=Path, default=DEFAULT_ARTIFACT_ROOT)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--build", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)
    try:
        report = run(
            repo_root=args.repo_root.resolve(),
            owner_root=args.owner_root.resolve(),
            artifact_root=args.artifact_root.resolve(),
            build=args.build,
        )
    except AntonovskyCollationError as exc:
        print(f"Antonovsky 2007/1911 collation: FAIL: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
