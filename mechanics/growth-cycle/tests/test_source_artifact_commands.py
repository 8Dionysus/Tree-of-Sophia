"""Synthetic Artifact metadata creation; no acquisition or witness assessment."""
import copy
from contextlib import contextmanager
import json
from pathlib import Path
import unittest
from unittest.mock import patch

import test_source_commands as fixtures
from test_source_native_metadata_commands import SAMPLES
import source_commands as source
import source_artifact_commands as artifact
import source_native_metadata_commands as native
import source_revisions as revisions

ROOT = fixtures.ROOT


class ArtifactCreationTests(unittest.TestCase):
    @contextmanager
    def fixture(self):
        owner_fixture = fixtures.HistoricalCreationTests()
        with owner_fixture.creation() as (root, owner, original, _, rebuild, graph_fixture):
            sample = json.loads((ROOT / SAMPLES['artifact']).read_bytes())
            for ref in (artifact.SCHEMA_REF, *artifact.INPUT_SCHEMAS.values(),
                        'ToS/contracts/provenance-event-v2.schema.json'):
                (root / ref).write_bytes((ROOT / ref).read_bytes())
            record = copy.deepcopy(sample)
            record.update({'$schema': 'https://tree-of-sophia.local/' + artifact.SCHEMA_REF,
                           'schema_version': artifact.SCHEMA_VERSION})
            record.update(artifact_id='tos.artifact.synthetic-created', record_version=1,
                provenance_event_ref='tos.event.synthetic-artifact-created', philosophy_planting_refs=[],
                created_at='2026-09-09T12:00:00Z',
                maker={'maker_type': original['maker_type'], 'agent_ref': original['principal_id'],
                       'human_review_performed': False})
            record['authority']['review_status'] = 'unreviewed'
            record['custody']['inventory_numbers'] = ['SYNTHETIC-CREATION-ONLY']
            record['path_identity']['note'] = 'Synthetic physical identity, no historical assessment.'
            rights = json.loads((ROOT / sample['rights_ref']).read_bytes())
            rights.update(rights_id='tos.rights.synthetic-created-artifact', scope_refs=[record['artifact_id']])
            discovery = json.loads((ROOT / sample['discovery_ref']).read_bytes())
            discovery['target']['known_tos_refs'] = [record['artifact_id']]
            paths = {'rights_ref': 'ToS/source-witnesses/rights/synthetic-artifact.json',
                'discovery_ref': 'ToS/source-witnesses/discovery/runs/synthetic-artifact.json',
                'research_ref': 'ToS/research-packets/synthetic-artifact.md'}
            values = {'rights_ref': revisions._encode(rights), 'discovery_ref': revisions._encode(discovery),
                      'research_ref': b'Synthetic test research input; not a real rights or discovery decision.\n'}
            bindings = {}
            for field, ref in paths.items():
                (root / ref).parent.mkdir(parents=True, exist_ok=True)
                (root / ref).write_bytes(values[field])
                record[field] = ref
                bindings[field] = {'ref': ref, 'sha256': source._digest(values[field])[7:]}
            relative = 'ToS/source-witnesses/artifacts/synthetic/uncertain/new-native-artifact/artifact-witness.json'
            (root / relative).parent.parent.mkdir(parents=True, exist_ok=True)
            config = {key: original[key] for key in
                      ('uid', 'principal_id', 'maker_type', 'source_root', 'authority_ref', 'expires_at')}
            config.update(schema_version=artifact.CONFIG, source_path=relative, record_id=record['artifact_id'],
                provenance_event_id=record['provenance_event_ref'], allowed_operations=['source.create'],
                allowed_form_ids=['tos.form.synthetic-artifact-name', 'tos.form.synthetic-artifact-note'],
                source_bindings=bindings)
            owner.write_bytes(revisions._encode(config))
            proposal = {'record': record, 'source_bindings': copy.deepcopy(bindings),
                'forms': [{'form_id': config['allowed_form_ids'][0], 'field_id': 'metadata.preferred-name'},
                          {'form_id': config['allowed_form_ids'][1], 'field_id': 'metadata.source-note'}]}
            yield root, owner, config, proposal, rebuild, graph_fixture

    def create(self, owner, proposal):
        preview = source.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                                                   'operation': 'prepare-create', **proposal})
        request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'source.create',
            'command_id': 'synthetic:artifact-create', 'expected_configuration': preview['owner_configuration'],
            'expected_source': None, 'expected_revision': None,
            'expected_dependencies': preview['expected_dependencies'], **proposal}
        return source.run_local_command(owner, request), request

    def test_native_shape_and_three_input_bindings_do_not_accept_authority(self):
        with self.fixture() as (root, _, config, proposal, *_):
            artifact.configuration(config)
            record = proposal['record']
            self.assertEqual(artifact.initial_record(config, record).id, config['record_id'])
            artifact._read_inputs(root, record, proposal['source_bindings'])
            for change in ({'artifact_id': 'tos.artifact.other'}, {'record_id': config['record_id']},
                           {'philosophy_planting_refs': ['ToS/philosophy/fabricated.md']},
                           {'maker': {**record['maker'], 'human_review_performed': True}},
                           {'authority': {**record['authority'], 'review_status': 'human_reviewed'}},
                           {'schema_version': 'tos_artifact_source_witness_v1'}):
                with self.subTest(change=change), self.assertRaises((PermissionError, ValueError)):
                    artifact.initial_record(config, {**record, **change})
            wrong = copy.deepcopy(proposal['source_bindings'])
            wrong['rights_ref']['sha256'] = '0' * 64
            with self.assertRaises(source.JournalConflict):
                artifact._read_inputs(root, record, wrong)
            for ref in ('/tmp/private.json', 'ToS/source-witnesses/.owner-local/x.json',
                        'ToS/source-witnesses/artifacts/x/payload/text.json', 'ToS/source-witnesses/catalog/artifacts.jsonl'):
                wrong = copy.deepcopy(config)
                wrong['source_bindings']['rights_ref']['ref'] = ref
                with self.subTest(ref=ref), self.assertRaises(PermissionError):
                    artifact.configuration(wrong)

    def test_shared_creation_replay_catalog_and_portable_form_keep_native_identity(self):
        from metadata_version_reader import MetadataVersionReader
        with self.fixture() as (root, owner, config, proposal, rebuild, fixture):
            before = {field: (root / binding['ref']).read_bytes() for field, binding in config['source_bindings'].items()}
            result, request = self.create(owner, proposal)
            self.assertFalse(result['grants_admission'])
            self.assertTrue(source.run_local_command(owner, request)['replayed'])
            path = root / config['source_path']
            with patch.object(source, '_configuration', side_effect=AssertionError('evidence reader must not load a grant')):
                origin = artifact.verify_creation(root, config['source_path'], proposal['record'])
            self.assertEqual(origin['source'], result['receipt']['source'])
            self.assertEqual(origin['source_bindings'], request['source_bindings'])
            self.assertFalse(origin['grants_admission'])
            outputs = origin['event']['entities']['outputs']
            self.assertEqual({entry['entity_ref'] for entry in outputs}, {config['source_path'],
                path.with_name('artifact-witness.human-forms.json').relative_to(root).as_posix()})
            projection = rebuild()
            graph, _, _ = fixture.historical_knowledge(root, projection)
            node = next(node for node in graph['nodes'] if node.get('entity_id') == config['record_id'])
            self.assertEqual(node['type_id'], 'tos.entity.artifact')
            self.assertEqual(node['attributes']['source_record']['artifact_id'], config['record_id'])
            self.assertNotIn('record_id', node['attributes']['source_record'])
            typed = MetadataVersionReader(root).resolve_typed(result['receipt']['source'])
            self.assertEqual(typed['status'], 'available', typed)
            self.assertEqual(typed['descriptor']['identity_field'], 'artifact_id')
            forms = json.loads(path.with_name('artifact-witness.human-forms.json').read_bytes())
            views = source.materialize_metadata_forms(proposal['record'], forms, access_allowed=True)
            self.assertTrue(all(view['state'] == 'ready' and view['admission'] is None for view in views))
            self.assertTrue(all((root / config['source_bindings'][field]['ref']).read_bytes() == raw for field, raw in before.items()))

    def test_creation_origin_survives_selected_correction_and_ignores_descendants(self):
        with self.fixture() as (root, owner, config, proposal, *_):
            created, request = self.create(owner, proposal)
            path = root / config['source_path']
            descendants = path.parent / 'representations/private/payload'
            descendants.mkdir(parents=True)
            (descendants / 'untouched.bin').write_bytes(b'No inspection or acquisition.')
            revision = {key: config[key] for key in ('uid', 'principal_id', 'source_root', 'source_path',
                                                    'authority_ref', 'expires_at', 'record_id', 'allowed_form_ids')}
            revision.update(schema_version=native.CONFIG, record_type='artifact', record_schema_version=artifact.SCHEMA_VERSION,
                allowed_operations=['record.revise', 'record.recover'], allowed_fields=['path_identity'])
            owner.write_bytes(revisions._encode(revision))
            change = {'fields': {'path_identity': {**proposal['record']['path_identity'], 'note': 'Corrected synthetic description.'}},
                      'forms': proposal['forms'], 'reason': 'Synthetic descriptive correction only.'}
            preview = source.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                                                       'operation': 'prepare-revise', **change})
            correction = {'schema_version': 'tos_local_source_command_v1', 'operation': 'record.revise',
                'command_id': 'synthetic:artifact-correction', 'expected_configuration': preview['owner_configuration'],
                'expected_source': preview['source'], 'expected_revision': preview['revision'],
                'expected_dependencies': preview['expected_dependencies'], 'expected_publication': preview['expected_publication'], **change}
            source.run_local_command(owner, correction)
            current = json.loads(path.read_bytes())
            read, listing = source._read, Path.iterdir
            def bounded_read(selected, limit):
                self.assertFalse(selected.is_relative_to(descendants))
                return read(selected, limit)
            def bounded_listing(selected):
                self.assertNotEqual(selected, path.parent)
                return listing(selected)
            with patch.object(source, '_read', side_effect=bounded_read), patch.object(Path, 'iterdir', bounded_listing):
                origin = artifact.verify_creation(root, config['source_path'], current)
                owner.write_bytes(revisions._encode(config))
                self.assertTrue(source.run_local_command(owner, request)['replayed'])
            self.assertEqual(origin['source'], created['receipt']['source'])
            self.assertEqual(origin['current_source']['version'], 2)
            self.assertEqual((descendants / 'untouched.bin').read_bytes(), b'No inspection or acquisition.')

    def test_stale_or_substituted_inputs_cannot_publish_or_rebind_old_origin(self):
        with self.fixture() as (root, owner, config, proposal, *_):
            prepared = source.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                                                        'operation': 'prepare-create', **proposal})
            request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'source.create',
                'command_id': 'synthetic:stale-artifact', 'expected_configuration': prepared['owner_configuration'],
                'expected_source': None, 'expected_revision': None, 'expected_dependencies': prepared['expected_dependencies'], **proposal}
            research = root / proposal['record']['research_ref']
            original = research.read_bytes()
            research.write_bytes(original + b'Changed exact input.\n')
            with self.assertRaises(source.JournalConflict):
                source.run_local_command(owner, request)
            self.assertFalse((root / config['source_path']).parent.exists())
            research.write_bytes(original)
            _, created_request = self.create(owner, proposal)
            research.write_bytes(original + b'Later independent source version.\n')
            with self.assertRaises(source.JournalConflict):
                artifact.verify_creation(root, config['source_path'], proposal['record'])
            with self.assertRaises(source.JournalConflict):
                source.run_local_command(owner, created_request)
            wrong = copy.deepcopy(proposal)
            wrong['source_bindings']['research_ref']['sha256'] = source._digest(research.read_bytes())[7:]
            research.write_bytes(original)
            with self.assertRaises(PermissionError):
                source.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                                                 'operation': 'prepare-create', **wrong})

    def test_old_corpus_grant_and_occupied_directory_cannot_create_artifact(self):
        with self.fixture() as (root, owner, config, proposal, *_):
            old = {key: value for key, value in config.items() if key != 'source_bindings'}
            old.update(schema_version=source.CORPUS_COLLECTION_CONFIG, record_type='artifact')
            owner.write_bytes(revisions._encode(old))
            with self.assertRaises((PermissionError, ValueError)):
                source.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
            owner.write_bytes(revisions._encode(config))
            target = (root / config['source_path']).parent
            target.mkdir()
            other = target / 'unrelated.txt'
            other.write_bytes(b'Unrelated existing user data.')
            with self.assertRaises(source.JournalConflict):
                self.create(owner, proposal)
            self.assertEqual(other.read_bytes(), b'Unrelated existing user data.')
            self.assertFalse((root / config['source_path']).exists())

    def test_shared_atomic_publication_has_no_partial_target_and_retains_exact_retry(self):
        for after_commit in (False, True):
            with self.subTest(after_commit=after_commit), self.fixture() as (root, owner, config, proposal, *_):
                prepared = source.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                                                            'operation': 'prepare-create', **proposal})
                request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'source.create',
                    'command_id': 'synthetic:artifact-publication', 'expected_configuration': prepared['owner_configuration'],
                    'expected_source': None, 'expected_revision': None,
                    'expected_dependencies': prepared['expected_dependencies'], **proposal}
                publish = source._publish_new_directory
                def interrupted(staging, target):
                    if after_commit:
                        publish(staging, target)
                    raise RuntimeError('Synthetic interruption at the shared atomic boundary.')
                with patch.object(source, '_publish_new_directory', side_effect=interrupted):
                    with self.assertRaises(RuntimeError):
                        source.run_local_command(owner, request)
                self.assertEqual((root / config['source_path']).exists(), after_commit)
                result = source.run_local_command(owner, request)
                self.assertEqual(result['replayed'], after_commit)
                self.assertEqual(artifact.verify_creation(root, config['source_path'], proposal['record'])['source'],
                                 result['receipt']['source'])

    def test_origin_refuses_partial_capture_and_forged_research_output_even_with_rehashed_receipt(self):
        with self.fixture() as (root, owner, config, proposal, *_):
            self.create(owner, proposal)
            base = (root / config['source_path']).parent
            event_path = base / 'source-create-provenance.jsonl'
            receipt_path = base / 'source-create-receipt.json'
            event = json.loads(event_path.read_bytes())
            event['entities']['outputs'][0]['entity_ref'] = proposal['record']['research_ref']
            raw = source._canonical(event) + b'\n'
            event_path.write_bytes(raw)
            receipt = json.loads(receipt_path.read_bytes())
            receipt['files'][event_path.name] = {'sha256': source._digest(raw), 'bytes': len(raw)}
            receipt_path.write_bytes(revisions._encode(receipt))
            with self.assertRaises(source.JournalCorruption):
                artifact.verify_creation(root, config['source_path'], proposal['record'])
            event_path.unlink()
            with self.assertRaises(source.JournalCorruption):
                artifact.verify_creation(root, config['source_path'], proposal['record'])
