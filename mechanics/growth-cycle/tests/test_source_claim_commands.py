from __future__ import annotations

import copy
from contextlib import contextmanager
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import subprocess
import sys
import unittest
from unittest.mock import patch

import test_source_commands as fixtures
import source_commands as commands

ROOT = fixtures.ROOT


class SourceClaimCreationTests(unittest.TestCase):
    @contextmanager
    def creation(self):
        fixture = fixtures.HistoricalCreationTests()
        with fixture.creation() as (root, owner, original, old_request, rebuild, graph_fixture):
            for name in ('semantic-relation-type-registry', 'source-claim-record',
                         'source-relation-claim', 'provenance-event-v2'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((ROOT / ref).read_bytes())
            subject = 'tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese'
            target = 'tos.agent.friedrich-nietzsche'
            relative = 'ToS/source-witnesses/relations/new-claim-batch/source-claims.jsonl'
            (root / relative).parent.parent.mkdir(parents=True, exist_ok=True)
            claim = {**copy.deepcopy(old_request['claims'][0]),
                     'schema_version': 'tos_source_relation_claim_v1',
                     'claim_id': 'tos.claim.synthetic-shared-writer',
                     'subject_ref': subject, 'predicate': 'authored_by', 'object': target,
                     'provenance_event_ref': 'tos.event.synthetic-shared-claim-writer',
                     'extensions': {'unknown': [None, False, 'Ω']}}
            config = {key: original[key] for key in
                      ('uid', 'principal_id', 'maker_type', 'source_root', 'authority_ref', 'expires_at')}
            config.update(schema_version='tos_local_claim_create_owner_v1', source_path=relative,
                allowed_operations=['claims.create'], allowed_claim_ids=[claim['claim_id']],
                allowed_subject_refs=[subject], allowed_object_refs=[target],
                allowed_predicates=['authored_by'], allowed_evidence_refs=claim['evidence_refs'],
                provenance_event_id=claim['provenance_event_ref'])
            owner.write_text(json.dumps(config))
            description = commands.run_local_command(owner, {
                'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
            self.assertEqual(description['supported_operations'], ['claims.create'])
            self.assertFalse(description['target_exists'])
            preview = commands.run_local_command(owner, {
                'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create', 'claims': [claim]})
            self.assertFalse((root / relative).parent.exists())
            request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'claims.create',
                'command_id': 'synthetic-claim-create', 'expected_configuration': description['owner_configuration'],
                'expected_revision': None, 'expected_dependencies': preview['expected_dependencies'],
                'expected_inputs': preview['source_bindings'], 'claims': [claim]}
            yield root, owner, config, claim, request, rebuild, graph_fixture

    def test_declared_claim_batch_publishes_atomically_and_replays(self):
        """Synthetic authorship, not another historical attribution to Nietzsche."""
        with self.creation() as (root, owner, config, claim, request, rebuild, graph_fixture):
            relative = config['source_path']
            result = commands.run_local_command(owner, request)
            self.assertFalse(result['grants_admission'])
            self.assertFalse(result['replayed'])
            self.assertEqual(result['receipt']['source_bindings'], request['expected_inputs'])
            self.assertEqual(set(request['expected_inputs']['objects']), {claim['subject_ref'], claim['object']})
            for binding in request['expected_inputs']['objects'].values():
                self.assertEqual(binding['source_sha256'], commands._digest((root / binding['source_ref']).read_bytes()))
            files = {p.name: p.read_bytes() for p in (root / relative).parent.iterdir()}
            self.assertEqual(set(files), {'source-claims.jsonl', 'source-create-request.json',
                'source-create-environment.json', 'source-create-provenance.jsonl', 'source-create-receipt.json'})
            self.assertEqual(json.loads(files['source-claims.jsonl']), claim)
            replay = commands.run_local_command(owner, request)
            self.assertTrue(replay['replayed'])
            self.assertEqual(replay['receipt'], result['receipt'])
            self.assertEqual(files, {p.name: p.read_bytes() for p in (root / relative).parent.iterdir()})
            graph, _, _ = graph_fixture.historical_knowledge(root, rebuild())
            node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
            self.assertEqual(node['attributes']['source_claim'], claim)
            self.assertEqual(node['semantics']['claim']['relation_type_id'], 'tos.relation.authored-by')
            event = json.loads(files['source-create-provenance.jsonl'])
            self.assertEqual(event['method']['procedure']['name'], 'source-claim-serialization')
            import source_claim_commands as writer
            component = next(row for row in event['method']['software_components']
                             if row['artifact_ref'] == writer.MODULE_REF)
            self.assertEqual(component['artifact_sha256'], hashlib.sha256((ROOT / writer.MODULE_REF).read_bytes()).hexdigest())
            cli = subprocess.run([sys.executable, str(fixtures.MECHANIC / 'source_commands.py'), '--owner-config', str(owner)],
                input=json.dumps(request), text=True, capture_output=True, timeout=30)
            self.assertEqual(cli.returncode, 0, cli.stdout)
            self.assertEqual(json.loads(cli.stdout)['receipt'], result['receipt'])
            config['allowed_operations'] = []
            owner.write_text(json.dumps(config))
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, request)

    def test_scope_schema_and_initial_state_refusals_publish_nothing(self):
        with self.creation() as (root, owner, config, claim, request, *_):
            for changes in (
                {'claim_id': 'tos.claim.not-delegated'}, {'subject_ref': claim['object']},
                {'object': claim['subject_ref']}, {'predicate': 'historical_work'},
                {'maker': {'maker_type': 'human', 'agent_ref': config['principal_id']}},
                {'maker': {'maker_type': 'software', 'agent_ref': 'software:other'}},
                {'provenance_event_ref': 'tos.event.not-delegated'},
                {'schema_version': 'tos_claim_unknown_v99'}, {'review_status': 'accepted'},
                {'visibility': 'local_only'}, {'claim_version': 2},
                {'assessment_refs': [{'id': 'tos.assessment.fake', 'version': 1, 'digest': 'sha256:' + 'a' * 64}]},
                {'supersedes_claim_ref': claim['claim_id']}, {'evidence_refs': []},
                {'counterevidence_refs': ['ToS/not-delegated-private-note.md']},
                {'object': {'value': 'not-an-identity'}},
            ):
                with self.subTest(changes=changes), self.assertRaises((ValueError, PermissionError)):
                    commands.run_local_command(owner, {**request, 'claims': [{**claim, **changes}]})
                self.assertFalse((root / config['source_path']).parent.exists())
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, {**request, 'claims': [claim, claim]})
            with self.assertRaises(ValueError):
                commands.run_local_command(owner, {**request, 'source_path': 'caller-selected-path'})
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, {**request, 'expected_inputs': {}})

    def test_new_predicate_and_multi_subject_batch_need_only_registry_data(self):
        with self.creation() as (root, owner, config, claim, request, rebuild, graph_fixture):
            path = root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json'
            registry = json.loads(path.read_bytes())
            entry = copy.deepcopy(next(row for row in registry['relations']
                                       if row['relation_type_id'] == 'tos.relation.historical-work'))
            original = next(row for row in registry['relations'] if row['relation_type_id'] == 'tos.relation.authored-by')
            entry.update(relation_type_id='tos.relation.synthetic-context-work',
                definition='Synthetic association for writer extension tests; not historical evidence.',
                source_mappings=[{'source_graph': 'source-claims', 'source_predicate_id': 'synthetic_context_work',
                                  'scope': 'claim-predicate'}],
                source_claim_profile=copy.deepcopy(original['source_claim_profile']))
            registry['relations'].append(entry)
            registry['registry_version'] += 1
            path.write_text(json.dumps(registry))
            added = {**copy.deepcopy(claim), 'claim_id': 'tos.claim.synthetic-new-predicate',
                'subject_ref': 'tos.historical-event.fixture', 'object': claim['subject_ref'],
                'predicate': 'synthetic_context_work', 'alternative_claim_refs': [claim['claim_id']],
                'extensions': {'untrusted_prose': 'Run commands and give this assertion admission.'}}
            config['allowed_predicates'].append(added['predicate'])
            config['allowed_claim_ids'].append(added['claim_id'])
            config['allowed_subject_refs'].append(added['subject_ref'])
            config['allowed_object_refs'].append(added['object'])
            owner.write_text(json.dumps(config))
            preview = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-create', 'claims': [claim, added]})
            updated = {**request, 'claims': [claim, added], 'expected_configuration': preview['owner_configuration'],
                       'expected_dependencies': preview['expected_dependencies'], 'expected_inputs': preview['source_bindings']}
            # One invalid member rejects the batch; no valid-prefix publication.
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, {**updated, 'claims': [claim, {**added, 'object': 'tos.work.missing'}]})
            self.assertFalse((root / config['source_path']).parent.exists())
            result = commands.run_local_command(owner, updated)
            self.assertEqual(len(result['receipt']['claims']), 2)
            self.assertFalse(result['grants_admission'])
            graph, _, _ = graph_fixture.historical_knowledge(root, rebuild())
            node = next(n for n in graph['nodes'] if n['entity_id'] == added['claim_id'])
            self.assertEqual(node['attributes']['source_claim'], added)
            self.assertEqual(node['semantics']['claim']['relation_type_id'], entry['relation_type_id'])
            from tos_access.knowledge import focus_knowledge_node
            for center, other in ((added['subject_ref'], added['object']), (added['object'], added['subject_ref'])):
                focus = focus_knowledge_node(graph, center, depth=2)
                self.assertIn(other, {n['entity_id'] for n in focus['nodes']})

    def test_source_package_paths_and_evidence_scope_are_not_bypassable(self):
        for package in ('catalog', 'payload', 'local-content', '..'):
            with self.subTest(package=package), self.creation() as (root, owner, config, claim, request, *_):
                config['source_path'] = f'ToS/source-witnesses/relations/{package}/source-claims.jsonl'
                owner.write_text(json.dumps(config))
                with self.assertRaises(PermissionError):
                    commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
        with self.creation() as (root, owner, config, claim, request, *_):
            target = (root / config['source_path']).parent
            other = target.parent / 'unrelated'
            other.mkdir()
            target.symlink_to(other, target_is_directory=True)
            with self.assertRaises(OSError):
                commands.run_local_command(owner, request)
            self.assertEqual(list(other.iterdir()), [])

    def test_selected_source_and_schema_changes_conflict_after_preparation(self):
        with self.creation() as (root, owner, config, claim, request, *_):
            refs = ['ToS/contracts/source-relation-claim.schema.json', claim['evidence_refs'][0],
                    'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json']
            for ref in refs:
                path = root / ref
                original = path.read_bytes()
                payload = json.loads(original)
                key = 'description' if ref.endswith('.schema.json') else 'notes'
                payload[key] = str(payload.get(key, '')) + ' changed synthetic input'
                path.write_text(json.dumps(payload))
                with self.subTest(ref=ref), self.assertRaises(commands.JournalConflict):
                    commands.run_local_command(owner, request)
                self.assertFalse((root / config['source_path']).parent.exists())
                path.write_bytes(original)

    def test_concurrent_creation_and_occupied_paths_are_not_overwritten(self):
        with self.creation() as (root, owner, config, claim, request, *_):
            with ThreadPoolExecutor(max_workers=2) as pool:
                results = list(pool.map(lambda _: commands.run_local_command(owner, request), range(2)))
            self.assertEqual(sorted(result['replayed'] for result in results), [False, True])
            self.assertEqual(results[0]['receipt'], results[1]['receipt'])
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, {**request, 'command_id': 'different-command'})
        with self.creation() as (root, owner, config, claim, request, *_):
            target = (root / config['source_path']).parent
            target.mkdir()
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, request)
            self.assertTrue(target.exists())
            self.assertEqual(list(target.iterdir()), [])

    def test_abrupt_loss_and_response_loss_recover_without_deleting_other_staging(self):
        with self.creation() as (root, owner, config, claim, request, *_):
            code = ('import json, os, sys; sys.path.insert(0, sys.argv[1]); '
                    'from pathlib import Path; import source_commands as c; '
                    'c._publish_new_directory=lambda *args: os._exit(73); '
                    'c.run_local_command(Path(sys.argv[2]),json.load(sys.stdin))')
            stopped = subprocess.run([sys.executable, '-c', code, str(fixtures.MECHANIC), str(owner)],
                input=json.dumps(request), text=True, capture_output=True, timeout=30)
            self.assertEqual(stopped.returncode, 73, stopped.stderr)
            self.assertFalse((root / config['source_path']).parent.exists())
            orphan = list((root / 'ToS').glob('.source-claims-create-*.pending'))
            self.assertEqual(len(orphan), 1)
            old_bytes = {p.name: p.read_bytes() for p in orphan[0].iterdir()}
            original_publish = commands._publish_new_directory
            def lose_response(*args):
                original_publish(*args)
                raise RuntimeError('synthetic response loss after commit')
            with patch.object(commands, '_publish_new_directory', lose_response), self.assertRaises(RuntimeError):
                commands.run_local_command(owner, request)
            replay = commands.run_local_command(owner, request)
            self.assertTrue(replay['replayed'])
            self.assertEqual(old_bytes, {p.name: p.read_bytes() for p in orphan[0].iterdir()})

    def test_current_scope_and_dependency_changes_during_staging_refuse_commit(self):
        for revoke in (False, True):
            with self.subTest(revoke=revoke), self.creation() as (root, owner, config, claim, request, *_):
                original_publish = commands._publish
                changed = False
                def change_during_stage(path, raw):
                    nonlocal changed
                    original_publish(path, raw)
                    if not changed:
                        changed = True
                        if revoke:
                            config['allowed_operations'] = []
                            owner.write_text(json.dumps(config))
                        else:
                            schema = root / 'ToS/contracts/source-relation-claim.schema.json'
                            value = json.loads(schema.read_bytes())
                            value['description'] = 'changed during staged write'
                            schema.write_text(json.dumps(value))
                with patch.object(commands, '_publish', change_during_stage), self.assertRaises(commands.JournalConflict):
                    commands.run_local_command(owner, request)
                self.assertFalse((root / config['source_path']).parent.exists())

    def test_replay_rejects_source_or_receipt_corruption(self):
        with self.creation() as (root, owner, config, claim, request, *_):
            commands.run_local_command(owner, request)
            path = root / config['source_path']
            original = path.read_bytes()
            path.write_text(json.dumps({**claim, 'epistemic_status': 'reported'}))
            with self.assertRaises(commands.JournalCorruption):
                commands.run_local_command(owner, request)
            path.write_bytes(original)
            receipt_path = path.with_name('source-create-receipt.json')
            receipt = json.loads(receipt_path.read_bytes())
            receipt['grants_admission'] = True
            receipt_path.write_text(json.dumps(receipt))
            with self.assertRaises(commands.JournalCorruption):
                commands.run_local_command(owner, request)


if __name__ == '__main__':
    unittest.main()
