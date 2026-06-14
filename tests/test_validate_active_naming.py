import importlib.util
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "scripts" / "validate_active_naming.py"
SPEC = importlib.util.spec_from_file_location("validate_active_naming", SCRIPT_PATH)
validate_active_naming = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(validate_active_naming)


def retired_s_token() -> str:
    return "s" + "eed"


def retired_w_token() -> str:
    return "w" + "ave"


def active_reference(text: str) -> str | None:
    match = validate_active_naming.ACTIVE_REFERENCE_PATTERN.search(text)
    return match.group(0) if match else None


class ValidateActiveNamingTests(unittest.TestCase):
    def test_terminal_sentence_period_is_not_path_or_id_marker(self) -> None:
        self.assertIsNone(active_reference(f"This was a {retired_s_token()}."))
        self.assertIsNone(active_reference(f"the next {retired_w_token()}."))

    def test_path_and_identifier_references_still_match(self) -> None:
        cases = (
            ("ToS/" + retired_s_token() + "/entry.md", "ToS/" + retired_s_token() + "/entry.md"),
            (retired_s_token() + "-pack", retired_s_token() + "-pack"),
            (retired_w_token() + "_route", retired_w_token() + "_route"),
            (retired_w_token() + ".v1", retired_w_token() + ".v1"),
            (retired_s_token() + "2", retired_s_token() + "2"),
        )
        for text, expected in cases:
            with self.subTest(text=text):
                self.assertEqual(active_reference(text), expected)


if __name__ == "__main__":
    unittest.main()
