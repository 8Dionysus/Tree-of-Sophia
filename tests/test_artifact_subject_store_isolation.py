from __future__ import annotations

import importlib.util
import os
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
VALIDATOR_PATH = (
    REPO_ROOT
    / "mechanics"
    / "release-support"
    / "parts"
    / "artifact-bundles"
    / "scripts"
    / "validate_abyss_machine_generated_readmodel_bundle.py"
)


def load_validator():
    spec = importlib.util.spec_from_file_location(
        "tos_validate_generated_readmodel_bundle",
        VALIDATOR_PATH,
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FakeArtifactBundles:
    """Small local stand-in for the owner resolver used by the scope test.

    The Tree test owns the process-binding contract. The full owner package is
    exercised by the release audit; importing it here would make a repo-local
    unit test depend on an undeclared sibling checkout.
    """

    def __init__(self, default_root: Path) -> None:
        self.DEFAULT_ARTIFACT_SUBJECT_STORE_ROOT = default_root

    def _artifact_subject_store_roots(self) -> list[Path]:
        roots: list[Path] = []
        for name in validator_subject_store_env_names:
            raw = os.environ.get(name, "")
            roots.extend(
                Path(item).expanduser()
                for item in raw.split(os.pathsep)
                if item.strip()
            )
        roots.append(self.DEFAULT_ARTIFACT_SUBJECT_STORE_ROOT)
        unique: list[Path] = []
        for root in roots:
            resolved = root.resolve()
            if resolved not in unique:
                unique.append(resolved)
        return unique


validator_subject_store_env_names = (
    "ABYSS_MACHINE_ARTIFACT_SUBJECT_STORE_ROOT",
    "ABYSS_MACHINE_ARTIFACT_SUBJECT_STORE_ROOTS",
)


class ArtifactSubjectStoreIsolationTests(unittest.TestCase):
    def test_raw_empty_root_is_rejected_before_materialization(self) -> None:
        validator = load_validator()

        with self.assertRaisesRegex(ValueError, "non-empty path"):
            validator._resolve_subject_store_root("")

    def test_explicit_current_directory_root_is_rejected_before_materialization(self) -> None:
        validator = load_validator()

        with self.assertRaisesRegex(ValueError, "repository root"):
            validator._resolve_subject_store_root(Path("."))

    def test_explicit_empty_root_suppresses_ambient_default_and_restores_state(self) -> None:
        validator = load_validator()

        with tempfile.TemporaryDirectory(prefix="tos-subject-store-isolation-") as tmp:
            root = Path(tmp)
            ambient_root = root / "ambient"
            isolated_root = root / "isolated-empty"
            ambient_root.mkdir()
            isolated_root.mkdir()
            artifact_bundles = FakeArtifactBundles(ambient_root)
            old_default = artifact_bundles.DEFAULT_ARTIFACT_SUBJECT_STORE_ROOT
            old_env = {
                name: os.environ.get(name)
                for name in validator.SUBJECT_STORE_ENV_NAMES
            }
            for name in validator.SUBJECT_STORE_ENV_NAMES:
                os.environ[name] = str(ambient_root)

            try:
                with validator._subject_store_scope(artifact_bundles, isolated_root):
                    roots = artifact_bundles._artifact_subject_store_roots()
                    self.assertEqual([isolated_root.resolve()], [path.resolve() for path in roots])
                    self.assertEqual([], list(isolated_root.iterdir()))
            finally:
                artifact_bundles.DEFAULT_ARTIFACT_SUBJECT_STORE_ROOT = old_default
                for name, value in old_env.items():
                    if value is None:
                        os.environ.pop(name, None)
                    else:
                        os.environ[name] = value

            self.assertEqual(old_default, artifact_bundles.DEFAULT_ARTIFACT_SUBJECT_STORE_ROOT)
            self.assertEqual([], list(isolated_root.iterdir()))


if __name__ == "__main__":
    unittest.main()
