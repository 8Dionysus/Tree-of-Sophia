"""Compact projection parity and explicit refusal of unavailable field queries."""
import copy
from dataclasses import replace
import json
import unittest

from tos_access import knowledge as k
from tos_access.compact_lens_carrier import compact_lens_carrier, supports_compact_lens_carrier
from tos_access.published_read_metadata import _compact, PublishedReadModelError
from test_human_form_codec import item as form_item


class CompactLensCarrierTests(unittest.TestCase):
    def source(self, forms=False):
        row = k._normalize_node({'node_id': 'example', 'label': 'Свобода', 'source_ref': 'test:compact',
                                 'properties': {'large': 'x' * 100000}}, 'philosophy')
        row.update(source_record={'raw': 'x' * 100000}, readable_context={'points_to': '/source_record'})
        row['extension'] = {'10': 2**53+1, '2': 1.0, 'negative_zero': -0.0, 'unknown': ['é', False, None]}
        row['semantics']['claim'] = {'source_canonical_json': 'x' * 100000, 'unreviewed': True}
        if forms:
            row.update(form_item())
            row['attributes']['human_forms'][0]['context'][0]['value']['native'] = copy.deepcopy(row['extension'])
        return row

    def test_compact_forms_values_order_and_language_match_full_reference(self):
        for forms in (False, True):
            row = self.source(forms)
            raw = _compact(row)
            carrier = compact_lens_carrier('node', raw)
            for language in ('auto', 'original', 'ru', 'en', 'de', 'ru-Cyrl'):
                with self.subTest(forms=forms, language=language):
                    expected = k._lens_carrier(row, 'compact', language=language)
                    self.assertEqual(_compact(carrier.render(language)), _compact(expected))
                    self.assertEqual(carrier.render(language)['attributes'], {})
            self.assertEqual(_compact(row), raw)
            self.assertNotIn('source_record', json.loads(carrier.seed_json))
            if not forms:
                self.assertLess(len(carrier.seed_json), len(raw)//10)

    def test_invalid_forms_are_preserved_as_invalid_not_replaced_with_missing(self):
        row = self.source(True)
        for value in (None, {'not': 'a-list'}, [None] * 33):
            row['attributes']['human_forms'] = value
            actual = compact_lens_carrier('node', _compact(row)).render('ru')
            self.assertEqual(_compact(actual), _compact(k._lens_carrier(row, 'compact', language='ru')))

    def test_carrier_corruption_and_source_bounds_refuse(self):
        carrier = compact_lens_carrier('node', _compact(self.source()))
        with self.assertRaises(PublishedReadModelError):
            replace(carrier, seed_json=carrier.seed_json+' ').render()
        with self.assertRaises(PublishedReadModelError):
            replace(carrier, identifier='other').render()
        with self.assertRaises(PublishedReadModelError):
            compact_lens_carrier('node', ' ' * 1048577)
        with self.assertRaises(PublishedReadModelError):
            compact_lens_carrier('node', '{"id":"a","id":"b","attributes":{}}')

    def test_omitted_fields_are_not_unknown_or_negative_query_results(self):
        base = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'compact', 'detail': 'compact'}
        self.assertTrue(supports_compact_lens_carrier(k.normalize_lens_spec(base)))
        for field in ('attributes.value', 'source_record', 'semantics', 'semantics.claim', 'semantics.claim.source_canonical_json'):
            for operation in ('exists', 'neq'):
                # Check the internal covered-field boundary independently of
                # today's wire field allowlist, including parent containers.
                spec = k.normalize_lens_spec(base)
                spec['node_query']['filters'] = [{'field': field, 'op': operation, 'value': False}]
                self.assertFalse(supports_compact_lens_carrier(spec), (field, operation))
        self.assertFalse(supports_compact_lens_carrier(k.normalize_lens_spec({**base, 'detail': 'full'})))
        self.assertFalse(supports_compact_lens_carrier(k.normalize_lens_spec({**base, 'seed': {'text_query': 'source'}})))
        for location in ('group', 'sort', 'path-node', 'path-relation'):
            spec = k.normalize_lens_spec(base)
            if location == 'group':
                spec['composition']['group_by'] = ['semantics.claim']
            elif location == 'sort':
                spec['composition']['sort_relations'] = [{'field':'attributes.value','direction':'asc'}]
            else:
                step = {'node_query':{'filters':[]}, 'relation_query':{'filters':[]}}
                step[location.removeprefix('path-') + '_query']['filters'] = [
                    {'field':'readable_context','op':'exists','value':False}]
                spec['path_query'] = [{'steps':[step]}]
            self.assertFalse(supports_compact_lens_carrier(spec), location)


if __name__ == '__main__':
    unittest.main()
