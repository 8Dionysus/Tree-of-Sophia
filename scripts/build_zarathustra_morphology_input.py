#!/usr/bin/env python3
"""Materialize the private exact-form input for the Zarathustra morphology census.

The JSONL packet stays under the ignored local-content boundary. The tracked
receipt contains only fixity, counts, and source-withholding aggregates. This
route never executes a morphology provider or creates a lemma or lexeme.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
import tempfile
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PLAN = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "morphology-evaluation-plan.v1.json"
)
PLAN_SCHEMA = Path("ToS/contracts/morphology-evaluation-plan.schema.json")
RECEIPT_SCHEMA = Path("ToS/contracts/morphology-input-receipt.schema.json")
GENERATOR_REF = "scripts/build_zarathustra_morphology_input.py"
AUTHORITY_BOUNDARY = (
    "private exact-form morphology input materialization only; no accepted "
    "German, source correction, lemma, lexeme, sign, concept, translation, "
    "claim, relation, graph, canon, rights clearance, publication, or human backlog"
)
ROW_FIELDS = [
    "schema_version",
    "form_key",
    "exact_form",
    "exact_form_sha256",
    "normalized_form_sha256",
    "occurrence_count",
]
JOINERS = {"-", "'", "’", "‐", "‑"}


class MorphologyInputError(RuntimeError):
    """Raised when the private morphology input cannot be closed exactly."""


def load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise MorphologyInputError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise MorphologyInputError(f"{path} must contain a JSON object")
    return payload


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as exc:
        raise MorphologyInputError(f"cannot hash {path}: {exc}") from exc
    return digest.hexdigest()


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


def validate_schema(payload: object, schema_path: Path, label: str) -> None:
    schema = load_json(schema_path)
    errors = sorted(
        Draft202012Validator(schema).iter_errors(payload),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        raise MorphologyInputError(
            f"{label} schema validation failed: "
            + "; ".join(error.message for error in errors[:8])
        )


def resolve_under(root: Path, relative: str) -> Path:
    resolved_root = root.resolve()
    candidate = (resolved_root / relative).resolve()
    try:
        candidate.relative_to(resolved_root)
    except ValueError as exc:
        raise MorphologyInputError(f"path escapes explicit root: {relative}") from exc
    return candidate


def read_form_rows(database_path: Path) -> list[dict[str, Any]]:
    try:
        connection = sqlite3.connect(
            f"file:{database_path.as_posix()}?mode=ro&immutable=1",
            uri=True,
        )
    except sqlite3.Error as exc:
        raise MorphologyInputError(
            f"cannot open lexical database read-only: {exc}"
        ) from exc
    connection.row_factory = sqlite3.Row
    try:
        quick_check = connection.execute("PRAGMA quick_check").fetchone()
        if quick_check is None or quick_check[0] != "ok":
            raise MorphologyInputError("lexical database quick_check failed")
        rows = [
            dict(row)
            for row in connection.execute(
                """
                SELECT
                  form_key,
                  exact_form,
                  exact_form_sha256,
                  normalized_form_sha256,
                  occurrence_count
                FROM forms
                ORDER BY exact_form_sha256, exact_form
                """
            )
        ]
    except sqlite3.Error as exc:
        raise MorphologyInputError(f"cannot query lexical database: {exc}") from exc
    finally:
        connection.close()
    return rows


def build_packet(rows: list[dict[str, Any]]) -> tuple[bytes, dict[str, int]]:
    packet_parts: list[bytes] = []
    normalized_hashes: set[str] = set()
    token_occurrence_count = 0
    singleton_form_count = 0
    joiner_form_count = 0
    maximum_codepoint_length = 0
    previous_order: tuple[str, str] | None = None

    for source_row in rows:
        exact_form = source_row.get("exact_form")
        exact_digest = source_row.get("exact_form_sha256")
        normalized_digest = source_row.get("normalized_form_sha256")
        occurrence_count = source_row.get("occurrence_count")
        form_key = source_row.get("form_key")
        if not isinstance(exact_form, str) or not exact_form:
            raise MorphologyInputError("form row has no exact surface")
        if hashlib.sha256(exact_form.encode("utf-8")).hexdigest() != exact_digest:
            raise MorphologyInputError("exact form digest mismatch")
        if form_key != f"lexical-form:sha256:{exact_digest}":
            raise MorphologyInputError("form key does not bind exact form digest")
        if (
            not isinstance(normalized_digest, str)
            or len(normalized_digest) != 64
            or any(character not in "0123456789abcdef" for character in normalized_digest)
        ):
            raise MorphologyInputError("invalid normalized form digest")
        if not isinstance(occurrence_count, int) or occurrence_count < 1:
            raise MorphologyInputError("invalid occurrence count")
        order_key = (exact_digest, exact_form)
        if previous_order is not None and order_key <= previous_order:
            raise MorphologyInputError("form rows are not in strict frozen order")
        previous_order = order_key

        row = {
            "schema_version": "tos_morphology_input_row_v1",
            "form_key": form_key,
            "exact_form": exact_form,
            "exact_form_sha256": exact_digest,
            "normalized_form_sha256": normalized_digest,
            "occurrence_count": occurrence_count,
        }
        if list(row) != ROW_FIELDS:
            raise MorphologyInputError("private row field order drift")
        packet_parts.append(canonical_json_bytes(row))
        normalized_hashes.add(normalized_digest)
        token_occurrence_count += occurrence_count
        singleton_form_count += occurrence_count == 1
        joiner_form_count += any(joiner in exact_form for joiner in JOINERS)
        maximum_codepoint_length = max(maximum_codepoint_length, len(exact_form))

    if not packet_parts:
        raise MorphologyInputError("lexical database returned no form rows")
    return b"".join(packet_parts), {
        "exact_form_row_count": len(rows),
        "normalized_form_hash_count": len(normalized_hashes),
        "token_occurrence_count": token_occurrence_count,
        "singleton_form_count": singleton_form_count,
        "joiner_form_count": joiner_form_count,
        "maximum_codepoint_length": maximum_codepoint_length,
    }


def build_receipt(
    *,
    plan: dict[str, Any],
    plan_path: Path,
    database_path: Path,
    packet_bytes: bytes,
    summary: dict[str, int],
) -> dict[str, Any]:
    source = plan["source_lexical_index"]
    local_packet = plan["a_census"]["local_packet"]
    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "morphology-input-receipt.schema.json"
        ),
        "schema_version": "tos_morphology_input_receipt_v1",
        "generated_or_authored": "generated_from_local_lexical_projection",
        "receipt_id": (
            "morphology-input-receipt:"
            "zarathustra-dta-exact-form-census-v1"
        ),
        "plan_id": plan["plan_id"],
        "plan_ref": plan_path.relative_to(REPO_ROOT).as_posix(),
        "plan_sha256": sha256_file(plan_path),
        "generator_ref": GENERATOR_REF,
        "generator_sha256": sha256_file(REPO_ROOT / GENERATOR_REF),
        "source_database": {
            "relative_path": source["local_database_relative_path"],
            "sha256": sha256_file(database_path),
            "bytes": database_path.stat().st_size,
        },
        "source_projection": {
            "index_plan_ref": source["index_plan_ref"],
            "index_plan_sha256": source["index_plan_sha256"],
            "tracked_projection_ref": source["tracked_projection_ref"],
            "tracked_projection_sha256": source["tracked_projection_sha256"],
        },
        "local_packet": {
            "relative_path": local_packet["relative_path"],
            "format": "jsonl",
            "schema_version": "tos_morphology_input_row_v1",
            "sha256": sha256_bytes(packet_bytes),
            "bytes": len(packet_bytes),
            "mode": "0600",
            "row_count": summary["exact_form_row_count"],
            "required_fields": ROW_FIELDS,
        },
        "summary": summary,
        "content_exposure": {
            "local_exact_strings": True,
            "tracked_exact_strings": False,
            "tracked_sequence": False,
            "tracked_context": False,
            "tracked_positions": False,
        },
        "semantic_boundary": {
            "creates_accepted_source": False,
            "creates_lemma": False,
            "creates_lexeme": False,
            "creates_sign": False,
            "creates_semantic_claim": False,
            "opens_human_backlog": False,
        },
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def verify_source_refs(plan: dict[str, Any]) -> None:
    source = plan["source_lexical_index"]
    for ref_key, digest_key in (
        ("index_plan_ref", "index_plan_sha256"),
        ("tracked_projection_ref", "tracked_projection_sha256"),
    ):
        referenced_path = resolve_under(REPO_ROOT, source[ref_key])
        if sha256_file(referenced_path) != source[digest_key]:
            raise MorphologyInputError(f"{ref_key} digest drift")


def materialize(
    *,
    plan_path: Path,
    local_input_root: Path,
    local_output_root: Path,
    receipt_path: Path,
    check: bool,
) -> dict[str, Any]:
    resolved_plan = plan_path.resolve()
    try:
        resolved_plan.relative_to(REPO_ROOT.resolve())
    except ValueError as exc:
        raise MorphologyInputError("plan must remain inside the Tree repository") from exc
    plan = load_json(resolved_plan)
    validate_schema(plan, REPO_ROOT / PLAN_SCHEMA, "plan")
    verify_source_refs(plan)

    source = plan["source_lexical_index"]
    database_path = resolve_under(
        local_input_root,
        source["local_database_relative_path"],
    )
    if sha256_file(database_path) != source["local_database_sha256"]:
        raise MorphologyInputError("local lexical database digest drift")
    rows = read_form_rows(database_path)
    packet_bytes, summary = build_packet(rows)
    for field in (
        "exact_form_row_count",
        "normalized_form_hash_count",
        "token_occurrence_count",
    ):
        if summary[field] != source[field]:
            raise MorphologyInputError(
                f"{field} drift: observed {summary[field]}, expected {source[field]}"
            )

    packet_path = resolve_under(
        local_output_root,
        plan["a_census"]["local_packet"]["relative_path"],
    )
    receipt = build_receipt(
        plan=plan,
        plan_path=resolved_plan,
        database_path=database_path,
        packet_bytes=packet_bytes,
        summary=summary,
    )
    validate_schema(receipt, REPO_ROOT / RECEIPT_SCHEMA, "receipt")

    if check:
        if not packet_path.is_file():
            raise MorphologyInputError(f"private packet is missing: {packet_path}")
        if packet_path.read_bytes() != packet_bytes:
            raise MorphologyInputError("private packet is stale or non-deterministic")
        existing_receipt = load_json(receipt_path)
        if existing_receipt != receipt:
            raise MorphologyInputError("tracked input receipt is stale")
        if (packet_path.stat().st_mode & 0o777) != 0o600:
            raise MorphologyInputError("private packet mode must be 0600")
        return receipt

    packet_path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix="tos-morphology-input-",
        suffix=".jsonl",
        dir=packet_path.parent,
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(packet_bytes)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary_path, 0o600)
        os.replace(temporary_path, packet_path)
    finally:
        temporary_path.unlink(missing_ok=True)

    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path.write_bytes(canonical_json_bytes(receipt))
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--plan",
        type=Path,
        default=REPO_ROOT / DEFAULT_PLAN,
    )
    parser.add_argument(
        "--local-input-root",
        type=Path,
        required=True,
        help="explicit root containing the ignored lexical database",
    )
    parser.add_argument(
        "--local-output-root",
        type=Path,
        required=True,
        help="explicit root owning the ignored morphology packet",
    )
    parser.add_argument(
        "--receipt",
        type=Path,
        default=None,
        help="tracked text-free receipt path; defaults to the plan declaration",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    try:
        plan = load_json(args.plan)
        receipt_path = (
            args.receipt.resolve()
            if args.receipt is not None
            else resolve_under(
                REPO_ROOT,
                plan["a_census"]["tracked_receipt_ref"],
            )
        )
        receipt = materialize(
            plan_path=args.plan,
            local_input_root=args.local_input_root,
            local_output_root=args.local_output_root,
            receipt_path=receipt_path,
            check=args.check,
        )
    except MorphologyInputError as exc:
        parser.error(str(exc))
    action = "verified" if args.check else "materialized"
    print(
        f"[ok] {action} {receipt['summary']['exact_form_row_count']} exact "
        f"forms / {receipt['summary']['token_occurrence_count']} occurrences"
    )
    print(
        "[boundary] input materialization creates no morphology output, "
        "accepted German, lemma, lexeme, sign, or human work"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
