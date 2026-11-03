"""Regression for ledger paths containing literal dots in JSON keys."""
import unittest

from apply_language import parse_path, string_spans


class LedgerPathTest(unittest.TestCase):
    def test_literal_dots_select_one_key_instead_of_nested_fields(self):
        route = r'bonsai/antiquity/places.json:situations.tos\.historical-process\.example.statement'
        filename, key = parse_path(route)
        spans = string_spans('{"situations":{"tos.historical-process.example":{"statement":{"ru":"текст"}}}}')
        self.assertEqual(filename, 'bonsai/antiquity/places.json')
        self.assertEqual(spans[key + ('ru',)][2], 'текст')
        self.assertEqual(parse_path('bonsai/antiquity/collection.json:branches[2].title')[1],
                         ('branches', 2, 'title'))

    def test_outside_module_and_invalid_escape_are_rejected(self):
        for route in ('other/file.json:record', r'bonsai/antiquity/places.json:bad\escape'):
            with self.assertRaises(ValueError):
                parse_path(route)


if __name__ == '__main__':
    unittest.main()
