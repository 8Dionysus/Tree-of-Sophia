#!/usr/bin/env python3
"""Bind one selected exact-form hash to its complete raw-witness recurrence.

The source-bearing occurrence census is written only below an explicit local
owner root. Tracked outputs retain hashes, aggregate distribution, fixity and
the no-promotion boundary, but no source string or occurrence position.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import sqlite3
import subprocess
import tempfile
from collections import defaultdict
from fractions import Fraction
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PLAN = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "semantic-source-recurrence-plan.v1.json"
)
GENERATOR_REF = "scripts/build_semantic_source_recurrence_bundle.py"
PROVENANCE_EVENT_ID = (
    "tos.event.annotation."
    "zarathustra-semantic-source-recurrence-v1.2026-08-10"
)
AUTHORITY_BOUNDARY = (
    "complete private raw-witness return for one previously selected exact-form "
    "hash plus tracked aggregate recurrence evidence; no accepted German, "
    "morphology, lemma, sense, motif, philosophical importance, translation, "
    "sign candidate, human task, semantic claim, relation, graph, canon, "
    "transfer, promotion, or publication authority"
)
ELEMENT_STEP = re.compile(r"^([A-Za-z_][A-Za-z0-9_.:-]*)(\[[1-9][0-9]*\])?$")


class RecurrenceBuildError(RuntimeError):
    """Raised when source recurrence cannot be closed exactly."""


def load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise RecurrenceBuildError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise RecurrenceBuildError(f"{path} must contain a JSON object")
    return payload


def canonical_json_bytes(payload: object) -> bytes:
    return (
        json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("utf-8")


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as exc:
        raise RecurrenceBuildError(f"cannot hash {path}: {exc}") from exc
    return digest.hexdigest()


def resolve_under(root: Path, relative: str) -> Path:
    resolved_root = root.resolve()
    candidate = (resolved_root / relative).resolve()
    try:
        candidate.relative_to(resolved_root)
    except ValueError as exc:
        raise RecurrenceBuildError(
            f"path escapes explicit root: {relative}"
        ) from exc
    return candidate


def verify_ref_digest(ref: str, expected: str, label: str) -> Path:
    path = resolve_under(REPO_ROOT, ref)
    actual = sha256_file(path)
    if actual != expected:
        raise RecurrenceBuildError(
            f"{label} digest drift: observed {actual}, expected {expected}"
        )
    return path


def _atomic_write(path: Path, payload: bytes, *, mode: int | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary_path, 0o644 if mode is None else mode)
        os.replace(temporary_path, path)
    finally:
        temporary_path.unlink(missing_ok=True)


def tei_path_to_xpath(path: str) -> str:
    steps: list[str] = []
    for index, step in enumerate(path.split("/")):
        if step.startswith("text()"):
            match = re.fullmatch(r"text\(\)(\[[1-9][0-9]*\])?", step)
            if match is None:
                raise RecurrenceBuildError(f"unsupported text-node step: {step}")
            steps.append("/text()" + (match.group(1) or ""))
            continue
        if step.startswith("tail()"):
            match = re.fullmatch(r"tail\(\)(\[[1-9][0-9]*\])?", step)
            if match is None:
                raise RecurrenceBuildError(f"unsupported tail-node step: {step}")
            steps.append(
                "/following-sibling::text()" + (match.group(1) or "[1]")
            )
            continue
        match = ELEMENT_STEP.fullmatch(step)
        if match is None:
            raise RecurrenceBuildError(f"unsupported TEI path step: {step}")
        name, position = match.groups()
        prefix = "" if index == 0 else "/"
        steps.append(
            f"{prefix}*[local-name()='{name}']{position or ''}"
        )
    return "/" + "".join(steps)


def xmllint_text_node(path: Path, tei_path: str) -> str:
    xpath = tei_path_to_xpath(tei_path)
    try:
        result = subprocess.run(
            ["xmllint", "--nonet", "--xpath", f"string({xpath})", str(path)],
            check=True,
            capture_output=True,
        )
    except FileNotFoundError as exc:
        raise RecurrenceBuildError("xmllint is not installed") from exc
    except subprocess.CalledProcessError as exc:
        detail = exc.stderr.decode("utf-8", errors="replace").strip()
        raise RecurrenceBuildError(
            f"xmllint source return failed for {tei_path}: {detail}"
        ) from exc
    return result.stdout.decode("utf-8")


def xmllint_libxml_version() -> str:
    try:
        result = subprocess.run(
            ["xmllint", "--version"],
            check=True,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError as exc:
        raise RecurrenceBuildError("xmllint is not installed") from exc
    except subprocess.CalledProcessError as exc:
        raise RecurrenceBuildError("cannot determine xmllint libxml version") from exc
    match = re.search(
        r"using libxml version ([0-9]+)",
        result.stdout + result.stderr,
    )
    if match is None:
        raise RecurrenceBuildError("cannot parse xmllint libxml version")
    encoded = int(match.group(1))
    return f"{encoded // 10000}.{(encoded // 100) % 100}.{encoded % 100}"


def find_item_manifests(plan: dict[str, Any]) -> dict[str, tuple[Path, dict[str, Any]]]:
    expected = {row["item_ref"] for row in plan["source_items"]}
    found: dict[str, tuple[Path, dict[str, Any]]] = {}
    root = (
        REPO_ROOT
        / "ToS/source-witnesses/works/friedrich-nietzsche/"
        "also-sprach-zarathustra/expressions"
    )
    for path in root.rglob("item.manifest.json"):
        payload = load_json(path)
        item_ref = payload.get("item_id")
        if item_ref not in expected:
            continue
        if item_ref in found:
            raise RecurrenceBuildError(f"duplicate Item manifest: {item_ref}")
        found[item_ref] = (path, payload)
    missing = expected - set(found)
    if missing:
        raise RecurrenceBuildError(
            "missing Item manifests: " + ", ".join(sorted(missing))
        )
    return found


def verify_tracked_inputs(plan: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    selected = plan["selected_source_observation"]
    recurrence = plan["recurrence_input"]
    verify_ref_digest(
        selected["initial_plan_ref"],
        selected["initial_plan_sha256"],
        "initial source-observation plan",
    )
    verify_ref_digest(
        selected["packet_ref"], selected["packet_sha256"], "semantic packet"
    )
    verify_ref_digest(
        recurrence["recurrence_plan_ref"],
        recurrence["recurrence_plan_sha256"],
        "recurrence plan",
    )
    projection_path = verify_ref_digest(
        recurrence["recurrence_projection_ref"],
        recurrence["recurrence_projection_sha256"],
        "recurrence projection",
    )
    verify_ref_digest(
        recurrence["lexical_projection_ref"],
        recurrence["lexical_projection_sha256"],
        "lexical projection",
    )
    projection = load_json(projection_path)
    matches = [
        row
        for row in projection.get("rows", [])
        if isinstance(row, dict)
        and row.get("exact_form_sha256") == selected["exact_form_sha256"]
    ]
    if len(matches) != 1:
        raise RecurrenceBuildError(
            "selected exact-form hash must resolve to exactly one recurrence row"
        )
    row = matches[0]
    if row.get("form_key") != selected["form_key"]:
        raise RecurrenceBuildError("selected recurrence form key drift")
    observed = {
        field: row.get(field)
        for field in plan["expected_tracked_recurrence_tuple"]
    }
    if observed != plan["expected_tracked_recurrence_tuple"]:
        raise RecurrenceBuildError(f"tracked recurrence tuple drift: {observed!r}")
    return projection, row


def _part_dp_millionths(
    part_token_counts: list[int], occurrence_counts: list[int]
) -> int:
    token_total = sum(part_token_counts)
    occurrence_total = sum(occurrence_counts)
    divergence = sum(
        abs(
            Fraction(part_tokens, token_total)
            - Fraction(part_occurrences, occurrence_total)
        )
        for part_tokens, part_occurrences in zip(
            part_token_counts, occurrence_counts, strict=True
        )
    ) / 2
    return round(divergence * 1_000_000)


def build_private_bundle(
    *,
    plan: dict[str, Any],
    plan_path: Path,
    local_input_root: Path,
    database_path: Path,
    database_sha256: str,
    recurrence_projection: dict[str, Any],
    recurrence_row: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any]]:
    selected = plan["selected_source_observation"]
    manifests = find_item_manifests(plan)
    expected_items = {
        row["item_ref"]: row for row in plan["source_items"]
    }
    source_paths: dict[str, Path] = {}
    source_bindings: list[dict[str, Any]] = []
    for item_ref, expected in sorted(
        expected_items.items(), key=lambda pair: pair[1]["part_order"]
    ):
        manifest_path, manifest = manifests[item_ref]
        files = manifest.get("payload_files", [])
        matches = [
            row
            for row in files
            if isinstance(row, dict)
            and row.get("sha256") == expected["file_sha256"]
        ]
        if len(matches) != 1:
            raise RecurrenceBuildError(
                f"Item payload does not resolve exactly once: {item_ref}"
            )
        payload_ref = (
            manifest_path.parent.relative_to(REPO_ROOT) / matches[0]["relative_path"]
        ).as_posix()
        payload_path = resolve_under(local_input_root, payload_ref)
        if sha256_file(payload_path) != expected["file_sha256"]:
            raise RecurrenceBuildError(f"source payload fixity drift: {item_ref}")
        source_paths[item_ref] = payload_path
        source_bindings.append(
            {
                "part_order": expected["part_order"],
                "item_ref": item_ref,
                "manifest_ref": manifest_path.relative_to(REPO_ROOT).as_posix(),
                "manifest_sha256": sha256_file(manifest_path),
                "payload_ref": payload_ref,
                "payload_sha256": expected["file_sha256"],
            }
        )

    try:
        connection = sqlite3.connect(
            f"file:{database_path.as_posix()}?mode=ro&immutable=1", uri=True
        )
    except sqlite3.Error as exc:
        raise RecurrenceBuildError(
            f"cannot open lexical database read-only: {exc}"
        ) from exc
    connection.row_factory = sqlite3.Row
    try:
        quick_check = connection.execute("PRAGMA quick_check").fetchone()
        if quick_check is None or quick_check[0] != "ok":
            raise RecurrenceBuildError("lexical database quick_check failed")
        form = connection.execute(
            """
            SELECT form_key, exact_form, normalized_form, exact_form_sha256,
                   normalized_form_sha256, occurrence_count
            FROM forms WHERE exact_form_sha256 = ?
            """,
            (selected["exact_form_sha256"],),
        ).fetchone()
        if form is None:
            raise RecurrenceBuildError("selected exact form is absent from database")
        if (
            form["form_key"] != selected["form_key"]
            or sha256_bytes(form["exact_form"].encode("utf-8"))
            != selected["exact_form_sha256"]
            or form["occurrence_count"]
            != plan["expected_tracked_recurrence_tuple"]["occurrence_count"]
        ):
            raise RecurrenceBuildError("selected database form identity drift")
        occurrences = connection.execute(
            """
            SELECT o.occurrence_id, o.item_ref, s.part_order,
                   s.file_sha256 AS source_file_sha256, o.token_ordinal,
                   o.form_key, o.exact_form, o.exact_form_sha256,
                   o.page_resource_id, o.section_resource_id,
                   o.text_node_path, o.start_offset, o.end_offset,
                   o.editorial_status
            FROM occurrences AS o
            JOIN source_items AS s ON s.item_ref = o.item_ref
            WHERE o.exact_form_sha256 = ?
            ORDER BY s.part_order, o.token_ordinal, o.occurrence_id
            """,
            (selected["exact_form_sha256"],),
        ).fetchall()
    except sqlite3.Error as exc:
        raise RecurrenceBuildError(f"cannot query lexical database: {exc}") from exc
    finally:
        connection.close()

    if len(occurrences) != form["occurrence_count"]:
        raise RecurrenceBuildError("complete occurrence census does not close")

    node_cache: dict[tuple[str, str], str] = {}
    private_occurrences: list[dict[str, Any]] = []
    part_counts: dict[tuple[int, str], int] = defaultdict(int)
    part_pages: dict[tuple[int, str], set[str]] = defaultdict(set)
    part_sections: dict[tuple[int, str], set[str]] = defaultdict(set)
    page_refs: set[tuple[str, str]] = set()
    section_refs: set[tuple[str, str]] = set()
    editorial_count = 0
    unsectioned_count = 0
    for occurrence in occurrences:
        item_ref = occurrence["item_ref"]
        expected_item = expected_items.get(item_ref)
        if (
            expected_item is None
            or occurrence["source_file_sha256"] != expected_item["file_sha256"]
            or occurrence["form_key"] != selected["form_key"]
            or occurrence["exact_form"] != form["exact_form"]
        ):
            raise RecurrenceBuildError("occurrence identity or source fixity drift")
        cache_key = (item_ref, occurrence["text_node_path"])
        if cache_key not in node_cache:
            node_cache[cache_key] = xmllint_text_node(
                source_paths[item_ref], occurrence["text_node_path"]
            )
        node_value = node_cache[cache_key]
        start = occurrence["start_offset"]
        end = occurrence["end_offset"]
        returned = node_value[start:end]
        if (
            returned != form["exact_form"]
            or sha256_bytes(returned.encode("utf-8"))
            != selected["exact_form_sha256"]
        ):
            raise RecurrenceBuildError(
                f"raw TEI offset return drift: {occurrence['occurrence_id']}"
            )
        key = (occurrence["part_order"], item_ref)
        part_counts[key] += 1
        part_pages[key].add(occurrence["page_resource_id"])
        page_refs.add((item_ref, occurrence["page_resource_id"]))
        if occurrence["section_resource_id"] is None:
            unsectioned_count += 1
        else:
            part_sections[key].add(occurrence["section_resource_id"])
            section_refs.add((item_ref, occurrence["section_resource_id"]))
        if occurrence["editorial_status"] != "witness-text":
            editorial_count += 1
        private_occurrences.append(
            {
                "occurrence_id": occurrence["occurrence_id"],
                "item_ref": item_ref,
                "part_order": occurrence["part_order"],
                "source_file_sha256": occurrence["source_file_sha256"],
                "token_ordinal": occurrence["token_ordinal"],
                "page_resource_id": occurrence["page_resource_id"],
                "section_resource_id": occurrence["section_resource_id"],
                "text_node_path": occurrence["text_node_path"],
                "start_offset": start,
                "end_offset": end,
                "editorial_status": occurrence["editorial_status"],
                "text_node_sha256": sha256_bytes(node_value.encode("utf-8")),
                "raw_return_sha256": sha256_bytes(returned.encode("utf-8")),
                "raw_return_verified": True,
            }
        )

    parts = [
        {
            "part_order": part_order,
            "item_ref": item_ref,
            "occurrence_count": part_counts[(part_order, item_ref)],
            "page_count": len(part_pages[(part_order, item_ref)]),
            "section_count": len(part_sections[(part_order, item_ref)]),
        }
        for part_order, item_ref in sorted(part_counts)
    ]
    token_counts = {
        row["item_ref"]: row["token_count"]
        for row in recurrence_projection["segmentation_totals"]["parts"]
    }
    part_token_counts = [token_counts[row["item_ref"]] for row in parts]
    occurrence_counts = [row["occurrence_count"] for row in parts]
    observed_tuple = {
        "occurrence_count": len(private_occurrences),
        "part_range": len(parts),
        "section_range": len(section_refs),
        "page_range": len(page_refs),
        "part_dp_millionths": _part_dp_millionths(
            part_token_counts, occurrence_counts
        ),
        "maximum_part_share_millionths": round(
            Fraction(max(occurrence_counts), len(private_occurrences)) * 1_000_000
        ),
        "source_editorial_occurrence_count": editorial_count,
        "unsectioned_occurrence_count": unsectioned_count,
    }
    if observed_tuple != plan["expected_tracked_recurrence_tuple"]:
        raise RecurrenceBuildError(
            f"independent raw-witness recurrence tuple drift: {observed_tuple!r}"
        )
    if observed_tuple != {
        key: recurrence_row[key]
        for key in plan["expected_tracked_recurrence_tuple"]
    }:
        raise RecurrenceBuildError("raw-witness tuple differs from tracked row")

    bundle = {
        "schema_version": "tos_semantic_source_recurrence_private_bundle_v1",
        "bundle_id": "semantic-source-recurrence:zarathustra-initial-selected-form-v1",
        "plan_ref": plan_path.relative_to(REPO_ROOT).as_posix(),
        "plan_sha256": sha256_file(plan_path),
        "selected_form": {
            "form_key": form["form_key"],
            "exact_form": form["exact_form"],
            "normalized_form": form["normalized_form"],
            "exact_form_sha256": form["exact_form_sha256"],
            "normalized_form_sha256": form["normalized_form_sha256"],
        },
        "source_database_sha256": database_sha256,
        "source_bindings": source_bindings,
        "recurrence_row": recurrence_row,
        "recurrence_row_sha256": sha256_bytes(canonical_json_bytes(recurrence_row)),
        "observed_tuple": observed_tuple,
        "parts": parts,
        "raw_text_node_return_count": len(node_cache),
        "raw_offset_return_count": len(private_occurrences),
        "occurrences": private_occurrences,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    aggregates = {
        "observed_tuple": observed_tuple,
        "parts": parts,
        "source_bindings": source_bindings,
        "recurrence_row_sha256": bundle["recurrence_row_sha256"],
        "raw_text_node_return_count": len(node_cache),
        "raw_offset_return_count": len(private_occurrences),
    }
    return bundle, aggregates


def build_receipt(
    *,
    plan: dict[str, Any],
    plan_path: Path,
    database_path: Path,
    database_sha256: str,
    private_bytes: bytes,
    aggregates: dict[str, Any],
) -> dict[str, Any]:
    selected = plan["selected_source_observation"]
    recurrence = plan["recurrence_input"]
    return {
        "schema_version": "tos_semantic_source_recurrence_receipt_v1",
        "receipt_id": "semantic-source-recurrence-receipt:zarathustra-initial-selected-form-v1",
        "status": "completed-source-observation-no-promotion",
        "plan": {
            "ref": plan_path.relative_to(REPO_ROOT).as_posix(),
            "sha256": sha256_file(plan_path),
        },
        "generator": {
            "ref": GENERATOR_REF,
            "sha256": sha256_file(REPO_ROOT / GENERATOR_REF),
        },
        "selected_source_observation": {
            "exact_form_sha256": selected["exact_form_sha256"],
            "form_key": selected["form_key"],
            "packet_ref": selected["packet_ref"],
            "packet_sha256": selected["packet_sha256"],
            "selection_reopened": False,
            "source_value_tracked": False,
        },
        "recurrence_sources": {
            "recurrence_plan_ref": recurrence["recurrence_plan_ref"],
            "recurrence_plan_sha256": recurrence["recurrence_plan_sha256"],
            "recurrence_projection_ref": recurrence["recurrence_projection_ref"],
            "recurrence_projection_sha256": recurrence[
                "recurrence_projection_sha256"
            ],
            "recurrence_row_sha256": aggregates["recurrence_row_sha256"],
            "local_database_ref": recurrence["local_database_ref"],
            "local_database_sha256": database_sha256,
            "local_database_bytes": database_path.stat().st_size,
        },
        "local_bundle": {
            "ref": plan["local_output"]["ref"],
            "sha256": sha256_bytes(private_bytes),
            "bytes": len(private_bytes),
            "mode": "0600",
            "occurrence_count": aggregates["raw_offset_return_count"],
            "source_values_local_only": True,
            "occurrence_positions_local_only": True,
        },
        "observed_tuple": aggregates["observed_tuple"],
        "parts": aggregates["parts"],
        "source_bindings": [
            {
                key: row[key]
                for key in (
                    "part_order",
                    "item_ref",
                    "manifest_ref",
                    "manifest_sha256",
                    "payload_sha256",
                )
            }
            for row in aggregates["source_bindings"]
        ],
        "verification": {
            "complete_occurrence_census": True,
            "raw_text_node_return_count": aggregates[
                "raw_text_node_return_count"
            ],
            "raw_offset_return_count": aggregates["raw_offset_return_count"],
            "raw_offset_return_match_count": aggregates[
                "raw_offset_return_count"
            ],
            "source_payload_fixity_match_count": len(
                aggregates["source_bindings"]
            ),
            "tracked_recurrence_tuple_match": True,
            "independent_part_size_aware_recalculation": True,
            "xmllint_nonet": True,
        },
        "content_exposure": {
            "local_exact_strings": True,
            "local_occurrence_positions": True,
            "tracked_exact_strings": False,
            "tracked_occurrence_positions": False,
            "tracked_form_hashes": True,
            "dictionary_recovery_possible": True,
            "confidentiality_claimed": False,
        },
        "packet_effect": {
            "packet_ref": selected["packet_ref"],
            "packet_changed": False,
            "ladder_stage_changed": False,
            "human_work_scheduled": False,
            "promotion_authorized": False,
        },
        "authority_boundary": plan["authority_boundary"],
        "provenance_event_ref": PROVENANCE_EVENT_ID,
    }


def build_provenance(
    *,
    plan: dict[str, Any],
    plan_path: Path,
    receipt_path: Path,
    receipt_bytes: bytes,
    private_bytes: bytes,
    aggregates: dict[str, Any],
) -> dict[str, Any]:
    selected = plan["selected_source_observation"]
    recurrence = plan["recurrence_input"]
    execution = plan["execution"]
    return {
        "schema_version": "tos_provenance_event_v1",
        "event_id": PROVENANCE_EVENT_ID,
        "event_type": "annotation",
        "started_at": execution["started_at"],
        "ended_at": execution["ended_at"],
        "agent_refs": [
            "software:python-" + execution["python_version"],
            "software:sqlite3-" + execution["sqlite_version"],
            "software:libxml2-xmllint-" + execution["xmllint_libxml_version"],
        ],
        "inputs": [
            {
                "ref": plan_path.relative_to(REPO_ROOT).as_posix(),
                "role": "frozen-selected-form-source-recurrence-plan",
                "sha256": sha256_file(plan_path),
            },
            {
                "ref": selected["packet_ref"],
                "role": "unchanged-observational-semantic-ladder-packet",
                "sha256": selected["packet_sha256"],
            },
            {
                "ref": recurrence["recurrence_projection_ref"],
                "role": "tracked-hash-only-four-part-recurrence-projection",
                "sha256": recurrence["recurrence_projection_sha256"],
            },
            {
                "ref": recurrence["local_database_ref"],
                "role": "ignored-local-source-bearing-lexical-database",
                "sha256": recurrence["local_database_sha256"],
            },
        ],
        "outputs": [
            {
                "ref": plan["local_output"]["ref"],
                "role": "ignored-private-complete-raw-witness-recurrence-bundle",
                "sha256": sha256_bytes(private_bytes),
            },
            {
                "ref": receipt_path.relative_to(REPO_ROOT).as_posix(),
                "role": "tracked-source-withholding-recurrence-receipt",
                "sha256": sha256_bytes(receipt_bytes),
            },
        ],
        "method": {
            "maker_type": "software",
            "name": "complete-exact-form-raw-witness-recurrence-return",
            "version": "1",
            "artifact_digest": sha256_file(REPO_ROOT / GENERATOR_REF),
            "runtime": (
                f"Python {execution['python_version']}; "
                f"SQLite {execution['sqlite_version']}; "
                f"libxml2 xmllint {execution['xmllint_libxml_version']}"
            ),
            "device": "abyss-machine-cpu",
            "configuration": {
                "selection_reopened": False,
                "selected_exact_form_sha256": selected["exact_form_sha256"],
                "complete_occurrence_census": True,
                "occurrence_count": aggregates["observed_tuple"][
                    "occurrence_count"
                ],
                "source_item_count": len(aggregates["parts"]),
                "raw_offset_returns_verified": aggregates[
                    "raw_offset_return_count"
                ],
                "tracked_source_values": False,
                "tracked_occurrence_positions": False,
                "packet_changed": False,
                "human_work_scheduled": False,
                "automatic_promotion_authorized": False,
                "publication_authorized": False,
            },
            "prompt_or_instruction_ref": plan_path.relative_to(
                REPO_ROOT
            ).as_posix(),
        },
        "status": "completed_with_warnings",
        "warnings": [
            "the complete exact form and all occurrence positions remain in an ignored mode-0600 local bundle",
            "form hashes are navigational fingerprints and do not provide confidentiality",
            "mechanical recurrence across four source Items does not establish one lemma, sense, motif, sign, or philosophical importance",
            "the existing semantic packet and every language, human, semantic, graph, canon, transfer, and publication gate remain unchanged",
        ],
        "receipt_refs": [
            receipt_path.relative_to(REPO_ROOT).as_posix(),
            plan_path.relative_to(REPO_ROOT).as_posix(),
        ],
        "rights_basis_ref": None,
        "event_version": 1,
        "supersedes_event_ref": None,
    }


def materialize(
    *,
    plan_path: Path,
    local_input_root: Path,
    local_output_root: Path,
    check: bool,
) -> dict[str, Any]:
    plan_path = plan_path.resolve()
    try:
        plan_path.relative_to(REPO_ROOT.resolve())
    except ValueError as exc:
        raise RecurrenceBuildError("plan must remain inside Tree repository") from exc
    plan = load_json(plan_path)
    if (
        plan.get("schema_version") != "tos_semantic_source_recurrence_plan_v1"
        or plan.get("status") != "frozen-before-output"
        or plan.get("authority_boundary", {}).get("source_observation_only")
        is not True
        or plan.get("authority_boundary", {}).get(
            "packet_stage_change_authorized"
        )
        is not False
    ):
        raise RecurrenceBuildError("recurrence plan boundary is malformed")
    execution = plan["execution"]
    if (
        execution["python_version"] != platform.python_version()
        or execution["sqlite_version"] != sqlite3.sqlite_version
        or execution["xmllint_libxml_version"] != xmllint_libxml_version()
    ):
        raise RecurrenceBuildError("pinned Python, SQLite, or libxml runtime drift")
    recurrence_projection, recurrence_row = verify_tracked_inputs(plan)
    recurrence = plan["recurrence_input"]
    database_path = resolve_under(local_input_root, recurrence["local_database_ref"])
    database_sha256 = sha256_file(database_path)
    if database_sha256 != recurrence["local_database_sha256"]:
        raise RecurrenceBuildError("local lexical database digest drift")
    bundle, aggregates = build_private_bundle(
        plan=plan,
        plan_path=plan_path,
        local_input_root=local_input_root,
        database_path=database_path,
        database_sha256=database_sha256,
        recurrence_projection=recurrence_projection,
        recurrence_row=recurrence_row,
    )
    private_bytes = canonical_json_bytes(bundle)
    receipt = build_receipt(
        plan=plan,
        plan_path=plan_path,
        database_path=database_path,
        database_sha256=database_sha256,
        private_bytes=private_bytes,
        aggregates=aggregates,
    )
    receipt_bytes = canonical_json_bytes(receipt)
    receipt_path = resolve_under(
        REPO_ROOT, plan["tracked_outputs"]["receipt_ref"]
    )
    provenance = build_provenance(
        plan=plan,
        plan_path=plan_path,
        receipt_path=receipt_path,
        receipt_bytes=receipt_bytes,
        private_bytes=private_bytes,
        aggregates=aggregates,
    )
    provenance_bytes = canonical_json_bytes(provenance)
    provenance_path = resolve_under(
        REPO_ROOT, plan["tracked_outputs"]["provenance_ref"]
    )
    private_path = resolve_under(local_output_root, plan["local_output"]["ref"])

    tracked_serialization = receipt_bytes + provenance_bytes
    forbidden_keys = (
        b'"exact_form"',
        b'"normalized_form"',
        b'"occurrence_id"',
        b'"text_node_path"',
        b'"token_ordinal"',
        b'"start_offset"',
        b'"end_offset"',
    )
    if any(key in tracked_serialization for key in forbidden_keys):
        raise RecurrenceBuildError(
            "tracked recurrence outputs expose source values or occurrence positions"
        )

    if check:
        if not private_path.is_file() or private_path.read_bytes() != private_bytes:
            raise RecurrenceBuildError("private recurrence bundle is missing or stale")
        if (private_path.stat().st_mode & 0o777) != 0o600:
            raise RecurrenceBuildError("private recurrence bundle mode must be 0600")
        if not receipt_path.is_file() or receipt_path.read_bytes() != receipt_bytes:
            raise RecurrenceBuildError("tracked recurrence receipt is missing or stale")
        if (
            not provenance_path.is_file()
            or provenance_path.read_bytes() != provenance_bytes
        ):
            raise RecurrenceBuildError(
                "tracked recurrence provenance is missing or stale"
            )
        return receipt

    _atomic_write(private_path, private_bytes, mode=0o600)
    _atomic_write(receipt_path, receipt_bytes)
    _atomic_write(provenance_path, provenance_bytes)
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, default=REPO_ROOT / DEFAULT_PLAN)
    parser.add_argument("--local-input-root", type=Path, required=True)
    parser.add_argument("--local-output-root", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        plan_path = args.plan.resolve()
        plan = load_json(plan_path)
        receipt = materialize(
            plan_path=plan_path,
            local_input_root=args.local_input_root,
            local_output_root=args.local_output_root,
            check=args.check,
        )
        receipt_path = resolve_under(
            REPO_ROOT, plan["tracked_outputs"]["receipt_ref"]
        )
    except (RecurrenceBuildError, OSError) as exc:
        parser.error(str(exc))
    action = "verified" if args.check else "materialized"
    print(
        f"{action} {receipt['observed_tuple']['occurrence_count']} "
        "private raw-witness recurrence returns; "
        f"tracked receipt sha256={sha256_file(receipt_path)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
