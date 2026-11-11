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


if __name__ == '__main__':
    unittest.main()
