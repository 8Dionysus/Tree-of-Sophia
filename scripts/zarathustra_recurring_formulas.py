"""Source-bound repeated lexical sequences; not a semantic-equivalence detector.

The caller supplies complete source-spine context and surface rows.  Exact
strings in the returned objects are private material.  Only a caller that
strips them may persist a public or tracked projection.

Identity is a deterministic *derived candidate snapshot*, not a concept ID.
The source-spine IDs and context-local offsets remain the authority for text.
"""

from __future__ import annotations

from collections import Counter, defaultdict
from hashlib import sha256
import json
from typing import Any


METHOD_VERSION = "zarathustra-recurring-formulas-v1"
LEXICAL_KINDS = frozenset({"word", "number"})


def _digest(value: str) -> str:
    return sha256(value.encode("utf-8")).hexdigest()


def _canonical(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _identifier(kind: str, binding: Any) -> str:
    return f"tos.{kind}.sid-{_digest(_canonical([METHOD_VERSION, binding]))[:32]}"


def _validated_streams(
    contexts: list[dict], surfaces: list[dict]
) -> tuple[list[list[dict]], dict[str, dict], dict]:
    """Validate source return before ignoring nonlexical units for comparison."""
    by_ref: dict[str, dict] = {}
    orders: set[tuple[str, int]] = set()
    for context in contexts:
        ref = context["context_unit_ref"]
        if ref in by_ref:
            raise ValueError(f"duplicate context reference: {ref}")
        order = (context["language"], int(context["witness_order"]))
        if order in orders:
            raise ValueError(f"duplicate witness order: {order}")
        orders.add(order)
        exact = context["exact_text"]
        if context.get("exact_sha256", _digest(exact)) != _digest(exact):
            raise ValueError(f"context text digest mismatch: {ref}")
        by_ref[ref] = context

    context_surfaces: dict[str, list[dict]] = defaultdict(list)
    seen_surface_refs: set[str] = set()
    for surface in surfaces:
        ref = surface["context_unit_ref"]
        sid = surface["surface_unit_id"]
        if ref not in by_ref:
            raise ValueError(f"surface has unknown context: {sid}")
        if sid in seen_surface_refs:
            raise ValueError(f"duplicate surface reference: {sid}")
        seen_surface_refs.add(sid)
        context_surfaces[ref].append(surface)

    streams: list[list[dict]] = []
    current: list[dict] | None = None
    previous: dict | None = None
    lexical_count = 0
    verse_joins = 0
    context_with_lexical = 0
    for context in sorted(
        contexts,
        key=lambda row: (row["language"], int(row["witness_order"]), row["context_unit_ref"]),
    ):
        ref = context["context_unit_ref"]
        ordered = sorted(context_surfaces[ref], key=lambda row: int(row["start_offset"]))
        end = 0
        tokens: list[dict] = []
        for surface in ordered:
            start, next_end = int(surface["start_offset"]), int(surface["end_offset"])
            if start != end or next_end <= start or next_end > len(context["exact_text"]):
                raise ValueError(f"surface coverage gap, overlap, or invalid extent: {ref}")
            if surface.get("language", context["language"]) != context["language"]:
                raise ValueError(f"surface/context language mismatch: {ref}")
            if int(surface.get("part", context["part"])) != int(context["part"]):
                raise ValueError(f"surface/context part mismatch: {ref}")
            exact = context["exact_text"][start:next_end]
            if surface["exact_text"] != exact:
                raise ValueError(f"surface text differs from context: {surface['surface_unit_id']}")
            if surface.get("exact_sha256", _digest(exact)) != _digest(exact):
                raise ValueError(f"surface text digest mismatch: {surface['surface_unit_id']}")
            end = next_end
            if surface["surface_kind"] not in LEXICAL_KINDS:
                continue
            normalized = surface["normalized_text"]
            if not isinstance(normalized, str) or not normalized:
                raise ValueError(f"empty lexical normalization: {surface['surface_unit_id']}")
            if surface.get("normalized_sha256", _digest(normalized)) != _digest(normalized):
                raise ValueError(f"normalized text digest mismatch: {surface['surface_unit_id']}")
            tokens.append(surface)
        if end != len(context["exact_text"]):
            raise ValueError(f"incomplete surface coverage: {ref}")
        context_with_lexical += bool(tokens)
        lexical_count += len(tokens)
        join_verse = (
            previous is not None
            and current is not None
            and bool(tokens)
            and previous["unit_kind"] == context["unit_kind"] == "verse_line"
            and previous["language"] == context["language"]
            and previous["part"] == context["part"]
            and previous["reading_ref"] == context["reading_ref"]
            and int(previous["witness_order"]) + 1 == int(context["witness_order"])
        )
        if join_verse:
            current.extend(tokens)
            verse_joins += 1
        elif tokens:
            current = list(tokens)
            streams.append(current)
        else:
            # Even a tokenless context is a real source barrier, not permission
            # to join the surrounding lexical material.
            current = None
        previous = context
    coverage = {
        "input_contexts": len(contexts),
        "input_surface_units": len(surfaces),
        "lexical_surface_units": lexical_count,
        "contexts_with_lexical_units": context_with_lexical,
        "lexical_streams": len(streams),
        "adjacent_verse_context_joins": verse_joins,
        "source_surface_reconstruction_exact": True,
        "readings_by_language": {
            language: len({(row["part"], row["reading_ref"]) for row in contexts
                           if row["language"] == language and not row["reading_ref"].endswith(".unscoped-technical")})
            for language in sorted({row["language"] for row in contexts})
        },
        "unscoped_technical_groups_by_language": {
            language: len({(row["part"], row["reading_ref"]) for row in contexts
                           if row["language"] == language and row["reading_ref"].endswith(".unscoped-technical")})
            for language in sorted({row["language"] for row in contexts})
        },
    }
    return streams, by_ref, coverage


def _independent_count(positions: list[tuple[int, int]], length: int) -> int:
    """Greedily count non-overlapping matches independently in each stream."""
    count = 0
    ends: dict[int, int] = {}
    for stream, start in sorted(positions):
        if start >= ends.get(stream, 0):
            count += 1
            ends[stream] = start + length
    return count


def _shared_extension(
    streams: list[list[dict]], positions: list[tuple[int, int]], length: int, direction: str
) -> bool:
    values: set[str] = set()
    for stream_index, start in positions:
        offset = start - 1 if direction == "left" else start + length
        if offset < 0 or offset >= len(streams[stream_index]):
            return False
        values.add(streams[stream_index][offset]["normalized_text"])
        if len(values) > 1:
            return False
    return bool(values)


def _source_spans(tokens: list[dict], contexts: dict[str, dict]) -> list[dict]:
    spans: list[dict] = []
    for token in tokens:
        ref = token["context_unit_ref"]
        if spans and spans[-1]["context_unit_ref"] == ref:
            spans[-1]["end_offset"] = int(token["end_offset"])
        else:
            spans.append({
                "context_unit_ref": ref,
                "start_offset": int(token["start_offset"]),
                "end_offset": int(token["end_offset"]),
            })
    for span in spans:
        context = contexts[span["context_unit_ref"]]
        span["exact_text"] = context["exact_text"][span["start_offset"]:span["end_offset"]]
        span["exact_sha256"] = _digest(span["exact_text"])
    return spans


def _quality_flags(tokens: list[dict], contexts: dict[str, dict]) -> list[str]:
    """Recognize suspect letter spacing without repairing or merging tokens."""
    longest = run = 0
    previous: dict | None = None
    for token in tokens:
        exact = token["exact_text"]
        if len(exact) != 1 or not exact.isalpha():
            run, previous = 0, None
            continue
        contiguous = False
        if previous is not None and token["context_unit_ref"] == previous["context_unit_ref"]:
            separator = contexts[token["context_unit_ref"]]["exact_text"][previous["end_offset"]:token["start_offset"]]
            contiguous = bool(separator) and separator.isspace()
        run = run + 1 if contiguous else 1
        longest = max(longest, run)
        previous = token
    flags = []
    if longest >= 3:
        flags.append("suspected_letter_spacing_not_word_sequence")
    if len({token.get("sentence_id") for token in tokens}) > 1:
        flags.append("crosses_sentence_boundary")
    return flags


def build_formulas(
    contexts: list[dict],
    surfaces: list[dict],
    *,
    min_tokens: int = 4,
    max_tokens: int = 32,
    min_occurrences: int = 2,
) -> tuple[list[dict], list[dict], list[dict], dict]:
    """Return ``families, memberships, relations, receipt`` for an entire scope.

    Normalized lexical sequences are compared only within one language.
    Source-spine normalization is reused verbatim, with no lemmatization,
    spelling repair, stop-word removal, translation, or semantic expansion.
    Paragraphs are barriers. Adjacent verse lines may form one lexical stream
    only when language, part and reading match and witness orders differ by 1.

    A repeated sequence is suppressed if *all* of its occurrences share a
    left or right extension: that longer phrase covers exactly the same sites.
    An independently recurring shorter phrase is retained. At ``max_tokens``
    only shared-left subwindows are suppressed; a common right extension is
    reported as a length-capped candidate, not as a complete maximal formula.
    At least ``min_occurrences`` non-overlapping occurrences are required.
    """
    if min_tokens < 4 or max_tokens < min_tokens or max_tokens > 512:
        raise ValueError("require 4 <= min_tokens <= max_tokens <= 512")
    if min_occurrences < 2:
        raise ValueError("min_occurrences must be at least 2")
    streams, by_ref, coverage = _validated_streams(contexts, surfaces)
    candidates: dict[tuple[str, tuple[str, ...]], list[tuple[int, int]]] = defaultdict(list)
    for stream_index, stream in enumerate(streams):
        language = by_ref[stream[0]["context_unit_ref"]]["language"]
        forms = [token["normalized_text"] for token in stream]
        for start in range(len(stream) - min_tokens + 1):
            candidates[(language, tuple(forms[start:start + min_tokens]))].append((stream_index, start))

    retained: list[tuple[str, tuple[str, ...], list[tuple[int, int]], bool]] = []
    candidate_count = 0
    suppressed_count = 0
    for length in range(min_tokens, max_tokens + 1):
        next_candidates: dict[tuple[str, tuple[str, ...]], list[tuple[int, int]]] = defaultdict(list)
        for (language, forms), positions in candidates.items():
            independent_count = _independent_count(positions, length)
            if independent_count < min_occurrences:
                continue
            candidate_count += 1
            left_extension = (
                _shared_extension(streams, positions, length, "left")
                and _independent_count([(index, start - 1) for index, start in positions], length + 1) == independent_count
            )
            right_extension = (
                _shared_extension(streams, positions, length, "right")
                and _independent_count(positions, length + 1) == independent_count
            )
            if not left_extension and (not right_extension or length == max_tokens):
                retained.append((language, forms, positions, right_extension and length == max_tokens))
            else:
                suppressed_count += 1
            if length == max_tokens:
                continue
            for stream_index, start in positions:
                stream = streams[stream_index]
                if start + length < len(stream):
                    extension = stream[start + length]["normalized_text"]
                    next_candidates[(language, (*forms, extension))].append((stream_index, start))
        candidates = next_candidates
        if not candidates:
            break

    families: list[dict] = []
    memberships: list[dict] = []
    relations: list[dict] = []
    for language, forms, positions, capped in sorted(retained, key=lambda row: (row[0], row[1])):
        occurrence_bindings = [
            [token["surface_unit_id"] for token in streams[index][start:start + len(forms)]]
            for index, start in sorted(positions)
        ]
        formula_id = _identifier("formula-candidate", [language, forms, occurrence_bindings])
        formula_memberships: list[dict] = []
        exact_signatures: dict[str, str] = {}
        for stream_index, start in sorted(positions):
            tokens = streams[stream_index][start:start + len(forms)]
            context = by_ref[tokens[0]["context_unit_ref"]]
            spans = _source_spans(tokens, by_ref)
            refs = [token["surface_unit_id"] for token in tokens]
            occurrence_id = _identifier("formula-occurrence", [formula_id, refs])
            # Context identities are anchors, not part of exact-text equality.
            exact_signatures[occurrence_id] = _canonical([span["exact_text"] for span in spans])
            single = len(spans) == 1
            flags = _quality_flags(tokens, by_ref)
            quality_deferred = "suspected_letter_spacing_not_word_sequence" in flags
            bundle = [{key: span[key] for key in ("context_unit_ref", "start_offset", "end_offset", "exact_sha256")}
                      for span in spans]
            formula_memberships.append({
                "formula_occurrence_id": occurrence_id,
                "formula_id": formula_id,
                "language": language,
                "part": int(context["part"]),
                "reading_ref": context["reading_ref"],
                "reading_scope_kind": "unscoped_technical" if context["reading_ref"].endswith(".unscoped-technical") else "reading",
                "witness_order": int(context["witness_order"]),
                "context_unit_ref": tokens[0]["context_unit_ref"],
                "start_offset": int(tokens[0]["start_offset"]),
                "end_offset": int(tokens[-1]["end_offset"]) if single else None,
                "surface_unit_refs": refs,
                "sentence_unit_refs": list(dict.fromkeys(token["sentence_id"] for token in tokens if token.get("sentence_id"))),
                "source_spans": spans,
                "exact_text": spans[0]["exact_text"] if single else None,
                "exact_sha256": spans[0]["exact_sha256"] if single else None,
                "source_span_bundle_sha256": _digest(_canonical(bundle)),
                "normalized_sha256": _digest(_canonical(list(forms))),
                "display_text": "\n".join(span["exact_text"] for span in spans),
                "display_joined_across_contexts": not single,
                "crosses_sentence_boundary": len({token.get("sentence_id") for token in tokens}) > 1,
                "quality_flags": flags,
                "quality_status": "deferred" if quality_deferred else "proposed",
                "status": "deferred" if quality_deferred else "proposed",
            })
        formula_memberships.sort(key=lambda row: (row["witness_order"], row["start_offset"], row["formula_occurrence_id"]))
        exact_variants = len(set(exact_signatures.values()))
        deferred_count = sum(row["quality_status"] == "deferred" for row in formula_memberships)
        families.append({
            "formula_id": formula_id,
            "language": language,
            "normalized_tokens": list(forms),
            "normalized_sha256": _digest(_canonical(list(forms))),
            "token_count": len(forms),
            "occurrence_count": len(formula_memberships),
            "independent_occurrence_count": _independent_count(positions, len(forms)),
            "reading_count": len({(row["part"], row["reading_ref"]) for row in formula_memberships if row["reading_scope_kind"] == "reading"}),
            "source_scope_count": len({(row["part"], row["reading_ref"]) for row in formula_memberships}),
            "exact_variant_count": exact_variants,
            "has_cross_context_occurrences": any(row["display_joined_across_contexts"] for row in formula_memberships),
            "right_extension_capped": capped,
            "match_kind": "repeats_exact" if exact_variants == 1 else "reprises_normalized",
            "identity_posture": "derived_candidate_membership_snapshot",
            "quality_flags": sorted({flag for row in formula_memberships for flag in row["quality_flags"]}),
            "quality_deferred_occurrence_count": deferred_count,
            "quality_status": "deferred" if deferred_count else "proposed",
            "status": "deferred" if deferred_count else "proposed",
        })
        memberships.extend(formula_memberships)
        for source, target in zip(formula_memberships, formula_memberships[1:]):
            source_ref, target_ref = source["formula_occurrence_id"], target["formula_occurrence_id"]
            kind = "repeats_exact" if exact_signatures[source_ref] == exact_signatures[target_ref] else "reprises_normalized"
            relations.append({
                "relation_id": _identifier("formula-relation-candidate", [kind, source_ref, target_ref]),
                "relation_type": kind,
                "formula_id": formula_id,
                "source_occurrence_ref": source_ref,
                "target_occurrence_ref": target_ref,
                "reason_codes": ["identical_normalized_lexical_sequence", "consecutive_family_occurrences_in_witness_order"],
                "status": "deferred" if source["quality_status"] == "deferred" or target["quality_status"] == "deferred" else "proposed",
            })
    covered = {ref for row in memberships for ref in row["surface_unit_refs"]}
    receipt = {
        "method": METHOD_VERSION,
        "settings": {"min_tokens": min_tokens, "max_tokens": max_tokens, "min_occurrences": min_occurrences},
        "coverage": coverage,
        "counts": {
            "repeated_sequences_before_maximal_suppression": candidate_count,
            "suppressed_nested_sequences": suppressed_count,
            "formula_families": len(families),
            "formula_occurrences": len(memberships),
            "formula_relations": len(relations),
            "lexical_units_in_retained_formulas": len(covered),
            "length_capped_families": sum(row["right_extension_capped"] for row in families),
            "families_by_language": dict(sorted(Counter(row["language"] for row in families).items())),
            "match_kinds": dict(sorted(Counter(row["match_kind"] for row in families).items())),
            "quality_statuses": dict(sorted(Counter(row["quality_status"] for row in families).items())),
        },
        "limitations": [
            "Lexical identity after the existing lossy source-spine normalization is not semantic equivalence.",
            "Punctuation and whitespace do not participate in matching; they remain exact in source spans.",
            "No lemmatization, OCR repair, dehyphenation, letter-spacing repair, or synonym matching is performed.",
            "Runs of three or more whitespace-separated single letters are quality-deferred; token_count is not a count of reconstructed words.",
            "Sentence-crossing repetitions are explicitly flagged, not asserted to be syntactic phrases.",
            "Paragraph and nonconsecutive-context boundaries are barriers; only adjacent verse lines may be joined.",
            "Stanza boundaries absent from the input context metadata cannot be inferred.",
            "Repeated formulas shorter than min_tokens are outside this detector's declared scope.",
            "A right_extension_capped family is only a prefix of a longer common sequence.",
            "Formula-free passages were scanned; no membership there does not assert the absence of a motif.",
            "No cross-language equivalence or altered-word near-variant relation is inferred.",
        ],
        "source_text_included": False,
        "separate_returned_rows_contain_private_source_text": True,
        "semantic_promotion": False,
        "human_review_count": 0,
        "accepted_candidate_count": 0,
    }
    return families, memberships, relations, receipt
