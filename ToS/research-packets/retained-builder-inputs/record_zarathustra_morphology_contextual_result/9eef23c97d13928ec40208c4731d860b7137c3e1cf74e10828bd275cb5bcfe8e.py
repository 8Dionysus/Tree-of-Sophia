#!/usr/bin/env python3
"""Record a text-free ToS receipt from one private ZDL contextual run."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import stat
import sys
from collections import Counter
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
PACKET_ROOT = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
)
PLAN_REF = PACKET_ROOT + "morphology-contextual-episode.selected-form-b.v1.json"
CONTEXT_RECEIPT_REF = (
    PACKET_ROOT + "morphology-contextual-episode.selected-form-b.receipt.v1.json"
)
ADMISSION_REF = (
    PACKET_ROOT
    + "morphology-contextual-episode.selected-form-b.artifact-admission.v1.json"
)
RESULT_REF = (
    PACKET_ROOT + "morphology-contextual-episode.selected-form-b.result.v1.json"
)
PROVENANCE_REF = (
    PACKET_ROOT
    + "provenance.morphology-contextual-episode.selected-form-b.result.v1.jsonl"
)
SCHEMA_REF = "ToS/contracts/morphology-contextual-result-receipt.schema.json"
PROVENANCE_SCHEMA_REF = "ToS/contracts/provenance-event.schema.json"
GENERATOR_REF = "scripts/record_zarathustra_morphology_contextual_result.py"

ARTIFACT_OWNER_ROOT = Path(
    "/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab"
)
RUNTIME_MANIFEST_PATH = Path(
    "/srv/abyss-machine/runtimes/tree-of-sophia-foundation-lab/"
    "zdl-de-zdl-lg-4.0.0-7eabc170-py312/runtime-manifest.json"
)
REGISTRY_RECORD_PATH = Path(
    "/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/"
    "zdl-de-zdl-lg-4.0.0-7eabc170-compat-typer0241-click821-registry/"
    "records/d6356a650a3f033469759a380264cbe00c1401b5713f90b97df674e04363a80b.json"
)
EXPERIMENT_ID = "tos-historical-german-morphology-v1"
VARIANT = "B"
EXPECTED_RUN_ID = "zarathustra-zdl-b-20260812t1354z"
EXPECTED_PLAN_SHA256 = (
    "73a88433cda008de222f288027d4ac2fc1a97357a47a5a0e99d9e67f95a34785"
)
EXPECTED_CONTEXT_RECEIPT_SHA256 = (
    "0baff212b114ae9d0781ad0e26cf93a25066ad42f371252dc8106d89440d936f"
)
EXPECTED_PACKET_SHA256 = (
    "d82a2d81b370f3131fb56e0b1328866120cb3313903ffdf115debb4f0e34aa66"
)
EXPECTED_FORM_SHA256 = (
    "0007489cd4b0a84b926a341d3540ae1e8a2ff9cfc2062069dbf5a4e994f6ef37"
)
EXPECTED_SELECTION_RANKS = [1, 73, 145]
EXPECTED_SELECTION_ROLES = ["first", "inclusive-median", "last"]
EXPECTED_PART_ORDERS = [1, 3, 4]
EXPECTED_STACK_COMMIT = "a321401b792b24e9b7dd6cbae4e7085b8fafe0e2"
EXPECTED_RUNNER_SHA256 = (
    "e86c0cafe281e73832a9475b5a54e4a12d75f26b8e8389a73e4595b9d6ffb672"
)
EXPECTED_RUNTIME_MANIFEST_SHA256 = (
    "89c8a70b779c3782cfdf5b10bdd7963736c181afa86918a92d4d967aebdc820c"
)
EXPECTED_ARTIFACT_SET_SHA256 = (
    "591824890eefdbc9f2d79897640a47fdd5c128b87f7eb29e70203f5dda97f561"
)
EXPECTED_RECORD_ID = (
    "sha256:d6356a650a3f033469759a380264cbe00c1401b5713f90b97df674e04363a80b"
)
EXPECTED_SUBJECT_DIGEST = (
    "sha256:9768ec14c9f4e0115bf285b7e50f24521cecbd7533c20fb00e0bb7204aba9ff4"
)
EXPECTED_SUBJECTS_AGGREGATE = (
    "sha256:df79c75c0e521bc5f1b379d7b06f27d7d433b5b8bac048fa5ed0ff088d48eed2"
)
EXPECTED_CONTROLS = [
    "abi_signature",
    "sbom",
    "ml_bom",
    "slsa_in_toto",
    "sigstore_cosign",
]
EXPECTED_PROVIDER = {
    "artifact": "ZDL de_zdl_lg",
    "version": "4.0.0",
    "source_commit": "7eabc17097a3ea39f5cc9c030a605ff7edc20ae4",
    "wheel_sha256": (
        "9d35263ac80e80e9730ee21830ffdbe96cf256b72c71e30326ae5865456ade9a"
    ),
    "spacy_version": "3.8.11",
    "pipeline": [
        "tok2vec",
        "tagger",
        "morphologizer",
        "parser",
        "ner",
        "trainable_lemmatizer",
    ],
    "surface_normalized_before_analysis": False,
    "confidence_scores_exposed": False,
}
AUTHORITY_BOUNDARY = (
    "This receipt proves one private, deterministic, source-bound contextual "
    "provider execution and its measured resource cost. It does not accept a "
    "German reading, token boundary, morphology, lemma, lexeme, sign, concept, "
    "translation, semantic claim, relation, graph edge, canon effect, rights "
    "clearance, publication route, winner, or human task."
)
PROVENANCE_EVENT_REF = (
    "tos.event.annotation.zarathustra-morphology-context-b-result.2026-08-12"
)


class MorphologyContextualResultError(RuntimeError):
    """Raised when private contextual evidence does not close exactly."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise MorphologyContextualResultError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise MorphologyContextualResultError(f"{path} must contain a JSON object")
    return payload


def canonical_line(payload: object) -> bytes:
    return (
        json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("utf-8")


def _assert_equal(actual: object, expected: object, label: str) -> None:
    if actual != expected:
        raise MorphologyContextualResultError(
            f"{label} drift: observed {actual!r}, expected {expected!r}"
        )


def _assert_float(actual: object, expected: float, label: str) -> None:
    if (
        not isinstance(actual, (int, float))
        or isinstance(actual, bool)
        or not math.isclose(float(actual), expected, rel_tol=0.0, abs_tol=1e-15)
    ):
        raise MorphologyContextualResultError(
            f"{label} drift: observed {actual!r}, expected {expected!r}"
        )


def file_record(path: Path, *, source_bearing: bool) -> dict[str, Any]:
    mode = stat.S_IMODE(path.stat().st_mode)
    if mode != 0o600:
        raise MorphologyContextualResultError(
            f"private evidence {path} has disallowed mode {mode:04o}"
        )
    return {
        "sha256": sha256_file(path),
        "bytes": path.stat().st_size,
        "mode": "0600",
        "source_bearing": source_bearing,
    }


def _validate_token(token: object, context: str, token_count: int) -> None:
    expected_keys = {
        "dep",
        "end_offset",
        "ent_type",
        "head_token_index",
        "is_sent_start",
        "lemma",
        "morph",
        "pos",
        "start_offset",
        "tag",
        "text",
        "token_index",
        "whitespace",
    }
    if not isinstance(token, dict) or set(token) != expected_keys:
        raise MorphologyContextualResultError("provider token shape drift")
    start = token["start_offset"]
    end = token["end_offset"]
    head = token["head_token_index"]
    if (
        not isinstance(start, int)
        or isinstance(start, bool)
        or not isinstance(end, int)
        or isinstance(end, bool)
        or not 0 <= start < end <= len(context)
        or context[start:end] != token["text"]
        or not isinstance(token["whitespace"], str)
        or (head is not None and (not isinstance(head, int) or not 0 <= head < token_count))
        or not isinstance(token["morph"], dict)
    ):
        raise MorphologyContextualResultError("provider token return drift")


def inspect_raw_output(path: Path) -> dict[str, Any]:
    """Recompute publishable aggregates without returning linguistic strings."""

    expected_keys = {
        "authority",
        "context_id",
        "context_sha256",
        "context_text",
        "episode_id",
        "exact_form_sha256",
        "form_key",
        "input_preserved",
        "item_ref",
        "occurrence_id",
        "part_order",
        "provider",
        "schema_version",
        "selection_rank",
        "selection_role",
        "target_end_offset",
        "target_exact_form",
        "target_start_offset",
        "target_tokens",
        "tokenization",
        "tokens",
    }
    row_count = 0
    exact_alignments = 0
    token_counts: Counter[str] = Counter()
    target_pos: Counter[str] = Counter()
    target_tag: Counter[str] = Counter()
    ranks: list[int] = []
    roles: list[str] = []
    parts: list[int] = []
    stream_digest = hashlib.sha256()

    with path.open("rb") as handle:
        for line_number, encoded in enumerate(handle, start=1):
            stream_digest.update(encoded)
            try:
                row = json.loads(encoded.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError) as exc:
                raise MorphologyContextualResultError(
                    f"raw row {line_number} is invalid UTF-8 JSON"
                ) from exc
            if not isinstance(row, dict) or set(row) != expected_keys:
                raise MorphologyContextualResultError(
                    f"raw row {line_number} has unexpected fields"
                )
            if canonical_line(row) != encoded:
                raise MorphologyContextualResultError(
                    f"raw row {line_number} is not canonical JSONL"
                )
            context = row["context_text"]
            target = row["target_exact_form"]
            start = row["target_start_offset"]
            end = row["target_end_offset"]
            if (
                row["schema_version"] != "tos_zdl_contextual_morphology_row_v1"
                or row["episode_id"] != "zarathustra-selected-form-context-b-v1"
                or row["exact_form_sha256"] != EXPECTED_FORM_SHA256
                or row["form_key"] != f"lexical-form:sha256:{EXPECTED_FORM_SHA256}"
                or not isinstance(context, str)
                or hashlib.sha256(context.encode("utf-8")).hexdigest()
                != row["context_sha256"]
                or not isinstance(target, str)
                or hashlib.sha256(target.encode("utf-8")).hexdigest()
                != EXPECTED_FORM_SHA256
                or not isinstance(start, int)
                or isinstance(start, bool)
                or not isinstance(end, int)
                or isinstance(end, bool)
                or not 0 <= start < end <= len(context)
                or context[start:end] != target
                or row["input_preserved"] is not True
                or row["provider"] != EXPECTED_PROVIDER
                or row["authority"] != "unreviewed-contextual-provider-proposal"
            ):
                raise MorphologyContextualResultError(
                    f"raw row {line_number} failed source/provider closure"
                )
            tokens = row["tokens"]
            target_tokens = row["target_tokens"]
            if not isinstance(tokens, list) or not tokens or not isinstance(target_tokens, list):
                raise MorphologyContextualResultError("provider token arrays drift")
            if [token.get("token_index") for token in tokens if isinstance(token, dict)] != list(
                range(len(tokens))
            ):
                raise MorphologyContextualResultError("provider token order drift")
            for token in tokens:
                _validate_token(token, context, len(tokens))
            reconstructed = "".join(
                token["text"] + token["whitespace"] for token in tokens
            )
            if reconstructed != context:
                raise MorphologyContextualResultError("provider token stream drift")
            expected_target_tokens = [
                token
                for token in tokens
                if token["start_offset"] < end and token["end_offset"] > start
            ]
            if target_tokens != expected_target_tokens or not target_tokens:
                raise MorphologyContextualResultError("target token alignment drift")
            alignment = row["tokenization"]
            exact = (
                len(target_tokens) == 1
                and target_tokens[0]["start_offset"] == start
                and target_tokens[0]["end_offset"] == end
                and target_tokens[0]["text"] == target
            )
            expected_alignment = {
                "token_count": len(tokens),
                "target_token_count": len(target_tokens),
                "exact_single_token_alignment": exact,
                "split_or_expanded_alignment": not exact,
                "target_covered": (
                    target_tokens[0]["start_offset"] <= start
                    and target_tokens[-1]["end_offset"] >= end
                ),
            }
            if alignment != expected_alignment or not alignment["target_covered"]:
                raise MorphologyContextualResultError("tokenization summary drift")

            row_count += 1
            exact_alignments += int(exact)
            token_counts[str(len(target_tokens))] += 1
            ranks.append(row["selection_rank"])
            roles.append(row["selection_role"])
            parts.append(row["part_order"])
            for token in target_tokens:
                target_pos[token["pos"] or "<none>"] += 1
                target_tag[token["tag"] or "<none>"] += 1

    if (
        row_count != 3
        or ranks != EXPECTED_SELECTION_RANKS
        or roles != EXPECTED_SELECTION_ROLES
        or parts != EXPECTED_PART_ORDERS
    ):
        raise MorphologyContextualResultError("frozen contextual selection drift")
    return {
        "stream_sha256": stream_digest.hexdigest(),
        "row_count": row_count,
        "selection_ranks": ranks,
        "selection_roles": roles,
        "part_orders": parts,
        "exact_single_token_alignment_count": exact_alignments,
        "split_or_expanded_alignment_count": row_count - exact_alignments,
        "target_token_count_distribution": dict(sorted(token_counts.items())),
        "target_pos": dict(sorted(target_pos.items())),
        "target_tag": dict(sorted(target_tag.items())),
    }


def verify_and_build_receipt(
    run_root: Path,
    resource_receipt_path: Path,
) -> dict[str, Any]:
    run_root = run_root.resolve()
    resource_receipt_path = resource_receipt_path.resolve()
    try:
        run_relative = run_root.relative_to(ARTIFACT_OWNER_ROOT.resolve())
        resource_relative = resource_receipt_path.relative_to(
            ARTIFACT_OWNER_ROOT.resolve()
        )
    except ValueError as exc:
        raise MorphologyContextualResultError(
            "run and resource evidence must remain under the owner artifact root"
        ) from exc
    if (
        run_relative.as_posix()
        != f"{EXPERIMENT_ID}/{EXPECTED_RUN_ID}/variant-B"
        or resource_relative.as_posix()
        != f"{EXPERIMENT_ID}/resource-runs/{EXPECTED_RUN_ID}.json"
    ):
        raise MorphologyContextualResultError("run/resource route drift")

    paths = {
        "run_receipt": run_root / "run.receipt.json",
        "experiment_spec": run_root / "experiment.spec.json",
        "preflight": run_root / "receipts/preflight.json",
        "execution": run_root / "receipts/execution.json",
        "repeat_determinism": run_root / "receipts/repeat-determinism.json",
        "metrics": run_root / "metrics/contextual-summary.json",
        "raw_output": run_root / "raw-output/zdl-contextual-morphology.jsonl",
        "resource_launch": resource_receipt_path,
        "runtime_manifest": RUNTIME_MANIFEST_PATH,
        "registry_record": REGISTRY_RECORD_PATH,
    }
    missing = [name for name, path in paths.items() if not path.is_file()]
    if missing:
        raise MorphologyContextualResultError(f"run evidence is incomplete: {missing}")

    plan_path = REPO_ROOT / PLAN_REF
    context_receipt_path = REPO_ROOT / CONTEXT_RECEIPT_REF
    admission_path = REPO_ROOT / ADMISSION_REF
    generator_path = REPO_ROOT / GENERATOR_REF
    _assert_equal(sha256_file(plan_path), EXPECTED_PLAN_SHA256, "plan digest")
    _assert_equal(
        sha256_file(context_receipt_path),
        EXPECTED_CONTEXT_RECEIPT_SHA256,
        "context receipt digest",
    )
    plan = load_json(plan_path)
    context_receipt = load_json(context_receipt_path)
    admission = load_json(admission_path)
    run_receipt = load_json(paths["run_receipt"])
    experiment = load_json(paths["experiment_spec"])
    preflight = load_json(paths["preflight"])
    execution = load_json(paths["execution"])
    repeat = load_json(paths["repeat_determinism"])
    metrics = load_json(paths["metrics"])
    resource = load_json(paths["resource_launch"])
    runtime = load_json(paths["runtime_manifest"])
    registry = load_json(paths["registry_record"])
    raw = inspect_raw_output(paths["raw_output"])

    _assert_equal(
        {
            "run_id": run_receipt.get("run_id"),
            "experiment_id": run_receipt.get("experiment_id"),
            "variant": run_receipt.get("variant"),
            "status": run_receipt.get("status"),
            "manual_review_refs": run_receipt.get("manual_review_refs"),
            "model_inspection_refs": run_receipt.get("model_inspection_refs"),
            "errors": run_receipt.get("errors"),
            "retention_decision": run_receipt.get("retention_decision"),
        },
        {
            "run_id": EXPECTED_RUN_ID,
            "experiment_id": EXPERIMENT_ID,
            "variant": VARIANT,
            "status": "awaiting-triggered-review",
            "manual_review_refs": [],
            "model_inspection_refs": [],
            "errors": [],
            "retention_decision": "retain",
        },
        "run receipt state",
    )
    _assert_equal(experiment.get("experiment_id"), EXPERIMENT_ID, "experiment")
    _assert_equal(
        experiment.get("source_plan_sha256_by_variant", {}).get("B"),
        EXPECTED_PLAN_SHA256,
        "experiment B plan",
    )
    _assert_equal(preflight.get("decision"), "ready", "preflight")
    _assert_equal(preflight.get("variant"), VARIANT, "preflight variant")
    for label in ("source_plan_admission", "runtime_admission"):
        if preflight.get(label, {}).get("verified") is not True:
            raise MorphologyContextualResultError(f"{label} is not verified")

    _assert_equal(
        execution.get("schema_version"),
        "tos_zdl_contextual_morphology_execution_v1",
        "execution schema",
    )
    _assert_equal(execution.get("source_plan_sha256"), EXPECTED_PLAN_SHA256, "execution plan")
    _assert_equal(execution.get("source_packet_sha256"), EXPECTED_PACKET_SHA256, "execution packet")
    _assert_equal(execution.get("runner_sha256"), EXPECTED_RUNNER_SHA256, "runner")
    _assert_equal(
        execution.get("runtime_manifest_sha256"),
        EXPECTED_RUNTIME_MANIFEST_SHA256,
        "runtime manifest",
    )
    _assert_equal(execution.get("network_used"), False, "network use")
    _assert_equal(execution.get("source_content_public"), False, "source visibility")
    for path_key, digest_key in (
        ("raw_output", "raw_output_sha256"),
        ("metrics", "metrics_sha256"),
        ("repeat_determinism", "repeat_receipt_sha256"),
    ):
        _assert_equal(execution.get(digest_key), sha256_file(paths[path_key]), digest_key)
    packet_path = Path(str(execution.get("source_packet_ref", ""))).resolve()
    if not packet_path.is_file():
        raise MorphologyContextualResultError("private source packet is unavailable")
    _assert_equal(sha256_file(packet_path), EXPECTED_PACKET_SHA256, "private packet")
    _assert_equal(stat.S_IMODE(packet_path.stat().st_mode), 0o600, "packet mode")

    _assert_equal(sha256_file(paths["runtime_manifest"]), EXPECTED_RUNTIME_MANIFEST_SHA256, "runtime manifest file")
    _assert_equal(runtime.get("status"), "verified", "runtime status")
    _assert_equal(runtime.get("runtime_id"), "zdl-de-zdl-lg-4.0.0-7eabc170-py312", "runtime id")
    _assert_equal(runtime.get("experiment_id"), EXPERIMENT_ID, "runtime experiment")
    _assert_equal(runtime.get("variant"), VARIANT, "runtime variant")
    _assert_equal(runtime.get("artifact_set_sha256"), EXPECTED_ARTIFACT_SET_SHA256, "artifact set")
    gate = runtime.get("artifact_admission", {})
    _assert_equal(gate.get("verdict"), "allow", "artifact gate")
    _assert_equal(gate.get("record_id"), EXPECTED_RECORD_ID, "artifact record")
    _assert_equal(gate.get("subject_digest"), EXPECTED_SUBJECT_DIGEST, "artifact subject")
    _assert_equal(
        gate.get("subjects_aggregate_digest"),
        EXPECTED_SUBJECTS_AGGREGATE,
        "artifact subject store",
    )
    _assert_equal(gate.get("trust_root_mode"), "local_dev", "trust root")
    _assert_equal(gate.get("gate_receipt_sha256"), sha256_file(Path(gate["gate_receipt_ref"])), "gate receipt")

    _assert_equal(registry.get("record_id"), EXPECTED_RECORD_ID, "registry record")
    _assert_equal(registry.get("subject_digest"), EXPECTED_SUBJECT_DIGEST, "registry subject")
    _assert_equal(registry.get("artifact_subjects_digest"), EXPECTED_SUBJECTS_AGGREGATE, "registry store")
    _assert_equal(registry.get("latest_eligible"), True, "latest eligibility")
    _assert_equal(registry.get("lifecycle_state"), "manually-verified", "lifecycle")
    _assert_equal(registry.get("trust_root_mode"), "local_dev", "registry trust root")
    _assert_equal(registry.get("verification_ok"), True, "registry verification")
    _assert_equal(registry.get("required_controls"), EXPECTED_CONTROLS, "required controls")
    _assert_equal(registry.get("present_controls"), EXPECTED_CONTROLS, "present controls")
    _assert_equal(registry.get("verified_controls"), EXPECTED_CONTROLS, "verified controls")
    _assert_equal(registry.get("artifact_subject_store", {}).get("ok"), True, "subject store")
    _assert_equal(registry.get("artifact_subject_store", {}).get("files"), 44, "subject store files")

    _assert_equal(metrics.get("experiment_id"), EXPERIMENT_ID, "metrics experiment")
    _assert_equal(metrics.get("variant"), VARIANT, "metrics variant")
    _assert_equal(metrics.get("provider"), {
        "model_version": "4.0.0",
        "spacy_version": "3.8.11",
        "pipeline": EXPECTED_PROVIDER["pipeline"],
        "source_commit": EXPECTED_PROVIDER["source_commit"],
        "wheel_sha256": EXPECTED_PROVIDER["wheel_sha256"],
        "confidence_scores_exposed": False,
    }, "metrics provider")
    _assert_equal(metrics.get("input"), {
        "plan_sha256": EXPECTED_PLAN_SHA256,
        "packet_sha256": EXPECTED_PACKET_SHA256,
        "row_count": raw["row_count"],
        "selection_ranks": raw["selection_ranks"],
        "selection_roles": raw["selection_roles"],
        "exact_surface_mutated": False,
        "b_output_visible_during_selection": False,
    }, "metrics input")
    _assert_equal(metrics.get("tokenization"), {
        "exact_single_token_alignment_count": raw["exact_single_token_alignment_count"],
        "split_or_expanded_alignment_count": raw["split_or_expanded_alignment_count"],
        "target_token_count_distribution": raw["target_token_count_distribution"],
    }, "tokenization")
    _assert_equal(metrics.get("proposal_distributions"), {
        "target_pos": raw["target_pos"],
        "target_tag": raw["target_tag"],
    }, "proposal distributions")
    _assert_equal(metrics.get("quality"), {
        "status": "unmeasured-no-german-competent-gold",
        "provider_proposal_is_accuracy": False,
        "machine_agreement_or_disagreement_is_gold": False,
    }, "quality boundary")
    _assert_equal(metrics.get("followup"), {
        "status": "closed-machine-proposal-awaiting-real-trigger",
        "c_status": "blocked-question-inapplicable",
        "human_work_scheduled": False,
        "semantic_effect": False,
    }, "followup boundary")
    _assert_equal(metrics.get("rights"), {
        "execution": "owner-local-private-research-only",
        "redistribution": "blocked",
        "source_content_public": False,
    }, "rights boundary")
    _assert_equal(metrics.get("bytes"), {
        "runtime": runtime["runtime_bytes"],
        "raw_output": paths["raw_output"].stat().st_size,
        "source_packet": packet_path.stat().st_size,
        "metrics": paths["metrics"].stat().st_size,
    }, "byte accounting")

    _assert_equal(repeat.get("deterministic"), True, "repeat determinism")
    _assert_equal(repeat.get("mismatch_count"), 0, "repeat mismatches")
    _assert_equal(repeat.get("row_count"), raw["row_count"], "repeat rows")
    _assert_equal(repeat.get("pass_1_stream_sha256"), raw["stream_sha256"], "repeat pass 1")
    _assert_equal(repeat.get("pass_2_stream_sha256"), raw["stream_sha256"], "repeat pass 2")
    _assert_equal(metrics.get("repeat_determinism", {}).get("pass_1_stream_sha256"), raw["stream_sha256"], "metrics repeat")

    _assert_equal(resource.get("ok"), True, "resource launch")
    _assert_equal(resource.get("execution", {}).get("returncode"), 0, "resource return code")
    _assert_equal(resource.get("request", {}).get("force"), False, "resource force")
    systemd = resource.get("execution", {}).get("systemd", {})
    _assert_equal(systemd.get("result"), "success", "resource service")
    peaks = resource.get("startup_admission", {}).get("demand_observation", {}).get("peaks", {})
    _assert_equal(peaks.get("ok"), True, "resource peaks")

    for label, value in (
        ("plan status", plan.get("status")),
        ("context receipt state", context_receipt.get("variant_state", {}).get("b")),
        ("historical admission", admission.get("status")),
    ):
        if value not in {
            "ready-to-materialize-context-packet",
            "admitted-unacquired",
            "artifact-acquired-admission-denied-b-not-run",
        }:
            raise MorphologyContextualResultError(f"{label} drift")

    private_artifacts = {
        name: file_record(path, source_bearing=name in {"execution", "raw_output"})
        for name, path in paths.items()
        if name not in {"runtime_manifest", "registry_record"}
    }
    private_artifacts["runtime_manifest"] = {
        "sha256": sha256_file(paths["runtime_manifest"]),
        "bytes": paths["runtime_manifest"].stat().st_size,
        "mode": f"{stat.S_IMODE(paths['runtime_manifest'].stat().st_mode):04o}",
        "source_bearing": False,
    }
    private_artifacts["registry_record"] = {
        "sha256": sha256_file(paths["registry_record"]),
        "bytes": paths["registry_record"].stat().st_size,
        "mode": f"{stat.S_IMODE(paths['registry_record'].stat().st_mode):04o}",
        "source_bearing": False,
    }

    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "morphology-contextual-result-receipt.schema.json"
        ),
        "schema_version": "tos_morphology_contextual_result_receipt_v1",
        "generated_or_authored": "generated_from_private_contextual_provider_run",
        "receipt_id": "morphology-contextual-result:zarathustra-selected-form-b.zdl-4.0.0.v1",
        "recorded_at_utc": run_receipt["finished_at_utc"],
        "status": "b-executed-machine-proposal-awaiting-real-trigger",
        "experiment_id": EXPERIMENT_ID,
        "variant": VARIANT,
        "question": {
            "plan": {"ref": PLAN_REF, "sha256": EXPECTED_PLAN_SHA256},
            "context_receipt": {
                "ref": CONTEXT_RECEIPT_REF,
                "sha256": EXPECTED_CONTEXT_RECEIPT_SHA256,
            },
            "historical_negative_admission": {
                "ref": ADMISSION_REF,
                "sha256": sha256_file(admission_path),
                "retained": True,
                "superseded": False,
            },
            "frozen_before_b_output": True,
        },
        "generator": {
            "ref": GENERATOR_REF,
            "sha256": sha256_file(generator_path),
        },
        "private_run": {
            "artifact_owner": "abyss-machine:storage/artifacts/tree-of-sophia-foundation-lab",
            "relative_ref": run_relative.as_posix(),
            "run_id": EXPECTED_RUN_ID,
            "status": "awaiting-triggered-review",
            "started_at_utc": run_receipt["started_at_utc"],
            "finished_at_utc": run_receipt["finished_at_utc"],
            "retention_decision": "retain",
            "visibility": "owner-local-only",
            "manual_review_count": 0,
            "model_inspection_count": 0,
        },
        "implementation": {
            "owner": "abyss-stack",
            "commit": EXPECTED_STACK_COMMIT,
            "runner_sha256": EXPECTED_RUNNER_SHA256,
        },
        "provider": {
            "artifact": "ZDL de_zdl_lg",
            "version": "4.0.0",
            "spacy_version": "3.8.11",
            "source_commit": EXPECTED_PROVIDER["source_commit"],
            "principal_wheel_sha256": EXPECTED_PROVIDER["wheel_sha256"],
            "pipeline": EXPECTED_PROVIDER["pipeline"],
            "confidence_scores_exposed": False,
            "execution_posture": "unreviewed-contextual-provider-proposal",
        },
        "artifact_admission": {
            "record_id": EXPECTED_RECORD_ID,
            "record_file_sha256": sha256_file(paths["registry_record"]),
            "subject_digest": EXPECTED_SUBJECT_DIGEST,
            "subjects_aggregate_digest": EXPECTED_SUBJECTS_AGGREGATE,
            "lifecycle_state": "manually-verified",
            "latest_eligible": True,
            "trust_root_mode": "local_dev",
            "required_controls": EXPECTED_CONTROLS,
            "present_controls": EXPECTED_CONTROLS,
            "verified_controls": EXPECTED_CONTROLS,
            "subject_store_file_count": 44,
            "trust_gate_verdict": "allow",
            "rights_effect": "none",
        },
        "source_input": {
            "work_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
            "packet_sha256": EXPECTED_PACKET_SHA256,
            "packet_bytes": packet_path.stat().st_size,
            "packet_mode": "0600",
            "row_count": raw["row_count"],
            "selection_ranks": raw["selection_ranks"],
            "selection_roles": raw["selection_roles"],
            "part_orders": raw["part_orders"],
            "exact_surface_mutated": False,
            "source_text_accepted": False,
        },
        "runtime": {
            "runtime_id": runtime["runtime_id"],
            "manifest_sha256": EXPECTED_RUNTIME_MANIFEST_SHA256,
            "artifact_set_sha256": EXPECTED_ARTIFACT_SET_SHA256,
            "runtime_bytes": runtime["runtime_bytes"],
            "network_used": False,
        },
        "private_artifacts": private_artifacts,
        "tokenization": metrics["tokenization"],
        "proposal_distributions": metrics["proposal_distributions"],
        "repeat_determinism": {
            "deterministic": True,
            "pass_1_stream_sha256": raw["stream_sha256"],
            "pass_2_stream_sha256": raw["stream_sha256"],
            "mismatch_count": 0,
            "second_pass_raw_output_retained": False,
        },
        "performance": {
            **metrics["performance"],
            "host_service_wall_seconds": float(str(systemd["service_runtime"]).removesuffix("s")),
            "host_service_cpu_seconds": float(str(systemd["cpu_time_consumed"]).removesuffix("s")),
            "host_cgroup_footprint_peak_mib": peaks["footprint_peak_mib"],
            "host_cgroup_memory_peak_bytes": peaks["memory_peak_bytes"],
            "host_cgroup_swap_peak_bytes": peaks["memory_swap_peak_bytes"],
            "resource_force_used": False,
        },
        "quality": {
            "status": "unmeasured-no-german-competent-gold",
            "german_competent_gold_count": 0,
            "accepted_tokenization_count": 0,
            "accepted_morphology_count": 0,
            "accepted_lemma_count": 0,
            "provider_proposal_is_accuracy": False,
            "machine_repeatability_is_gold": False,
        },
        "followup": {
            "status": "closed-machine-proposal-awaiting-real-trigger",
            "c_status": "blocked-question-inapplicable",
            "human_work_scheduled": False,
            "automatic_review_opened": False,
            "automatic_promotion_authorized": False,
        },
        "rights_and_visibility": {
            "execution": "owner-local-private-research-only",
            "redistribution": "blocked",
            "private_source_and_raw_output": "owner-local-only",
            "tracked_receipt_contains_source_strings": False,
            "tracked_receipt_contains_sequence": False,
            "tracked_receipt_contains_context": False,
            "tracked_receipt_contains_provider_lemma_strings": False,
            "source_payload_publication_authorized": False,
            "raw_output_publication_authorized": False,
            "tracked_receipt_publication_authorized": False,
            "future_site_route": "blocked",
        },
        "semantic_boundary": {
            "creates_accepted_source": False,
            "creates_tokenization": False,
            "creates_morphology": False,
            "creates_lemma": False,
            "creates_lexeme": False,
            "creates_sign_candidate": False,
            "creates_sign": False,
            "creates_semantic_claim": False,
            "creates_translation": False,
            "creates_graph_fact": False,
            "changes_canon": False,
            "opens_human_backlog": False,
        },
        "provenance_event_ref": PROVENANCE_EVENT_REF,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def build_provenance(receipt: dict[str, Any], receipt_sha256: str) -> dict[str, Any]:
    artifacts = receipt["private_artifacts"]
    return {
        "schema_version": "tos_provenance_event_v1",
        "event_id": PROVENANCE_EVENT_REF,
        "event_type": "annotation",
        "started_at": receipt["private_run"]["started_at_utc"],
        "ended_at": receipt["private_run"]["finished_at_utc"],
        "agent_refs": ["software:abyss-stack-zdl-contextual-morphology", "model:codex"],
        "inputs": [
            {"ref": PLAN_REF, "role": "frozen-output-blind-contextual-plan", "sha256": EXPECTED_PLAN_SHA256},
            {"ref": CONTEXT_RECEIPT_REF, "role": "text-free-context-packet-receipt", "sha256": EXPECTED_CONTEXT_RECEIPT_SHA256},
            {"ref": ADMISSION_REF, "role": "retained-historical-negative-artifact-admission", "sha256": receipt["question"]["historical_negative_admission"]["sha256"]},
            {"ref": f"owner-local-artifacts/tree-of-sophia-foundation-lab/{EXPERIMENT_ID}/{EXPECTED_RUN_ID}/variant-B/run.receipt.json", "role": "private-contextual-run-receipt", "sha256": artifacts["run_receipt"]["sha256"]},
            {"ref": f"owner-local-artifacts/tree-of-sophia-foundation-lab/{EXPERIMENT_ID}/{EXPECTED_RUN_ID}/variant-B/metrics/contextual-summary.json", "role": "private-text-free-metrics", "sha256": artifacts["metrics"]["sha256"]},
            {"ref": f"owner-local-artifacts/tree-of-sophia-foundation-lab/{EXPERIMENT_ID}/resource-runs/{EXPECTED_RUN_ID}.json", "role": "owner-resource-run-receipt", "sha256": artifacts["resource_launch"]["sha256"]},
        ],
        "outputs": [
            {"ref": RESULT_REF, "role": "tracked-text-free-contextual-result-receipt", "sha256": receipt_sha256}
        ],
        "method": {
            "maker_type": "software",
            "name": "Tree of Sophia private contextual morphology result recorder",
            "version": "1",
            "artifact_digest": receipt["generator"]["sha256"],
            "runtime": "abyss-stack ZDL de_zdl_lg 4.0.0 CPU lane",
            "device": "CPU",
            "configuration": {
                "variant": "B",
                "row_count": 3,
                "selection_ranks": EXPECTED_SELECTION_RANKS,
                "source_strings_tracked": False,
                "german_competent_gold_count": 0,
                "human_work_scheduled": False,
                "semantic_effect": False,
                "redistribution": "blocked",
            },
            "prompt_or_instruction_ref": "ToS/research-packets/foundation-laboratory-2026-07/HISTORICAL_GERMAN_MORPHOLOGY_B_EXECUTABLE_STOP_LINE_2026-08-12.md",
        },
        "status": "completed_with_warnings",
        "warnings": [
            "provider output is an unreviewed machine proposal and German accuracy remains unmeasured",
            "source packet and raw provider output remain owner-local and redistribution-blocked",
            "the earlier denied artifact remains retained historical evidence rather than being rewritten",
            "no linguistic, semantic, graph, canon, publication, or human-backlog authority was created",
        ],
        "receipt_refs": [RESULT_REF],
        "rights_basis_ref": None,
        "event_version": 1,
        "supersedes_event_ref": None,
    }


def _validate(payload: dict[str, Any], schema_ref: str, label: str) -> None:
    schema = load_json(REPO_ROOT / schema_ref)
    errors = sorted(
        Draft202012Validator(schema).iter_errors(payload),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        details = "; ".join(
            f"{'/'.join(map(str, error.absolute_path)) or '<root>'}: {error.message}"
            for error in errors
        )
        raise MorphologyContextualResultError(f"{label} schema failed: {details}")


def _encoded_json(payload: dict[str, Any]) -> bytes:
    return (json.dumps(payload, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def write_outputs(
    receipt_path: Path,
    provenance_path: Path,
    receipt: dict[str, Any],
    *,
    check: bool,
) -> None:
    receipt_bytes = _encoded_json(receipt)
    provenance = build_provenance(
        receipt,
        hashlib.sha256(receipt_bytes).hexdigest(),
    )
    _validate(receipt, SCHEMA_REF, "result receipt")
    _validate(provenance, PROVENANCE_SCHEMA_REF, "result provenance")
    provenance_bytes = canonical_line(provenance)
    serialized = (receipt_bytes + provenance_bytes).decode("utf-8")
    for prohibited in (
        "/srv/",
        "local-content/",
        '"context_text"',
        '"target_exact_form"',
        '"occurrence_id"',
        '"target_start_offset"',
        '"target_end_offset"',
    ):
        if prohibited in serialized:
            raise MorphologyContextualResultError(
                f"tracked contextual result leaks prohibited material: {prohibited}"
            )
    for path, content in (
        (receipt_path, receipt_bytes),
        (provenance_path, provenance_bytes),
    ):
        if check:
            if not path.is_file() or path.read_bytes() != content:
                raise MorphologyContextualResultError(
                    f"tracked contextual result is stale: {path}"
                )
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_suffix(path.suffix + ".tmp")
        temporary.write_bytes(content)
        os.replace(temporary, path)


def parser() -> argparse.ArgumentParser:
    command = argparse.ArgumentParser(
        description=(
            "Verify one private ZDL contextual morphology B run and record "
            "only a text-free result receipt and provenance in Tree of Sophia."
        )
    )
    command.add_argument("--run-root", type=Path, required=True)
    command.add_argument("--resource-receipt", type=Path, required=True)
    command.add_argument("--output", type=Path, default=REPO_ROOT / RESULT_REF)
    command.add_argument(
        "--provenance-output",
        type=Path,
        default=REPO_ROOT / PROVENANCE_REF,
    )
    command.add_argument("--check", action="store_true")
    return command


def main(argv: list[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
        receipt = verify_and_build_receipt(
            arguments.run_root,
            arguments.resource_receipt,
        )
        write_outputs(
            arguments.output.resolve(),
            arguments.provenance_output.resolve(),
            receipt,
            check=arguments.check,
        )
    except MorphologyContextualResultError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    print(
        json.dumps(
            {
                "status": receipt["status"],
                "run_id": receipt["private_run"]["run_id"],
                "row_count": receipt["source_input"]["row_count"],
                "repeatable": receipt["repeat_determinism"]["deterministic"],
                "quality": receipt["quality"]["status"],
                "human_work_scheduled": receipt["followup"]["human_work_scheduled"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
