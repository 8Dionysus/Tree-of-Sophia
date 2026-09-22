#!/usr/bin/env python3
"""Record a text-free ToS receipt from one private DWDSmor census run."""

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
from typing import Any, Iterable

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
PLAN_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "morphology-evaluation-plan.v1.json"
)
RESULT_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "morphology-census-result.a-dwdsmor-open-0.18.0.v1.json"
)
SCHEMA_REF = "ToS/contracts/morphology-census-result-receipt.schema.json"
GENERATOR_REF = "scripts/record_zarathustra_morphology_census_result.py"
ARTIFACT_OWNER_ROOT = Path(
    "/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab"
)
EXPERIMENT_ID = "tos-historical-german-morphology-v1"
VARIANT = "A"
EXPECTED_PLAN_SHA256 = (
    "6075b2926b49c4b79b0a186e5338eaa7dd33e847ea1b38b322ef3403dcfcd4c5"
)
EXPECTED_INPUT_SHA256 = (
    "1c734e6f8371e28b863d58e216b7a649aa6d603192f1f7310056e8caa022eb9a"
)
EXPECTED_PROVIDER = {
    "artifact": "DWDSmor Open",
    "version": "0.18.0",
    "source_commit": "f97b92ce2a5d6db8750afbdb222eb39470e57cf6",
    "wheel_sha256": (
        "395a15e15286b0c191b42355b6e3c2a43c8959621ccf3563336c2e30399a2973"
    ),
    "surface_normalized_before_analysis": False,
}
JOINERS = frozenset({"-", "'", "’", "‐", "‑"})
AUTHORITY_BOUNDARY = (
    "This receipt proves one exhaustive, deterministic, text-free mechanical "
    "provider census and its measured resource cost. It does not accept a "
    "German source reading, morphological analysis, lemma, lexeme, sign, "
    "concept, translation, claim, relation, graph edge, rights clearance, "
    "publication route, winner, contextual follow-up, or human task."
)


class MorphologyResultError(RuntimeError):
    """Raised when private census evidence does not close exactly."""


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
        raise MorphologyResultError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise MorphologyResultError(f"{path} must contain a JSON object")
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


def file_record(path: Path, *, source_bearing: bool) -> dict[str, Any]:
    mode = stat.S_IMODE(path.stat().st_mode)
    allowed_modes = {0o600} if source_bearing else {0o600, 0o644}
    if mode not in allowed_modes:
        raise MorphologyResultError(f"{path} has disallowed mode {mode:04o}")
    return {
        "sha256": sha256_file(path),
        "bytes": path.stat().st_size,
        "mode": f"{mode:04o}",
        "source_bearing": source_bearing,
    }


def numeric_counter(counter: Counter[int]) -> dict[str, int]:
    return {str(key): counter[key] for key in sorted(counter)}


def named_counter(counter: Counter[str]) -> dict[str, int]:
    return dict(sorted(counter.items()))


def occurrence_bucket(count: int) -> str:
    if count == 1:
        return "1"
    if count <= 4:
        return "2-4"
    if count <= 9:
        return "5-9"
    if count <= 49:
        return "10-49"
    return "50-plus"


def length_bucket(length: int) -> str:
    if length <= 2:
        return "1-2"
    if length <= 5:
        return "3-5"
    if length <= 10:
        return "6-10"
    if length <= 20:
        return "11-20"
    return "21-plus"


def case_bucket(surface: str) -> str:
    if surface.islower():
        return "lower"
    if surface.isupper():
        return "upper"
    if surface.istitle():
        return "title"
    return "mixed-or-uncased"


def character_bucket(surface: str) -> str:
    if any(character.isdigit() for character in surface):
        return "contains-digit"
    if any(character in JOINERS for character in surface):
        return "contains-joiner"
    if all(character.isalpha() for character in surface):
        return "alphabetic-only"
    return "contains-other"


def _increment_weighted(
    type_counter: Counter[str],
    token_counter: Counter[str],
    bucket: str,
    occurrence_count: int,
) -> None:
    type_counter[bucket] += 1
    token_counter[bucket] += occurrence_count


def _analysis_labels(
    analyses: Iterable[dict[str, Any]],
    *,
    pos: Counter[str],
    category: Counter[str],
) -> int:
    count = 0
    for analysis in analyses:
        if not isinstance(analysis, dict):
            raise MorphologyResultError("provider analysis must be an object")
        if not all(
            value is None or isinstance(value, (str, int, float, bool))
            for value in analysis.values()
        ):
            raise MorphologyResultError("provider analysis contains a non-scalar value")
        pos[str(analysis.get("pos") or "<none>")] += 1
        category[str(analysis.get("category") or "<none>")] += 1
        count += 1
    return count


def inspect_raw_output(path: Path) -> dict[str, Any]:
    """Recompute all publishable aggregates without returning source strings."""

    expected_keys = {
        "schema_version",
        "form_key",
        "exact_form",
        "exact_form_sha256",
        "normalized_form_sha256",
        "occurrence_count",
        "input_preserved",
        "provider",
        "lemma_analyses",
        "root_analyses",
        "lemma_analysis_count",
        "root_analysis_count",
        "unknown",
        "authority",
    }
    row_count = 0
    token_count = 0
    covered_types = 0
    covered_tokens = 0
    root_types = 0
    root_tokens = 0
    lemma_total = 0
    root_total = 0
    lemma_counts: Counter[int] = Counter()
    root_counts: Counter[int] = Counter()
    provider_pos: Counter[str] = Counter()
    provider_category: Counter[str] = Counter()
    root_pos: Counter[str] = Counter()
    root_category: Counter[str] = Counter()
    unknown_frequency_types: Counter[str] = Counter()
    unknown_frequency_tokens: Counter[str] = Counter()
    unknown_length_types: Counter[str] = Counter()
    unknown_length_tokens: Counter[str] = Counter()
    unknown_case_types: Counter[str] = Counter()
    unknown_case_tokens: Counter[str] = Counter()
    unknown_character_types: Counter[str] = Counter()
    unknown_character_tokens: Counter[str] = Counter()
    maximum_unknown_occurrences = 0
    maximum_unknown_length = 0
    previous_order: tuple[str, str] | None = None
    stream_digest = hashlib.sha256()

    with path.open("rb") as handle:
        for line_number, encoded in enumerate(handle, start=1):
            stream_digest.update(encoded)
            try:
                row = json.loads(encoded.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError) as exc:
                raise MorphologyResultError(
                    f"raw row {line_number} is invalid UTF-8 JSON"
                ) from exc
            if not isinstance(row, dict) or set(row) != expected_keys:
                raise MorphologyResultError(
                    f"raw row {line_number} has unexpected fields"
                )
            if canonical_line(row) != encoded:
                raise MorphologyResultError(
                    f"raw row {line_number} is not canonical JSONL"
                )
            surface = row["exact_form"]
            surface_digest = row["exact_form_sha256"]
            occurrence_count = row["occurrence_count"]
            if (
                row["schema_version"] != "tos_dwdsmor_analysis_row_v1"
                or not isinstance(surface, str)
                or not surface
                or hashlib.sha256(surface.encode("utf-8")).hexdigest() != surface_digest
                or row["form_key"] != f"lexical-form:sha256:{surface_digest}"
                or not isinstance(row["normalized_form_sha256"], str)
                or len(row["normalized_form_sha256"]) != 64
                or not isinstance(occurrence_count, int)
                or isinstance(occurrence_count, bool)
                or occurrence_count < 1
                or row["input_preserved"] is not True
                or row["provider"] != EXPECTED_PROVIDER
                or row["authority"] != "unreviewed-provider-candidate"
            ):
                raise MorphologyResultError(
                    f"raw row {line_number} failed identity or provider closure"
                )
            order = (surface_digest, surface)
            if previous_order is not None and order <= previous_order:
                raise MorphologyResultError(
                    f"raw row {line_number} violates frozen input order"
                )
            previous_order = order

            lemma = row["lemma_analyses"]
            root = row["root_analyses"]
            if not isinstance(lemma, list) or not isinstance(root, list):
                raise MorphologyResultError(
                    f"raw row {line_number} analysis fields must be arrays"
                )
            observed_lemma_count = _analysis_labels(
                lemma,
                pos=provider_pos,
                category=provider_category,
            )
            observed_root_count = _analysis_labels(
                root,
                pos=root_pos,
                category=root_category,
            )
            if (
                row["lemma_analysis_count"] != observed_lemma_count
                or row["root_analysis_count"] != observed_root_count
                or row["unknown"] is not (observed_lemma_count == 0)
            ):
                raise MorphologyResultError(
                    f"raw row {line_number} analysis counts drift"
                )

            row_count += 1
            token_count += occurrence_count
            lemma_total += observed_lemma_count
            root_total += observed_root_count
            lemma_counts[observed_lemma_count] += 1
            root_counts[observed_root_count] += 1
            if observed_lemma_count:
                covered_types += 1
                covered_tokens += occurrence_count
            else:
                frequency = occurrence_bucket(occurrence_count)
                length = length_bucket(len(surface))
                case = case_bucket(surface)
                character = character_bucket(surface)
                _increment_weighted(
                    unknown_frequency_types,
                    unknown_frequency_tokens,
                    frequency,
                    occurrence_count,
                )
                _increment_weighted(
                    unknown_length_types,
                    unknown_length_tokens,
                    length,
                    occurrence_count,
                )
                _increment_weighted(
                    unknown_case_types,
                    unknown_case_tokens,
                    case,
                    occurrence_count,
                )
                _increment_weighted(
                    unknown_character_types,
                    unknown_character_tokens,
                    character,
                    occurrence_count,
                )
                maximum_unknown_occurrences = max(
                    maximum_unknown_occurrences,
                    occurrence_count,
                )
                maximum_unknown_length = max(
                    maximum_unknown_length,
                    len(surface),
                )
            if observed_root_count:
                root_types += 1
                root_tokens += occurrence_count

    return {
        "stream_sha256": stream_digest.hexdigest(),
        "row_count": row_count,
        "token_occurrence_count": token_count,
        "covered_type_count": covered_types,
        "covered_token_count": covered_tokens,
        "unknown_type_count": row_count - covered_types,
        "unknown_token_count": token_count - covered_tokens,
        "root_type_count": root_types,
        "root_token_count": root_tokens,
        "lemma_analysis_total": lemma_total,
        "root_analysis_total": root_total,
        "lemma_analysis_count": numeric_counter(lemma_counts),
        "root_analysis_count": numeric_counter(root_counts),
        "provider_pos": named_counter(provider_pos),
        "provider_category": named_counter(provider_category),
        "unknown_residue": {
            "review_status": "unreviewed-mechanical-aggregation",
            "triggers_contextual_followup": False,
            "type_count": row_count - covered_types,
            "token_weight": token_count - covered_tokens,
            "maximum_occurrence_count": maximum_unknown_occurrences,
            "maximum_codepoint_length": maximum_unknown_length,
            "occurrence_frequency": {
                "type_counts": named_counter(unknown_frequency_types),
                "token_weights": named_counter(unknown_frequency_tokens),
            },
            "codepoint_length": {
                "type_counts": named_counter(unknown_length_types),
                "token_weights": named_counter(unknown_length_tokens),
            },
            "case_shape": {
                "type_counts": named_counter(unknown_case_types),
                "token_weights": named_counter(unknown_case_tokens),
            },
            "character_shape": {
                "type_counts": named_counter(unknown_character_types),
                "token_weights": named_counter(unknown_character_tokens),
            },
            "source_strings_included": False,
        },
    }


def _assert_equal(actual: object, expected: object, label: str) -> None:
    if actual != expected:
        raise MorphologyResultError(
            f"{label} drift: observed {actual!r}, expected {expected!r}"
        )


def _assert_float(actual: object, expected: float, label: str) -> None:
    if (
        not isinstance(actual, (int, float))
        or isinstance(actual, bool)
        or not math.isclose(float(actual), expected, rel_tol=0.0, abs_tol=1e-15)
    ):
        raise MorphologyResultError(
            f"{label} drift: observed {actual!r}, expected {expected!r}"
        )


def verify_and_build_receipt(run_root: Path) -> dict[str, Any]:
    run_root = run_root.resolve()
    try:
        run_relative = run_root.relative_to(ARTIFACT_OWNER_ROOT.resolve())
    except ValueError as exc:
        raise MorphologyResultError(
            "run root must remain under the owner artifact root"
        ) from exc
    if (
        len(run_relative.parts) != 3
        or run_relative.parts[0] != EXPERIMENT_ID
        or run_relative.parts[2] != "variant-A"
    ):
        raise MorphologyResultError("run root does not match experiment/variant")

    paths = {
        "run_receipt": run_root / "run.receipt.json",
        "experiment_spec": run_root / "experiment.spec.json",
        "preflight": run_root / "receipts/preflight.json",
        "execution": run_root / "receipts/execution.json",
        "repeat_determinism": run_root / "receipts/repeat-determinism.json",
        "host_resource_launch": run_root / "receipts/host-resource-launch.json",
        "metrics": run_root / "metrics/census-summary.json",
        "raw_output": run_root / "raw-output/dwdsmor-census.jsonl",
    }
    if any(not path.is_file() for path in paths.values()):
        missing = [name for name, path in paths.items() if not path.is_file()]
        raise MorphologyResultError(f"run evidence is incomplete: {missing}")

    plan_path = REPO_ROOT / PLAN_REF
    generator_path = REPO_ROOT / GENERATOR_REF
    _assert_equal(sha256_file(plan_path), EXPECTED_PLAN_SHA256, "plan digest")
    plan = load_json(plan_path)
    run_receipt = load_json(paths["run_receipt"])
    experiment = load_json(paths["experiment_spec"])
    preflight = load_json(paths["preflight"])
    execution = load_json(paths["execution"])
    repeat = load_json(paths["repeat_determinism"])
    resource_launch = load_json(paths["host_resource_launch"])
    metrics = load_json(paths["metrics"])
    raw = inspect_raw_output(paths["raw_output"])

    run_id = run_relative.parts[1]
    _assert_equal(run_receipt.get("run_id"), run_id, "run id")
    _assert_equal(
        {
            "experiment_id": run_receipt.get("experiment_id"),
            "variant": run_receipt.get("variant"),
            "status": run_receipt.get("status"),
            "manual_review_refs": run_receipt.get("manual_review_refs"),
            "model_inspection_refs": run_receipt.get("model_inspection_refs"),
            "errors": run_receipt.get("errors"),
            "retention_decision": run_receipt.get("retention_decision"),
        },
        {
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
        experiment.get("source_plan_sha256"), EXPECTED_PLAN_SHA256, "experiment plan"
    )
    _assert_equal(
        run_receipt.get("experiment_spec_sha256"),
        preflight.get("experiment_sha256"),
        "suite experiment digest",
    )
    _assert_equal(preflight.get("decision"), "ready", "preflight decision")
    _assert_equal(preflight.get("variant"), VARIANT, "preflight variant")
    source_admission = preflight.get("source_plan_admission")
    if (
        not isinstance(source_admission, dict)
        or source_admission.get("verified") is not True
    ):
        raise MorphologyResultError("preflight source admission is not verified")
    runtime_admission = preflight.get("runtime_admission")
    if (
        not isinstance(runtime_admission, dict)
        or runtime_admission.get("verified") is not True
    ):
        raise MorphologyResultError("preflight runtime admission is not verified")

    _assert_equal(
        execution.get("schema_version"),
        "tos_dwdsmor_census_execution_v1",
        "execution schema",
    )
    _assert_equal(
        execution.get("source_plan_sha256"), EXPECTED_PLAN_SHA256, "execution plan"
    )
    _assert_equal(
        execution.get("source_packet_sha256"), EXPECTED_INPUT_SHA256, "execution input"
    )
    _assert_equal(execution.get("network_used"), False, "network use")
    _assert_equal(execution.get("source_content_public"), False, "source visibility")
    for path_key, digest_key in (
        ("raw_output", "raw_output_sha256"),
        ("metrics", "metrics_sha256"),
        ("repeat_determinism", "repeat_receipt_sha256"),
    ):
        _assert_equal(
            execution.get(digest_key),
            sha256_file(paths[path_key]),
            digest_key,
        )
    runner_path = Path(str(execution.get("runner_ref", ""))).resolve()
    runtime_manifest_path = Path(
        str(execution.get("runtime_manifest_ref", ""))
    ).resolve()
    source_packet_path = Path(str(execution.get("source_packet_ref", ""))).resolve()
    if not runner_path.is_file() or not runtime_manifest_path.is_file():
        raise MorphologyResultError("runner or runtime manifest is unavailable")
    _assert_equal(
        execution.get("runner_sha256"),
        sha256_file(runner_path),
        "runner digest",
    )
    _assert_equal(
        execution.get("runtime_manifest_sha256"),
        sha256_file(runtime_manifest_path),
        "runtime manifest digest",
    )
    _assert_equal(
        sha256_file(source_packet_path), EXPECTED_INPUT_SHA256, "private input"
    )
    _assert_equal(
        f"{stat.S_IMODE(source_packet_path.stat().st_mode):04o}",
        "0600",
        "private input mode",
    )

    runtime_manifest = load_json(runtime_manifest_path)
    _assert_equal(runtime_manifest.get("status"), "verified", "runtime status")
    _assert_equal(
        runtime_manifest.get("experiment_id"), EXPERIMENT_ID, "runtime experiment"
    )
    _assert_equal(runtime_manifest.get("variant"), VARIANT, "runtime variant")
    _assert_equal(
        runtime_manifest.get("artifact_set_sha256"),
        run_receipt.get("method_revision", {}).get("artifact_digest"),
        "runtime artifact set",
    )

    _assert_equal(metrics.get("experiment_id"), EXPERIMENT_ID, "metrics experiment")
    _assert_equal(metrics.get("variant"), VARIANT, "metrics variant")
    _assert_equal(
        metrics.get("provider", {}).get("artifact"), "DWDSmor Open", "provider"
    )
    _assert_equal(
        metrics.get("provider", {}).get("version"), "0.18.0", "provider version"
    )
    _assert_equal(
        metrics.get("input"),
        {
            "plan_sha256": EXPECTED_PLAN_SHA256,
            "packet_sha256": EXPECTED_INPUT_SHA256,
            "row_count": raw["row_count"],
            "token_occurrence_count": raw["token_occurrence_count"],
            "exact_surface_mutated": False,
        },
        "metrics input",
    )
    coverage = metrics.get("coverage")
    if not isinstance(coverage, dict):
        raise MorphologyResultError("metrics coverage is missing")
    for field in (
        "covered_type_count",
        "covered_token_count",
        "unknown_type_count",
        "unknown_token_count",
        "root_type_count",
        "root_token_count",
    ):
        _assert_equal(coverage.get(field), raw[field], f"coverage {field}")
    _assert_float(
        coverage.get("form_type_coverage"),
        raw["covered_type_count"] / raw["row_count"],
        "form type coverage",
    )
    _assert_float(
        coverage.get("token_weighted_coverage"),
        raw["covered_token_count"] / raw["token_occurrence_count"],
        "token weighted coverage",
    )
    _assert_float(
        coverage.get("unknown_form_rate"),
        raw["unknown_type_count"] / raw["row_count"],
        "unknown form rate",
    )
    expected_distributions = {
        field: raw[field]
        for field in (
            "lemma_analysis_count",
            "root_analysis_count",
            "provider_pos",
            "provider_category",
        )
    }
    _assert_equal(
        metrics.get("distributions"),
        expected_distributions,
        "provider distributions",
    )
    _assert_equal(
        sum(raw["lemma_analysis_count"].values()),
        raw["row_count"],
        "lemma distribution denominator",
    )
    _assert_equal(
        sum(raw["root_analysis_count"].values()),
        raw["row_count"],
        "root distribution denominator",
    )
    _assert_equal(
        sum(raw["provider_pos"].values()),
        raw["lemma_analysis_total"],
        "POS distribution analysis total",
    )
    _assert_equal(
        sum(raw["provider_category"].values()),
        raw["lemma_analysis_total"],
        "category distribution analysis total",
    )

    _assert_equal(repeat.get("deterministic"), True, "repeat determinism")
    _assert_equal(repeat.get("mismatch_count"), 0, "repeat mismatches")
    _assert_equal(repeat.get("row_count"), raw["row_count"], "repeat rows")
    _assert_equal(
        repeat.get("pass_1_stream_sha256"),
        raw["stream_sha256"],
        "pass 1 stream digest",
    )
    _assert_equal(
        repeat.get("pass_2_stream_sha256"),
        raw["stream_sha256"],
        "pass 2 stream digest",
    )
    _assert_equal(
        metrics.get("repeat_determinism", {}).get("pass_1_stream_sha256"),
        raw["stream_sha256"],
        "metrics stream digest",
    )
    _assert_equal(
        metrics.get("bytes"),
        {
            "runtime": runtime_manifest["runtime_bytes"],
            "raw_output": paths["raw_output"].stat().st_size,
            "source_packet": source_packet_path.stat().st_size,
            "metrics": paths["metrics"].stat().st_size,
        },
        "byte accounting",
    )
    _assert_equal(
        metrics.get("accuracy"),
        {
            "status": "unmeasured-no-german-competent-gold",
            "coverage_is_accuracy": False,
            "machine_agreement_is_gold": False,
        },
        "accuracy boundary",
    )
    _assert_equal(
        metrics.get("followup"),
        {
            "status": "blocked-not-materialized",
            "b_acquired": False,
            "c_acquired": False,
            "human_work_scheduled": False,
            "trigger": (
                "reviewed-a-census-residue-or-concrete-source-translation-"
                "sign-retrieval-question"
            ),
        },
        "follow-up boundary",
    )
    _assert_equal(resource_launch.get("ok"), True, "resource launch")
    _assert_equal(
        resource_launch.get("execution", {}).get("returncode"),
        0,
        "resource return code",
    )
    _assert_equal(
        resource_launch.get("startup_admission", {})
        .get("demand_observation", {})
        .get("execution_succeeded"),
        True,
        "resource execution",
    )
    resource_peaks = (
        resource_launch.get("startup_admission", {})
        .get("demand_observation", {})
        .get("peaks", {})
    )
    if resource_peaks.get("ok") is not True:
        raise MorphologyResultError("host resource peak evidence is unavailable")

    private_artifacts = {
        "run_receipt": file_record(paths["run_receipt"], source_bearing=False),
        "experiment_spec": file_record(paths["experiment_spec"], source_bearing=False),
        "preflight": file_record(paths["preflight"], source_bearing=False),
        "execution": file_record(paths["execution"], source_bearing=True),
        "repeat_determinism": file_record(
            paths["repeat_determinism"],
            source_bearing=False,
        ),
        "host_resource_launch": file_record(
            paths["host_resource_launch"],
            source_bearing=False,
        ),
        "metrics": file_record(paths["metrics"], source_bearing=False),
        "raw_output": file_record(paths["raw_output"], source_bearing=True),
    }
    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "morphology-census-result-receipt.schema.json"
        ),
        "schema_version": "tos_morphology_census_result_receipt_v1",
        "generated_or_authored": "generated_from_private_provider_census",
        "receipt_id": (
            "morphology-census-result:"
            "zarathustra-dta-first-editions-parts-1-4.a.dwdsmor-open-0.18.0.v1"
        ),
        "recorded_at_utc": run_receipt["finished_at_utc"],
        "status": "a-census-executed-awaiting-triggered-review",
        "experiment_id": EXPERIMENT_ID,
        "variant": VARIANT,
        "plan": {
            "plan_id": plan["plan_id"],
            "ref": PLAN_REF,
            "sha256": EXPECTED_PLAN_SHA256,
            "frozen_before_variant_outputs": True,
        },
        "generator": {
            "ref": GENERATOR_REF,
            "sha256": sha256_file(generator_path),
        },
        "private_run": {
            "artifact_owner": (
                "abyss-machine:storage/artifacts/tree-of-sophia-foundation-lab"
            ),
            "relative_ref": run_relative.as_posix(),
            "run_id": run_id,
            "status": run_receipt["status"],
            "started_at_utc": run_receipt["started_at_utc"],
            "finished_at_utc": run_receipt["finished_at_utc"],
            "retention_decision": "retain",
            "visibility": "owner-local-only",
            "manual_review_count": 0,
            "model_inspection_count": 0,
        },
        "provider": {
            "artifact": "DWDSmor Open",
            "edition": metrics["provider"]["edition"],
            "version": metrics["provider"]["version"],
            "source_commit": metrics["provider"]["source_commit"],
            "wheel_sha256": metrics["provider"]["wheel_sha256"],
            "execution_posture": ("proposal-only-preserve-all-analyses-and-unknowns"),
        },
        "source_input": {
            "work_ref": plan["source_lexical_index"]["work_ref"],
            "packet_ref": plan["a_census"]["local_packet"]["relative_path"],
            "packet_sha256": EXPECTED_INPUT_SHA256,
            "packet_bytes": source_packet_path.stat().st_size,
            "packet_mode": "0600",
            "exact_form_row_count": raw["row_count"],
            "token_occurrence_count": raw["token_occurrence_count"],
            "exact_surface_mutated": False,
            "source_text_accepted": False,
            "rights_cleared": False,
        },
        "runtime": {
            "runtime_id": runtime_manifest["runtime_id"],
            "manifest_sha256": execution["runtime_manifest_sha256"],
            "artifact_set_sha256": runtime_manifest["artifact_set_sha256"],
            "runtime_bytes": runtime_manifest["runtime_bytes"],
            "license": "GPL-2.0-only",
            "network_used": False,
        },
        "private_artifacts": private_artifacts,
        "coverage": {
            **coverage,
            "coverage_is_accuracy": False,
        },
        "distributions": {
            **expected_distributions,
            "lemma_analysis_total": raw["lemma_analysis_total"],
            "root_analysis_total": raw["root_analysis_total"],
        },
        "mechanical_unknown_residue": raw["unknown_residue"],
        "repeat_determinism": {
            "deterministic": True,
            "pass_1_stream_sha256": raw["stream_sha256"],
            "pass_2_stream_sha256": raw["stream_sha256"],
            "mismatch_count": 0,
            "second_pass_raw_output_retained": False,
        },
        "performance": {
            **metrics["performance"],
            "host_service_wall_seconds": float(
                str(
                    resource_launch["execution"]["systemd"]["service_runtime"]
                ).removesuffix("s")
            ),
            "host_service_cpu_seconds": float(
                str(
                    resource_launch["execution"]["systemd"]["cpu_time_consumed"]
                ).removesuffix("s")
            ),
            "host_cgroup_memory_peak_bytes": resource_peaks["memory_peak_bytes"],
            "host_cgroup_swap_peak_bytes": resource_peaks["memory_swap_peak_bytes"],
        },
        "accuracy": {
            "status": "unmeasured-no-german-competent-gold",
            "german_competent_gold_count": 0,
            "accepted_morphology_count": 0,
            "accepted_lemma_count": 0,
            "coverage_is_accuracy": False,
            "machine_agreement_is_gold": False,
        },
        "followup": {
            "status": "blocked-not-materialized",
            "b_acquired": False,
            "c_acquired": False,
            "human_work_scheduled": False,
            "mechanical_residue_is_reviewed_residue": False,
            "trigger": metrics["followup"]["trigger"],
        },
        "rights_and_visibility": {
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
            "creates_morphology": False,
            "creates_lemma": False,
            "creates_lexeme": False,
            "creates_sign_candidate": False,
            "creates_sign": False,
            "creates_semantic_claim": False,
            "creates_translation": False,
            "creates_graph_fact": False,
            "opens_human_backlog": False,
        },
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def validate_receipt(receipt: dict[str, Any]) -> None:
    schema = load_json(REPO_ROOT / SCHEMA_REF)
    errors = sorted(
        Draft202012Validator(schema).iter_errors(receipt),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        details = "; ".join(
            f"{'/'.join(map(str, error.absolute_path)) or '<root>'}: {error.message}"
            for error in errors
        )
        raise MorphologyResultError(f"result receipt schema failed: {details}")


def write_receipt(path: Path, receipt: dict[str, Any], *, check: bool) -> None:
    encoded = (
        json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=False) + "\n"
    ).encode("utf-8")
    if check:
        if not path.is_file() or path.read_bytes() != encoded:
            raise MorphologyResultError("tracked result receipt is stale")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_bytes(encoded)
    os.replace(temporary, path)


def parser() -> argparse.ArgumentParser:
    command = argparse.ArgumentParser(
        description=(
            "Verify one private DWDSmor A census and record only a text-free "
            "aggregate result receipt in Tree of Sophia."
        )
    )
    command.add_argument("--run-root", type=Path, required=True)
    command.add_argument(
        "--output",
        type=Path,
        default=REPO_ROOT / RESULT_REF,
    )
    command.add_argument("--check", action="store_true")
    return command


def main(argv: list[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
        receipt = verify_and_build_receipt(arguments.run_root)
        validate_receipt(receipt)
        write_receipt(arguments.output.resolve(), receipt, check=arguments.check)
    except MorphologyResultError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    print(
        json.dumps(
            {
                "status": receipt["status"],
                "run_id": receipt["private_run"]["run_id"],
                "covered_type_count": receipt["coverage"]["covered_type_count"],
                "unknown_type_count": receipt["coverage"]["unknown_type_count"],
                "accuracy": receipt["accuracy"]["status"],
                "followup": receipt["followup"]["status"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
