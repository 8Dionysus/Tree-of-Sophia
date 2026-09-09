"""Pure metadata version reads over synthetic records and the real writer."""
import copy
from contextlib import contextmanager
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
MECHANIC = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
sys.path.insert(0, str(MECHANIC))
import metadata_version_reader as reader
import source_commands as source
import source_revisions as revisions
from source_record_profiles import SourceRecordProfiles
import test_source_revisions as fixtures


class MetadataVersionReaderTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.NativeSourceRevisionTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root, self.path = self.fixture.root, self.fixture.path
        self.record = copy.deepcopy(self.fixture.record)
        self.sync_catalog(self.fixture)

    def sync_catalog(self, fixture):
        record = json.loads(fixture.path.read_bytes())
        kind = record['record_type']
        if kind in reader.NATIVE_CATALOGS:
            filename, schema = reader.NATIVE_CATALOGS[kind], {}
        else:
            profiles = SourceRecordProfiles(fixture.root)
            filename = profiles.profiles[kind]['catalog_filename']
            schema = {'source_schema_ref': profiles.schema_routes[kind, record['schema_version']]['schema_ref']}
        catalog = fixture.root / reader.CATALOG_ROOT / filename
        catalog.parent.mkdir(parents=True, exist_ok=True)
        entry = {'schema_version': 'tos_source_witness_catalog_entry_v1',
                 'record_id': record['record_id'], 'record_type': kind,
                 'preferred_label': record['preferred_label'], 'identity_status': record['identity_status'],
                 'source_record_ref': fixture.relative, 'record_sha256': reader._record_ref(record)['digest'][7:],
                 'links': {}, **schema}
        catalog.write_bytes(source._canonical(entry) + b'\n')
        fixture.catalog = catalog

    def correct(self, fixture=None, label='Synthetic successor, no new admission.'):
        fixture = fixture or self.fixture
        record = json.loads(fixture.path.read_bytes())
        proposal = {'fields': {'notes': label, 'field_languages': {**record.get('field_languages', {}),
                          'notes': {'language': 'ru', 'script': 'Cyrl'}}},
                    'forms': fixture.selections, 'reason': 'Synthetic exact-version reader correction.'}
        prepared = fixture.run_command('prepare-revise', **proposal)
        request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'record.revise',
                   'command_id': 'test:metadata-correction-' + str(record['record_version']),
                   'expected_source': prepared['source'], 'expected_revision': prepared['revision'],
                   'expected_configuration': prepared['owner_configuration'],
                   'expected_dependencies': prepared['expected_dependencies'], **proposal}
        result = source.run_local_command(fixture.owner, request)
        self.sync_catalog(fixture)
        return result['receipt']

    def resolve(self, exact_ref=None, *, instance=None):
        return (instance or reader.MetadataVersionReader(self.root)).resolve(exact_ref or reader._record_ref(self.record))

    def assert_unavailable(self, result, status, reason=None):
        self.assertEqual(result['status'], status, result)
        if reason:
            self.assertEqual(result['reason'], reason, result)
        for field in ('record', 'record_digest', 'version_status', 'provenance'):
            if field in result:
                self.assertIsNone(result[field])
        if 'refs' in result:
            self.assertEqual(result['refs'], [])
            self.assertIsNone(result['current_ref'])
        for field in ('grants_current_use', 'performs_assessment', 'writes_to_source'):
            self.assertFalse(result[field])

    @contextmanager
    def family(self, kind):
        fixture = (fixtures.NativeSourceRevisionTests() if kind in reader.NATIVE_CATALOGS else
                   fixtures.ProfileSourceRevisionTests() if kind in {'letter', 'lexeme'} else fixtures.SourceRevisionTests())
        fixture.setUp()
        try:
            oldpath, oldforms = fixture.path, fixture.formpath
            fixture.relative = str(Path(fixture.relative).with_name(kind + '.json'))
            fixture.path = fixture.root / fixture.relative
            fixture.formpath = fixture.path.with_name(kind + '.human-forms.json')
            fixture.record.update(record_type=kind, record_id='tos.' + kind + '.revision-fixture')
            if kind == 'work':
                fixture.record['expression_claim_refs'] = []
            if kind == 'lexeme':
                for name in ('semantic-description-record', 'lexical-description-record'):
                    ref = 'ToS/contracts/' + name + '.schema.json'
                    (fixture.root / ref).write_bytes((ROOT / ref).read_bytes())
                fixture.record.update(schema_version='tos_lexical_description_record_v1',
                    field_languages={'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                                     'notes': {'language': 'ru', 'script': 'Cyrl'}},
                    semantic_scope={'scope_note': 'Only this synthetic reader test.',
                        'identity_criterion': 'A synthetic lexical referent, not its spelling.',
                        'language': 'en', 'script': 'Latn'},
                    semantic_content={'lexical_account': 'A synthetic lexical account.',
                        'grammatical_account': 'No observed linguistic evidence.', 'language': 'en', 'script': 'Latn',
                        'uninterpreted': [None, False, 0, '', {'qualification': 'not an instruction'}]})
                fixture.config['profile_type_id'] = 'tos.entity.lexeme'
            fixture.path.write_bytes(revisions._encode(fixture.record))
            changes = [source.prepare_metadata_change(fixture.record, None, 'test:synthetic-author', **selection)
                       for selection in fixture.selections]
            forms = source._apply(None, source.Record.from_payload(fixture.record['record_id'], 1, fixture.record), changes)
            fixture.formpath.write_bytes(revisions._encode(forms))
            if oldpath != fixture.path:
                oldpath.unlink()
                oldforms.unlink()
            fixture.config.update(source_path=fixture.relative, record_id=fixture.record['record_id'])
            if kind in reader.NATIVE_CATALOGS:
                fixture.config['record_type'] = kind
            fixture.owner.write_bytes(revisions._encode(fixture.config))
            self.sync_catalog(fixture)
            yield fixture
        finally:
            fixture.doCleanups()

    def test_current_native_record_is_lossless_without_commands_private_readers_or_companions(self):
        companion = self.path.parent / 'unrecognized.json'
        companion.write_bytes(b'PRIVATE UNKNOWN COMPANION MUST NOT BE READ')
        snapshot = {path: path.read_bytes() for path in self.root.rglob('*') if path.is_file()}
        original_read = source._read
        opened = []
        def selected_only(path, limit):
            opened.append(path)
            self.assertNotEqual(path, companion)
            return original_read(path, limit)
        with (patch.object(source, '_read', side_effect=selected_only),
              patch.object(source, '_configuration', side_effect=AssertionError('no current command authority')),
              patch.object(source, 'run_local_command', side_effect=AssertionError('no command entrypoint')),
              patch.object(SourceRecordProfiles, 'validate', side_effect=AssertionError('no semantic/private validation')),
              patch.object(SourceRecordProfiles, 'native_semantic_identities', side_effect=AssertionError('no private inventory'))):
            result = self.resolve()
        self.assertEqual(result['status'], 'available', result)
        self.assertEqual(result['version_status'], 'current')
        self.assertEqual(result['record'], self.record)
        self.assertEqual(result['record_digest'], reader._record_ref(self.record)['digest'])
        self.assertEqual(result['provenance']['source']['record_sha256'], source._digest(self.path.read_bytes()))
        self.assertNotEqual(result['record_digest'], result['provenance']['source']['record_sha256'])
        self.assertEqual(result['provenance']['verification_scope'], 'selected-record-chain')
        self.assertFalse(result['provenance']['all_package_bytes_verified'])
        self.assertNotIn('materializations', result)
        self.assertNotIn('PRIVATE UNKNOWN', json.dumps(result))
        self.assertEqual(snapshot, {path: path.read_bytes() for path in self.root.rglob('*') if path.is_file()})

    def test_native_and_declared_families_use_the_existing_revision_writer_and_exact_catalogs(self):
        for kind in ('agent', 'place', 'organization', 'work', 'historical-event', 'historical-process', 'historical-state', 'letter', 'lexeme'):
            with self.subTest(kind=kind), self.family(kind) as fixture:
                previous = copy.deepcopy(fixture.record)
                receipt = self.correct(fixture)
                current = json.loads(fixture.path.read_bytes())
                instance = reader.MetadataVersionReader(fixture.root)
                with (patch.object(source, '_configuration', side_effect=AssertionError('no authority lookup')),
                      patch.object(source, 'run_local_command', side_effect=AssertionError('no command lookup')),
                      patch.object(SourceRecordProfiles, 'validate', side_effect=AssertionError('no private inventory route')),
                      patch.object(SourceRecordProfiles, 'validate_native_binding', side_effect=AssertionError('no native binding resolution'))):
                    self.assertTrue(instance.supports(kind, source_ref=fixture.relative))
                    refs = instance.exact_refs(previous['record_id'])
                    old = instance.resolve(reader._record_ref(previous))
                    new = instance.resolve(reader._record_ref(current))
                self.assertEqual(refs['status'], 'available', refs)
                self.assertEqual(refs['refs'], [reader._record_ref(previous), reader._record_ref(current)])
                self.assertEqual(refs['current_ref'], reader._record_ref(current))
                self.assertEqual(old['record'], previous, old)
                self.assertEqual(new['record'], current, new)
                self.assertEqual(old['version_status'], 'historical')
                self.assertEqual(new['version_status'], 'current')
                self.assertEqual(old['provenance']['transition']['previous_source'], receipt['previous_source'])
                self.assertNotIn('request', old['provenance']['transition'])
                self.assertEqual(old['provenance']['catalog']['current_record_ref'], reader._record_ref(current))
                instance.verify_current()

    def test_edition_read_route_does_not_invent_earlier_versions_or_open_old_writers(self):
        with self.family('edition') as fixture:
            fixture.record.update(record_version=4,
                embodies_expression_refs=['tos.expression.synthetic.first', 'tos.expression.synthetic.second'],
                publication_claim_refs=[], exemplar_claim_refs=[])
            fixture.path.write_bytes(revisions._encode(fixture.record))
            self.sync_catalog(fixture)
            before = fixture.path.read_bytes()
            instance = reader.MetadataVersionReader(fixture.root)
            exact = reader._record_ref(fixture.record)
            refs = instance.exact_refs(fixture.record['record_id'])
            self.assertEqual(refs['status'], 'available', refs)
            self.assertEqual(refs['refs'], [exact])
            self.assertEqual(instance.resolve(exact)['record'], fixture.record)
            self.assert_unavailable(instance.resolve({**exact, 'version': 1}), 'missing', 'exact-version-not-retained')
            for schema in (source.CORPUS_CONFIG, source.CORPUS_REVISION_CONFIG, source.CORPUS_SELECTED_REVISION_CONFIG):
                with self.subTest(owner_schema=schema), self.assertRaises(PermissionError):
                    source._configured_corpus_profile({'schema_version': schema, 'record_type': 'edition'})
            self.assertEqual(fixture.path.read_bytes(), before)
            instance.verify_current()

    def project_fixture(self, fixture):
        """Use the public builder and portable access path, not a history mock."""
        sys.path.insert(0, str(ROOT / 'scripts'))
        sys.path.insert(0, str(ROOT / 'access/src'))
        import tos_corpus_index_common as corpus
        from tos_access.knowledge import build_knowledge_graph

        self.sync_catalog(fixture)
        record = json.loads(fixture.path.read_bytes())
        manifest = fixture.catalog.with_name('catalog.manifest.json')
        manifest.write_bytes(source._canonical({'record_files': {
            record['record_type']: fixture.catalog.relative_to(fixture.root).as_posix()}}))
        before = {path: path.read_bytes() for path in fixture.root.rglob('*') if path.is_file()}
        diagnostics = []
        with (patch.object(corpus, 'REPO_ROOT', fixture.root),
              patch.object(corpus, 'TOS_ROOT', fixture.root / 'ToS'),
              patch.object(source, '_configuration', side_effect=AssertionError('no current command authority')),
              patch.object(source, 'run_local_command', side_effect=AssertionError('no writer while projecting'))):
            navigation = corpus.build_source_navigation(diagnostics)
        graph = build_knowledge_graph({'source_navigation': navigation}, {}, {},
            json.loads((fixture.root / reader.REGISTRY_REF).read_bytes()),
            json.loads((ROOT / 'ToS/doctrine/semantic-interchange/relation-types.v1.json').read_bytes()))
        self.assertEqual(before, {path: path.read_bytes() for path in fixture.root.rglob('*') if path.is_file()})
        return navigation, graph, diagnostics

    def test_writer_to_navigation_and_lens_keeps_native_and_declared_exact_versions_separate(self):
        for kind in ('agent', 'letter'):
            with self.subTest(kind=kind), self.family(kind) as fixture:
                previous = copy.deepcopy(fixture.record)
                receipt = self.correct(fixture, label='Только новая синтетическая редакция; не новый допуск.')
                current = json.loads(fixture.path.read_bytes())
                references = [reader._record_ref(previous), reader._record_ref(current)]
                navigation, graph, diagnostics = self.project_fixture(fixture)
                self.assertEqual(diagnostics, [])
                subject = next(node for node in navigation['nodes'] if node['node_id'] == current['record_id'])
                history = subject['properties']['record_history']
                self.assertEqual(history['status'], 'available', history)
                self.assertEqual(history['current_ref'], references[-1])
                self.assertEqual(history['refs'], references)
                self.assertEqual(subject['properties']['source_record'], current)
                self.assertTrue(subject['properties']['human_forms'])
                self.assertTrue(all(form['subject'] == references[-1]
                                    for form in subject['properties']['human_forms']))
                raw_versions = {node['properties']['record_version_view']['record_ref']['version']: node
                                for node in navigation['nodes'] if node['node_kind'] == 'record-version'}
                self.assertEqual(set(raw_versions), {previous['record_version'], current['record_version']})
                self.assertEqual(len(navigation['edges']), 2)
                self.assertEqual({edge['to_id'] for edge in navigation['edges']},
                                 {node['node_id'] for node in raw_versions.values()})
                for edge in navigation['edges']:
                    self.assertEqual(edge['from_id'], current['record_id'])
                    self.assertEqual(edge['predicate_id'], 'has_record_version')
                    self.assertEqual(edge['review_status'], 'not_applicable')
                archive = fixture.root / receipt['archive_path']
                manifest = json.loads((archive / 'manifest.json').read_bytes())
                old_locator = receipt['archive_path'] + '/' + manifest['files'][fixture.path.name]['blob']
                self.assertEqual(raw_versions[previous['record_version']]['source_ref'], old_locator)
                self.assertEqual(raw_versions[current['record_version']]['source_ref'], fixture.relative)
                self.assertEqual(json.loads((fixture.root / old_locator).read_bytes()), previous)

                from tos_access.knowledge import execute_knowledge_lens
                for detail in ('compact', 'full'):
                    result = execute_knowledge_lens(graph, {'schema_version': 'tos_lens_spec_v1',
                        'lens_id': 'retained-metadata-history', 'sources': ['source-navigation'],
                        'detail': detail, 'language': 'en'})
                    self.assertEqual(len(result['nodes']), 3)
                    self.assertEqual(len(result['relations']), 2)
                    packets = {node['semantics']['record_version']['record_ref']['version']: node
                               for node in result['nodes'] if node['kind_id'] == 'record-version'}
                    self.assertEqual(set(packets), set(raw_versions))
                    for record, reference, status, language in (
                            (previous, references[0], 'historical', None),
                            (current, references[1], 'current', 'ru')):
                        packet = packets[record['record_version']]
                        view = packet['semantics']['record_version']
                        self.assertEqual(view['record_ref'], reference)
                        self.assertEqual(view['version_status'], status)
                        self.assertEqual(view['record_kind'], 'metadata')
                        self.assertEqual(view['status'], 'available')
                        self.assertFalse(view['grants_current_use'])
                        self.assertFalse(view['performs_assessment'])
                        self.assertNotEqual(packet['entity_id'], current['record_id'])
                        self.assertNotEqual(packet['native_id'], current['record_id'])
                        self.assertEqual(packet['type_id'], 'tos.entity.record-version')
                        self.assertNotIn('claim', packet['semantics'])
                        self.assertNotIn('time', packet['semantics'])
                        context = packet['semantics']['assertion_contexts'][0]['fields']['record']
                        self.assertEqual(context['value'], record)
                        self.assertEqual(context['source_pointer'], '/properties/record_version_view/record')
                        self.assertEqual(packet['display']['summary']['original'], record['notes'])
                        self.assertEqual(packet['display_selection']['fields']['summary']['actual_language'], language)
                        self.assertEqual(packet['display']['provenance']['summary_source_language'], language)
                        self.assertEqual(packet['epistemic'], {'authority_layer': 'derived-export',
                            'canon_status': None, 'review_posture': 'not-recorded', 'confidence': None})
                        self.assertNotIn('human_form_selection', packet)
                        self.assertNotIn('human_forms', packet['attributes'])
                        if detail == 'full':
                            self.assertEqual(packet['attributes']['record_version_view']['record'], record)
                        else:
                            self.assertEqual(packet['attributes'], {})
                            self.assertNotIn('source_record', packet)

    def test_corrupt_writer_archive_exports_unavailable_history_without_inventing_versions(self):
        for kind in ('agent', 'letter'):
            with self.subTest(kind=kind), self.family(kind) as fixture:
                receipt = self.correct(fixture)
                current = json.loads(fixture.path.read_bytes())
                archive = fixture.root / receipt['archive_path']
                manifest = json.loads((archive / 'manifest.json').read_bytes())
                (archive / manifest['files'][fixture.path.name]['blob']).write_bytes(b'corrupt retained record')
                navigation, graph, diagnostics = self.project_fixture(fixture)
                self.assertEqual(len(navigation['nodes']), 1)
                self.assertEqual(navigation['edges'], [])
                subject = navigation['nodes'][0]
                self.assertEqual(subject['node_id'], current['record_id'])
                self.assertEqual(subject['properties']['source_record'], current)
                self.assert_unavailable(subject['properties']['record_history'], 'corrupt')
                self.assertEqual(len(diagnostics), 1)
                self.assertEqual(diagnostics[0]['level'], 'warning')
                self.assertIn('exact metadata history unavailable: corrupt/', diagnostics[0]['message'])
                from tos_access.knowledge import execute_knowledge_lens
                for detail in ('compact', 'full'):
                    result = execute_knowledge_lens(graph, {'schema_version': 'tos_lens_spec_v1',
                        'lens_id': 'corrupt-retained-metadata-history', 'sources': ['source-navigation'],
                        'detail': detail, 'language': 'en'})
                    self.assertEqual(len(result['nodes']), 1)
                    self.assertEqual(result['nodes'][0]['entity_id'], current['record_id'])
                    self.assertNotIn('record_version', result['nodes'][0]['semantics'])
                    self.assertEqual(result['relations'], [])
                    if detail == 'full':
                        self.assert_unavailable(result['nodes'][0]['attributes']['record_history'], 'corrupt')

    def test_retained_baseline_can_start_above_one_and_refs_do_not_invent_prior_versions(self):
        record = {**self.record, 'record_version': 7}
        self.path.write_bytes(revisions._encode(record))
        self.fixture.formpath.unlink()
        self.sync_catalog(self.fixture)
        self.correct()
        self.correct(label='Ninth version; earlier baseline is not invented.')
        instance = reader.MetadataVersionReader(self.root)
        references = instance.exact_refs(record['record_id'])
        self.assertEqual([ref['version'] for ref in references['refs']], [7, 8, 9], references)
        self.assertEqual(references['provenance']['history']['retained_baseline_ref'], reader._record_ref(record))
        self.assertEqual(instance.resolve(reader._record_ref(record))['record'], record)
        self.assert_unavailable(instance.resolve(reader._record_ref(self.record)), 'missing', 'exact-version-not-retained')

    def test_mismatched_digest_unretained_version_and_absent_identity_never_fall_back(self):
        self.correct()
        ref = reader._record_ref(self.record)
        self.assert_unavailable(self.resolve({**ref, 'digest': 'sha256:' + 'f' * 64}), 'stale', 'exact-version-digest-mismatch')
        self.assert_unavailable(self.resolve({**ref, 'version': 9}), 'missing', 'exact-version-not-retained')
        self.assert_unavailable(self.resolve({**ref, 'id': 'tos.agent.absent'}), 'missing', 'record-not-in-public-catalog')
        self.assert_unavailable(reader.MetadataVersionReader(self.root).exact_refs('tos.agent.absent'), 'missing')

    def test_bad_exact_refs_are_rejected_before_reading_any_source(self):
        reference = reader._record_ref(self.record)
        for changed in ({'version': True}, {'version': 1.0}, {'version': 2**53}, {'version': 0},
                        {'id': 'tos.claim.wrong-family'}, {'id': '../private'}, {'digest': 'bad'}, {'unknown': None}):
            with (self.subTest(changed=changed), patch.object(source, '_read', side_effect=AssertionError('no read')),
                  self.assertRaises(ValueError)):
                self.resolve({**reference, **changed})

    def test_catalog_drift_duplicate_identity_and_wrong_schema_mapping_are_refused(self):
        catalog = self.fixture.catalog
        original = catalog.read_bytes()
        entry = source._json_object(original)
        for fields in ({'record_sha256': '0' * 64}, {'preferred_label': 'A stale generated label'},
                       {'identity_status': 'verified'}, {'source_schema_ref': 'ToS/contracts/document-record.schema.json'}):
            catalog.write_bytes(source._canonical({**entry, **fields}) + b'\n')
            with self.subTest(fields=fields):
                self.assert_unavailable(self.resolve(), 'stale')
        catalog.write_bytes(original + original)
        self.assert_unavailable(self.resolve(), 'corrupt')
        with self.family('letter') as fixture:
            entry = source._json_object(fixture.catalog.read_bytes())
            entry['source_schema_ref'] = 'ToS/contracts/corpus-record.schema.json'
            fixture.catalog.write_bytes(source._canonical(entry) + b'\n')
            self.assert_unavailable(reader.MetadataVersionReader(fixture.root).resolve(reader._record_ref(fixture.record)), 'stale')

    def test_nonpublic_or_foreign_current_source_and_reserved_paths_withhold_history(self):
        self.correct()
        current = self.path.read_bytes()
        for fields, status in (({'visibility': 'local_only'}, 'access-restricted'),
                               ({'schema_version': 'tos_historical_record_v1'}, 'access-restricted'),
                               ({'record_version': True}, 'corrupt'), ({'record_type': 'place'}, 'corrupt')):
            self.path.write_bytes(revisions._encode({**json.loads(current), **fields}))
            with self.subTest(fields=fields):
                self.assert_unavailable(self.resolve(), status)
        self.path.write_bytes(current)
        catalog = self.fixture.catalog
        entry = source._json_object(catalog.read_bytes())
        for part in ('payload', 'private', 'local-content', 'owner-local', '.record-revisions'):
            changed = {**entry, 'source_record_ref': f'ToS/source-witnesses/{part}/subject/agent.json'}
            catalog.write_bytes(source._canonical(changed))
            with (self.subTest(part=part), patch.object(reader.MetadataVersionReader, '_validate_current',
                    side_effect=AssertionError('private source cannot reach validation'))):
                self.assert_unavailable(self.resolve(), 'access-restricted')
        with self.family('letter') as fixture:
            before = reader._record_ref(fixture.record)
            self.correct(fixture)
            record = json.loads(fixture.path.read_bytes())
            record['visibility'] = 'local_only'
            fixture.path.write_bytes(revisions._encode(record))
            self.assert_unavailable(reader.MetadataVersionReader(fixture.root).resolve(before), 'access-restricted')

    def test_symlink_current_record_and_selected_archive_blob_are_never_followed(self):
        original = self.path.read_bytes()
        elsewhere = self.root / 'private.json'
        elsewhere.write_bytes(original)
        self.path.unlink()
        self.path.symlink_to(elsewhere)
        self.assert_unavailable(self.resolve(), 'access-restricted')
        self.path.unlink()
        self.path.write_bytes(original)
        receipt = self.correct()
        directory = self.root / receipt['archive_path']
        manifest = source._json_object((directory / 'manifest.json').read_bytes())
        blob = directory / manifest['files'][self.path.name]['blob']
        blob.unlink()
        blob.symlink_to(elsewhere)
        self.assert_unavailable(self.resolve(), 'access-restricted')

    def test_archive_companions_are_unread_even_when_unknown_bytes_are_missing_or_corrupt(self):
        companion = self.path.parent / 'unrecognized.json'
        companion.write_bytes(b'UNKNOWN PRIVATE-SCOPE COMPANION IS NOT READER INPUT')
        receipt = self.correct()
        directory = self.root / receipt['archive_path']
        manifest = source._json_object((directory / 'manifest.json').read_bytes())
        ignored = directory / manifest['files'][companion.name]['blob']
        ignored.unlink()
        companion.write_bytes(b'CHANGED UNKNOWN BYTES')
        original_read = source._read
        def selected_only(path, limit):
            self.assertNotIn(path, (ignored, companion))
            return original_read(path, limit)
        with patch.object(source, '_read', side_effect=selected_only):
            result = self.resolve()
        self.assertEqual(result['record'], self.record, result)
        self.assertFalse(result['provenance']['all_package_bytes_verified'])
        self.assertNotIn('unrecognized', json.dumps(result['provenance']))

    def test_corrupt_later_record_prevents_early_predecessor_return_and_refs(self):
        self.correct()
        later = self.correct(label='Third exact record.')
        directory = self.root / later['archive_path']
        manifest = source._json_object((directory / 'manifest.json').read_bytes())
        blob = directory / manifest['files'][self.path.name]['blob']
        original = blob.read_bytes()
        blob.write_bytes(b'corrupt later selected record')
        self.assert_unavailable(self.resolve(), 'corrupt')
        self.assert_unavailable(reader.MetadataVersionReader(self.root).exact_refs(self.record['record_id']), 'corrupt')
        blob.unlink()
        self.assert_unavailable(self.resolve(), 'missing', 'retained-record-file-missing')
        blob.write_bytes(original)
        manifest['source']['version'] = True
        (directory / 'manifest.json').write_bytes(revisions._encode(manifest))
        self.assert_unavailable(self.resolve(), 'corrupt')

    def test_changed_retained_request_and_bool_receipt_version_fail_closed(self):
        self.correct()
        path = self.path.parent / revisions.HISTORY
        original = path.read_bytes()
        for change in ('prose', 'schema', 'bool-version', 'operation'):
            history = source._json_object(original)
            receipt = history['receipts'][0]
            request = receipt['request']
            if change == 'prose': request['fields']['notes'] = 'Different reconstructed record.'
            elif change == 'schema': request['fields']['schema_version'] = 'another-schema'
            elif change == 'bool-version': request['expected_source']['version'] = True
            else: request['operation'] = 'source.create'
            receipt['changed_fields'] = sorted(request['fields'])
            receipt['request_digest'] = source._digest(source._canonical(request))
            path.write_bytes(revisions._encode(history))
            with self.subTest(change=change):
                self.assert_unavailable(self.resolve(), 'corrupt')

    def test_uncommitted_archive_is_not_history_and_no_history_does_not_imply_version_one(self):
        request = self.fixture.request()
        with patch.object(revisions, '_exchange', side_effect=RuntimeError('before publish')):
            with self.assertRaises(RuntimeError):
                source.run_local_command(self.fixture.owner, request)
        result = self.resolve()
        self.assertEqual(result['version_status'], 'current', result)
        self.assertEqual(result['provenance']['history']['receipt_count'], 0)
        self.assertEqual(reader.MetadataVersionReader(self.root).exact_refs(self.record['record_id'])['refs'],
                         [reader._record_ref(self.record)])

    def test_snapshot_reuse_does_not_hide_source_schema_history_or_locator_drift(self):
        self.correct()
        targets = [self.path, self.path.parent / revisions.HISTORY, self.fixture.catalog,
                   self.root / reader.CORPUS_REF]
        for target in targets:
            original = target.read_bytes()
            instance = reader.MetadataVersionReader(self.root)
            result = self.resolve(instance=instance)
            self.assertEqual(result['status'], 'available', result)
            result['record']['preferred_label'] = 'Consumer mutation is not source mutation.'
            with patch.object(source, '_read', wraps=source._read) as reads:
                self.assertEqual(self.resolve(instance=instance)['record'], self.record)
                instance.exact_refs(self.record['record_id'])
                self.assertEqual(reads.call_count, 0)
            target.write_bytes(original + b'\n')
            with self.subTest(target=target):
                self.assert_unavailable(self.resolve(instance=instance), 'stale', 'source-changed-during-read')
                with self.assertRaises(source.JournalConflict):
                    instance.verify_current()
            target.write_bytes(original)

    def test_concurrent_source_change_during_archive_verification_never_returns_mixed_record(self):
        self.correct()
        original = reader.MetadataVersionReader._archive_record
        def changed(instance, *args):
            value = original(instance, *args)
            self.path.write_bytes(self.path.read_bytes() + b'\n')
            return value
        with patch.object(reader.MetadataVersionReader, '_archive_record', changed):
            self.assert_unavailable(self.resolve(), 'stale', 'source-changed-during-read')

    def test_explicit_budgets_fail_without_partial_refs_or_records(self):
        for module, name, budget in ((reader, 'MAX_CATALOG_BYTES', 1), (reader, 'MAX_CATALOG_ROWS', 0),
                                    (reader, 'MAX_TOTAL_BYTES', 1), (reader, 'MAX_CONTRACTS', 0)):
            with self.subTest(budget=name), patch.object(module, name, budget):
                self.assert_unavailable(self.resolve(), 'over-budget')
        self.correct()
        with patch.object(revisions, 'MAX_REVISIONS', 0):
            self.assert_unavailable(self.resolve(), 'over-budget', 'correction-receipt-count-budget')
        with patch.object(revisions, 'MAX_FILES', 0):
            self.assert_unavailable(self.resolve(), 'over-budget', 'archive-file-binding-count-budget')

    def test_supports_uses_declared_profile_routes_and_never_enumerates_private_sources(self):
        instance = reader.MetadataVersionReader(self.root)
        with (patch.object(SourceRecordProfiles, 'native_semantic_identities', side_effect=AssertionError('no inventory')),
              patch.object(Path, 'rglob', side_effect=AssertionError('no source tree scan'))):
            for kind in ('agent', 'work', 'expression', 'letter', 'historical-state', 'sign'):
                self.assertTrue(instance.supports(kind), kind)
            for kind in ('artifact', 'item', 'link', 'claim', 'unknown-test-kind'):
                self.assertFalse(instance.supports(kind), kind)
            self.assertFalse(instance.supports('agent', source_ref='ToS/source-witnesses/public/test/place.json'))
        registry = self.root / reader.REGISTRY_REF
        registry.write_bytes(registry.read_bytes() + b'\n')
        with self.assertRaises(source.JournalConflict):
            instance.verify_current()

    def test_raw_source_provenance_resolves_current_then_committed_predecessor_without_latest_fallback(self):
        raw = self.path.read_bytes()
        digest = source._digest(raw)[7:]
        current = reader.MetadataVersionReader(self.root).resolve_source_bytes(self.fixture.relative, digest)
        self.assertEqual(current['status'], 'available', current)
        self.assertEqual(current['record'], self.record)
        self.assertIsNone(current['provenance']['source']['archive_blob_ref'])
        self.correct()
        retained = reader.MetadataVersionReader(self.root).resolve_source_bytes(self.fixture.relative, digest)
        self.assertEqual(retained['status'], 'available', retained)
        self.assertEqual(retained['record'], self.record)
        binding = retained['provenance']['source']
        self.assertEqual(binding['source_ref'], self.fixture.relative)
        self.assertEqual(binding['record_sha256'], 'sha256:' + digest)
        self.assertEqual((self.root / binding['archive_blob_ref']).read_bytes(), raw)
        missing = reader.MetadataVersionReader(self.root).resolve_source_bytes(self.fixture.relative, '0' * 64)
        self.assertEqual(missing['status'], 'missing', missing)
        self.assertIsNone(missing['record'])
        self.assertIsNone(missing['exact_ref'])
        self.assertIsNone(missing['provenance'])

    def test_raw_source_provenance_does_not_search_other_paths_or_uncommitted_blobs(self):
        digest = source._digest(self.path.read_bytes())[7:]
        other = str(Path(self.fixture.relative).parent.with_name('another-subject') / self.path.name)
        missing = reader.MetadataVersionReader(self.root).resolve_source_bytes(other, digest)
        self.assertEqual(missing['status'], 'missing', missing)
        self.correct()
        resolved = reader.MetadataVersionReader(self.root).resolve_source_bytes(self.fixture.relative, digest)
        archived = self.root / resolved['provenance']['source']['archive_blob_ref']
        archived.write_bytes(b'corrupt retained synthetic metadata')
        corrupt = reader.MetadataVersionReader(self.root).resolve_source_bytes(self.fixture.relative, digest)
        self.assertEqual(corrupt['status'], 'corrupt', corrupt)
        self.assertIsNone(corrupt['record'])
        self.assertIsNone(corrupt['provenance'])


if __name__ == '__main__':
    unittest.main()
