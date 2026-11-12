import unittest
from unittest.mock import patch

import kit


class SecondReaderScanTest(unittest.TestCase):
    def test_same_page_number_in_another_volume_does_not_count(self):
        reading = {'reader': {'id': 'maker'}, 'conceptions': [], 'links': [], 'left': []}
        review = {'reader': {'id': 'second', 'model': 'model', 'date': '2026-11-10'},
                  'images': [{'loc': 'p', 'n': 5, 'reads': 'as the packet'}], 'accept': True}
        ev = {'scan': {'item': 'volume-a', 'image_offset': 0},
              'scans': {'volume-a': {'item': 'volume-a', 'image_offset': 0},
                        'volume-b': {'item': 'volume-b', 'image_offset': 0}},
              'places': {'p': {'scan': 'volume-b', 'method': 'image', 'from_scan': True, 'leaves': [5]}},
              'images': [{'item': 'volume-a', 'file': 'volume-a-n5.jpg', 'n': 5, 'by': 'second'}]}
        with patch.object(kit, 'part', return_value=review):
            problems = kit.second_gate('.', reading, ev, {'p': {}})
            self.assertTrue(any('every page' in problem for problem in problems))
            self.assertTrue(any('loc, n' in problem for problem in problems))
            ev['images'].append({'item': 'volume-b', 'file': 'volume-b-n5.jpg', 'n': 5, 'by': 'second'})
            self.assertEqual(kit.second_gate('.', reading, ev, {'p': {}}), [])


class LineFaultTest(unittest.TestCase):
    def test_a_planting_that_died_is_the_fault_of_the_line(self):
        died = ("node:fs:562\n  return binding.open(\n                 ^\n\n"
                "Error: ENOENT: no such file or directory, open 'documents/6cbc.json'\n"
                "    at Object.openSync (node:fs:562:18)\n    at readFileSync (node:fs:446:35)\n\nNode.js v22.11.0\n")
        self.assertEqual(kit.thrown(died), "Error: ENOENT: no such file or directory, open 'documents/6cbc.json'")

    def test_a_finding_of_the_planting_is_the_packet_s(self):
        self.assertIsNone(kit.thrown('B:B-I043: the formula is not found in the place\n'))


class EyeAloneTest(unittest.TestCase):
    def test_spelling_of_the_print_changes_no_letters(self):
        self.assertEqual(kit.changed('Per affectum intelligo corporis affectiones quibus coercetur', 'Per Affectum intelligo Corporis affectiones, quibus coërcetur'), [])
        self.assertEqual(kit.changed('Über adæquata', 'Ueber adaequata'), [])

    def test_a_changed_letter_is_named(self):
        self.assertEqual(kit.changed('adaequata possimus esse causa', 'adaequata possumus esse causa'), [('possimus', 'possumus')])

    def test_a_correction_in_a_place_read_from_the_image_rests_on_the_eye(self):
        places = {'work': 'w', 'corrections': [{'place': 'w p1', 'from': 'possimus', 'to': 'possumus'}, {'place': 'w p2', 'from': 'bei', 'to': 'bey'}]}
        ev = {'scan': {'item': 's'}, 'places': {'p1': {'method': 'image'}, 'p2': {'method': 'ocr', 'differences': []}}}
        self.assertEqual([loc for loc, _ in kit.by_eye(places, ev)], ['p1'])
        ev['places']['p2']['differences'] = [{'transcription': 'bey', 'ocr': 'bcy'}]
        self.assertEqual([loc for loc, _ in kit.by_eye(places, ev)], ['p1', 'p2'])

    def test_s_and_f_in_a_long_s_print_rest_on_the_eye(self):
        places = {'work': 'w', 'corrections': [{'place': 'w p1', 'from': 'seinen', 'to': 'feinen'}]}
        ev = {'scan': {'item': 's', 'long_s': True}, 'places': {'p1': {'method': 'ocr', 'differences': []}}}
        self.assertEqual([loc for loc, _ in kit.by_eye(places, ev)], ['p1'])


class WholeSentencesTest(unittest.TestCase):
    def test_a_quoted_sentence_is_a_whole_sentence(self):
        self.assertEqual(kit.sentences('‘Now, here, you see, it takes all the running you can do, to keep in the same place.’'), [])
        self.assertEqual(kit.sentences('„Das ist das Numinose.“'), [])
        self.assertEqual(kit.sentences('«Tout est dit.»'), [])

    def test_a_fragment_is_still_named(self):
        self.assertTrue(any('capital' in problem for problem in kit.sentences('‘and so on.’')))
        self.assertTrue(any('close a sentence' in problem for problem in kit.sentences('It takes all the running you can do’')))

    def test_a_place_may_run_to_2400_characters(self):
        self.assertEqual(kit.sentences('A' + 'a' * 2141 + '.'), [])
        self.assertTrue(kit.sentences('A' + 'a' * 2400 + '.'))


class QuotedPlaceTest(unittest.TestCase):
    def test_a_place_keeps_the_opening_mark_of_the_transcription(self):
        source = 'The Queen said this. ‘Now, here, you see, it takes all the running you can do, to keep in the same place.’ Alice looked.'
        self.assertIsNone(kit.in_transcription('‘Now, here, you see, it takes all the running you can do, to keep in the same place.’', source))
        source = 'Er schreibt: „Das ist das Numinose.“ Dann folgt mehr.'
        self.assertIsNone(kit.in_transcription('„Das ist das Numinose.“', source))

    def test_a_mark_the_transcription_lacks_is_still_named(self):
        source = 'The Queen said this. Now, here, you see, it takes all the running you can do, to keep in the same place. Alice looked.'
        self.assertIsNotNone(kit.in_transcription('‘Now, here, you see, it takes all the running you can do, to keep in the same place.’', source))


if __name__ == '__main__':
    unittest.main()
