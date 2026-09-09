"""Publication-currentness boundaries over tiny synthetic source packages.

Transactions, catalog serialization, exact readers and catalog byte reads are
real. Profile inventories and unrelated graph schemas are deliberately absent:
this checks cooperative read admission, not philosophical/source acceptance.
"""
from contextlib import contextmanager
import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))
import build_source_witness_catalog as catalog
import claim_version_reader as claim_reader
import metadata_version_reader as metadata_reader
import source_commands as source
import source_metadata_snapshot as publication
import source_metadata_transactions as transactions
import source_witness_bibliographic_graph_common as graph
import tos_corpus_index_common as corpus


def encoded(value):
    return (catalog.canonical_json(value) + '\n').encode('utf-8')


class PublicationReaderTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='publication-readers-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.agent_ref = 'ToS/source-witnesses/agents/synthetic/agent.json'
        self.claim_ref = 'ToS/source-witnesses/relations/synthetic/source-claims.jsonl'
        self.observation_ref = 'ToS/source-witnesses/operations/synthetic/observation.json'
        self.agent = {'schema_version': 'tos_corpus_record_v1', 'record_type': 'agent',
            'record_id': 'tos.agent.synthetic.publication', 'record_version': 1,
            'preferred_label': 'Synthetic publication subject', 'identity_status': 'provisional',
            'source_refs': ['test:synthetic-only'], 'external_identifiers': [],
            'same_as_posture': 'no_equivalence_claim'}
        self.claim = {'schema_version': 'tos_synthetic_claim_v1',
            'claim_id': 'tos.claim.synthetic.publication', 'claim_type': 'relation',
            'claim_version': 1, 'visibility': 'public_metadata_only',
            'subject_ref': self.agent['record_id'], 'predicate': 'synthetic_proposal',
            'object': {'kind': 'uninterpreted', 'value': None},
            'qualifiers': {'statement': 'Synthetic only; no admitted assertion.'}}
        self.write(self.agent_ref, encoded(self.agent))
        self.write(self.claim_ref, encoded(self.claim))
        self.write(self.observation_ref, encoded({'synthetic_epoch': 0}))
        schema = 'ToS/contracts/corpus-record.schema.json'
        self.write(schema, (ROOT / schema).read_bytes())
        self.authority = {'command_id': 'test:publication-readers',
                          'authority_ref': 'test:synthetic-owner-only'}
        self.sequence = 0
        self.publish_catalog()

    def write(self, relative, raw):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)

    def profiles(self, *_args, **_kwargs):
        return SimpleNamespace(catalog_files={}, profiles={}, input_digests={})

    def rows(self, *, empty=False):
        records = {kind: [] for kind in catalog.RECORD_FILES}
        if empty:
            return records, []
        records['agent'].append({'schema_version': 'tos_source_witness_catalog_entry_v1',
            'record_id': self.agent['record_id'], 'record_type': 'agent',
            'preferred_label': self.agent['preferred_label'],
            'identity_status': self.agent['identity_status'], 'source_record_ref': self.agent_ref,
            'record_sha256': self.exact_ref(self.agent)['digest'][7:], 'links': {}})
        claims = [{'schema_version': 'tos_source_witness_claim_catalog_entry_v1',
            'claim_id': self.claim['claim_id'], 'claim_version': 1,
            'source_claim_file_ref': self.claim_ref, 'source_claim_line': 1,
            'claim_sha256': self.exact_ref(self.claim)['digest'][7:],
            'visibility': self.claim['visibility']}]
        return records, claims

    def render(self, *, empty=False):
        records, claims = self.rows(empty=empty)
        with (patch.object(catalog, 'SourceRecordProfiles', side_effect=self.profiles),
              patch.object(catalog, 'collect_records', return_value=records),
              patch.object(catalog, 'collect_claims', return_value=claims)):
            return catalog.render_outputs(self.root)

    def publish_catalog(self, *, empty=False):
        outputs = self.render(empty=empty)
        catalog.write_outputs(self.root, outputs)
        return outputs

    def exact_ref(self, record):
        claim = 'claim_id' in record
        return {'id': record['claim_id' if claim else 'record_id'],
                'version': record['claim_version' if claim else 'record_version'],
                'digest': publication._digest(source._canonical(record))}

    def reader_cases(self):
        return ((metadata_reader.MetadataVersionReader, self.agent),
                (claim_reader.ClaimVersionReader, self.claim))

    def assert_stale(self, result, reason='source-publication-changed'):
        self.assertEqual((result['status'], result['reason']), ('stale', reason), result)
        for field in ('record', 'record_digest', 'version_status', 'provenance'):
            if field in result:
                self.assertIsNone(result[field], result)
        if 'refs' in result:
            self.assertEqual(result['refs'], [])
            self.assertIsNone(result['current_ref'])
        for field in ('grants_current_use', 'performs_assessment', 'writes_to_source'):
            self.assertFalse(result[field], result)

    def transaction(self, *, files=None, new_directories=None, interrupt=False):
        self.sequence += 1
        plan = {'authorization': copy.deepcopy(self.authority), 'new_directories': new_directories or [],
                'files': files or [{'path': self.observation_ref,
                    'before': (self.root / self.observation_ref).read_bytes(),
                    'after': encoded({'synthetic_epoch': self.sequence})}]}
        identity = publication._digest(f'synthetic-reader-command-{self.sequence}'.encode())
        guard = lambda authorization, summary: authorization == self.authority
        with source._locked(self.root / 'ToS/source-witnesses/historical-create', allow_pending=True):
            if interrupt:
                with (patch.object(transactions, '_replace_file', side_effect=RuntimeError('synthetic interruption')),
                      self.assertRaisesRegex(RuntimeError, 'synthetic interruption')):
                    transactions.apply_transaction(self.root, plan,
                        expected_snapshot=publication.PublicationSnapshot(self.root),
                        authorization_guard=guard, transaction_id=identity)
            else:
                transactions.apply_transaction(self.root, plan,
                    expected_snapshot=publication.PublicationSnapshot(self.root),
                    authorization_guard=guard, transaction_id=identity)
        return identity

    def rollback(self, identity):
        with source._locked(self.root / 'ToS/source-witnesses/historical-create', allow_pending=True):
            return transactions.rollback_transaction(self.root, transaction_id=identity,
                authorization_guard=lambda authorization, summary: authorization == self.authority)

    @contextmanager
    def builders(self):
        """Empty real catalog inputs, without unrelated corpus/schema fixtures."""
        for ref in (graph.CLAIM_REGISTRY_REF, graph.CLAIM_CONTRACT_REF):
            self.write(ref, b'{}\n')
        with (patch.object(graph, 'SourceRecordProfiles', side_effect=self.profiles),
              patch.object(graph, 'load_claim_navigation_registry', return_value={}),
              patch.object(graph, '_scan_index', return_value={}),
              patch.object(graph, 'validate_payload_schema'),
              patch.object(corpus, 'REPO_ROOT', self.root),
              patch.object(corpus, 'TOS_ROOT', self.root / 'ToS'),
              patch.object(corpus, 'SourceRecordProfiles', side_effect=self.profiles)):
            yield

    def test_legacy_absent_control_keeps_unbound_catalog_and_exact_reads(self):
        self.assertIsNone(publication.PublicationSnapshot(self.root).token)
        manifest = json.loads((self.root / catalog.MANIFEST_PATH).read_bytes())
        self.assertNotIn('selected_metadata_publication', manifest)
        for factory, record in self.reader_cases():
            with self.subTest(reader=factory.__name__):
                result = factory(self.root).resolve(self.exact_ref(record))
                self.assertEqual(result['status'], 'available', result)
                self.assertEqual(result['record'], record)

    def test_bound_catalog_commits_manifest_last_and_exact_raw_file_hashes(self):
        self.transaction()
        outputs = self.render()
        manifest = json.loads(outputs[catalog.MANIFEST_PATH])
        binding = manifest['selected_metadata_publication']
        self.assertEqual(binding['token'], publication.PublicationSnapshot(self.root).token)
        self.assertEqual(binding['files'], {str(ref): hashlib.sha256(raw.encode()).hexdigest()
            for ref, raw in outputs.items() if ref != catalog.MANIFEST_PATH})
        replace, written = catalog.os.replace, []
        def observe(staging, path):
            replace(staging, path)
            written.append(path.relative_to(self.root))
        with patch.object(catalog.os, 'replace', side_effect=observe):
            catalog.write_outputs(self.root, outputs)
        self.assertEqual(written[-1], catalog.MANIFEST_PATH)
        for factory, record in self.reader_cases():
            self.assertEqual(factory(self.root).resolve(self.exact_ref(record))['status'], 'available')

    def test_pending_blocks_exact_readers_before_any_contract_catalog_or_source_read(self):
        self.transaction(interrupt=True)
        for factory, record in self.reader_cases():
            with self.subTest(reader=factory.__name__):
                instance = factory(self.root)
                with patch.object(instance._snapshot, 'read', side_effect=AssertionError('pending source read')):
                    self.assert_stale(instance.resolve(self.exact_ref(record)), 'source-publication-pending')
                    if isinstance(instance, metadata_reader.MetadataVersionReader):
                        # Non-native routing used to open profile contracts before checking pending.
                        with self.assertRaises(publication.PublicationPending):
                            instance.supports('expression')

    def guarded_entries(self):
        return ((catalog, 'collect_records', '_collect_records', lambda: catalog.collect_records(self.root)),
                (catalog, 'collect_claims', '_collect_claims', lambda: catalog.collect_claims(self.root)),
                (graph, 'graph', '_build_payload', lambda: graph.build_payload(self.root)),
                (corpus, 'corpus', '_build_payload', lambda: corpus.build_payload()),
                (corpus, 'navigation', '_build_source_navigation', lambda: corpus.build_source_navigation([])))

    def test_pending_blocks_builder_entry_before_inner_source_work(self):
        self.transaction(interrupt=True)
        with patch.object(corpus, 'REPO_ROOT', self.root):
            for module, name, inner, invoke in self.guarded_entries():
                with self.subTest(entry=name), patch.object(module, inner) as work:
                    with self.assertRaises(publication.PublicationPending):
                        invoke()
                    work.assert_not_called()
        with patch.object(catalog, 'SourceRecordProfiles') as profiles:
            with self.assertRaises(publication.PublicationPending):
                catalog.render_outputs(self.root)
            profiles.assert_not_called()

    def test_builder_entry_rechecks_original_epoch_before_return(self):
        self.transaction()
        with patch.object(corpus, 'REPO_ROOT', self.root):
            for module, name, inner, invoke in self.guarded_entries():
                def move_epoch(*_args, **_kwargs):
                    self.transaction()
                    return {'synthetic': True}
                with self.subTest(entry=name), patch.object(module, inner, side_effect=move_epoch):
                    with self.assertRaises(publication.PublicationChanged):
                        invoke()

    def test_render_rechecks_epoch_after_collecting_inputs(self):
        self.transaction()
        records, claims = self.rows()
        def move_epoch(*_args, **_kwargs):
            self.transaction()
            return claims
        with (patch.object(catalog, 'SourceRecordProfiles', side_effect=self.profiles),
              patch.object(catalog, 'collect_records', return_value=records),
              patch.object(catalog, 'collect_claims', side_effect=move_epoch),
              self.assertRaises(publication.PublicationChanged)):
            catalog.render_outputs(self.root)

    def test_writer_rejects_rendered_old_epoch_before_staging(self):
        self.transaction()
        outputs = self.render()
        self.transaction()
        with patch.object(catalog.tempfile, 'mkstemp') as stage:
            with self.assertRaises(publication.PublicationChanged):
                catalog.write_outputs(self.root, outputs)
            stage.assert_not_called()

    def test_writer_final_fence_catches_epoch_change_after_manifest_replace(self):
        self.transaction()
        outputs = self.render()
        replace = catalog.os.replace
        def move_after_manifest(staging, path, **kwargs):
            replace(staging, path, **kwargs)
            if path == self.root / catalog.MANIFEST_PATH:
                self.transaction()
        with patch.object(catalog.os, 'replace', side_effect=move_after_manifest):
            with self.assertRaises(publication.PublicationChanged):
                catalog.write_outputs(self.root, outputs)

    def test_cached_exact_readers_do_not_survive_commit_or_rollback_aba(self):
        self.transaction()
        self.publish_catalog()
        for rollback in (False, True):
            with self.subTest(rollback=rollback):
                instances = [(factory(self.root), record) for factory, record in self.reader_cases()]
                for instance, record in instances:
                    self.assertEqual(instance.resolve(self.exact_ref(record))['status'], 'available')
                old = publication.PublicationSnapshot(self.root).token
                if rollback:
                    raw = (self.root / self.observation_ref).read_bytes()
                    self.rollback(self.transaction(interrupt=True))
                    self.assertEqual((self.root / self.observation_ref).read_bytes(), raw)
                else:
                    self.transaction()
                self.assertNotEqual(publication.PublicationSnapshot(self.root).token, old)
                self.publish_catalog()
                for instance, record in instances:
                    self.assert_stale(instance.resolve(self.exact_ref(record)))
                    self.assertEqual(type(instance)(self.root).resolve(self.exact_ref(record))['status'], 'available')

    def test_exact_readers_fail_closed_when_transaction_completes_during_source_read(self):
        self.transaction()
        for factory, record in self.reader_cases():
            self.publish_catalog()
            instance = factory(self.root)
            method = '_validate_current' if factory is metadata_reader.MetadataVersionReader else '_package'
            original = getattr(instance, method)
            def move_epoch(*args, **kwargs):
                result = original(*args, **kwargs)
                self.transaction()
                return result
            with self.subTest(reader=factory.__name__), patch.object(instance, method, side_effect=move_epoch):
                self.assert_stale(instance.resolve(self.exact_ref(record)))

    def test_old_catalog_after_new_publication_cannot_report_new_subject_missing(self):
        self.transaction()
        self.publish_catalog()
        new_agent = {**self.agent, 'record_id': 'tos.agent.synthetic.new'}
        new_claim = {**self.claim, 'claim_id': 'tos.claim.synthetic.new'}
        new_ref = 'ToS/source-witnesses/agents/second/agent.json'
        self.transaction(files=[{'path': new_ref, 'before': None, 'after': encoded(new_agent)},
            {'path': self.claim_ref, 'before': (self.root / self.claim_ref).read_bytes(),
             'after': encoded(self.claim) + encoded(new_claim)}],
            new_directories=['ToS/source-witnesses/agents/second'])
        for factory, record in ((metadata_reader.MetadataVersionReader, new_agent),
                                (claim_reader.ClaimVersionReader, new_claim)):
            with self.subTest(reader=factory.__name__):
                self.assert_stale(factory(self.root).resolve(self.exact_ref(record)))

    def test_exact_reader_unavailable_result_also_rechecks_original_epoch(self):
        self.transaction()
        cases = [(factory, record, 'resolve') for factory, record in self.reader_cases()]
        cases.extend((metadata_reader.MetadataVersionReader, self.agent, method)
                     for method in ('exact_refs', 'resolve_source_bytes'))
        for factory, record, method in cases:
            self.publish_catalog()
            instance = factory(self.root)
            read, changed = instance._snapshot.read, False
            def move_after_catalog_read(path, *args, **kwargs):
                nonlocal changed
                raw = read(path, *args, **kwargs)
                if not changed and path.parent == self.root / catalog.CATALOG_ROOT:
                    changed = True
                    self.transaction()
                return raw
            missing = {**self.exact_ref(record), 'id': self.exact_ref(record)['id'] + '.absent'}
            arguments = ((missing['id'],) if method == 'exact_refs' else
                         (self.agent_ref.replace('/synthetic/', '/absent/'), 'a' * 64)
                         if method == 'resolve_source_bytes' else (missing,))
            with self.subTest(reader=factory.__name__, method=method):
                with patch.object(instance._snapshot, 'read', side_effect=move_after_catalog_read):
                    self.assert_stale(getattr(instance, method)(*arguments))

    def test_partial_catalog_write_with_old_manifest_fails_exact_raw_file_hash(self):
        self.transaction()
        for factory, record, ref in (
                (metadata_reader.MetadataVersionReader, self.agent, catalog.CATALOG_ROOT / 'agents.jsonl'),
                (claim_reader.ClaimVersionReader, self.claim, catalog.CLAIM_CATALOG_PATH)):
            outputs = self.publish_catalog()
            old_manifest = (self.root / catalog.MANIFEST_PATH).read_bytes()
            # Same JSON record, different exact bytes: source checks alone cannot catch this tear.
            outputs[ref] = ' ' + outputs[ref]
            manifest = json.loads(outputs[catalog.MANIFEST_PATH])
            manifest['selected_metadata_publication']['files'][str(ref)] = hashlib.sha256(outputs[ref].encode()).hexdigest()
            outputs[catalog.MANIFEST_PATH] = json.dumps(manifest) + '\n'
            replace = catalog.os.replace
            def stop_before_manifest(staging, path):
                if path == self.root / catalog.MANIFEST_PATH:
                    raise RuntimeError('synthetic partial catalog publication')
                replace(staging, path)
            with self.subTest(reader=factory.__name__):
                with patch.object(catalog.os, 'replace', side_effect=stop_before_manifest):
                    with self.assertRaisesRegex(RuntimeError, 'synthetic partial'):
                        catalog.write_outputs(self.root, outputs)
                self.assertEqual((self.root / catalog.MANIFEST_PATH).read_bytes(), old_manifest)
                self.assert_stale(factory(self.root).resolve(self.exact_ref(record)))

    def test_bound_catalog_is_not_legacy_if_publication_control_disappears(self):
        self.transaction()
        self.publish_catalog()
        (self.root / publication.CONTROL_REF).unlink()
        for factory, record in self.reader_cases():
            with self.subTest(reader=factory.__name__):
                self.assert_stale(factory(self.root).resolve(self.exact_ref(record)))

    def test_projections_reject_old_catalog_even_when_all_rows_are_empty(self):
        self.transaction()
        self.publish_catalog(empty=True)
        self.transaction()
        with self.builders():
            for name, invoke in (('graph', lambda: graph.build_payload(self.root)),
                                 ('navigation', lambda: corpus.build_source_navigation([]))):
                with self.subTest(projection=name), self.assertRaises(publication.PublicationChanged):
                    invoke()

    def test_initialized_navigation_requires_a_catalog_publication_manifest(self):
        self.transaction()
        self.publish_catalog(empty=True)
        (self.root / catalog.MANIFEST_PATH).unlink()
        with self.builders(), self.assertRaises(publication.PublicationChanged):
            corpus.build_source_navigation([])

    def test_projection_catalog_hashes_cover_actual_consumption_not_only_preflight(self):
        self.transaction()
        with self.builders():
            for module, name, loader, invoke in (
                    (graph, 'graph', '_load_object_catalog', lambda: graph.build_payload(self.root)),
                    (corpus, 'navigation', '_jsonl', lambda: corpus.build_source_navigation([]))):
                for ref in (catalog.MANIFEST_PATH, catalog.CATALOG_ROOT / 'agents.jsonl'):
                    self.publish_catalog(empty=True)
                    original = getattr(module, loader)
                    changed = False
                    def alter_before_consumption(*args, **kwargs):
                        nonlocal changed
                        if not changed:
                            changed = True
                            path = self.root / ref
                            path.write_bytes(path.read_bytes() + b'\n')
                        return original(*args, **kwargs)
                    with self.subTest(projection=name, changed=str(ref)):
                        with patch.object(module, loader, side_effect=alter_before_consumption):
                            with self.assertRaises(publication.PublicationChanged):
                                invoke()

    def test_empty_legacy_and_bound_projection_catalogs_remain_readable(self):
        with self.builders():
            for initialized in (False, True):
                if initialized:
                    self.transaction()
                self.publish_catalog(empty=True)
                with self.subTest(initialized=initialized):
                    self.assertEqual(graph.build_payload(self.root)['nodes'], [])
                    self.assertEqual(corpus.build_source_navigation([])['nodes'], [])

    def test_common_corpus_lock_denies_legacy_writer_body_until_recovery(self):
        target = self.root / 'ToS/source-witnesses/historical-create'
        identity = self.transaction(interrupt=True)
        with self.assertRaises(publication.PublicationPending):
            with source._locked(target):
                self.fail('pending must not admit the legacy critical section')
        with source._locked(target, allow_pending=True):
            self.assertEqual(publication.read_publication_state(self.root)['phase'], 'pending')
        self.rollback(identity)
        with source._locked(target):
            self.assertEqual(publication.read_publication_state(self.root)['phase'], 'ready')


if __name__ == '__main__':
    unittest.main()
