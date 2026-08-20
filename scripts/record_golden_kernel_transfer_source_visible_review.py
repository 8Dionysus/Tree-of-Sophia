#!/usr/bin/env python3
"""Record a text-free receipt from one private model source-visible review.

The private bundle and all source-bearing inputs remain in the operator-local,
gitignored Tree of Sophia storage. The tracked result contains only identities,
fixity, aggregate token diagnostics, reproduced render hashes, and closed gates.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import subprocess
import sys
import tempfile
import unicodedata
from datetime import datetime, timezone
from difflib import SequenceMatcher
from html.parser import HTMLParser
from pathlib import Path
from typing import Any, Iterable

from jsonschema import FormatChecker
from jsonschema.validators import validator_for


REPO_ROOT = Path(__file__).resolve().parents[1]
PRIVATE_SCHEMA_REF = (
    "ToS/contracts/private-transfer-source-visible-review-bundle.schema.json"
)
RECEIPT_SCHEMA_REF = "ToS/contracts/transfer-source-visible-review-receipt.schema.json"
PRIVATE_BUNDLE_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/"
    "transfer-source-visible-review/v1/jenseits-187/review-bundle.json"
)
RECEIPT_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "transfer-source-visible-review.jenseits-187.v1.json"
)
GENERATOR_REF = "scripts/record_golden_kernel_transfer_source_visible_review.py"
ALLOWED_TEMP_ROOT = Path("/srv/abyss-machine/tmp")
AUTHORITY_BOUNDARY = (
    "This text-free receipt proves deterministic reconstruction of four exact "
    "page renders, private-bundle and input fixity, a bounded critical-edition "
    "selector, and aggregate alpha-token diagnostics produced from a private "
    "model source-visible transcript. It does not make that transcript human "
    "evidence or accept German or Russian text; it does not establish historical-"
    "critical identity, source-to-target alignment, translation quality, rights "
    "clearance, eligibility, gold, a sign, concept, claim, relation, graph edge, "
    "canon effect, publication authority, or scheduled human work."
)


class TransferSourceVisibleReviewError(RuntimeError):
    """Raised when the bounded private evidence does not close exactly."""


class _AnchoredTextBlockParser(HTMLParser):
    def __init__(self, anchor: str) -> None:
        super().__init__(convert_charrefs=True)
        self.anchor = anchor
        self.anchor_seen = False
        self.in_text_block = False
        self.finished = False
        self.depth = 0
        self.parts: list[str] = []

    def _observe_anchor(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag == "a" and dict(attrs).get("name") == self.anchor:
            self.anchor_seen = True

    def handle_startendtag(
        self,
        tag: str,
        attrs: list[tuple[str, str | None]],
    ) -> None:
        self._observe_anchor(tag, attrs)

    def handle_starttag(
        self,
        tag: str,
        attrs: list[tuple[str, str | None]],
    ) -> None:
        self._observe_anchor(tag, attrs)
        if self.finished:
            return
        classes = (dict(attrs).get("class") or "").split()
        if (
            self.anchor_seen
            and not self.in_text_block
            and tag == "div"
            and "txt_block" in classes
        ):
            self.in_text_block = True
            self.depth = 1
            return
        if self.in_text_block:
            self.depth += 1

    def handle_endtag(self, tag: str) -> None:
        if not self.in_text_block:
            return
        self.depth -= 1
        if self.depth == 0:
            self.in_text_block = False
            self.finished = True

    def handle_data(self, data: str) -> None:
        if self.in_text_block and data.strip():
            self.parts.append(data)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


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


def load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise TransferSourceVisibleReviewError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise TransferSourceVisibleReviewError(f"{path} must contain a JSON object")
    return payload


def _validator(schema_path: Path):
    schema = load_json(schema_path)
    validator_class = validator_for(schema)
    validator_class.check_schema(schema)
    return validator_class(schema, format_checker=FormatChecker())


def _validate(payload: object, schema_path: Path, label: str) -> None:
    errors = sorted(
        _validator(schema_path).iter_errors(payload),
        key=lambda item: list(item.absolute_path),
    )
    if not errors:
        return
    detail = "; ".join(
        f"{'.'.join(str(part) for part in error.absolute_path) or '<root>'}: {error.message}"
        for error in errors[:8]
    )
    raise TransferSourceVisibleReviewError(
        f"{label} schema validation failed: {detail}"
    )


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise TransferSourceVisibleReviewError(message)


def _safe_local_path(local_input_root: Path, relative_ref: str) -> Path:
    root = local_input_root.resolve()
    path = (root / relative_ref).resolve()
    try:
        path.relative_to(root)
    except ValueError as exc:
        raise TransferSourceVisibleReviewError(
            f"private reference escapes local input root: {relative_ref}"
        ) from exc
    _require(path.is_file(), f"private input is absent: {relative_ref}")
    return path


def _verify_sha(path: Path, expected: str, label: str) -> None:
    observed = sha256_file(path)
    _require(observed == expected, f"{label} digest differs: {observed} != {expected}")


def source_aware_text(lines: Iterable[dict[str, Any]]) -> str:
    """Join printed lines while removing only end-of-line word hyphenation."""
    text = "\n".join(str(line["text"]).strip() for line in lines)
    characters: list[str] = []
    index = 0
    while index < len(text):
        character = text[index]
        if (
            character == "-"
            and index > 0
            and index + 2 < len(text)
            and text[index + 1] == "\n"
            and text[index - 1].isalpha()
            and text[index + 2].isalpha()
        ):
            index += 2
            continue
        characters.append(" " if character == "\n" else character)
        index += 1
    return " ".join("".join(characters).split())


def alpha_tokens(text: str) -> list[str]:
    normalized = unicodedata.normalize("NFC", text)
    tokens: list[str] = []
    current: list[str] = []
    for character in normalized:
        if character.isalpha():
            current.append(character)
        elif current:
            tokens.append("".join(current))
            current = []
    if current:
        tokens.append("".join(current))
    return tokens


def token_diff(reference: list[str], candidate: list[str]) -> dict[str, Any]:
    matcher = SequenceMatcher(None, reference, candidate, autojunk=False)
    equal = 0
    reference_missing = 0
    candidate_extra = 0
    replacement_blocks = 0
    deletion_blocks = 0
    insertion_blocks = 0
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == "equal":
            equal += i2 - i1
        elif tag == "replace":
            replacement_blocks += 1
            reference_missing += i2 - i1
            candidate_extra += j2 - j1
        elif tag == "delete":
            deletion_blocks += 1
            reference_missing += i2 - i1
        elif tag == "insert":
            insertion_blocks += 1
            candidate_extra += j2 - j1
    return {
        "reference_token_count": len(reference),
        "candidate_token_count": len(candidate),
        "equal_token_count": equal,
        "reference_missing_token_count": reference_missing,
        "candidate_extra_token_count": candidate_extra,
        "replacement_block_count": replacement_blocks,
        "deletion_block_count": deletion_blocks,
        "insertion_block_count": insertion_blocks,
        "exact_alpha_token_sequence": reference == candidate,
    }


def _selected_critical_tokens(path: Path, anchor: str) -> list[str]:
    parser = _AnchoredTextBlockParser(anchor)
    parser.feed(path.read_text(encoding="utf-8"))
    parser.close()
    _require(parser.anchor_seen, f"critical selector anchor is absent: {anchor}")
    _require(
        parser.finished and parser.parts, f"critical text block is unresolved: {anchor}"
    )
    return alpha_tokens(" ".join(parser.parts))


def _render_and_verify(
    witnesses: Iterable[tuple[str, dict[str, Any], Path]],
    *,
    temporary_root: Path,
    resolution_dpi: int,
) -> None:
    allowed = ALLOWED_TEMP_ROOT.resolve()
    temporary_root = temporary_root.resolve()
    try:
        temporary_root.relative_to(allowed)
    except ValueError as exc:
        raise TransferSourceVisibleReviewError(
            f"temporary root must remain under {allowed}"
        ) from exc
    _require(temporary_root.is_dir(), f"temporary root is absent: {temporary_root}")
    with tempfile.TemporaryDirectory(
        prefix="tos-jgb187-source-visible-review.",
        dir=temporary_root,
    ) as temporary:
        output_root = Path(temporary)
        rendered = 0
        for label, witness, pdf_path in witnesses:
            for page_record in witness["pages"]:
                page = page_record["page"]
                prefix = output_root / f"{label}-{page}"
                command = [
                    "pdftoppm",
                    "-f",
                    str(page),
                    "-l",
                    str(page),
                    "-r",
                    str(resolution_dpi),
                    "-png",
                    "-singlefile",
                    os.fspath(pdf_path),
                    os.fspath(prefix),
                ]
                completed = subprocess.run(
                    command,
                    check=False,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.PIPE,
                    text=True,
                )
                if completed.returncode != 0:
                    raise TransferSourceVisibleReviewError(
                        f"pdftoppm failed for {label} page {page}: {completed.stderr.strip()}"
                    )
                output = prefix.with_suffix(".png")
                _require(
                    output.is_file(), f"render was not created: {label} page {page}"
                )
                _verify_sha(
                    output,
                    page_record["render_sha256"],
                    f"{label} page {page} render",
                )
                rendered += 1
        _require(
            rendered == 4, f"expected four reproduced renders, observed {rendered}"
        )


def _utc_timestamp(value: str) -> str:
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    _require(parsed.tzinfo is not None, "private bundle created_at lacks timezone")
    return parsed.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def _witness_receipt(
    witness: dict[str, Any],
) -> dict[str, Any]:
    rights_path = (REPO_ROOT / witness["rights_ref"]).resolve()
    try:
        rights_path.relative_to(REPO_ROOT.resolve())
    except ValueError as exc:
        raise TransferSourceVisibleReviewError(
            f"tracked rights reference escapes repository: {witness['rights_ref']}"
        ) from exc
    _require(
        rights_path.is_file(),
        f"tracked rights record is absent: {witness['rights_ref']}",
    )
    return {
        "language": witness["language"],
        "expression_ref": witness["expression_ref"],
        "item_ref": witness["item_ref"],
        "file_ref": witness["file_ref"],
        "file_sha256": witness["file_sha256"],
        "rights_ref": witness["rights_ref"],
        "rights_sha256": sha256_file(rights_path),
        "automatic_candidate_ref": witness["automatic_candidate_ref"],
        "automatic_candidate_sha256": witness["automatic_candidate_sha256"],
        "page_renders": [
            {"page": row["page"], "sha256": row["render_sha256"]}
            for row in witness["pages"]
        ],
    }


def build_receipt(
    *,
    local_input_root: Path,
    temporary_root: Path,
) -> dict[str, Any]:
    private_path = _safe_local_path(local_input_root, PRIVATE_BUNDLE_REF)
    mode = stat.S_IMODE(private_path.stat().st_mode)
    _require(mode == 0o600, f"private bundle mode must be 0600, observed {mode:04o}")
    bundle = load_json(private_path)
    _validate(
        bundle,
        REPO_ROOT / PRIVATE_SCHEMA_REF,
        "private source-visible review bundle",
    )

    source = bundle["source"]
    target = bundle["target"]
    for label, witness in (("source", source), ("target", target)):
        payload_path = _safe_local_path(local_input_root, witness["payload_ref"])
        _verify_sha(payload_path, witness["file_sha256"], f"{label} payload")
        _require(
            witness["file_ref"] == f"tos.file.sha256.{witness['file_sha256']}",
            f"{label} file_ref does not name its payload digest",
        )
        candidate_path = _safe_local_path(
            local_input_root,
            witness["automatic_candidate_ref"],
        )
        _verify_sha(
            candidate_path,
            witness["automatic_candidate_sha256"],
            f"{label} automatic candidate",
        )

    source_pdf = _safe_local_path(local_input_root, source["payload_ref"])
    target_pdf = _safe_local_path(local_input_root, target["payload_ref"])
    _render_and_verify(
        (("source", source, source_pdf), ("target", target, target_pdf)),
        temporary_root=temporary_root,
        resolution_dpi=bundle["render_method"]["resolution_dpi"],
    )

    critical = source["critical_witness"]
    critical_path = _safe_local_path(
        local_input_root,
        critical["local_payload_ref"],
    )
    _verify_sha(
        critical_path,
        critical["local_payload_sha256"],
        "critical comparison payload",
    )
    _require(
        critical_path.stat().st_size == critical["local_payload_bytes"],
        "critical comparison payload byte count differs",
    )
    selected_critical_tokens = _selected_critical_tokens(
        critical_path,
        critical["selector"]["value"],
    )
    declared_critical_tokens = alpha_tokens(critical["text"])
    _require(
        selected_critical_tokens == declared_critical_tokens,
        "declared critical text differs from the selected HTML alpha-token sequence",
    )

    source_candidate = load_json(
        _safe_local_path(local_input_root, source["automatic_candidate_ref"])
    )
    target_candidate = load_json(
        _safe_local_path(local_input_root, target["automatic_candidate_ref"])
    )
    _require(
        source_candidate.get("qualified_unit_key") == bundle["qualified_unit_key"],
        "source automatic candidate unit differs",
    )
    _require(
        target_candidate.get("qualified_unit_key") == bundle["qualified_unit_key"],
        "target automatic candidate unit differs",
    )
    source_diplomatic_tokens = alpha_tokens(
        source_aware_text(source["diplomatic_lines"])
    )
    target_diplomatic_tokens = alpha_tokens(
        source_aware_text(target["diplomatic_lines"])
    )
    source_automatic_tokens = alpha_tokens(source_candidate["automatic_candidate_text"])
    target_automatic_tokens = alpha_tokens(target_candidate["automatic_candidate_text"])

    source_critical_comparison = token_diff(
        selected_critical_tokens,
        source_diplomatic_tokens,
    )
    source_critical_comparison.update(
        {
            "comparison_role": "critical_comparison_not_historical_identity",
            "historical_critical_equivalence_established": False,
        }
    )
    source_automatic_comparison = token_diff(
        source_diplomatic_tokens,
        source_automatic_tokens,
    )
    source_automatic_comparison["comparison_role"] = (
        "automatic_ocr_diagnostic_against_private_model_transcript"
    )
    target_automatic_comparison = token_diff(
        target_diplomatic_tokens,
        target_automatic_tokens,
    )
    target_automatic_comparison.update(
        {
            "comparison_role": "automatic_extraction_diagnostic_against_private_model_transcript",
            "observed_complete_visible_line_omission_count": 1,
        }
    )

    receipt = {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/transfer-source-visible-review-receipt.schema.json",
        "schema_version": "tos_transfer_source_visible_review_receipt_v1",
        "generated_or_authored": "generated_from_private_model_source_visible_review_bundle",
        "receipt_id": "tos.transfer-source-visible-review.jenseits-187.v1",
        "recorded_at_utc": _utc_timestamp(bundle["created_at"]),
        "status": "machine-triangulated-candidate-not-admitted",
        "route_readiness_id": bundle["route_readiness_id"],
        "work_ref": bundle["work_ref"],
        "qualified_unit_key": bundle["qualified_unit_key"],
        "generator": {
            "ref": GENERATOR_REF,
            "sha256": sha256_file(REPO_ROOT / GENERATOR_REF),
        },
        "private_bundle": {
            "owner": "operator-local-tree-of-sophia",
            "relative_ref": PRIVATE_BUNDLE_REF,
            "sha256": sha256_file(private_path),
            "bytes": private_path.stat().st_size,
            "mode": "0600",
            "source_bearing": True,
            "review_mode": "model_source_visible",
            "human_review_performed": False,
        },
        "evidence_inputs": {
            "source": _witness_receipt(source),
            "target": _witness_receipt(target),
            "critical_witness": {
                "stable_locator": critical["stable_locator"],
                "source_role": critical["source_role"],
                "payload_sha256": critical["local_payload_sha256"],
                "payload_bytes": critical["local_payload_bytes"],
                "selector_kind": critical["selector"]["kind"],
                "selector_value": critical["selector"]["value"],
                "transport": critical["retrieval"]["transport"],
                "bounded_repeat_fetches": critical["retrieval"][
                    "bounded_repeat_fetches"
                ],
                "repeat_fetches_byte_identical": critical["retrieval"][
                    "repeat_fetches_byte_identical"
                ],
                "transport_authenticated": False,
            },
        },
        "reproduction": {
            "private_schema_validated": True,
            "private_fixity_verified": True,
            "source_payload_fixity_verified": True,
            "target_payload_fixity_verified": True,
            "automatic_candidate_fixity_verified": True,
            "critical_payload_fixity_verified": True,
            "critical_selector_resolved": True,
            "critical_declared_text_matches_selected_alpha_tokens": True,
            "render_tool": bundle["render_method"]["tool"],
            "resolution_dpi": bundle["render_method"]["resolution_dpi"],
            "render_count": 4,
            "render_hashes_reproduced": True,
            "temporary_renders_retained": False,
            "network_used": False,
        },
        "comparisons": {
            "source_diplomatic_against_critical": source_critical_comparison,
            "source_automatic_against_diplomatic": source_automatic_comparison,
            "target_automatic_against_diplomatic": target_automatic_comparison,
        },
        "observed_findings": [
            {
                "finding_id": finding["finding_id"],
                "scope": finding["scope"],
                "severity": finding["severity"],
                "evidence_role": "model_observation_not_human_truth",
            }
            for finding in bundle["observed_findings"]
        ],
        "effects": {
            "accepted_source_passage": False,
            "accepted_target_passage": False,
            "source_to_target_alignment": False,
            "eligible_for_variant_execution": False,
            "target_gold": False,
            "human_review_performed": False,
            "human_debt_units": 0,
            "semantic_work_opened": False,
            "signs_created": 0,
            "concepts_created": 0,
            "claims_created": 0,
            "relations_created": 0,
            "graph_edges_created": 0,
            "canon_effect": False,
            "publication_authorized": False,
        },
        "rights_and_visibility": {
            "private_source_and_target_text_retained_local_only": True,
            "tracked_receipt_contains_source_or_target_strings": False,
            "tracked_receipt_contains_private_absolute_paths": False,
            "source_and_target_publication_authorized": False,
            "critical_derivative_publication_authorized": False,
            "rights_clearance_established": False,
        },
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    _validate(
        receipt,
        REPO_ROOT / RECEIPT_SCHEMA_REF,
        "tracked source-visible review receipt",
    )
    return receipt


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--local-input-root",
        type=Path,
        required=True,
        help="Owner-local Tree of Sophia root containing ignored payloads.",
    )
    parser.add_argument(
        "--temporary-root",
        type=Path,
        required=True,
        help="Existing temporary root under /srv/abyss-machine/tmp.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=REPO_ROOT / RECEIPT_REF,
        help="Tracked text-free receipt path.",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Fail unless the existing receipt equals deterministic output.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        receipt = build_receipt(
            local_input_root=args.local_input_root,
            temporary_root=args.temporary_root,
        )
        encoded = canonical_bytes(receipt)
        if args.check:
            if not args.output.is_file():
                raise TransferSourceVisibleReviewError(
                    f"tracked receipt is absent: {args.output}"
                )
            if args.output.read_bytes() != encoded:
                raise TransferSourceVisibleReviewError(
                    f"tracked receipt differs from deterministic output: {args.output}"
                )
            print(f"source-visible transfer review receipt check passed: {args.output}")
            return 0
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(encoded)
        print(f"wrote text-free source-visible transfer review receipt: {args.output}")
        return 0
    except (OSError, TransferSourceVisibleReviewError) as exc:
        print(f"source-visible transfer review receipt error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
