"""Synthetic-only checks for exact extraction and proposal construction.

No retained Item, private context, source prose, grant or journal is read.
Green checks prove the declared mechanics, never textual or rights admission.
"""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import source_text_layer_proposal as proposal
from source_text_layer_proposal import (
    DEFAULT_POLICY, TextLayerProposalError, build_text_layer_proposal,
    extract_xhtml_text, record_bytes, validate_extraction_profile,
)
from validate_source_witness_foundation import (
    _anchor_v2_semantic_issues, _source_text_layer_semantic_issues,
)


def opaque(kind, number):
    return "tos." + kind + ".sid-" + format(number, "032x")


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def selector(value="p:1", scheme="tos.xhtml.element-ordinal.v1"):
    return {"type": "structural", "scheme": scheme, "value": value}


def xhtml(body, *, declaration="", namespace=proposal.XHTML):
    return (declaration + '<html xmlns="' + namespace + '"><head/><body>' + body + "</body></html>").encode("utf-8")


class XhtmlExtractionTests(unittest.TestCase):
    def extract(self, raw, *, selected=None, policy=None):
        return extract_xhtml_text(raw, selector=selector() if selected is None else selected,
                                  policy=copy.deepcopy(DEFAULT_POLICY) if policy is None else policy)

    def test_exact_nfd_whitespace_inline_and_explicit_break(self):
        raw = xhtml('<p> \n cafe\u0301 <em>word</em>\t<br/>next \n</p>ROOT-TAIL<p>elsewhere</p>')
        actual = self.extract(raw)
        self.assertEqual(actual, " \n cafe\u0301 word\t\nnext \n")
        self.assertNotIn("caf\u00e9", actual)
        self.assertNotIn("ROOT-TAIL", actual)

    def test_ordinal_is_global_one_based_and_namespace_qualified(self):
        raw = xhtml('<section><p>one</p></section><p xmlns="urn:foreign">foreign</p><p>two</p>')
        self.assertEqual(self.extract(raw, selected=selector("p:2")), "two")
        with self.assertRaises(TextLayerProposalError):
            self.extract(raw, selected=selector("p:3"))

    def test_id_and_xml_id_are_exact_unique_targets(self):
        self.assertEqual(self.extract(xhtml('<p id="selected">alpha</p>'),
                                     selected=selector("selected", "tos.xhtml.element-id.v1")), "alpha")
        self.assertEqual(self.extract(xhtml('<p xml:id="selected">beta</p>'),
                                     selected=selector("selected", "tos.xhtml.element-id.v1")), "beta")
        for body in ('<p>none</p>', '<p id="selected">one</p><p xml:id="selected">two</p>'):
            with self.subTest(body=body), self.assertRaises(TextLayerProposalError):
                self.extract(xhtml(body), selected=selector("selected", "tos.xhtml.element-id.v1"))

    def test_utf8_xml10_declaration_bom_and_comments_are_explicit(self):
        self.assertEqual(self.extract(xhtml('<p>A<!-- omitted comment -->B</p>',
            declaration='<?xml version="1.0" encoding="utf-8"?>\n')), "AB")
        self.assertEqual(self.extract(b"\xef\xbb\xbf" + xhtml('<p>A</p>',
            declaration="<?xml version='1.0' encoding='UTF-8'?>")), "A")

    def test_strict_encoding_no_reference_or_line_ending_repair(self):
        bad = [xhtml('<p>A\rB</p>'), xhtml('<p>A\r\nB</p>'), xhtml('<p>A&amp;B</p>'),
               xhtml('<p>A&#32;B</p>'), xhtml('<p>A&#x20;B</p>'), xhtml('<p>A&nbsp;B</p>'),
               xhtml('<p>A</p>').replace(b"A", b"\xff"),
               xhtml('<p>A</p>', declaration='<?xml version="1.0" encoding="ISO-8859-1"?>'),
               xhtml('<p>A</p>', declaration='<?xml version="1.1"?>')]
        for raw in bad:
            with self.subTest(index=bad.index(raw)), self.assertRaises(TextLayerProposalError):
                self.extract(raw)

    def test_no_doctype_entities_or_arbitrary_processing_instruction(self):
        for prefix in ('<!DOCTYPE html SYSTEM "file:///private.txt">',
                       '<!DOCTYPE html [<!ENTITY secret SYSTEM "https://invalid.example/secret">]>',
                       '<?xml-stylesheet href="https://invalid.example/style"?>', '<?other private?>'):
            with self.subTest(prefix=prefix), self.assertRaises(TextLayerProposalError):
                self.extract(xhtml('<p>A</p>', declaration=prefix))

    def test_unknown_selected_markup_namespace_and_empty_text_fail_closed(self):
        for body in ('<p>A<script>private-code</script>B</p>', '<p>A<style>private-style</style></p>',
                     '<p>A<img alt="hidden text"/></p>', '<p><div>nested block</div></p>',
                     '<p>A<em xmlns="urn:foreign">B</em></p>', '<p/>', '<p><br>not-empty</br></p>'):
            with self.subTest(body=body), self.assertRaises(TextLayerProposalError):
                self.extract(xhtml(body))
        with self.assertRaises(TextLayerProposalError):
            self.extract(xhtml('<p>A</p>', namespace="urn:wrong"))
        with self.assertRaises(TextLayerProposalError):
            self.extract(b'<html xmlns="http://www.w3.org/1999/xhtml"><p>unfinished')

    def test_no_xpath_css_zero_negative_or_unbounded_selector(self):
        candidates = [selector("p:0"), selector("p:-1"), selector("p:01"), selector("p:999999"),
                      selector("p:1[evil]"), selector("//p", "xpath"), selector("p", "css"),
                      selector("body:1"), selector(" a ", "tos.xhtml.element-id.v1"),
                      {**selector(), "extra": "not allowed"}]
        for candidate in candidates:
            with self.subTest(candidate=candidate), self.assertRaises(TextLayerProposalError):
                validate_extraction_profile(candidate, DEFAULT_POLICY)

    def test_mutated_policy_and_budgets_are_not_silently_accepted(self):
        changed = copy.deepcopy(DEFAULT_POLICY)
        changed["unicode_normalization"] = "NFC"
        with self.assertRaises(TextLayerProposalError):
            self.extract(xhtml('<p>A</p>'), policy=changed)
        for limit, value, raw in (("MAX_MEMBER_BYTES", 8, xhtml('<p>A</p>')),
                                  ("MAX_TEXT_BYTES", 2, xhtml('<p>ABC</p>')),
                                  ("MAX_DEPTH", 3, xhtml('<p><em>A</em></p>')),
                                  ("MAX_ELEMENTS", 3, xhtml('<p>A</p>'))):
            with self.subTest(limit=limit), patch.object(proposal, limit, value), self.assertRaises(TextLayerProposalError):
                self.extract(raw)

    def test_oversized_markup_fails_before_parser_allocation_with_quoted_gt(self):
        for value in (">" * proposal.MAX_MARKUP_TOKEN_BYTES,
                      "\u00e9" * (proposal.MAX_MARKUP_TOKEN_BYTES // 2)):
            raw = xhtml('<p data="' + value + '">A</p>')
            with (self.subTest(value_bytes=len(value.encode("utf-8"))),
                  patch.object(proposal.ET, "XMLParser", side_effect=AssertionError("parser allocated")),
                  self.assertRaisesRegex(TextLayerProposalError, "markup token exceeds")):
                self.extract(raw)

    def test_attribute_limit_precedes_parser_and_counts_namespace_declarations(self):
        for prefix in ("a", "xmlns:a"):
            attributes = " ".join(prefix + str(index) + '="urn:test"'
                                  for index in range(proposal.MAX_ATTRIBUTES + 1))
            raw = xhtml("<p " + attributes + ">A</p>")
            with (self.subTest(prefix=prefix),
                  patch.object(proposal.ET, "XMLParser", side_effect=AssertionError("parser allocated")),
                  self.assertRaisesRegex(TextLayerProposalError, "attribute budget")):
                self.extract(raw)

    def test_markup_and_attribute_boundaries_allow_quoted_gt_and_equals(self):
        prefix, suffix = '<p data="', '">'
        value = ">=" * ((proposal.MAX_MARKUP_TOKEN_BYTES - len(prefix) - len(suffix)) // 2)
        value += "x" * (proposal.MAX_MARKUP_TOKEN_BYTES - len(prefix) - len(suffix) - len(value))
        self.assertEqual(len((prefix + value + suffix).encode()), proposal.MAX_MARKUP_TOKEN_BYTES)
        self.assertEqual(self.extract(xhtml(prefix + value + suffix + "A</p>")), "A")
        attributes = " ".join('a' + str(index) + '=\'> = "\'' for index in range(proposal.MAX_ATTRIBUTES))
        self.assertEqual(self.extract(xhtml("<p " + attributes + ">B</p>")), "B")

    def test_scanner_distinguishes_bounded_comments_and_literal_cdata(self):
        fake = "<fake " + " ".join('a' + str(index) + '="x"' for index in range(proposal.MAX_ATTRIBUTES + 1)) + ">"
        self.assertEqual(self.extract(xhtml("<p>A<!--" + fake + "-->B</p>")), "AB")
        self.assertEqual(self.extract(xhtml("<p>A<![CDATA[" + fake + "]]>B</p>")), "A" + fake + "B")
        text = "x" * (proposal.MAX_MARKUP_TOKEN_BYTES + 1)
        self.assertEqual(self.extract(xhtml("<p><![CDATA[" + text + "]]></p>")), text)
        for body in ("<p><!--" + text + "-->A</p>", "<p><!--unterminated", "<p><![CDATA[unterminated"):
            with (self.subTest(body_size=len(body)),
                  patch.object(proposal.ET, "XMLParser", side_effect=AssertionError("parser allocated")),
                  self.assertRaises(TextLayerProposalError)):
                self.extract(xhtml(body))

    def test_errors_do_not_echo_private_snippets_or_locators(self):
        private = "PRIVATE-SYNTHETIC-CANARY"
        try:
            self.extract(xhtml('<p><script>' + private + '</script></p>'))
        except TextLayerProposalError as error:
            self.assertNotIn(private, str(error))
            self.assertIsNone(error.__cause__)
        else:
            self.fail("unsupported selected script did not fail")


class TextLayerProposalTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.validators = {}
        for name in ("source-text-layer", "source-anchor-v2"):
            schema = json.loads((REPO_ROOT / "ToS/contracts" / (name + ".schema.json")).read_bytes())
            Draft202012Validator.check_schema(schema)
            cls.validators[name] = Draft202012Validator(schema)

    def setUp(self):
        base = "ToS/source-witnesses/owner-local/" + opaque("context", 1).split(".")[-1] + "/synthetic/layer"
        self.member = xhtml('<p> \ncafe\u0301 <em>word</em><br/>tail\t </p>')
        self.inputs = {
            "exact_text": extract_xhtml_text(self.member, selector=selector(), policy=DEFAULT_POLICY),
            "source_scope": {**{kind + "_ref": opaque(kind, 1) for kind in ("work", "expression", "edition", "item")},
                             "file_ref": "tos.file.sha256." + "1" * 64, "file_sha256": "1" * 64},
            "identities": {"layer_id": opaque("text-layer", 1), "anchor_id": opaque("anchor", 1),
                           "passage_id": opaque("passage", 1), "provenance_event_id": opaque("event", 1)},
            "refs": {"layer_ref": base + "/source-text-layer.v1.json", "anchor_ref": base + "/source-anchor.v1.json",
                     "content_ref": base + "/source-text.txt", "policy_ref": base + "/extraction-policy.v1.json",
                     "configuration_ref": base + "/source-create-owner-configuration.json", "configuration_sha256": "2" * 64,
                     "source_payload_ref": "ToS/source-witnesses/owner-local/synthetic/item/payload/source.epub"},
            "member": {"member_path": "EPUB/chapter.xhtml", "member_sha256": sha(self.member)},
            "selector": selector(), "policy": copy.deepcopy(DEFAULT_POLICY),
            "maker": {"maker_type": "software", "agent_ref": "tos.agent.synthetic-maker",
                      "method": "synthetic XHTML extraction", "version": "1"},
            "language": "en", "rights_record_refs": [{"ref": base + "/synthetic-rights.json", "sha256": "3" * 64}],
        }

    def build(self, **updates):
        return build_text_layer_proposal(**{**self.inputs, **updates})

    def test_schema_pure_semantics_exact_content_and_fixity(self):
        result = self.build()
        self.assertEqual(set(result), {"layer", "anchor", "policy", "content"})
        layer, anchor = result["layer"], result["anchor"]
        self.validators["source-text-layer"].validate(layer)
        self.validators["source-anchor-v2"].validate(anchor)
        self.assertEqual(_source_text_layer_semantic_issues(layer), [])
        self.assertEqual(_anchor_v2_semantic_issues(anchor), [])
        self.assertEqual(result["content"], self.inputs["exact_text"].encode("utf-8"))
        rep = layer["representation"]
        self.assertEqual(rep["content_sha256"], sha(result["content"]))
        self.assertEqual(rep["content_file_id"], "tos.file.sha256." + sha(result["content"]))
        self.assertEqual(rep["text_scope"]["end"], len(self.inputs["exact_text"]))
        self.assertEqual(layer["editorial_policy"]["policy_sha256"], sha(record_bytes(result["policy"])))
        self.assertEqual(layer["source_binding"]["anchors"][0]["anchor_record_sha256"], sha(record_bytes(anchor)))
        steps = anchor["selector_payload"]["expression"]["steps"]
        self.assertEqual(steps[0]["state"]["representation_sha256"], self.inputs["source_scope"]["file_sha256"])
        self.assertEqual(steps[0]["selector"]["member_sha256"], sha(self.member))
        self.assertEqual(steps[1]["selector"], self.inputs["selector"])

    def test_no_implicit_source_read_rights_review_or_publication(self):
        result = self.build()
        admission = result["layer"]["admission"]
        self.assertEqual(admission["review_status"], "unreviewed")
        self.assertEqual(admission["mechanical_status"], "materialized")
        self.assertFalse(admission["human_review_performed"])
        self.assertFalse(admission["automatic_validation_complete"])
        self.assertFalse(admission["promotion_authorized"])
        self.assertEqual(admission["accepted_uses"], [])
        self.assertIsNone(admission["review_ref"])
        self.assertEqual(result["anchor"]["resolution_status"], "locator_only")
        self.assertFalse(result["anchor"]["publication_boundary"]["source_text_in_record"])
        rep = result["layer"]["representation"]
        self.assertEqual(rep["content_visibility"], "local_only")
        self.assertEqual(rep["storage"], "ignored_local")
        self.assertFalse(rep["publication_authorized"])
        self.assertEqual(rep["publication_authority_refs"], [])
        self.assertEqual(result["layer"]["derivation"]["input_layers"], [])
        mutated = copy.deepcopy(result["layer"])
        mutated["admission"].update(review_status="accepted", review_ref=opaque("review", 1), accepted_uses=["citation"])
        self.assertFalse(self.validators["source-text-layer"].is_valid(mutated))

    def test_scope_wording_is_editable_while_admission_remains_typed(self):
        layer = self.build()["layer"]
        validator = self.validators["source-text-layer"]
        for wording in (
            "Exact extracted source representation.",
            "Точная извлечённая запись источника.",
            "Mechanical validation does not assess textual quality.",
        ):
            with self.subTest(wording=wording):
                revised = {**layer, "authority_boundary": wording}
                validator.validate(revised)
                self.assertEqual(_source_text_layer_semantic_issues(revised), [])
                promoted = copy.deepcopy(revised)
                promoted["admission"].update(
                    review_status="accepted", review_ref=opaque("review", 1),
                    accepted_uses=["citation"],
                )
                self.assertFalse(validator.is_valid(promoted))
        for missing_description in ("", None, []):
            self.assertFalse(validator.is_valid({**layer, "authority_boundary": missing_description}))

    def test_finite_canonical_json_lf_not_assessment_canonical_bytes(self):
        raw = record_bytes({"z": "\u00e9", "a": 1})
        self.assertEqual(raw, '{"a":1,"z":"\u00e9"}\n'.encode("utf-8"))
        for invalid in ({"bad": float("nan")}, {"bad": float("inf")}, {"bad": "\ud800"}, []):
            with self.subTest(invalid_type=type(invalid)), self.assertRaises(TextLayerProposalError):
                record_bytes(invalid)

    def test_inputs_and_outputs_do_not_share_mutable_state(self):
        before = copy.deepcopy(self.inputs)
        result = self.build()
        self.assertEqual(self.inputs, before)
        self.assertEqual(result, self.build())
        result["layer"]["representation"]["rights_record_refs"][0]["ref"] = "changed"
        result["policy"]["inline_elements"].append("script")
        result["anchor"]["selector_payload"]["expression"]["steps"][1]["selector"]["value"] = "p:2"
        self.assertEqual(self.inputs, before)

    def test_invalid_delegation_text_refs_rights_and_maker_fail_closed(self):
        mutations = [
            ("exact_text", ""), ("exact_text", "A\rB"), ("exact_text", "\ud800"), ("exact_text", b"text"),
            ("language", "invalid language"), ("rights_record_refs", []),
            ("rights_record_refs", self.inputs["rights_record_refs"] * 2),
            ("identities", {**self.inputs["identities"], "layer_id": opaque("passage", 2)}),
            ("refs", {**self.inputs["refs"], "content_ref": "../escape"}),
            ("refs", {**self.inputs["refs"], "configuration_sha256": "bad"}),
            ("refs", {**self.inputs["refs"], "content_ref": self.inputs["refs"]["layer_ref"]}),
            ("member", {"member_path": "https://invalid.example/member", "member_sha256": "4" * 64}),
            ("member", {"member_path": "../escape", "member_sha256": "4" * 64}),
            ("maker", {**self.inputs["maker"], "maker_type": "human"}),
            ("source_scope", {**self.inputs["source_scope"], "file_sha256": "bad"}),
        ]
        for key, value in mutations:
            with self.subTest(key=key), self.assertRaises(TextLayerProposalError):
                self.build(**{key: value})

    def test_extract_and_build_have_no_file_process_or_network_io(self):
        with (patch("builtins.open", side_effect=AssertionError("file I/O")),
              patch("pathlib.Path.open", side_effect=AssertionError("path I/O")),
              patch("subprocess.Popen", side_effect=AssertionError("process I/O")),
              patch("socket.socket", side_effect=AssertionError("network I/O"))):
            text = extract_xhtml_text(self.member, selector=self.inputs["selector"], policy=self.inputs["policy"])
            self.assertEqual(self.build(exact_text=text)["content"], text.encode("utf-8"))


class DerivedLayerProposalTests(unittest.TestCase):
    def setUp(self):
        seed = TextLayerProposalTests()
        seed.setUp()
        self.previous = seed.build()['layer']
        self.text = seed.inputs['exact_text']
        self.refs = {key: value.replace('/synthetic/layer/', '/synthetic/successor/')
                     for key, value in seed.inputs['refs'].items()
                     if key in {'content_ref', 'policy_ref', 'configuration_ref', 'configuration_sha256'}}
        self.config = {'allowed_operations': ['text-layer.normalize'],
            'policy': proposal.derivation_policy('text-layer.normalize', unicode_form='NFC'),
            'identities': {'layer_id': opaque('text-layer', 2), 'provenance_event_id': opaque('event', 2)},
            'maker': copy.deepcopy(seed.inputs['maker']), 'material': {}, 'language': 'en',
            'derivation_access': {'rights_record_refs': copy.deepcopy(seed.inputs['rights_record_refs'])}}
        self.bind(self.text)

    def bind(self, text):
        self.text = text
        self.previous['representation'].update(content_sha256=sha(text.encode()), content_file_id='tos.file.sha256.' + sha(text.encode()))
        self.previous['representation']['text_scope']['end'] = len(text)
        self.config['input'] = {'binding': {'text_layer': {'layer_id': self.previous['layer_id'],
            'layer_version': self.previous['layer_version'], 'record_ref': 'ToS/source-witnesses/synthetic/input.json',
            'record_sha256': sha(record_bytes(self.previous))}}}

    def build(self):
        return proposal.build_derived_text_layer(config=self.config, refs=self.refs,
            source_binding=self.previous['source_binding'], predecessor=self.previous, input_text=self.text)

    def test_all_declared_unicode_forms_are_exact_and_schema_valid(self):
        schema = json.loads((REPO_ROOT / 'ToS/contracts/source-text-layer.schema.json').read_bytes())
        for form, original, expected in [('NFC', 'e\u0301', '\u00e9'), ('NFD', '\u00e9', 'e\u0301'),
                ('NFKC', '\ufb01 \u2460', 'fi 1'), ('NFKD', '\u00e9 \u2460', 'e\u0301 1'),
                ('NFC', 'already normalized', 'already normalized')]:
            with self.subTest(form=form, original=original):
                self.bind(original)
                before = record_bytes(self.previous)
                self.config['policy'] = proposal.derivation_policy('text-layer.normalize', unicode_form=form)
                output = self.build()
                self.assertEqual(output['content'], expected.encode())
                Draft202012Validator(schema).validate(output['layer'])
                self.assertEqual(_source_text_layer_semantic_issues(output['layer']), [])
                self.assertEqual(record_bytes(self.previous), before)
                edit = output['layer']['derivation']['change_payload']['operations'][0]
                self.assertEqual(edit['input_exact'], original)
                self.assertEqual(edit['output_exact'], expected)
                self.assertEqual(edit['status'], 'proposed')

    def test_no_predecessor_quality_or_admission_is_transferred(self):
        self.previous['admission'].update(review_status='accepted', accepted_uses=['citation'],
            human_review_performed=True, human_language_competence='competent', promotion_authorized=True)
        self.bind(self.text)
        output = self.build()['layer']
        self.assertEqual(output['admission']['accepted_uses'], [])
        self.assertEqual(output['admission']['review_status'], 'unreviewed')
        self.assertFalse(output['admission']['human_review_performed'])
        self.assertFalse(output['admission']['promotion_authorized'])
        self.assertEqual(output['admission']['human_language_competence'], 'not_assessed')

    def test_partial_scope_wrong_version_identity_and_policy_are_refused(self):
        config, previous = copy.deepcopy((self.config, self.previous))
        for mutation in ('scope', 'version', 'identity', 'policy', 'digest'):
            self.config, self.previous = copy.deepcopy((config, previous))
            if mutation == 'scope':
                self.previous['representation']['text_scope']['start'] = 1
            elif mutation == 'version':
                self.config['input']['binding']['text_layer']['layer_version'] += 1
            elif mutation == 'identity':
                self.config['identities']['layer_id'] = self.previous['layer_id']
            elif mutation == 'policy':
                self.config['policy']['unicode_database_version'] = 'unsupported'
            else:
                self.previous['representation']['content_sha256'] = '0' * 64
            with self.subTest(case=mutation), self.assertRaises(TextLayerProposalError):
                self.build()

    def test_normalized_text_cannot_be_silently_relabelled_as_source_near_correction(self):
        self.previous['layer_role'] = 'normalized_text'
        self.previous['representation']['character_normalization'] = 'NFC'
        self.config['allowed_operations'] = ['text-layer.correct']
        self.config['policy'] = proposal.derivation_policy('text-layer.correct')
        self.config['material'] = {'edits': []}
        with self.assertRaisesRegex(TextLayerProposalError, 'cannot erase predecessor normalization'):
            self.build()

    def test_bounded_text_and_edit_sequence_reject_excess_before_expansion(self):
        self.bind('x' * (proposal.MAX_DERIVED_TEXT_BYTES + 1))
        with self.assertRaises(TextLayerProposalError):
            self.build()
        self.bind('A')
        self.config['allowed_operations'] = ['text-layer.correct']
        self.config['policy'] = proposal.derivation_policy('text-layer.correct')
        self.config['material'] = {'edits': [{}] * (proposal.MAX_EDITS + 1)}
        with self.assertRaises(TextLayerProposalError):
            self.build()

    def test_native_transform_is_pure_and_does_not_execute_supplied_source(self):
        with (patch('builtins.open', side_effect=AssertionError('file I/O')),
              patch('subprocess.Popen', side_effect=AssertionError('process I/O')),
              patch('socket.socket', side_effect=AssertionError('network I/O'))):
            self.assertEqual(self.build()['layer']['derivation']['method'], 'unicode_normalization')


if __name__ == "__main__":
    unittest.main()
