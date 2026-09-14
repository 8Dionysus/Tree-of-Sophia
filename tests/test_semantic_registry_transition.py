from __future__ import annotations

import contextlib
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPO_ROOT / "scripts/validate_semantic_registry_transition.py"
spec = importlib.util.spec_from_file_location("semantic_registry_transition", SCRIPT)
transition = importlib.util.module_from_spec(spec)
spec.loader.exec_module(transition)


class SemanticRegistryTransitionTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tos-registry-transition-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for ref in (*transition.REGISTRY_REFS, *transition.SCHEMA_REFS):
            target = self.root / ref
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((REPO_ROOT / ref).read_bytes())
        self.git("init", "--quiet")
        self.git("commit", "--quiet", "--allow-empty", "-m", "Synthetic pre-registry source history")
        self.initial = self.git("rev-parse", "HEAD").strip()
        self.git("add", "ToS")
        self.git("commit", "--quiet", "-m", "Synthetic immutable registry baseline")
        self.baseline = self.git("rev-parse", "HEAD").strip()
        self.registries = [json.loads((self.root / ref).read_text()) for ref in transition.REGISTRY_REFS]

    def git(self, *args):
        return subprocess.run(
            ["git", "-C", str(self.root), "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false",
             "-c", "user.name=ToS synthetic fixture", "-c", "user.email=fixture@example.invalid", *args],
            check=True, capture_output=True, text=True,
        ).stdout

    def save(self, registries):
        for ref, value in zip(transition.REGISTRY_REFS, registries):
            (self.root / ref).write_text(json.dumps(value, ensure_ascii=False))

    def profile(self, registries, index):
        entries_key, profile_key = (("types", "source_record_profile"), ("relations", "source_claim_profile"))[index]
        return next(entry[profile_key] for entry in registries[index][entries_key]
                    if profile_key in entry and (index == 0 or entry[profile_key]["reader"] == "semantic-relation-v1"))

    def extension(self, index):
        changed = copy.deepcopy(self.registries)
        profile = self.profile(changed, index)
        route = copy.deepcopy(profile["schemas"][0])
        route["schema_version"] += "_synthetic_extension"
        profile["schemas"].append(route)
        profile["profile_version"] += 1
        changed[index]["registry_version"] += 1
        return changed

    def test_missing_floating_short_zero_unknown_and_noncommit_baselines_fail_closed(self):
        for value in (None, "", "main", "HEAD^", self.baseline[:12], "0" * 40, "f" * 40):
            with self.subTest(baseline=value), self.assertRaises(ValueError):
                transition.validate_transition(self.root, value)
        blob = self.git("rev-parse", f"{self.baseline}:{transition.REGISTRY_REFS[0]}").strip()
        with self.assertRaisesRegex(ValueError, "not a commit object"):
            transition.validate_transition(self.root, blob)
        with mock.patch.dict(os.environ, {}, clear=True), contextlib.redirect_stderr(io.StringIO()) as errors:
            self.assertEqual(transition.main(["--repo-root", str(self.root)]), 1)
        self.assertIn(transition.BASELINE_ENV, errors.getvalue())
        self.assertIn("--baseline-commit FULL_COMMIT_OID", errors.getvalue())

    def test_unchanged_and_compatible_entity_and_claim_extensions_are_valid(self):
        unchanged = transition.validate_transition(self.root, self.baseline)
        self.assertTrue(unchanged["valid"], unchanged["violations"])
        self.assertEqual(unchanged["baseline_sha256"], unchanged["current_sha256"])
        self.assertFalse(unchanged["semantic_acceptance"])
        for index in (0, 1):
            with self.subTest(registry=index):
                self.save(self.extension(index))
                result = transition.validate_transition(self.root, self.baseline)
                self.assertTrue(result["valid"], result["violations"])
                self.assertEqual(result["baseline_commit"], self.baseline)
                self.assertNotEqual(result["baseline_sha256"], result["current_sha256"])

    def test_changed_profiles_and_registries_require_separate_version_bumps(self):
        for index in (0, 1):
            for missing in ("profile", "registry"):
                changed = self.extension(index)
                if missing == "profile":
                    self.profile(changed, index)["profile_version"] -= 1
                else:
                    changed[index]["registry_version"] -= 1
                self.save(changed)
                result = transition.validate_transition(self.root, self.baseline)
                with self.subTest(registry=index, missing=missing):
                    self.assertFalse(result["valid"])
                    self.assertIn(f"must increase {missing}_version", "; ".join(result["violations"]))

    def test_bumps_do_not_authorize_historical_route_removal_or_repurpose(self):
        for index in (0, 1):
            for action in ("remove", "repurpose"):
                changed = self.extension(index)
                routes = self.profile(changed, index)["schemas"]
                if action == "remove":
                    routes.pop(0)
                else:
                    routes[0]["schema_ref"] = "ToS/contracts/synthetic-repurposed.schema.json"
                self.save(changed)
                result = transition.validate_transition(self.root, self.baseline)
                with self.subTest(registry=index, action=action):
                    self.assertFalse(result["valid"])
                    self.assertIn("removed or repurposed a historical schema route", "; ".join(result["violations"]))

    def test_bumps_do_not_authorize_identity_or_reader_repurpose(self):
        for index, field, value in ((0, "id_prefix", "tos.synthetic-repurposed."),
                                    (0, "record_type", "synthetic-repurposed"),
                                    (1, "reader", "historical-temporal-v1")):
            changed = self.extension(index)
            self.profile(changed, index)[field] = value
            self.save(changed)
            result = transition.validate_transition(self.root, self.baseline)
            with self.subTest(registry=index, field=field):
                self.assertFalse(result["valid"])
                self.assertIn("explicit successor identity", "; ".join(result["violations"]))

    def test_baseline_is_not_replaced_by_newer_head_or_git_replace(self):
        invalid = self.extension(0)
        self.profile(invalid, 0)["schemas"].pop(0)
        self.save(invalid)
        self.git("add", "ToS")
        self.git("commit", "--quiet", "-m", "Synthetic incompatible current registry")
        current = self.git("rev-parse", "HEAD").strip()
        self.git("replace", self.baseline, current)
        result = transition.validate_transition(self.root, self.baseline)
        self.assertFalse(result["valid"])
        self.assertEqual(result["baseline_commit"], self.baseline)
        self.assertIn("historical schema route", "; ".join(result["violations"]))

    def test_baseline_missing_a_required_registry_is_not_an_empty_previous_snapshot(self):
        self.git("rm", "--quiet", transition.REGISTRY_REFS[1])
        self.git("commit", "--quiet", "-m", "Synthetic incomplete baseline")
        missing = self.git("rev-parse", "HEAD").strip()
        self.save(self.registries)
        with self.assertRaisesRegex(ValueError, "partial baseline"):
            transition.validate_transition(self.root, missing, allow_initial_introduction=True)

    def test_initial_introduction_requires_explicit_authority_and_reports_no_previous_comparison(self):
        with self.assertRaisesRegex(ValueError, "initial introduction requires explicit"):
            transition.validate_transition(self.root, self.initial)
        result = transition.validate_transition(self.root, self.initial, allow_initial_introduction=True)
        self.assertTrue(result["valid"], result["violations"])
        self.assertEqual(result["transition_kind"], "initial-introduction")
        self.assertFalse(result["compared_previous_registry"])
        self.assertTrue(result["initial_introduction_explicitly_allowed"])
        self.assertFalse(result["semantic_acceptance"])
        self.assertEqual(result["baseline_sha256"], {})
        self.assertEqual(set(result["baseline_absent_refs"]), {*transition.REGISTRY_REFS, *transition.SCHEMA_REFS, transition.DECLARED_READER_REF})
        changed = self.extension(0)
        self.profile(changed, 0)["profile_version"] -= 1
        self.save(changed)
        self.assertFalse(transition.validate_transition(self.root, self.baseline, allow_initial_introduction=True)["valid"])
        with mock.patch.dict(os.environ, {transition.BASELINE_ENV: self.initial, transition.INTRODUCTION_ENV: "1"}), contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(transition.main(["--repo-root", str(self.root), "--json"]), 0)
        self.assertEqual(json.loads(output.getvalue())["transition_kind"], "initial-introduction")

    def test_initial_introduction_rejects_shallow_ancestry(self):
        original_git = transition._git

        def shallow_git(root, *args):
            return b"true\n" if args == ("rev-parse", "--is-shallow-repository") else original_git(root, *args)

        with mock.patch.object(transition, "_git", side_effect=shallow_git), self.assertRaisesRegex(ValueError, "shallow history"):
            transition.validate_transition(self.root, self.initial, allow_initial_introduction=True)

    def test_grafts_cannot_hide_a_deleted_previous_registry(self):
        self.git("rm", "--quiet", "-r", "ToS")
        self.git("commit", "--quiet", "-m", "Synthetic erased registry baseline")
        deleted = self.git("rev-parse", "HEAD").strip()
        self.git("restore", "--source=" + self.baseline, "--", "ToS")
        graft = self.root / ".git/info/grafts"
        graft.write_text(deleted + "\n")
        with self.assertRaisesRegex(ValueError, "Git grafts are not allowed"):
            transition.validate_transition(self.root, deleted, allow_initial_introduction=True)
        # Grafts affect graph ancestry, not an ordinary exact-blob comparison.
        self.assertTrue(transition.validate_transition(self.root, self.baseline)["valid"])
        graft.unlink()
        external_graft = self.root / "synthetic-grafts"
        external_graft.write_text(deleted + "\n")
        with mock.patch.dict(os.environ, {"GIT_GRAFT_FILE": str(external_graft)}), self.assertRaisesRegex(ValueError, "Git grafts are not allowed"):
            transition.validate_transition(self.root, deleted, allow_initial_introduction=True)

    def test_deleted_prior_registries_and_old_declared_readers_are_not_initial_introduction(self):
        self.git("rm", "--quiet", "-r", "ToS")
        self.git("commit", "--quiet", "-m", "Synthetic deleted prior registries")
        deleted = self.git("rev-parse", "HEAD").strip()
        with self.assertRaisesRegex(ValueError, "history already contains"):
            transition.validate_transition(self.root, deleted, allow_initial_introduction=True)
        # An independent pre-registry branch with the declared reader is also
        # not an empty previous reader model, even without registry JSON.
        self.git("switch", "--quiet", "--detach", self.initial)
        reader = self.root / transition.DECLARED_READER_REF
        reader.parent.mkdir(parents=True, exist_ok=True)
        reader.write_text("# Synthetic old declared-profile reader; never executed.\n")
        self.git("add", transition.DECLARED_READER_REF)
        self.git("commit", "--quiet", "-m", "Synthetic pre-registry declared reader")
        old_reader = self.git("rev-parse", "HEAD").strip()
        with self.assertRaisesRegex(ValueError, "history already contains"):
            transition.validate_transition(self.root, old_reader, allow_initial_introduction=True)

    def test_schema_and_duplicate_json_fail_before_transition_comparison(self):
        changed = copy.deepcopy(self.registries)
        self.profile(changed, 0)["profile_version"] = False
        self.save(changed)
        with self.assertRaisesRegex(ValueError, "profile_version"):
            transition.validate_transition(self.root, self.baseline)
        target = self.root / transition.REGISTRY_REFS[0]
        target.write_text('{"types": [], "types": []}')
        with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
            transition.validate_transition(self.root, self.baseline)

    def test_explicit_environment_baseline_drives_registered_release_gate(self):
        with mock.patch.dict(os.environ, {transition.BASELINE_ENV: self.baseline}), contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(transition.main(["--repo-root", str(self.root), "--json"]), 0)
        self.assertEqual(json.loads(output.getvalue())["baseline_commit"], self.baseline)
        manifest = json.loads((REPO_ROOT / "docs/validation/validation_lanes.json").read_text())
        gate = manifest["command_sequences"]["semantic_registry_transition"]
        self.assertEqual(len(gate), 1)
        self.assertIn(gate[0], manifest["command_sequences"]["release_check"])
        self.assertIn("semantic_registry_transition", manifest["lanes"]["release"]["covers_lanes"])
        workflow = (REPO_ROOT / ".github/workflows/repo-validation.yml").read_text()
        self.assertIn(transition.BASELINE_ENV + ":", workflow)
        self.assertIn(transition.INTRODUCTION_ENV + ': "1"', workflow)
        self.assertIn("github.event.pull_request.base.sha || github.event.before", workflow)
        self.assertIn("fetch-depth: 0", workflow)


if __name__ == "__main__":
    unittest.main()
