import importlib.util
import json
import sqlite3
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest import mock


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "scripts" / "validate_active_naming.py"
SPEC = importlib.util.spec_from_file_location("validate_active_naming", SCRIPT_PATH)
validate_active_naming = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(validate_active_naming)


def retired_s_token() -> str:
    return "s" + "eed"


def retired_w_token() -> str:
    return "w" + "ave"


def quoted_a48_artifact_identity() -> str:
    return (
        "ToS Deep Research_ A48 — Океания _ khipu _ rongorongo as frontier "
        + retired_s_token()
        + ".docx"
    )


def quoted_capture_provenance_fragment() -> str:
    return next(iter(validate_active_naming.QUOTED_CAPTURE_PROVENANCE_FRAGMENTS))


def active_reference(text: str) -> str | None:
    match = validate_active_naming.ACTIVE_REFERENCE_PATTERN.search(text)
    return match.group(0) if match else None


def active_content_reference(text: str) -> str | None:
    for match in validate_active_naming.ACTIVE_REFERENCE_PATTERN.finditer(text):
        reference = match.group(0)
        if reference.lower() not in validate_active_naming.ALLOWED_ACTIVE_CONTENT_REFERENCES:
            return reference
    return None


def old_route_prefix() -> str:
    return "z" + "v"


def old_experience_version(value: str) -> str:
    return "v" + "0." + value


def old_experience_ref() -> str:
    return "experience." + old_experience_version("7") + ".adoption" + "_forge"


class ValidateActiveNamingTests(unittest.TestCase):
    def test_lens_selection_fields_are_content_not_retired_routes(self) -> None:
        schema = json.loads((SCRIPT_PATH.parents[1] / "access/contracts/lens-spec.v1.schema.json").read_text())
        token = retired_s_token()
        fields = [token + "." + name for name in schema['properties'][token]['properties']]
        fields.append(token + '_field')  # Existing catalog field points to the LensSpec center.
        for reference in fields:
            with self.subTest(reference=reference):
                self.assertIsNone(validate_active_naming.retired_content_issue(reference))
                self.assertIsNotNone(validate_active_naming.retired_path_issue('ToS/' + reference + '/item.json'))
                self.assertIsNotNone(validate_active_naming.retired_content_issue(reference + '_retired'))

    def test_repository_kag_family_is_outside_authored_naming_checks(self) -> None:
        for relative_path in (
            "kag/indexes/index_family.manifest.json",
            "kag/indexes/source_surface_index.json",
            "kag/indexes/repo_artifact_index.json",
            "kag/indexes/repo_anchor_index.json",
            "kag/indexes/repo_entity_index.json",
            "kag/indexes/repo_event_index.json",
            "kag/indexes/repo_assertion_index.json",
            "kag/indexes/repo_relation_index.json",
            "kag/indexes/segments/source/00.jsonl",
            "kag/indexes/segments/event/0.jsonl",
            "kag/receipts/index_family_budget/digest.json",
        ):
            with self.subTest(relative_path=relative_path):
                self.assertTrue(
                    validate_active_naming.is_excluded(
                        validate_active_naming.REPO_ROOT / relative_path
                    )
                )

    def test_documentation_currentness_projection_is_not_active_naming_authority(self) -> None:
        self.assertTrue(
            validate_active_naming.is_excluded(
                validate_active_naming.REPO_ROOT / "docs/validation/documentation-family.current.json"
            )
        )

    def test_web_dependency_install_and_lock_are_not_authored_naming_authority(self) -> None:
        for relative_path in (
            "access/web/node_modules/example/package.json",
            "access/web/package-lock.json",
        ):
            with self.subTest(relative_path=relative_path):
                self.assertTrue(
                    validate_active_naming.is_excluded(
                        validate_active_naming.REPO_ROOT / relative_path
                    )
                )
        self.assertFalse(
            validate_active_naming.is_excluded(
                validate_active_naming.REPO_ROOT / "access/web/package.json"
            )
        )

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

    def test_linear_active_reference_matches_legacy_boundaries(self) -> None:
        old_s = retired_s_token()
        old_w = retired_w_token()
        cases = (
            old_s + ".",
            old_s + ".v1",
            old_s + ".Ω",
            old_s + "١",
            old_s + "١-" + old_w,
            "ſ" + "eed-pack",
            "w" + "ı" + "ve-pack",
            "seed_claim_ref",
            "first-wave-resident",
            "x-" * 64 + "seed_claim_ref " + "x_" * 64 + old_w + "-pack",
            "İıK/ſ" + "EED-pack",
            "İ Ω " + old_s + "-pack",
            "ſ" + "EED_CLAIM_REF " + old_w + "-pack",
            old_s + ". " + old_w + "-pack",
            old_s + "_claim_ref/" + old_w + "-pack",
            old_s + "_claim_ref " + old_w + "١",
        )
        for text in cases:
            with self.subTest(text=text):
                self.assertEqual(
                    validate_active_naming.active_reference_issue(text),
                    active_content_reference(text),
                )

    def test_exact_corpus_domain_identifiers_are_not_retired_routes(self) -> None:
        for reference in (
            "seed_claim_ref",
            "may_seed_drafts",
            "may_seed_gold",
            "first-wave",
            "first-wave-resident",
        ):
            with self.subTest(reference=reference):
                self.assertIsNotNone(active_reference(reference))
                self.assertIsNone(validate_active_naming.retired_content_issue(reference))

    def test_domain_identifier_exceptions_are_exact_and_content_only(self) -> None:
        for reference in (
            retired_s_token() + "_claim_route",
            "may_" + retired_s_token() + "_runtime",
            "second-" + retired_w_token() + "-resident",
            "first-" + retired_w_token() + "-runtime",
        ):
            with self.subTest(reference=reference):
                self.assertIsNotNone(validate_active_naming.retired_content_issue(reference))

        for relative_path in (
            "ToS/" + retired_s_token() + "_claim_ref/record.json",
            "ToS/may_" + retired_s_token() + "_gold/record.json",
            "ToS/first-" + retired_w_token() + "/record.json",
        ):
            with self.subTest(relative_path=relative_path):
                self.assertIsNotNone(validate_active_naming.retired_path_issue(relative_path))

    def test_exact_external_artifact_identity_is_quoted_provenance(self) -> None:
        reference = quoted_a48_artifact_identity()
        self.assertIsNotNone(active_reference(reference))
        self.assertIsNone(validate_active_naming.retired_content_issue(reference))
        self.assertIsNotNone(validate_active_naming.retired_path_issue(reference))

    def test_external_artifact_identity_exception_rejects_near_misses(self) -> None:
        reference = quoted_a48_artifact_identity()
        for near_miss in (
            reference.removesuffix(".docx") + "-copy.docx",
            reference.replace("A48", "A49"),
            "frontier " + retired_s_token() + ".docx",
        ):
            with self.subTest(near_miss=near_miss):
                self.assertIsNotNone(validate_active_naming.retired_content_issue(near_miss))

    def test_exact_capture_provenance_fragment_is_content_only(self) -> None:
        reference = quoted_capture_provenance_fragment()
        self.assertIsNotNone(active_reference(reference))
        self.assertIsNone(validate_active_naming.retired_content_issue(reference))
        self.assertIsNotNone(validate_active_naming.retired_path_issue(reference))

    def test_capture_provenance_exception_rejects_near_misses_and_paths(self) -> None:
        reference = quoted_capture_provenance_fragment()
        for near_miss in (
            reference.replace("master-" + retired_s_token(), "master-" + retired_s_token() + "s"),
            reference.replace("Bentham", "Bentham's"),
            reference.removesuffix("."),
        ):
            with self.subTest(near_miss=near_miss):
                self.assertIsNotNone(validate_active_naming.retired_content_issue(near_miss))
        self.assertIsNotNone(
            validate_active_naming.retired_path_issue(
                "docs/" + reference.replace(" ", "-") + ".md"
            )
        )

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

    def test_validate_prunes_excluded_trees_and_checks_directory_paths(self) -> None:
        with TemporaryDirectory(prefix="tos-active-naming-") as raw_root:
            root = Path(raw_root)
            (root / "legacy" / retired_w_token()).mkdir(parents=True)
            (root / "legacy" / retired_w_token() / "ignored.md").write_text(
                retired_w_token() + "-pack\n",
                encoding="utf-8",
            )
            retired_dir = root / "mechanics" / "experience" / old_experience_version("7")
            retired_dir.mkdir(parents=True)
            retired_dir.joinpath("README.md").write_text("clean\n", encoding="utf-8")

            original_root = validate_active_naming.REPO_ROOT
            validate_active_naming.REPO_ROOT = root
            try:
                issues = validate_active_naming.validate()
            finally:
                validate_active_naming.REPO_ROOT = original_root

        self.assertTrue(any(old_experience_version("7") in issue for issue in issues))
        self.assertFalse(any("legacy" in issue for issue in issues))

    def test_feedback_cache_reuses_content_but_keeps_paths_and_edits_live(self) -> None:
        with TemporaryDirectory(prefix="tos-active-naming-cache-") as raw_root:
            root = Path(raw_root)
            clean = root / "clean.txt"
            bad = root / "bad.txt"
            clean.write_text("plain-text", encoding="utf-8")
            retired_content = retired_s_token() + "-route"
            bad.write_text(retired_content, encoding="utf-8")
            retired_dir = root / "mechanics" / "experience" / old_experience_version("7")
            retired_dir.mkdir(parents=True)
            cache_path = root.parent / f"{root.name}-cache.sqlite"

            original_root = validate_active_naming.REPO_ROOT
            original_check = validate_active_naming.retired_content_issue
            validate_active_naming.REPO_ROOT = root
            try:
                with mock.patch.object(
                    validate_active_naming,
                    "retired_content_issue",
                    wraps=original_check,
                ) as check:
                    first = validate_active_naming.validate(feedback_cache=cache_path)
                self.assertEqual(check.call_count, 2)

                with mock.patch.object(
                    validate_active_naming,
                    "retired_content_issue",
                    wraps=original_check,
                ) as check:
                    second = validate_active_naming.validate(feedback_cache=cache_path)
                self.assertEqual(first, second)
                self.assertEqual(check.call_count, 0)
                self.assertTrue(any(old_experience_version("7") in issue for issue in second))

                bad.write_text("fixed-text", encoding="utf-8")
                with mock.patch.object(
                    validate_active_naming,
                    "retired_content_issue",
                    wraps=original_check,
                ) as check:
                    repaired = validate_active_naming.validate(feedback_cache=cache_path)
                self.assertEqual(check.call_count, 1)
                self.assertFalse(any("bad.txt" in issue for issue in repaired))

                renamed_name = retired_s_token() + "-route.txt"
                renamed = root / renamed_name
                bad.rename(renamed)
                renamed_result = validate_active_naming.validate(feedback_cache=cache_path)
                self.assertTrue(any(renamed_name in issue for issue in renamed_result))
            finally:
                validate_active_naming.REPO_ROOT = original_root
                cache_path.unlink(missing_ok=True)

    def test_feedback_cache_recomputes_corrupt_rows_and_rejects_repository_paths(self) -> None:
        with TemporaryDirectory(prefix="tos-active-naming-cache-corrupt-") as raw_root:
            root = Path(raw_root)
            bad = root / "bad.txt"
            retired_content = retired_s_token() + "-route"
            bad.write_text(retired_content, encoding="utf-8")
            cache_path = root.parent / f"{root.name}-corrupt.sqlite"

            original_root = validate_active_naming.REPO_ROOT
            original_check = validate_active_naming.retired_content_issue
            validate_active_naming.REPO_ROOT = root
            try:
                self.assertTrue(validate_active_naming.validate(feedback_cache=cache_path))
                cache = validate_active_naming.FeedbackContentCache(cache_path)
                policy = cache.policy
                digest = cache.content_digest(retired_content)
                cache.close()
                with sqlite3.connect(cache_path) as connection:
                    connection.execute(
                        "UPDATE content_results SET result_json=? WHERE policy=? AND content_digest=?",
                        ("not-json", policy, digest),
                    )

                with mock.patch.object(
                    validate_active_naming,
                    "retired_content_issue",
                    wraps=original_check,
                ) as check:
                    issues = validate_active_naming.validate(feedback_cache=cache_path)
                self.assertEqual(check.call_count, 1)
                self.assertTrue(any("bad.txt" in issue for issue in issues))
                with self.assertRaises(ValueError):
                    validate_active_naming.validate(feedback_cache=root / "inside.sqlite")

                cache_path.unlink()
                self.assertTrue(validate_active_naming.validate(feedback_cache=cache_path))
                cache = validate_active_naming.FeedbackContentCache(cache_path)
                policy = cache.policy
                digest = cache.content_digest(retired_content)
                cache.close()
                with sqlite3.connect(cache_path) as connection:
                    connection.execute(
                        "UPDATE content_results SET result_json=? WHERE policy=? AND content_digest=?",
                        (sqlite3.Binary(b"\xff"), policy, digest),
                    )

                with mock.patch.object(
                    validate_active_naming,
                    "retired_content_issue",
                    wraps=original_check,
                ) as check:
                    issues = validate_active_naming.validate(feedback_cache=cache_path)
                self.assertEqual(check.call_count, 1)
                self.assertTrue(any("bad.txt" in issue for issue in issues))

                cache_path.write_bytes(b"not a sqlite database")
                with mock.patch.object(
                    validate_active_naming,
                    "retired_content_issue",
                    wraps=original_check,
                ) as check:
                    issues = validate_active_naming.validate(feedback_cache=cache_path)
                self.assertEqual(check.call_count, 1)
                self.assertTrue(any("bad.txt" in issue for issue in issues))
            finally:
                validate_active_naming.REPO_ROOT = original_root
                cache_path.unlink(missing_ok=True)

    def test_feedback_cache_policy_binds_validator_source_and_python(self) -> None:
        with TemporaryDirectory(prefix="tos-active-naming-cache-policy-") as raw_root:
            cache_path = Path(raw_root) / "policy.sqlite"
            first = validate_active_naming.FeedbackContentCache(cache_path)
            first_policy = first.policy
            first.close()
            with mock.patch.object(Path, "read_bytes", return_value=b"different-validator"):
                second = validate_active_naming.FeedbackContentCache(cache_path)
                second_policy = second.policy
                second.close()
            self.assertNotEqual(first_policy, second_policy)

    def test_feedback_cache_lock_falls_back_without_waiting(self) -> None:
        with TemporaryDirectory(prefix="tos-active-naming-cache-lock-") as raw_root:
            root = Path(raw_root)
            bad = root / "bad.txt"
            retired_content = retired_s_token() + "-route"
            bad.write_text(retired_content, encoding="utf-8")
            cache_path = root.parent / f"{root.name}-lock.sqlite"

            validate_active_naming.FeedbackContentCache(cache_path).close()
            holder = sqlite3.connect(cache_path, timeout=0.0)
            holder.execute("BEGIN EXCLUSIVE")
            original_root = validate_active_naming.REPO_ROOT
            original_check = validate_active_naming.retired_content_issue
            validate_active_naming.REPO_ROOT = root
            try:
                with mock.patch.object(
                    validate_active_naming.sqlite3,
                    "connect",
                    wraps=sqlite3.connect,
                ) as connect:
                    with mock.patch.object(
                        validate_active_naming,
                        "retired_content_issue",
                        wraps=original_check,
                    ) as check:
                        issues = validate_active_naming.validate(feedback_cache=cache_path)
                self.assertEqual(connect.call_args.kwargs["timeout"], 0.0)
                self.assertEqual(check.call_count, 1)
                self.assertTrue(any("bad.txt" in issue for issue in issues))
            finally:
                validate_active_naming.REPO_ROOT = original_root
                holder.rollback()
                holder.close()
                cache_path.unlink(missing_ok=True)

    def test_feedback_cache_closes_connection_when_setup_fails(self) -> None:
        class TrackingConnection:
            def __init__(self, *, fail_execute: bool = False) -> None:
                self.fail_execute = fail_execute
                self.closed = False

            def execute(self, *args: object) -> "TrackingConnection":
                if self.fail_execute:
                    raise sqlite3.OperationalError("schema setup failed")
                return self

            def commit(self) -> None:
                return None

            def close(self) -> None:
                self.closed = True

        schema_connection = TrackingConnection(fail_execute=True)
        with mock.patch.object(
            validate_active_naming.sqlite3,
            "connect",
            return_value=schema_connection,
        ):
            with self.assertRaises(sqlite3.OperationalError):
                validate_active_naming.FeedbackContentCache(Path("unused-schema.sqlite"))
        self.assertTrue(schema_connection.closed)

        policy_connection = TrackingConnection()
        with mock.patch.object(
            validate_active_naming.sqlite3,
            "connect",
            return_value=policy_connection,
        ), mock.patch.object(
            Path,
            "read_bytes",
            side_effect=OSError("policy source unavailable"),
        ):
            with self.assertRaises(OSError):
                validate_active_naming.FeedbackContentCache(Path("unused-policy.sqlite"))
        self.assertTrue(policy_connection.closed)

    def test_mechanics_topology_checks_active_targets_not_historical_keys(self) -> None:
        retired_path = "ToS/doctrine/NO_DIRECT_" + "CONSTITUTION" + "_" + "RUNTIME" + "_WRITE.md"
        active_target = "mechanics/experience/parts/write-guards/docs/NO_DIRECT_GOVERNANCE_RUNTIME_WRITE.md"
        payload = {
            "schema_version": "tos_mechanics_topology_v2",
            "owner_repo": "Tree-of-Sophia",
            "root": "mechanics/",
            "legacy_policy": "package-local-only-when-active-route-has-moved-path-or-raw-receipt-accounting",
            "packages": [
                {
                    "slug": "experience",
                    "class": "head-fed/local",
                    "status": "active",
                    "active_parts": ["write-guards"],
                    "legacy_required": True,
                }
            ],
            "moved_path_accounting": {"experience": {"write-guards": [retired_path]}},
            "moved_path_targets": {retired_path: active_target},
        }
        text = validate_active_naming.active_content_text(
            validate_active_naming.MECHANICS_TOPOLOGY_ROUTE,
            json.dumps(payload),
        )

        self.assertIsNone(validate_active_naming.retired_content_issue(text))


if __name__ == "__main__":
    unittest.main()
