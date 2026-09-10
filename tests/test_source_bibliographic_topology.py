"""Synthetic closure and writer-boundary regressions; no corpus mutation."""
from __future__ import annotations

import copy
import hashlib
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
for directory in (ROOT / 'scripts', ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'):
    if str(directory) not in sys.path:
        sys.path.insert(0, str(directory))

import claim_revisions
import source_claim_commands
import source_owner_claim_commands
from source_bibliographic_topology import (
    BibliographicTopologyError, validate_current_topology, validate_work_expression_delta, validate_expression_edition_delta,
)
from source_record_profiles import SourceClaimProfiles, SourceProfileError, SourceRecordProfiles
from validate_source_witness_foundation import _legacy_topology_configuration, _topology_evidence_matches


WORK_PATH = 'ToS/source-witnesses/works/example/work.json'
EXPRESSION_PATH = 'ToS/source-witnesses/works/example/expressions/new/expression.json'
WORK_ID, EXPRESSION_ID, CLAIM_ID = 'tos.work.example', 'tos.expression.new', 'tos.claim.new'


class ScopedCompositionTests(unittest.TestCase):
    """Shared structure invariants, independent of historical truth or admission."""

    @classmethod
    def setUpClass(cls):
        cls.profiles = SourceClaimProfiles(ROOT)

    def fixture(self, kind='intellectual-part-composition', count=3):
        _, _, _, claim = delta()
        member_kind = 'artifact' if kind == 'physical-part-composition' else 'textual-fragment'
        members = [f'tos.{member_kind}.synthetic.{index}' for index in range(count)]
        subject_kind = {'intellectual-part-composition': 'work',
                        'physical-part-composition': 'artifact',
                        'research-corpus-membership': 'research-corpus'}[kind]
        subject = WORK_ID if subject_kind == 'work' else f'tos.{subject_kind}.synthetic'
        value = {'kind': kind, 'members': members,
            'source_wording': {'text': 'Synthetic scoped composition; no source truth claimed.', 'language': 'en', 'script': 'Latn'},
            'source_scope': 'Only this synthetic selection.', 'coverage': 'partial',
            'membership_basis': 'Declared test selection, not a discovered fact.',
            'ordering': {'mode': 'total', 'basis': 'Synthetic editorial order, not historical chronology.',
                         'precedes': [[a, b] for a, b in zip(members, members[1:])]},
            'limitations': 'No admission or completeness beyond this test.',
            'extensions': {'unknown': [None, False, 0], 'members': ['tos.agent.inert']}}
        claim.update(schema_version='tos_source_member_structure_claim_v1', subject_ref=subject,
            predicate=kind.replace('-', '_'), object=value, assertion_layer='scholarly_report')
        objects = {subject: {'record_id': subject, 'record_type': subject_kind}}
        objects.update({ref: {'record_id': ref, 'record_type': member_kind} for ref in members})
        return claim, objects

    def test_typed_parts_and_corpus_members_preserve_complete_value_and_dependencies(self):
        for kind in ('intellectual-part-composition', 'physical-part-composition', 'research-corpus-membership'):
            with self.subTest(kind=kind):
                claim, objects = self.fixture(kind)
                original = copy.deepcopy(claim)
                self.profiles.validate(claim, objects)
                self.assertEqual(self.profiles.identity_refs(claim), set(objects))
                self.assertEqual(claim, original)
                self.assertNotIn(claim['claim_id'], self.profiles.identity_refs(claim))
                self.assertNotIn('tos.agent.inert', self.profiles.identity_refs(claim))

    def test_physical_parts_do_not_recast_digital_items_or_intellectual_parts(self):
        for wrong_kind in ('item', 'work', 'textual-fragment', 'composite', 'research-corpus'):
            for endpoint in ('subject', 'member'):
                with self.subTest(wrong_kind=wrong_kind, endpoint=endpoint):
                    claim, objects = self.fixture('physical-part-composition')
                    ref = claim['subject_ref'] if endpoint == 'subject' else claim['object']['members'][0]
                    objects[ref]['record_type'] = wrong_kind
                    with self.assertRaisesRegex(SourceProfileError, 'domain/range'):
                        self.profiles.validate(claim, objects)
        claim, objects = self.fixture('physical-part-composition')
        claim['object']['kind'] = 'intellectual-part-composition'
        with self.assertRaises(SourceProfileError):
            self.profiles.validate(claim, objects)

    def test_order_modes_are_local_and_serialization_does_not_supply_order(self):
        claim, objects = self.fixture()
        original = copy.deepcopy(claim)
        claim['object']['members'].reverse()
        self.profiles.validate(claim, objects)
        claim['object']['ordering']['precedes'] = []
        with self.assertRaisesRegex(SourceProfileError, 'incomparable'):
            self.profiles.validate(claim, objects)
        for mode in ('unordered', 'partial'):
            claim['object']['ordering']['mode'] = mode
            self.profiles.validate(claim, objects)
        # Opposite complete order in a different Claim is retained as a rival,
        # not unioned with the first Claim into an artificial global cycle.
        rival = copy.deepcopy(original)
        rival['claim_id'] = 'tos.claim.rival'
        rival['object']['ordering']['precedes'] = [edge[::-1] for edge in rival['object']['ordering']['precedes']]
        for selected in (original, rival):
            self.profiles.validate(selected, objects)

    def test_rejects_cycle_unknown_member_duplicate_self_and_false_total_order(self):
        claim, objects = self.fixture()
        a, b, c = claim['object']['members']
        changes = (
            {'ordering': {'mode': 'partial', 'basis': 'Test', 'precedes': [[a, b], [b, a]]}},
            {'ordering': {'mode': 'unordered', 'basis': 'Test', 'precedes': [[a, b]]}},
            {'ordering': {'mode': 'total', 'basis': 'Test', 'precedes': [[a, b]]}},
            {'ordering': {'mode': 'partial', 'basis': 'Test', 'precedes': [[a, 'tos.work.outside']]}},
            {'ordering': {'mode': 'partial', 'basis': 'Test', 'precedes': [[a, a]]}},
            {'ordering': {'mode': 'partial', 'basis': 'Test', 'precedes': [[a, b], [a, b]]}},
            {'members': [a, a, c]}, {'members': [a, b, claim['subject_ref']]}, {'members': []},
            {'coverage': 'complete'}, {'source_scope': ' '}, {'membership_basis': None},
            {'ordering': {'mode': 'historical', 'basis': 'Test', 'precedes': []}},
        )
        for change in changes:
            with self.subTest(change=change), self.assertRaises(SourceProfileError):
                selected = copy.deepcopy(claim)
                selected['object'].update(change)
                self.profiles.validate(selected, objects)

    def test_bounds_and_specific_domain_range_are_enforced(self):
        claim, objects = self.fixture(count=128)
        self.profiles.validate(claim, objects)
        claim129, objects129 = self.fixture(count=129)
        with self.assertRaises(SourceProfileError):
            self.profiles.validate(claim129, objects129)
        for role in ('subject', 'member', 'missing'):
            with self.subTest(role=role), self.assertRaises(SourceProfileError):
                claim, objects = self.fixture()
                ref = claim['subject_ref'] if role == 'subject' else claim['object']['members'][0]
                if role == 'missing':
                    del objects[ref]
                else:
                    objects[ref]['record_type'] = 'place'
                self.profiles.validate(claim, objects)

    def test_research_corpus_description_has_own_identity_and_no_inline_membership(self):
        profiles = SourceRecordProfiles(ROOT)
        record = {'schema_version': 'tos_research_corpus_record_v1', 'record_type': 'research-corpus',
            'record_id': 'tos.research-corpus.synthetic', 'record_version': 1,
            'preferred_label': 'Synthetic research selection', 'notes': 'A test corpus, not a publication.',
            'field_languages': {key: {'language': 'en', 'script': 'Latn'} for key in ('preferred_label', 'notes')},
            'identity_status': 'provisional', 'source_refs': ['ToS/doctrine/CORPUS_FOUNDATION.md'],
            'external_identifiers': [], 'same_as_posture': 'no_equivalence_claim', 'visibility': 'public_metadata_only',
            'semantic_scope': {'scope_note': 'This test only.', 'identity_criterion': 'The same research purpose across corrected descriptions.', 'language': 'en', 'script': 'Latn'},
            'semantic_content': {'research_purpose': 'Test selection.', 'selection_criterion': 'Only artificial members.',
                                 'coverage_account': 'Explicitly incomplete.', 'language': 'en', 'script': 'Latn'},
            'extensions': {'unknown': [None, False, 0]}}
        profiles.validate('research-corpus', record)
        for change in ({'record_id': 'tos.collection.synthetic'}, {'membership_claim_refs': []}, {'semantic_content': {'language': 'en', 'script': 'Latn'}}):
            with self.subTest(change=change), self.assertRaises(SourceProfileError):
                profiles.validate('research-corpus', {**record, **change})


def delta():
    before = {
        'schema_version': 'tos_corpus_record_v1', 'record_id': WORK_ID, 'record_type': 'work',
        'record_version': 4, 'preferred_label': 'Work', 'notes': 'Keep this source wording.',
        'identity_status': 'verified', 'expression_claim_refs': ['tos.claim.old'],
        'responsibility_claim_refs': ['tos.claim.author'], 'supersedes_ref': None,
    }
    after = {**copy.deepcopy(before), 'record_version': 5,
             'expression_claim_refs': ['tos.claim.old', CLAIM_ID]}
    expression = {
        'schema_version': 'tos_corpus_record_v1', 'record_id': EXPRESSION_ID,
        'record_type': 'expression', 'record_version': 1, 'work_ref': WORK_ID,
        'identity_status': 'provisional', 'same_as_posture': 'no_equivalence_claim',
        'supersedes_ref': None, 'responsibility_claim_refs': [], 'embodiment_claim_refs': [],
        'variant_labels': [], 'external_identifiers': [], 'language': 'ru',
    }
    claim = {
        'schema_version': 'tos_source_relation_claim_v1', 'claim_id': CLAIM_ID,
        'claim_type': 'relation', 'assertion_layer': 'bibliographic_assertion',
        'predicate': 'has_expression', 'subject_ref': WORK_ID, 'object': EXPRESSION_ID,
        'claim_version': 1, 'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
        'epistemic_status': 'observed', 'polarity': 'positive',
        'evidence_refs': [WORK_PATH, EXPRESSION_PATH], 'assessment_refs': [],
        'maker': {'maker_type': 'model', 'agent_ref': 'model:example'},
        'provenance_event_ref': 'tos.event.example',
        'qualifiers': {'statement': 'The records declare this bibliographic link.',
                       'statement_language': 'en', 'statement_script': 'Latn',
                       'limits': 'Not an accepted textual identity.'},
    }
    return before, after, expression, claim


def topology():
    before, after, expression, claim = delta()
    records = {
        WORK_ID: after,
        'tos.expression.old': {'record_type': 'expression', 'record_id': 'tos.expression.old',
            'work_ref': WORK_ID, 'embodiment_claim_refs': ['tos.claim.edition']},
        EXPRESSION_ID: expression,
        'tos.edition.old': {'record_type': 'edition', 'record_id': 'tos.edition.old',
            'embodies_expression_refs': ['tos.expression.old'], 'exemplar_claim_refs': ['tos.claim.item']},
        'tos.item.old': {'record_type': 'item', 'record_id': 'tos.item.old'},
    }
    legacy = [
        {'claim_id': 'tos.claim.old', 'predicate': 'has_expression', 'subject_ref': WORK_ID,
         'object': 'tos.expression.old', 'claim_type': 'bibliographic'},
        {'claim_id': 'tos.claim.edition', 'predicate': 'embodied_by', 'subject_ref': 'tos.expression.old',
         'object': 'tos.edition.old', 'claim_type': 'bibliographic'},
        {'claim_id': 'tos.claim.item', 'predicate': 'exemplified_by', 'subject_ref': 'tos.edition.old',
         'object': 'tos.item.old', 'claim_type': 'bibliographic'},
    ]
    return records, legacy, claim, {'tos.item.old': 'tos.edition.old'}


class EditionDeltaTests(unittest.TestCase):
    edition_path = EXPRESSION_PATH.replace('expression.json', 'editions/new/edition.json')

    def values(self):
        _, _, before, claim = delta()
        before.update(record_version=2, embodiment_claim_refs=['tos.claim.old-edition'],
            responsibility_claim_refs=['tos.claim.translator'], language='en', notes='Retain qualified attribution.')
        after = {**copy.deepcopy(before), 'record_version': 3,
                 'embodiment_claim_refs': ['tos.claim.old-edition', CLAIM_ID]}
        edition = {'schema_version': 'tos_corpus_record_v1', 'record_type': 'edition', 'record_id': 'tos.edition.new',
            'record_version': 1, 'identity_status': 'provisional', 'same_as_posture': 'no_equivalence_claim',
            'supersedes_ref': None, 'embodies_expression_refs': [EXPRESSION_ID], 'publication_claim_refs': [],
            'exemplar_claim_refs': [], 'variant_labels': [], 'external_identifiers': []}
        claim.update(predicate='embodied_by', subject_ref=EXPRESSION_ID, object=edition['record_id'],
                     evidence_refs=[EXPRESSION_PATH, self.edition_path])
        return before, after, edition, claim

    def check_delta(self, values):
        validate_expression_edition_delta(*values, expression_source_ref=EXPRESSION_PATH,
                                           edition_source_ref=self.edition_path)

    def test_only_exact_append_and_provisional_manifestation_are_allowed(self):
        values = self.values()
        original = copy.deepcopy(values)
        self.check_delta(values)
        self.assertEqual(values, original)
        for index, update in ((1, {'language': 'ru'}), (1, {'responsibility_claim_refs': []}),
                (1, {'record_version': 4}), (1, {'embodiment_claim_refs': [CLAIM_ID]}),
                (2, {'record_version': True}), (2, {'identity_status': 'verified'}),
                (2, {'publication_claim_refs': ['tos.claim.date']}), (2, {'exemplar_claim_refs': ['tos.claim.item']}),
                (2, {'embodies_expression_refs': [EXPRESSION_ID, 'tos.expression.other']}),
                (2, {'collection_ref': 'tos.collection.other'}), (2, {'work_ref': WORK_ID}),
                (3, {'review_status': 'accepted'}), (3, {'object': 'tos.edition.other'}),
                (3, {'evidence_refs': [EXPRESSION_PATH]})):
            with self.subTest(index=index, update=update):
                altered = self.values()
                altered[index].update(update)
                with self.assertRaises(BibliographicTopologyError):
                    self.check_delta(altered)

    def test_profile_does_not_open_standalone_public_private_or_revision_writers(self):
        claim = self.values()[3]
        profiles = SourceClaimProfiles(ROOT)
        profiles.validate(claim)
        with self.assertRaisesRegex(PermissionError, 'compound bibliographic'):
            source_claim_commands._scope({'allowed_operations': ['claims.create']}, [claim], profiles=profiles)
        for initial in (True, False):
            with self.subTest(initial=initial), self.assertRaisesRegex(PermissionError, 'compound bibliographic'):
                source_owner_claim_commands._scope({}, [claim], profiles, initial=initial)
        with self.assertRaisesRegex(PermissionError, 'compound bibliographic'):
            claim_revisions._scope({'allowed_operations': ['claim.revise']}, {}, claim, profiles=profiles)

    def test_global_closure_keeps_multi_expression_and_no_item_editions_valid(self):
        records, legacy, new_claim, item_links = topology()
        records['tos.edition.old']['embodies_expression_refs'].append(EXPRESSION_ID)
        records[EXPRESSION_ID]['embodiment_claim_refs'] = ['tos.claim.shared-edition']
        shared = {'claim_id': 'tos.claim.shared-edition', 'predicate': 'embodied_by',
                  'subject_ref': EXPRESSION_ID, 'object': 'tos.edition.old'}
        records['tos.edition.metadata-only'] = {'record_id': 'tos.edition.metadata-only', 'record_type': 'edition',
            'embodies_expression_refs': [], 'exemplar_claim_refs': [], 'collection_ref': 'tos.collection.example'}
        validate_current_topology(records, [*legacy, new_claim, shared], item_edition_by_id=item_links)


class BibliographicDeltaTests(unittest.TestCase):
    def check_delta(self, values, **paths):
        return validate_work_expression_delta(*values, work_source_ref=paths.get('work', WORK_PATH),
            expression_source_ref=paths.get('expression', EXPRESSION_PATH))

    def test_exact_append_preserves_source_objects(self):
        values = delta()
        unchanged = copy.deepcopy(values)
        self.check_delta(values)
        self.assertEqual(values, unchanged)

    def test_read_profile_is_declared_without_changing_legacy_carrier(self):
        profiles = SourceClaimProfiles(ROOT)
        self.assertEqual(profiles.profiles['has_expression']['reader'], 'identity-relation-v1')
        claim = delta()[3]
        profiles.validate(claim)

    def test_work_changes_and_nonsequential_versions_fail_closed(self):
        for update in ({'preferred_label': 'Changed'}, {'responsibility_claim_refs': []},
                       {'record_version': 6}, {'record_version': True},
                       {'expression_claim_refs': [CLAIM_ID, 'tos.claim.old']},
                       {'expression_claim_refs': [CLAIM_ID]}, {'extra': 'silent rewrite'}):
            with self.subTest(update=update):
                values = delta()
                values[1].update(update)
                with self.assertRaises(BibliographicTopologyError):
                    self.check_delta(values)

    def test_expression_cannot_import_admission_or_other_ladder_steps(self):
        for update in ({'identity_status': 'verified'}, {'record_version': 2}, {'record_version': True},
                       {'work_ref': 'tos.work.other'}, {'record_type': 'edition'},
                       {'same_as_posture': 'equivalent'}, {'supersedes_ref': 'tos.expression.old'},
                       {'responsibility_claim_refs': ['tos.claim.author']},
                       {'embodiment_claim_refs': ['tos.claim.edition']},
                       {'derivation_claim_refs': ['tos.claim.derivation']},
                       {'variant_labels': [{'value': 'Accepted', 'status': 'verified'}]}):
            with self.subTest(update=update):
                values = delta()
                values[2].update(update)
                with self.assertRaises(BibliographicTopologyError):
                    self.check_delta(values)

    def test_claim_endpoint_version_evidence_and_assessment_must_remain_exact(self):
        for update in ({'object': 'tos.expression.other'}, {'subject_ref': 'tos.work.other'},
                       {'predicate': 'embodied_by'}, {'claim_version': 2}, {'claim_version': True},
                       {'epistemic_status': 'inferred'}, {'polarity': 'negative'}, {'polarity': None},
                       {'confidence': 0.9},
                       {'review_status': 'accepted'}, {'assessment_refs': [{'id': 'accepted'}]},
                       {'evidence_refs': [WORK_PATH]}, {'evidence_refs': [WORK_PATH, EXPRESSION_PATH, 'extra']},
                       {'evidence_refs': [WORK_PATH, WORK_PATH, EXPRESSION_PATH]}):
            with self.subTest(update=update):
                values = delta()
                values[3].update(update)
                with self.assertRaises(BibliographicTopologyError):
                    self.check_delta(values)

    def test_expression_home_cannot_escape_or_change_work_parent(self):
        for path in (EXPRESSION_PATH.replace('/new/', '/../'), '/absolute/expression.json',
                     EXPRESSION_PATH.replace('/example/', '/other/'),
                     EXPRESSION_PATH.replace('/new/', '/.hidden/')):
            with self.subTest(path=path), self.assertRaises(BibliographicTopologyError):
                self.check_delta(delta(), expression=path)

    def test_standalone_public_private_and_revision_scopes_refuse_even_replays(self):
        claim = delta()[3]
        profiles = SimpleNamespace(profiles={'has_expression': {'reader': 'identity-relation-v1'}})
        with self.assertRaisesRegex(PermissionError, 'compound bibliographic'):
            source_claim_commands._scope({'allowed_operations': ['claims.create']}, [claim], profiles=profiles)
        for initial in (True, False):
            with self.subTest(initial=initial), self.assertRaisesRegex(PermissionError, 'compound bibliographic'):
                source_owner_claim_commands._scope({}, [claim], profiles, initial=initial)
        with self.assertRaisesRegex(PermissionError, 'compound bibliographic'):
            claim_revisions._scope({'allowed_operations': ['claim.revise']}, {}, claim, profiles=profiles)


class BibliographicClosureTests(unittest.TestCase):
    def test_current_union_closes_without_rewriting_legacy_counts(self):
        records, legacy, new_claim, items = topology()
        counts = _legacy_topology_configuration(legacy)
        untouched = copy.deepcopy(legacy)
        self.assertEqual({'has_expression': 2, 'embodied_by': 1, 'exemplified_by': 1},
            validate_current_topology(records, [*legacy, new_claim], item_edition_by_id=items))
        self.assertEqual(counts, _legacy_topology_configuration(legacy))
        self.assertEqual(counts['work_expression_claims_materialized'], 1)
        self.assertEqual(legacy, untouched)

    def test_missing_extra_duplicate_and_conflicting_links_fail_closed(self):
        for mutation in ('missing-claim', 'extra-ref', 'duplicate-ref', 'duplicate-pair', 'two-works',
                         'wrong-expression-backlink', 'missing-item', 'wrong-edition', 'missing-record'):
            with self.subTest(mutation=mutation):
                records, legacy, new_claim, items = topology()
                claims = [*legacy, new_claim]
                if mutation == 'missing-claim':
                    claims.pop()
                elif mutation == 'extra-ref':
                    records[WORK_ID]['expression_claim_refs'].append('tos.claim.absent')
                elif mutation == 'duplicate-ref':
                    records[WORK_ID]['expression_claim_refs'].append(CLAIM_ID)
                elif mutation == 'duplicate-pair':
                    claims.append({**new_claim, 'claim_id': 'tos.claim.duplicate'})
                    records[WORK_ID]['expression_claim_refs'].append('tos.claim.duplicate')
                elif mutation == 'two-works':
                    records['tos.work.other'] = {'record_id': 'tos.work.other', 'record_type': 'work',
                                                'expression_claim_refs': ['tos.claim.other']}
                    claims.append({**new_claim, 'claim_id': 'tos.claim.other', 'subject_ref': 'tos.work.other'})
                elif mutation == 'wrong-expression-backlink':
                    records[EXPRESSION_ID]['work_ref'] = 'tos.work.other'
                elif mutation == 'missing-item':
                    items.clear()
                elif mutation == 'wrong-edition':
                    items['tos.item.old'] = 'tos.edition.absent'
                else:
                    del records[EXPRESSION_ID]
                with self.assertRaises(BibliographicTopologyError):
                    validate_current_topology(records, claims, item_edition_by_id=items)

    def test_issue_collection_is_bounded(self):
        claims = [{'predicate': 'has_expression', 'claim_id': 'tos.claim.test',
                   'subject_ref': 'tos.work.absent', 'object': 'tos.expression.absent'}] * 100
        with self.assertRaises(BibliographicTopologyError) as caught:
            validate_current_topology({}, claims)
        self.assertEqual(len(caught.exception.issues), 64)


class BibliographicLegacyInputTests(unittest.TestCase):
    def result(self, ref=WORK_PATH):
        digest = hashlib.sha256(b'old source bytes').hexdigest()
        return digest, {'status': 'available', 'source_path': ref, 'requested_sha256': digest,
            'exact_ref': {'id': WORK_ID, 'version': 4, 'digest': 'sha256:' + 'a' * 64},
            'record': {'record_id': WORK_ID, 'record_version': 4},
            'provenance': {'source': {'source_ref': ref, 'record_sha256': 'sha256:' + digest,
                                      'archive_blob_ref': 'committed-retained-blob'}}}

    def test_exact_current_bytes_do_not_need_retained_resolution(self):
        reader = Mock()
        with patch('validate_source_witness_foundation._recorded_provenance_input_path', return_value=Path(WORK_PATH)):
            self.assertTrue(_topology_evidence_matches(ROOT, WORK_PATH,
                [{'ref': WORK_PATH, 'sha256': 'a' * 64}], reader))
        reader.resolve_source_bytes.assert_not_called()

    def test_retained_exact_version_is_resolved_by_original_path_and_raw_digest(self):
        digest, result = self.result()
        reader = Mock(resolve_source_bytes=Mock(return_value=result))
        with patch('validate_source_witness_foundation._recorded_provenance_input_path', return_value=None):
            self.assertTrue(_topology_evidence_matches(ROOT, WORK_PATH,
                [{'ref': WORK_PATH, 'sha256': digest}], reader))
        reader.resolve_source_bytes.assert_called_once_with(WORK_PATH, digest)

    def test_unavailable_or_misbound_retained_evidence_does_not_substitute_current(self):
        for mutation in ('missing', 'corrupt', 'wrong-path', 'wrong-hash', 'wrong-id', 'two-inputs'):
            with self.subTest(mutation=mutation):
                digest, result = self.result()
                inputs = [{'ref': WORK_PATH, 'sha256': digest}]
                if mutation in {'missing', 'corrupt'}:
                    result = {'status': mutation, 'record': None, 'provenance': None}
                elif mutation == 'wrong-path':
                    result['provenance']['source']['source_ref'] = EXPRESSION_PATH
                elif mutation == 'wrong-hash':
                    result['provenance']['source']['record_sha256'] = 'sha256:' + 'b' * 64
                elif mutation == 'wrong-id':
                    result['exact_ref']['id'] = 'tos.work.other'
                else:
                    inputs.append(inputs[0])
                reader = Mock(resolve_source_bytes=Mock(return_value=result))
                with patch('validate_source_witness_foundation._recorded_provenance_input_path', return_value=None):
                    self.assertFalse(_topology_evidence_matches(ROOT, WORK_PATH, inputs, reader))

    def test_unsupported_legacy_inputs_do_not_search_a_retained_blob(self):
        reader = Mock()
        for ref in ('ToS/source-witnesses/works/example/edition.json',
                    'ToS/source-witnesses/works/example/item-manifest.json'):
            with self.subTest(ref=ref), patch('validate_source_witness_foundation._recorded_provenance_input_path', return_value=None):
                self.assertFalse(_topology_evidence_matches(ROOT, ref, [{'ref': ref, 'sha256': 'a' * 64}], reader))
        reader.resolve_source_bytes.assert_not_called()
