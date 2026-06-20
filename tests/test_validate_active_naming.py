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


def old_route_prefix() -> str:
    return "z" + "v"


def old_experience_version(value: str) -> str:
    return "v" + "0." + value


def old_experience_ref() -> str:
    return "experience." + old_experience_version("7") + ".adoption" + "_forge"


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

    def test_route_labels_and_experience_pass_markers_are_retired(self) -> None:
        cases = (
            old_route_prefix() + "1-old-route",
            old_experience_ref(),
            "deployment" + "-" + "watchtower",
            "federation " + "harvest",
            "Adoption " + "Forge",
            "CONSTITUTION" + "_" + "RUNTIME",
        )
        for text in cases:
            with self.subTest(text=text):
                self.assertIsNotNone(validate_active_naming.retired_content_issue(text))

    def test_experience_pass_markers_are_route_scoped(self) -> None:
        self.assertIsNotNone(validate_active_naming.retired_experience_pass_issue(old_experience_version("6")))
        self.assertIsNotNone(validate_active_naming.retired_experience_pass_issue(old_experience_version("8.0")))
        self.assertIsNone(validate_active_naming.retired_content_issue("Tree-of-Sophia v0.2.2"))
        self.assertIsNone(validate_active_naming.retired_content_issue("Tree-of-Sophia " + old_experience_version("6")))
        self.assertIsNone(validate_active_naming.retired_path_issue("mechanics/experience/parts/adoption-boundary/README.md"))


if __name__ == "__main__":
    unittest.main()
