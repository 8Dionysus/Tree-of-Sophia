from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import acquire_registry_sources as acquisition


class RegistrySourceAcquisitionTests(unittest.TestCase):
    def test_translation_profile_keeps_language_and_version_role_explicit(self) -> None:
        prefix = b'<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader><fileDesc/></teiHeader>'
        body = prefix + ('<text><body><div type="translation" n="urn:cts:greekLit:test.eng1" xml:lang="eng">'
            '<div type="textpart" subtype="section" n="1">' + 'translated words ' * 100 + '</div>'
            '</div></body></text></TEI>').encode()
        target = {"slug": "fixture-english", "language": "en", "expression_role": "translation",
            "coverage": {"kind": "perseus-tei-translation", "citation_scope": "hierarchical_divisions",
                "cts_urn": "urn:cts:greekLit:test.eng1", "header_prefix_sha256": acquisition.sha256(prefix)}}
        report = acquisition.inspect_payloads(target, [({"basename": "source.xml"}, body)])
        self.assertGreater(report["files"][0]["latin_letter_count"], 1000)
        self.assertNotIn("greek_character_count", report["files"][0])
        prefix_target = copy.deepcopy(target)
        prefix_target['coverage'].update(reviewed_body_prefix_bytes=len(prefix) + 100,
            reviewed_body_prefix_sha256=acquisition.sha256(body[:len(prefix) + 100]))
        acquisition.inspect_payloads(prefix_target, [({"basename": "source.xml"}, body)])
        prefix_target['coverage']['reviewed_body_prefix_sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'source opening'):
            acquisition.inspect_payloads(prefix_target, [({"basename": "source.xml"}, body)])
        for changed in (body.replace(b'type="translation"', b'type="edition"'),
                body.replace(b'xml:lang="eng"', b'xml:lang="grc"'),
                body.replace(b'test.eng1', b'test.eng2'),
                body.replace(b'translated words ', '\u03b1'.encode())):
            with self.subTest(changed=changed[-100:]), self.assertRaises(ValueError):
                acquisition.inspect_payloads(target, [({"basename": "source.xml"}, changed)])
        with self.assertRaises(ValueError):
            acquisition.inspect_payloads({**target, "expression_role": "source_language"}, [({"basename": "source.xml"}, body)])

    def test_latin_profile_binds_reviewed_identity_carrier_and_source_language(self) -> None:
        prefix = b'<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader><fileDesc/></teiHeader>'
        urn = "urn:cts:latinLit:fixture.lat1"
        body = prefix + ('<text><body xml:base="' + urn + '"><div type="edition" xml:lang="lat">'
            '<div type="textpart" subtype="book" n="1">' + 'ratio et natura ' * 100 + '</div>'
            '</div></body></text></TEI>').encode()
        target = {"slug": "fixture-latin", "language": "la", "expression_role": "source_language",
            "coverage": {"kind": "perseus-tei-latin-work", "citation_scope": "hierarchical_divisions",
                "cts_urn": urn, "identity_anchor": "body_xml_base", "header_prefix_sha256": acquisition.sha256(prefix)}}
        report = acquisition.inspect_payloads(target, [({"basename": "source.xml"}, body)])
        self.assertGreater(report["files"][0]["latin_letter_count"], 1000)
        for changed in (body.replace(b'fixture.lat1', b'fixture.lat2'),
                body.replace(b'type="edition"', b'type="translation"'),
                body.replace(b'xml:lang="lat"', b'xml:lang="eng"'),
                body.replace(b'type="edition"', b'type="edition" n="urn:cts:latinLit:other.lat1"')):
            with self.subTest(changed=changed[-100:]), self.assertRaises(ValueError):
                acquisition.inspect_payloads(target, [({"basename": "source.xml"}, changed)])
        for changes in ({"language": "en"}, {"expression_role": "translation"}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                acquisition.inspect_payloads({**target, **changes}, [({"basename": "source.xml"}, body)])
        target["coverage"]["identity_anchor"] = "edition_n"
        with self.assertRaises(ValueError):
            acquisition.inspect_payloads(target, [({"basename": "source.xml"}, body)])
        named = body.replace(b'type="edition"', ('type="edition" n="' + urn + '"').encode())
        acquisition.inspect_payloads(target, [({"basename": "source.xml"}, named)])

    def test_translation_body_identity_and_section_milestones(self) -> None:
        prefix = b'<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader><fileDesc/></teiHeader>'
        urn = "urn:cts:latinLit:fixture.eng1"
        body = prefix + ('<text><body xml:base="' + urn + '"><div type="translation" xml:lang="eng">'
            '<div type="textpart" subtype="book" n="1"><p><milestone unit="section" n="1"/>'
            + 'English translation ' * 100 + '</p></div>'
            '<div type="textpart" subtype="book" n="2"><p><milestone unit="section" n="1"/>text</p></div>'
            '</div></body></text></TEI>').encode()
        target = {"slug": "milestones", "language": "en", "expression_role": "translation",
            "coverage": {"kind": "perseus-tei-translation", "cts_urn": urn, "identity_anchor": "body_xml_base",
                "citation_scope": "hierarchical_divisions_and_section_milestones", "header_prefix_sha256": acquisition.sha256(prefix)}}
        report = acquisition.inspect_payloads(target, [({"basename": "source.xml"}, body)])
        self.assertEqual(report["files"][0]["division_count"], 4)
        repeated = body.replace(b'<p>', b'<p><milestone unit="section" n="1"/>', 1)
        observed = acquisition.inspect_payloads(target, [({"basename": "source.xml"}, repeated)])["files"][0]
        self.assertEqual(observed["division_count"], 5)
        self.assertEqual(observed["repeated_section_markers"], [{"source_address": [("book", "1"), ("division_occurrence", "1"), ("section", "1")], "occurrences": 2}])
        self.assertNotEqual(observed["division_addresses_sha256"], report["files"][0]["division_addresses_sha256"])
        for changed in (body.replace(b'unit="section" n="1"', b'unit="section"'),
                body.replace(b'type="translation"', b'type="translation" n="urn:cts:latinLit:wrong.eng1"'),
                body.replace(b'xml:base=', b'wrong='),
                body.replace(b'xml:lang="eng"', b'xml:lang="lat"')):
            with self.subTest(changed=changed[:250]), self.assertRaises(ValueError):
                acquisition.inspect_payloads(target, [({"basename": "source.xml"}, changed)])

    def test_occurrence_profile_retains_repeated_division_labels_without_rewriting(self) -> None:
        import xml.etree.ElementTree as ET
        text = '<div xmlns="http://www.tei-c.org/ns/1.0"><div type="textpart" subtype="book" n="1">First</div><div type="textpart" subtype="book" n="1">Third</div></div>'
        edition = ET.fromstring(text)
        with self.assertRaisesRegex(ValueError, "duplicated"):
            acquisition.tei_division_addresses(edition)
        duplicates = []
        addresses = acquisition.tei_division_addresses(edition, repeated_divisions=duplicates)
        self.assertEqual(len(set(addresses)), 2)
        self.assertEqual(duplicates, [{"source_address": [("book", "1")], "occurrences": 2}])
        self.assertEqual([node.get("n") for node in edition], ["1", "1"])

    def test_unnumbered_containers_preserve_scope_and_reject_ambiguous_or_leaf_paths(self) -> None:
        import xml.etree.ElementTree as ET
        text = ('<div xmlns="http://www.tei-c.org/ns/1.0">'
                '<div type="textpart" subtype="fragments"><div type="textpart" subtype="section" n="1">fragment</div></div>'
                '<div type="textpart" subtype="speech"><div type="textpart" subtype="section" n="1">speech</div></div></div>')
        edition = ET.fromstring(text)
        before = ET.tostring(edition)
        paths = [json.loads(value) for value in acquisition.tei_division_addresses(edition)]
        self.assertEqual(paths, [[["fragments", None]], [["fragments", None], ["section", "1"]],
                                [["speech", None]], [["speech", None], ["section", "1"]]])
        self.assertEqual(ET.tostring(edition), before)
        for changed in (text.replace('subtype="speech"', 'subtype="fragments"'),
                        text.replace(' n="1"', ''), text.replace('subtype="speech"', 'subtype="speech" n=""'),
                        text.replace('subtype="speech"', 'subtype="speech" n=" "')):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                acquisition.tei_division_addresses(ET.fromstring(changed))

    def test_metadata_elapsed_requires_retained_measurement_or_valid_interval(self) -> None:
        observation = {"started_at": "2026-09-09T07:00:00+00:00", "ended_at": "2026-09-09T07:00:02.5+00:00"}
        self.assertEqual(acquisition.metadata_elapsed_seconds(observation), 2.5)
        self.assertEqual(acquisition.metadata_elapsed_seconds({**observation, "elapsed_seconds": 1.2}), 1.2)
        for value in (-1, float("inf"), float("nan"), True, "unknown"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                acquisition.metadata_elapsed_seconds({**observation, "elapsed_seconds": value})
        with self.assertRaises(ValueError):
            acquisition.metadata_elapsed_seconds({**observation, "ended_at": "2026-09-09T06:59:59+00:00"})

    def test_existing_item_resumes_only_missing_discovery_bound_to_same_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "manifest.json"
            manifest.write_text("{}")
            item = root / "item"
            item.mkdir()
            (item / "item.json").write_text("{}")
            event = {"event_type": "acquisition", "inputs": [{"ref": "manifest.json", "sha256": acquisition.sha256(manifest.read_bytes())}]}
            (item / "provenance.jsonl").write_text(json.dumps(event) + "\n")
            target = {"slug": "resume", "ids": {"item": "tos.item.resume"},
                "paths": {"item_root": "item"}, "files": [{"basename": "source.xml"}]}
            preparation = {"prepared_packages_ref": "packages.jsonl"}
            package = {"claims": []}
            with patch.object(acquisition, "verify_target", return_value={"verified": True}), \
                    patch.object(acquisition, "transfer", return_value=(b"text", {"status": "completed"})) as transfer, \
                    patch.object(acquisition, "write_discovery") as discovery:
                self.assertEqual(acquisition.install_target(root, manifest, preparation, target, package), {"verified": True})
                transfer.assert_called_once()
                discovery.assert_called_once()
                self.assertEqual(discovery.call_args.args[-1], event)
            event["inputs"][0]["sha256"] = "0" * 64
            (item / "provenance.jsonl").write_text(json.dumps(event) + "\n")
            with patch.object(acquisition, "verify_target", return_value={}), patch.object(acquisition, "transfer") as transfer:
                with self.assertRaisesRegex(ValueError, "does not bind"):
                    acquisition.install_target(root, manifest, preparation, target, package)
                transfer.assert_not_called()

    def test_claim_collision_stops_the_whole_batch_before_installation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            claim_ref = acquisition.SOURCE + "/relations/work-expression/work-expression-claims.jsonl"
            old = {"claim_id": "tos.claim.shared-title", "subject_ref": "tos.work.cicero.de-fato",
                "predicate": "has_expression", "object": "tos.expression.cicero.de-fato.la"}
            incoming = {**old, "subject_ref": "tos.work.plutarch.de-fato", "object": "tos.expression.plutarch.de-fato.grc"}
            acquisition.append_jsonl(root / claim_ref, old)
            targets = [{"slug": slug, "ids": {"item": "tos.item." + slug}} for slug in ("first", "de-fato")]
            packages = {"first": {"claims": []}, "de-fato": {"claims": [{"path": claim_ref, "record": incoming}]}}
            before = {p.relative_to(root): p.read_bytes() for p in root.rglob("*") if p.is_file()}
            with (patch.object(acquisition, "ROOT", root),
                  patch.object(acquisition.sys, "argv", ["acquire", "acquire", "--manifest", str(root / "manifest.json"),
                      "--preparation-receipt", str(root / "checkpoint.json")]),
                  patch.object(acquisition, "load_preparation", return_value=({"targets": targets}, packages)),
                  patch.object(acquisition, "check_preparation_receipt"),
                  patch.object(acquisition, "install_target") as install,
                  patch.object(acquisition, "transfer") as transfer):
                with self.assertRaisesRegex(ValueError, "existing bibliographic claim"):
                    acquisition.main()
                install.assert_not_called()
                transfer.assert_not_called()
            self.assertEqual(before, {p.relative_to(root): p.read_bytes() for p in root.rglob("*") if p.is_file()})

    def test_identity_collision_precedes_transfer_and_existing_item_shortcut(self) -> None:
        for collision in ("claim", "claim-other-owner", "discovery", "discovery-item", "discovery-other-path"):
            for installed in (False, True):
                with self.subTest(collision=collision, installed=installed), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    claim_ref = acquisition.SOURCE + "/relations/work-expression/work-expression-claims.jsonl"
                    claim = {"claim_id": "tos.claim.shared-title", "subject_ref": "tos.work.plutarch.de-fato"}
                    target = {"slug": "de-fato", "operation_date": "2026-09-09",
                        "ids": {"work": "tos.work.plutarch.de-fato", "item": "tos.item.plutarch.de-fato"},
                        "paths": {"item_root": "item"}}
                    package = {"claims": [{"path": claim_ref, "record": claim}]}
                    if collision.startswith("claim"):
                        owner_ref = claim_ref if collision == "claim" else acquisition.SOURCE + "/relations/other/source-claims.jsonl"
                        acquisition.append_jsonl(root / owner_ref, {**claim, "subject_ref": "tos.work.cicero.de-fato"})
                    else:
                        run_ref = acquisition.SOURCE + "/discovery/runs/registry-de-fato.2026-09-09.v1.json"
                        if collision == "discovery-other-path":
                            run_ref = acquisition.SOURCE + "/discovery/runs/other.json"
                        known_ids = list(target["ids"].values())
                        if collision == "discovery":
                            known_ids = ["tos.work.cicero.de-fato", "tos.item.cicero.de-fato"]
                        elif collision == "discovery-item":
                            known_ids[-1] = "tos.item.other-edition"
                        acquisition.write_json(root / run_ref, {"discovery_id": "tos.discovery.registry-de-fato.2026-09-09.v1",
                            "target": {"known_tos_refs": known_ids}})
                    if installed:
                        acquisition.write_json(root / "item/item.json", {})
                    before = {p.relative_to(root): p.read_bytes() for p in root.rglob("*") if p.is_file()}
                    with (patch.object(acquisition, "verify_target") as verify,
                          patch.object(acquisition, "transfer") as transfer,
                          patch.object(acquisition, "write_discovery") as discovery):
                        with self.assertRaisesRegex(ValueError, "existing bibliographic claim|discovery run identity collision"):
                            acquisition.install_target(root, root / "manifest.json", {}, target, package)
                        verify.assert_not_called()
                        transfer.assert_not_called()
                        discovery.assert_not_called()
                    self.assertEqual(before, {p.relative_to(root): p.read_bytes() for p in root.rglob("*") if p.is_file()})

    def test_identical_claim_and_discovery_allow_idempotent_item_verification(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = {"slug": "same", "ids": {"work": "tos.work.same", "item": "tos.item.same"},
                "paths": {"item_root": "item"}}
            claim_ref = acquisition.SOURCE + "/relations/work-expression/work-expression-claims.jsonl"
            claim = {"claim_id": "tos.claim.same", "subject_ref": "tos.work.same"}
            package = {"claims": [{"path": claim_ref, "record": claim}]}
            acquisition.append_jsonl(root / claim_ref, claim)
            acquisition.write_json(root / "item/item.json", {})
            run_ref = acquisition.SOURCE + "/discovery/runs/registry-same.2026-09-08.v1.json"
            acquisition.write_json(root / run_ref, {"discovery_id": "tos.discovery.registry-same.2026-09-08.v1",
                "target": {"known_tos_refs": list(reversed(target["ids"].values()))}})
            before = {p.relative_to(root): p.read_bytes() for p in root.rglob("*") if p.is_file()}
            with (patch.object(acquisition, "verify_target", return_value={"verified": True}) as verify,
                  patch.object(acquisition, "transfer") as transfer,
                  patch.object(acquisition, "write_discovery") as discovery):
                self.assertEqual(acquisition.install_target(root, root / "manifest.json",
                    {"prepared_packages_ref": "packages.jsonl"}, target, package), {"verified": True})
                verify.assert_called_once_with(root, target)
                transfer.assert_not_called()
                discovery.assert_not_called()
            self.assertEqual(before, {p.relative_to(root): p.read_bytes() for p in root.rglob("*") if p.is_file()})
            second = {**target, "slug": "second"}
            with self.assertRaisesRegex(ValueError, "duplicate prepared bibliographic claim ID"):
                acquisition.preflight_identities(root, [target, second], {"same": package, "second": package})

    def test_existing_work_extension_preserves_identity_and_prior_assertions(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            work_ref = 'ToS/source-witnesses/works/author/work/work.json'
            before_ref = 'ToS/source-witnesses/discovery/batch/work-before.json'
            old = {"record_type": "work", "record_id": "tos.work.author.work", "preferred_label": "Original label",
                "source_refs": ["old-evidence"], "expression_claim_refs": ["tos.claim.old"], "record_version": 2}
            before = acquisition.json_bytes(old)
            acquisition.safe_path(root, before_ref).parent.mkdir(parents=True)
            acquisition.safe_path(root, before_ref).write_bytes(before)
            expected = {**old, "expression_claim_refs": ["tos.claim.old", "tos.claim.new"], "record_version": 3}
            target = {"ids": {"work": old["record_id"]}, "paths": {"work": work_ref}}
            package = {"existing_work": {"record_ref": work_ref, "preimage_ref": before_ref, "sha256": acquisition.sha256(before)},
                "records": {work_ref: expected, "new-expression": {"record_type": "expression", "record_id": "tos.expression.new", "work_ref": old["record_id"]}},
                "claims": [{"record": {"subject_ref": old["record_id"], "predicate": "has_expression", "object": "tos.expression.new", "claim_id": "tos.claim.new"}}]}
            self.assertEqual(acquisition.validate_work_extension(root, target, package), (work_ref, before))
            for field, value in (("preferred_label", "Silently renamed"), ("source_refs", []),
                    ("expression_claim_refs", ["tos.claim.new"]), ("record_version", 4)):
                changed = copy.deepcopy(package)
                changed["records"][work_ref][field] = value
                with self.subTest(field=field), self.assertRaises(ValueError):
                    acquisition.validate_work_extension(root, target, changed)
            changed = copy.deepcopy(package)
            changed["claims"][0]["record"]["object"] = "tos.expression.other"
            with self.assertRaises(ValueError):
                acquisition.validate_work_extension(root, target, changed)
            acquisition.safe_path(root, before_ref).write_bytes(before + b' ')
            with self.assertRaises(ValueError):
                acquisition.validate_work_extension(root, target, package)

    def test_existing_work_drift_stops_before_network_or_source_replacement(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            work_ref = 'ToS/source-witnesses/works/author/work/work.json'
            work = acquisition.safe_path(root, work_ref)
            work.parent.mkdir(parents=True)
            work.write_bytes(b'changed by another source operation')
            target = {"slug": "work", "ids": {"item": "tos.item.work"},
                "paths": {"item_root": 'ToS/source-witnesses/works/author/work/expressions/en/editions/one/items/one'}}
            preparation = {"prepared_packages_ref": 'prepared.jsonl'}
            package = {"records": {work_ref: {}}, "claims": []}
            with (patch.object(acquisition, 'validate_work_extension', return_value=(work_ref, b'original')),
                  patch.object(acquisition, 'transfer') as transfer):
                with self.assertRaisesRegex(ValueError, 'refusing replacement'):
                    acquisition.install_target(root, root / 'manifest.json', preparation, target, package)
                transfer.assert_not_called()
            self.assertEqual(work.read_bytes(), b'changed by another source operation')

    def test_large_batch_keeps_target_evidence_exact_and_rejects_missing_refs(self) -> None:
        preparation = {"metadata_observations": [{"retained_ref": ref} for ref in ('perseus/license', 'perseus/one', 'perseus/other')]}
        target = {"provider": "perseus", "metadata_evidence_refs": ['perseus/license', 'perseus/one']}
        self.assertEqual(acquisition.selected_metadata_observations(preparation, target), preparation['metadata_observations'][:2])
        for refs in ([], ['missing'], ['perseus/one', 'perseus/one']):
            with self.subTest(refs=refs), self.assertRaises(ValueError):
                acquisition.selected_metadata_observations(preparation, {**target, 'metadata_evidence_refs': refs})
        self.assertEqual(acquisition.selected_metadata_observations(preparation, {'provider': 'perseus'}), preparation['metadata_observations'])

    def test_perseus_hierarchical_citations_distinguish_repeated_section_numbers(self) -> None:
        prefix = b'<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader><fileDesc/></teiHeader>'
        books = ''.join(f'<div type="textpart" subtype="book" n="{number}">'
            '<div type="textpart" subtype="section" n="1">' + '\u03b1' * 600 + '</div></div>' for number in ('1', '2'))
        body = prefix + ('<text><body><div type="edition" n="urn:cts:greekLit:test.grc1" xml:lang="grc">'
            + books + '</div></body></text></TEI>').encode()
        target = {"slug": "fixture", "coverage": {"kind": "perseus-tei-work", "citation_scope": "hierarchical_divisions",
            "cts_urn": "urn:cts:greekLit:test.grc1", "header_prefix_sha256": acquisition.sha256(prefix)}}
        report = acquisition.inspect_payloads(target, [({"basename": "source.xml"}, body)])
        self.assertEqual(report["files"][0]["division_count"], 4)
        self.assertEqual(json.loads(report["files"][0]["last_division"]), [["book", "2"], ["section", "1"]])
        absent = acquisition.inspect_payloads(target, [({"basename": "source.xml"}, body.replace(b'n="2"', b''))])["files"][0]
        self.assertEqual(absent["unnumbered_containers"], [[["book", None]]])
        self.assertEqual(json.loads(absent["last_division"]), [["book", None], ["section", "1"]])
        for altered in (body.replace(b'n="2"', b'n="1"'), body.replace(b'n="2"', b'n=""')):
            with self.subTest(altered=altered[-200:]), self.assertRaises(ValueError):
                acquisition.inspect_payloads(target, [({"basename": "source.xml"}, altered)])

    def test_perseus_checks_reviewed_header_cts_language_and_section_identity(self) -> None:
        prefix = b'<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader><fileDesc/></teiHeader>'
        body = prefix + ('<text><body><div type="edition" n="urn:cts:greekLit:test.grc1" xml:lang="grc">'
                         '<div subtype="section" n="1a">' + '\u03b1\u0313' * 1100 + '</div>'
                         '</div></body></text></TEI>').encode()
        target = {"slug": "fixture", "coverage": {"kind": "perseus-tei-work",
                  "cts_urn": "urn:cts:greekLit:test.grc1", "header_prefix_sha256": acquisition.sha256(prefix)}}
        report = acquisition.inspect_payloads(target, [({"basename": "source.xml"}, body)])
        self.assertEqual(report["files"][0]["first_section"], "1a")
        self.assertEqual(report["files"][0]["greek_character_count"], 1100)
        self.assertFalse(report["source_bytes_changed"])
        for altered in (body.replace(b"fileDesc", b"sourceDesc"),
                        body.replace(b"test.grc1", b"other.grc1"),
                        body.replace(b'xml:lang="grc"', b'xml:lang="eng"'),
                        body.replace(b'</div></body>', b'<div subtype="section" n="1a">duplicate</div></div></body>'),
                        body.replace(('\u03b1\u0313' * 1100).encode(), b"only English")):
            with self.subTest(altered=altered[:150]), self.assertRaises(ValueError):
                acquisition.inspect_payloads(target, [({"basename": "source.xml"}, altered)])

    def test_topology_refresh_keeps_one_jsonl_record_and_retains_exact_preimage(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            topology_path = root / acquisition.TOPOLOGY
            topology_path.parent.mkdir(parents=True)
            original = {"event_id": acquisition.TOPOLOGY_EVENT, "event_version": 1,
                        "method": {"configuration": {}}, "inputs": [], "outputs": []}
            original_bytes = (json.dumps(original, ensure_ascii=False) + "\n").encode()
            topology_path.write_bytes(original_bytes)
            evidence_path = root / "ToS/source-witnesses/fixture/record.json"
            evidence_path.parent.mkdir(parents=True)
            evidence_path.write_text('{"label":"source evidence"}\n')
            for route in ("work-expression", "expression-edition", "edition-item"):
                path = topology_path.parent / route / f"{route}-claims.jsonl"
                path.parent.mkdir()
                path.write_text(json.dumps({"evidence_refs": [evidence_path.relative_to(root).as_posix()]}) + "\n")
            evidence_root = root / "prepared-evidence"
            acquisition.refresh_topology(root, evidence_root, "2026-09-08T00:00:00+00:00")
            lines = topology_path.read_text().splitlines()
            self.assertEqual(len(lines), 1)
            refreshed = json.loads(lines[0])
            self.assertEqual(refreshed["event_id"], original["event_id"])
            self.assertEqual(refreshed["event_version"], 2)
            self.assertEqual(len(refreshed["outputs"]), 3)
            self.assertEqual(refreshed["inputs"][0]["sha256"], hashlib.sha256(evidence_path.read_bytes()).hexdigest())
            preserved = evidence_root / "topology-before" / (hashlib.sha256(original_bytes).hexdigest() + ".json")
            self.assertEqual(preserved.read_bytes(), original_bytes)

    def test_acquired_file_companions_bind_every_file_without_mutating_prepared_rights(self) -> None:
        prepared = {"scope_refs": ["tos.item.example"], "license_uri": "https://creativecommons.org/licenses/by/4.0/",
            "layer_assessments": [{"assessment_status": "public_domain_reviewed", "rights_statement_uri": "https://creativecommons.org/licenses/by/4.0/"},
                                  {"assessment_status": "licensed", "rights_statement_uri": "https://creativecommons.org/licenses/by/4.0/"}]}
        original = copy.deepcopy(prepared)
        manifest = {"payload_files": [{"file_id": f"tos.file.sha256.{digit * 64}",
            "sha256": digit * 64, "relative_path": f"payload/source-{digit}.json"} for digit in ("a", "b")]}
        acquired = acquisition.rights_with_file_scopes(prepared, manifest)
        self.assertEqual(prepared, original)
        self.assertEqual(acquired["scope_refs"], ["tos.item.example", "tos.file.sha256." + "a" * 64, "tos.file.sha256." + "b" * 64])
        self.assertEqual(acquired["layer_assessments"][0]["rights_statement_uri"], "https://creativecommons.org/publicdomain/mark/1.0/")
        self.assertEqual(acquired["layer_assessments"][1], original["layer_assessments"][1])
        self.assertEqual(acquired["license_uri"], original["license_uri"])
        self.assertEqual(acquisition.rights_with_file_scopes(acquired, manifest), acquired)
        self.assertEqual(acquisition.manifest_fixity(manifest), "a" * 64 + "  payload/source-a.json\n" + "b" * 64 + "  payload/source-b.json\n")

    def test_repository_paths_reject_escape_absolute_and_symlink_routes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for ref in ("../outside", "/outside", "a/../outside", "a\\outside", "a//b", "a/./b", "a\0b"):
                with self.subTest(ref=ref), self.assertRaises(ValueError):
                    acquisition.safe_path(root, ref)
            (root / "link").symlink_to(root.parent, target_is_directory=True)
            with self.assertRaises(ValueError):
                acquisition.safe_path(root, "link/outside")
            self.assertEqual(acquisition.safe_path(root, "ToS/source-witnesses/work.json"), root / "ToS/source-witnesses/work.json")

    def test_payload_fixity_checks_size_and_git_blob_identity(self) -> None:
        body = b"exact\r\n"
        entry = {"basename": "source.txt", "byte_size": len(body),
                 "git_blob_sha1": hashlib.sha1(b"blob 7\0" + body).hexdigest()}
        self.assertEqual(acquisition.check_file(body, entry), hashlib.sha256(body).hexdigest())
        for altered in (b"exact\n", b"other\r\n"):
            with self.subTest(altered=altered), self.assertRaises(ValueError):
                acquisition.check_file(altered, entry)

    def test_json_rejects_duplicate_keys_and_nonstandard_numbers(self) -> None:
        for body in (b'{"x":1,"x":2}', b'{"x":NaN}', b'{"x":Infinity}', b'{"x":-Infinity}'):
            with self.subTest(body=body), self.assertRaises(ValueError):
                acquisition.strict_json(body)
        decomposed = "e\u0301"
        self.assertEqual(acquisition.strict_json(json.dumps({"x": decomposed}).encode()), {"x": decomposed})

    def test_preparation_receipt_requires_the_exact_digest_commit_and_checks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "manifest.json"
            manifest.write_text('{"version":1}\n')
            receipt_path = root / "checkpoint.json"
            receipt = {"manifest_sha256": acquisition.sha256(manifest.read_bytes()), "commit": "a" * 40,
                "runtime_session_id": "test-session", "checkpoint_review_ref": "review:test",
                "passed_checks": ["prepared source closure"], "status": "passed"}
            with patch.object(acquisition.subprocess, "check_output", return_value="a" * 40 + "\n"):
                receipt_path.write_text(json.dumps(receipt))
                acquisition.check_preparation_receipt(root, manifest, receipt_path)
                for field, value in (("manifest_sha256", "b" * 64), ("commit", "b" * 40),
                                     ("runtime_session_id", ""), ("checkpoint_review_ref", ""),
                                     ("passed_checks", []), ("passed_checks", [True]), ("status", "pending")):
                    altered = {**receipt, field: value}
                    receipt_path.write_text(json.dumps(altered))
                    with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                        acquisition.check_preparation_receipt(root, manifest, receipt_path)

    def test_pinned_manifest_rejects_an_unbound_payload_url(self) -> None:
        manifest_path = ROOT / "ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/manifest.json"
        manifest = json.loads(manifest_path.read_bytes())
        manifest["targets"][0]["files"][0]["url"] = "https://example.invalid/unbound-source.xml"
        with tempfile.TemporaryDirectory() as directory:
            changed = Path(directory) / "manifest.json"
            changed.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "unbound source URL"):
                acquisition.load_preparation(ROOT, changed)

    def test_osis_validates_real_book_addresses_and_retains_source_characters(self) -> None:
        target = {"slug": "example", "coverage": {"kind": "osis-book", "book": "Prov", "chapter_count": 1}}
        body = ('<osis xmlns="http://www.bibletechnologies.net/2003/OSIS/namespace">'
                '<chapter osisID="Prov.1"><verse osisID="Prov.1.1"><w>א\u05b7</w></verse></chapter></osis>').encode()
        report = acquisition.inspect_payloads(target, [({"basename": "Prov.xml"}, body)])
        self.assertFalse(report["source_bytes_changed"])
        self.assertEqual(report["files"][0]["chapter_ids"], ["Prov.1"])
        self.assertEqual(report["files"][0]["word_count"], 1)
        for altered in (body.replace(b"Prov.1.1", b"Job.1.1"), body.replace(b"Prov.1\"", b"Prov.2\"")):
            with self.subTest(altered=altered), self.assertRaises(ValueError):
                acquisition.inspect_payloads(target, [({"basename": "Prov.xml"}, altered)])

    def test_bilara_refuses_wrong_work_and_missing_required_unit(self) -> None:
        target = {"slug": "khuddakapatha", "coverage": {"kind": "bilara-root", "uid": "kp", "file_count": 2}}
        bodies = [({"basename": f"kp{number}_root-pli-ms.json"}, json.dumps({f"kp{number}:1": "source"}).encode()) for number in (1, 2)]
        self.assertEqual(acquisition.inspect_payloads(target, bodies)["unique_segment_count"], 2)
        for altered in (bodies[:1], [bodies[0], ({"basename": "kp2_root-pli-ms.json"}, b'{"mn9:1":"source"}')]):
            with self.subTest(altered=altered), self.assertRaises(ValueError):
                acquisition.inspect_payloads(target, altered)

    def test_oraec_components_cannot_be_flattened_into_one_source_language(self) -> None:
        target = {"slug": "egyptian-example", "coverage": {"kind": "oraec-composition"},
                  "ids": {"expression": "tos.expression.example.egy", "translation_expression": "tos.expression.example.de"}}
        parsed = {"sentences": [{"translation": "German translation", "words": [{"written_form": "Egyptian form"}]}]}
        body = json.dumps(parsed).encode()
        result = acquisition.inspect_payloads(target, [({"basename": "source.json"}, body)])
        components = result["component_witnesses"]
        self.assertEqual([component["language"] for component in components], ["egy", "de"])
        self.assertEqual(len({component["file_id"] for component in components}), 1)
        self.assertEqual(components[0]["selectors"][0]["json_pointer_pattern"], "/sentences/*/words/*/written_form")
        self.assertNotIn("German translation", json.dumps(result))
        with_gloss = copy.deepcopy(parsed)
        with_gloss["sentences"][0]["words"][0]["cotext_translation"] = "German lexical gloss"
        gloss_report = acquisition.inspect_payloads(target, [({"basename": "source.json"}, json.dumps(with_gloss).encode())])
        self.assertEqual(gloss_report["component_witnesses"][1]["selectors"], [
            {"json_pointer_pattern": "/sentences/*/translation", "nonempty_string_count": 1},
            {"json_pointer_pattern": "/sentences/*/words/*/cotext_translation", "nonempty_string_count": 1},
        ])
        self.assertNotIn("German lexical gloss", json.dumps(gloss_report))
        without_translation = copy.deepcopy(parsed)
        del without_translation["sentences"][0]["translation"]
        with self.assertRaisesRegex(ValueError, "separate German"):
            acquisition.inspect_payloads(target, [({"basename": "source.json"}, json.dumps(without_translation).encode())])


if __name__ == "__main__":
    unittest.main()
