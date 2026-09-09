"""Retained legacy object-Link context, not source migration or admission."""
from __future__ import annotations

import copy
from contextlib import contextmanager
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
for directory in (ROOT / 'scripts', ROOT / 'access/src'):
    if str(directory) not in sys.path:
        sys.path.insert(0, str(directory))

from source_object_link_read import (
    LegacyObjectLinkReader, LegacyObjectLinkError, SOURCE_REF, SCHEMA_REF, canonical_digest,
)


class LegacyObjectLinkReturnTests(unittest.TestCase):
    @contextmanager
    def source_fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for ref in (SCHEMA_REF, SOURCE_REF):
                (root / ref).parent.mkdir(parents=True, exist_ok=True)
                (root / ref).write_bytes((ROOT / ref).read_bytes())
            yield root

    def test_exact_legacy_rows_return_without_fabricated_fields_or_source_writes(self):
        with self.source_fixture() as root:
            before = (root / SOURCE_REF).read_bytes()
            reader = LegacyObjectLinkReader(root)
            rows = reader.rows()
            self.assertEqual(len(rows), 5)
            for line, claim in rows:
                with self.subTest(claim=claim['claim_id']):
                    entry = {'claim_id': claim['claim_id'], 'source_claim_file_ref': SOURCE_REF,
                        'source_claim_line': line, 'claim_sha256': canonical_digest(claim)}
                    self.assertEqual(reader.from_catalog(entry), claim)
                    self.assertEqual(reader.context(line, claim)['source_claim'], claim)
                    self.assertEqual(claim['reviews'], [])
                    self.assertFalse(claim['qualifiers']['availability_is_rights_conclusion'])
                    for field in ('statement', 'statement_language', 'assessment_refs', 'human_forms'):
                        self.assertNotIn(field, claim)
                    reader.from_catalog(entry)['maker']['agent_ref'] = 'no-mutation'
                    self.assertEqual(reader.from_catalog(entry), claim)
            reader.verify_current()
            self.assertEqual((root / SOURCE_REF).read_bytes(), before)

    def test_exact_path_line_digest_schema_and_endpoint_domains_fail_closed(self):
        with self.source_fixture() as root:
            reader = LegacyObjectLinkReader(root)
            line, claim = reader.rows()[0]
            entry = {'claim_id': claim['claim_id'], 'source_claim_file_ref': SOURCE_REF,
                'source_claim_line': line, 'claim_sha256': canonical_digest(claim)}
            for alteration in ({'source_claim_file_ref': SOURCE_REF.replace('object-link/', 'other/')},
                    {'source_claim_line': True}, {'source_claim_line': 999}, {'claim_id': 'tos.claim.other'},
                    {'claim_sha256': '0' * 64}, {'source_schema_ref': 'ToS/contracts/claim-packet.schema.json'}):
                with self.subTest(alteration=alteration), self.assertRaises(LegacyObjectLinkError):
                    reader.from_catalog({**entry, **alteration})
            objects = {claim['subject_ref']: {'record_type': 'work'}, claim['object']: {'record_type': 'link'}}
            reader.validate(claim, objects)
            for field, kind in (('subject_ref', 'artifact'), ('subject_ref', 'agent'), ('object', 'work')):
                altered = copy.deepcopy(objects)
                altered[claim[field]]['record_type'] = kind
                with self.subTest(field=field, kind=kind), self.assertRaises(LegacyObjectLinkError):
                    reader.validate(claim, altered)
            with self.assertRaises(LegacyObjectLinkError):
                reader.context(line, {**claim, 'claim_version': 2})
            (root / SOURCE_REF).write_bytes((root / SOURCE_REF).read_bytes() + b'\n')
            with self.assertRaises(LegacyObjectLinkError):
                reader.verify_current()

    def test_legacy_schema_scope_and_unknown_qualifiers_remain_distinct(self):
        with self.source_fixture() as root:
            reader = LegacyObjectLinkReader(root)
            _, claim = reader.rows()[0]
            extended = copy.deepcopy(claim)
            extended['qualifiers']['uninterpreted_source_limit'] = {'scope': None, 'explicit': False, 'gaps': []}
            (root / SOURCE_REF).write_text(json.dumps(extended) + '\n')
            reread = LegacyObjectLinkReader(root)
            self.assertEqual(reread.context(1, reread.rows()[0][1])['source_claim'], extended)
            for alteration in ({'subject_ref': 'tos.artifact.fixture'}, {'schema_version': 'tos_object_link_claim_v2'},
                    {'review_status': 'accepted'}, {'reviews': [{'invented': True}]},
                    {'visibility': 'private'}, {'statement': 'Fabricated wording.'}, {'predicate': []}):
                with self.subTest(alteration=alteration), self.assertRaises(LegacyObjectLinkError):
                    reader.validate({**claim, **alteration})

    def test_duplicate_identity_duplicate_json_key_and_symlink_are_not_legacy_sources(self):
        with self.source_fixture() as root:
            first = (root / SOURCE_REF).read_bytes().splitlines()[0]
            for raw in (first + b'\n' + first + b'\n', first[:-1] + b',"reviews":[]}\n'):
                (root / SOURCE_REF).write_bytes(raw)
                with self.assertRaises(LegacyObjectLinkError):
                    LegacyObjectLinkReader(root)
            path = root / SOURCE_REF
            retained = path.with_name('retained.jsonl')
            path.rename(retained)
            path.symlink_to(retained.name)
            with self.assertRaises(LegacyObjectLinkError):
                LegacyObjectLinkReader(root)

    def test_undeclared_schema_dependency_is_rejected_without_network_retrieval(self):
        with self.source_fixture() as root:
            path = root / SCHEMA_REF
            schema = json.loads(path.read_bytes())
            schema['allOf'] = [{'$ref': 'https://unregistered.invalid/schema.json'}]
            path.write_text(json.dumps(schema))
            with patch('urllib.request.urlopen', side_effect=AssertionError('network retrieval')):
                with self.assertRaisesRegex(LegacyObjectLinkError, 'undeclared dependency'):
                    LegacyObjectLinkReader(root)


class NativeVersionViewTests(unittest.TestCase):
    def test_actual_native_identity_and_exact_version_digest_survive_portable_view(self):
        from source_witness_human_forms import metadata_subject
        from tos_access.knowledge import _record_version_view, _metadata_history_refs
        samples = [sorted((ROOT / 'ToS/source-witnesses').rglob(basename))[0]
                   for basename in ('artifact-witness.json', 'composite-witness.json')]
        for path in samples:
            record = json.loads(path.read_bytes())
            reference = metadata_subject(record).ref
            view = {'schema_version': 'tos_record_version_view_v1', 'record_ref': reference,
                'record_kind': 'metadata', 'status': 'available', 'reason': 'exact-current-record',
                'version_status': 'current', 'record': record,
                'provenance': {'source': {'source_ref': path.relative_to(ROOT).as_posix()}},
                'grants_current_use': False, 'performs_assessment': False}
            carrier = {'node_id': 'record-version:' + canonical_digest(reference), 'node_kind': 'record-version',
                'source_ref': path.relative_to(ROOT).as_posix(), 'properties': {'record_version_view': view}}
            history = {'schema_version': 'tos_metadata_record_history_v1', 'status': 'available',
                'reason': 'exact-current-record', 'record_id': reference['id'], 'current_ref': reference,
                'refs': [reference], 'provenance': view['provenance'], 'grants_current_use': False,
                'performs_assessment': False, 'writes_to_source': False}
            identity = {'entity_id': reference['id'], 'attributes': {'record_history': history, 'source_record': record}}
            with self.subTest(schema=record['schema_version']):
                self.assertEqual(_record_version_view(carrier), view)
                self.assertNotIn('record_id', _record_version_view(carrier)['record'])
                self.assertEqual(_metadata_history_refs(identity), [reference])
                for alteration in ({'record_version': record['record_version'] + 1},
                        {'record_id': reference['id']}, {'notes': 'Unbound changed body.'}):
                    modified = copy.deepcopy(carrier)
                    modified['properties']['record_version_view']['record'].update(alteration)
                    with self.subTest(alteration=alteration), self.assertRaises(ValueError):
                        _record_version_view(modified)
                    changed_identity = copy.deepcopy(identity)
                    changed_identity['attributes']['source_record'].update(alteration)
                    self.assertIsNone(_metadata_history_refs(changed_identity))


class LegacyObjectLinkProjectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from source_witness_bibliographic_graph_common import build_payload
        from tos_corpus_index_common import build_source_navigation
        from tos_access.knowledge import build_knowledge_graph
        cls.original = (ROOT / SOURCE_REF).read_bytes()
        cls.rows = LegacyObjectLinkReader(ROOT).rows()
        cls.projection = build_payload(ROOT)
        diagnostics = []
        cls.navigation = build_source_navigation(diagnostics)
        if diagnostics:
            raise AssertionError(diagnostics)
        cls.registries = [json.loads((ROOT / 'ToS/doctrine/semantic-interchange' / name).read_bytes())
                          for name in ('entity-types.v1.json', 'relation-types.v1.json')]
        cls.graph = build_knowledge_graph({'source_navigation': cls.navigation}, {}, cls.projection, *cls.registries)

    def test_all_five_legacy_claims_and_direct_relations_have_exact_core_context_and_form_gaps(self):
        from tos_access.core import ToSAccessCore
        from tos_access.knowledge import select_human_forms
        core = ToSAccessCore.discover(tos_root=ROOT)
        with patch.object(ToSAccessCore, 'knowledge_graph', return_value=self.graph):
            for line, claim in self.rows:
                with self.subTest(claim=claim['claim_id']):
                    packet = core.knowledge_node(claim['claim_id'])
                    node = next(node for node in packet['matches'] if node['source_graph'] == 'source-claims')
                    relation = core.knowledge_relation('source-navigation:claim:' + claim['claim_id'])['matches'][0]
                    self.assertEqual(node['attributes']['source_claim'], claim)
                    self.assertEqual(relation['attributes']['source_claim'], claim)
                    self.assertEqual(node['attributes']['source_line'], line)
                    self.assertEqual(relation['attributes']['source_claim_line'], line)
                    for carrier in (node, relation):
                        self.assertEqual(carrier['attributes']['source_sha256'], canonical_digest(claim))
                        context = carrier['semantics']['assertion_contexts'][0]
                        for field in ('qualifiers', 'evidence_refs', 'maker', 'reviews', 'claim_version',
                                      'provenance_event_ref', 'supersedes_claim_ref', 'review_status'):
                            self.assertEqual(context['fields'][field]['value'], claim[field])
                            self.assertEqual(context['fields'][field]['source_pointer'], '/properties/source_claim/' + field)
                        self.assertNotIn('assessment_refs', context['fields'])
                        self.assertTrue(all(role['state'] == 'missing' for role in select_human_forms(carrier)['roles'].values()))
                    self.assertTrue(node['semantics']['claim']['evidence_node_ids'])
                    self.assertEqual(node['semantics']['claim']['object_entity_id'], claim['object'])
                    link = core.knowledge_node(claim['object'])['matches']
                    self.assertEqual({entry['source_graph'] for entry in link}, {'source-navigation', 'source-claims'})
                    self.assertTrue(all(entry['type_id'] == 'tos.entity.link' for entry in link))
                    self.assertEqual(link[0]['attributes']['source_record'], link[1]['attributes']['source_record'])
        self.assertEqual((ROOT / SOURCE_REF).read_bytes(), self.original)

    def test_direct_navigation_and_legacy_source_contract_are_preserved(self):
        from jsonschema import Draft202012Validator
        from source_record_profiles import SourceClaimProfiles, SourceProfileError
        schema = json.loads((ROOT / 'ToS/contracts/tos-corpus-index.schema.json').read_bytes())
        Draft202012Validator({'$defs': schema['$defs'], '$ref': '#/$defs/sourceNavigation'}).validate(self.navigation)
        profiles = SourceClaimProfiles(ROOT)
        for line, claim in self.rows:
            with self.subTest(claim=claim['claim_id']):
                edge = next(edge for edge in self.navigation['edges'] if edge.get('claim_ref') == claim['claim_id'])
                self.assertEqual({key: value for key, value in edge.items() if key != 'properties'}, {
                    'edge_id': 'source-navigation:claim:' + claim['claim_id'], 'from_id': claim['subject_ref'],
                    'predicate_id': claim['predicate'], 'to_id': claim['object'], 'edge_kind': 'evidence_claim',
                    'review_status': claim['review_status'], 'source_refs': sorted(set([SOURCE_REF, *claim['evidence_refs']])),
                    'claim_ref': claim['claim_id']})
                with self.assertRaises(SourceProfileError):
                    profiles.validate(claim)

    def test_portable_context_refuses_endpoint_digest_and_cross_carrier_substitution(self):
        from tos_access.knowledge import _validate_retained_object_link_contexts
        edges = [edge for edge in self.navigation['edges'] if edge.get('properties', {}).get('source_adapter')]
        edge = copy.deepcopy(edges[0])
        _validate_retained_object_link_contexts({'edges': [edge]}, self.projection['nodes'])
        for alteration in ({'to_id': edge['from_id']}, {'claim_ref': 'tos.claim.other'},
                {'properties': {**edge['properties'], 'source_sha256': '0' * 64}},
                {'properties': {**edge['properties'], 'source_claim_line': edge['properties']['source_claim_line'] + 1}}):
            with self.subTest(alteration=list(alteration)), self.assertRaises(ValueError):
                _validate_retained_object_link_contexts({'edges': [{**edge, **alteration}]}, self.projection['nodes'])
        altered = copy.deepcopy(edge)
        altered['properties']['source_claim']['qualifiers']['new_unbound_scope'] = 'Another carrier cannot silently win.'
        altered['properties']['source_sha256'] = canonical_digest(altered['properties']['source_claim'])
        with self.assertRaisesRegex(ValueError, 'carriers disagree'):
            _validate_retained_object_link_contexts({'edges': [altered]}, self.projection['nodes'])
        old = {key: value for key, value in edge.items() if key != 'properties'}
        _validate_retained_object_link_contexts({'edges': [old]}, [])
