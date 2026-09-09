"""Pure-constructor checks using synthetic native evidence only.

No retained private text, model runtime, owner-local store or real source
packet is accessed. Schema success is mechanical, not segmentation approval.
"""
from __future__ import annotations

import copy
import json
import math
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import unicodedata

from jsonschema import Draft202012Validator, FormatChecker

from test_native_text_binding import NativeTextBindingFixture, REPO_ROOT, digest
from source_text_unit_proposal import (
    CONFIDENCE_MEANING,
    MAX_GAPS,
    MAX_UNITS,
    TextUnitProposalError,
    build_text_unit_proposal,
)
from validate_source_witness_foundation import _source_text_unit_v1_issues


def opaque(kind, number):
    return "tos." + kind + ".sid-" + "d" * 16 + format(number, "016x")


def anchor(number):
    return opaque("anchor", number)


class TextUnitProposalTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        schema = json.loads((REPO_ROOT / "ToS/contracts/source-text-unit-packet-v1.schema.json").read_bytes())
        Draft202012Validator.check_schema(schema)
        cls.validator = Draft202012Validator(schema, format_checker=FormatChecker())

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tos-unit-proposal-test-")
        self.addCleanup(self.temporary.cleanup)
        self.fixture = NativeTextBindingFixture(Path(self.temporary.name))
        original_scheme = self.fixture.packet["schemes"][0]
        self.inputs = {
            "verified_packet": copy.deepcopy(self.fixture.packet),
            "verified_layer": copy.deepcopy(self.fixture.layer),
            "exact_text": self.fixture.text,
            "scope": {"start": 3, "end": 10},
            "identities": {
                "packet_id": opaque("source-text-unit-packet", 1),
                "scheme_id": opaque("text-unit-scheme", 1),
                "segmentation_id": opaque("text-segmentation", 1),
                "scope_anchor_ref": anchor(1),
                "unit_slots": [{"unit_id": opaque("text-unit", index),
                                "anchor_ref": anchor(index + 10), "unit_kind": "surface_token"}
                               for index in (1, 2)],
                "gap_anchor_refs": [anchor(index + 20) for index in range(3)],
            },
            "spans": [self.span(1, 3, 8, 0.37), self.span(2, 9, 10, 0.81)],
            "excluded_gaps": [{"anchor_ref": anchor(20), "start": 8, "end": 9}],
            "scheme": {key: copy.deepcopy(original_scheme[key]) for key in
                       ("scheme_name", "analysis_role", "boundary_basis", "policies")},
            "method": copy.deepcopy(original_scheme["method"]),
        }
        self.inputs["scheme"].update(scheme_name="Synthetic explicit-span proposal", boundary_basis="manual")
        self.inputs["method"].update(method_name="Synthetic explicit-span proposal", method_version="1",
            configuration_ref="ToS/source-witnesses/owner-local/sid-" + "d" * 32 + "/synthetic/source-create-owner-configuration.json",
            provenance_event_ref="tos.event.synthetic.text-unit-proposal", made_at="2026-09-08T00:00:00Z")

    @staticmethod
    def span(number, start, end, certainty=0.37):
        return {"unit_id": opaque("text-unit", number), "start": start, "end": end,
                "certainty": {"value": certainty, "meaning": CONFIDENCE_MEANING},
                "status_reason": "Synthetic span selection only; no accepted linguistic analysis."}

    def build(self, **updates):
        return build_text_unit_proposal(**{**self.inputs, **updates})

    def assert_valid(self, packet, text=None):
        self.validator.validate(packet)
        self.assertEqual(_source_text_unit_v1_issues(packet, text=self.inputs["exact_text"] if text is None else text), [])

    def assert_rejected(self, inputs):
        with self.assertRaises(TextUnitProposalError):
            build_text_unit_proposal(**inputs)

    def exact_inputs(self, text, start, end):
        """Rebind synthetic in-memory source metadata, never retained bytes."""
        inputs = copy.deepcopy(self.inputs)
        inputs["exact_text"], inputs["scope"] = text, {"start": start, "end": end}
        rep = inputs["verified_layer"]["representation"]
        representation_digest = digest(text.encode("utf-8"))
        rep.update(content_sha256=representation_digest, content_file_id="tos.file.sha256." + representation_digest)
        rep["text_scope"].update(start=start, end=end)
        packet = inputs["verified_packet"]
        packet["source_layer"]["text_layer_sha256"] = representation_digest
        for row in packet["anchors"]:
            left, right = {"scope": (start, end), "content": (start, start + 1), "gap": (start + 1, end)}[row["anchor_role"]]
            row["selector"].update(start=left, end=right)
            row.update(text_layer_sha256=representation_digest,
                       exact_sha256=digest(text[left:right].encode("utf-8")))
        return inputs

    def test_nonzero_scope_multiple_units_preserves_nfd_and_declares_gap(self):
        packet = self.build()
        self.assert_valid(packet)
        self.assertEqual(packet["source_scope"], self.inputs["verified_packet"]["source_scope"])
        self.assertEqual(packet["source_layer"], self.inputs["verified_packet"]["source_layer"])
        by_id = {row["anchor_ref"]: row for row in packet["anchors"]}
        self.assertEqual(by_id[anchor(11)]["selector"]["start"], 3)
        self.assertEqual(by_id[anchor(11)]["exact_sha256"], digest("cafe\u0301".encode()))
        self.assertNotEqual(by_id[anchor(11)]["exact_sha256"], digest("caf\u00e9".encode()))
        self.assertEqual(by_id[anchor(20)]["exact_sha256"], digest(b" "))
        self.assertEqual(by_id[anchor(1)]["exact_sha256"], digest(self.fixture.text[3:10].encode()))
        self.assertNotEqual(by_id[anchor(1)]["exact_sha256"], digest(self.fixture.content))
        coverage = packet["segmentations"][0]["coverage"]
        self.assertEqual(coverage["coverage_posture"], "declared_partial")
        self.assertEqual(coverage["excluded_anchor_refs"], [anchor(20)])
        self.assertEqual([row["certainty"]["value"] for row in packet["units"]], [0.37, 0.81])

    def test_explicit_layer_only_first_segmentation_has_no_predecessor_packet(self):
        inputs = copy.deepcopy(self.inputs)
        del inputs['verified_packet']
        inputs['verified_layer_binding'] = {'schema_version': 'tos_native_text_layer_binding_v1',
            'text_layer': copy.deepcopy(self.fixture.binding['text_layer']),
            'source_record_refs': copy.deepcopy(self.fixture.binding['source_record_refs'])}
        packet = build_text_unit_proposal(**inputs)
        self.assert_valid(packet)
        self.assertIsNone(packet['supersedes_packet_ref'])
        self.assertEqual(packet['source_scope'], self.fixture.packet['source_scope'])
        self.assertEqual(packet['source_layer'], self.fixture.packet['source_layer'])
        self.assertEqual(packet['segmentations'][0]['status'], 'proposed')
        self.assertTrue(packet['rights_and_visibility']['private_source_used'])
        self.assertFalse(packet['rights_and_visibility']['publication_authorized'])
        self.assertEqual(packet['rights_and_visibility']['packet_visibility'], 'local_only')
        with self.assertRaises(TextUnitProposalError):
            build_text_unit_proposal(**{**inputs, 'verified_packet': self.fixture.packet})
        bad = copy.deepcopy(inputs)
        bad['verified_layer_binding']['text_layer']['layer_id'] += '.different'
        with self.assertRaises(TextUnitProposalError):
            build_text_unit_proposal(**bad)

    def test_prefix_interior_suffix_gaps_are_explicit_and_source_ordered(self):
        spans = [self.span(1, 4, 5), self.span(2, 6, 7)]
        gaps = [{"anchor_ref": anchor(20 + index), "start": start, "end": end}
                for index, (start, end) in enumerate(((3, 4), (5, 6), (7, 10)))]
        packet = self.build(spans=spans, excluded_gaps=gaps)
        self.assert_valid(packet)
        self.assertEqual([row["ordinal"] for row in packet["anchors"]], list(range(1, 7)))
        self.assertEqual([row["selector"]["start"] for row in packet["anchors"]], [3, 3, 4, 5, 6, 7])
        self.assertEqual(packet["segmentations"][0]["coverage"]["excluded_anchor_refs"], [anchor(20), anchor(21), anchor(22)])

    def test_exhaustive_partition_has_no_synthetic_empty_gap(self):
        packet = self.build(spans=[self.span(1, 3, 8), self.span(2, 8, 10)], excluded_gaps=[])
        self.assert_valid(packet)
        self.assertEqual(packet["segmentations"][0]["coverage"]["coverage_posture"], "exhaustive_nonoverlapping")
        self.assertEqual(packet["segmentations"][0]["coverage"]["excluded_anchor_refs"], [])
        self.assertFalse(any(row["anchor_role"] == "gap" for row in packet["anchors"]))

    def test_constructor_is_deterministic_io_free_and_does_not_alias_inputs(self):
        before = copy.deepcopy(self.inputs)
        with patch("builtins.open", side_effect=AssertionError("constructor attempted file I/O")), \
                patch.object(Path, "open", side_effect=AssertionError("constructor attempted path I/O")), \
                patch("os.open", side_effect=AssertionError("constructor attempted descriptor I/O")):
            first, second = self.build(), self.build()
        self.assertEqual(first, second)
        self.assertEqual(self.inputs, before)
        first["source_scope"]["work_ref"] = "mutated"
        first["source_layer"]["language"] = "mutated"
        first["rights_and_visibility"]["rights_record_refs"].append("mutated")
        first["schemes"][0]["method"]["software_refs"].append("mutated")
        first["units"][0]["certainty"]["value"] = 1
        self.assertEqual(self.inputs, before)
        self.assertEqual(self.build(), second)

    def test_every_new_state_is_proposal_only_and_ids_are_retained(self):
        packet = self.build()
        self.assert_valid(packet)
        self.assertEqual(packet["packet_id"], self.inputs["identities"]["packet_id"])
        self.assertEqual(packet["packet_version"], 1)
        self.assertIsNone(packet["supersedes_packet_ref"])
        self.assertEqual(packet["reviews"], [])
        self.assertEqual(packet["projections"], [])
        for unit, span in zip(packet["units"], self.inputs["spans"]):
            self.assertEqual(unit["unit_id"], span["unit_id"])
            self.assertEqual(unit["boundary_posture"], "method_proposed")
            self.assertEqual(unit["unit_version"], 1)
            self.assertIsNone(unit["supersedes_unit_ref"])
            self.assertFalse(unit["source_text_mutated"])
            self.assertFalse(unit["semantic_promotion"])
        segmentation = packet["segmentations"][0]
        self.assertEqual(segmentation["status"], "proposed")
        self.assertEqual(segmentation["review_refs"], [])
        for key in ("source_text_authority", "linguistic_authority", "semantic_authority"):
            self.assertFalse(segmentation[key])
        self.assertEqual(segmentation["maker"], self.inputs["method"])

    def test_public_source_stays_public_but_new_packet_is_local_only(self):
        self.fixture.make_public()
        packet = self.build(verified_packet=self.fixture.packet, verified_layer=self.fixture.layer)
        self.assert_valid(packet)
        self.assertEqual(packet["source_layer"], self.fixture.packet["source_layer"])
        self.assertTrue(packet["source_layer"]["publication_authorized"])
        rights = packet["rights_and_visibility"]
        self.assertEqual(rights["source_visibility"], "public")
        self.assertEqual(rights["packet_visibility"], "local_only")
        self.assertEqual(rights["effective_visibility"], "local_only")
        self.assertFalse(rights["private_source_used"])
        self.assertFalse(rights["publication_authorized"])

    def test_restricted_or_unknown_source_visibility_is_not_lowered(self):
        for visibility in ("restricted", "unknown"):
            with self.subTest(visibility=visibility):
                inputs = copy.deepcopy(self.inputs)
                inputs["verified_layer"]["representation"]["content_visibility"] = visibility
                inputs["verified_packet"]["source_layer"]["visibility"] = visibility
                inputs["verified_packet"]["rights_and_visibility"].update(
                    source_visibility=visibility, effective_visibility=visibility)
                packet = build_text_unit_proposal(**inputs)
                self.assert_valid(packet)
                self.assertEqual(packet["rights_and_visibility"]["effective_visibility"], visibility)

    def test_stronger_packet_restriction_cannot_be_relabeled_local_only(self):
        for visibility in ("restricted", "unknown"):
            with self.subTest(visibility=visibility):
                inputs = copy.deepcopy(self.inputs)
                inputs["verified_packet"]["rights_and_visibility"].update(
                    packet_visibility=visibility, effective_visibility=visibility)
                self.assert_rejected(inputs)

    def test_overlap_order_empty_range_and_invalid_integer_coordinates_reject(self):
        changes = (
            [{"start": 3, "end": 9}, {}],
            [{"start": 9, "end": 10}, {"start": 3, "end": 8}],
            [{"start": 3, "end": 3}, {}],
            [{"start": 4, "end": 3}, {}],
            [{"start": 2}, {}],
            [{}, {"end": 11}],
            [{"start": True}, {}],
            [{"start": 3.0}, {}],
        )
        for replacements in changes:
            with self.subTest(replacements=replacements):
                inputs = copy.deepcopy(self.inputs)
                for span, fields in zip(inputs["spans"], replacements):
                    span.update(fields)
                self.assert_rejected(inputs)

    def test_gaps_must_be_exact_explicit_complement_not_hidden_or_redundant(self):
        changes = (
            [],
            [{"anchor_ref": anchor(20), "start": 7, "end": 9}],
            [{"anchor_ref": anchor(20), "start": 8, "end": 8}],
            [{"anchor_ref": anchor(20), "start": 8, "end": 10}],
            [{"anchor_ref": anchor(20), "start": 8, "end": 9}, {"anchor_ref": anchor(21), "start": 8, "end": 9}],
            [{"anchor_ref": anchor(99), "start": 8, "end": 9}],
        )
        for gaps in changes:
            with self.subTest(gaps=gaps):
                self.assert_rejected({**self.inputs, "excluded_gaps": gaps})

    def test_scope_never_rebases_or_widens_the_verified_representation(self):
        for scope in ({"start": 0, "end": 10}, {"start": 3, "end": 11},
                      {"start": 3, "end": 3}, {"start": True, "end": 10},
                      {"start": 3, "end": 10, "position_unit": "byte"}):
            with self.subTest(scope=scope):
                self.assert_rejected({**self.inputs, "scope": scope})

    def test_native_identity_delegation_is_exact_unique_and_does_not_reuse_source(self):
        mutations = (
            lambda row: row.update(packet_id="tos.source-text-unit-packet.not-opaque"),
            lambda row: row.update(packet_id=self.fixture.packet["packet_id"]),
            lambda row: row.update(scope_anchor_ref=self.fixture.scope_id),
            lambda row: row["unit_slots"][0].update(unit_id=self.fixture.packet["units"][0]["unit_id"]),
            lambda row: row["unit_slots"][1].update(unit_id=row["unit_slots"][0]["unit_id"]),
            lambda row: row["unit_slots"][1].update(anchor_ref=row["unit_slots"][0]["anchor_ref"]),
            lambda row: row["gap_anchor_refs"].append(row["scope_anchor_ref"]),
            lambda row: row["unit_slots"][0].update(unit_kind="milestone"),
            lambda row: row["unit_slots"][0].update(unit_kind="empty_analytic_node"),
        )
        for index, mutate in enumerate(mutations):
            with self.subTest(case=index):
                inputs = copy.deepcopy(self.inputs)
                mutate(inputs["identities"])
                self.assert_rejected(inputs)
        for spans in (self.inputs["spans"][:1], [self.inputs["spans"][0]] * 2,
                      [self.span(99, 3, 8), self.inputs["spans"][1]]):
            with self.subTest(spans=spans):
                self.assert_rejected({**self.inputs, "spans": spans})

    def test_confidence_is_explicit_finite_and_never_truth_probability(self):
        for confidence in ({"value": True, "meaning": CONFIDENCE_MEANING},
                           {"value": math.nan, "meaning": CONFIDENCE_MEANING},
                           {"value": math.inf, "meaning": CONFIDENCE_MEANING},
                           {"value": 10 ** 400, "meaning": CONFIDENCE_MEANING},
                           {"value": -0.01, "meaning": CONFIDENCE_MEANING},
                           {"value": 1.01, "meaning": CONFIDENCE_MEANING},
                           {"value": 0.5, "meaning": "truth probability"}, {}):
            with self.subTest(confidence=confidence):
                inputs = copy.deepcopy(self.inputs)
                inputs["spans"][0]["certainty"] = confidence
                self.assert_rejected(inputs)

    def test_method_and_policy_cannot_claim_truth_mutation_or_synthetic_source(self):
        methods = ({"maker_kind": "synthetic_fixture"}, {"output_posture": "source_truth"},
                   {"locale": "de"}, {"method_name": ""}, {"accepted": True})
        for fields in methods:
            with self.subTest(method=fields):
                self.assert_rejected({**self.inputs, "method": {**self.inputs["method"], **fields}})
        for fields in ({"unreported_gaps_allowed": True}, {"overlap": "allow_declared"},
                       {"normalization": "rewrite-NFC"}):
            with self.subTest(policies=fields):
                scheme = copy.deepcopy(self.inputs["scheme"])
                scheme["policies"].update(fields)
                self.assert_rejected({**self.inputs, "scheme": scheme})
        for fields in ({"boundary_basis": "public_synthetic_fixture"}, {"analysis_role": "semantic_truth"},
                       {"review_status": "accepted"}):
            with self.subTest(scheme=fields):
                self.assert_rejected({**self.inputs, "scheme": {**self.inputs["scheme"], **fields}})

    def test_unknown_method_locale_is_preserved_without_guessing_a_language(self):
        packet = self.build(method={**self.inputs['method'], 'locale': None})
        self.assert_valid(packet)
        self.assertIsNone(packet['schemes'][0]['method']['locale'])
        self.assertEqual(packet['source_layer']['language'], self.fixture.layer['representation']['language'])

    def test_exact_text_is_not_newline_normalized_unicode_normalized_or_rebased(self):
        for text in (self.fixture.text.replace("\r\n", "\n"),
                     unicodedata.normalize("NFC", self.fixture.text), self.fixture.text[3:10]):
            with self.subTest(representation=text):
                self.assert_rejected({**self.inputs, "exact_text": text})

    def test_normalization_declaration_applies_only_to_frozen_representation_scope(self):
        text = "e\u0301\r\nAB CD"
        inputs = self.exact_inputs(text, 4, 9)
        inputs["verified_layer"]["representation"]["character_normalization"] = "NFC"
        inputs["verified_packet"]["source_layer"]["unicode_form"] = "NFC"
        inputs["spans"] = [self.span(1, 4, 6), self.span(2, 7, 9)]
        inputs["excluded_gaps"] = [{"anchor_ref": anchor(20), "start": 6, "end": 7}]
        self.assertNotEqual(unicodedata.normalize("NFC", text), text)
        self.assertEqual(unicodedata.normalize("NFC", text[4:9]), text[4:9])
        packet = build_text_unit_proposal(**inputs)
        self.assert_valid(packet, text)
        self.assertEqual(packet["source_layer"]["text_layer_sha256"], digest(text.encode()))
        self.assertEqual(packet["source_layer"]["unicode_form"], "NFC")

    def test_packet_layer_and_rights_mismatches_reject(self):
        mutations = (
            lambda row: row["verified_layer"]["source_binding"].update(work_ref="tos.work.different"),
            lambda row: row["verified_layer"]["source_binding"].update(source_file_sha256="0" * 64),
            lambda row: row["verified_layer"]["representation"].update(language="de"),
            lambda row: row["verified_layer"]["representation"].update(content_file_id="tos.file.sha256." + "0" * 64),
            lambda row: row["verified_layer"]["representation"].update(character_normalization="NFC"),
            lambda row: row["verified_packet"]["source_layer"].update(text_layer_sha256="0" * 64),
            lambda row: row["verified_packet"]["anchors"][0]["source_return"].update(locator_ref="ToS/source-witnesses/different.txt"),
            lambda row: row["verified_packet"]["rights_and_visibility"].update(rights_record_refs=[]),
            lambda row: row["verified_packet"].update(content_posture="public_synthetic_contract_exercise"),
        )
        for index, mutate in enumerate(mutations):
            with self.subTest(case=index):
                inputs = copy.deepcopy(self.inputs)
                mutate(inputs)
                self.assert_rejected(inputs)

    def test_unit_and_gap_budgets_are_enforced_before_construction(self):
        for identities in ({**self.inputs["identities"], "unit_slots": []},
                           {**self.inputs["identities"], "unit_slots": self.inputs["identities"]["unit_slots"] * (MAX_UNITS // 2 + 1)},
                           {**self.inputs["identities"], "gap_anchor_refs": [anchor(1000 + index) for index in range(MAX_GAPS + 1)]}):
            with self.subTest(units=len(identities["unit_slots"]), gaps=len(identities["gap_anchor_refs"])):
                self.assert_rejected({**self.inputs, "identities": identities})

    def test_maximum_bounded_unit_list_is_constructible_with_all_gaps_explicit(self):
        text = "P\r\n" + "x " * MAX_UNITS + "TAIL"
        inputs = self.exact_inputs(text, 3, 3 + 2 * MAX_UNITS)
        identities = inputs["identities"]
        identities["unit_slots"] = [{"unit_id": opaque("text-unit", index + 1),
                                      "anchor_ref": anchor(1000 + index), "unit_kind": "surface_token"}
                                     for index in range(MAX_UNITS)]
        identities["gap_anchor_refs"] = [anchor(2000 + index) for index in range(MAX_UNITS)]
        inputs["spans"] = [self.span(index + 1, 3 + 2 * index, 4 + 2 * index)
                           for index in range(MAX_UNITS)]
        inputs["excluded_gaps"] = [{"anchor_ref": anchor(2000 + index),
                                    "start": 4 + 2 * index, "end": 5 + 2 * index}
                                   for index in range(MAX_UNITS)]
        packet = build_text_unit_proposal(**inputs)
        self.assert_valid(packet, text)
        self.assertEqual(len(packet["units"]), MAX_UNITS)
        self.assertEqual(len(packet["anchors"]), 1 + 2 * MAX_UNITS)
        self.assertEqual(len(packet["segmentations"][0]["coverage"]["excluded_anchor_refs"]), MAX_UNITS)


if __name__ == "__main__":
    unittest.main()
