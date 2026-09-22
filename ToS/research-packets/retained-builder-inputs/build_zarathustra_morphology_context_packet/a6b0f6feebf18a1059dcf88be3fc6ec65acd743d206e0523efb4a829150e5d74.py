#!/usr/bin/env python3
"""Freeze one output-blind private context packet for morphology B.

The builder verifies one exact ambiguity in the retained DWDSmor A stream,
selects first/median/last occurrences from the already complete recurrence
bundle, and returns each selected occurrence to fixity-bound raw TEI character
data. Exact context and positions stay in an ignored mode-0600 JSONL packet.
The tracked receipt and provenance retain only digests, counts, rank roles,
variant state, and explicit non-authority.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import tempfile
from collections import Counter
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator
from lxml import etree


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PLAN = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "morphology-contextual-episode.selected-form-b.v1.json"
)
PLAN_SCHEMA = Path("ToS/contracts/morphology-contextual-episode-plan.schema.json")
ROW_SCHEMA = Path("ToS/contracts/morphology-contextual-episode-row.schema.json")
RECEIPT_SCHEMA = Path(
    "ToS/contracts/morphology-contextual-episode-receipt.schema.json"
)
PROVENANCE_SCHEMA = Path("ToS/contracts/provenance-event.schema.json")
GENERATOR_REF = "scripts/build_zarathustra_morphology_context_packet.py"
PROVENANCE_EVENT_ID = (
    "tos.event.annotation.zarathustra-morphology-context-b-v1.2026-08-10"
)
AUTHORITY_BOUNDARY = (
    "one output-blind private raw-TEI context packet for a concrete "
    "machine-only B disambiguation proposal; no accepted German, sentence, "
    "tokenization, morphology, lemma, lexeme, normalization, sign, "
    "translation, semantic claim, graph fact, canon effect, public route, or "
    "human backlog"
)
ELEMENT_STEP = re.compile(r"^([A-Za-z_][A-Za-z0-9_.:-]*)(\[[1-9][0-9]*\])?$")
TEXT_STEP = re.compile(r"^(text|tail)\(\)(\[[1-9][0-9]*\])?$")
RANK_ROLES = {1: "first", 73: "inclusive-median", 145: "last"}


class MorphologyContextError(RuntimeError):
    """Raised when the contextual packet cannot close exactly."""


def load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise MorphologyContextError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise MorphologyContextError(f"{path} must contain a JSON object")
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
        raise MorphologyContextError(f"cannot hash {path}: {exc}") from exc
    return digest.hexdigest()


def validate_schema(payload: object, schema_path: Path, label: str) -> None:
    schema = load_json(schema_path)
    errors = sorted(
        Draft202012Validator(schema).iter_errors(payload),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        details = "; ".join(
            f"{'/'.join(map(str, error.absolute_path)) or '<root>'}: {error.message}"
            for error in errors[:8]
        )
        raise MorphologyContextError(f"{label} schema failed: {details}")


def resolve_under(root: Path, relative: str) -> Path:
    resolved_root = root.resolve()
    candidate = (resolved_root / relative).resolve()
    try:
        candidate.relative_to(resolved_root)
    except ValueError as exc:
        raise MorphologyContextError(f"path escapes explicit root: {relative}") from exc
    return candidate


def verify_repo_ref(binding: dict[str, Any], label: str) -> Path:
    path = resolve_under(REPO_ROOT, binding["ref"])
    actual = sha256_file(path)
    if actual != binding["sha256"]:
        raise MorphologyContextError(
            f"{label} digest drift: observed {actual}, expected {binding['sha256']}"
        )
    return path


def verify_a_trigger(plan: dict[str, Any], raw_output_path: Path) -> dict[str, Any]:
    trigger = plan["a_trigger"]
    verify_repo_ref(trigger["result_receipt"], "A result receipt")
    expected_raw = trigger["private_raw_output"]
    if raw_output_path.stat().st_size != expected_raw["bytes"]:
        raise MorphologyContextError("private A raw-output byte-size drift")
    if sha256_file(raw_output_path) != expected_raw["sha256"]:
        raise MorphologyContextError("private A raw-output digest drift")

    matches: list[tuple[bytes, dict[str, Any]]] = []
    try:
        with raw_output_path.open("rb") as handle:
            for line_number, line in enumerate(handle, start=1):
                if not line.strip():
                    continue
                try:
                    row = json.loads(line)
                except json.JSONDecodeError as exc:
                    raise MorphologyContextError(
                        f"invalid A row {line_number}: {exc}"
                    ) from exc
                if row.get("exact_form_sha256") == trigger["exact_form_sha256"]:
                    matches.append((line, row))
    except OSError as exc:
        raise MorphologyContextError(f"cannot inspect private A output: {exc}") from exc
    if len(matches) != 1:
        raise MorphologyContextError("selected A row must resolve exactly once")
    raw_line, row = matches[0]
    if sha256_bytes(raw_line) != trigger["selected_row_sha256"]:
        raise MorphologyContextError("selected A row digest drift")
    if row.get("form_key") != trigger["form_key"]:
        raise MorphologyContextError("selected A form key drift")
    if row.get("normalized_form_sha256") != trigger["normalized_form_sha256"]:
        raise MorphologyContextError("selected A normalized-form hash drift")
    if row.get("input_preserved") is not True or row.get("unknown") is not False:
        raise MorphologyContextError("selected A preservation/unknown state drift")
    analyses = row.get("lemma_analyses")
    if not isinstance(analyses, list):
        raise MorphologyContextError("selected A lemma analyses are absent")
    categories = sorted({analysis.get("pos") for analysis in analyses})
    separable_count = sum(analysis.get("syninfo") == "SEP" for analysis in analyses)
    if (
        len(analyses) != trigger["lemma_analysis_count"]
        or categories != trigger["provider_pos_categories"]
        or separable_count != trigger["separable_candidate_count"]
    ):
        raise MorphologyContextError("selected A ambiguity shape drift")
    return {
        "result_receipt": trigger["result_receipt"],
        "private_raw_output_sha256": expected_raw["sha256"],
        "selected_row_sha256": trigger["selected_row_sha256"],
        "selected_row_match_count": 1,
        "input_preserved": True,
        "lemma_analysis_count": len(analyses),
        "provider_pos_categories": categories,
        "separable_candidate_count": separable_count,
    }


def element_path_to_xpath(path: str) -> str:
    steps: list[str] = []
    for index, step in enumerate(path.split("/")):
        match = ELEMENT_STEP.fullmatch(step)
        if match is None:
            raise MorphologyContextError(f"unsupported TEI element step: {step}")
        name, position = match.groups()
        prefix = "" if index == 0 else "/"
        steps.append(f"{prefix}*[local-name()='{name}']{position or ''}")
    return "/" + "".join(steps)


def target_owner(path: str) -> tuple[str, str]:
    steps = path.split("/")
    if len(steps) < 2:
        raise MorphologyContextError("TEI text-node path is too short")
    match = TEXT_STEP.fullmatch(steps[-1])
    if match is None or (match.group(2) not in (None, "[1]")):
        raise MorphologyContextError("unsupported TEI text/tail step")
    return "/".join(steps[:-1]), match.group(1)


def context_unit(path: str) -> tuple[str, str]:
    steps = path.split("/")
    paragraph_indexes = [
        index for index, step in enumerate(steps) if step.startswith("p[")
    ]
    if paragraph_indexes:
        index = paragraph_indexes[-1]
        return "/".join(steps[: index + 1]), "paragraph"
    verse_indexes = [
        index for index, step in enumerate(steps) if step.startswith("lg[")
    ]
    if verse_indexes:
        index = verse_indexes[-1]
        return "/".join(steps[: index + 1]), "verse-group"
    raise MorphologyContextError("selected occurrence lacks supported p/lg context")


def one_element(tree: etree._ElementTree, path: str, label: str) -> etree._Element:
    matches = tree.xpath(element_path_to_xpath(path))
    if len(matches) != 1 or not isinstance(matches[0], etree._Element):
        raise MorphologyContextError(f"{label} must resolve to one TEI element")
    return matches[0]


def flatten_with_target_base(
    context: etree._Element,
    owner: etree._Element,
    owner_kind: str,
) -> tuple[str, int]:
    chunks: list[str] = []
    codepoints = 0
    target_base: int | None = None

    def append(value: str | None, element: etree._Element, kind: str) -> None:
        nonlocal codepoints, target_base
        if element is owner and kind == owner_kind:
            if target_base is not None:
                raise MorphologyContextError("target node appears more than once")
            target_base = codepoints
        if value:
            chunks.append(value)
            codepoints += len(value)

    def walk(element: etree._Element) -> None:
        append(element.text, element, "text")
        for child in element:
            if isinstance(child.tag, str):
                walk(child)
            append(child.tail, child, "tail")

    walk(context)
    if target_base is None:
        raise MorphologyContextError("target text node is outside selected context")
    return "".join(chunks), target_base


def parse_tei(path: Path) -> etree._ElementTree:
    parser = etree.XMLParser(
        resolve_entities=False,
        no_network=True,
        recover=False,
        remove_blank_text=False,
        strip_cdata=False,
        huge_tree=False,
    )
    try:
        return etree.parse(str(path), parser)
    except (OSError, etree.XMLSyntaxError) as exc:
        raise MorphologyContextError(f"cannot parse TEI {path}: {exc}") from exc


def build_context_rows(
    *,
    plan: dict[str, Any],
    local_input_root: Path,
) -> tuple[list[dict[str, Any]], dict[str, Any], list[dict[str, str]]]:
    recurrence = plan["source_recurrence"]
    recurrence_path = resolve_under(local_input_root, recurrence["local_bundle"]["ref"])
    local_binding = recurrence["local_bundle"]
    if recurrence_path.stat().st_size != local_binding["bytes"]:
        raise MorphologyContextError("private recurrence bundle byte-size drift")
    if sha256_file(recurrence_path) != local_binding["sha256"]:
        raise MorphologyContextError("private recurrence bundle digest drift")
    if (recurrence_path.stat().st_mode & 0o777) != 0o600:
        raise MorphologyContextError("private recurrence bundle mode must be 0600")
    bundle = load_json(recurrence_path)
    selected = bundle.get("selected_form", {})
    trigger = plan["a_trigger"]
    if (
        selected.get("form_key") != trigger["form_key"]
        or selected.get("exact_form_sha256") != trigger["exact_form_sha256"]
        or selected.get("normalized_form_sha256") != trigger["normalized_form_sha256"]
    ):
        raise MorphologyContextError("recurrence selected-form identity drift")
    occurrences = bundle.get("occurrences")
    if not isinstance(occurrences, list) or len(occurrences) != recurrence[
        "occurrence_count"
    ]:
        raise MorphologyContextError("complete recurrence occurrence count drift")
    order_keys = [
        (row["part_order"], row["token_ordinal"], row["occurrence_id"])
        for row in occurrences
    ]
    if order_keys != sorted(order_keys) or len(set(order_keys)) != len(order_keys):
        raise MorphologyContextError("recurrence occurrence order is not strict")
    bindings = {
        row["item_ref"]: row for row in bundle.get("source_bindings", [])
    }
    ranks = plan["selection"]["recurrence_ranks"]
    selected_occurrences = [occurrences[rank - 1] for rank in ranks]
    trees: dict[str, etree._ElementTree] = {}
    source_inputs: list[dict[str, str]] = []
    rows: list[dict[str, Any]] = []
    for rank, occurrence in zip(ranks, selected_occurrences, strict=True):
        item_ref = occurrence["item_ref"]
        binding = bindings.get(item_ref)
        if binding is None:
            raise MorphologyContextError("selected occurrence lacks source binding")
        payload_path = resolve_under(local_input_root, binding["payload_ref"])
        if sha256_file(payload_path) != binding["payload_sha256"]:
            raise MorphologyContextError("selected TEI payload fixity drift")
        if item_ref not in trees:
            trees[item_ref] = parse_tei(payload_path)
            source_inputs.append(
                {"ref": binding["payload_ref"], "sha256": binding["payload_sha256"]}
            )
        tree = trees[item_ref]
        owner_path, owner_kind = target_owner(occurrence["text_node_path"])
        owner = one_element(tree, owner_path, "target owner")
        node_text = owner.text if owner_kind == "text" else owner.tail
        if node_text is None:
            raise MorphologyContextError("selected TEI text node is empty")
        start = occurrence["start_offset"]
        end = occurrence["end_offset"]
        target = node_text[start:end]
        if sha256_bytes(target.encode("utf-8")) != trigger["exact_form_sha256"]:
            raise MorphologyContextError("selected raw TEI target return drift")
        context_path, context_kind = context_unit(occurrence["text_node_path"])
        context_element = one_element(tree, context_path, "context unit")
        context_text, target_base = flatten_with_target_base(
            context_element, owner, owner_kind
        )
        target_start = target_base + start
        target_end = target_base + end
        if context_text[target_start:target_end] != target:
            raise MorphologyContextError("context target offset return drift")
        context_sha256 = sha256_bytes(context_text.encode("utf-8"))
        context_id_payload = (
            plan["episode_id"]
            + "\n"
            + occurrence["occurrence_id"]
            + "\n"
            + context_sha256
        ).encode("utf-8")
        row = {
            "schema_version": "tos_morphology_contextual_episode_row_v1",
            "episode_id": plan["episode_id"],
            "context_id": "morphology-context:sha256:"
            + sha256_bytes(context_id_payload),
            "selection_rank": rank,
            "selection_role": RANK_ROLES[rank],
            "form_key": trigger["form_key"],
            "exact_form_sha256": trigger["exact_form_sha256"],
            "occurrence_id": occurrence["occurrence_id"],
            "item_ref": item_ref,
            "part_order": occurrence["part_order"],
            "source_file_sha256": occurrence["source_file_sha256"],
            "page_resource_id": occurrence["page_resource_id"],
            "section_resource_id": occurrence["section_resource_id"],
            "text_node_path": occurrence["text_node_path"],
            "source_node_start_offset": start,
            "source_node_end_offset": end,
            "context_unit_kind": context_kind,
            "context_node_path": context_path,
            "context_text": context_text,
            "context_sha256": context_sha256,
            "target_start_offset": target_start,
            "target_end_offset": target_end,
            "target_exact_form": target,
            "target_return_verified": True,
            "sentence_boundary_claimed": False,
            "exact_surface_mutated": False,
            "input_variant": "unchanged-historical-context",
            "authority": (
                "unreviewed-source-visible-context-for-machine-proposal-only"
            ),
        }
        validate_schema(row, REPO_ROOT / ROW_SCHEMA, f"private row rank {rank}")
        rows.append(row)
    if len({row["context_id"] for row in rows}) != len(rows):
        raise MorphologyContextError("derived context IDs are not unique")
    source_inputs.sort(key=lambda row: row["ref"])
    codepoint_counts = [len(row["context_text"]) for row in rows]
    context_summary = {
        "target_return_verified_count": len(rows),
        "context_unit_kind_counts": dict(
            sorted(Counter(row["context_unit_kind"] for row in rows).items())
        ),
        "total_context_codepoints": sum(codepoint_counts),
        "minimum_context_codepoints": min(codepoint_counts),
        "maximum_context_codepoints": max(codepoint_counts),
        "sentence_boundary_claimed": False,
        "exact_surface_mutated": False,
    }
    return rows, context_summary, source_inputs


def build_receipt(
    *,
    plan: dict[str, Any],
    plan_path: Path,
    trigger_closure: dict[str, Any],
    packet_bytes: bytes,
    rows: list[dict[str, Any]],
    context_summary: dict[str, Any],
    source_input_count: int,
) -> dict[str, Any]:
    recurrence = plan["source_recurrence"]
    part_counts = Counter(str(row["part_order"]) for row in rows)
    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "morphology-contextual-episode-receipt.schema.json"
        ),
        "schema_version": "tos_morphology_contextual_episode_receipt_v1",
        "receipt_id": (
            "morphology-contextual-episode-receipt:"
            "zarathustra-selected-form-b-v1"
        ),
        "status": "context-packet-materialized-b-unacquired",
        "plan": {
            "ref": plan_path.relative_to(REPO_ROOT).as_posix(),
            "sha256": sha256_file(plan_path),
        },
        "generator": {
            "ref": GENERATOR_REF,
            "sha256": sha256_file(REPO_ROOT / GENERATOR_REF),
        },
        "trigger_closure": trigger_closure,
        "source_recurrence": {
            "plan": recurrence["plan"],
            "receipt": recurrence["receipt"],
            "local_bundle_sha256": recurrence["local_bundle"]["sha256"],
            "complete_occurrence_count": recurrence["occurrence_count"],
            "source_payload_fixity_match_count": source_input_count,
        },
        "selection": {
            "method": plan["selection"]["method"],
            "recurrence_ranks": plan["selection"]["recurrence_ranks"],
            "rank_roles": plan["selection"]["rank_roles"],
            "row_count": len(rows),
            "part_counts": dict(sorted(part_counts.items())),
            "b_output_visible_during_selection": False,
            "semantic_labels_used": False,
        },
        "local_packet": {
            "relative_path": plan["local_packet"]["relative_path"],
            "format": "jsonl",
            "schema_ref": plan["local_packet"]["schema_ref"],
            "schema_version": plan["local_packet"]["schema_version"],
            "sha256": sha256_bytes(packet_bytes),
            "bytes": len(packet_bytes),
            "mode": "0600",
            "row_count": len(rows),
            "source_bearing": True,
        },
        "context_summary": context_summary,
        "variant_state": {
            "a": "existing-context-free-candidate-set",
            "b": "admitted-unacquired",
            "c": "blocked-question-inapplicable",
            "b_acquisition_requires_artifact_audit": True,
            "b_execution_requires_fresh_host_preflight": True,
            "human_work_scheduled": False,
        },
        "content_exposure": {
            "local_exact_strings": True,
            "local_context": True,
            "local_occurrence_positions": True,
            "tracked_exact_strings": False,
            "tracked_context": False,
            "tracked_occurrence_positions": False,
            "tracked_form_hashes": True,
        },
        "rights_and_visibility": plan["rights_and_visibility"],
        "competence_boundary": plan["competence_boundary"],
        "semantic_boundary": plan["semantic_boundary"],
        "provenance_event_ref": PROVENANCE_EVENT_ID,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def build_provenance(
    *,
    plan: dict[str, Any],
    plan_path: Path,
    receipt_path: Path,
    receipt_bytes: bytes,
    packet_bytes: bytes,
    source_inputs: list[dict[str, str]],
) -> dict[str, Any]:
    recurrence = plan["source_recurrence"]
    inputs = [
        {
            "ref": plan_path.relative_to(REPO_ROOT).as_posix(),
            "role": "frozen-output-blind-contextual-morphology-plan",
            "sha256": sha256_file(plan_path),
        },
        {
            "ref": plan["a_trigger"]["result_receipt"]["ref"],
            "role": "tracked-source-free-direct-a-census-result",
            "sha256": plan["a_trigger"]["result_receipt"]["sha256"],
        },
        {
            "ref": (
                "owner-local-artifacts/tree-of-sophia-foundation-lab/"
                "tos-historical-german-morphology-v1/"
                "zarathustra-dwdsmor-a-20260730t0009z/variant-A/"
                "raw-output/dwdsmor-census.jsonl"
            ),
            "role": "private-direct-a-provider-stream",
            "sha256": plan["a_trigger"]["private_raw_output"]["sha256"],
        },
        {
            "ref": recurrence["plan"]["ref"],
            "role": "tracked-selected-form-recurrence-plan",
            "sha256": recurrence["plan"]["sha256"],
        },
        {
            "ref": recurrence["receipt"]["ref"],
            "role": "tracked-complete-recurrence-receipt",
            "sha256": recurrence["receipt"]["sha256"],
        },
        {
            "ref": recurrence["local_bundle"]["ref"],
            "role": "private-complete-raw-witness-recurrence-bundle",
            "sha256": recurrence["local_bundle"]["sha256"],
        },
        {
            "ref": plan["research"]["ref"],
            "role": "ordered-and-refreshed-historical-german-morphology-research",
            "sha256": plan["research"]["sha256"],
        },
    ]
    inputs.extend(
        {"ref": row["ref"], "role": "selected-fixity-bound-raw-tei", "sha256": row["sha256"]}
        for row in source_inputs
    )
    return {
        "schema_version": "tos_provenance_event_v1",
        "event_id": PROVENANCE_EVENT_ID,
        "event_type": "annotation",
        "started_at": "2026-08-10T22:00:00Z",
        "ended_at": "2026-08-10T22:00:00Z",
        "agent_refs": [
            "software:python-" + platform.python_version(),
            "software:lxml-" + ".".join(map(str, etree.LXML_VERSION)),
        ],
        "inputs": inputs,
        "outputs": [
            {
                "ref": plan["local_packet"]["relative_path"],
                "role": "private-output-blind-raw-tei-context-packet",
                "sha256": sha256_bytes(packet_bytes),
            },
            {
                "ref": receipt_path.relative_to(REPO_ROOT).as_posix(),
                "role": "tracked-text-and-position-free-context-receipt",
                "sha256": sha256_bytes(receipt_bytes),
            },
        ],
        "method": {
            "maker_type": "software",
            "name": "raw-tei-first-median-last-morphology-context-freeze",
            "version": "1",
            "artifact_digest": sha256_file(REPO_ROOT / GENERATOR_REF),
            "runtime": f"Python {platform.python_version()} with lxml {etree.__version__}",
            "device": "CPU",
            "configuration": {
                "selection": plan["selection"]["method"],
                "recurrence_ranks": plan["selection"]["recurrence_ranks"],
                "context_units": plan["context_policy"]["unit_rules"],
                "network_allowed": False,
                "b_output_visible_during_selection": False,
                "c_admitted_for_question": False,
                "human_work_scheduled": False,
            },
            "prompt_or_instruction_ref": plan["research"]["ref"],
        },
        "status": "completed_with_warnings",
        "warnings": [
            "exact context and occurrence positions remain ignored mode-0600 local-only material",
            "paragraph and verse-group boundaries are transparent TEI context units, not accepted sentence or sense boundaries",
            "the packet admits only a machine B proposal after separate artifact and host gates",
            "C remains blocked because normalization is not decision-relevant to this episode",
            "no German competence, accepted morphology, lemma, semantic effect, publication route, or human backlog is created",
        ],
        "receipt_refs": [
            receipt_path.relative_to(REPO_ROOT).as_posix(),
            plan_path.relative_to(REPO_ROOT).as_posix(),
            plan["research"]["ref"],
        ],
        "rights_basis_ref": None,
        "event_version": 1,
        "supersedes_event_ref": None,
    }


def atomic_write(path: Path, payload: bytes, mode: int) -> None:
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
        os.chmod(temporary_path, mode)
        os.replace(temporary_path, path)
    finally:
        temporary_path.unlink(missing_ok=True)


def materialize(
    *,
    plan_path: Path,
    local_input_root: Path,
    local_output_root: Path,
    a_raw_output: Path,
    receipt_path: Path,
    provenance_path: Path,
    check: bool,
) -> dict[str, Any]:
    plan_path = plan_path.resolve()
    try:
        plan_path.relative_to(REPO_ROOT.resolve())
    except ValueError as exc:
        raise MorphologyContextError("plan must remain inside the Tree repository") from exc
    plan = load_json(plan_path)
    validate_schema(plan, REPO_ROOT / PLAN_SCHEMA, "plan")
    verify_repo_ref(plan["research"], "research")
    verify_repo_ref(plan["parent_morphology_plan"], "parent morphology plan")
    verify_repo_ref(plan["source_recurrence"]["plan"], "recurrence plan")
    verify_repo_ref(plan["source_recurrence"]["receipt"], "recurrence receipt")
    trigger_closure = verify_a_trigger(plan, a_raw_output.resolve())
    rows, context_summary, source_inputs = build_context_rows(
        plan=plan,
        local_input_root=local_input_root,
    )
    packet_bytes = b"".join(canonical_json_bytes(row) for row in rows)
    packet_path = resolve_under(local_output_root, plan["local_packet"]["relative_path"])
    receipt = build_receipt(
        plan=plan,
        plan_path=plan_path,
        trigger_closure=trigger_closure,
        packet_bytes=packet_bytes,
        rows=rows,
        context_summary=context_summary,
        source_input_count=len(source_inputs),
    )
    validate_schema(receipt, REPO_ROOT / RECEIPT_SCHEMA, "receipt")
    receipt_bytes = canonical_json_bytes(receipt)
    provenance = build_provenance(
        plan=plan,
        plan_path=plan_path,
        receipt_path=receipt_path,
        receipt_bytes=receipt_bytes,
        packet_bytes=packet_bytes,
        source_inputs=source_inputs,
    )
    validate_schema(provenance, REPO_ROOT / PROVENANCE_SCHEMA, "provenance")
    provenance_bytes = canonical_json_bytes(provenance)

    if check:
        if not packet_path.is_file() or packet_path.read_bytes() != packet_bytes:
            raise MorphologyContextError("private context packet is absent or stale")
        if (packet_path.stat().st_mode & 0o777) != 0o600:
            raise MorphologyContextError("private context packet mode must be 0600")
        if not receipt_path.is_file() or receipt_path.read_bytes() != receipt_bytes:
            raise MorphologyContextError("tracked context receipt is absent or stale")
        if not provenance_path.is_file() or provenance_path.read_bytes() != provenance_bytes:
            raise MorphologyContextError("tracked context provenance is absent or stale")
        return receipt

    atomic_write(packet_path, packet_bytes, 0o600)
    atomic_write(receipt_path, receipt_bytes, 0o644)
    atomic_write(provenance_path, provenance_bytes, 0o644)
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, default=REPO_ROOT / DEFAULT_PLAN)
    parser.add_argument("--local-input-root", type=Path, required=True)
    parser.add_argument("--local-output-root", type=Path, required=True)
    parser.add_argument("--a-raw-output", type=Path, required=True)
    parser.add_argument("--receipt", type=Path)
    parser.add_argument("--provenance", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        plan = load_json(args.plan)
        receipt_path = (
            args.receipt.resolve()
            if args.receipt
            else resolve_under(REPO_ROOT, plan["tracked_receipt_ref"])
        )
        provenance_path = (
            args.provenance.resolve()
            if args.provenance
            else resolve_under(REPO_ROOT, plan["provenance_ref"])
        )
        receipt = materialize(
            plan_path=args.plan,
            local_input_root=args.local_input_root.resolve(),
            local_output_root=args.local_output_root.resolve(),
            a_raw_output=args.a_raw_output,
            receipt_path=receipt_path,
            provenance_path=provenance_path,
            check=args.check,
        )
    except MorphologyContextError as exc:
        parser.error(str(exc))
    action = "verified" if args.check else "materialized"
    print(
        f"{action} {receipt['selection']['row_count']} output-blind contexts; "
        f"B={receipt['variant_state']['b']}; C={receipt['variant_state']['c']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
