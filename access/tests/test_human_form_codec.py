"""Synthetic transport checks: no real source assessment or private fixture."""
import copy
import json
from pathlib import Path
import random
import shutil
import subprocess
import unittest

from tos_access.human_form_codec import (
    ROLES, WIRE_BUDGET, bounded_cost, decode_human_form_selection,
    encode_human_form_selection,
)
from tos_access.knowledge import select_human_forms


def ref(name):
    return {'id': 'tos.test.' + name, 'version': 1, 'digest': 'sha256:' + 'a' * 64}


def item(large=False):
    subject = ref('subject')
    context = [{'slot': 'mandatory', 'binding': {'record': subject, 'pointer': '/qualifiers'},
                'value': {'unreviewed': True, 'wording': 'Квалификация ' * (200 if large else 1)}}]
    forms = []
    for role in ROLES[:4]:
        forms.append({'schema_version': 'tos_human_form_materialization_v1', 'form': ref(role),
                      'subject': subject, 'state': 'ready', 'role': role, 'language': 'ru',
                      'script': 'Cyrl', 'display_text': role + ' wording', 'derivation': 'source-copy',
                      'context': copy.deepcopy(context), 'standalone_reading': False,
                      'performs_semantic_assessment': False,
                      'admission': {'limits': [('Research only ' * (23 if large else 1)), 'Not canon', role],
                                    'is_semantic_evaluation': False},
                      'unknown_future_key': {'null': None, 'empty': {}, 'false': False, 'zero': 0}})
    return {'entity_id': subject['id'], 'content_revision': 'b' * 64,
            'attributes': {'source_record': {'record_id': subject['id'], 'record_version': 1},
                           'source_sha256': 'a' * 64, 'human_forms': forms}}


def logical(value):
    result = select_human_forms(value)
    for packet in value['attributes']['human_forms']:
        result['roles'][packet['role']] = {'state': 'ready', 'reason': 'automatic', 'form': packet['form'], 'packet': packet}
    return result


class HumanFormCodecTests(unittest.TestCase):
    def test_string_cost_preserves_json_escaping_and_exact_budget_boundary(self):
        # The byte guard must keep its conservative Unicode/escaping semantics
        # when its string encoder changes; this is not a timing assertion.
        class StringSubclass(str):
            pass

        rng = random.Random(705)
        alphabet = ''.join(map(chr, range(32))) + '\\"' + 'αЖ詞🌌\ud800\udfff\u2028\u2029'
        values = ['', 'plain', StringSubclass('α\n"'), alphabet]
        values.extend(''.join(rng.choices(alphabet, k=rng.randrange(80))) for _ in range(300))
        for value in values:
            expected = len(json.dumps(value, ensure_ascii=False).encode('utf-8', errors='backslashreplace'))
            with self.subTest(value=repr(value)):
                self.assertEqual(bounded_cost(value, expected), expected)
                with self.assertRaises(ValueError):
                    bounded_cost(value, expected - 1)

    def test_exact_roundtrip_and_independent_objects(self):
        original = logical(item(True))
        wire = encode_human_form_selection(original)
        restored = decode_human_form_selection(json.loads(json.dumps(wire)))
        self.assertEqual(restored, original)
        self.assertLessEqual(bounded_cost(wire, WIRE_BUDGET), WIRE_BUDGET)
        restored['roles']['name']['packet']['context'][0]['value']['wording'] = 'changed'
        self.assertEqual(original['roles']['name']['packet']['context'], original['roles']['caption']['packet']['context'])
        self.assertNotEqual(restored['roles']['name']['packet']['context'], restored['roles']['caption']['packet']['context'])
        self.assertEqual(decode_human_form_selection(wire), original)

    def test_selection_keeps_v1_default_but_v2_delivers_all_four(self):
        source = item(True)
        original = copy.deepcopy(source)
        inline = select_human_forms(source)
        wire = select_human_forms(source, representation='shared-v2')
        self.assertEqual(inline['schema_version'], 'tos_human_form_selection_v1')
        self.assertLess(sum(role['state'] == 'ready' for role in inline['roles'].values()), 4)
        self.assertEqual(sum(role['state'] == 'ready' for role in wire['roles'].values()), 4)
        self.assertEqual(decode_human_form_selection(wire), logical(source))
        self.assertEqual(source, original)

    def test_absence_null_empty_unknown_and_order_survive(self):
        original = logical(item())
        packets = [original['roles'][role]['packet'] for role in ROLES[:4]]
        del packets[0]['admission']
        packets[1]['admission'] = None
        packets[2]['admission'] = {'unknown': []}
        packets[3]['admission']['limits'] = ['same', 'same', 'different', 'same']
        packets[0]['unknown_future_key'] = [False, 0, None, {}, []]
        self.assertEqual(decode_human_form_selection(encode_human_form_selection(original)), original)

    def test_invalid_wire_fails_closed(self):
        wire = encode_human_form_selection(logical(item()))
        mutations = [
            lambda value: value['packet_base'].update(context='collision'),
            lambda value: value['shared_limits'].append('unused'),
            lambda value: value['shared_limits'].append(value['shared_limits'][0]),
            lambda value: value['roles']['name'].update(packet_delta=None),
            lambda value: value['roles']['technical'].update(packet_delta={}),
            lambda value: value['roles'].update(eighth=value['roles']['name']),
            lambda value: value['packet_base'].update(__proto__={}),
            lambda value: value['packet_base'].update(large='x' * 16_384),
        ]
        # Explicit overlap is invalid even when it repeats the same value.
        mutations[0] = lambda value: value['roles']['name']['packet_delta'].update(context=value['packet_base']['context'])
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                corrupt = copy.deepcopy(wire)
                mutate(corrupt)
                with self.assertRaises(ValueError):
                    decode_human_form_selection(corrupt)
        for bad in [True, False, -1, 10000, 1.5, None, '0']:
            corrupt = copy.deepcopy(wire)
            corrupt['roles']['name']['packet_delta'].setdefault('admission', {})['limit_refs'] = [bad]
            with self.subTest(index=bad), self.assertRaises(ValueError):
                decode_human_form_selection(corrupt)

    def test_reserved_source_wire_keys_fail_closed(self):
        original = logical(item())
        original['roles']['name']['packet']['admission']['limit_refs'] = []
        with self.assertRaises(ValueError):
            encode_human_form_selection(original)
        source = item()
        source['attributes']['human_forms'][0]['admission']['limit_refs'] = []
        self.assertEqual(select_human_forms(source, representation='shared-v2')['state'], 'invalid')

    def test_role_form_factoring_requires_exact_identity_without_shadowing(self):
        original = logical(item())
        for change in [{'id': 'tos.test.other'}, {'version': 2}, {'digest': 'sha256:' + 'b' * 64},
                       {'version': True}, {'version': 1.0}, {'id': 42}, {'extra': 'forbidden'}]:
            value = copy.deepcopy(original)
            value['roles']['name']['form'] = {**value['roles']['name']['form'], **change}
            with self.subTest(change=change), self.assertRaises(ValueError):
                encode_human_form_selection(value)
        for bad in [None, {}, {'id': 'x', 'version': 1}, {'id': 'x', 'version': True, 'digest': 'sha256:' + 'a' * 64}]:
            for target in ['role', 'packet']:
                value = copy.deepcopy(original)
                selected = value['roles']['name']
                (selected if target == 'role' else selected['packet'])['form'] = bad
                with self.subTest(bad=bad, target=target), self.assertRaises(ValueError):
                    encode_human_form_selection(value)
            wire = encode_human_form_selection(original)
            wire['roles']['name']['form'] = bad
            with self.assertRaises(ValueError):
                decode_human_form_selection(wire)
        value = copy.deepcopy(original)
        del value['roles']['name']['packet']['form']
        with self.assertRaises(ValueError):
            encode_human_form_selection(value)
        wire = encode_human_form_selection(original)
        self.assertNotIn('form', wire['packet_base'])
        self.assertTrue(all('form' not in role['packet_delta'] for role in wire['roles'].values() if role['state'] == 'ready'))
        for target in ['base', 'delta']:
            corrupt = copy.deepcopy(wire)
            (corrupt['packet_base'] if target == 'base' else corrupt['roles']['name']['packet_delta'])['form'] = ref('name')
            with self.assertRaises(ValueError):
                decode_human_form_selection(corrupt)
        decoded = decode_human_form_selection(wire)
        self.assertIsNot(decoded['roles']['name']['form'], decoded['roles']['name']['packet']['form'])

    def test_cycles_and_structural_or_packet_expansion_are_bounded(self):
        cycle = []; cycle.append(cycle)
        with self.assertRaises(ValueError):
            bounded_cost(cycle, 524_288)
        deep = value = {}
        for _ in range(65):
            value['next'] = {}; value = value['next']
        with self.assertRaises(ValueError):
            bounded_cost(deep, 524_288)
        with self.assertRaises(ValueError):
            bounded_cost([None] * 30_001, 524_288)
        with self.assertRaises(ValueError):
            bounded_cost(float('nan'), 524_288)
        with self.assertRaises(ValueError):
            bounded_cost(10 ** 400, 524_288)
        source = item()
        source['attributes']['human_forms'][0]['future_number'] = 10 ** 400
        self.assertEqual(select_human_forms(source, representation='shared-v2')['state'], 'invalid')
        oversized = logical(item())
        oversized['roles']['name']['packet']['future'] = 'x' * 65_536
        with self.assertRaises(ValueError):
            encode_human_form_selection(oversized, enforce_budget=False)
        wire = encode_human_form_selection(logical(item()))
        wire['shared_limits'][0] = 'x' * 1000
        wire['roles']['name']['packet_delta']['admission']['limit_refs'] = [0] * 100
        self.assertLessEqual(bounded_cost(wire, WIRE_BUDGET), WIRE_BUDGET)
        with self.assertRaises(ValueError):
            decode_human_form_selection(wire)

    @unittest.skipUnless(shutil.which('node'), 'Node is required for cross-language transport parity')
    def test_seeded_cross_language_exact_roundtrips(self):
        rng = random.Random(705)
        cases = []
        for index in range(40):
            value = logical(item())
            for role in ROLES[:4]:
                packet = value['roles'][role]['packet']
                packet['unknown_future_key'] = {'same': {'nested': [None, False, 0, '🌌']},
                                                'variable': rng.choice([None, [], {}, {'x': [index, True]}])}
                if rng.choice([True, False]):
                    packet['unknown_future_key']['optional'] = None
                packet['admission'] = rng.choice([None, {}, {'limits': []}, {'limits': ['α', 'β', 'α', role]}])
            cases.append(value)
        path = Path(__file__).resolve().parents[1] / 'shared/human-form-selection-codec.ts'
        script = "import fs from 'node:fs';import {encodeHumanFormSelection as e,decodeHumanFormSelection as d} from " + json.dumps(path.as_uri()) + ";const cases=JSON.parse(fs.readFileSync(0,'utf8'));process.stdout.write(JSON.stringify(cases.map(v=>{const w=e(v);return [w,d(w)]})));"
        completed = subprocess.run(['node', '--experimental-strip-types', '--input-type=module', '-e', script],
                                   input=json.dumps(cases), text=True, capture_output=True, check=True, timeout=30)
        for original, (wire, restored) in zip(cases, json.loads(completed.stdout), strict=True):
            self.assertEqual(wire, encode_human_form_selection(original))
            self.assertEqual(restored, original)
            self.assertEqual(decode_human_form_selection(wire), original)


if __name__ == '__main__':
    unittest.main()
