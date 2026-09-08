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
from test_native_text_binding import digest
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

    def motif_fixture(self, *, form=False):
        """Three distinct native spans; all grants and proposed judgments are synthetic."""
        f = self.claim_fixture(form=form)
        native = f.native
        template_anchor, template_unit = copy.deepcopy(native.packet['anchors'][1]), copy.deepcopy(native.packet['units'][0])
        anchors, units = [], []
        for number, (start, end) in enumerate(((3, 4), (4, 6), (6, 8))):
            anchor = copy.deepcopy(template_anchor)
            anchor.update(anchor_ref=f'tos.anchor.synthetic.motif-assessment-{number}', ordinal=number + 2,
                          exact_sha256=digest(native.text[start:end].encode('utf-8')))
            anchor['selector'].update(start=start, end=end)
            unit = copy.deepcopy(template_unit)
            unit.update(unit_id=template_unit['unit_id'] if number == 0 else 'tos.text-unit.sid-' + str(number) * 32,
                        ordered_anchor_refs=[anchor['anchor_ref']])
            anchors.append(anchor); units.append(unit)
        gap = copy.deepcopy(native.packet['anchors'][-1]); gap['ordinal'] = 5
        native.packet['anchors'] = [native.packet['anchors'][0], *anchors, gap]
        native.packet['units'] = units
        native.packet['segmentations'][0]['ordered_unit_refs'] = [unit['unit_id'] for unit in units]
        native.refresh()
        f.config['source_records'] = [row for row in f.config['source_records'] if row['record_id'] != f.lexical.id]
        f.config['native_text_units'] = []
        required, f.members, f.member_paths = {}, [], []
        for number, unit in enumerate(units):
            binding = {**copy.deepcopy(native.binding), 'unit_id': unit['unit_id'],
                       'ordered_anchor_refs': unit['ordered_anchor_refs']}
            record = occurrence(binding)
            record['record_id'] = f.occurrence_subject.id if number == 0 else f'tos.occurrence.synthetic.motif-assessment-{number}'
            path = f.occurrence_path if number == 0 else f'ToS/source-witnesses/lexical-descriptions/motif-assessment-{number}/occurrence.json'
            native.write_json(path, record)
            if number:
                f.config['source_records'].append({'path': path, 'record_id': record['record_id'], 'origin_id': ORIGIN})
            source = Record.from_payload(record['record_id'], 1, record)
            f.members.append(source); f.member_paths.append(path)
            required[source.id] = source.ref
            f.config['native_text_units'].append({'binding': binding, 'origin_id': ORIGIN, 'read_scope': 'exact_public'})
            adapted = NativeTextBindingResolver(f.root).assessment_records(binding, origin_id=ORIGIN, verify_content=True)
            for row in adapted['records']:
                dependency = Record.from_payload(**row)
                required[dependency.id] = dependency.ref
                if dependency.id == unit['unit_id']:
                    f.config['subjects'][dependency.id] = {'record': dependency.ref, 'assertion_layer': 'textual_observation',
                        'risk': 'low', 'languages': ['und'], 'maker_id': unit.get('maker', {}).get('agent_ref', 'software:synthetic-test-fixture'),
                        'requested_use': 'research', 'access_allowed': True}
        f.occurrence_subject = f.members[0]
        claim = copy.deepcopy(f.claim.payload)
        claim.update(schema_version='tos_source_occurrence_motif_claim_v1', predicate='occurrence_motif_proposal',
            assertion_layer='semantic_interpretation', object={'kind': 'motif-proposal',
                'members': [row.id for row in f.members],
                'source_wording': {'text': 'Synthetic complete motif hypothesis, not a witness quote.',
                    'language': 'en', 'script': 'Latn', 'wording_kind': 'research_paraphrase'},
                'proposed_signification': 'A test hypothesis only.', 'grouping_basis': 'Three distinct synthetic spans.',
                'source_scope': 'Only this synthetic packet.', 'contrast': 'No real interpretation is asserted.',
                'limitations': 'Artificial competence and assessment test, no Sign promotion.'})
        native.write_bytes(f.claim_path, (json.dumps(claim) + '\n').encode())
        f.claim = Record.from_payload(claim['claim_id'], 1, claim)
        if form:
            body = copy.deepcopy(f.subject.payload)
            body['subject'] = f.claim.ref
            body['bindings']['context']['record'] = f.claim.ref
            path = claim_forms_path(Path(f.claim_path), f.claim.id).as_posix()
            native.write_json(path, {'schema_version': 'tos_human_form_set_v1',
                'subject': f.claim.ref, 'forms': [body], 'prior_forms': []})
            f.subject = Record.from_payload(body['form_id'], 1, body)
            required[f.claim.id] = f.claim.ref
        else:
            f.subject = f.claim
        f.required = [required[key] for key in sorted(required)]
        f.config['subjects'][f.identifier]['record'] = f.subject.ref
        if not form:
            f.config['subjects'][f.identifier]['assertion_layer'] = 'semantic_interpretation'
        for index, competence in enumerate(f.config['competencies']):
            competence['payload']['assertion_layers'].append('semantic_interpretation')
            f.config['authorities'][index]['payload'].update(
                assertion_layers=competence['payload']['assertion_layers'],
                competence_refs=[Record.from_payload(**competence).ref])
        f.save()
        return f

    def promotion_fixture(self, *, high=False):
        from datetime import datetime
        clock_patch = patch('assessment_journal.datetime')
        clock = clock_patch.start()
        self.addCleanup(clock_patch.stop)
        clock.now.return_value = datetime.fromisoformat('2026-09-05T12:00:00+00:00')
        f = self.motif_fixture()
        policy = Record.from_payload('tos.policy.knowledge-assessment', 2,
            json.loads((ROOT / 'ToS/doctrine/semantic-interchange/assessment-policy.v2.json').read_text()))
        f.config['policy'] = {'id': policy.id, 'version': policy.version,
                             'payload': policy.payload, 'origin_id': None}
        f.policy.policy = policy
        f.scope.update(requested_use='sign-promotion', risk='high' if high else 'moderate')
        for index, row in enumerate(f.config['competencies']):
            row['payload']['profile_ids'].extend(['sign-promotion', 'sign-promotion-high'])
            authority = f.config['authorities'][index]['payload']
            authority.update(policy=policy.ref, uses=['research', 'sign-promotion'],
                competence_refs=[Record.from_payload(**row).ref])
            authority['profile_ids'].extend(['sign-promotion', 'sign-promotion-high'])
        f.save()
        f.promotion_config = {'source_root': str(f.root), 'promotion_candidate_id': f.identifier,
                             'promotion_assessment_owner_config': str(f.owner)}
        request = self.claim_request(f)
        request['assessments'][0].update(policy=policy.ref,
            profile_id='sign-promotion-high' if high else 'sign-promotion')
        return f, request

    def test_sign_promotion_is_a_separate_current_agent_use_with_exact_grounding(self):
        from source_commands import _sign_promotion
        f, request = self.promotion_fixture()
        self.assertFalse(_sign_promotion(f.promotion_config, require_ready=False)['eligible'])
        with self.assertRaises(PermissionError):
            _sign_promotion(f.promotion_config)
        native_before = (f.root / f.native.packet_ref).read_bytes()
        result = f.run(request)
        view = _sign_promotion(f.promotion_config)
        basis = view['basis']
        self.assertTrue(view['eligible'])
        self.assertEqual(basis['candidate'], f.claim.ref)
        self.assertEqual(basis['required_sources'], f.required)
        self.assertEqual(basis['journal_revision'], result['result']['revision'])
        self.assertEqual(basis['use'], 'sign-promotion')
        self.assertFalse(basis['grants_current_use'])
        self.assertFalse(view['grants_issuance_authority'])
        self.assertEqual(view['current_admission']['reviewer_kinds'], ['agent'])
        self.assertEqual(native_before, (f.root / f.native.packet_ref).read_bytes())
        self.assertEqual(f.claim.payload['review_status'], 'unreviewed')
        self.assertNotIn(str(f.owner), json.dumps(basis))

    def test_research_or_lowered_risk_does_not_authorize_sign_issuance(self):
        from source_commands import _sign_promotion
        f = self.motif_fixture()
        f.run(self.claim_request(f))
        config = {'source_root': str(f.root), 'promotion_candidate_id': f.identifier,
                  'promotion_assessment_owner_config': str(f.owner)}
        with self.assertRaisesRegex(PermissionError, 'research admission'):
            _sign_promotion(config)
        f, request = self.promotion_fixture()
        f.scope['risk'] = 'low'
        f.save()
        with self.assertRaisesRegex(PermissionError, 'lowered risk'):
            _sign_promotion(f.promotion_config)

    def test_sign_promotion_keeps_limits_and_closes_on_revocation_or_metadata_only(self):
        from source_commands import _sign_promotion
        f, request = self.promotion_fixture()
        request['assessments'][0].update(decision='admit-with-limits',
            limits=['Only this exact synthetic candidate; no universal sign inventory.'])
        result = f.run(request)
        basis = _sign_promotion(f.promotion_config)['basis']
        self.assertEqual(basis['status'], 'admitted-with-limits')
        self.assertEqual(basis['limits'], request['assessments'][0]['limits'])
        authority = f.config['authorities'][0]
        authority['version'] += 1
        authority['payload'].update(authority_version=authority['version'], state='revoked')
        f.save()
        self.assertFalse(_sign_promotion(f.promotion_config, require_ready=False)['eligible'])
        with self.assertRaises(PermissionError):
            _sign_promotion(f.promotion_config)
        self.assertTrue(result['result']['receipt']['admission_at_commit']['can_use'])
        self.assertEqual(AssessmentJournal(Path(f.config['journal_directory']))._load(f.identifier)[0],
                         result['result']['revision'])
        f, request = self.promotion_fixture()
        f.run(request)
        for selection in f.config['native_text_units']:
            selection['read_scope'] = 'metadata_only'
        # Rebound native evidence changes its digest. Both source reading and
        # exact evidence stay false; an earlier positive vote cannot repair it.
        f.save()
        self.assertFalse(_sign_promotion(f.promotion_config, require_ready=False)['eligible'])

    def test_consequential_sign_promotion_needs_independent_reviewers_not_source_copies(self):
        from source_commands import _sign_promotion
        f, request = self.promotion_fixture(high=True)
        first = f.run(request)
        self.assertFalse(_sign_promotion(f.promotion_config, require_ready=False)['eligible'])
        second = copy.deepcopy(request['assessments'][0])
        actor = f.config['authorities'][1]['payload']['actor_id']
        second.update(assessment_id='tos.review.synthetic-sign-second',
            authority=Record.from_payload(**f.config['authorities'][1]).ref,
            competence=Record.from_payload(**f.config['competencies'][1]).ref,
            reviewer={'actor_id': actor, 'kind': 'agent'})
        f.config['principal_id'] = actor
        f.save()
        request.update(command_id='synthetic-sign-second', assessments=[second],
            expected_snapshot=f.describe()['owner_snapshot'], expected_revision=first['result']['revision'])
        f.run(request)
        self.assertTrue(_sign_promotion(f.promotion_config)['eligible'])
        self.assertEqual({row['origin_id'] for row in f.config['source_records']}, {ORIGIN})
        # Two account labels cannot manufacture independent reviewer groups.
        first_group = f.config['authorities'][0]['payload']['independence_group']
        authority = f.config['authorities'][1]
        authority['version'] += 1
        authority['payload'].update(authority_version=authority['version'], independence_group=first_group)
        f.save()
        described = f.describe()
        same_group = copy.deepcopy(second)
        same_group.update(assessment_id='tos.review.synthetic-sign-same-group',
            authority=Record.from_payload(**authority).ref,
            supersedes=[Record.from_payload(second['assessment_id'], 1, second).ref])
        request.update(command_id='synthetic-sign-same-group', assessments=[same_group],
            expected_snapshot=described['owner_snapshot'], expected_revision=described['result']['revision'])
        f.run(request)
        view = _sign_promotion(f.promotion_config, require_ready=False)
        self.assertFalse(view['eligible'])
        self.assertEqual(view['current_admission']['status'], 'deferred')
        self.assertIn(same_group['assessment_id'], {ref['id'] for ref in view['current_admission']['assessment_refs']})

    def test_sign_promotion_rejects_inline_candidate_and_confidential_owner_before_read(self):
        from source_commands import _sign_promotion
        f, request = self.promotion_fixture()
        f.config['source_records'] = [row for row in f.config['source_records'] if row['record_id'] != f.identifier]
        f.config['records'].append({'id': f.subject.id, 'version': f.subject.version,
            'payload': f.subject.payload, 'origin_id': ORIGIN})
        f.save()
        with self.assertRaisesRegex(PermissionError, 'not an inline record'):
            _sign_promotion(f.promotion_config)
        f.config['schema_version'] = 'tos_local_assessment_owner_v4'
        f.save()
        with self.assertRaisesRegex(PermissionError, 'public source root'):
            _sign_promotion(f.promotion_config)

    def sign_command_fixture(self):
        import os
        import source_commands as commands
        f, review = self.promotion_fixture()
        f.run(review)
        config = {'schema_version': commands.SIGN_CONFIG, 'uid': os.getuid(),
            'principal_id': 'software:synthetic-sign-writer', 'source_root': str(f.root),
            'source_path': 'ToS/source-witnesses/signs/synthetic-one/sign.json',
            'authority_ref': 'fixture:separate-operator-sign-issuance-grant',
            'allowed_form_ids': ['tos.form.synthetic-sign.name', 'tos.form.synthetic-sign.note'],
            'allowed_operations': ['sign.promote'], 'expires_at': '2099-01-01T00:00:00Z',
            'record_id': 'tos.sign.synthetic-one', 'profile_type_id': 'tos.entity.sign',
            'maker_type': 'software', 'provenance_event_id': 'tos.event.synthetic-sign-issuance',
            **{key: value for key, value in f.promotion_config.items() if key != 'source_root'}}
        (f.root / 'ToS/source-witnesses/signs').mkdir()
        owner = f.owner.parent / 'sign-owner.json'
        owner.write_text(json.dumps(config)); owner.chmod(0o600)
        described = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
        body = {'schema_version': 'tos_sign_description_record_v1', 'record_type': 'sign',
            'record_id': config['record_id'], 'record_version': 1, 'preferred_label': 'Условный знак',
            'notes': 'Synthetic Sign of the exact candidate, not historical or linguistic evidence.',
            'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                                'notes': {'language': 'en', 'script': 'Latn'}},
            'identity_status': 'provisional', 'source_refs': [f.claim_path],
            'external_identifiers': [], 'same_as_posture': 'no_equivalence_claim',
            'visibility': 'public_metadata_only', 'promotion_basis': described['promotion']['basis']}
        preview_request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create',
            'record': body, 'forms': [
                {'form_id': config['allowed_form_ids'][0], 'field_id': 'metadata.preferred-name'},
                {'form_id': config['allowed_form_ids'][1], 'field_id': 'metadata.source-note'}]}
        prepared = commands.run_local_command(owner, preview_request)
        request = {**preview_request, 'operation': 'sign.promote', 'command_id': 'synthetic-sign-once',
            'expected_configuration': described['owner_configuration'], 'expected_source': None,
            'expected_revision': None, 'expected_dependencies': prepared['expected_dependencies']}
        return f, owner, config, request

    def test_sign_command_issues_atomically_replays_history_and_refuses_generic_or_duplicate_issuance(self):
        import source_commands as commands
        f, owner, config, request = self.sign_command_fixture()
        before = (f.root / f.native.packet_ref).read_bytes()
        publish = commands._publish_new_directory
        checked = []
        def checked_publish(staging, target):
            # A normal append/withdraw uses this same independent lock handle.
            # It must be unable to publish a head between final check and rename.
            journal = AssessmentJournal(Path(f.config['journal_directory']), lock_timeout_seconds=0,
                                        protected_storage=True)
            with self.assertRaises(commands.JournalBusy):
                with journal._locked(journal._home(f.identifier)):
                    self.fail('assessment writer interleaved at Sign publication')
            checked.append(True)
            publish(staging, target)
        with patch.object(commands, '_publish_new_directory', checked_publish):
            result = commands.run_local_command(owner, request)
        self.assertEqual(checked, [True])
        self.assertFalse(result['grants_admission'])
        self.assertEqual(result['supported_operations'], ['sign.promote'])
        path = f.root / config['source_path']
        body = json.loads(path.read_bytes())
        self.assertEqual(body, request['record'])
        self.assertEqual(before, (f.root / f.native.packet_ref).read_bytes())
        replay = commands.run_local_command(owner, request)
        self.assertTrue(replay['replayed'])
        self.assertEqual(replay['receipt'], result['receipt'])
        forms = json.loads(path.with_name('sign.human-forms.json').read_bytes())
        self.assertTrue(all(any(binding['pointer'] == '/promotion_basis' for binding in form['bindings'].values())
                            for form in forms['forms']))
        generic = {key: value for key, value in config.items() if not key.startswith('promotion_')}
        generic.update(schema_version=commands.PROFILE_CONFIG, allowed_operations=['source.create'])
        owner.write_text(json.dumps(generic))
        with self.assertRaisesRegex(PermissionError, 'separately delegated'):
            commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
        config.update(record_id='tos.sign.synthetic-duplicate',
            source_path='ToS/source-witnesses/signs/synthetic-duplicate/sign.json')
        owner.write_text(json.dumps(config))
        with self.assertRaisesRegex(JournalConflict, 'candidate already has a Sign'):
            commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-create', 'record': {**body, 'record_id': config['record_id']}, 'forms': request['forms']})

    def test_sign_command_refuses_withdrawn_basis_at_final_publish_without_losing_history(self):
        import source_commands as commands
        f, owner, config, request = self.sign_command_fixture()
        target = f.root / config['source_path']
        forged = copy.deepcopy(request)
        forged['record']['promotion_basis']['candidate']['digest'] = 'sha256:' + '0' * 64
        with self.assertRaisesRegex(JournalConflict, 'exact current promotion basis'):
            commands.run_local_command(owner, forged)
        aliased = copy.deepcopy(request)
        aliased['record']['promotion_basis']['candidate']['version'] = float(
            aliased['record']['promotion_basis']['candidate']['version'])
        with self.assertRaisesRegex(JournalConflict, 'exact current promotion basis'):
            commands.run_local_command(owner, aliased)
        with self.assertRaises(ValueError):
            commands.run_local_command(owner, {**request, 'assessments': [{'decision': 'admit'}]})
        self.assertFalse(target.parent.exists())
        original_publish = commands._publish
        changed = False
        def revoke_after_staging(path, encoded):
            nonlocal changed
            original_publish(path, encoded)
            if path.name == 'source-create-receipt.json' and not changed:
                changed = True
                authority = f.config['authorities'][0]
                authority['version'] += 1
                authority['payload'].update(authority_version=authority['version'], state='revoked')
                f.save()
        with patch.object(commands, '_publish', revoke_after_staging), self.assertRaises(PermissionError):
            commands.run_local_command(owner, request)
        self.assertTrue(changed)
        self.assertFalse(target.parent.exists())
        self.assertEqual(list((f.root / 'ToS').glob('.source-create-*.pending')), [])
        self.assertTrue(f.head_paths())

    def test_motif_claim_and_form_assessment_require_every_member_and_native_ground(self):
        for form in (False, True):
            with self.subTest(form=form):
                f = self.motif_fixture(form=form)
                context = f.describe()['result']['command_context']
                self.assertEqual(context['required_sources'], f.required)
                self.assertEqual(context['source_read'], {'required': True, 'ready': True})
                self.assertEqual(len(f.required), 8 if form else 7)
                request = self.claim_request(f)
                for missing in f.required:
                    incomplete = copy.deepcopy(request)
                    incomplete['assessments'][0]['evidence'] = [row for row in incomplete['assessments'][0]['evidence']
                                                               if row['record'] != missing]
                    with self.subTest(missing=missing['id']), self.assertRaises(AssessmentRejected) as error:
                        f.run(incomplete)
                    self.assertIn('evidence.required-source-omitted', error.exception.invalid_assessments[0]['reasons'])
                    self.assertEqual(f.head_paths(), [])
                first = f.run(request)['result']
                self.assertTrue(first['current_admission']['can_use'])
                history = self.journal_bytes(f)
                self.assertTrue(f.run(request)['result']['replayed'])
                self.assertEqual(self.journal_bytes(f), history)
                # A later participant, not just the focal, invalidates current admission.
                changed = copy.deepcopy(f.members[2].payload)
                changed.update(record_version=2, notes='Changed third-member interpretation, still synthetic.')
                f.native.write_json(f.member_paths[2], changed)
                current = f.describe()['result']
                self.assertFalse(current['current_admission']['can_use'])
                self.assertEqual(current['revision'], first['revision'])
                self.assertEqual(self.journal_bytes(f), history)

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
