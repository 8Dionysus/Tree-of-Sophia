#!/usr/bin/env python3
"""The layer holds together: every reference resolves and the counts are those of the registers.

    python3 bonsai/review/aesthetics/check_layer.py
"""
import json
import unittest
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
layer = lambda name: json.loads((HERE / 'layer' / f'{name}.json').read_text(encoding='utf-8'))


def register_ids(letter: str, sheet: str) -> list[str]:
    """The IDs of a register sheet's rows: its first column."""
    lines = [json.loads(line) for line in (HERE / 'registry' / letter / f'{sheet}.jsonl').read_text(encoding='utf-8').splitlines()]
    return [row[lines[0]['columns'][0]] for row in lines[1:]]



class LayerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.areas, cls.concepts, cls.agents, cls.works = layer('areas'), layer('concepts'), layer('agents'), layer('works')
        cls.readings, cls.links, cls.sources, cls.keys = layer('readings'), layer('links'), layer('sources'), layer('keys')

    def test_every_register_row_is_in_the_layer_once(self):
        for sheet, records, own in (('readings', self.readings, lambda r: [r['key']]), ('links', self.links, lambda r: [r['key']]), ('sources', self.sources, lambda r: [r['key']]),
                                    ('concepts', self.concepts, lambda r: r['keys']), ('agents', self.agents, lambda r: r['keys']), ('works', self.works, lambda r: r['keys'])):
            held = Counter(key for record in records for key in own(record))
            self.assertEqual(set(held.values()), {1}, sheet)
            in_registers = {f'{letter}:{identifier}' for letter in 'ABCD' for identifier in register_ids(letter, sheet)}
            self.assertEqual(set(held), in_registers, sheet)

    def test_every_reference_resolves(self):
        areas = {area['id'] for area in self.areas}
        concepts = {concept['key'] for concept in self.concepts}
        agents = {agent['id'] for agent in self.agents}
        works = {work['id']: work for work in self.works}
        readings = {reading['key'] for reading in self.readings}
        sources = {source['key'] for source in self.sources}
        self.assertEqual(len(areas), 36)
        for work in works.values():
            self.assertTrue(work['agents'] and set(work['agents']) <= agents, work['id'])
            self.assertEqual(work['id'].split('.')[0], work['agents'][0])
        for reading in self.readings:
            at = reading['key']
            self.assertTrue(reading['areas'] and set(reading['areas']) <= areas, at)
            self.assertTrue(reading['concepts'] and set(reading['concepts']) <= concepts, at)
            self.assertIn(reading['work'], works, at)
            self.assertTrue(reading['agents'] and set(reading['agents']) <= agents, at)
            self.assertTrue(set(reading['sources']) <= sources, at)
            self.assertTrue(reading['locus'] and reading['url'] and next(iter(reading['thesis'].values())), at)
            self.assertNotIn('concepts_unresolved', reading)
            self.assertNotIn('authors_unresolved', reading)
        for link in self.links:
            for end in (link['from'], link['to']):
                (kind, target), = end.items()
                self.assertIn(target, {'concept': concepts, 'reading': readings}[kind], link['key'])
            self.assertTrue(link['grounds'] and set(link['grounds']) <= readings, link['key'])
            self.assertNotIn('grounds_unresolved', link)
        for concept in self.concepts:
            self.assertTrue(concept['areas'] and set(concept['areas']) <= areas, concept['key'])
        for key, target in self.keys.items():
            self.assertIn(target['id'], {'agent': agents, 'work': set(works), 'concept': concepts}[target['kind']], key)

    def test_identifiers_are_plain(self):
        for record in self.agents + self.works:
            self.assertRegex(record['id'], r'^[a-z0-9]+(?:[-.][a-z0-9]+)*$')

    def test_the_corrected_names_stand(self):
        names = {agent['id']: agent for agent in self.agents}
        for identifier, was in (('valentin-wagner', 'Vera Wagner'), ('gerhard-thonhauser', 'Shai Averbach'), ('ophelia-deroy', 'Étienne Deroy')):
            self.assertEqual([entry['original'] for entry in names[identifier]['register_names']], [was])
        self.assertNotIn('vera-wagner', names)


if __name__ == '__main__':
    unittest.main()
