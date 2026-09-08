"""Pure bounded construction of one private native segmentation proposal.

The caller supplies already-resolved source metadata, exact decoded UTF-8
text, delegated identities and an independently authorized scope. This module
performs no source, schema or configuration reads and grants no permission.
The command owner must validate input and output against its pinned native
schemas, check cross-package identity collisions, and guard publication with
the original source/context snapshot. The result is confidential source data.
"""
from __future__ import annotations

import copy
import hashlib
import json
import math
import re
import unicodedata

from validate_source_witness_foundation import (
    _source_text_layer_semantic_issues,
    _source_text_unit_v1_issues,
)


MAX_UNITS = 256
MAX_GAPS = MAX_UNITS + 1
MAX_PACKET_BYTES = 1_048_576
MAX_TEXT_BYTES = 8_388_608
SCHEMA_URI = "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json"
CONFIDENCE_MEANING = "maker-declared-boundary-confidence-not-truth-probability"
METHOD_POSTURE = "method_result_not_source_or_linguistic_truth"
VISIBILITY_RANK = {
    "public": 0, "public_metadata_only": 1, "controlled": 2,
    "local_only": 3, "restricted": 4, "unknown": 5,
}
# This operation creates positive-width, source-bearing, contiguous units.
# Milestones and empty analytic nodes belong to other native constructions.
UNIT_KINDS = frozenset({
    "document", "section", "paragraph", "physical_line", "verse_group",
    "verse_line", "sentence", "s_unit", "clause", "phrase", "surface_token",
    "syntactic_word", "multiword_token", "punctuation", "whitespace",
    "grapheme_cluster", "model_subword", "other",
})


class TextUnitProposalError(ValueError):
    """Inputs cannot construct the delegated exact, proposal-only packet."""


def _fail(message):
    # Do not include source strings, coordinates, locators or short-span hashes.
    raise TextUnitProposalError(message)


def _keys(value, fields, label):
    if type(value) is not dict or set(value) != set(fields):
        _fail(f"{label} has an incompatible field set")


def _string(value):
    return type(value) is str and bool(value.strip())


def _json_bytes(value):
    try:
        raw = json.dumps(value, ensure_ascii=False, sort_keys=True,
                         separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError, UnicodeError, RecursionError):
        _fail("proposal input is not finite UTF-8 JSON")
    if len(raw) > MAX_PACKET_BYTES:
        _fail("proposal metadata exceeds its byte budget")
    return raw


def _interval(value, label):
    start, end = value.get("start"), value.get("end")
    if type(start) is not int or type(end) is not int or not 0 <= start < end:
        _fail(f"{label} requires a positive exact integer interval")
    return start, end


def _identity(value, kind):
    suffix = r"[a-z0-9]+(?:[.-][a-z0-9]+)*" if kind == "anchor" else r"sid-[a-f0-9]{32}"
    if type(value) is not str or not re.fullmatch(r"tos\." + re.escape(kind) + r"\." + suffix, value):
        _fail("delegated native identity has an incompatible kind or shape")
    return value


def _source_inputs(packet, layer, exact_text, scope):
    if (type(packet) is not dict or type(layer) is not dict
            or packet.get("schema_version") != "tos_source_text_unit_packet_v1"
            or packet.get("content_posture") != "source_bound"
            or layer.get("schema_version") != "tos_source_text_layer_v1"
            or type(exact_text) is not str):
        _fail("proposal requires verified native source-bound packet and layer inputs")
    try:
        raw = exact_text.encode("utf-8")
    except UnicodeError:
        _fail("source text is not exact UTF-8")
    if not raw or len(raw) > MAX_TEXT_BYTES:
        _fail("source representation exceeds the bounded text budget")
    _json_bytes(packet)
    _json_bytes(layer)
    _keys(scope, {"start", "end"}, "proposal scope")
    selected = _interval(scope, "proposal scope")
    try:
        rep, source_layer, source_scope = layer["representation"], packet["source_layer"], packet["source_scope"]
        layer_scope, rights = layer["source_binding"], packet["rights_and_visibility"]
        frozen_scope = rep["text_scope"]
        frozen = _interval(frozen_scope, "source representation scope")
        digest = hashlib.sha256(raw).hexdigest()
        unicode_form = {"none": "source_preserved"}.get(rep["character_normalization"], rep["character_normalization"])
        if (not frozen[0] <= selected[0] < selected[1] <= frozen[1] <= len(exact_text)
                or frozen_scope["position_unit"] != "unicode_code_point"
                or frozen_scope["interval"] != "half_open"
                or rep["content_sha256"] != digest
                or rep["content_file_id"] != "tos.file.sha256." + digest
                or source_layer["text_layer_sha256"] != digest
                or source_layer["language"] != rep["language"]
                or source_layer["unicode_form"] != unicode_form
                or source_layer["visibility"] != rep["content_visibility"]
                or type(source_layer["publication_authorized"]) is not bool
                or source_layer["publication_authorized"] is not rep["publication_authorized"]
                or source_layer["immutable"] is not True
                or source_layer["position_unit"] != "unicode_code_point"
                or source_layer["interval"] != "half_open"
                or rep["media_type"] not in {"text/plain", "text/plain; charset=utf-8"}
                or source_layer["media_type"] not in {"text/plain", "text/plain; charset=utf-8"}):
            _fail("packet, layer, text or selected scope no longer share exact source evidence")
        if unicode_form not in {"source_preserved", "NFC", "NFD", "NFKC", "NFKD"}:
            _fail("source normalization declaration is unknown")
        frozen_text = exact_text[frozen[0]:frozen[1]]
        if unicode_form != "source_preserved" and unicodedata.normalize(unicode_form, frozen_text) != frozen_text:
            _fail("source text differs from its declared normalization without rewriting")
        for name in ("work_ref", "expression_ref", "edition_ref", "item_ref"):
            if source_scope[name] != layer_scope[name]:
                _fail("native source identity scope differs from the verified layer")
        if (source_scope["file_ref"] != layer_scope["source_file_ref"]
                or source_scope["file_sha256"] != layer_scope["source_file_sha256"]):
            _fail("native source file binding differs from the verified layer")
        for anchor in packet["anchors"]:
            start, end = anchor["selector"]["start"], anchor["selector"]["end"]
            if (type(start) is not int or type(end) is not int
                    or not frozen[0] <= start <= end <= frozen[1]
                    or anchor["source_return"]["locator_ref"] != rep["content_ref"]):
                _fail("native anchor leaves its exact representation scope or locator")
        if (rights["source_visibility"] != source_layer["visibility"]
                or any(rights[key] not in VISIBILITY_RANK for key in
                       ("source_visibility", "packet_visibility", "effective_visibility"))
                or type(rights["private_source_used"]) is not bool
                or set(rights["rights_record_refs"]) != {row["ref"] for row in rep["rights_record_refs"]}):
            _fail("native rights and source-layer closure differs")
        if VISIBILITY_RANK[rights["packet_visibility"]] > VISIBILITY_RANK["local_only"]:
            _fail("fixed local-only packet would weaken the inherited packet restriction")
        if _source_text_layer_semantic_issues(layer) or _source_text_unit_v1_issues(packet, text=exact_text):
            _fail("verified native inputs violate their pure evidence contract")
    except (KeyError, TypeError, AttributeError, IndexError):
        _fail("verified native inputs have an incompatible shape")
    return rep


def _delegated_identities(identities, packet):
    _keys(identities, {"packet_id", "scheme_id", "segmentation_id", "scope_anchor_ref",
                       "unit_slots", "gap_anchor_refs"}, "delegated identities")
    _identity(identities["packet_id"], "source-text-unit-packet")
    _identity(identities["scheme_id"], "text-unit-scheme")
    _identity(identities["segmentation_id"], "text-segmentation")
    _identity(identities["scope_anchor_ref"], "anchor")
    slots, gaps = identities["unit_slots"], identities["gap_anchor_refs"]
    if type(slots) is not list or not 1 <= len(slots) <= MAX_UNITS or type(gaps) is not list or len(gaps) > MAX_GAPS:
        _fail("delegation exceeds the bounded unit or gap budget")
    delegated = [identities[key] for key in ("packet_id", "scheme_id", "segmentation_id", "scope_anchor_ref")]
    by_unit = {}
    for slot in slots:
        _keys(slot, {"unit_id", "anchor_ref", "unit_kind"}, "unit identity slot")
        _identity(slot["unit_id"], "text-unit")
        _identity(slot["anchor_ref"], "anchor")
        if type(slot["unit_kind"]) is not str or slot["unit_kind"] not in UNIT_KINDS:
            _fail("unit slot is not a supported positive-width source-bearing kind")
        delegated.extend((slot["unit_id"], slot["anchor_ref"]))
        by_unit[slot["unit_id"]] = slot
    for identity in gaps:
        delegated.append(_identity(identity, "anchor"))
    if len(set(delegated)) != len(delegated):
        _fail("native identity delegation repeats an identity")
    existing = {packet["packet_id"]}
    for rows, key in (("schemes", "scheme_id"), ("segmentations", "segmentation_id"),
                      ("units", "unit_id"), ("anchors", "anchor_ref")):
        existing.update(row[key] for row in packet[rows])
    if existing.intersection(delegated):
        _fail("new native identities reuse an identity from the source packet")
    return by_unit


def _method_and_scheme(scheme, method, language):
    _keys(scheme, {"scheme_name", "analysis_role", "boundary_basis", "policies"}, "proposal scheme")
    _keys(method, {"maker_kind", "agent_ref", "method_name", "method_version", "software_refs",
                   "model_ref", "configuration_ref", "locale", "unicode_version", "unicode_revision",
                   "tailoring_ref", "provenance_event_ref", "made_at", "output_posture"}, "proposal method")
    if (not _string(scheme["scheme_name"])
            or scheme["analysis_role"] not in {"source_layout", "source_structure", "orthographic", "linguistic", "model_input", "interchange"}
            or scheme["boundary_basis"] not in {"source_markup", "source_layout", "unicode_default", "unicode_tailored", "rule_based", "statistical", "model", "imported", "manual"}
            or method["maker_kind"] not in {"real_human", "software", "model", "import"}
            or any(not _string(method[key]) for key in ("agent_ref", "method_name", "method_version", "configuration_ref", "provenance_event_ref", "made_at"))
            or method["locale"] not in {None, language} or method["output_posture"] != METHOD_POSTURE):
        _fail("scheme or method leaves its explicit source-language proposal posture")
    policies = scheme["policies"]
    _keys(policies, {"normalization", "punctuation", "whitespace", "line_break", "hyphenation",
                     "unreported_gaps_allowed", "overlap"}, "proposal policies")
    if (policies["normalization"] != "no-text-mutation-separate-successor-layer"
            or policies["unreported_gaps_allowed"] is not False or policies["overlap"] != "forbid"):
        _fail("bounded proposal forbids mutation, hidden gaps and overlapping units")


def _partition(scope, slots, spans, gaps, allowed_gaps):
    if type(spans) is not list or len(spans) != len(slots) or type(gaps) is not list or len(gaps) > MAX_GAPS:
        _fail("proposal must use every delegated unit and bounded explicit gaps")
    start, end = scope["start"], scope["end"]
    cursor, expected_gaps, seen = start, [], set()
    for span in spans:
        _keys(span, {"unit_id", "start", "end", "certainty", "status_reason"}, "unit span")
        identity = span["unit_id"]
        if type(identity) is not str or identity not in slots or identity in seen:
            _fail("unit span repeats or leaves its delegated identity")
        seen.add(identity)
        left, right = _interval(span, "unit span")
        if not cursor <= left < right <= end:
            _fail("unit spans are not ordered disjoint subsets of the selected scope")
        if cursor < left:
            expected_gaps.append((cursor, left))
        cursor = right
        certainty = span["certainty"]
        _keys(certainty, {"value", "meaning"}, "unit certainty")
        value = certainty["value"]
        if (type(value) not in (int, float) or not 0 <= value <= 1 or not math.isfinite(value)
                or certainty["meaning"] != CONFIDENCE_MEANING or not _string(span["status_reason"])):
            _fail("unit needs explicit bounded maker confidence and proposal reasoning")
    if cursor < end:
        expected_gaps.append((cursor, end))
    actual_gaps, used = [], set()
    for gap in gaps:
        _keys(gap, {"anchor_ref", "start", "end"}, "excluded gap")
        identity = gap["anchor_ref"]
        if type(identity) is not str or identity not in allowed_gaps or identity in used:
            _fail("excluded gap repeats or leaves its delegated anchor identity")
        used.add(identity)
        actual_gaps.append(_interval(gap, "excluded gap"))
    if actual_gaps != expected_gaps:
        _fail("explicit excluded gaps are not the exact ordered complement of the units")


def build_text_unit_proposal(*, verified_packet, verified_layer, exact_text, scope,
                             identities, spans, excluded_gaps, scheme, method):
    """Construct a native packet without I/O, mutation or authority promotion.

    ``exact_text`` is the full representation, not an offset-rebased fragment.
    Schema validation, source-read authority, global ID uniqueness and atomic
    storage remain obligations of the caller, not assertions of this function.
    """
    rep = _source_inputs(verified_packet, verified_layer, exact_text, scope)
    for value in (identities, spans, excluded_gaps, scheme, method):
        _json_bytes(value)
    slots = _delegated_identities(identities, verified_packet)
    try:
        _method_and_scheme(scheme, method, rep["language"])
    except (TypeError, KeyError):
        _fail("proposal method has an incompatible native shape")
    _partition(scope, slots, spans, excluded_gaps, identities["gap_anchor_refs"])
    source_layer = copy.deepcopy(verified_packet["source_layer"])

    def anchor(identity, role, left, right):
        return {"anchor_ref": identity, "text_layer_ref": source_layer["text_layer_ref"],
                "text_layer_sha256": source_layer["text_layer_sha256"],
                "selector": {"type": "text_position", "start": left, "end": right,
                             "position_unit": "unicode_code_point", "interval": "half_open"},
                "exact_sha256": hashlib.sha256(exact_text[left:right].encode("utf-8")).hexdigest(),
                "anchor_role": role,
                "source_return": {"required": True, "locator_ref": rep["content_ref"]}}

    scope_anchor = anchor(identities["scope_anchor_ref"], "scope", scope["start"], scope["end"])
    selected = [anchor(slots[row["unit_id"]]["anchor_ref"], "content", row["start"], row["end"]) for row in spans]
    excluded = [anchor(row["anchor_ref"], "gap", row["start"], row["end"]) for row in excluded_gaps]
    anchors = [scope_anchor, *sorted([*selected, *excluded], key=lambda row: row["selector"]["start"])]
    for ordinal, row in enumerate(anchors, start=1):
        row["ordinal"] = ordinal
    units = [{"unit_id": row["unit_id"], "unit_version": 1, "supersedes_unit_ref": None,
              "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
              "unit_kind": slots[row["unit_id"]]["unit_kind"], "surface_posture": "source_bearing",
              "continuity": "contiguous", "ordered_anchor_refs": [slots[row["unit_id"]]["anchor_ref"]],
              "parent_unit_refs": [], "ordered_child_unit_refs": [], "boundary_posture": "method_proposed",
              "certainty": copy.deepcopy(row["certainty"]), "status_reason": row["status_reason"],
              "source_text_mutated": False, "semantic_promotion": False} for row in spans]
    rights = copy.deepcopy(verified_packet["rights_and_visibility"])
    rights.update(packet_visibility="local_only", publication_authorized=False,
                  effective_visibility=max((rights["source_visibility"], "local_only"), key=VISIBILITY_RANK.__getitem__))
    packet = {
        "$schema": SCHEMA_URI, "schema_version": "tos_source_text_unit_packet_v1",
        "packet_id": identities["packet_id"], "packet_version": 1, "supersedes_packet_ref": None,
        "content_posture": "source_bound", "source_scope": copy.deepcopy(verified_packet["source_scope"]),
        "source_layer": source_layer,
        "schemes": [{"scheme_id": identities["scheme_id"], "scheme_version": 1, "supersedes_scheme_ref": None,
                     "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
                     **copy.deepcopy(scheme), "unit_kinds": list(dict.fromkeys(row["unit_kind"] for row in units)),
                     "method": copy.deepcopy(method),
                     "authority_limit": {"algorithmic_output_is_source_truth": False,
                                         "algorithmic_output_is_linguistic_truth": False,
                                         "model_subword_is_lexeme": False, "unit_identity_is_semantic_identity": False,
                                         "text_mutation_allowed": False}}],
        "anchors": anchors, "units": units,
        "segmentations": [{"segmentation_id": identities["segmentation_id"], "segmentation_version": 1,
                           "supersedes_segmentation_ref": None,
                           "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
                           "scheme_ref": identities["scheme_id"], "ordered_unit_refs": [row["unit_id"] for row in units],
                           "coverage": {"scope_anchor_ref": identities["scope_anchor_ref"],
                                        "coverage_posture": "declared_partial" if excluded_gaps else "exhaustive_nonoverlapping",
                                        "excluded_anchor_refs": [row["anchor_ref"] for row in excluded_gaps],
                                        "unreported_gaps_allowed": False, "overlap_requires_declaration": True,
                                        "source_reconstruction_required": True},
                           "status": "proposed", "status_reason": "Exact delegated span proposal; no boundary review or source acceptance is asserted.",
                           "maker": copy.deepcopy(method), "competing_segmentation_refs": [], "review_refs": [],
                           "declared_uses": ["source_observation"], "source_text_authority": False,
                           "linguistic_authority": False, "semantic_authority": False}],
        "reviews": [], "projections": [], "rights_and_visibility": rights,
        "authority_boundary": {"tree_role": "orientation", "graph_role": "relation", "source_role": "authority",
                               "validators_prove_mechanics_not_truth": True,
                               "segmentation_is_method_result_not_source_truth": True,
                               "source_unit_is_not_lexeme_sign_or_concept": True,
                               "model_token_is_not_linguistic_token": True, "projection_is_owner_truth": False,
                               "legacy_bulk_migration_authorized": False},
    }
    _json_bytes(packet)
    if _source_text_unit_v1_issues(packet, text=exact_text):
        _fail("constructed proposal violates the existing native evidence contract")
    return packet
