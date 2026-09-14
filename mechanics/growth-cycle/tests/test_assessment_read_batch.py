"""Bounded synthetic v2 read assembly; not substantive assessment or authority."""
import copy
from datetime import datetime
import json
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import test_knowledge_assessment as fixtures
import assessment_journal as journal
import knowledge_assessment
from knowledge_assessment import Record
import source_metadata_snapshot as publication
from source_witness_human_forms import AssessedFormSnapshot, materialize_metadata_forms, write_assessed_candidate

ROOT = fixtures.ROOT


class AssessmentReadBatchTests(unittest.TestCase):
    def fixture(self, count=2, *, ready=False, source_copy=False):
        assessor = fixtures.AssessmentPolicyTests()
        assessor.setUp()
        self.addCleanup(assessor.doCleanups)
        owner, config, form_path, source = assessor.assessed_form_fixture(source_copy=source_copy)
        package = json.loads(form_path.read_text())
        base = package['forms'][0]
        forms = [{**copy.deepcopy(base), 'form_id': f'tos.form.batch-fixture-{index}'} for index in range(count)]
        package['forms'] = forms
        form_path.write_text(json.dumps(package))
        scope = next(iter(config['subjects'].values()))
        config['subjects'] = {form['form_id']: {**copy.deepcopy(scope),
            'record': Record.from_payload(form['form_id'], form['form_version'], form).ref} for form in forms}
        config['source_records'] = [config['source_records'][0], *[
            {**config['source_records'][1], 'record_id': form['form_id']} for form in forms]]
        owner.write_text(json.dumps(config))
        ids = [form['form_id'] for form in forms]
        requests = [{'schema_version': 'tos_local_assessment_command_v1', 'operation': 'describe',
                     'subject_id': identity} for identity in ids]
        nodes = [{'node_id': 'fixture:source', 'source_ref': config['source_records'][0]['path'],
                  'source_sha256': source.ref['digest'].removeprefix('sha256:'),
                  'properties': {'source_record': source.payload,
                      'human_forms_source_ref': config['source_records'][1]['path'],
                      'human_forms': materialize_metadata_forms(source.payload, package, access_allowed=True)}}]
        fx = SimpleNamespace(assessor=assessor, owner=owner, config=config, form_path=form_path,
                             source=source, forms=forms, ids=ids, requests=requests, nodes=nodes,
                             root=Path(config['source_root']))
        if ready:
            for index in range(count):
                self.append(fx, index)
        return fx

    def append(self, fx, index, *, name=None):
        form = fx.forms[index]
        fx.assessor.subject = Record.from_payload(form['form_id'], form['form_version'], form)
        described = journal.run_local_command(fx.owner, fx.requests[index])
        request = {**fx.requests[index], 'operation': 'append', 'expected_subject': fx.assessor.subject.ref,
            'expected_snapshot': described['owner_snapshot'], 'expected_revision': described['result']['revision'],
            'command_id': name or f'synthetic-batch-review-{index}',
            'assessments': [fx.assessor.review(profile='interpretation',
                           name=name or f'tos.review.batch-fixture-{index}').assessment]}
        return journal.run_local_command(fx.owner, request)

    @staticmethod
    def materializations(descriptions):
        return [{'schema_version': 'tos_local_assessment_command_v1', 'operation': 'materialize-form',
                 'subject_id': reply['result']['command_context']['subject']['id'],
                 'expected_subject': reply['result']['command_context']['subject'],
                 'expected_snapshot': reply['owner_snapshot']} for reply in descriptions]

    def test_batch_matches_single_commands_and_preparation_count_does_not_scale_with_forms(self):
        preparations = []
        for count in (1, 6):
            with self.subTest(count=count):
                fx = self.fixture(count, source_copy=count == 6)
                expected = [journal.run_local_command(fx.owner, request) for request in fx.requests]
                requests = self.materializations(expected)
                packets = [journal.run_local_command(fx.owner, request) for request in requests]
                with patch.object(journal, '_source_records', wraps=journal._source_records) as source_reads, \
                     patch.object(journal.AssessmentJournal, '_locked', side_effect=AssertionError('read must not lock')), \
                     patch.object(journal.AssessmentJournal, '_write_blob', side_effect=AssertionError('read must not write')):
                    with journal.PublicSourceReadSession(fx.owner, fx.ids) as reader:
                        self.assertEqual(reader.read_batch(fx.requests), expected)
                        actual = reader.read_batch(requests)
                        self.assertEqual(actual, packets)
                        actual[0]['result']['materialization']['display_text'] = 'caller mutation is not admission'
                        self.assertEqual(reader.read_batch(requests), packets)
                    preparations.append(source_reads.call_count)
                self.assertFalse(list(Path(fx.config['journal_directory']).iterdir()))
                with self.assertRaises(journal.JournalConflict):
                    reader.read_batch(fx.requests)
        # One preparation, then the same two full source checks per batch.
        # This is a structural work bound, not a wall-clock acceptance test.
        self.assertEqual(preparations, [7, 7])

    def test_batch_refuses_scope_request_budget_and_version_widening(self):
        fx = self.fixture()
        malformed = [[], iter(fx.requests), fx.requests * 129,
                     [fx.requests[0], fx.requests[0]],
                     [{**fx.requests[0], 'subject_id': 'tos.form.not-selected'}],
                     [{**fx.requests[0], 'operation': 'append'}],
                     [{**fx.requests[0], 'prepared': {'verified': True}}],
                     [{**fx.requests[0], 'source_read_ready': True}],
                     [{**fx.requests[0], 'schema_version': 'unowned'}],
                     [{**fx.requests[0], 'subject_id': 'x' * journal.MAX_RECORD_BYTES}]]
        for requests in malformed:
            with self.subTest(requests=str(requests)[:80]):
                reader = journal.PublicSourceReadSession(fx.owner, fx.ids)
                with self.assertRaises((ValueError, PermissionError)):
                    reader.read_batch(requests)
                with self.assertRaises(journal.JournalConflict):
                    reader.read_batch(fx.requests)
        reader = journal.PublicSourceReadSession(fx.owner, fx.ids)
        with patch.object(journal.PublicSourceReadSession, 'MAX_OUTPUT_BYTES', 32), self.assertRaises(ValueError):
            reader.read_batch(fx.requests)
        with self.assertRaises(journal.JournalConflict):
            reader.read_batch(fx.requests)
        for version in ('v1', 'v3', 'v4', 'v5', 'v6'):
            fx.config['schema_version'] = 'tos_local_assessment_owner_' + version
            fx.owner.write_text(json.dumps(fx.config))
            with self.subTest(version=version), patch.object(journal, '_source_records',
                    side_effect=AssertionError('unsupported version must not read sources')):
                if version in ('v1', 'v3'):
                    self.assertIsNone(journal.PublicSourceReadSession.for_public_owner(fx.owner, fx.ids))
                else:
                    with self.assertRaises(PermissionError):
                        journal.PublicSourceReadSession.for_public_owner(fx.owner, fx.ids)

    def test_source_configuration_and_journal_drift_never_return_a_partial_batch(self):
        for mutation in ('source-bytes', 'configuration', 'journal'):
            with self.subTest(mutation=mutation):
                fx = self.fixture()
                reader = journal.PublicSourceReadSession(fx.owner, fx.ids)
                original, changed = journal.AssessmentJournal.inspect, []

                def inspect_then_change(owner, *args, **kwargs):
                    result = original(owner, *args, **kwargs)
                    if not changed:
                        changed.append(True)
                        if mutation == 'source-bytes':
                            path = fx.root / fx.config['source_records'][0]['path']
                            path.write_bytes(path.read_bytes() + b'\n')
                        elif mutation == 'configuration':
                            fx.config['authorities'][0]['payload']['state'] = 'revoked'
                            fx.owner.write_text(json.dumps(fx.config))
                        else:
                            self.append(fx, 0)
                    return result

                sentinel = object()
                result = sentinel
                with patch.object(journal.AssessmentJournal, 'inspect', inspect_then_change), \
                     self.assertRaises(journal.JournalConflict):
                    result = reader.read_batch(fx.requests)
                self.assertIs(result, sentinel)
                with self.assertRaises(journal.JournalConflict):
                    reader.read_batch(fx.requests)

    def test_source_schema_and_registry_drift_are_not_replaced_by_publication_epoch(self):
        import test_historical_claim_assessment as historical
        for ref in ('ToS/contracts/historical-claim.schema.json',
                    'ToS/doctrine/semantic-interchange/relation-types.v1.json'):
            helper = historical.HistoricalClaimAssessmentTests()
            self.addCleanup(helper.doCleanups)
            with self.subTest(ref=ref), helper.fixture() as fx:
                ids = [form['form_id'] for form in fx.forms['forms']]
                requests = [{'schema_version': 'tos_local_assessment_command_v1', 'operation': 'describe',
                             'subject_id': identity} for identity in ids]
                reader = journal.PublicSourceReadSession(fx.owner, ids)
                reader.read_batch(requests)
                self.assertIsNone(publication.PublicationSnapshot(fx.root).token)
                path = fx.root / ref
                path.write_bytes(path.read_bytes() + b'\n')
                with self.assertRaises(journal.JournalConflict):
                    reader.read_batch(requests)

    def test_assessment_grammar_is_fresh_pinned_and_separate_from_source_root(self):
        fx = self.fixture(ready=True)
        grammar = fx.owner.parent / 'independent-grammar'
        names = ('knowledge-assessment', 'knowledge-assessment-policy', 'knowledge-assessment-authority',
                 'knowledge-assessment-competence', 'knowledge-assessment-batch', 'human-form',
                 'human-form-template', 'human-form-set', 'corpus-record', 'claim-display-fields')
        for name in names:
            target = grammar / 'ToS/contracts' / (name + '.schema.json')
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((ROOT / 'ToS/contracts' / target.name).read_bytes())
        self.assertNotEqual(grammar, fx.root)
        # Prime the old root-keyed cache. A new session must not inherit it.
        knowledge_assessment._validators(grammar.resolve())
        reader = journal.PublicSourceReadSession(fx.owner, fx.ids, contract_root=grammar)
        before = reader.read_batch(fx.requests)
        self.assertTrue(before[0]['result']['current_admission']['can_use'])
        target = grammar / 'ToS/contracts/knowledge-assessment.schema.json'
        schema = json.loads(target.read_text())
        schema['properties']['rationale']['const'] = 'A new independently owned narrow grammar.'
        target.write_text(json.dumps(schema))
        with self.assertRaises(journal.JournalConflict):
            reader.read_batch(fx.requests)
        fresh = journal.PublicSourceReadSession(fx.owner, fx.ids, contract_root=grammar)
        # Retained batches now violate the exact selected assessment grammar;
        # old LRU validators cannot manufacture a successful current read.
        with self.assertRaises((journal.JournalCorruption, journal.ValidationError)):
            fresh.read_batch(fx.requests)

    def test_consumer_reuses_preparation_but_rechecks_time_and_cross_subject_bindings(self):
        fx = self.fixture(ready=True)
        original = copy.deepcopy(fx.nodes)
        with patch.object(journal, '_source_records', wraps=journal._source_records) as reads:
            snapshot = AssessedFormSnapshot(fx.owner, fx.ids)
            first = snapshot.materialize(fx.nodes)
            second = snapshot.materialize(fx.nodes)
            self.assertEqual(first, second)
            self.assertEqual(reads.call_count, 13)  # One prep + two guards for each of six batches.
        self.assertEqual(fx.nodes, original)
        self.assertTrue(first[0]['properties']['human_forms'][0]['admission']['can_use'])
        with patch.object(journal, 'datetime') as clock:
            clock.now.return_value = datetime.fromisoformat('2026-10-02T00:00:00+00:00')
            with self.assertRaises(journal.JournalConflict):
                snapshot.verify_current()
        for mutation in ('source', 'form', 'source-path', 'form-path', 'duplicate'):
            with self.subTest(mutation=mutation):
                changed = copy.deepcopy(fx.nodes)
                node = changed[0]
                if mutation == 'source':
                    node['properties']['source_record']['notes'] = 'Unbound copy.'
                elif mutation == 'form':
                    node['properties']['human_forms'][0]['subject'] = node['properties']['human_forms'][0]['form']
                elif mutation == 'source-path':
                    node['source_ref'] = 'ToS/source-witnesses/another/source.json'
                elif mutation == 'form-path':
                    node['properties']['human_forms_source_ref'] = 'ToS/source-witnesses/another/source.human-forms.json'
                else:
                    changed.append(copy.deepcopy(node))
                before = copy.deepcopy(changed)
                with self.assertRaises((ValueError, journal.JournalConflict)):
                    AssessedFormSnapshot(fx.owner, fx.ids).materialize(changed)
                self.assertEqual(changed, before)

    def test_both_real_build_payload_functions_share_one_ready_source_copy_snapshot(self):
        sys.path.insert(0, str(ROOT / 'tests'))
        from test_source_witness_bibliographic_graph import SourceWitnessBibliographicGraphTest
        import source_witness_bibliographic_graph_common as bibliographic
        import tos_corpus_index_common as corpus
        fx = self.fixture(source_copy=True)
        with SourceWitnessBibliographicGraphTest().historical_fixture() as (root, _, _, _, rebuild):
            target = root / fx.config['source_records'][1]['path']
            target.write_bytes(fx.form_path.read_bytes())
            fx.config['source_root'] = str(root)
            fx.owner.write_text(json.dumps(fx.config))
            fx.root, fx.form_path = root, target
            for index in range(len(fx.ids)):
                self.assertTrue(self.append(fx, index)['result']['current_admission']['can_use'])
            rebuild()  # The bounded fixture's actual source catalog, not a mock projection.
            (root / 'ToS/source_home.manifest.json').write_text(json.dumps({
                'schema_version': 'tos_source_home_v1', 'home': 'ToS', 'branches': []}))
            (root / corpus.SCHEMA_REF).write_bytes((ROOT / corpus.SCHEMA_REF).read_bytes())
            paths = tuple(sorted(path for path in (root / 'ToS').rglob('*') if path.is_file()))
            before = target.read_bytes()
            with patch.object(journal, '_source_records', wraps=journal._source_records) as reads, \
                 patch.object(journal.AssessmentJournal, '_publish_head', side_effect=AssertionError('builders must not append')):
                snapshot = AssessedFormSnapshot(fx.owner, fx.ids)
                graph = bibliographic.build_payload(root, assessed_forms=snapshot)
                with patch.object(corpus, 'REPO_ROOT', root), patch.object(corpus, 'TOS_ROOT', root / 'ToS'), \
                     patch.object(corpus, 'tracked_tos_paths', return_value=paths):
                    index = corpus.build_payload(assessed_forms=snapshot)
                self.assertEqual(reads.call_count, 13)
                snapshot.verify_current()
            graph_packets = {packet['form']['id']: packet for node in graph['nodes']
                for packet in node.get('properties', {}).get('human_forms', []) if packet['form']['id'] in fx.ids}
            corpus_packets = {packet['form']['id']: packet for node in index['source_navigation']['nodes']
                for packet in node.get('properties', {}).get('human_forms', []) if packet['form']['id'] in fx.ids}
            self.assertEqual(set(graph_packets), set(fx.ids))
            self.assertEqual(graph_packets, corpus_packets)
            for packet in graph_packets.values():
                self.assertEqual(packet['state'], 'ready')
                self.assertEqual(packet['derivation'], 'source-copy')
                self.assertTrue(packet['admission']['can_use'])
                self.assertEqual(packet['display_text'], fx.source.payload['notes'])
                self.assertIn({'slot': 'owner:subject', 'binding': {'record': fx.source.ref, 'pointer': ''},
                               'value': fx.source.payload}, packet['context'])
                self.assertFalse(packet['assessment_snapshot']['publication_authorized'])
            self.assertEqual(target.read_bytes(), before)

    def test_committed_withdrawal_and_corruption_cannot_reuse_a_ready_view(self):
        for mutation in ('withdraw', 'batch', 'head'):
            with self.subTest(mutation=mutation):
                fx = self.fixture(ready=True, source_copy=True)
                snapshot = AssessedFormSnapshot(fx.owner, fx.ids)
                prior = snapshot.materialize(fx.nodes)
                self.assertEqual(prior[0]['properties']['human_forms'][0]['state'], 'ready')
                history = journal.AssessmentJournal(Path(fx.config['journal_directory']))
                revision, chain = history._load(fx.ids[0])
                if mutation == 'withdraw':
                    form = fx.forms[0]
                    fx.assessor.subject = Record.from_payload(form['form_id'], form['form_version'], form)
                    old = chain[0]['events'][0]['assessment']
                    withdrawn = fx.assessor.review(profile='interpretation', decision='withdraw',
                                                   name='tos.review.batch-withdrawn').assessment
                    withdrawn['supersedes'] = [Record.from_payload(old['assessment_id'], 1, old).ref]
                    described = journal.run_local_command(fx.owner, fx.requests[0])
                    journal.run_local_command(fx.owner, {**fx.requests[0], 'operation': 'append',
                        'expected_subject': fx.assessor.subject.ref, 'expected_snapshot': described['owner_snapshot'],
                        'expected_revision': revision, 'command_id': 'synthetic-batch-withdrawal',
                        'assessments': [withdrawn]})
                    with self.assertRaises(journal.JournalConflict):
                        snapshot.verify_current()
                    current = AssessedFormSnapshot(fx.owner, fx.ids).materialize(fx.nodes)
                    packet = current[0]['properties']['human_forms'][0]
                    self.assertEqual(packet['state'], 'needs-assessment')
                    self.assertIsNone(packet['display_text'])
                    self.assertFalse(packet['admission']['can_use'])
                    self.assertEqual(len(history._load(fx.ids[0])[1]), 2)
                else:
                    home = history._home(fx.ids[0])
                    target = home / ('head' if mutation == 'head' else revision + '.json')
                    target.write_text('broken' if mutation == 'head' else '{}')
                    with self.assertRaises(journal.JournalCorruption):
                        snapshot.verify_current()
                    with self.assertRaises(journal.JournalConflict):
                        snapshot.verify_current()

    def test_post_fsync_grant_or_publication_drift_prevents_candidate_visibility(self):
        import os
        for mutation in ('grant', 'pending', 'ready-epoch'):
            with self.subTest(mutation=mutation):
                fx = self.fixture()
                snapshot = AssessedFormSnapshot(fx.owner, fx.ids)
                rendered = json.dumps(snapshot.materialize(fx.nodes))
                target = fx.owner.parent / 'candidate.json'
                original, changed = os.fsync, []

                def sync_then_change(descriptor):
                    original(descriptor)
                    if changed:
                        return
                    changed.append(True)
                    if mutation == 'grant':
                        fx.config['subjects'][fx.ids[0]]['access_allowed'] = False
                        fx.owner.write_text(json.dumps(fx.config))
                    else:
                        state = {'schema_version': publication.STATE_SCHEMA, 'generation': 1,
                            'transition_id': '1' * 32, 'phase': 'pending' if mutation == 'pending' else 'ready',
                            'transaction_id': 'sha256:' + '1' * 64, 'manifest_sha256': 'sha256:' + '2' * 64,
                            'outcome': None if mutation == 'pending' else 'rolled-back', 'recovery_authorization': None}
                        state['token'] = publication._digest(publication._canonical(state))
                        (fx.root / publication.CONTROL_REF).write_text(json.dumps(state))

                with patch.object(os, 'fsync', sync_then_change), self.assertRaises(
                        (journal.JournalConflict, publication.PublicationStateError)):
                    write_assessed_candidate(target, rendered, snapshot)
                self.assertFalse(target.exists())
                self.assertEqual(list(target.parent.glob('.tos-assessed-*')), [])


if __name__ == '__main__':
    unittest.main()
