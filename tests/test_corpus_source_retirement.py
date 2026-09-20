"""Source lifecycle events must bind the reviewed operation, not arbitrary bytes."""
from __future__ import annotations

from contextlib import contextmanager
import copy
import hashlib
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))

import build_source_witness_catalog  # noqa: E402
import corpus_source_retirement  # noqa: E402
import corpus_source_validation  # noqa: E402
from corpus_source_retirement import (  # noqa: E402
    SCHEMA_REF,
    validate_retirements,
)
from corpus_store import CorpusStore, CorpusStoreError, ValidationIndex, canonical  # noqa: E402

SOURCE = 'ToS/source-witnesses/records/old.md'
REVIEW = 'ToS/review-ledger/source-retirement-review.md'
EVENT = 'ToS/source-witnesses/retirements/old-source.json'
SURVIVING = 'ToS/source-witnesses/records/surviving.md'
UNRELATED = 'ToS/source-witnesses/records/unrelated.md'
EXTRA_UNRELATED = 'ToS/source-witnesses/records/new-unrelated.md'


class SourceRetirementTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='tos-retirement-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.store = CorpusStore(self.root / 'store')
        self.validator_sha256 = 'a' * 64
        updates = {
            SCHEMA_REF: self.update(SCHEMA_REF, (ROOT / SCHEMA_REF).read_bytes()),
            SOURCE: self.update(SOURCE, b'Original source bytes retained in history.\n'),
            REVIEW: self.update(REVIEW, b'Synthetic source-owner review; no production approval.\n'),
        }
        self.base = self.store.admit(base_revision=None, updates=updates, retirements={},
            validator_sha256=self.validator_sha256, validate=self.validate)
        self.files = {entry['path']: entry for entry in self.base['files']}

    def update(self, path, raw):
        source = self.root / 'input' / path
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(raw)
        return {'source': source, 'sha256': hashlib.sha256(raw).hexdigest(),
                'size_bytes': len(raw), 'mode': 0o644}

    @staticmethod
    def validate(candidate, base, affected):
        identities = validate_retirements(candidate, base)
        if SOURCE in candidate.paths:
            identities['tos.source.fixture'] = SOURCE
        # Only the fixture transport's source identity is synthesized here;
        # actual retirement semantics are checked by the production function.
        return ValidationIndex(identities, {})

    def event(self):
        source_digest = self.files[SOURCE]['sha256']
        review_digest = self.files[REVIEW]['sha256']
        return {
            'schema_version': 'tos_provenance_event_v1',
            'event_id': 'tos.event.fixture-retirement', 'event_type': 'migration',
            'started_at': '2026-09-14T00:00:00Z', 'ended_at': '2026-09-14T00:01:00Z',
            'agent_refs': ['model:synthetic-test'],
            'inputs': [
                {'ref': SOURCE, 'role': 'retired_source', 'sha256': source_digest},
                {'ref': REVIEW, 'role': 'source_owner_review', 'sha256': review_digest},
            ],
            'outputs': [{'ref': EVENT, 'role': 'corpus_retirement_event'}],
            'method': {'maker_type': 'model', 'name': 'corpus-source-retirement', 'version': '1',
                'configuration': {'base_revision': self.base['revision'],
                    'retirements': [{'path': SOURCE, 'sha256': source_digest}],
                    'reason': 'Retain superseded fixture source in immutable history.',
                    'review_ref': REVIEW, 'review_sha256': review_digest}},
            'status': 'completed', 'event_version': 1, 'receipt_refs': [REVIEW],
        }

    def admit(self, event, *, raw=None, extra_updates=None):
        update = self.update(EVENT, canonical(event) if raw is None else raw)
        return self.store.admit(base_revision=self.base['revision'],
            updates={EVENT: update, **(extra_updates or {})},
            retirements={SOURCE: {'event_ref': EVENT, 'event_sha256': update['sha256']}},
            validator_sha256=self.validator_sha256, validate=self.validate)

    def _actual_source_validator_fixture(self, *, incoming=False, name='actual'):
        """Build a transport-only accepted base for the real source adapter.

        The IDs and dependency map are deliberately explicit synthetic facts;
        this helper does not use the production source scan as a baseline.
        """
        fixture_root = self.root / name
        store = CorpusStore(fixture_root / 'store')
        grammar_root = fixture_root / 'grammar'
        grammar_schema = grammar_root / SCHEMA_REF
        grammar_schema.parent.mkdir(parents=True, exist_ok=True)
        grammar_schema.write_bytes((ROOT / SCHEMA_REF).read_bytes())
        validator = corpus_source_validation.SourceValidator(grammar_root)

        source_bytes = b'Original source bytes retained in history.\n'
        review_bytes = b'Synthetic source-owner review; no production approval.\n'
        updates = {
            SCHEMA_REF: self.update(SCHEMA_REF, (ROOT / SCHEMA_REF).read_bytes()),
            SOURCE: self.update(SOURCE, source_bytes),
            SURVIVING: self.update(SURVIVING, b'Surviving synthetic source record.\n'),
            UNRELATED: self.update(UNRELATED, b'Unrelated synthetic source record.\n'),
        }
        identities = {
            'tos.synthetic.source.retiring': SOURCE,
            'tos.synthetic.source.surviving': SURVIVING,
            'tos.synthetic.source.unrelated': UNRELATED,
        }
        dependencies = {SURVIVING: [SOURCE]} if incoming else {}

        def transport_validate(candidate, base, affected):
            self.assertIsNone(base)
            self.assertEqual(set(candidate.paths), set(updates))
            self.assertEqual(affected, frozenset(updates))
            return ValidationIndex(identities, dependencies)

        base = store.admit(
            base_revision=None,
            updates=updates,
            retirements={},
            validator_sha256=validator.sha256,
            validate=transport_validate,
        )
        event = self.event()
        source_digest = hashlib.sha256(source_bytes).hexdigest()
        review_digest = hashlib.sha256(review_bytes).hexdigest()
        configuration = event['method']['configuration']
        configuration['base_revision'] = base['revision']
        configuration['retirements'][0]['sha256'] = source_digest
        configuration['review_sha256'] = review_digest
        event['inputs'][0]['sha256'] = source_digest
        event['inputs'][1]['sha256'] = review_digest
        review_update = self.update(REVIEW, review_bytes)
        event_update = self.update(EVENT, canonical(event))
        batch = {
            'base': base,
            'event': event,
            'event_update': event_update,
            'review_update': review_update,
            'updates': {EVENT: event_update, REVIEW: review_update},
            'retirements': {
                SOURCE: {'event_ref': EVENT, 'event_sha256': event_update['sha256']},
            },
            'store': store,
            'validator': validator,
            'files': {entry['path']: entry for entry in base['files']},
        }
        return batch

    @staticmethod
    def _forbidden_full_route_module():
        module = types.ModuleType('validate_source_witness_foundation')

        def validate_foundation(*args, **kwargs):
            raise AssertionError('full source audit unexpectedly executed')

        @contextmanager
        def source_snapshot_membership(*args, **kwargs):
            yield

        module.validate_foundation = validate_foundation
        module.source_snapshot_membership = source_snapshot_membership
        return module

    def _actual_retirement_batch(self, fixture, *, extra_updates=None):
        updates = dict(fixture['updates'])
        if extra_updates:
            updates.update(extra_updates)
        return fixture['store'].admit(
            base_revision=fixture['base']['revision'],
            updates=updates,
            retirements=fixture['retirements'],
            validator_sha256=fixture['validator'].sha256,
            validate=fixture['validator'],
        )

    def test_reviewed_event_retires_membership_and_retains_exact_history(self):
        result = self.admit(self.event())
        self.assertNotIn(SOURCE, {entry['path'] for entry in result['files']})
        self.assertEqual(result['identities']['tos.event.fixture-retirement'], EVENT)
        self.assertEqual(result['retirements'][0]['sha256'], self.files[SOURCE]['sha256'])
        self.assertEqual(self.store.load(result['revision'], verify_objects=True), result)
        self.store.restore(self.base['revision'], self.root / 'original')
        self.assertEqual((self.root / 'original' / SOURCE).read_bytes(),
                         b'Original source bytes retained in history.\n')

    def test_byte_bound_but_wrong_operation_or_review_cannot_advance_pointer(self):
        def configuration(event):
            return event['method']['configuration']
        mutations = {
            'ordinary provenance event': lambda e: e.update(event_type='annotation'),
            'failed event': lambda e: e.update(status='failed'),
            'wrong base': lambda e: configuration(e).update(base_revision='b' * 64),
            'different retired bytes': lambda e: configuration(e)['retirements'][0].update(sha256='b' * 64),
            'empty reason': lambda e: configuration(e).update(reason='  '),
            'wrong review bytes': lambda e: configuration(e).update(review_sha256='b' * 64),
            'unresolved review': lambda e: configuration(e).update(review_ref='ToS/review-ledger/absent.md'),
            'outside review owner': lambda e: configuration(e).update(review_ref=SOURCE),
            'unbound provenance': lambda e: e['inputs'].pop(),
            'different output': lambda e: e['outputs'][0].update(ref=SOURCE),
            'no actor': lambda e: e.update(agent_refs=[]),
            'reversed chronology': lambda e: e.update(ended_at='2026-09-13T00:00:00Z'),
        }
        for name, mutate in mutations.items():
            event = copy.deepcopy(self.event())
            mutate(event)
            with self.subTest(name=name), self.assertRaises(CorpusStoreError):
                self.admit(event)
            self.assertEqual(self.store.current(), self.base['revision'])

    def test_duplicate_json_or_updated_review_is_not_a_valid_exact_binding(self):
        raw = canonical(self.event()).replace(b'{', b'{"status":"failed",', 1)
        with self.assertRaises(CorpusStoreError):
            self.admit(self.event(), raw=raw)
        with self.assertRaises(CorpusStoreError):
            self.admit(self.event(), extra_updates={REVIEW: self.update(REVIEW, b'Changed review.\n')})
        self.assertEqual(self.store.current(), self.base['revision'])

    def test_damaged_retired_object_is_rejected_before_pointer_change(self):
        source = self.store._object(self.files[SOURCE]['sha256'])
        source.chmod(0o644)
        source.write_bytes(b'damaged\n')
        with self.assertRaisesRegex(CorpusStoreError, 'corrupt corpus object'):
            self.admit(self.event())
        self.assertEqual(self.store.current(), self.base['revision'])

    def test_actual_source_validator_uses_narrow_transition_without_unrelated_reads(self):
        fixture = self._actual_source_validator_fixture(name='actual-valid')
        store = fixture['store']
        unrelated = fixture['files'][UNRELATED]
        unrelated_object = store._object(unrelated['sha256'])
        unrelated_object.chmod(0o644)
        unrelated_object.write_bytes(b'corrupt unrelated object\n')

        verified_paths = []
        original_verify = store._verify_object

        def record_verify(entry):
            verified_paths.append(entry['path'])
            return original_verify(entry)

        forbidden = self._forbidden_full_route_module()
        with patch.object(store, '_verify_object', side_effect=record_verify), \
                patch.dict(sys.modules, {'validate_source_witness_foundation': forbidden}):
            result = self._actual_retirement_batch(fixture)

        self.assertEqual(result['validator_sha256'], fixture['validator'].sha256)
        self.assertEqual(store.current(), result['revision'])
        self.assertNotIn(UNRELATED, verified_paths)
        self.assertNotIn(SOURCE, {entry['path'] for entry in result['files']})
        self.assertIn(EVENT, {entry['path'] for entry in result['files']})
        self.assertIn(REVIEW, {entry['path'] for entry in result['files']})
        with self.assertRaisesRegex(CorpusStoreError, 'corrupt corpus object for .*unrelated'):
            store.load(result['revision'], verify_objects=True)

    def test_incoming_dependency_to_retired_source_rejects_and_keeps_pointer(self):
        fixture = self._actual_source_validator_fixture(incoming=True, name='actual-incoming')
        forbidden = self._forbidden_full_route_module()
        with patch.dict(sys.modules, {'validate_source_witness_foundation': forbidden}):
            with self.assertRaisesRegex(
                    CorpusStoreError,
                    'retirement leaves an incoming source dependency unresolved: '
                    + SURVIVING):
                self._actual_retirement_batch(fixture)
        self.assertEqual(fixture['store'].current(), fixture['base']['revision'])

    def test_broader_source_change_returns_to_full_owner_route(self):
        cases = {
            'changed surviving record': (SURVIVING, b'Changed surviving record.\n'),
            'added unrelated source': (EXTRA_UNRELATED, b'New unrelated source record.\n'),
        }
        for name, (extra_path, extra_bytes) in cases.items():
            fixture = self._actual_source_validator_fixture(name='actual-' + name.replace(' ', '-'))
            extra_updates = {extra_path: self.update(extra_path, extra_bytes)}
            observed_transitions = []
            real_transition = corpus_source_retirement.membership_transition

            def observe_transition(candidate, base, event_identities):
                result = real_transition(candidate, base, event_identities)
                observed_transitions.append(result)
                return result

            forbidden = self._forbidden_full_route_module()
            with self.subTest(name=name), \
                    patch.object(corpus_source_retirement, 'membership_transition',
                                 side_effect=observe_transition), \
                    patch.object(build_source_witness_catalog, 'render_outputs',
                                 return_value={}), \
                    patch.object(build_source_witness_catalog, 'write_outputs'), \
                    patch.dict(sys.modules, {'validate_source_witness_foundation': forbidden}):
                with self.assertRaisesRegex(
                        AssertionError, 'full source audit unexpectedly executed'):
                    self._actual_retirement_batch(fixture, extra_updates=extra_updates)
            self.assertEqual(observed_transitions, [None])
            self.assertEqual(fixture['store'].current(), fixture['base']['revision'])


if __name__ == '__main__':
    unittest.main()
