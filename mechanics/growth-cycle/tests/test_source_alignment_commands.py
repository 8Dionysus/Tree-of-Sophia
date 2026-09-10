"""Synthetic two-source native alignment contracts; no translation assessment."""
from __future__ import annotations

import copy
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path[:0] = [str(ROOT / 'scripts'), str(ROOT / 'tests'),
               str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]

import test_source_text_unit_commands as unit_tests
from native_text_binding import NativeTextBindingResolver
from source_owner_context import OwnerLocalSourceContext
import source_alignment_commands as align
import source_commands as source
import source_text_layer_commands as layers


def encode(value):
    return source._canonical(value) + b'\n'


def target_value(value):
    if isinstance(value, str):
        value = value.replace('native-binding', 'native-target')
        return re.sub(r'sid-[a-f0-9]{32}', lambda m: 'sid-' + hashlib.sha256(('target-' + m[0]).encode()).hexdigest()[:32], value)
    if isinstance(value, dict):
        return {key: target_value(item) for key, item in value.items()}
    if isinstance(value, list):
        return [target_value(item) for item in value]
    return value


class NativeAlignmentCommandTests(unittest.TestCase):
    def setUp(self):
        self.seed = unit_tests.NativeUnitCommandTests()
        self.seed.setUp()
        self.addCleanup(self.seed.doCleanups)
        self.public, self.private = self.seed.public, self.seed.private
        self.fixture, self.prefix = self.seed.fixture, self.seed.prefix
        self.owner, self.context_path = self.seed.owner, self.seed.context_path
        for name in align.CONTRACTS:
            self.fixture.write_bytes('ToS/contracts/' + name, (ROOT / 'ToS/contracts' / name).read_bytes())
        # A second independent synthetic Expression/Edition/Item/native graph.
        # Exact text happens to match; no equivalence or translation is claimed.
        self.target = copy.deepcopy(self.fixture)
        for key, value in self.fixture.__dict__.items():
            setattr(self.target, key, target_value(value))
        for path in list((self.public / 'ToS/source-witnesses').rglob('*')):
            if not path.is_file():
                continue
            ref = path.relative_to(self.public).as_posix()
            if target_value(ref) == ref:
                continue
            if path.suffix == '.json':
                self.target.write_json(target_value(ref), target_value(json.loads(path.read_bytes())))
            else:
                self.target.write_bytes(target_value(ref), path.read_bytes())
        self.target.refresh()
        self.target_bindings = self.split_target()
        self.source_ref = self.prefix + 'alignments/first/' + align.BASENAME
        self.path = self.private / self.source_ref
        self.path.parent.parent.mkdir(mode=0o700)
        self.config = {'schema_version': align.CONFIG, 'uid': os.getuid(),
            'principal_id': 'actor:synthetic-mapping-supplier', 'authority_ref': 'operator:synthetic-mapping-grant',
            'expires_at': '2099-01-01T00:00:00Z', 'source_context_ref': str(self.context_path),
            'source_path': self.source_ref, 'allowed_operations': ['alignment.create'],
            'source_access': {'read_scope': 'exact_owner_local', 'access_allowed': True,
                'authority_ref': 'operator:synthetic-source-read', 'expires_at': '2099-01-01T00:00:00Z'},
            'alignment_access': {'derivation_allowed': True, 'authority_ref': 'operator:synthetic-alignment-write',
                'expires_at': '2099-01-01T00:00:00Z'},
            'record_id': 'tos.translation-alignment-record.sid-' + 'a' * 32,
            'alignment_id': 'tos.translation-alignment.sid-' + 'b' * 32,
            'claim_id': 'tos.translation-alignment-claim.sid-' + 'c' * 32,
            'provenance_event_id': 'tos.event.synthetic.native-alignment.first',
            'change_kind': 'initial', 'predecessor': None, 'competing_records': [],
            'native_bindings': {'source': [copy.deepcopy(self.fixture.binding)], 'target': copy.deepcopy(self.target_bindings)},
            'granularity': 'token', 'tokenization': {'source': True, 'target': True},
            'maker': {'maker_kind': 'imported_source', 'agent_ref': 'actor:synthetic-mapping-supplier',
                'made_at': '2026-09-09T00:00:00Z', 'method': 'Synthetic supplied proposal, not an aligner run.',
                'provenance_event_ref': 'tos.event.synthetic.native-alignment.first', 'method_output_posture': 'proposal_not_truth'}}
        source_refs = list(self.fixture.binding['ordered_anchor_refs'])
        target_refs = [ref for binding in self.target_bindings for ref in binding['ordered_anchor_refs']]
        self.proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create',
            'mapping': {'direction': 'source_to_target', 'correspondence_shape': 'one_to_many',
                'order_posture': 'monotonic', 'ordered_source_anchor_refs': source_refs, 'ordered_target_anchor_refs': target_refs},
            'qualifications': {'translation_techniques': ['unresolved'], 'epistemic_status': 'uncertain',
                'certainty': {'value': 0.2, 'meaning': 'maker_declared_uncertainty_not_truth_probability'},
                'status_reason': 'Synthetic correspondence proposal only.',
                'evidence': [{'evidence_ref': self.fixture.policy_ref, 'role': 'method_input',
                    'source_anchor_refs': source_refs, 'target_anchor_refs': target_refs,
                    'description': 'Synthetic exact native inputs; not assessed translation.'}]}}
        self.write_owner()

    def split_target(self, fixture=None):
        fixture = fixture or self.target
        packet = fixture.packet
        first = packet['anchors'][1]
        second = copy.deepcopy(first)
        second.update(anchor_ref=first['anchor_ref'] + '.second', ordinal=3)
        first['selector']['end'] = 5
        second['selector']['start'] = 5
        for anchor in (first, second):
            start, end = anchor['selector']['start'], anchor['selector']['end']
            anchor['exact_sha256'] = hashlib.sha256(fixture.text[start:end].encode()).hexdigest()
        packet['anchors'][-1]['ordinal'] = 4
        packet['anchors'].insert(2, second)
        unit = copy.deepcopy(packet['units'][0])
        unit['unit_id'] = 'tos.text-unit.sid-' + ('7' if fixture is self.target else '6') * 32
        unit['ordered_anchor_refs'] = [second['anchor_ref']]
        packet['units'].append(unit)
        packet['segmentations'][0]['ordered_unit_refs'].append(unit['unit_id'])
        fixture.refresh()
        second_binding = copy.deepcopy(fixture.binding)
        second_binding.update(unit_id=unit['unit_id'], ordered_anchor_refs=unit['ordered_anchor_refs'])
        return [copy.deepcopy(fixture.binding), second_binding]

    def write_owner(self):
        self.owner.write_bytes(encode(self.config))
        self.owner.chmod(0o600)

    def run_command(self, request):
        return source.run_local_command(self.owner, request)

    def prepare(self):
        prepared = self.run_command(self.proposal)
        self.request = {**self.proposal, 'operation': self.config['allowed_operations'][0],
            'command_id': 'synthetic-alignment-command-' + self.path.parent.name,
            'expected_configuration': prepared['owner_configuration'], 'expected_dependencies': prepared['expected_dependencies'],
            'expected_source': None, 'expected_revision': None}
        return prepared

    def create(self):
        self.prepare()
        result = self.run_command(self.request)
        return result, json.loads(self.path.read_bytes())

    def resolver(self):
        context = OwnerLocalSourceContext.load(self.context_path)
        return NativeTextBindingResolver(self.public, owner_context=context, read_bytes=source._read)

    def successor(self, kind='describe', label='second'):
        prior_raw = self.path.read_bytes()
        prior = json.loads(prior_raw)
        old_ref = align.record_ref(self.config['source_path'], prior, prior_raw)
        self.source_ref = self.prefix + 'alignments/' + label + '/' + align.BASENAME
        self.path = self.private / self.source_ref
        self.config.update(source_path=self.source_ref, change_kind=kind, predecessor=old_ref,
            allowed_operations=['alignment.revise'], provenance_event_id='tos.event.synthetic.native-alignment.' + label)
        self.config['maker']['provenance_event_ref'] = self.config['provenance_event_id']
        self.proposal['operation'] = 'prepare-revise'
        self.proposal['qualifications']['status_reason'] += ' Revised qualification.'
        if kind == 'remap':
            self.config['claim_id'] = 'tos.translation-alignment-claim.sid-' + 'd' * 32
            self.proposal['mapping']['correspondence_shape'] = 'one_to_one'
            self.proposal['mapping']['ordered_target_anchor_refs'] = self.proposal['mapping']['ordered_target_anchor_refs'][:1]
        self.write_owner()
        return old_ref, prior_raw

    def test_create_exact_replay_inspect_and_provenance_ceiling(self):
        result, body = self.create()
        self.assertFalse(result['aligner_executed'])
        self.assertFalse(result['grants_admission'])
        self.assertNotIn('source_path', result)
        self.assertEqual(body['claim']['mapping']['correspondence_shape'], 'one_to_many')
        self.assertEqual(body['rights_and_visibility']['effective_visibility'], 'local_only')
        original = {p.name: p.read_bytes() for p in self.path.parent.iterdir()}
        replay = self.run_command(self.request)
        self.assertTrue(replay['replayed'])
        self.assertEqual(original, {p.name: p.read_bytes() for p in self.path.parent.iterdir()})
        event = json.loads(original['source-create-provenance.jsonl'])
        self.assertEqual(event['activity']['event_type'], 'annotation')
        self.assertEqual(event['method']['model_invocations'], [])
        self.assertIn('no aligner', event['activity']['warnings'][0])
        self.assertTrue(all(row['content_disclosure'] == 'private_content' for group in event['entities'].values() for row in group))
        original_read = NativeTextBindingResolver._read
        def metadata_only(resolver, ref, **kwargs):
            self.assertFalse(kwargs.get('content', False), 'inspect opened representation bytes')
            return original_read(resolver, ref, **kwargs)
        with patch.object(NativeTextBindingResolver, '_read', metadata_only):
            inspected = self.run_command({'schema_version': align.contract.REQUEST, 'operation': 'inspect'})
        self.assertFalse(inspected['content_verified'])
        self.assertEqual(inspected['record_version'], 1)
        self.assertEqual(os.stat(self.path).st_mode & 0o777, 0o600)
        self.assertEqual(os.stat(self.path.parent).st_mode & 0o777, 0o700)

    def test_describe_remap_and_exact_historical_inspection(self):
        _, initial = self.create()
        original_owner, original_request = copy.deepcopy(self.config), copy.deepcopy(self.request)
        first_ref, first_raw = self.successor()
        _, described = self.create()
        self.assertEqual(described['alignment_id'], initial['alignment_id'])
        self.assertEqual(described['claim']['claim_id'], initial['claim']['claim_id'])
        self.assertEqual(described['claim']['claim_version'], 2)
        self.assertEqual(described['claim']['predecessor'], align.claim_ref(initial['claim']))
        self.assertEqual(described['claim']['mapping'], initial['claim']['mapping'])
        self.assertEqual((self.private / first_ref['record_ref']).read_bytes(), first_raw)
        inspected = self.run_command({'schema_version': align.contract.REQUEST, 'operation': 'inspect-version', 'source': first_ref})
        self.assertEqual(inspected['record_version'], 1)
        self.successor('remap', 'third')
        _, remapped = self.create()
        self.assertEqual(remapped['record_version'], 3)
        self.assertEqual(remapped['alignment_id'], initial['alignment_id'])
        self.assertNotEqual(remapped['claim']['claim_id'], initial['claim']['claim_id'])
        self.assertEqual(remapped['claim']['claim_version'], 1)
        self.config = original_owner
        self.write_owner()
        self.assertTrue(self.run_command(original_request)['replayed'])

    def test_schema_and_shared_mapping_negative_controls(self):
        body = align._prepare(self.config, self.proposal)[0]
        mutations = {
            'wrong-shape': lambda b: b['claim']['mapping'].update(correspondence_shape='one_to_one'),
            'omission-with-target': lambda b: b['claim']['mapping'].update(correspondence_shape='source_omission', order_posture='not_applicable'),
            'undeclared-anchor': lambda b: b['claim']['mapping']['ordered_target_anchor_refs'].append('tos.anchor.synthetic.foreign'),
            'missing-evidence': lambda b: b['claim']['qualifications']['evidence'][0].update(source_anchor_refs=[]),
            'mixed-technique': lambda b: b['claim']['qualifications'].update(translation_techniques=['unresolved', 'literal']),
            'tokenization-absent': lambda b: b['target_side'].update(tokenization=None),
            'monotonic-reversed': lambda b: b['claim']['mapping']['ordered_target_anchor_refs'].reverse(),
            'anchor-raw-drift': lambda b: b['target_side']['anchors'][0].update(exact_sha256='0' * 64),
            'rights-widening': lambda b: b['rights_and_visibility'].update(effective_visibility='public'),
            'review-laundering': lambda b: b.update(status='accepted'),
            'provider-laundering': lambda b: b.update(execution_posture='aligner_executed'),
            'foreign-property': lambda b: b.update(review_refs=['review:invented']),
        }
        for name, mutation in mutations.items():
            with self.subTest(name=name):
                changed = copy.deepcopy(body)
                mutation(changed)
                with self.assertRaises((ValueError, PermissionError)):
                    align.validate_record(self.resolver(), changed)
        self.assertFalse(self.path.parent.exists())

    def test_omission_addition_and_reordered_proposals_remain_unassessed(self):
        for shape, source_refs, target_refs, order in (
            ('source_omission', self.proposal['mapping']['ordered_source_anchor_refs'], [], 'not_applicable'),
            ('target_addition', [], self.proposal['mapping']['ordered_target_anchor_refs'], 'not_applicable'),
            ('one_to_many', self.proposal['mapping']['ordered_source_anchor_refs'], list(reversed(self.proposal['mapping']['ordered_target_anchor_refs'])), 'reordered')):
            with self.subTest(shape=shape):
                proposal = copy.deepcopy(self.proposal)
                proposal['mapping'].update(correspondence_shape=shape, ordered_source_anchor_refs=source_refs,
                    ordered_target_anchor_refs=target_refs, order_posture=order)
                body = align._prepare(self.config, proposal)[0]
                self.assertEqual(body['status'], 'proposed')
                self.assertFalse(align.validate_record(self.resolver(), body)['assessment_applied'])

    def test_rights_denial_on_second_side_precedes_all_source_bytes(self):
        self.target.rights['derivative_posture'] = 'permission_required'
        self.target.refresh()
        self.config['native_bindings']['target'] = [copy.deepcopy(self.target.binding),
            {**copy.deepcopy(self.target.binding), 'unit_id': self.target_bindings[1]['unit_id'],
             'ordered_anchor_refs': self.target_bindings[1]['ordered_anchor_refs']}]
        self.write_owner()
        original = NativeTextBindingResolver._read
        opened = []
        def traced(resolver, ref, **kwargs):
            if kwargs.get('content'):
                opened.append(ref)
            return original(resolver, ref, **kwargs)
        with patch.object(NativeTextBindingResolver, '_read', traced):
            with self.assertRaises(PermissionError):
                self.prepare()
        self.assertEqual(opened, [])
        self.assertFalse(self.path.parent.exists())

    def test_revision_rekeys_remaps_and_stale_forks_fail_closed(self):
        self.create()
        self.successor()
        saved = copy.deepcopy(self.config)
        for key, value in (('alignment_id', 'tos.translation-alignment.sid-' + 'f' * 32),
                           ('claim_id', 'tos.translation-alignment-claim.sid-' + 'f' * 32)):
            self.config = copy.deepcopy(saved)
            self.config[key] = value
            self.write_owner()
            with self.assertRaises(ValueError):
                self.prepare()
        self.config = copy.deepcopy(saved)
        self.write_owner()
        proposal = copy.deepcopy(self.proposal)
        self.proposal['mapping']['ordered_target_anchor_refs'].reverse()
        self.proposal['mapping']['order_posture'] = 'reordered'
        with self.assertRaises(ValueError):
            self.prepare()
        self.proposal = proposal
        self.create()
        self.config = saved
        self.config['source_path'] = self.prefix + 'alignments/stale/' + align.BASENAME
        self.config['provenance_event_id'] = 'tos.event.synthetic.native-alignment.stale'
        self.config['maker']['provenance_event_ref'] = self.config['provenance_event_id']
        self.write_owner()
        with self.assertRaises(source.JournalConflict):
            self.prepare()

    def test_competing_record_does_not_rewrite_original(self):
        _, initial = self.create()
        old_raw = self.path.read_bytes()
        previous = align.record_ref(self.config['source_path'], initial, old_raw)
        self.config.update(source_path=self.prefix + 'alignments/alternative/' + align.BASENAME,
            record_id='tos.translation-alignment-record.sid-' + 'd' * 32,
            alignment_id='tos.translation-alignment.sid-' + 'e' * 32,
            claim_id='tos.translation-alignment-claim.sid-' + 'f' * 32,
            change_kind='competing', competing_records=[previous],
            provenance_event_id='tos.event.synthetic.native-alignment.alternative')
        self.config['maker']['provenance_event_ref'] = self.config['provenance_event_id']
        self.path = self.private / self.config['source_path']
        self.write_owner()
        _, competitor = self.create()
        self.assertEqual(competitor['competing_records'], [previous])
        self.assertEqual((self.private / previous['record_ref']).read_bytes(), old_raw)
        self.assertNotEqual(competitor['alignment_id'], initial['alignment_id'])

    def test_partial_stage_resumes_from_the_retained_exact_plan(self):
        self.prepare()
        real = layers._write_new
        written = []
        def interrupted(path, raw):
            real(path, raw)
            written.append(path)
            if len(written) == 3:
                raise OSError('synthetic interruption after a complete staged file')
        with patch.object(layers, '_write_new', interrupted):
            with self.assertRaises(OSError):
                self.run_command(self.request)
        self.assertFalse(self.path.parent.exists())
        recovery = self.run_command({'schema_version': align.contract.REQUEST, 'operation': 'inspect-recovery',
                                     'command_id': self.request['command_id']})
        self.assertEqual(recovery['recovery_state'], 'retained_exact_plan')
        self.run_command(self.request)
        self.assertTrue(self.path.exists())

    def test_torn_stage_is_not_overwritten_or_discarded(self):
        self.prepare()
        real = layers._write_new
        written = []
        def interrupted(path, raw):
            real(path, raw)
            written.append(path)
            if len(written) == 3:
                raise OSError('synthetic interruption')
        with patch.object(layers, '_write_new', interrupted):
            with self.assertRaises(OSError):
                self.run_command(self.request)
        damaged = written[1]
        damaged.write_bytes(b'{torn')
        plan_before = written[0].read_bytes()
        with self.assertRaises(source.JournalCorruption):
            self.run_command(self.request)
        self.assertEqual(damaged.read_bytes(), b'{torn')
        self.assertEqual(written[0].read_bytes(), plan_before)
        self.assertFalse(self.path.parent.exists())

    def test_exact_predecessor_claim_and_descriptive_scope_guards(self):
        self.create()
        self.successor()
        body = align._prepare(self.config, self.proposal)[0]
        def drop_delta(b):
            raw = (self.private / b['predecessor']['record_ref']).read_bytes()
            previous = json.loads(raw)
            b['claim']['qualifications'] = previous['claim']['qualifications']
        mutations = {
            'claim-hash': lambda b: b['claim']['predecessor'].update(sha256='0' * 64),
            'claim-version': lambda b: b['claim']['predecessor'].update(claim_version=2),
            'record-hash': lambda b: b['predecessor'].update(sha256='0' * 64),
            'record-version': lambda b: b['predecessor'].update(record_version=2),
            'granularity-drift': lambda b: b.update(granularity='sentence'),
            'unit-version': lambda b: b['native_bindings']['source'][0].update(unit_version=2),
            'no-description-delta': drop_delta,
        }
        for name, mutation in mutations.items():
            with self.subTest(name=name):
                changed = copy.deepcopy(body)
                mutation(changed)
                with self.assertRaises((ValueError, PermissionError)):
                    align.validate_record(self.resolver(), changed)
        self.assertFalse(self.path.parent.exists())

        changed = copy.deepcopy(body)
        changed['claim']['predecessor']['sha256'] = '0' * 64
        original_read, opened = NativeTextBindingResolver._read, []
        def traced(resolver, ref, **kwargs):
            if kwargs.get('content'):
                opened.append(ref)
            return original_read(resolver, ref, **kwargs)
        with patch.object(NativeTextBindingResolver, '_read', traced):
            with self.assertRaises(ValueError):
                align.validate_record(self.resolver(), changed, verify_content=True)
        self.assertEqual(opened, [])
        self.config['predecessor']['record_version'] += 1
        self.write_owner()
        with self.assertRaises(PermissionError):
            self.run_command({'schema_version': align.contract.REQUEST, 'operation': 'inspect'})

    def test_multiple_source_and_target_members_use_the_same_owner_contract(self):
        self.config['native_bindings']['source'] = self.split_target(self.fixture)
        source_refs = [ref for binding in self.config['native_bindings']['source'] for ref in binding['ordered_anchor_refs']]
        self.proposal['mapping'].update(correspondence_shape='many_to_many', ordered_source_anchor_refs=source_refs)
        self.proposal['qualifications']['evidence'][0]['source_anchor_refs'] = source_refs
        self.write_owner()
        body = align._prepare(self.config, self.proposal)[0]
        self.assertEqual(body['claim']['mapping']['correspondence_shape'], 'many_to_many')
        self.proposal['mapping'].update(correspondence_shape='many_to_one',
            ordered_target_anchor_refs=self.proposal['mapping']['ordered_target_anchor_refs'][:1])
        body = align._prepare(self.config, self.proposal)[0]
        self.assertEqual(body['claim']['mapping']['correspondence_shape'], 'many_to_one')

    def test_changed_source_or_command_identity_never_replaces_a_package(self):
        self.create()
        original = {p.name: p.read_bytes() for p in self.path.parent.iterdir()}
        with self.assertRaises(source.JournalConflict):
            self.run_command({**self.request, 'command_id': 'another-command'})
        (self.public / self.fixture.content_ref).write_bytes(self.fixture.content + b'corrupt')
        with self.assertRaises(ValueError):
            self.run_command(self.request)
        self.assertEqual(original, {p.name: p.read_bytes() for p in self.path.parent.iterdir()})

    def test_revoked_grant_before_content_read_fails_with_actual_permission_cause(self):
        original = source._read
        content_paths = {self.public / f.content_ref for f in (self.fixture, self.target)}
        opened, revoked = [], []
        def revoke(path, limit):
            if Path(path) in content_paths:
                opened.append(str(path))
            raw = original(path, limit)
            if Path(path) == self.public / self.target.rights_ref and not revoked:
                revoked.append(True)
                self.config['source_access']['expires_at'] = '2000-01-01T00:00:00Z'
                self.write_owner()
            return raw
        with patch.object(source, '_read', revoke):
            with self.assertRaises((ValueError, PermissionError)) as caught:
                self.prepare()
        error, causes = caught.exception, []
        while error is not None and error not in causes:
            causes.append(error)
            error = error.__cause__ or error.__context__
        self.assertTrue(any(isinstance(error, PermissionError) for error in causes))
        self.assertTrue(revoked)
        self.assertEqual(opened, [])
        self.assertFalse(self.path.parent.exists())

    def test_shared_deadline_is_checked_before_source_access(self):
        with self.assertRaisesRegex(ValueError, 'deadline'):
            align._prepare(self.config, self.proposal, deadline=0)
        self.assertFalse(self.path.parent.exists())

    def test_grant_discovery_and_describe_do_not_open_sources(self):
        original = NativeTextBindingResolver._read
        def contracts_only(resolver, ref, **kwargs):
            self.assertTrue(ref.startswith('ToS/contracts/'))
            return original(resolver, ref, **kwargs)
        with patch.object(NativeTextBindingResolver, '_read', contracts_only):
            described = self.run_command({'schema_version': align.contract.REQUEST, 'operation': 'describe'})
        self.assertNotIn(self.prefix, json.dumps(described))
        self.assertFalse(described['aligner_executed'])
        public = next(h for h in source.command_handlers() if h.handler_id == 'owner-local-native-translation-alignment').public()
        self.assertEqual(public['authorization_status'], 'not_evaluated')
        self.assertNotIn(self.prefix, json.dumps(public))
        self.config['maker']['maker_kind'] = 'model'
        self.write_owner()
        with self.assertRaises(PermissionError):
            self.run_command({'schema_version': align.contract.REQUEST, 'operation': 'describe'})


if __name__ == '__main__':
    unittest.main()
