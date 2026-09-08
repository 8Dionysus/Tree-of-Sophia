"""Synthetic public Occurrence assessment keeps its native dependency closure.

The source description needs an explicitly selected, exactly matching native
read for admission. Metadata-only readers keep history without inheriting that
eligibility. No historical text, rights judgment or source admission is asserted.
"""
from __future__ import annotations

import copy
from contextlib import contextmanager
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[3]
sys.path[:0] = [str(ROOT / "mechanics/growth-cycle/tests"), str(ROOT / "scripts"),
               str(ROOT / "tests"),
               str(ROOT / "mechanics/growth-cycle/parts/branch-growth-cycle/scripts")]

from assessment_journal import AssessmentJournal, AssessmentRejected, JournalConflict
from knowledge_assessment import AssessmentEngine, Record
from native_text_binding import NativeTextBindingError, NativeTextBindingResolver
from source_record_profiles import SourceProfileError, SourceRecordProfiles
from source_witness_human_forms import claim_forms_path
from test_native_text_assessment import NativeAssessmentFixture, ORIGIN
from test_occurrence_growth import copy_contracts, occurrence


class OccurrenceAssessmentGuardTests(unittest.TestCase):
    def fixture(self, *, read_scope="exact_public"):
        fixture = NativeAssessmentFixture(self)
        fixture.native.make_public()
        fixture.rebind(read_scope)
        copy_contracts(fixture.root)
        body = occurrence(fixture.native.binding)
        path = "ToS/source-witnesses/lexical-descriptions/synthetic/occurrence.json"
        fixture.native.write_json(path, body)
        scope = copy.deepcopy(fixture.scope)
        fixture.native_subject = fixture.subject
        fixture.identifier = body["record_id"]
        fixture.subject = Record.from_payload(body["record_id"], body["record_version"], body)
        scope["record"] = fixture.subject.ref
        fixture.config["source_records"] = [
            {"path": path, "record_id": fixture.identifier, "origin_id": ORIGIN}]
        fixture.config["subjects"][fixture.identifier] = scope
        for authority in fixture.config["authorities"]:
            authority["payload"]["subject_prefixes"] = ["tos.occurrence."]
        fixture.save()
        return fixture

    def disable_native_read(self, fixture):
        fixture.config["schema_version"] = "tos_local_assessment_owner_v2"
        fixture.config.pop("native_text_units")
        fixture.config["subjects"] = {fixture.identifier: fixture.scope}
        # Preserve the same public layer evidence and origin so a missing
        # evidence record cannot accidentally mask the source-read guard.
        fixture.config["records"].append({"id": fixture.layer.id, "version": fixture.layer.version,
            "payload": fixture.layer.payload, "origin_id": ORIGIN})
        fixture.save()

    def select_other_packet(self, fixture, *, same_unit=False):
        packet = copy.deepcopy(fixture.native.packet)
        packet["packet_id"] = "tos.source-text-unit-packet.sid-" + "d" * 32
        binding = copy.deepcopy(fixture.native.binding)
        if not same_unit:
            other_id = "tos.text-unit.sid-" + "e" * 32
            packet["units"][0]["unit_id"] = other_id
            packet["segmentations"][0]["ordered_unit_refs"] = [other_id]
            binding["unit_id"] = other_id
        ref = fixture.native.native_home + "/source-text-unit.other-packet.v1.json"
        fixture.native.write_json(ref, packet)
        binding.update(packet_ref=ref, packet_id=packet["packet_id"],
                       packet_sha256=fixture.native.file_digest(ref))
        selected = NativeTextBindingResolver(fixture.root).assessment_records(
            binding, origin_id=ORIGIN, verify_content=True)
        unit = Record.from_payload(**selected["records"][0])
        scope = fixture.config["subjects"].pop(fixture.native_subject.id)
        scope["record"] = unit.ref
        fixture.config["subjects"][unit.id] = scope
        fixture.config["native_text_units"][0]["binding"] = binding
        fixture.save()

    def claim_fixture(self, *, form=False, read_scope="exact_public"):
        """An artificial pair assignment; no real Claim or assessment is made."""
        fixture = self.fixture(read_scope=read_scope)
        fixture.occurrence_subject = fixture.subject
        fixture.occurrence_path = fixture.config["source_records"][0]["path"]
        lexical = copy.deepcopy(fixture.subject.payload)
        lexical.pop("native_text_binding")
        lexical.update(schema_version="tos_lexical_description_record_v1", record_type="lexical-form",
                       record_id="tos.lexical-form.synthetic.claim-grounding")
        lexical["form_identity"] = {"written_representation": "synthetic-form", "language": "und",
            "script": "Latn", "representation_kind": "unknown",
            "notation_scope": "Synthetic fixture representation only.",
            "unicode_posture": "preserved_as_supplied"}
        lexical["semantic_content"] = {"form_account": "Synthetic form, not historical evidence.",
                                       "language": "en", "script": "Latn"}
        lexical_path = "ToS/source-witnesses/lexical-descriptions/synthetic-claim/lexical-form.json"
        fixture.native.write_json(lexical_path, lexical)
        fixture.lexical = Record.from_payload(lexical["record_id"], 1, lexical)
        claim = {"schema_version": "tos_semantic_relation_claim_v1", "claim_type": "relation",
            "claim_id": "tos.claim.synthetic.exact-form-assignment", "claim_version": 1,
            "assertion_layer": "linguistic_analysis", "subject_ref": fixture.subject.id,
            "predicate": "occurrence_has_form", "object": fixture.lexical.id,
            "evidence_refs": [fixture.native.policy_ref],
            "maker": {"maker_type": "software", "agent_ref": "software:synthetic-claim-fixture"},
            "provenance_event_ref": "tos.event.synthetic-claim-fixture", "epistemic_status": "inferred",
            "review_status": "unreviewed", "visibility": "public_metadata_only",
            "qualifiers": {"statement": "Synthetic assignment only, not a linguistic conclusion.",
                "statement_language": "en", "statement_script": "Latn",
                "relation_basis": "Synthetic pair for source-closure mechanics.",
                "attestation_scope": "Only the exact synthetic occurrence and form."}}
        fixture.claim_path = "ToS/source-witnesses/relations/synthetic-claim/source-claims.jsonl"
        fixture.native.write_bytes(fixture.claim_path, (json.dumps(claim) + "\n").encode())
        fixture.claim = Record.from_payload(claim["claim_id"], 1, claim)
        fixture.config["source_records"].extend([
            {"path": lexical_path, "record_id": fixture.lexical.id, "origin_id": ORIGIN},
            {"path": fixture.claim_path, "record_id": fixture.claim.id, "origin_id": ORIGIN}])
        fixture.required = [fixture.occurrence_subject.ref, fixture.lexical.ref,
                            fixture.native_subject.ref, fixture.layer.ref]
        fixture.subject = fixture.claim
        layer, maker = claim["assertion_layer"], claim["maker"]["agent_ref"]
        if form:
            body = {"schema_version": "tos_human_form_v1", "form_id": "tos.form.synthetic.claim-grounding",
                "form_version": 1, "subject": fixture.claim.ref, "role": "hover",
                "language": "ru", "script": "Cyrl", "creator_id": "software:synthetic-form-fixture",
                "revises": None, "bindings": {"context": {"record": fixture.claim.ref, "pointer": ""}},
                "content": {"kind": "freeform", "text": "Синтетическое описание спорного разбора, не реальный вывод."}}
            path = claim_forms_path(Path(fixture.claim_path), fixture.claim.id).as_posix()
            fixture.native.write_json(path, {"schema_version": "tos_human_form_set_v1",
                "subject": fixture.claim.ref, "forms": [body], "prior_forms": []})
            fixture.subject = Record.from_payload(body["form_id"], 1, body)
            fixture.config["source_records"].append(
                {"path": path, "record_id": fixture.subject.id, "origin_id": ORIGIN})
            fixture.required.append(fixture.claim.ref)
            layer, maker = "human_projection", body["creator_id"]
        fixture.required.sort(key=lambda ref: ref["id"])
        fixture.identifier = fixture.subject.id
        fixture.config["subjects"][fixture.identifier] = {"record": fixture.subject.ref,
            "assertion_layer": layer, "risk": "low", "languages": ["en", "ru", "und"],
            "maker_id": maker, "requested_use": "research", "access_allowed": True}
        for index, competence in enumerate(fixture.config["competencies"]):
            competence["payload"].update(assertion_layers=["linguistic_analysis", "human_projection"],
                                          languages=["en", "ru", "und"])
            fixture.config["authorities"][index]["payload"].update(
                assertion_layers=["linguistic_analysis", "human_projection"], languages=["en", "ru", "und"],
                subject_prefixes=["tos.claim.", "tos.form."],
                competence_refs=[Record.from_payload(**competence).ref])
        fixture.save()
        return fixture

    def claim_request(self, fixture):
        request = fixture.request()
        request["assessments"][0].update(profile_id="interpretation", evidence=[{
            "record": ref, "stance": "supports" if ref["id"] in {
                fixture.occurrence_subject.id, fixture.layer.id} else "context",
            "locator": "Synthetic exact source dependency; these records share one origin."}
            for ref in fixture.required])
        return request

    @staticmethod
    def journal_bytes(fixture):
        journal = fixture.owner.parent / "journal"
        return {path.relative_to(journal): path.read_bytes()
                for path in journal.rglob("*") if path.is_file()}

    def test_public_claim_native_closure_requires_each_citation_for_claim_and_form(self):
        for form in (False, True):
            with self.subTest(form=form):
                fixture = self.claim_fixture(form=form)
                described = fixture.describe()["result"]["command_context"]
                self.assertEqual(described["required_sources"], fixture.required)
                self.assertEqual(described["source_read"], {"required": True, "ready": True})
                self.assertNotIn(fixture.subject.ref, described["required_sources"])
                request = self.claim_request(fixture)
                original_claim = (fixture.root / fixture.claim_path).read_bytes()
                # Each omitted reference remains in the same complete engine
                # snapshot. Supporting origin count stays sufficient without
                # either the Occurrence or layer citation, so only closure fails.
                for missing in fixture.required:
                    with self.subTest(omitted=missing["id"]):
                        incomplete = copy.deepcopy(request)
                        incomplete["assessments"][0]["evidence"] = [row for row in
                            incomplete["assessments"][0]["evidence"] if row["record"] != missing]
                        with self.assertRaises(AssessmentRejected) as rejected:
                            fixture.run(incomplete)
                        self.assertEqual(rejected.exception.invalid_assessments[0]["reasons"],
                                         ["evidence.required-source-omitted"])
                        self.assertEqual(fixture.head_paths(), [])
                first = fixture.run(request)["result"]
                self.assertTrue(first["current_admission"]["can_use"])
                history = self.journal_bytes(fixture)
                replay = fixture.run(request)["result"]
                self.assertTrue(replay["replayed"])
                self.assertEqual(replay["receipt"], first["receipt"])
                self.assertEqual(self.journal_bytes(fixture), history)
                self.assertEqual((fixture.root / fixture.claim_path).read_bytes(), original_claim)
                if form:
                    result = fixture.run(fixture.request("materialize-form"))["result"]
                    self.assertEqual(result["materialization"]["state"], "ready")
                    self.assertEqual(result["materialization"]["display_text"],
                                     fixture.subject.payload["content"]["text"])

    def test_public_claim_native_metadata_history_is_not_exact_admission(self):
        for mode in ("v2", "metadata_only", "other-unit", "same-unit-other-packet"):
            with self.subTest(mode=mode):
                fixture = self.claim_fixture(read_scope="metadata_only" if mode == "metadata_only" else "exact_public")
                if mode == "v2":
                    self.disable_native_read(fixture)
                if mode in {"other-unit", "same-unit-other-packet"}:
                    self.select_other_packet(fixture, same_unit=mode == "same-unit-other-packet")
                else:
                    (fixture.root / fixture.native.content_ref).unlink()
                result = fixture.describe()["result"]
                self.assertEqual(result["command_context"]["source_read"], {"required": True, "ready": False})
                self.assertEqual(result["command_context"]["supported_operations"], ["describe", "inspect"])
                self.assertFalse(fixture.run(fixture.request("inspect"))["result"]["current_admission"]["can_use"])
                with self.assertRaises(PermissionError):
                    fixture.run(self.claim_request(fixture))
                self.assertEqual(fixture.head_paths(), [])

        fixture = self.claim_fixture(form=True)
        first = fixture.run(self.claim_request(fixture))["result"]
        history = self.journal_bytes(fixture)
        original_claim = (fixture.root / fixture.claim_path).read_bytes()
        self.disable_native_read(fixture)
        (fixture.root / fixture.native.content_ref).unlink()
        for operation in ("describe", "inspect"):
            result = (fixture.describe() if operation == "describe"
                      else fixture.run(fixture.request("inspect")))["result"]
            self.assertFalse(result["current_admission"]["can_use"])
            self.assertEqual(result["revision"], first["revision"])
            self.assertEqual(result["batch_count"], 1)
            self.assertIn("subject.exact-source-unverified", {
                reason for row in result["current_admission"]["invalid_assessments"] for reason in row["reasons"]})
        with self.assertRaises(PermissionError):
            fixture.run(fixture.request("materialize-form"))
        self.assertEqual(self.journal_bytes(fixture), history)
        self.assertEqual((fixture.root / fixture.claim_path).read_bytes(), original_claim)

    def test_public_claim_native_closure_excludes_unrelated_reads_and_detects_exact_drift(self):
        fixture = self.claim_fixture()
        packet = copy.deepcopy(fixture.native.packet)
        packet["packet_id"] = "tos.source-text-unit-packet.sid-" + "d" * 32
        other_id = "tos.text-unit.sid-" + "e" * 32
        packet["units"][0]["unit_id"] = other_id
        packet["segmentations"][0]["ordered_unit_refs"] = [other_id]
        path = fixture.native.native_home + "/source-text-unit.unrelated.v1.json"
        fixture.native.write_json(path, packet)
        binding = {**copy.deepcopy(fixture.native.binding), "unit_id": other_id,
            "packet_id": packet["packet_id"], "packet_ref": path, "packet_sha256": fixture.native.file_digest(path)}
        selected = NativeTextBindingResolver(fixture.root).assessment_records(binding, origin_id=ORIGIN)
        other_unit = Record.from_payload(**selected["records"][0])
        self.assertFalse(other_unit.payload["content_verified"])
        other_scope = copy.deepcopy(fixture.config["subjects"][fixture.native_subject.id])
        other_scope["record"] = other_unit.ref
        fixture.config["subjects"][other_id] = other_scope
        fixture.config["native_text_units"].append(
            {"binding": binding, "origin_id": ORIGIN, "read_scope": "metadata_only"})
        other = occurrence(binding)
        other["record_id"] = "tos.occurrence.synthetic.unrelated"
        other_path = "ToS/source-witnesses/lexical-descriptions/synthetic-unrelated/occurrence.json"
        fixture.native.write_json(other_path, other)
        fixture.config["source_records"].append(
            {"path": other_path, "record_id": other["record_id"], "origin_id": ORIGIN})
        fixture.save()
        context = fixture.describe()["result"]["command_context"]
        self.assertEqual(context["required_sources"], fixture.required)
        self.assertEqual(context["source_read"], {"required": True, "ready": True})
        request = self.claim_request(fixture)
        first = fixture.run(request)["result"]
        self.assertTrue(first["current_admission"]["can_use"])
        history = self.journal_bytes(fixture)
        original_claim = (fixture.root / fixture.claim_path).read_bytes()
        other["notes"] += " Synthetic unrelated description correction."
        fixture.native.write_json(other_path, other)
        self.assertTrue(fixture.describe()["result"]["current_admission"]["can_use"])
        with self.assertRaises(JournalConflict):
            fixture.run(request)  # Global command snapshot remains stricter than Claim eligibility.
        changed = copy.deepcopy(fixture.occurrence_subject.payload)
        changed["notes"] += " Synthetic relevant description correction."
        fixture.native.write_json(fixture.occurrence_path, changed)
        current = fixture.describe()["result"]
        self.assertFalse(current["current_admission"]["can_use"])
        reasons = current["current_admission"]["invalid_assessments"][0]["reasons"]
        self.assertIn("evidence.stale-or-missing", reasons)
        self.assertIn("evidence.required-source-omitted", reasons)
        self.assertEqual(self.journal_bytes(fixture), history)
        self.assertEqual((fixture.root / fixture.claim_path).read_bytes(), original_claim)

    def test_exact_occurrence_guard_covers_evaluation_publication_and_replay(self):
        control = self.fixture()
        self.assertEqual(control.describe()["result"]["command_context"]["source_read"],
                         {"required": True, "ready": True})
        request = control.request()
        first = control.run(request)["result"]
        self.assertTrue(first["current_admission"]["can_use"])
        self.assertFalse(first["replayed"])
        self.assertEqual(len(control.head_paths()), 1)
        head = control.head_paths()[0]
        original_head = head.read_bytes()
        replay = control.run(request)["result"]
        self.assertTrue(replay["replayed"])
        self.assertEqual(replay["receipt"], first["receipt"])
        self.assertEqual(head.read_bytes(), original_head)

        for edge in ("before-evaluation", "after-blob", "before-replay"):
            with self.subTest(edge=edge):
                fixture = control if edge == "before-replay" else self.fixture()
                command = request if edge == "before-replay" else fixture.request()
                mutated = []

                def change_bound_authority():
                    target = fixture.root / fixture.native.authority_ref
                    target.write_bytes(target.read_bytes() + b"\n")
                    mutated.append(True)

                if edge == "after-blob":
                    original_write = AssessmentJournal._write_blob

                    def write_then_change(journal, *args, **kwargs):
                        result = original_write(journal, *args, **kwargs)
                        change_bound_authority()
                        return result

                    with patch.object(AssessmentJournal, "_write_blob", write_then_change):
                        with self.assertRaises((SourceProfileError, NativeTextBindingError)):
                            fixture.run(command)
                    # Retaining an unpublished immutable blob is permitted;
                    # making it committed assessment history is not.
                    self.assertEqual(len(list((fixture.owner.parent / "journal").rglob("*.json"))), 1)
                else:
                    original_lock = AssessmentJournal._locked

                    @contextmanager
                    def lock_then_change(journal, *args, **kwargs):
                        with original_lock(journal, *args, **kwargs):
                            change_bound_authority()
                            yield

                    with patch.object(AssessmentJournal, "_locked", lock_then_change), \
                         patch.object(AssessmentEngine, "evaluate",
                                      side_effect=AssertionError("stale native input reached evaluation")) as evaluate:
                        with self.assertRaises((SourceProfileError, NativeTextBindingError)):
                            fixture.run(command)
                        evaluate.assert_not_called()
                self.assertEqual(mutated, [True])
                if edge == "before-replay":
                    self.assertEqual(fixture.head_paths(), [head])
                    self.assertEqual(head.read_bytes(), original_head)
                else:
                    self.assertEqual(fixture.head_paths(), [])

    def test_missing_metadata_only_and_other_native_reads_cannot_append(self):
        for mode in ("v2", "metadata_only", "other-unit", "same-unit-other-packet", "inline-v1", "inline-bool-version-v3"):
            with self.subTest(mode=mode):
                fixture = self.fixture(read_scope="metadata_only" if mode == "metadata_only" else "exact_public")
                if mode in {"v2", "inline-v1"}:
                    self.disable_native_read(fixture)
                if mode == "inline-v1":
                    fixture.config["schema_version"] = "tos_local_assessment_owner_v1"
                    fixture.config.pop("source_root")
                    fixture.config.pop("source_records")
                    fixture.config["records"].append({"id": fixture.subject.id,
                        "version": fixture.subject.version, "payload": fixture.subject.payload,
                        "origin_id": ORIGIN})
                    fixture.save()
                if mode == "inline-bool-version-v3":
                    body = copy.deepcopy(fixture.subject.payload)
                    body["native_text_binding"]["unit_version"] = True
                    fixture.subject = Record.from_payload(fixture.subject.id, fixture.subject.version, body)
                    fixture.scope["record"] = fixture.subject.ref
                    fixture.config["source_records"] = []
                    fixture.config["records"].append({"id": fixture.subject.id,
                        "version": fixture.subject.version, "payload": body, "origin_id": ORIGIN})
                    fixture.save()
                if mode in {"other-unit", "same-unit-other-packet"}:
                    self.select_other_packet(fixture, same_unit=mode == "same-unit-other-packet")
                elif mode != "inline-bool-version-v3":
                    # Neither discovery nor inspection may secretly upgrade
                    # a metadata-only selection by opening source text.
                    (fixture.root / fixture.native.content_ref).unlink()
                described = fixture.describe()["result"]
                context = described["command_context"]
                self.assertEqual(context["source_read"], {"required": True, "ready": False})
                self.assertEqual(context["supported_operations"], ["describe", "inspect"])
                self.assertFalse(described["current_admission"]["can_use"])
                inspected = fixture.run(fixture.request("inspect"))["result"]
                self.assertFalse(inspected["current_admission"]["can_use"])
                with self.assertRaises(PermissionError):
                    fixture.run(fixture.request())
                self.assertEqual(fixture.head_paths(), [])

    def test_v2_inspection_preserves_history_without_reusing_exact_admission(self):
        fixture = self.fixture()
        request = fixture.request()
        committed = fixture.run(request)["result"]
        self.assertTrue(committed["current_admission"]["can_use"])
        journal = fixture.owner.parent / "journal"
        before = {path.relative_to(journal): path.read_bytes()
                  for path in journal.rglob("*") if path.is_file()}
        self.disable_native_read(fixture)
        (fixture.root / fixture.native.content_ref).unlink()
        for operation in ("describe", "inspect"):
            result = (fixture.describe() if operation == "describe"
                      else fixture.run(fixture.request("inspect")))["result"]
            self.assertFalse(result["current_admission"]["can_use"])
            self.assertEqual(result["revision"], committed["revision"])
            self.assertEqual(result["batch_count"], 1)
            reasons = {reason for row in result["current_admission"]["invalid_assessments"]
                       for reason in row["reasons"]}
            self.assertIn("subject.exact-source-unverified", reasons)
        with self.assertRaises(PermissionError):
            fixture.run(fixture.request())
        self.assertEqual({path.relative_to(journal): path.read_bytes()
                          for path in journal.rglob("*") if path.is_file()}, before)

    def test_source_bound_freeform_cannot_materialize_after_exact_read_is_removed(self):
        fixture = self.fixture()
        subject = fixture.subject
        form = {"schema_version": "tos_human_form_v1", "form_id": "tos.form.synthetic-occurrence-reading",
            "form_version": 1, "subject": subject.ref, "role": "hover",
            "language": "ru", "script": "Cyrl", "creator_id": "fixture-form-writer", "revises": None,
            "bindings": {"context": {"record": subject.ref, "pointer": ""}},
            "content": {"kind": "freeform", "text": "Синтетическое описание употребления; не реальная оценка."}}
        form_ref = fixture.config["source_records"][0]["path"].replace("occurrence.json", "occurrence.human-forms.json")
        fixture.native.write_json(form_ref, {"schema_version": "tos_human_form_set_v1",
            "subject": subject.ref, "forms": [form], "prior_forms": []})
        fixture.identifier = form["form_id"]
        fixture.subject = Record.from_payload(fixture.identifier, 1, form)
        fixture.config["source_records"].append({"path": form_ref,
            "record_id": fixture.identifier, "origin_id": ORIGIN})
        fixture.config["subjects"][fixture.identifier] = {"record": fixture.subject.ref,
            "assertion_layer": "human_projection", "risk": "low", "languages": ["ru", "und"],
            "maker_id": form["creator_id"], "requested_use": "research", "access_allowed": True}
        for index, competence in enumerate(fixture.config["competencies"]):
            competence["payload"]["assertion_layers"].append("human_projection")
            authority = fixture.config["authorities"][index]["payload"]
            authority["assertion_layers"].append("human_projection")
            authority["subject_prefixes"] = ["tos.form."]
            authority["competence_refs"] = [Record.from_payload(**competence).ref]
        fixture.save()
        request = fixture.request()
        request["assessments"][0]["profile_id"] = "interpretation"
        committed = fixture.run(request)["result"]
        self.assertTrue(committed["current_admission"]["can_use"])
        ready = fixture.run(fixture.request("materialize-form"))["result"]
        self.assertEqual(ready["materialization"]["state"], "ready")
        self.assertEqual(ready["materialization"]["display_text"], form["content"]["text"])
        journal = fixture.owner.parent / "journal"
        before = {path.relative_to(journal): path.read_bytes()
                  for path in journal.rglob("*") if path.is_file()}
        self.disable_native_read(fixture)
        (fixture.root / fixture.native.content_ref).unlink()
        described = fixture.describe()["result"]
        self.assertFalse(described["current_admission"]["can_use"])
        self.assertNotIn("materialize-form", described["command_context"]["supported_operations"])
        # Hiding the operation from discovery is not sufficient: a caller
        # can still submit this supported command shape directly.
        with self.assertRaises(PermissionError):
            fixture.run(fixture.request("materialize-form"))
        self.assertEqual({path.relative_to(journal): path.read_bytes()
                          for path in journal.rglob("*") if path.is_file()}, before)

    def test_claim_grounding_keeps_native_text_and_identity_snapshots(self):
        import source_commands as commands
        from build_source_witness_catalog import CatalogBuildError
        from source_claim_commands import _ground_claims

        fixture = self.fixture()
        lexical = copy.deepcopy(fixture.subject.payload)
        lexical.pop("native_text_binding")
        lexical.update(schema_version="tos_lexical_description_record_v1", record_type="lexeme",
                       record_id="tos.lexeme.synthetic.group")
        lexical["semantic_content"] = {"lexical_account": "Synthetic grouping only.",
            "grammatical_account": "Unknown synthetic analysis.", "language": "en", "script": "Latn"}
        fixture.native.write_json("ToS/source-witnesses/lexical-descriptions/synthetic-group/lexeme.json", lexical)
        claim = {"schema_version": "tos_semantic_relation_claim_v1", "claim_type": "relation",
            "claim_id": "tos.claim.synthetic.occurrence-group", "claim_version": 1,
            "assertion_layer": "linguistic_analysis", "subject_ref": fixture.identifier,
            "predicate": "occurrence_of_lexeme", "object": lexical["record_id"],
            "evidence_refs": [fixture.native.policy_ref],
            "maker": {"maker_type": "software", "agent_ref": "software:synthetic-claim-fixture"},
            "provenance_event_ref": "tos.event.synthetic-native-claim-create", "epistemic_status": "inferred",
            "review_status": "unreviewed", "visibility": "public_metadata_only",
            "qualifiers": {"statement": "Synthetic grouping test only.",
                "statement_language": "en", "statement_script": "Latn",
                "relation_basis": "Synthetic source relationship, not linguistic evidence.",
                "attestation_scope": "Synthetic packet and use only."}}
        config = {"schema_version": commands.CLAIM_CONFIG, "uid": fixture.config["uid"],
            "principal_id": claim["maker"]["agent_ref"], "maker_type": "software",
            "source_root": str(fixture.root), "source_path": "ToS/source-witnesses/relations/native-closure/source-claims.jsonl",
            "authority_ref": "synthetic:source-creation-only", "expires_at": "2099-01-01T00:00:00Z",
            "provenance_event_id": claim["provenance_event_ref"], "allowed_operations": ["claims.create"],
            "allowed_claim_ids": [claim["claim_id"]], "allowed_subject_refs": [fixture.identifier],
            "allowed_object_refs": [lexical["record_id"]], "allowed_predicates": [claim["predicate"]],
            "allowed_evidence_refs": claim["evidence_refs"]}
        (fixture.root / "ToS/source-witnesses/relations").mkdir()
        fixture.native.write_json("claim-owner.json", config)
        owner = fixture.root / "claim-owner.json"
        proposal = {"schema_version": "tos_local_source_command_v1", "operation": "prepare-create", "claims": [claim]}
        first = commands.run_local_command(owner, proposal)
        before = _ground_claims(config, [claim], initial=False)
        profiles = SourceRecordProfiles(fixture.root)
        profiles.validate("occurrence", fixture.subject.payload)
        native_before = profiles.native_text_snapshot()
        fixture.native.manifest["manifest_version"] += 1
        fixture.native.write_json(fixture.native.manifest_ref, fixture.native.manifest)
        profiles = SourceRecordProfiles(fixture.root)
        profiles.validate("occurrence", fixture.subject.payload)
        self.assertNotEqual(profiles.native_text_snapshot(), native_before)
        after = _ground_claims(config, [claim], initial=False)
        self.assertEqual(before[0], after[0])
        self.assertEqual(before[2], after[2])
        self.assertNotEqual(before[1], after[1])
        request = {**proposal, "operation": "claims.create", "command_id": "synthetic:claim-native-closure",
            "expected_configuration": first["owner_configuration"], "expected_revision": None,
            "expected_dependencies": first["expected_dependencies"], "expected_inputs": first["source_bindings"]}
        with self.assertRaises(commands.JournalConflict):
            commands.run_local_command(owner, request)
        target = fixture.root / config["source_path"]
        self.assertFalse(target.parent.exists())

        # A reserved native identity is still owned by the native packet even
        # when its synthetic body is withheld from public projection. Changes
        # after the grammar is already loaded must affect the opaque snapshot.
        packet = fixture.native.skeleton("semantic-annotation-v2-abc/variant-b-competing-sign-proposals.json")
        packet["content_posture"] = "source_bound"
        packet["rights_and_visibility"].update(private_source_used=True, publication_authorized=False,
            record_visibility="local_only", source_content_visibility="local_only")
        packet_ref = fixture.native.native_home + "/semantic-annotation.synthetic-identity.json"
        fixture.native.write_json(packet_ref, packet)
        reserved_before = _ground_claims(config, [claim], initial=False)
        packet["entities"][-1]["display_labels"][0]["value"] = "Changed synthetic native label only."
        fixture.native.write_json(packet_ref, packet)
        reserved_after = _ground_claims(config, [claim], initial=False)
        self.assertEqual(reserved_before[0], reserved_after[0])
        self.assertEqual(reserved_before[2], reserved_after[2])
        self.assertNotEqual(reserved_before[1], reserved_after[1])
        self.assertNotIn(packet_ref, json.dumps(reserved_after[2]))
        collision = copy.deepcopy(packet["entities"][0])
        collision["entity_id"] = fixture.identifier
        packet["entities"].append(collision)
        fixture.native.write_json(packet_ref, packet)
        self.assertIn(fixture.identifier, SourceRecordProfiles(fixture.root).native_semantic_identities())
        with self.assertRaises(CatalogBuildError):
            _ground_claims(config, [claim], initial=False)
        with self.assertRaises(CatalogBuildError):
            commands.run_local_command(owner, proposal)
        self.assertFalse(target.parent.exists())

        packet["entities"].pop()
        fixture.native.write_json(packet_ref, packet)
        fresh = commands.run_local_command(owner, proposal)
        request.update(expected_configuration=fresh["owner_configuration"],
            expected_dependencies=fresh["expected_dependencies"], expected_inputs=fresh["source_bindings"])
        created = commands.run_local_command(owner, request)
        self.assertFalse(created["grants_admission"])
        self.assertEqual(json.loads(target.read_bytes().splitlines()[0]), claim)


if __name__ == "__main__":
    unittest.main()
