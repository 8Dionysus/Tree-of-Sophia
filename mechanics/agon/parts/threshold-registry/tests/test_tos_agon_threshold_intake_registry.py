from __future__ import annotations

import json
import pathlib
import subprocess
import sys
import unittest

PART_ROOT = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = pathlib.Path(__file__).resolve().parents[5]
GENERATED = PART_ROOT / 'generated/tos_agon_threshold_intake_registry.min.json'
SOURCE = PART_ROOT / 'config/tos_agon_threshold_intakes.config.json'
SCRIPT = PART_ROOT / 'scripts/build_tos_agon_threshold_intake_registry.py'
VALIDATOR = PART_ROOT / 'scripts/validate_tos_agon_threshold_intake_registry.py'
ITEM_KEY = 'threshold_intakes'


class TosAgonThresholdIntakeRegistryTestCase(unittest.TestCase):
    def test_generated_registry_shape(self) -> None:
        reg = json.loads(GENERATED.read_text(encoding='utf-8'))
        source = json.loads(SOURCE.read_text(encoding='utf-8'))
        expected_count = len(source[ITEM_KEY])
        self.assertEqual(reg['review_phase_order'], 'XVIII')
        self.assertGreater(expected_count, 0)
        self.assertEqual(reg['count'], expected_count)
        self.assertEqual(len(reg[ITEM_KEY]), expected_count)
        self.assertTrue(all(item.get('live_protocol') is False for item in reg[ITEM_KEY]))
        self.assertTrue(all(item.get('review_status') == 'candidate_only' for item in reg[ITEM_KEY]))

    def test_builder_check_and_validator(self) -> None:
        builder = subprocess.run([sys.executable, str(SCRIPT), '--check'], cwd=str(REPO_ROOT), text=True, capture_output=True)
        self.assertEqual(builder.returncode, 0, builder.stderr)
        validator = subprocess.run([sys.executable, str(VALIDATOR)], cwd=str(REPO_ROOT), text=True, capture_output=True)
        self.assertEqual(validator.returncode, 0, validator.stderr)


if __name__ == '__main__':
    unittest.main()
