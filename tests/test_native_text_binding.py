"""Synthetic native closure tests, not historical evidence or assessment.

Only public laboratory skeletons and authored schemas are copied. Every
source, rights statement, actor and byte sequence below is a temporary test
construction. The original Item payload is deliberately never written.
"""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from native_text_binding import NativeTextBindingError, NativeTextBindingResolver


LAB = "ToS/research-packets/foundation-laboratory-2026-07"
CONTRACTS = (
    "native-text-unit-binding.schema.json",
    "native-text-layer-binding.schema.json",
    "source-text-unit-packet-v1.schema.json",
    "source-text-layer.schema.json",
    "source-anchor-v2.schema.json",
    "corpus-record.schema.json",
    "source-item-manifest.schema.json",
    "rights-record.schema.json",
)
NOTICE = "Synthetic test fixture only; no historical, linguistic or rights judgment."
STAMP = "2026-09-08T00:00:00Z"


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


class NativeTextBindingFixture:
    """One finite source-owned graph with nonzero absolute text coordinates."""

    def __init__(self, root: Path):
        self.root = root
        for name in CONTRACTS:
            self.write_bytes("ToS/contracts/" + name,
                             (REPO_ROOT / "ToS/contracts" / name).read_bytes())
        work_home = "ToS/source-witnesses/works/synthetic-native-binding"
        expression_home = work_home + "/expressions/und-synthetic"
        edition_home = expression_home + "/editions/synthetic-edition"
        self.item_home = edition_home + "/items/synthetic-item"
        self.native_home = work_home + "/technical-markup/synthetic-binding"
        self.refs = {
            "work": work_home + "/work.json",
            "expression": expression_home + "/expression.json",
            "edition": edition_home + "/edition.json",
            "item": self.item_home + "/item.json",
        }
        self.ids = {kind: "tos." + kind + ".synthetic.native-binding" for kind in self.refs}
        self.manifest_ref = self.item_home + "/item.manifest.json"
        self.rights_ref = self.item_home + "/rights.json"
        self.anchor_ref = self.native_home + "/source-anchor-v2.synthetic.v1.json"
        self.layer_ref = self.native_home + "/source-text-layer.synthetic.v1.json"
        self.packet_ref = self.native_home + "/source-text-unit.synthetic.v1.json"
        self.content_ref = self.native_home + "/local-content/synthetic.txt"
        self.authority_ref = self.native_home + "/synthetic-publication-authority.json"
        self.policy_ref = self.native_home + "/synthetic-editorial-policy.json"
        self.original_ref = self.item_home + "/payload/synthetic-original.xml"
        original = b"<synthetic>Not a historical witness.</synthetic>\r\n"
        self.original_digest = digest(original)
        self.original_id = "tos.file.sha256." + self.original_digest
        # Code points 3:8 are NFD cafe-accent, 8:10 are space + Greek omega.
        # CRLF must survive. The layer's selected scope is 3:10, not 0:N.
        self.text = "P\r\ncafe\u0301 Ω\r\nTAIL"
        self.content = self.text.encode("utf-8")
        self.write_bytes(self.content_ref, self.content)
        self.write_json(self.policy_ref, {"notice": NOTICE})
        self.write_json(self.authority_ref, {"notice": NOTICE})
        for kind, ref in self.refs.items():
            record = {
                "schema_version": "tos_corpus_record_v1", "record_type": kind,
                "record_id": self.ids[kind], "preferred_label": NOTICE,
                "identity_status": "provisional", "source_refs": [self.policy_ref],
                "external_identifiers": [], "same_as_posture": "no_equivalence_claim",
                "record_version": 1, "notes": NOTICE,
            }
            if kind == "work":
                record["expression_claim_refs"] = []
            elif kind == "expression":
                record.update(work_ref=self.ids["work"], language="und",
                              expression_role="source_language", responsibility_claim_refs=[],
                              embodiment_claim_refs=[])
            elif kind == "edition":
                record.update(embodies_expression_refs=[self.ids["expression"]],
                              publication_claim_refs=[], exemplar_claim_refs=[])
            else:
                record["item_manifest_ref"] = self.manifest_ref
            self.write_json(ref, record)
        self.rights = {
            "schema_version": "tos_rights_record_v1",
            "rights_id": "tos.rights.synthetic.native-binding",
            "scope_refs": [self.ids["item"], self.original_id],
            "assessment_status": "not_assessed", "jurisdictions_reviewed": [],
            "source_refs": [self.policy_ref], "permissions": [],
            "restrictions": [NOTICE], "visibility": "local_only",
            "redistribution_posture": "not_authorized", "derivative_posture": "local_research_only",
            "assessed_by": {"maker_type": "model", "agent_ref": "model:synthetic-fixture"},
            "assessed_at": STAMP, "rationale": NOTICE,
            "review_status": "unreviewed", "record_version": 1,
        }
        self.manifest = {
            "schema_version": "tos_source_item_manifest_v1", "item_id": self.ids["item"],
            "item_kind": "born_digital", "embodiment_ref": self.ids["edition"],
            "storage_posture": "local_gitignored_payload",
            "payload_files": [{"file_id": self.original_id,
                "relative_path": "payload/synthetic-original.xml", "original_basename": "synthetic-original.xml",
                "media_type": "application/xml", "byte_size": len(original),
                "sha256": self.original_digest, "fixity_verified_at": STAMP}],
            "acquisition_event_ref": "tos.event.synthetic.native-binding",
            "rights_ref": self.rights_ref, "provenance_ref": self.policy_ref,
            "forensic_report_ref": self.policy_ref, "resource_inventory_ref": self.policy_ref,
            "visibility": "local_only", "manifest_version": 1,
        }
        self.write_json(self.manifest_ref, self.manifest)
        self.anchor = self.skeleton("source-anchor-v2-abc/variant-b.anchor.json")
        self.anchor.update(anchor_id="tos.anchor.synthetic.native-binding.origin",
                           passage_id="tos.passage.synthetic.native-binding")
        self.anchor["target"].update(item_id=self.ids["item"], file_id=self.original_id,
                                     file_sha256=self.original_digest, media_type="application/xml")
        envelope = self.anchor["selector_payload"]["expression"]["selector"]
        envelope["state"].update(representation_ref=self.original_ref,
                                 representation_sha256=self.original_digest, media_type="application/xml")
        envelope["selector"].update(start=0, end=len(original))
        self.anchor["publication_boundary"].update(source_content_visibility="local_only",
                                                  public_payload_expected=False)
        self.anchor["selector_method"].update(method="synthetic test selector",
            configuration_ref=self.policy_ref, configuration_digest=self.file_digest(self.policy_ref))
        self.layer = self.skeleton("source-text-layer-abc/variant-a.layer.json")
        self.layer.update(layer_id="tos.text-layer.synthetic.native-binding",
                          layer_role="machine_transcription")
        self.layer["source_binding"].update(
            **{kind + "_ref": self.ids[kind] for kind in self.ids},
            source_file_ref=self.original_id, source_file_sha256=self.original_digest)
        self.layer["representation"].update(
            content_file_id="tos.file.sha256." + digest(self.content), content_ref=self.content_ref,
            content_sha256=digest(self.content), language="und",
            text_scope={"start": 3, "end": 10, "position_unit": "unicode_code_point", "interval": "half_open"},
            character_normalization="none", line_break_posture="source_preserved",
            storage="ignored_local", content_visibility="local_only", tracked_content=False)
        self.layer["derivation"].update(method="structural_extraction")
        self.layer["derivation"]["maker"].update(method="synthetic test extraction",
            configuration_ref=self.policy_ref, configuration_digest=self.file_digest(self.policy_ref))
        self.layer["editorial_policy"].update(policy_ref=self.policy_ref,
            policy_sha256=self.file_digest(self.policy_ref))
        self.packet = self.skeleton("source-text-unit-v1-abc/variant-a-source-layout-observation.json")
        self.packet.update(content_posture="source_bound")
        self.packet["source_scope"] = {
            **{kind + "_ref": self.ids[kind] for kind in self.ids},
            "file_ref": self.original_id, "file_sha256": self.original_digest,
        }
        self.packet["source_layer"].update(text_layer_ref=self.layer_ref,
            text_layer_sha256=digest(self.content), language="und", unicode_form="source_preserved",
            visibility="local_only", publication_authorized=False)
        scheme = self.packet["schemes"][0]
        scheme.update(analysis_role="orthographic", boundary_basis="rule_based",
                      scheme_name=NOTICE, unit_kinds=["surface_token"])
        scheme["policies"].update(whitespace="declared_excluded", line_break="declared_excluded")
        scheme["method"].update(maker_kind="software", agent_ref="software:synthetic-test-fixture",
            method_name=NOTICE, configuration_ref=self.policy_ref, locale="und",
            software_refs=[self.policy_ref])
        self.packet["schemes"] = [scheme]
        unit = self.packet["units"][0]
        self.scope_id = "tos.anchor.synthetic.native-binding.scope"
        self.token_id = "tos.anchor.synthetic.native-binding.token"
        self.gap_id = "tos.anchor.synthetic.native-binding.gap"
        anchors = []
        for ordinal, (identity, role, start, end) in enumerate((
            (self.scope_id, "scope", 3, 10), (self.token_id, "content", 3, 8),
            (self.gap_id, "gap", 8, 10)), start=1):
            anchor = copy.deepcopy(self.packet["anchors"][0])
            anchor.update(anchor_ref=identity, anchor_role=role, ordinal=ordinal,
                text_layer_ref=self.layer_ref, text_layer_sha256=digest(self.content),
                exact_sha256=digest(self.text[start:end].encode("utf-8")))
            anchor["selector"].update(start=start, end=end)
            anchor["source_return"]["locator_ref"] = self.content_ref
            anchors.append(anchor)
        self.packet["anchors"] = anchors
        unit.update(unit_kind="surface_token", ordered_anchor_refs=[self.token_id],
                    boundary_posture="method_proposed", status_reason=NOTICE,
                    parent_unit_refs=[], ordered_child_unit_refs=[])
        self.packet["units"] = [unit]
        segmentation = self.packet["segmentations"][0]
        segmentation.update(ordered_unit_refs=[unit["unit_id"]], status="proposed", status_reason=NOTICE,
                            maker=copy.deepcopy(scheme["method"]))
        segmentation["coverage"].update(scope_anchor_ref=self.scope_id,
            coverage_posture="declared_partial", excluded_anchor_refs=[self.gap_id])
        self.packet["segmentations"] = [segmentation]
        self.packet["reviews"], self.packet["projections"] = [], []
        self.packet["rights_and_visibility"].update(source_visibility="local_only",
            packet_visibility="public_metadata_only", effective_visibility="local_only",
            rights_record_refs=[self.rights_ref], private_source_used=True, publication_authorized=False)
        self.binding = {
            "schema_version": "tos_native_text_unit_binding_v1",
            "packet_ref": self.packet_ref, "packet_sha256": "0" * 64,
            "packet_id": self.packet["packet_id"], "packet_version": self.packet["packet_version"],
            "segmentation_id": segmentation["segmentation_id"], "segmentation_version": 1,
            "unit_id": unit["unit_id"], "unit_version": 1, "ordered_anchor_refs": [self.token_id],
            "text_layer": {"record_ref": self.layer_ref, "record_sha256": "0" * 64,
                           "layer_id": self.layer["layer_id"], "layer_version": 1},
            "source_record_refs": dict(self.refs),
        }
        self.refresh()

    @staticmethod
    def skeleton(relative: str) -> dict:
        return json.loads((REPO_ROOT / LAB / relative).read_bytes())

    def write_bytes(self, ref: str, raw: bytes) -> None:
        path = self.root / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)

    def write_json(self, ref: str, body: dict) -> None:
        self.write_bytes(ref, (json.dumps(body, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8"))

    def read_json(self, ref: str) -> dict:
        return json.loads((self.root / ref).read_bytes())

    def file_digest(self, ref: str) -> str:
        return digest((self.root / ref).read_bytes())

    def refresh(self) -> None:
        """Rebind only fixture-authored metadata, never the Item payload."""
        self.write_json(self.rights_ref, self.rights)
        self.write_json(self.anchor_ref, self.anchor)
        self.layer["source_binding"]["anchors"] = [{
            "anchor_id": self.anchor["anchor_id"], "anchor_record_ref": self.anchor_ref,
            "anchor_record_sha256": self.file_digest(self.anchor_ref)}]
        self.layer["representation"]["rights_record_refs"] = [{
            "ref": self.rights_ref, "sha256": self.file_digest(self.rights_ref)}]
        self.write_json(self.layer_ref, self.layer)
        self.write_json(self.packet_ref, self.packet)
        self.binding["packet_sha256"] = self.file_digest(self.packet_ref)
        self.binding["text_layer"]["record_sha256"] = self.file_digest(self.layer_ref)

    def make_public(self) -> None:
        public_ref = self.native_home + "/public-synthetic-content.txt"
        self.write_bytes(public_ref, self.content)
        self.content_ref = public_ref
        self.layer["representation"].update(content_ref=public_ref, content_visibility="public",
            storage="tracked", tracked_content=True, publication_authorized=True,
            publication_authority_refs=[{"ref": self.authority_ref,
                                         "sha256": self.file_digest(self.authority_ref)}])
        for anchor in self.packet["anchors"]:
            anchor["source_return"]["locator_ref"] = public_ref
        self.anchor["publication_boundary"].update(source_content_visibility="public", public_payload_expected=True)
        self.packet["source_layer"].update(visibility="public", publication_authorized=True)
        self.packet["rights_and_visibility"].update(source_visibility="public", packet_visibility="public",
            effective_visibility="public", private_source_used=False, publication_authorized=True)
        self.rights.update(assessment_status="licensed", visibility="public_payload",
                           redistribution_posture="authorized", derivative_posture="allowed",
                           license_uri="https://example.invalid/synthetic-license")
        self.manifest["visibility"] = "public_payload"
        self.write_json(self.manifest_ref, self.manifest)
        self.refresh()


class NativeTextBindingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tos-native-binding-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.fixture = NativeTextBindingFixture(self.root)

    def resolve(self, **kwargs):
        return NativeTextBindingResolver(self.root).resolve(self.fixture.binding, **kwargs)

    def layer_binding(self):
        return {'schema_version': 'tos_native_text_layer_binding_v1',
                'text_layer': copy.deepcopy(self.fixture.binding['text_layer']),
                'source_record_refs': copy.deepcopy(self.fixture.refs)}

    def test_layer_only_bootstrap_needs_no_packet_or_original_payload(self):
        (self.root / self.fixture.packet_ref).unlink()
        resolver = NativeTextBindingResolver(self.root)
        metadata = resolver.resolve_layer(self.layer_binding())
        self.assertFalse(metadata['content_verified'])
        exact = resolver.resolve_layer(self.layer_binding(), verify_content=True, allow_private_content=True)
        self.assertTrue(exact['content_verified'])
        self.assertFalse(exact['public_content_declared'])
        self.assertFalse(exact['assessment_applied'])
        self.assert_private_safe(exact)
        self.assertEqual((self.root / self.fixture.content_ref).read_bytes(), self.fixture.content)
        self.assertFalse((self.root / self.fixture.original_ref).exists())

    def test_layer_only_rights_and_read_grant_refusal_precede_representation_io(self):
        def reader(path, limit):
            self.assertNotEqual(path, self.root / self.fixture.content_ref)
            self.assertNotEqual(path, self.root / self.fixture.original_ref)
            return path.read_bytes()
        with self.assertRaises(NativeTextBindingError):
            NativeTextBindingResolver(self.root, read_bytes=reader).resolve_layer(
                self.layer_binding(), verify_content=True)
        self.fixture.rights['derivative_posture'] = 'not_authorized'
        self.fixture.refresh()
        with self.assertRaises((NativeTextBindingError, PermissionError)):
            NativeTextBindingResolver(self.root, read_bytes=reader).resolve_layer(
                self.layer_binding(), verify_content=True, allow_private_content=True)

    def test_layer_only_binding_does_not_accept_packet_surrogate_or_drift(self):
        for change in ('packet', 'identity', 'digest'):
            binding = self.layer_binding()
            if change == 'packet':
                binding['packet_ref'] = self.fixture.packet_ref
            elif change == 'identity':
                binding['text_layer']['layer_id'] += '.other'
            else:
                binding['text_layer']['record_sha256'] = '0' * 64
            with self.subTest(change=change), self.assertRaises(NativeTextBindingError):
                NativeTextBindingResolver(self.root).resolve_layer(binding)

    def test_layer_only_view_never_becomes_a_public_content_route(self):
        self.fixture.make_public()
        result = NativeTextBindingResolver(self.root).resolve_layer(
            self.layer_binding(), verify_content=True, allow_private_content=True)
        self.assertFalse(result['public_content_declared'])
        self.assertFalse(result['assessment_applied'])

    def assert_private_safe(self, value):
        rendered = json.dumps(value, ensure_ascii=False)
        self.assertNotIn(self.fixture.text[3:8], rendered)
        self.assertNotIn(self.fixture.content_ref, rendered)
        self.assertNotIn(self.fixture.packet_ref, rendered)
        self.assertNotIn(digest(self.fixture.content), rendered)
        for forbidden in ("exact_sha256", "selector", "ordered_anchor_refs", "source_record_refs"):
            self.assertNotIn(forbidden, rendered)

    def test_integrated_fixture_satisfies_actual_native_schemas(self):
        cases = [(self.fixture.packet_ref, "source-text-unit-packet-v1.schema.json"),
                 (self.fixture.layer_ref, "source-text-layer.schema.json"),
                 (self.fixture.anchor_ref, "source-anchor-v2.schema.json"),
                 (self.fixture.rights_ref, "rights-record.schema.json"),
                 (self.fixture.manifest_ref, "source-item-manifest.schema.json")]
        cases.extend((ref, "corpus-record.schema.json") for ref in self.fixture.refs.values())
        for ref, schema_name in cases:
            with self.subTest(schema=schema_name):
                schema = self.fixture.read_json("ToS/contracts/" + schema_name)
                errors = list(Draft202012Validator(schema, format_checker=FormatChecker()).iter_errors(
                    self.fixture.read_json(ref)))
                self.assertEqual([], [error.message for error in errors])
        self.assertFalse((self.root / self.fixture.original_ref).exists())

    def test_metadata_only_does_not_open_content_or_original_payload(self):
        seen = []

        def read(path, limit):
            ref = path.relative_to(self.root).as_posix()
            self.assertNotIn("local-content", path.parts)
            self.assertNotIn("payload", path.parts)
            seen.append(ref)
            return path.read_bytes()[:limit + 1]

        resolver = NativeTextBindingResolver(self.root, read_bytes=read)
        summary = resolver.resolve(self.fixture.binding)
        self.assertEqual({
            "metadata_verified": True, "content_verified": False, "original_payload_verified": False,
            "unit_id": self.fixture.binding["unit_id"], "unit_version": 1, "unit_kind": "surface_token",
            "segmentation_id": self.fixture.binding["segmentation_id"], "segmentation_version": 1,
            "layer_id": self.fixture.layer["layer_id"], "layer_version": 1, "language": "und",
            "effective_visibility": "local_only", "public_content_declared": False, "public_content_available": False,
            "native_status": {"unit_boundary_posture": "method_proposed", "segmentation_status": "proposed",
                              "layer_review_status": "unreviewed"},
            "assessment_applied": False,
        }, summary)
        self.assert_private_safe(summary)
        self.assertTrue(resolver.schema_digests)
        self.assertTrue(all(ref.startswith("ToS/contracts/") and ref.endswith(".schema.json")
                            for ref in resolver.schema_digests))
        self.assertNotIn(self.fixture.packet_ref, resolver.schema_digests)
        self.assertNotIn(self.fixture.content_ref, seen)
        self.assertRegex(resolver.snapshot(read_bytes=read), r"^sha256:[a-f0-9]{64}$")

    def test_private_exact_verification_requires_explicit_content_access(self):
        with self.assertRaises(NativeTextBindingError):
            self.resolve(verify_content=True)
        summary = self.resolve(verify_content=True, allow_private_content=True)
        self.assertTrue(summary["metadata_verified"])
        self.assertTrue(summary["content_verified"])
        self.assertFalse(summary["original_payload_verified"])
        self.assertFalse(summary["public_content_available"])
        self.assert_private_safe(summary)

    def test_private_access_refusal_happens_before_any_content_read(self):
        def read(path, limit):
            self.assertNotIn("local-content", path.parts)
            self.assertNotIn("payload", path.parts)
            return path.read_bytes()[:limit + 1]

        resolver = NativeTextBindingResolver(self.root, read_bytes=read)
        with self.assertRaises(NativeTextBindingError):
            resolver.resolve(self.fixture.binding, verify_content=True)

    def test_truthy_values_do_not_grant_private_access_or_request_verification(self):
        for value in (1, "false", [True]):
            with self.subTest(flag="access", value_type=type(value).__name__), self.assertRaises(NativeTextBindingError):
                self.resolve(verify_content=True, allow_private_content=value)
            with self.subTest(flag="verify", value_type=type(value).__name__), self.assertRaises(NativeTextBindingError):
                self.resolve(verify_content=value, allow_private_content=True)

    def test_exact_bytes_preserve_crlf_nfd_and_nonzero_absolute_coordinates(self):
        before = (self.root / self.fixture.content_ref).read_bytes()
        self.assertIn(b"\r\n", before)
        self.assertTrue(self.resolve(verify_content=True, allow_private_content=True)["content_verified"])
        self.assertEqual(before, (self.root / self.fixture.content_ref).read_bytes())
        self.fixture.write_bytes(self.fixture.content_ref, before.replace(b"\r\n", b"\n"))
        with self.assertRaises(NativeTextBindingError):
            self.resolve(verify_content=True, allow_private_content=True)

    def test_public_availability_requires_exact_bytes_and_all_existing_gates(self):
        self.fixture.make_public()
        self.assertFalse(self.resolve()["public_content_available"])
        summary = self.resolve(verify_content=True)
        self.assertTrue(summary["content_verified"])
        self.assertTrue(summary["public_content_available"])
        self.assertFalse(summary["original_payload_verified"])
        self.assert_private_safe(summary)
        self.fixture.packet["rights_and_visibility"]["publication_authorized"] = False
        self.fixture.refresh()
        self.assertFalse(self.resolve(verify_content=True, allow_private_content=True)["public_content_available"])

    def test_closed_rights_cannot_be_hidden_by_public_packet_flags(self):
        self.fixture.make_public()
        self.fixture.rights.update(assessment_status="permission_requested", visibility="permission_requested")
        self.fixture.refresh()
        try:
            result = self.resolve(verify_content=True)
        except NativeTextBindingError:
            return  # A contradictory closure may be refused instead of projected.
        self.assertFalse(result["public_content_available"])

    def test_selected_identity_membership_and_order_are_exact(self):
        mutations = (
            {"unit_id": "tos.text-unit.sid-" + "0" * 32}, {"unit_version": 2},
            {"segmentation_id": "tos.text-segmentation.sid-" + "0" * 32},
            {"segmentation_version": 2}, {"packet_version": 2},
            {"ordered_anchor_refs": [self.fixture.gap_id]},
            {"ordered_anchor_refs": [self.fixture.token_id, self.fixture.token_id]},
            {"packet_sha256": "0" * 64}, {"schema_version": "unknown"},
        )
        for mutation in mutations:
            with self.subTest(fields=sorted(mutation)), self.assertRaises(NativeTextBindingError):
                NativeTextBindingResolver(self.root).resolve({**self.fixture.binding, **mutation})
        invalid = copy.deepcopy(self.fixture.binding)
        invalid["text_layer"]["layer_version"] = 2
        with self.assertRaises(NativeTextBindingError):
            NativeTextBindingResolver(self.root).resolve(invalid)

    def test_multiple_anchor_order_is_bound_without_normalizing_the_selection(self):
        first = self.fixture.packet["anchors"][1]
        second = copy.deepcopy(first)
        second_id = "tos.anchor.synthetic.native-binding.token-second"
        first["selector"]["end"] = 5
        first["exact_sha256"] = digest(self.fixture.text[3:5].encode("utf-8"))
        second.update(anchor_ref=second_id, ordinal=4,
                      exact_sha256=digest(self.fixture.text[5:8].encode("utf-8")))
        second["selector"]["start"] = 5
        self.fixture.packet["anchors"].append(second)
        self.fixture.packet["units"][0]["ordered_anchor_refs"] = [self.fixture.token_id, second_id]
        self.fixture.binding["ordered_anchor_refs"] = [self.fixture.token_id, second_id]
        self.fixture.refresh()
        self.assertTrue(self.resolve(verify_content=True, allow_private_content=True)["content_verified"])
        self.fixture.binding["ordered_anchor_refs"].reverse()
        with self.assertRaises(NativeTextBindingError):
            self.resolve()

    def test_source_record_topology_and_canonical_basenames_are_not_labels(self):
        for kind, field, wrong in (("expression", "work_ref", "tos.work.wrong"),
                                  ("edition", "embodies_expression_refs", ["tos.expression.wrong"])):
            ref = self.fixture.refs[kind]
            original = self.fixture.read_json(ref)
            self.fixture.write_json(ref, {**original, field: wrong})
            with self.subTest(kind=kind), self.assertRaises(NativeTextBindingError):
                self.resolve()
            self.fixture.write_json(ref, original)
        alias_ref = str(Path(self.fixture.refs["work"]).with_name("work-alias.json"))
        self.fixture.write_bytes(alias_ref, (self.root / self.fixture.refs["work"]).read_bytes())
        self.fixture.binding["source_record_refs"]["work"] = alias_ref
        with self.assertRaises(NativeTextBindingError):
            self.resolve()

    def test_rehashed_packet_cannot_substitute_another_source_or_raw_text_layer(self):
        original = copy.deepcopy(self.fixture.packet)
        for field in ("work_ref", "expression_ref", "edition_ref", "item_ref", "file_ref", "file_sha256"):
            with self.subTest(field=field):
                self.fixture.packet = copy.deepcopy(original)
                self.fixture.packet["source_scope"][field] = (
                    "0" * 64 if field == "file_sha256" else "tos." + field.removesuffix("_ref") + ".wrong")
                self.fixture.refresh()
                with self.assertRaises(NativeTextBindingError):
                    self.resolve()
        self.fixture.packet = copy.deepcopy(original)
        self.fixture.packet["source_layer"]["text_layer_ref"] = self.fixture.content_ref
        self.fixture.refresh()
        with self.assertRaises(NativeTextBindingError):
            self.resolve()

    def test_original_file_manifest_and_source_anchor_identity_must_close(self):
        original = copy.deepcopy(self.fixture.manifest)
        for mutate in (
            lambda row: row.update(embodiment_ref="tos.edition.wrong"),
            lambda row: row["payload_files"][0].update(file_id="tos.file.wrong"),
            lambda row: row["payload_files"][0].update(sha256="0" * 64),
        ):
            self.fixture.manifest = copy.deepcopy(original)
            mutate(self.fixture.manifest)
            self.fixture.write_json(self.fixture.manifest_ref, self.fixture.manifest)
            with self.assertRaises(NativeTextBindingError):
                self.resolve()
        self.fixture.manifest = original
        self.fixture.write_json(self.fixture.manifest_ref, original)
        self.fixture.anchor["target"]["item_id"] = "tos.item.wrong"
        self.fixture.refresh()
        with self.assertRaises(NativeTextBindingError):
            self.resolve()

    def test_layer_and_anchor_digests_are_not_original_file_digest(self):
        self.fixture.packet["source_layer"]["text_layer_sha256"] = self.fixture.original_digest
        for anchor in self.fixture.packet["anchors"]:
            anchor["text_layer_sha256"] = self.fixture.original_digest
        self.fixture.refresh()
        with self.assertRaises(NativeTextBindingError):
            self.resolve()

    def test_exact_mode_rejects_bad_token_digest_and_scope_escape(self):
        self.fixture.packet["anchors"][1]["exact_sha256"] = "0" * 64
        self.fixture.refresh()
        with self.assertRaises(NativeTextBindingError):
            self.resolve(verify_content=True, allow_private_content=True)
        self.fixture.packet["anchors"][1]["exact_sha256"] = digest(self.fixture.text[3:8].encode("utf-8"))
        self.fixture.layer["representation"]["text_scope"]["start"] = 4
        self.fixture.refresh()
        with self.assertRaises(NativeTextBindingError):
            self.resolve(verify_content=True, allow_private_content=True)

    def test_exact_mode_rejects_invalid_utf8_even_when_full_digest_is_rebound(self):
        corrupt = b"P\r\ncaf\xff \xce\xa9\r\nTAIL"
        self.fixture.write_bytes(self.fixture.content_ref, corrupt)
        self.fixture.layer["representation"].update(content_sha256=digest(corrupt),
                                                    content_file_id="tos.file.sha256." + digest(corrupt))
        self.fixture.packet["source_layer"]["text_layer_sha256"] = digest(corrupt)
        for anchor in self.fixture.packet["anchors"]:
            anchor["text_layer_sha256"] = digest(corrupt)
        self.fixture.refresh()
        with self.assertRaises(NativeTextBindingError):
            self.resolve(verify_content=True, allow_private_content=True)

    def test_unknown_native_packet_and_synthetic_lab_shortcut_fail_closed(self):
        for field, value in (("schema_version", "tos_unknown_native_packet_v1"),
                             ("content_posture", "public_synthetic_contract_exercise")):
            old = self.fixture.packet[field]
            self.fixture.packet[field] = value
            self.fixture.refresh()
            with self.assertRaises(NativeTextBindingError):
                self.resolve()
            self.fixture.packet[field] = old

    def test_duplicate_keys_and_nonfinite_values_are_not_silently_decoded(self):
        ref = self.fixture.refs["work"]
        saved = (self.root / ref).read_bytes()
        for extra in (b', "record_version": 1', b', "notes": NaN'):
            self.fixture.write_bytes(ref, saved.rstrip()[:-1] + extra + b"}\n")
            with self.assertRaises(NativeTextBindingError):
                self.resolve()
        self.fixture.write_bytes(ref, saved)

    def test_paths_symlinks_and_read_budgets_fail_closed(self):
        for ref in ("../outside.json", "/outside.json", "ToS/source-witnesses/payload/packet.json"):
            invalid = {**self.fixture.binding, "packet_ref": ref}
            with self.subTest(path_kind=ref.split("/")[0]), self.assertRaises(NativeTextBindingError):
                NativeTextBindingResolver(self.root).resolve(invalid)
        target = self.root / self.fixture.packet_ref
        other = target.with_name("symlink-target.json")
        target.rename(other)
        target.symlink_to(other)
        with self.assertRaises(NativeTextBindingError):
            self.resolve()
        target.unlink()
        other.rename(target)
        with self.assertRaises(NativeTextBindingError):
            NativeTextBindingResolver(self.root, max_metadata_bytes=1).resolve(self.fixture.binding)
        with self.assertRaises(NativeTextBindingError):
            NativeTextBindingResolver(self.root, max_content_bytes=1).resolve(
                self.fixture.binding, verify_content=True, allow_private_content=True)

    def test_symlink_ancestor_is_not_an_equivalent_owner_home(self):
        home = self.root / self.fixture.native_home
        other = self.root / "relocated-synthetic-native"
        home.rename(other)
        home.symlink_to(other, target_is_directory=True)
        with self.assertRaises(NativeTextBindingError):
            self.resolve()

    def test_schema_cannot_select_an_external_resource(self):
        ref = "ToS/contracts/source-text-unit-packet-v1.schema.json"
        schema = self.fixture.read_json(ref)
        schema["allOf"].append({"$ref": "https://example.invalid/private-schema-selector"})
        self.fixture.write_json(ref, schema)
        with self.assertRaises(NativeTextBindingError) as caught:
            self.resolve()
        self.assertNotIn("example.invalid", str(caught.exception))

    def test_snapshot_rechecks_exact_observed_metadata_and_schema_bytes(self):
        for ref in (self.fixture.packet_ref, self.fixture.layer_ref, self.fixture.rights_ref,
                    self.fixture.anchor_ref, self.fixture.manifest_ref, self.fixture.refs["work"],
                    "ToS/contracts/source-text-unit-packet-v1.schema.json"):
            resolver = NativeTextBindingResolver(self.root)
            resolver.resolve(self.fixture.binding)
            before = resolver.snapshot()
            self.assertRegex(before, r"^sha256:[a-f0-9]{64}$")
            saved = (self.root / ref).read_bytes()
            self.fixture.write_bytes(ref, saved + b" ")
            with self.subTest(dependency=Path(ref).name), self.assertRaises(NativeTextBindingError):
                resolver.snapshot()
            self.fixture.write_bytes(ref, saved)

    def test_snapshot_does_not_invent_content_verification_but_tracks_verified_content(self):
        metadata = NativeTextBindingResolver(self.root)
        metadata.resolve(self.fixture.binding)
        before = metadata.snapshot()
        self.fixture.write_bytes(self.fixture.content_ref, self.fixture.content + b"!")
        self.assertEqual(before, metadata.snapshot())
        self.fixture.write_bytes(self.fixture.content_ref, self.fixture.content)
        verified = NativeTextBindingResolver(self.root)
        verified.resolve(self.fixture.binding, verify_content=True, allow_private_content=True)
        self.fixture.write_bytes(self.fixture.content_ref, self.fixture.content + b"!")
        with self.assertRaises(NativeTextBindingError):
            verified.snapshot()

    def test_cached_resolve_does_not_refresh_or_ignore_a_stale_dependency(self):
        resolver = NativeTextBindingResolver(self.root)
        resolver.resolve(self.fixture.binding)
        ref = self.fixture.refs["expression"]
        self.fixture.write_bytes(ref, (self.root / ref).read_bytes() + b" ")
        with self.assertRaises(NativeTextBindingError):
            resolver.resolve(self.fixture.binding)

    def test_snapshot_accepts_protected_reader_but_refuses_changed_buffers(self):
        resolver = NativeTextBindingResolver(self.root)
        resolver.resolve(self.fixture.binding)
        seen = []

        def read(path, limit):
            self.assertNotIn("payload", path.parts)
            self.assertNotIn("local-content", path.parts)
            seen.append(path)
            raw = path.read_bytes()
            return raw + b" " if path == self.root / self.fixture.rights_ref else raw[:limit + 1]

        with self.assertRaises(NativeTextBindingError):
            resolver.snapshot(read_bytes=read)
        self.assertIn(self.root / self.fixture.rights_ref, seen)

    def test_errors_do_not_echo_source_paths_or_private_material(self):
        self.fixture.write_bytes(self.fixture.content_ref, b"SYNTHETIC_PRIVATE_SENTINEL")
        try:
            self.resolve(verify_content=True, allow_private_content=True)
        except NativeTextBindingError as error:
            message = str(error)
            self.assertNotIn("SYNTHETIC_PRIVATE_SENTINEL", message)
            self.assertNotIn(self.fixture.content_ref, message)
            self.assertNotIn(self.fixture.packet_ref, message)
            self.assertNotIn(str(self.root), message)
        else:
            self.fail("exact-content drift was accepted")

    def test_agent_assessment_cannot_be_fabricated_as_native_human_review(self):
        self.fixture.packet["reviews"] = [{
            "review_id": "tos.text-unit-review.sid-" + "a" * 32,
            "segmentation_refs": [self.fixture.binding["segmentation_id"]],
            "reviewer_kind": "agent", "reviewer_ref": "agent:synthetic-test",
            "reviewed_at": STAMP, "review_scope": "all_units",
            "reviewed_unit_refs": [self.fixture.binding["unit_id"]],
            "source_layer_ref": self.fixture.layer_ref, "source_layer_sha256": digest(self.fixture.content),
            "source_visible": True, "independent_boundary_decision_recorded_before_assistance": True,
            "unassisted_baseline_ref": self.fixture.policy_ref,
            "language_competence": {"language": "und", "declared": True, "scope": NOTICE},
            "outcome": "accepted", "notes_ref": self.fixture.policy_ref,
            "provenance_event_ref": "synthetic:no-real-review", "sample_does_not_accept_unreviewed_units": True,
        }]
        review_id = self.fixture.packet["reviews"][0]["review_id"]
        self.fixture.packet["segmentations"][0].update(status="accepted", review_refs=[review_id])
        self.fixture.packet["units"][0]["boundary_posture"] = "reviewed_accepted"
        self.fixture.refresh()
        with self.assertRaises(NativeTextBindingError):
            self.resolve()


if __name__ == "__main__":
    unittest.main()
