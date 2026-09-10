"""Synthetic identity plans: exact history, separate authority, no redirects."""
from __future__ import annotations

import copy
from contextlib import contextmanager
import json
from pathlib import Path
from types import SimpleNamespace
import unittest

import test_source_claim_commands as fixtures
import source_commands as commands
import claim_revisions
from source_record_profiles import SourceClaimProfiles
import source_identity_proposals as proposals

ROOT = fixtures.ROOT


def plan(refs, *, operation='merge'):
    before, after = (refs[:2], refs[2:]) if operation == 'merge' else (refs[:1], refs[1:])
    return {'kind': 'identity-transition-proposal', 'operation': operation,
        'members': [ref['id'] for ref in refs], 'predecessors': before, 'successors': after,
        'mapping': [{'predecessor': old['id'], 'successor': new['id']} for old in before for new in after],
        'source_wording': {'text': 'Synthetic proposal only; subjects remain distinct.', 'language': 'en', 'script': 'Latn'},
        'grounds': 'Artificial test grounds, not a historical assertion.',
        'counterreading': 'The synthetic records may concern distinct referents.',
        'scope': 'Only the three artificial metadata identities in this test.',
        'unresolved_links': [], 'supersedes_proposal': None}


def synthetic_claim():
    refs = [{'id': f'tos.agent.synthetic-{name}', 'version': 1, 'digest': 'sha256:' + digit * 64}
            for name, digit in (('a', 'a'), ('b', 'b'), ('c', 'c'))]
    return {'schema_version': 'tos_source_identity_transition_claim_v1',
        'claim_id': 'tos.claim.synthetic-identity-proposal', 'claim_version': 1,
        'claim_type': 'relation', 'assertion_layer': 'identity_assertion',
        'subject_ref': refs[0]['id'], 'predicate': proposals.PREDICATE, 'object': plan(refs),
        'evidence_refs': ['ToS/source-witnesses/synthetic.json'],
        'maker': {'maker_type': 'software', 'agent_ref': 'software:synthetic'},
        'provenance_event_ref': 'tos.event.synthetic-identity-proposal', 'epistemic_status': 'uncertain',
        'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
        'qualifiers': {'statement': 'Merge is proposed, not performed or accepted.', 'statement_language': 'en', 'statement_script': 'Latn'}}


class IdentityProposalContractTests(unittest.TestCase):
    def test_source_profile_accepts_only_complete_merge_or_split_plans(self):
        profiles = SourceClaimProfiles(ROOT)
        claim = synthetic_claim()
        profiles.validate(claim)
        refs = list(proposals.participants(claim))
        claim['object'] = plan(refs, operation='split')
        profiles.validate(claim)
        self.assertEqual(set(profiles.reference_members(claim)), set(claim['object']['members']))
        for change in ({'mapping': claim['object']['mapping'][:1]}, {'members': refs[:1]},
                       {'successors': refs[1:2]}, {'predecessors': [refs[1]]},
                       {'operation': 'redirect'}, {'successors': [refs[0], refs[1]]}):
            invalid = {**claim, 'object': {**claim['object'], **change}}
            with self.subTest(change=change), self.assertRaises(ValueError):
                profiles.validate(invalid)

    def test_only_concrete_source_identity_descriptor_is_eligible(self):
        profiles = SourceClaimProfiles(ROOT)
        self.assertTrue(proposals.eligible_type(profiles.entities, 'tos.entity.agent'))
        for kind in ('tos.entity.identity', 'tos.entity.literal', 'tos.entity.claim', 'tos.entity.unmapped', 'tos.entity.missing'):
            with self.subTest(kind=kind):
                self.assertFalse(proposals.eligible_type(profiles.entities, kind))

    def test_proposal_assessment_scope_cannot_be_lowered_or_turned_into_execution(self):
        claim = synthetic_claim()
        proposals.validate_assessment_scope(claim, {'risk': 'high', 'requested_use': 'research'})
        for risk, use in (('low', 'research'), ('medium', 'research'), ('high', 'merge'), ('high', 'publication')):
            with self.subTest(risk=risk, use=use), self.assertRaises(PermissionError):
                proposals.validate_assessment_scope(claim, {'risk': risk, 'requested_use': use})

    def test_ordinary_value_and_layer_grants_do_not_authorize_proposals(self):
        from source_claim_commands import _value_scope
        claim, profiles = synthetic_claim(), SourceClaimProfiles(ROOT)
        config = {'schema_version': proposals.CREATE_CONFIG, 'allowed_object_values': [claim['object']],
            'allowed_object_refs': claim['object']['members'], 'allowed_related_claim_refs': []}
        _value_scope(config, claim, profiles)
        for schema in (commands.CLAIM_CONFIG, commands.CLAIM_VALUE_CONFIG, commands.CLAIM_STRUCTURED_CONFIG,
                       commands.CLAIM_REFERENCE_CONFIG, commands.CLAIM_REFERENCE_REVISION_CONFIG,
                       commands.CLAIM_LAYER_REVISION_CONFIG, commands.OWNER_CLAIM_REFERENCE_CONFIG):
            with self.subTest(schema=schema), self.assertRaises(PermissionError):
                _value_scope({**config, 'schema_version': schema}, claim, profiles)
        with self.assertRaises(PermissionError):
            _value_scope({**config, 'allowed_object_refs': claim['object']['members'][1:]}, claim, profiles)

    def test_topology_cannot_be_changed_under_a_correction_identity(self):
        claim = synthetic_claim()
        allowed = claim_revisions._advance(claim, {'object': {**claim['object'], 'grounds': 'Corrected synthetic grounds.'}})
        self.assertEqual(allowed['claim_version'], 2)
        for field in proposals.FROZEN:
            value = copy.deepcopy(claim['object'])
            value[field] = {} if value[field] is None else None
            with self.subTest(field=field), self.assertRaises(PermissionError):
                claim_revisions._advance(claim, {'object': value})

    def test_exact_predecessor_and_unresolved_links_need_separate_scope(self):
        from source_claim_commands import _value_scope
        claim = synthetic_claim()
        previous = {'id': 'tos.claim.synthetic-previous-proposal', 'version': 2, 'digest': 'sha256:' + 'd' * 64}
        claim['object']['supersedes_proposal'] = previous
        claim['supersedes_claim_ref'] = previous['id']
        profiles = SourceClaimProfiles(ROOT)
        profiles.validate(claim)
        config = {'schema_version': proposals.CREATE_CONFIG, 'allowed_object_values': [claim['object']],
            'allowed_object_refs': claim['object']['members'], 'allowed_related_claim_refs': []}
        with self.assertRaises(PermissionError):
            _value_scope(config, claim, profiles)
        _value_scope({**config, 'allowed_related_claim_refs': [previous]}, claim, profiles)
        claim['supersedes_claim_ref'] = 'tos.claim.another'
        with self.assertRaises(ValueError):
            profiles.validate(claim)

    def test_grounding_refuses_unavailable_or_forged_endpoint_capability(self):
        claim, profiles = synthetic_claim(), SourceClaimProfiles(ROOT)
        endpoint = {'status': 'available', 'descriptor': {'type_id': 'tos.entity.agent'}, 'provenance': {}}
        reader = SimpleNamespace(resolve_typed=lambda ref: endpoint, verify_current=lambda: None)
        claims = SimpleNamespace(resolve=lambda ref: {}, verify_current=lambda: None)
        self.assertEqual(len(proposals.ground(claim, profiles, reader, claims)['participants']), 3)
        for status, descriptor in (('stale', endpoint['descriptor']), ('available', None),
                                   ('available', {'type_id': 'tos.entity.identity'})):
            reader.resolve_typed = lambda ref: {'status': status, 'descriptor': descriptor}
            with self.subTest(status=status, descriptor=descriptor), self.assertRaises(ValueError):
                proposals.ground(claim, profiles, reader, claims)


class IdentityProposalCommandTests(unittest.TestCase):
    creation = fixtures.SourceClaimCreationTests.creation
    revision_config = proposals.REVISION_CONFIG
    semantic_kinds = ()

    @contextmanager
    def identity_creation(self):
        with self.creation() as (root, owner, config, claim, request, rebuild, graph_fixture):
            for name in ('source-identity-transition-claim', 'subject-identity-transition-claim',
                         'source-structured-value', 'source-metadata-record',
                         'semantic-description-record', 'lexical-description-record'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((ROOT / ref).read_bytes())
            original = json.loads((root / 'ToS/source-witnesses/agents/friedrich-nietzsche/agent.json').read_bytes())
            paths, refs = [], []
            for index, suffix in enumerate(('a', 'b', 'c')):
                kind = self.semantic_kinds[index] if self.semantic_kinds else 'agent'
                if self.semantic_kinds:
                    record = {'schema_version': 'tos_semantic_description_record_v1',
                        'record_type': kind, 'record_id': f'tos.{kind}.synthetic-{suffix}', 'record_version': 1,
                        'preferred_label': 'Synthetic subject ' + suffix, 'identity_status': 'provisional',
                        'source_refs': claim['evidence_refs'], 'external_identifiers': [],
                        'same_as_posture': 'no_equivalence_claim', 'visibility': 'public_metadata_only',
                        'notes': 'An artificial bounded research referent; no historical assertion.',
                        'field_languages': {field: {'language': 'en', 'script': 'Latn'}
                                            for field in ('preferred_label', 'notes')},
                        'semantic_scope': {'scope_note': 'Only this synthetic fixture.',
                            'identity_criterion': 'The same artificial subject, not its label.',
                            'language': 'en', 'script': 'Latn'}}
                    if kind == 'lexeme':
                        record['schema_version'] = 'tos_lexical_description_record_v1'
                        record['semantic_content'] = {'lexical_account': 'Synthetic lexical grouping only.',
                            'grammatical_account': 'Grammatical role is unassessed.', 'language': 'en', 'script': 'Latn'}
                else:
                    record = {**copy.deepcopy(original), 'record_id': 'tos.agent.synthetic-' + suffix,
                              'preferred_label': 'Synthetic person ' + suffix, 'record_version': 1}
                path = root / f'ToS/source-witnesses/{kind}s/synthetic-{suffix}/{kind}.json'
                path.parent.mkdir(parents=True)
                path.write_text(json.dumps(record))
                paths.append(path)
                refs.append(commands.Record.from_payload(record['record_id'], 1, record).ref)
            reader = proposals.READER_V2 if self.semantic_kinds else proposals.READER
            predicate = proposals.READER_PREDICATES[reader]
            claim.update(schema_version=proposals.READER_SCHEMAS[reader][0], predicate=predicate,
                assertion_layer='identity_assertion', subject_ref=refs[0]['id'], object=plan(refs),
                qualifiers=synthetic_claim()['qualifiers'])
            config.update(schema_version=proposals.CREATE_CONFIG_V2 if self.semantic_kinds else proposals.CREATE_CONFIG,
                allowed_subject_refs=[claim['subject_ref']],
                allowed_predicates=[predicate], allowed_object_values=[copy.deepcopy(claim['object'])],
                allowed_object_refs=claim['object']['members'], allowed_related_claim_refs=[])
            owner.write_text(json.dumps(config))
            rebuild()
            yield root, owner, config, claim, request, rebuild, graph_fixture, paths

    def create(self, owner, request):
        preview = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare-create', 'claims': request['claims']})
        request.update(expected_configuration=preview['owner_configuration'],
            expected_dependencies=preview['expected_dependencies'], expected_inputs=preview['source_bindings'])
        return commands.run_local_command(owner, request), preview

    def test_create_correct_replay_exact_history_forms_and_old_subjects(self):
        from claim_version_reader import ClaimVersionReader
        from metadata_version_reader import MetadataVersionReader
        with self.identity_creation() as (root, owner, config, claim, request, rebuild, fixture, paths):
            before = [path.read_bytes() for path in paths]
            created, initial = self.create(owner, request)
            self.assertFalse(created['grants_admission'])
            self.assertTrue(commands.run_local_command(owner, request)['replayed'])
            exact = created['receipt']['claims'][0]
            projection = rebuild()
            graph, _, _ = fixture.historical_knowledge(root, projection)
            schema = json.loads((root / 'ToS/contracts/source-witness-catalog.schema.json').read_bytes())
            catalog_validator = commands.Draft202012Validator({'$ref': '#/$defs/claim_entry', '$defs': schema['$defs']})
            for line in (root / 'ToS/source-witnesses/catalog/claims.jsonl').read_text().splitlines():
                catalog_validator.validate(json.loads(line))
            missing_member = copy.deepcopy(projection)
            selected_edge = next(edge for edge in missing_member['edges']
                                 if edge['claim_ref'] == claim['claim_id'] and edge['edge_kind'] == 'has_value_member')
            missing_member['edges'].remove(selected_edge)
            with self.assertRaises(ValueError):
                fixture.historical_knowledge(root, missing_member)
            self.assertTrue(any(node['type_id'] == 'tos.entity.identity-transition-proposal' for node in graph['nodes']))
            self.assertEqual(ClaimVersionReader(root).resolve(exact)['record'], claim)
            from assessment_journal import _source_records
            bindings = [{'path': config['source_path'], 'record_id': claim['claim_id'], 'origin_id': 'synthetic:proposal'}]
            bindings += [{'path': path.relative_to(root).as_posix(), 'record_id': ref['id'], 'origin_id': 'synthetic:' + str(index)}
                         for index, (ref, path) in enumerate(zip(proposals.participants(claim), paths, strict=True))]
            dependencies = {}
            _source_records(root, bindings, claim_dependencies=dependencies)
            self.assertEqual(dependencies[claim['claim_id']], list(proposals.participants(claim)))
            drifted = {**json.loads(before[0]), 'record_version': 2, 'preferred_label': 'A changed synthetic source.'}
            paths[0].write_text(json.dumps(drifted))
            with self.assertRaisesRegex(commands.JournalConflict, 'frozen exact source'):
                _source_records(root, bindings)
            paths[0].write_bytes(before[0])
            changed_value = {**copy.deepcopy(claim['object']), 'grounds': 'Corrected synthetic grounds; no transition executed.'}
            revision = {key: config[key] for key in ('uid', 'principal_id', 'source_root', 'source_path', 'authority_ref', 'expires_at')}
            revision.update(schema_version=self.revision_config, claim_id=claim['claim_id'],
                allowed_operations=['claim.revise'], allowed_fields=['object', 'qualifiers'],
                allowed_object_values=[claim['object'], changed_value], allowed_object_refs=config['allowed_object_refs'],
                allowed_related_claim_refs=[], allowed_evidence_refs=config['allowed_evidence_refs'],
                allowed_form_ids=['tos.form.synthetic-identity-proposal'])
            owner.write_text(json.dumps(revision))
            change = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
                'fields': {'object': changed_value}, 'reason': 'Correct the artificial proposal grounds only.',
                'forms': [{'form_id': 'tos.form.synthetic-identity-proposal', 'field_id': 'claim.statement'}]}
            preview = commands.run_local_command(owner, change)
            correction = {**change, 'operation': 'claim.revise', 'command_id': 'synthetic:identity-correct',
                'expected_configuration': preview['owner_configuration'], 'expected_source': preview['source'],
                'expected_revision': preview['revision'], 'expected_dependencies': preview['expected_dependencies'],
                'expected_inputs': preview['source_bindings']}
            corrected = commands.run_local_command(owner, correction)
            self.assertTrue(commands.run_local_command(owner, correction)['replayed'])
            self.assertEqual(corrected['source']['version'], 2)
            materialized = corrected['materializations'][0]
            self.assertFalse(materialized['standalone_reading'])
            self.assertIsNone(materialized['admission'])
            self.assertEqual(materialized['context'][0]['value']['object'], changed_value)
            inspected = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'inspect-version', 'source': exact})
            self.assertEqual(inspected['record'], claim)
            rebuild()
            self.assertEqual(ClaimVersionReader(root).resolve(exact)['record'], claim)
            for ref, path, raw in zip(proposals.participants(claim), paths, before, strict=True):
                self.assertEqual(path.read_bytes(), raw)
                resolved = MetadataVersionReader(root).resolve_typed(ref)
                self.assertEqual(resolved['record']['record_id'], ref['id'])
                self.assertEqual(resolved['descriptor']['record_type'], json.loads(raw)['record_type'])
                self.assertTrue(proposals.eligible_type(SourceClaimProfiles(root).entities,
                    resolved['descriptor']['type_id'], reader=proposals.reader_for_claim(claim)))
            owner.write_text(json.dumps(config))
            self.assertTrue(commands.run_local_command(owner, request)['replayed'])

    def test_changed_topology_is_a_new_exact_successor_proposal_not_subject_succession(self):
        from claim_version_reader import ClaimVersionReader
        with self.identity_creation() as (root, owner, config, claim, request, rebuild, fixture, paths):
            created, _ = self.create(owner, request)
            prior = created['receipt']['claims'][0]
            rebuild()
            successor = copy.deepcopy(claim)
            successor.update(claim_id='tos.claim.synthetic-split-successor',
                provenance_event_ref='tos.event.synthetic-split-successor', supersedes_claim_ref=claim['claim_id'])
            successor['object'] = plan(list(proposals.participants(claim)), operation='split')
            successor['object']['supersedes_proposal'] = prior
            successor['qualifiers']['statement'] = 'A split is now proposed; neither plan has been executed.'
            next_config = {**config, 'source_path': 'ToS/source-witnesses/relations/synthetic-split-successor/source-claims.jsonl',
                'allowed_claim_ids': [successor['claim_id']], 'provenance_event_id': successor['provenance_event_ref'],
                'allowed_object_values': [successor['object']], 'allowed_related_claim_refs': [prior]}
            owner.write_text(json.dumps(next_config))
            next_request = {**request, 'claims': [successor], 'command_id': 'synthetic:split-successor'}
            second, _ = self.create(owner, next_request)
            graph, _, _ = fixture.historical_knowledge(root, rebuild())
            self.assertEqual(ClaimVersionReader(root).resolve(prior)['record'], claim)
            self.assertEqual(ClaimVersionReader(root).resolve(second['receipt']['claims'][0])['record'], successor)
            self.assertTrue(any(edge['relation_type_id'] == 'tos.relation.claim-supersedes' for edge in graph['relations']))
            denied = {**next_config, 'allowed_related_claim_refs': []}
            owner.write_text(json.dumps(denied))
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, next_request)
            self.assertEqual(ClaimVersionReader(root).resolve(prior)['record'], claim)

    def test_creation_rejects_bad_exact_ref_and_never_touches_participants(self):
        with self.identity_creation() as (root, owner, config, claim, request, *_):
            claim['object']['successors'][0]['digest'] = 'sha256:' + '0' * 64
            config['allowed_object_values'] = [claim['object']]
            owner.write_text(json.dumps(config))
            with self.assertRaises(ValueError):
                self.create(owner, request)
            self.assertFalse((root / config['source_path']).exists())


class SemanticIdentityProposalContractTests(unittest.TestCase):
    def test_adapter_is_explicit_and_old_reader_never_inherits_semantic_scope(self):
        profiles = SourceClaimProfiles(ROOT)
        opted_in = {key for key, entry in profiles.entities.items()
                    if entry.get('source_record_profile', {}).get('identity_proposal_adapter')}
        self.assertTrue({'tos.entity.crosscutting-concept', 'tos.entity.conception', 'tos.entity.lexeme',
                         'tos.entity.occurrence', 'tos.entity.sign'} <= opted_in)
        for kind in opted_in:
            with self.subTest(kind=kind):
                self.assertTrue(proposals.eligible_type(profiles.entities, kind, reader=proposals.READER_V2))
                self.assertFalse(proposals.eligible_type(profiles.entities, kind))
        kind = 'tos.entity.conception'
        entry = profiles.entities[kind]
        future = copy.deepcopy(entry)
        future['type_id'] = 'tos.entity.synthetic-future-subject'
        del future['source_record_profile']['identity_proposal_adapter']
        contract = json.loads((ROOT / 'ToS/contracts/semantic-entity-type-registry.schema.json').read_bytes())
        commands.Draft202012Validator({'$ref': '#/$defs/typeEntry', '$defs': contract['$defs']}).validate(future)
        self.assertFalse(proposals.eligible_type({future['type_id']: future}, future['type_id'], reader=proposals.READER_V2))
        unsupported = [
            {**entry, 'abstract': True}, {**entry, 'object_role': 'literal'},
            {**entry, 'source_mappings': []},
            *({**entry, 'source_record_profile': {**entry['source_record_profile'], field: value}}
              for field, value in (('identity_proposal_adapter', None), ('reader', 'native-annotation-v1'),
                  ('graph_layer', 'native'), ('record_type', 'claim'), ('schemas', []),
                  ('id_prefix', 'tos.claim.'), ('source_basename', 'opaque.json')))]
        for index, replacement in enumerate(unsupported):
            with self.subTest(invalid=index):
                self.assertFalse(proposals.eligible_type({kind: replacement}, kind, reader=proposals.READER_V2))
        for kind in ('tos.entity.semantic-object', 'tos.entity.claim', 'tos.entity.literal',
                     'tos.entity.identity', 'tos.entity.unmapped', 'tos.entity.annotation-packet'):
            with self.subTest(kind=kind):
                self.assertFalse(proposals.eligible_type(profiles.entities, kind, reader=proposals.READER_V2))

    def test_versioned_grants_schema_topology_and_assessment_stay_separate(self):
        from source_claim_commands import _value_scope
        profiles = SourceClaimProfiles(ROOT)
        old, claim = synthetic_claim(), synthetic_claim()
        claim.update(predicate=proposals.PREDICATE_V2, schema_version=proposals.READER_SCHEMAS[proposals.READER_V2][0])
        profiles.validate(claim)
        for selected in (old, claim):
            reader = proposals.reader_for_claim(selected)
            grants = {key for key, value in proposals.CONFIG_READERS.items() if value == reader}
            config = {'allowed_object_values': [selected['object']],
                'allowed_object_refs': selected['object']['members'], 'allowed_related_claim_refs': []}
            for schema in proposals.CONFIGS:
                with self.subTest(reader=reader, config=schema):
                    if schema in grants:
                        _value_scope({**config, 'schema_version': schema}, selected, profiles)
                    else:
                        with self.assertRaises(PermissionError):
                            _value_scope({**config, 'schema_version': schema}, selected, profiles)
        with self.assertRaises(ValueError):
            profiles.validate({**claim, 'schema_version': old['schema_version']})
        proposals.validate_assessment_scope(claim, {'risk': 'high', 'requested_use': 'research'})
        for scope in ({'risk': 'medium', 'requested_use': 'research'}, {'risk': 'high', 'requested_use': 'merge'}):
            with self.assertRaises(PermissionError):
                proposals.validate_assessment_scope(claim, scope)
        for field in proposals.FROZEN:
            changed = copy.deepcopy(claim['object'])
            changed[field] = None if changed[field] is not None else 'changed'
            with self.subTest(field=field), self.assertRaises(PermissionError):
                claim_revisions._advance(claim, {'object': changed})
        self.assertTrue(proposals.predecessor_allowed(claim, old))
        self.assertTrue(proposals.predecessor_allowed(claim, claim))
        self.assertFalse(proposals.predecessor_allowed(old, claim))
        self.assertFalse(proposals.predecessor_allowed(claim, {'predicate': 'ordinary'}))


class SemanticIdentityProposalCommandTests(IdentityProposalCommandTests):
    """Same transaction/history engine over explicitly declared semantic subjects."""
    revision_config = proposals.REVISION_CONFIG_V2
    semantic_kinds = ('crosscutting-concept', 'conception', 'lexeme')

    def test_grounding_and_portable_closure_require_exact_declared_subject_capability(self):
        from build_source_witness_catalog import CatalogBuildError
        from metadata_version_reader import MetadataVersionReader
        from claim_version_reader import ClaimVersionReader
        with self.identity_creation() as (root, owner, config, claim, request, rebuild, fixture, paths):
            profiles, reader = SourceClaimProfiles(root), MetadataVersionReader(root)
            refs = list(proposals.participants(claim))
            views = {ref['id']: reader.resolve_typed(ref) for ref in refs}
            self.assertEqual(len(proposals.ground(claim, profiles, reader, ClaimVersionReader(root))['participants']), 3)
            ref, good = refs[0], views[refs[0]['id']]
            for mutation in (
                {'status': 'stale'}, {'exact_ref': {**ref, 'version': 2}}, {'record_digest': 'sha256:' + '0' * 64},
                {'descriptor': {**good['descriptor'], 'record_kind': 'claim'}},
                {'descriptor': {**good['descriptor'], 'source_schema_ref': proposals.SCHEMA_REF_V2}},
                {'descriptor': {**good['descriptor'], 'type_id': 'tos.entity.claim'}},
                {'record': {**good['record'], 'record_version': 2}},
                {'record': {**good['record'], 'visibility': 'local_only'}},
                {'record': {**good['record'], 'schema_version': 'unknown'}},
                {'provenance': {'source': {'source_ref': 'ToS/source-witnesses/items/private/payload/crosscutting-concept.json'}}},
            ):
                fake = SimpleNamespace(resolve_typed=lambda selected: ({**good, **mutation}
                    if selected == ref else views[selected['id']]), verify_current=lambda: None)
                with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                    proposals.ground(claim, profiles, fake, ClaimVersionReader(root))
            original = paths[0].read_bytes()
            for record in ({**good['record'], 'visibility': 'local_only'},
                           {**good['record'], 'record_version': 2}):
                paths[0].write_text(json.dumps(record))
                with self.assertRaises((ValueError, PermissionError, CatalogBuildError)):
                    self.create(owner, request)
                self.assertFalse((root / config['source_path']).exists())
            paths[0].write_bytes(original)
            created, _ = self.create(owner, request)
            projection = rebuild()
            fixture.historical_knowledge(root, projection)
            for mutation in ('missing', 'private', 'wrong-id', 'wrong-schema', 'opaque', 'payload-path'):
                broken = copy.deepcopy(projection)
                carrier = next(row for row in broken['nodes'] if row['node_id'] == 'identity:' + ref['id'])
                if mutation == 'missing':
                    broken['nodes'].remove(carrier)
                elif mutation == 'payload-path':
                    carrier['source_ref'] = 'ToS/source-witnesses/items/private/payload/crosscutting-concept.json'
                elif mutation == 'opaque':
                    carrier['properties']['source_record'] = ['opaque']
                else:
                    field, value = {'private': ('visibility', 'local_only'), 'wrong-id': ('record_id', refs[1]['id']),
                                    'wrong-schema': ('schema_version', 'unknown')}[mutation]
                    carrier['properties']['source_record'][field] = value
                with self.subTest(carrier=mutation), self.assertRaises(ValueError):
                    fixture.historical_knowledge(root, broken)
            revoked = {**config, 'allowed_object_refs': config['allowed_object_refs'][1:]}
            owner.write_text(json.dumps(revoked))
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, request)
            self.assertFalse(created['grants_admission'])
            self.assertEqual(paths[0].read_bytes(), original)
