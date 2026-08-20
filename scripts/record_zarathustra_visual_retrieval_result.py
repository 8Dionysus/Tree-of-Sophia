#!/usr/bin/env python3
"""Record one text-free ToS receipt from a private direct-visual retrieval run."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import stat
import struct
import sys
from pathlib import Path
from typing import Any, Iterable

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
GOLD_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1"
)
PLAN_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "visual-retrieval-plan.v1.json"
)
RESULT_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "visual-retrieval-result.c-qwen3-vl-embedding-2b.v1.json"
)
SCHEMA_REF = "ToS/contracts/visual-retrieval-result-receipt.schema.json"
GENERATOR_REF = "scripts/record_zarathustra_visual_retrieval_result.py"
ARTIFACT_OWNER_ROOT = Path(
    "/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab"
)
EXPERIMENT_ID = "tos-visual-retrieval-foundation-v1"
VARIANT = "C"
MODEL_ID = "Qwen/Qwen3-VL-Embedding-2B"
MODEL_REVISION = "9f2f7e710d6d81056aa5c0a4f04764fec6bb7bda"
EXPECTED_DIGESTS = {
    "tree_plan": "27e142883d4385b8b69857ab7ebcbee7facb16454e54649ef6ed1df15e3b7c25",
    "source_sample_plan": "a813f0a19361ccc773d1427ab696a76296993c999abb2ff35376bae422bbb9d1",
    "visual_sample_plan": "3fd111023cf2ffdbee935d771f36f314d8087677d57e9ef34a64e19e27ddae06",
    "query_plan": "481e1f8ea32b9f7b40eb73a59e7bc3e614d08a3b8958bebd7b0c7fce94fed7a4",
    "query_content": "5c9ce96fb362512a9ef775bd7d7212b5df6b00570226961a96d51478ba980a1e",
    "render_manifest": "734e7474923ec2697a243b1e1b322c43c4e2a8b82b6336df57ab82f3bbc8fec7",
    "runtime_manifest": "ff730fe5780216fe84f26b7fbed6c57d0f0de58cabac5026d6af403e094632ba",
}
EXPECTED_VECTOR_COUNT = 76
EXPECTED_PAGE_COUNT = 36
EXPECTED_QUERY_COUNT = 20
EXPECTED_VECTOR_DIMENSION = 2048
PRE_NORMALIZATION_LIMIT = 0.02
POST_NORMALIZATION_LIMIT = 0.00001
AUTHORITY_BOUNDARY = (
    "This receipt proves one exact offline direct-page-image retrieval run, "
    "its frozen inputs, private artifact fixity, persisted normalization "
    "audit, source-anchor closure, measured resource cost, and a narrow "
    "trigger decision. It does not accept relevance, transcription, "
    "quotation, German or Russian text, translation, semantics, a sign, "
    "concept, claim, relation, graph edge, rights clearance, publication, "
    "method adoption, winner, or promotion."
)


class VisualRetrievalResultError(RuntimeError):
    """Raised when private visual-retrieval evidence does not close exactly."""


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
        raise VisualRetrievalResultError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise VisualRetrievalResultError(f"{path} must contain a JSON object")
    return payload


def canonical_bytes(payload: object) -> bytes:
    return (
        json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("utf-8")


def _equal(observed: object, expected: object, label: str) -> None:
    if observed != expected:
        raise VisualRetrievalResultError(
            f"{label} differs: observed={observed!r} expected={expected!r}"
        )


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise VisualRetrievalResultError(message)


def file_record(path: Path, *, source_bearing: bool) -> dict[str, Any]:
    mode = stat.S_IMODE(path.stat().st_mode)
    if mode not in {0o600, 0o644}:
        raise VisualRetrievalResultError(
            f"{path} has disallowed mode {mode:04o}"
        )
    return {
        "sha256": sha256_file(path),
        "bytes": path.stat().st_size,
        "mode": f"{mode:04o}",
        "source_bearing": source_bearing,
    }


def file_set_record(
    paths: Iterable[Path],
    *,
    root: Path,
    source_bearing: bool,
) -> dict[str, Any]:
    records = []
    total_bytes = 0
    for path in sorted(paths):
        size = path.stat().st_size
        total_bytes += size
        records.append(
            {
                "ref": path.relative_to(root).as_posix(),
                "sha256": sha256_file(path),
                "bytes": size,
            }
        )
    if not records:
        raise VisualRetrievalResultError("file set is empty")
    return {
        "file_count": len(records),
        "total_bytes": total_bytes,
        "set_sha256": hashlib.sha256(canonical_bytes(records)).hexdigest(),
        "source_bearing": source_bearing,
    }


def _owner_relative(path: Path) -> Path:
    try:
        relative = path.resolve().relative_to(ARTIFACT_OWNER_ROOT.resolve())
    except ValueError as exc:
        raise VisualRetrievalResultError(
            f"run root is outside the artifact owner: {path}"
        ) from exc
    if (
        len(relative.parts) != 3
        or relative.parts[0] != EXPERIMENT_ID
        or relative.parts[2] != "variant-C"
    ):
        raise VisualRetrievalResultError(
            f"run root has unexpected owner topology: {relative}"
        )
    return relative


def _source_to_visual_anchor_map(
    source_plan: dict[str, Any],
    visual_plan: dict[str, Any],
) -> tuple[dict[str, str], list[dict[str, Any]]]:
    source_by_id = {
        sample["sample_id"]: sample
        for group in source_plan["source_groups"]
        for sample in group["samples"]
    }
    visual_samples = [
        {
            **sample,
            "group_id": group["group_id"],
            "item_ref": group["item_ref"],
            "language": group["language"],
        }
        for group in visual_plan["source_groups"]
        for sample in group["samples"]
    ]
    mapping = {
        source_by_id[sample["source_sample_id"]]["anchor_ref"]: sample[
            "anchor_ref"
        ]
        for sample in visual_samples
    }
    _equal(len(source_by_id), EXPECTED_PAGE_COUNT, "source sample count")
    _equal(len(visual_samples), EXPECTED_PAGE_COUNT, "visual sample count")
    _equal(len(mapping), EXPECTED_PAGE_COUNT, "anchor crosswalk count")
    return mapping, visual_samples


def _inspect_pngs(
    render_manifest: dict[str, Any],
    input_manifest: dict[str, Any],
) -> dict[str, Any]:
    render_by_id = {
        record["sample_id"]: record for record in render_manifest["renders"]
    }
    input_by_id = {
        record["id"]: record for record in input_manifest["page_images"]
    }
    _equal(len(render_by_id), EXPECTED_PAGE_COUNT, "render count")
    _equal(len(input_by_id), EXPECTED_PAGE_COUNT, "input image count")
    records = []
    total_bytes = 0
    for sample_id in sorted(render_by_id):
        render = render_by_id[sample_id]
        observed = input_by_id.get(sample_id)
        if observed is None:
            raise VisualRetrievalResultError(
                f"render is absent from input manifest: {sample_id}"
            )
        path = Path(render["png_ref"])
        if not path.is_absolute():
            path = Path(render_manifest["artifact_root"]) / path
        encoded = path.read_bytes()
        _require(
            encoded[:8] == b"\x89PNG\r\n\x1a\n",
            f"invalid PNG signature: {sample_id}",
        )
        width, height = struct.unpack(">II", encoded[16:24])
        _equal(
            (width, height),
            (render["width_pixels"], render["height_pixels"]),
            f"PNG dimensions {sample_id}",
        )
        digest = hashlib.sha256(encoded).hexdigest()
        _equal(digest, render["png_sha256"], f"PNG digest {sample_id}")
        _equal(len(encoded), render["png_bytes"], f"PNG bytes {sample_id}")
        for field in (
            "png_sha256",
            "png_bytes",
            "page",
            "item_ref",
            "language",
        ):
            _equal(
                observed[field],
                render[field],
                f"render/input {sample_id}:{field}",
            )
        _equal(
            observed["visual_anchor_ref"],
            render["anchor_ref"],
            f"render/input visual anchor {sample_id}",
        )
        total_bytes += len(encoded)
        records.append(
            {
                "sample_id": sample_id,
                "sha256": digest,
                "bytes": len(encoded),
            }
        )
    _equal(
        total_bytes,
        render_manifest["total_png_bytes"],
        "render total PNG bytes",
    )
    return {
        "count": len(records),
        "total_bytes": total_bytes,
        "set_sha256": hashlib.sha256(canonical_bytes(records)).hexdigest(),
    }


def _inspect_controls(
    fixed_controls: dict[str, Any],
) -> dict[str, dict[str, dict[str, Any]]]:
    controls = {control["variant"]: control for control in fixed_controls["controls"]}
    _equal(set(controls), {"A", "B"}, "fixed control variants")
    _equal(fixed_controls["rerun_performed"], False, "control rerun posture")
    _equal(fixed_controls["source_text_copied"], False, "control text posture")
    output: dict[str, dict[str, dict[str, Any]]] = {}
    for variant, control in controls.items():
        root = Path(control["run_ref"])
        _equal(
            sha256_file(root / "run.receipt.json"),
            control["run_receipt_sha256"],
            f"control {variant} run receipt",
        )
        query_results: dict[str, dict[str, Any]] = {}
        for record in control["result_digests"]:
            path = (
                root
                / "raw-output/query-results"
                / f"{record['query_id']}.json"
            )
            _equal(
                sha256_file(path),
                record["sha256"],
                f"control {variant} result {record['query_id']}",
            )
            query_results[record["query_id"]] = load_json(path)
        _equal(
            len(query_results),
            EXPECTED_QUERY_COUNT,
            f"control {variant} result count",
        )
        output[variant] = query_results
    return output


def _inspect_index(
    index: dict[str, Any],
    input_manifest: dict[str, Any],
    lifecycle: dict[str, Any],
    index_path: Path,
) -> tuple[dict[str, dict[str, Any]], list[float]]:
    _equal(index["vector_dimension"], EXPECTED_VECTOR_DIMENSION, "vector dimension")
    _equal(index["normalization"], "L2", "vector normalization")
    _equal(
        index["distance"],
        "cosine-via-normalized-dot-product",
        "vector distance",
    )
    _equal(
        index["source_or_query_text_included"],
        False,
        "vector text posture",
    )
    input_by_id = {
        record["id"]: record for record in input_manifest["page_images"]
    }
    index_by_id = {
        record["visual_sample_id"]: record for record in index["images"]
    }
    _equal(len(index_by_id), EXPECTED_PAGE_COUNT, "unique indexed image count")
    norms = []
    for sample_id, record in index_by_id.items():
        vector = record["vector"]
        _equal(
            len(vector),
            EXPECTED_VECTOR_DIMENSION,
            f"vector dimension {sample_id}",
        )
        norm = math.sqrt(
            math.fsum(float(value) * float(value) for value in vector)
        )
        if abs(norm - 1.0) > POST_NORMALIZATION_LIMIT:
            raise VisualRetrievalResultError(
                f"serialized vector norm exceeds limit: {sample_id}"
            )
        norms.append(norm)
        expected = input_by_id[sample_id]
        for field in (
            "png_sha256",
            "source_anchor_ref",
            "visual_anchor_ref",
            "source_sample_id",
            "item_ref",
            "page",
            "language",
        ):
            _equal(
                record[field],
                expected[field],
                f"index/input {sample_id}:{field}",
            )
    index_digest = sha256_file(index_path)
    index_bytes = index_path.stat().st_size
    for stage in ("first_materialization", "rebuild"):
        _equal(
            lifecycle[stage]["sha256"],
            index_digest,
            f"index {stage} digest",
        )
        _equal(
            lifecycle[stage]["bytes"],
            index_bytes,
            f"index {stage} bytes",
        )
    _equal(lifecycle["digest_stable"], True, "index lifecycle stability")
    _equal(
        lifecycle["deletion_proof"]["absent_after_delete"],
        True,
        "index deletion proof",
    )
    return index_by_id, norms


def _inspect_runtime_and_model(
    runtime: dict[str, Any],
    invocation: dict[str, Any],
) -> dict[str, Any]:
    _equal(
        runtime["schema_version"],
        "tos_qwen_vl_runtime_and_model_v2",
        "runtime receipt schema",
    )
    _equal(
        runtime["network_used_during_inference"],
        False,
        "network posture",
    )
    _equal(
        runtime["unreviewed_remote_code_executed"],
        False,
        "remote code posture",
    )
    _equal(
        runtime["reviewed_upstream_helper_executed"],
        True,
        "reviewed helper posture",
    )
    _equal(
        invocation["runtime_manifest_sha256"],
        EXPECTED_DIGESTS["runtime_manifest"],
        "invocation runtime manifest",
    )
    host_cli = Path(invocation["argv"][0])
    _equal(
        host_cli.name,
        "tos_foundation_lab.py",
        "host laboratory CLI identity",
    )
    runner = host_cli.parent / "qwen_vl_visual_retrieval.py"
    bridge = host_cli.parent / "qwen_vl_embedding_bridge.py"
    helper = (
        Path(runtime["model"]["snapshot_path"])
        / runtime["model"]["helper_relative_path"]
    )
    _equal(
        sha256_file(runner),
        invocation["runner_sha256"],
        "retrieval runner digest",
    )
    _equal(
        sha256_file(bridge),
        invocation["bridge_sha256"],
        "embedding bridge digest",
    )
    _equal(
        sha256_file(helper),
        invocation["upstream_helper_sha256"],
        "reviewed helper digest",
    )
    artifact_records = []
    artifact_bytes = 0
    for artifact in runtime["model"]["artifacts"]:
        relative = (
            artifact.get("relative_path")
            or artifact.get("path")
            or artifact.get("name")
        )
        path = Path(runtime["model"]["snapshot_path"]) / relative
        _equal(path.stat().st_size, artifact["bytes"], f"model bytes {relative}")
        _equal(
            sha256_file(path),
            artifact["sha256"],
            f"model digest {relative}",
        )
        artifact_bytes += path.stat().st_size
        artifact_records.append(
            {
                "ref": relative,
                "sha256": artifact["sha256"],
                "bytes": artifact["bytes"],
            }
        )
    _equal(
        len(artifact_records),
        runtime["model"]["artifact_count"],
        "model artifact count",
    )
    _equal(
        artifact_bytes,
        runtime["model"]["artifact_bytes"],
        "model artifact bytes",
    )
    guard = runtime["bridge_offline_guard"]
    _equal(guard["active"], True, "offline guard active")
    _equal(guard["attempted_events"], [], "offline attempted events")
    _equal(
        guard["permitted_local_events"],
        [
            {
                "event": "socket.bind",
                "host": "::1",
                "port": 0,
                "scope": "loopback-ephemeral-capability-probe",
            }
        ],
        "offline permitted event",
    )
    return {
        "model_artifact_count": len(artifact_records),
        "model_artifact_bytes": artifact_bytes,
        "model_artifact_set_sha256": hashlib.sha256(
            canonical_bytes(sorted(artifact_records, key=lambda item: item["ref"]))
        ).hexdigest(),
        "runner_sha256": invocation["runner_sha256"],
        "bridge_sha256": invocation["bridge_sha256"],
        "reviewed_helper_sha256": invocation["upstream_helper_sha256"],
    }


def _inspect_normalization(
    runtime: dict[str, Any],
    input_manifest: dict[str, Any],
    query_ids: list[str],
    page_vector_norms: list[float],
) -> dict[str, float]:
    audit = runtime["bridge_normalization_audit"]
    records = audit["records"]
    roles = (
        ["page-image"] * EXPECTED_PAGE_COUNT
        + ["query-first"] * EXPECTED_QUERY_COUNT
        + ["query-warm"] * EXPECTED_QUERY_COUNT
    )
    ids = (
        [record["id"] for record in input_manifest["page_images"]]
        + query_ids
        + query_ids
    )
    _equal(len(records), EXPECTED_VECTOR_COUNT, "normalization record count")
    _equal(audit["record_count"], EXPECTED_VECTOR_COUNT, "normalization count")
    _equal(
        [record["ordinal"] for record in records],
        list(range(1, EXPECTED_VECTOR_COUNT + 1)),
        "normalization ordinals",
    )
    _equal(
        [record["role"] for record in records],
        roles,
        "normalization roles",
    )
    _equal([record["id"] for record in records], ids, "normalization IDs")
    expected_fields = {
        "ordinal",
        "role",
        "id",
        "pre_serialization_norm",
        "post_serialization_norm",
    }
    if any(set(record) != expected_fields for record in records):
        raise VisualRetrievalResultError(
            "normalization records contain unexpected fields"
        )
    pre = [float(record["pre_serialization_norm"]) for record in records]
    post = [float(record["post_serialization_norm"]) for record in records]
    summary = {
        "pre_serialization_norm_min": min(pre),
        "pre_serialization_norm_max": max(pre),
        "max_abs_pre_serialization_norm_error": max(
            abs(value - 1.0) for value in pre
        ),
        "post_serialization_norm_min": min(post),
        "post_serialization_norm_max": max(post),
        "max_abs_post_serialization_norm_error": max(
            abs(value - 1.0) for value in post
        ),
    }
    for field, expected in summary.items():
        _equal(audit[field], expected, f"normalization summary {field}")
    if (
        summary["max_abs_pre_serialization_norm_error"]
        > PRE_NORMALIZATION_LIMIT
    ):
        raise VisualRetrievalResultError("pre-normalization limit exceeded")
    if (
        summary["max_abs_post_serialization_norm_error"]
        > POST_NORMALIZATION_LIMIT
    ):
        raise VisualRetrievalResultError("post-normalization limit exceeded")
    for position, norm in enumerate(page_vector_norms):
        if abs(norm - post[position]) > 0.000001:
            raise VisualRetrievalResultError(
                "persisted page normalization differs from vector bytes"
            )
    return {
        "maximum_pre_normalization_error": summary[
            "max_abs_pre_serialization_norm_error"
        ],
        "maximum_post_normalization_error": summary[
            "max_abs_post_serialization_norm_error"
        ],
    }


def _control_result_anchors(
    variant: str,
    result: dict[str, Any],
) -> list[str]:
    if variant == "A":
        return [record["source_anchor_ref"] for record in result["results"]]
    return [
        record["payload"]["source_anchor_ref"]
        for record in result["reranked_results"]
    ]


def _inspect_queries(
    *,
    run_root: Path,
    prior_run_root: Path,
    query_plan: dict[str, Any],
    query_content: dict[str, Any],
    source_to_visual: dict[str, str],
    controls: dict[str, dict[str, dict[str, Any]]],
    index_by_id: dict[str, dict[str, Any]],
    input_manifest: dict[str, Any],
    comparison: dict[str, Any],
    metrics: dict[str, Any],
) -> dict[str, Any]:
    plan_by_id = {
        record["query_id"]: record for record in query_plan["queries"]
    }
    content_by_id = {
        record["query_id"]: record for record in query_content["queries"]
    }
    query_ids = list(content_by_id)
    _equal(len(query_ids), EXPECTED_QUERY_COUNT, "private query count")
    _equal(list(plan_by_id), query_ids, "query identity and order")
    comparison_by_id = {
        record["query_id"]: record for record in comparison["queries"]
    }
    _equal(len(comparison_by_id), EXPECTED_QUERY_COUNT, "comparison query count")
    input_visual_anchors = {
        record["visual_anchor_ref"]
        for record in input_manifest["page_images"]
    }
    input_source_anchors = {
        record["source_anchor_ref"]
        for record in input_manifest["page_images"]
    }
    stable = 0
    evaluable = 0
    expected_hits = 0
    coverage_recovery = 0
    hard_slots = 0
    hard_presence = 0
    hard_outranks = []
    cross_queries = 0
    cross_hits = 0
    resolved = 0
    r8_r9_identical = 0
    trigger_reasons: dict[str, set[str]] = {}
    query_paths = []
    for query_id in query_ids:
        path = run_root / "raw-output/query-results" / f"{query_id}.json"
        query_paths.append(path)
        result = load_json(path)
        plan = plan_by_id[query_id]
        query_text = content_by_id[query_id]["text"]
        _equal(
            result["query_text_sha256"],
            hashlib.sha256(query_text.encode("utf-8")).hexdigest(),
            f"query text digest {query_id}",
        )
        expected_source = list(plan["expected_source_anchor_refs"])
        hard_source = list(plan["hard_negative_anchor_refs"])
        expected_visual = [
            source_to_visual[anchor] for anchor in expected_source
        ]
        hard_visual = [source_to_visual[anchor] for anchor in hard_source]
        _equal(
            result["model_proposed_expected_source_anchor_refs"],
            expected_source,
            f"expected source anchors {query_id}",
        )
        _equal(
            result["model_proposed_expected_visual_anchor_refs"],
            expected_visual,
            f"expected visual anchors {query_id}",
        )
        _equal(
            result["model_proposed_hard_negative_source_anchor_refs"],
            hard_source,
            f"hard-negative source anchors {query_id}",
        )
        _equal(
            result["model_proposed_hard_negative_visual_anchor_refs"],
            hard_visual,
            f"hard-negative visual anchors {query_id}",
        )
        ranked = result["results"]
        _equal(len(ranked), 10, f"ranked result count {query_id}")
        _equal(
            [record["rank"] for record in ranked],
            list(range(1, 11)),
            f"rank order {query_id}",
        )
        visual_anchors = [
            record["visual_anchor_ref"] for record in ranked
        ]
        _equal(
            len(set(visual_anchors)),
            10,
            f"unique visual anchors {query_id}",
        )
        _equal(
            result["result_visual_anchor_refs"],
            visual_anchors,
            f"first ranking projection {query_id}",
        )
        _equal(
            result["warm_result_visual_anchor_refs"],
            visual_anchors,
            f"warm ranking projection {query_id}",
        )
        _equal(
            result["ranking_stable_across_repeat"],
            True,
            f"repeat ranking {query_id}",
        )
        stable += 1
        scores = [float(record["score"]) for record in ranked]
        if any(scores[index] < scores[index + 1] for index in range(9)):
            raise VisualRetrievalResultError(
                f"result scores are not descending: {query_id}"
            )
        for record in ranked:
            _require(
                record["visual_anchor_ref"] in input_visual_anchors,
                f"unresolved visual anchor: {query_id}",
            )
            _require(
                record["source_anchor_ref"] in input_source_anchors,
                f"unresolved source anchor: {query_id}",
            )
            _equal(
                source_to_visual[record["source_anchor_ref"]],
                record["visual_anchor_ref"],
                f"source/visual crosswalk {query_id}",
            )
            _equal(
                index_by_id[record["visual_sample_id"]]["visual_anchor_ref"],
                record["visual_anchor_ref"],
                f"result/index anchor {query_id}",
            )
        resolved += len(ranked)
        expected_ranks = {
            anchor: visual_anchors.index(anchor) + 1
            for anchor in expected_visual
            if anchor in visual_anchors
        }
        _equal(
            result["model_proposed_expected_visual_ranks"],
            expected_ranks,
            f"expected ranks {query_id}",
        )
        if plan["expected_behavior"] != "coverage-failure":
            evaluable += 1
            expected_hits += bool(expected_ranks)
        hard_ranks = [
            visual_anchors.index(anchor) + 1
            for anchor in hard_visual
            if anchor in visual_anchors
        ]
        hard_slots += len(hard_visual)
        hard_presence += len(hard_ranks)
        if expected_ranks and hard_ranks and min(hard_ranks) < min(
            expected_ranks.values()
        ):
            hard_outranks.append(query_id)
            trigger_reasons.setdefault(query_id, set()).add(
                "hard-negative-outranks-model-proposed-expected"
            )
        if plan["category"] == "cross-lingual":
            cross_queries += 1
            cross_hits += bool(expected_ranks)
            trigger_reasons.setdefault(query_id, set()).add(
                "cross-language-behavior-remains-human-unjudged"
            )
        a_anchors = _control_result_anchors(
            "A", controls["A"][query_id]
        )
        b_anchors = _control_result_anchors(
            "B", controls["B"][query_id]
        )
        a_hit = any(anchor in a_anchors for anchor in expected_source)
        b_hit = any(anchor in b_anchors for anchor in expected_source)
        c_hit = bool(expected_ranks)
        if expected_visual and c_hit and not a_hit and not b_hit:
            coverage_recovery += 1
            trigger_reasons.setdefault(query_id, set()).add(
                "challenger-materially-changes-source-return-route"
            )
        row = comparison_by_id[query_id]
        expected_row = {
            "A_model_proposed_target_present": a_hit,
            "A_source_anchor_refs": a_anchors,
            "B_model_proposed_target_present": b_hit,
            "B_source_anchor_refs": b_anchors,
            "C_model_proposed_target_present": c_hit,
            "C_source_anchor_refs": [
                record["source_anchor_ref"] for record in ranked
            ],
            "C_visual_anchor_refs": visual_anchors,
            "model_proposed_expected_source_anchor_refs": expected_source,
            "model_proposed_expected_visual_anchor_refs": expected_visual,
        }
        for field, expected in expected_row.items():
            _equal(
                row[field],
                expected,
                f"comparison {query_id}:{field}",
            )
        prior = load_json(
            prior_run_root
            / "raw-output/query-results"
            / f"{query_id}.json"
        )
        prior_scores = [
            float(record["score"]) for record in prior["results"]
        ]
        if (
            prior["result_visual_anchor_refs"] == visual_anchors
            and prior_scores == scores
        ):
            r8_r9_identical += 1
    _equal(stable, EXPECTED_QUERY_COUNT, "stable ranking total")
    _equal(resolved, 200, "resolved ranking total")
    _equal(
        metrics["repeat_ranking_stability"]["stable_queries"],
        stable,
        "stable ranking metric",
    )
    _equal(
        metrics["model_proposed_target_at_10"],
        {
            "present": expected_hits,
            "evaluable_queries": evaluable,
            "status": "advisory-nonhuman-not-a-quality-score",
        },
        "target metric",
    )
    _equal(
        metrics["coverage_recovery_advisory"][
            "C_hit_where_A_and_B_missed"
        ],
        coverage_recovery,
        "coverage recovery metric",
    )
    _equal(
        metrics["model_proposed_hard_negative_presence"]["present"],
        hard_presence,
        "hard-negative presence metric",
    )
    _equal(
        metrics["model_proposed_hard_negative_presence"]["declared_slots"],
        hard_slots,
        "hard-negative slot metric",
    )
    _equal(
        metrics["cross_lingual"]["queries"],
        cross_queries,
        "cross-language query metric",
    )
    _equal(
        metrics["cross_lingual"]["model_proposed_expected_hits"],
        cross_hits,
        "cross-language hit metric",
    )
    _equal(
        metrics["source_anchor_resolution"]["resolved_ranked_results"],
        resolved,
        "resolved ranking metric",
    )
    _equal(r8_r9_identical, EXPECTED_QUERY_COUNT, "r8/r9 ranking parity")
    expected_trigger_ids = [
        "tos-query-003",
        "tos-query-009",
        "tos-query-010",
        "tos-query-011",
        "tos-query-020",
    ]
    _equal(
        [query_id for query_id in query_ids if query_id in trigger_reasons],
        expected_trigger_ids,
        "trigger query set",
    )
    return {
        "query_paths": query_paths,
        "stable": stable,
        "evaluable": evaluable,
        "expected_hits": expected_hits,
        "coverage_recovery": coverage_recovery,
        "hard_slots": hard_slots,
        "hard_presence": hard_presence,
        "hard_outranks": hard_outranks,
        "cross_queries": cross_queries,
        "cross_hits": cross_hits,
        "resolved": resolved,
        "r8_r9_identical": r8_r9_identical,
        "trigger_query_ids": expected_trigger_ids,
    }


def _inspect_content_withholding(
    run_root: Path,
    query_content: dict[str, Any],
) -> tuple[int, int]:
    blobs = [
        path.read_bytes()
        for path in run_root.rglob("*")
        if path.is_file() and path.suffix.lower() in {".json", ".jsonl"}
    ]
    leaks = 0
    for query in query_content["queries"]:
        for field in ("text", "fts5_query"):
            value = query.get(field)
            if (
                isinstance(value, str)
                and value
                and any(value.encode("utf-8") in blob for blob in blobs)
            ):
                leaks += 1
    copied_binaries = [
        path
        for path in run_root.rglob("*")
        if path.is_file()
        and path.suffix.lower()
        in {".png", ".jpg", ".jpeg", ".pdf", ".epub", ".tif", ".tiff"}
    ]
    _equal(leaks, 0, "private query text leaks")
    _equal(len(copied_binaries), 0, "copied source/image binaries")
    return leaks, len(copied_binaries)


def build_receipt(
    *,
    run_root: Path,
    prior_run_root: Path,
    local_query_content: Path,
    generator_path: Path,
) -> dict[str, Any]:
    run_relative = _owner_relative(run_root)
    prior_relative = _owner_relative(prior_run_root)
    paths = {
        "run_receipt": run_root / "run.receipt.json",
        "experiment_spec": run_root / "experiment.spec.json",
        "preflight": run_root / "receipts/preflight.json",
        "input_manifest": run_root / "inputs/visual-retrieval-input-manifest.json",
        "fixed_controls": run_root / "inputs/fixed-controls.json",
        "vector_index": run_root / "derived-index/qwen3-vl-page-vectors.json",
        "abc_comparison": run_root / "raw-output/abc-anchor-comparison.json",
        "index_lifecycle": run_root / "receipts/qwen-vl-index-lifecycle.json",
        "invocation": run_root / "receipts/qwen-vl-invocation.json",
        "runtime_and_model": run_root / "receipts/qwen-vl-runtime-and-model.json",
        "metrics": run_root / "metrics/qwen-vl-visual-retrieval-summary.json",
        "host_resource_launch": (
            run_root / "receipts/abyss-machine-resource-launch.json"
        ),
    }
    for label, path in paths.items():
        if not path.is_file():
            raise VisualRetrievalResultError(
                f"private artifact is missing: {label}: {path}"
            )
    plan_path = REPO_ROOT / PLAN_REF
    source_sample_path = GOLD_ROOT / "sample-plan.json"
    visual_sample_path = GOLD_ROOT / "ocr-visual-samples.json"
    query_plan_path = GOLD_ROOT / "retrieval-queries.json"
    plan = load_json(plan_path)
    source_sample_plan = load_json(source_sample_path)
    visual_sample_plan = load_json(visual_sample_path)
    query_plan = load_json(query_plan_path)
    query_content = load_json(local_query_content)
    for label, path in {
        "tree_plan": plan_path,
        "source_sample_plan": source_sample_path,
        "visual_sample_plan": visual_sample_path,
        "query_plan": query_plan_path,
        "query_content": local_query_content,
    }.items():
        _equal(
            sha256_file(path),
            EXPECTED_DIGESTS[label],
            f"frozen {label} digest",
        )
    run_receipt = load_json(paths["run_receipt"])
    input_manifest = load_json(paths["input_manifest"])
    fixed_controls = load_json(paths["fixed_controls"])
    index = load_json(paths["vector_index"])
    comparison = load_json(paths["abc_comparison"])
    lifecycle = load_json(paths["index_lifecycle"])
    invocation = load_json(paths["invocation"])
    runtime = load_json(paths["runtime_and_model"])
    metrics = load_json(paths["metrics"])
    resource_launch = load_json(paths["host_resource_launch"])
    prior_run_receipt = load_json(prior_run_root / "run.receipt.json")
    prior_runtime = load_json(
        prior_run_root / "receipts/qwen-vl-runtime-and-model.json"
    )
    _equal(run_receipt["experiment_id"], EXPERIMENT_ID, "run experiment")
    _equal(run_receipt["variant"], VARIANT, "run variant")
    _equal(
        run_receipt["status"],
        "awaiting-triggered-review",
        "run status",
    )
    _equal(run_receipt["errors"], [], "run errors")
    _equal(run_receipt["retention_decision"], "pending", "run retention")
    _equal(
        prior_runtime["schema_version"],
        "tos_qwen_vl_runtime_and_model_v1",
        "prior audit-incomplete schema",
    )
    _require(
        "bridge_normalization_audit" not in prior_runtime,
        "prior run unexpectedly contains the persisted normalization audit",
    )
    for field, label in (
        ("tree_plan", "tree_plan"),
        ("source_sample_plan", "source_sample_plan"),
        ("visual_sample_plan", "visual_sample_plan"),
        ("query_plan", "query_plan"),
        ("query_content", "query_content"),
    ):
        _equal(
            input_manifest[field]["sha256"],
            EXPECTED_DIGESTS[label],
            f"input manifest {field}",
        )
    render_manifest_path = Path(input_manifest["render_manifest"]["ref"])
    _equal(
        sha256_file(render_manifest_path),
        EXPECTED_DIGESTS["render_manifest"],
        "render manifest digest",
    )
    render_manifest = load_json(render_manifest_path)
    _equal(
        input_manifest["render_manifest"]["sha256"],
        EXPECTED_DIGESTS["render_manifest"],
        "input render manifest digest",
    )
    _equal(
        input_manifest["source_or_query_content_copied"],
        False,
        "input content posture",
    )
    source_to_visual, _visual_samples = _source_to_visual_anchor_map(
        source_sample_plan,
        visual_sample_plan,
    )
    pngs = _inspect_pngs(render_manifest, input_manifest)
    controls = _inspect_controls(fixed_controls)
    index_by_id, page_norms = _inspect_index(
        index,
        input_manifest,
        lifecycle,
        paths["vector_index"],
    )
    model = _inspect_runtime_and_model(runtime, invocation)
    query_ids = [record["query_id"] for record in query_content["queries"]]
    normalization = _inspect_normalization(
        runtime,
        input_manifest,
        query_ids,
        page_norms,
    )
    query_evidence = _inspect_queries(
        run_root=run_root,
        prior_run_root=prior_run_root,
        query_plan=query_plan,
        query_content=query_content,
        source_to_visual=source_to_visual,
        controls=controls,
        index_by_id=index_by_id,
        input_manifest=input_manifest,
        comparison=comparison,
        metrics=metrics,
    )
    private_text_leaks, copied_source_files = _inspect_content_withholding(
        run_root,
        query_content,
    )
    _equal(comparison["promotion_authorized"], False, "comparison promotion")
    _equal(comparison["winner"], None, "comparison winner")
    _equal(comparison["human_relevance_status"], "not_started", "human status")
    _equal(metrics["promotion_authorized"], False, "metrics promotion")
    _equal(metrics["winner"], None, "metrics winner")
    _equal(
        metrics["quality"],
        {
            "human_ndcg_at_10": None,
            "human_hard_negative_error_rate": None,
            "reason": "no declared human review trigger has opened",
        },
        "human quality posture",
    )
    _equal(resource_launch["ok"], True, "host launch")
    _equal(resource_launch["dry_run"], False, "host launch dry-run posture")
    _equal(resource_launch["blocked_reasons"], [], "host blocked reasons")
    _equal(resource_launch["denied_reasons"], [], "host denied reasons")
    _equal(resource_launch["request"]["force"], False, "host force posture")
    _equal(
        resource_launch["request"]["memory_demand_mib"],
        12288.0,
        "host memory demand",
    )
    _equal(
        resource_launch["request"]["demand_key"],
        "tos-qwen-vl-visual-c",
        "host demand key",
    )
    _equal(
        resource_launch["request"]["command"][2],
        str(run_root),
        "host run root",
    )
    _equal(
        resource_launch["execution"]["returncode"],
        0,
        "host return code",
    )
    _equal(
        resource_launch["execution"]["systemd"]["result"],
        "success",
        "host service result",
    )
    peaks = (
        resource_launch["startup_admission"]["demand_observation"]["peaks"]
    )
    _equal(peaks["ok"], True, "host peak evidence")
    private_artifacts = {
        label: file_record(path, source_bearing=False)
        for label, path in paths.items()
    }
    private_artifacts["query_results"] = file_set_record(
        query_evidence["query_paths"],
        root=run_root,
        source_bearing=False,
    )
    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "visual-retrieval-result-receipt.schema.json"
        ),
        "schema_version": "tos_visual_retrieval_result_receipt_v1",
        "generated_or_authored": (
            "generated_from_private_direct_visual_retrieval_run"
        ),
        "receipt_id": (
            "visual-retrieval-result:"
            "zarathustra-foundation-pilot-v1.c.qwen3-vl-embedding-2b.v1"
        ),
        "recorded_at_utc": run_receipt["finished_at_utc"],
        "status": "c-executed-triggered-review-open-unscheduled",
        "experiment_id": EXPERIMENT_ID,
        "variant": VARIANT,
        "plan": {
            "plan_id": plan["plan_id"],
            "ref": PLAN_REF,
            "sha256": EXPECTED_DIGESTS["tree_plan"],
            "frozen_before_challenger_outputs": True,
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
            "run_id": run_receipt["run_id"],
            "status": run_receipt["status"],
            "started_at_utc": run_receipt["started_at_utc"],
            "finished_at_utc": run_receipt["finished_at_utc"],
            "retention_decision": "retain",
            "visibility": "owner-local-only",
            "manual_review_count": 0,
            "model_inspection_count": 1,
        },
        "prior_audit_incomplete_run": {
            "relative_ref": prior_relative.as_posix(),
            "run_id": prior_run_receipt["run_id"],
            "classification": "executed-audit-incomplete",
            "run_receipt_sha256": sha256_file(
                prior_run_root / "run.receipt.json"
            ),
            "missing_evidence": "persisted-normalization-audit",
            "rankings_and_scores_identical_query_count": query_evidence[
                "r8_r9_identical"
            ],
            "query_count": EXPECTED_QUERY_COUNT,
            "retained": True,
            "promotion_authorized": False,
        },
        "method": {
            "model_id": MODEL_ID,
            "repository_revision": MODEL_REVISION,
            "vector_dimension": EXPECTED_VECTOR_DIMENSION,
            "distance": "cosine-via-normalized-dot-product",
            "normalization": "L2",
            "runtime_route": "isolated-offline-python-3.12-cpu",
            "network_used": False,
            "unreviewed_remote_code_executed": False,
        },
        "inputs": {
            "work_ref": plan["work_ref"],
            "query_count": EXPECTED_QUERY_COUNT,
            "page_image_count": EXPECTED_PAGE_COUNT,
            "tree_plan_sha256": EXPECTED_DIGESTS["tree_plan"],
            "source_sample_plan_sha256": EXPECTED_DIGESTS[
                "source_sample_plan"
            ],
            "visual_sample_plan_sha256": EXPECTED_DIGESTS[
                "visual_sample_plan"
            ],
            "query_plan_sha256": EXPECTED_DIGESTS["query_plan"],
            "query_content_sha256": EXPECTED_DIGESTS["query_content"],
            "render_manifest_sha256": EXPECTED_DIGESTS["render_manifest"],
            "png_set_sha256": pngs["set_sha256"],
            "png_total_bytes": pngs["total_bytes"],
            "source_or_query_content_copied": False,
        },
        "runtime": {
            "runtime_id": runtime["runtime"]["runtime_id"],
            "manifest_sha256": EXPECTED_DIGESTS["runtime_manifest"],
            "artifact_set_sha256": runtime["runtime"]["artifact_set_sha256"],
            **model,
            "normalization_audit_schema": runtime["schema_version"],
            "normalization_record_count": EXPECTED_VECTOR_COUNT,
            "offline_attempted_event_count": len(
                runtime["bridge_offline_guard"]["attempted_events"]
            ),
            "permitted_loopback_bind_count": len(
                runtime["bridge_offline_guard"]["permitted_local_events"]
            ),
        },
        "private_artifacts": private_artifacts,
        "mechanical_reconstruction": {
            "status": "completed-without-unresolved-mechanical-issue",
            "performed_by": "model:codex-independent-reconstruction",
            "frozen_digest_checks": 7,
            "png_fixity_checks": EXPECTED_PAGE_COUNT,
            "control_fixity_checks": 42,
            "model_artifact_fixity_checks": model["model_artifact_count"],
            "page_vector_count": EXPECTED_PAGE_COUNT,
            "vector_dimension": EXPECTED_VECTOR_DIMENSION,
            "normalization_record_count": EXPECTED_VECTOR_COUNT,
            "resolved_ranked_result_count": query_evidence["resolved"],
            "private_text_leak_count": private_text_leaks,
            "copied_source_or_image_file_count": copied_source_files,
            "r8_r9_identical_rankings_and_scores": query_evidence[
                "r8_r9_identical"
            ],
            **normalization,
            "query_vector_recomputation": (
                "not-possible-query-vectors-not-persisted"
            ),
        },
        "advisory_results": {
            "stable_rankings": query_evidence["stable"],
            "query_count": EXPECTED_QUERY_COUNT,
            "model_proposed_target_at_10": query_evidence["expected_hits"],
            "evaluable_query_count": query_evidence["evaluable"],
            "coverage_recovery": query_evidence["coverage_recovery"],
            "hard_negative_presence": query_evidence["hard_presence"],
            "hard_negative_slots": query_evidence["hard_slots"],
            "hard_negative_outranks_expected": len(
                query_evidence["hard_outranks"]
            ),
            "cross_language_expected_hits": query_evidence["cross_hits"],
            "cross_language_query_count": query_evidence["cross_queries"],
            "resolved_ranked_results": query_evidence["resolved"],
            "advisory_only": True,
        },
        "performance": {
            "model_load_seconds": metrics["model_load_seconds"],
            "image_encoding_seconds_total": metrics[
                "image_encoding_seconds_total"
            ],
            "first_query_latency_ms_median": metrics[
                "first_query_end_to_end_latency_ms_median"
            ],
            "warm_query_latency_ms_median": metrics[
                "warm_query_end_to_end_latency_ms_median"
            ],
            "total_runner_seconds": metrics["total_runner_seconds"],
            "host_elapsed_seconds": resource_launch["elapsed_sec"],
            "index_bytes": metrics["index_bytes"],
            "packet_bytes_before_host_receipt": metrics["packet_bytes"],
            "bridge_peak_rss_bytes": metrics["bridge_peak_rss_bytes"],
            "host_runner_peak_rss_bytes": metrics[
                "host_runner_peak_rss_bytes"
            ],
            "host_cgroup_memory_peak_bytes": peaks["memory_peak_bytes"],
            "host_cgroup_swap_peak_bytes": peaks[
                "memory_swap_peak_bytes"
            ],
            "host_cgroup_footprint_peak_mib": peaks["footprint_peak_mib"],
        },
        "triggered_review": {
            "status": "open-unscheduled",
            "routine_review_scheduled": False,
            "review_opened": True,
            "human_judgment_status": "not_started",
            "human_debt_count": 0,
            "trigger_conditions_met": [
                "challenger-materially-changes-a-source-return-route",
                (
                    "hard-negative-or-cross-language-behavior-"
                    "remains-ambiguous"
                ),
            ],
            "query_ids": query_evidence["trigger_query_ids"],
            "query_count": len(query_evidence["trigger_query_ids"]),
            "review_scope": (
                "criteria-only-source-visible-ranking-review-no-retyping"
            ),
            "review_interface_materialized": False,
        },
        "quality_and_promotion": {
            "human_ndcg_at_10": None,
            "human_hard_negative_error_rate": None,
            "human_review_minutes": None,
            "winner": None,
            "method_adoption_under_consideration": False,
            "promotion_authorized": False,
        },
        "rights_and_visibility": {
            "private_source_and_query_content": "owner-local-only",
            "tracked_receipt_contains_source_strings": False,
            "tracked_receipt_contains_query_strings": False,
            "tracked_receipt_contains_vectors": False,
            "public_page_images_authorized": False,
            "publication_authorized": False,
        },
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def validate_receipt(payload: dict[str, Any]) -> None:
    schema = load_json(REPO_ROOT / SCHEMA_REF)
    errors = sorted(
        Draft202012Validator(schema).iter_errors(payload),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        rendered = "; ".join(
            f"{'/'.join(str(part) for part in error.absolute_path) or '<root>'}: "
            f"{error.message}"
            for error in errors
        )
        raise VisualRetrievalResultError(
            f"result receipt does not satisfy its schema: {rendered}"
        )
    encoded = json.dumps(payload, ensure_ascii=False)
    for forbidden in (
        "сверхчеловек смысл земли",
        "Uebermensch ist der Sinn der Erde",
        "государство",
        "Sinn der Erde",
    ):
        if forbidden in encoded:
            raise VisualRetrievalResultError(
                "tracked receipt contains a private query string"
            )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-root", type=Path, required=True)
    parser.add_argument("--prior-run-root", type=Path, required=True)
    parser.add_argument("--local-query-content", type=Path, required=True)
    parser.add_argument(
        "--output",
        type=Path,
        default=REPO_ROOT / RESULT_REF,
    )
    parser.add_argument("--check", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(list(argv or sys.argv[1:]))
    try:
        receipt = build_receipt(
            run_root=args.run_root,
            prior_run_root=args.prior_run_root,
            local_query_content=args.local_query_content,
            generator_path=Path(__file__).resolve(),
        )
        validate_receipt(receipt)
        encoded = json.dumps(receipt, ensure_ascii=False, indent=2) + "\n"
        if args.check:
            if not args.output.is_file():
                raise VisualRetrievalResultError(
                    f"tracked receipt is missing: {args.output}"
                )
            if args.output.read_text(encoding="utf-8") != encoded:
                raise VisualRetrievalResultError(
                    f"tracked receipt differs from private evidence: {args.output}"
                )
            print(f"[ok] visual retrieval result matches {args.output}")
            return 0
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
        os.chmod(args.output, 0o644)
        print(f"[ok] wrote {args.output}")
        print(
            "[boundary] result is mechanical and trigger-scoped; "
            "relevance and promotion remain human-unjudged"
        )
        return 0
    except VisualRetrievalResultError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
