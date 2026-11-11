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


if __name__ == '__main__':
    unittest.main()
