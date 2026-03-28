from __future__ import annotations

import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from validate_intake_pack import run_validation  # noqa: E402


class IntakePackValidationTestCase(unittest.TestCase):
    def test_current_repo_intake_pack_validates(self) -> None:
        self.assertEqual(run_validation(REPO_ROOT), [])


if __name__ == "__main__":
    unittest.main()
