"""Synthetic native TextUnit -> owner-local assessment boundary checks.

No historical source bytes, rights, competence or substantive judgment are
asserted here. The temporary native fixture has no original Item payload;
its only text is an explicitly synthetic CRLF/NFD construction. A journal
receipt must not rewrite native review or segmentation status.
"""
from __future__ import annotations

import copy
from contextlib import contextmanager
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
sys.path.insert(0, str(REPO_ROOT / "tests"))
sys.path.insert(0, str(REPO_ROOT / "mechanics/growth-cycle/tests"))

import test_knowledge_assessment as policy_fixtures
import assessment_journal
from test_native_text_binding import NativeTextBindingFixture, digest
from native_text_binding import NativeTextBindingError, NativeTextBindingResolver
from knowledge_assessment import Record
from assessment_journal import AssessmentJournal, AssessmentRejected, JournalConflict


SUBJECT_SCHEMA = "native-text-unit-assessment-subject.schema.json"
ORIGIN = "synthetic-one-source-origin"


class NativeAssessmentFixture:
    """Use the normal protected local command fixture, with real adapter APIs."""

    def __init__(self, test: unittest.TestCase, *, read_scope="exact_owner_local"):
        self.policy = policy_fixtures.AssessmentPolicyTests(methodName="runTest")
        self.policy.setUp()
        test.addCleanup(self.policy.doCleanups)
        self.owner, self.config, _ = self.policy.local_command_fixture()
        self.root = self.owner.parent / "source-copy"
        self.native = NativeTextBindingFixture(self.root)
        self.native.write_bytes("ToS/contracts/" + SUBJECT_SCHEMA,
            (REPO_ROOT / "ToS/contracts" / SUBJECT_SCHEMA).read_bytes())
        self.identifier = self.native.binding["unit_id"]
        self.config.update(schema_version="tos_local_assessment_owner_v3",
            source_root=str(self.root), source_records=[], native_text_units=[{
                "binding": copy.deepcopy(self.native.binding), "origin_id": ORIGIN,
                "read_scope": read_scope,
            }])
        self.config["records"] = [row for row in self.config["records"]
                                  if row["id"] != self.policy.subject.id]
        for index, row in enumerate(self.config["competencies"]):
            row["payload"]["assertion_layers"] = ["textual_observation", "linguistic_analysis"]
            row["payload"]["languages"] = ["ru", "und"]
            competence = Record.from_payload(**row)
            authority = self.config["authorities"][index]["payload"]
            authority.update(assertion_layers=["textual_observation", "linguistic_analysis"],
                languages=["ru", "und"], subject_prefixes=["tos.text-unit."],
                competence_refs=[competence.ref])
        self.config["subjects"] = {self.identifier: {
            "record": {}, "assertion_layer": "textual_observation", "risk": "low",
            "languages": ["und"],
            "maker_id": self.native.packet["segmentations"][0]["maker"]["agent_ref"],
            "requested_use": "research", "access_allowed": True,
        }}
        self.rebind(read_scope)

    @property
    def scope(self):
        return self.config["subjects"][self.identifier]

    def save(self):
        self.owner.write_text(json.dumps(self.config, ensure_ascii=False), encoding="utf-8")
        self.owner.chmod(0o600)

    def rebind(self, read_scope=None):
        binding = self.config["native_text_units"][0]
        if read_scope is not None:
            binding["read_scope"] = read_scope
        binding["binding"] = copy.deepcopy(self.native.binding)
        result = NativeTextBindingResolver(self.root).assessment_records(
            binding["binding"], origin_id=ORIGIN,
            verify_content=binding["read_scope"] != "metadata_only",
            allow_private_content=binding["read_scope"] == "exact_owner_local")
        self.adapted = result
        self.subject, self.layer = [Record.from_payload(**row) for row in result["records"]]
        self.scope["record"] = self.subject.ref
        self.save()

    def run(self, request):
        return self.policy.run_local(self.owner, request)

    def describe(self):
        return self.run({"schema_version": "tos_local_assessment_command_v1",
            "operation": "describe", "subject_id": self.identifier})

    def request(self, operation="append", *, command_id="synthetic-native-one"):
        described = self.describe()
        request = {"schema_version": "tos_local_assessment_command_v1",
            "operation": operation, "subject_id": self.identifier,
            "expected_subject": self.subject.ref,
            "expected_snapshot": described["owner_snapshot"]}
        if operation == "append":
            review = copy.deepcopy(self.policy.review().assessment)
            review.update(subject=self.subject.ref,
                authority=Record.from_payload(**self.config["authorities"][0]).ref,
                competence=Record.from_payload(**self.config["competencies"][0]).ref,
                evidence=[{"record": self.layer.ref, "stance": "supports",
                           "locator": "Synthetic selected layer; no quoted text or private path."}])
            request.update(command_id=command_id, expected_revision=None, assessments=[review])
        return request

    def head_paths(self):
        return list((self.owner.parent / "journal").rglob("head"))


class NativeTextAssessmentTests(unittest.TestCase):
    def fixture(self, **kwargs):
        return NativeAssessmentFixture(self, **kwargs)

    def assert_private_safe(self, fixture, value):
        rendered = json.dumps(value, ensure_ascii=False)
        native = fixture.native
        for forbidden in (native.text[3:8], native.content_ref, native.packet_ref,
                          native.layer_ref, native.original_ref, digest(native.content),
                          native.binding["packet_sha256"],
                          native.binding["text_layer"]["record_sha256"],
                          native.file_digest(native.rights_ref),
                          "exact_sha256", "ordered_anchor_refs", "source_record_refs"):
            self.assertNotIn(forbidden, rendered)

    def test_adapter_preserves_native_payload_and_one_origin_without_original_bytes(self):
        fixture = self.fixture()
        payload = fixture.subject.payload
        self.assertEqual(payload["schema_version"], "tos_native_text_unit_assessment_subject_v1")
        self.assertEqual(payload["packet"], fixture.native.packet)
        self.assertEqual(payload["native_binding"], fixture.native.binding)
        self.assertEqual(payload["text_layer"], fixture.layer.ref)
        self.assertEqual(fixture.layer.payload, fixture.native.layer)
        self.assertEqual(fixture.subject.id, fixture.native.binding["unit_id"])
        self.assertEqual(fixture.subject.version, fixture.native.binding["unit_version"])
        self.assertTrue(payload["content_verified"])
        self.assertEqual({row["origin_id"] for row in fixture.adapted["records"]}, {ORIGIN})
        self.assertNotEqual(fixture.subject.ref["digest"], "sha256:" + fixture.native.binding["packet_sha256"])
        self.assertFalse((fixture.root / fixture.native.original_ref).exists())

    def test_exact_private_append_is_agent_assessment_not_native_status_rewrite(self):
        fixture = self.fixture()
        native_before = {ref: (fixture.root / ref).read_bytes() for ref in
            (fixture.native.packet_ref, fixture.native.layer_ref, fixture.native.rights_ref,
             fixture.native.content_ref)}
        request = fixture.request()
        result = fixture.run(request)
        self.assertTrue(result["result"]["current_admission"]["can_use"])
        event = result["result"]["receipt"]["events"][0]["assessment"]
        self.assertEqual(event["reviewer"]["kind"], "agent")
        for ref, before in native_before.items():
            self.assertEqual((fixture.root / ref).read_bytes(), before)
        self.assertEqual(fixture.native.packet["segmentations"][0]["status"], "proposed")
        self.assertEqual(fixture.native.packet["units"][0]["boundary_posture"], "method_proposed")
        self.assertEqual(fixture.native.packet["reviews"], [])
        self.assertEqual(fixture.native.layer["admission"]["review_status"], "unreviewed")
        self.assertFalse(fixture.adapted["summary"]["assessment_applied"])
        self.assertFalse(fixture.adapted["summary"]["public_content_available"])
        self.assertFalse((fixture.root / fixture.native.original_ref).exists())
        self.assert_private_safe(fixture, result)
        described = fixture.describe()
        self.assert_private_safe(fixture, described)
        context = described["result"]["command_context"]
        self.assertTrue(context["native_contracts"])
        self.assertTrue(all(row["path"].startswith("ToS/contracts/")
                            for row in context["native_contracts"]))
        self.assertEqual(context["source_records"], [])
        self.assertEqual(context["source_contracts"], [])
        replay = fixture.run(request)["result"]
        self.assertTrue(replay["replayed"])
        self.assertEqual(replay["revision"], result["result"]["revision"])

    def test_current_revocation_does_not_erase_commit_time_receipt(self):
        fixture = self.fixture()
        request = fixture.request()
        first = fixture.run(request)["result"]
        authority = fixture.config["authorities"][0]
        authority["version"] += 1
        authority["payload"]["authority_version"] += 1
        authority["payload"]["state"] = "revoked"
        fixture.save()
        request["expected_snapshot"] = fixture.describe()["owner_snapshot"]
        replay = fixture.run(request)["result"]
        self.assertTrue(replay["replayed"])
        self.assertEqual(replay["revision"], first["revision"])
        self.assertTrue(replay["receipt"]["admission_at_commit"]["can_use"])
        self.assertFalse(replay["current_admission"]["can_use"])

    def test_metadata_only_describe_and_inspect_never_open_content_and_cannot_append(self):
        fixture = self.fixture(read_scope="metadata_only")
        original_read = assessment_journal._owned_path

        def bounded_open(path, *args, **kwargs):
            self.assertNotEqual(path, fixture.root / fixture.native.content_ref)
            self.assertNotIn("payload", path.parts)
            return original_read(path, *args, **kwargs)

        with patch.object(assessment_journal, "_owned_path", bounded_open):
            described = fixture.describe()
            inspected = fixture.run(fixture.request("inspect"))
            request = fixture.request()
            with self.assertRaises((PermissionError, ValueError)):
                fixture.run(request)
        self.assertFalse(fixture.subject.payload["content_verified"])
        self.assert_private_safe(fixture, described)
        self.assert_private_safe(fixture, inspected)
        self.assertEqual(fixture.head_paths(), [])

    def test_exact_mode_downgrade_changes_target_digest_and_cannot_replay_admission(self):
        fixture = self.fixture()
        request = fixture.request()
        fixture.run(request)
        original = fixture.subject.ref
        fixture.rebind("metadata_only")
        self.assertEqual(fixture.subject.id, original["id"])
        self.assertEqual(fixture.subject.version, original["version"])
        self.assertNotEqual(fixture.subject.ref["digest"], original["digest"])
        inspected = fixture.run(fixture.request("inspect"))["result"]
        self.assertFalse(inspected["current_admission"]["can_use"])
        request["expected_snapshot"] = fixture.describe()["owner_snapshot"]
        with self.assertRaises((JournalConflict, PermissionError, ValueError)):
            fixture.run(request)
        fresh = fixture.request(command_id="synthetic-metadata-no-append")
        fresh["expected_revision"] = inspected["revision"]
        with self.assertRaises((PermissionError, ValueError)):
            fixture.run(fresh)

    def test_access_denial_and_invalid_read_scopes_fail_before_native_io(self):
        fixture = self.fixture()
        baseline = copy.deepcopy(fixture.config)
        for case in ("false", "missing", "truthy", "scope-unknown", "scope-bool", "scope-list"):
            with self.subTest(case=case):
                fixture.config = copy.deepcopy(baseline)
                if case == "false":
                    fixture.scope["access_allowed"] = False
                elif case == "missing":
                    del fixture.scope["access_allowed"]
                elif case == "truthy":
                    fixture.scope["access_allowed"] = 1
                else:
                    fixture.config["native_text_units"][0]["read_scope"] = {
                        "scope-unknown": "false", "scope-bool": True, "scope-list": ["exact_owner_local"]}[case]
                fixture.save()
                with patch.object(NativeTextBindingResolver, "assessment_records",
                                  side_effect=AssertionError("native I/O began before scope preflight")) as reader:
                    with self.assertRaises((PermissionError, ValueError)):
                        fixture.describe()
                    reader.assert_not_called()
        self.assertEqual(fixture.head_paths(), [])

    def test_exact_public_cannot_open_private_content_from_request_or_configuration(self):
        fixture = self.fixture()
        request = fixture.request()
        for field in ("allow_private_content", "native_text_units", "read_scope", "access_allowed"):
            with self.subTest(field=field), self.assertRaises((PermissionError, ValueError)):
                fixture.run({**request, field: True})
        fixture.config["native_text_units"][0]["read_scope"] = "exact_public"
        fixture.save()
        with self.assertRaises((NativeTextBindingError, PermissionError, ValueError)):
            fixture.describe()
        self.assertEqual(fixture.head_paths(), [])

    def test_exact_public_uses_same_journal_with_separately_public_synthetic_inputs(self):
        fixture = self.fixture()
        fixture.native.make_public()
        fixture.rebind("exact_public")
        self.assertTrue(fixture.adapted["summary"]["public_content_available"])
        result = fixture.run(fixture.request())["result"]
        self.assertTrue(result["current_admission"]["can_use"])
        self.assertFalse((fixture.root / fixture.native.original_ref).exists())

    def test_native_scope_cannot_relabel_maker_language_or_assertion_layer(self):
        fixture = self.fixture()
        original = copy.deepcopy(fixture.scope)
        for field, value in (("maker_id", "someone-else"), ("languages", ["ru"]),
                             ("assertion_layer", "semantic_interpretation"),
                             ("assertion_layer", "bibliographic_assertion")):
            with self.subTest(field=field, value=value):
                fixture.config["subjects"][fixture.identifier] = {**original, field: value}
                fixture.save()
                with self.assertRaises((PermissionError, ValueError)):
                    fixture.describe()
        self.assertEqual(fixture.head_paths(), [])

    def test_schema_first_read_as_support_keeps_its_schema_role_and_exact_fixity(self):
        fixture = self.fixture()
        ref = "ToS/contracts/" + SUBJECT_SCHEMA
        fixture.native.layer["editorial_policy"].update(
            policy_ref=ref, policy_sha256=fixture.native.file_digest(ref))
        fixture.native.refresh()
        fixture.rebind()
        described = fixture.describe()
        contracts = described["result"]["command_context"]["native_contracts"]
        self.assertIn({"path": ref, "digest": "sha256:" + fixture.native.file_digest(ref)}, contracts)
        resolver = NativeTextBindingResolver(fixture.root)
        resolver.assessment_records(fixture.native.binding, origin_id=ORIGIN,
                                    verify_content=True, allow_private_content=True)
        original_snapshot = resolver.snapshot()
        self.assertRegex(original_snapshot, r"^sha256:[a-f0-9]{64}$")
        path = fixture.root / ref
        path.write_bytes(path.read_bytes() + b" ")
        with self.assertRaises(NativeTextBindingError):
            resolver.snapshot()
        with self.assertRaises((NativeTextBindingError, JournalConflict)):
            fixture.describe()
        self.assertEqual(fixture.head_paths(), [])

    def test_native_subject_cannot_be_its_own_supporting_source(self):
        fixture = self.fixture()
        request = fixture.request()
        request["assessments"][0]["evidence"][0]["record"] = fixture.subject.ref
        with self.assertRaises(AssessmentRejected) as rejected:
            fixture.run(request)
        self.assertIn("evidence.circular-support", rejected.exception.invalid_assessments[0]["reasons"])
        self.assertEqual(fixture.head_paths(), [])

    def test_native_companion_layer_needs_its_own_explicit_subject_adapter(self):
        fixture = self.fixture()
        fixture.config["subjects"][fixture.layer.id] = {
            **fixture.scope, "record": fixture.layer.ref}
        fixture.save()
        with self.assertRaises(PermissionError):
            fixture.run({"schema_version": "tos_local_assessment_command_v1",
                "operation": "describe", "subject_id": fixture.layer.id})
        self.assertEqual(fixture.head_paths(), [])

    def test_metadata_only_layer_cannot_support_append_on_another_subject(self):
        fixture = self.fixture(read_scope="metadata_only")
        other = fixture.policy.subject
        fixture.config["records"].append({"id": other.id, "version": other.version,
            "payload": other.payload, "origin_id": other.origin_id})
        fixture.config["subjects"][other.id] = {
            "record": other.ref, "assertion_layer": "bibliographic_assertion", "risk": "low",
            "languages": ["de"], "maker_id": "extractor", "requested_use": "research",
            "access_allowed": True}
        fixture.save()
        described = fixture.run({"schema_version": "tos_local_assessment_command_v1",
            "operation": "describe", "subject_id": other.id})
        review = copy.deepcopy(fixture.policy.review().assessment)
        review["evidence"][0]["record"] = fixture.layer.ref
        request = {"schema_version": "tos_local_assessment_command_v1", "operation": "append",
            "subject_id": other.id, "expected_subject": other.ref,
            "expected_snapshot": described["owner_snapshot"], "command_id": "synthetic-evidence-no-append",
            "expected_revision": None, "assessments": [review]}
        with self.assertRaises(PermissionError):
            fixture.run(request)
        self.assertEqual(fixture.head_paths(), [])

    def test_inline_and_source_bindings_cannot_shadow_native_unit_or_layer(self):
        fixture = self.fixture()
        baseline = copy.deepcopy(fixture.config)
        for identity in (fixture.subject.id, fixture.layer.id):
            for carrier in ("inline", "source"):
                with self.subTest(identity=identity, carrier=carrier):
                    fixture.config = copy.deepcopy(baseline)
                    if carrier == "inline":
                        fixture.config["records"].append({"id": identity, "version": 1,
                            "payload": {"synthetic": True}, "origin_id": ORIGIN})
                    else:
                        fixture.config["source_records"] = [{"path": fixture.native.refs["work"],
                            "record_id": identity, "origin_id": ORIGIN}]
                    fixture.save()
                    with self.assertRaises((PermissionError, ValueError)):
                        fixture.describe()
        self.assertEqual(fixture.head_paths(), [])

    def test_duplicate_native_unit_and_missing_pinned_subject_are_rejected(self):
        fixture = self.fixture()
        baseline = copy.deepcopy(fixture.config)
        for case in ("duplicate", "missing-subject", "missing-record", "null-record", "wrong-record"):
            with self.subTest(case=case):
                fixture.config = copy.deepcopy(baseline)
                if case == "duplicate":
                    fixture.config["native_text_units"].append(copy.deepcopy(fixture.config["native_text_units"][0]))
                elif case == "missing-subject":
                    fixture.config["subjects"] = {}
                elif case == "missing-record":
                    del fixture.scope["record"]
                elif case == "null-record":
                    fixture.scope["record"] = None
                else:
                    fixture.scope["record"] = {**fixture.subject.ref, "version": 2}
                fixture.save()
                with self.assertRaises((PermissionError, ValueError)):
                    fixture.describe()
        self.assertEqual(fixture.head_paths(), [])

    def test_changes_after_blob_write_cannot_publish_a_head(self):
        for target in ("packet", "layer", "rights", "content", "schema", "owner"):
            with self.subTest(target=target):
                fixture = self.fixture()
                request = fixture.request()
                paths = {"packet": fixture.root / fixture.native.packet_ref,
                    "layer": fixture.root / fixture.native.layer_ref,
                    "rights": fixture.root / fixture.native.rights_ref,
                    "content": fixture.root / fixture.native.content_ref,
                    "schema": fixture.root / "ToS/contracts" / SUBJECT_SCHEMA,
                    "owner": fixture.owner}
                selected = paths[target]
                original_write = AssessmentJournal._write_blob
                writes = []

                def write_then_change(journal, home, revision, payload):
                    original_write(journal, home, revision, payload)
                    writes.append(revision)
                    selected.write_bytes(selected.read_bytes() + b" ")

                with patch.object(AssessmentJournal, "_write_blob", write_then_change):
                    with self.assertRaises((JournalConflict, NativeTextBindingError, PermissionError, ValueError)):
                        fixture.run(request)
                self.assertEqual(len(writes), 1, "test must reach the commit-edge guard")
                self.assertEqual(fixture.head_paths(), [])
                # A durable, unreachable blob is acceptable; visible partial history is not.
                self.assertEqual(len(list((fixture.owner.parent / "journal").rglob("*.json"))), 1)

    def test_changed_inputs_under_lock_are_rejected_before_replay_return(self):
        fixture = self.fixture()
        request = fixture.request()
        first = fixture.run(request)["result"]
        head = fixture.head_paths()[0]
        original_head = head.read_bytes()
        original_lock = AssessmentJournal._locked

        @contextmanager
        def lock_then_change(journal, home):
            with original_lock(journal, home):
                fixture.owner.write_bytes(fixture.owner.read_bytes() + b" ")
                yield

        with patch.object(AssessmentJournal, "_locked", lock_then_change):
            with self.assertRaises((JournalConflict, NativeTextBindingError, PermissionError, ValueError)):
                fixture.run(request)
        self.assertEqual(head.read_bytes(), original_head)
        self.assertEqual(head.read_text().strip(), first["revision"])

    def test_owner_v1_and_v2_inline_commands_remain_compatible(self):
        for version in (1, 2):
            with self.subTest(version=version):
                case = policy_fixtures.AssessmentPolicyTests(methodName="runTest")
                case.setUp()
                self.addCleanup(case.doCleanups)
                owner, config, request = case.local_command_fixture()
                if version == 2:
                    source_root = owner.parent / "empty-source"
                    source_root.mkdir(mode=0o700)
                    config.update(schema_version="tos_local_assessment_owner_v2",
                                  source_root=str(source_root), source_records=[])
                    owner.write_text(json.dumps(config), encoding="utf-8")
                    described = case.run_local(owner, {"schema_version": "tos_local_assessment_command_v1",
                        "operation": "describe", "subject_id": case.subject.id})
                    request["expected_snapshot"] = described["owner_snapshot"]
                result = case.run_local(owner, request)["result"]
                self.assertTrue(result["current_admission"]["can_use"])
                self.assertTrue(case.run_local(owner, request)["result"]["replayed"])


if __name__ == "__main__":
    unittest.main()
