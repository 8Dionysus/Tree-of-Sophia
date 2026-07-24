from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from tos_corpus_index_common import (  # noqa: E402
    TOS_CORPUS_INDEX_PATH,
    build_payload,
    render_payload,
    tracked_tos_paths,
)


class ToSCorpusIndexTest(unittest.TestCase):
    def test_generated_index_matches_builder(self) -> None:
        expected = render_payload(build_payload())
        current = TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8")
        self.assertEqual(current, expected)

    def test_index_keeps_runtime_projection_subordinate(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")
        self.assertIn("runtime_projection", [entry["layer"] for entry in payload["authority_order"]])
        self.assertGreater(payload["counts"]["resources"], payload["counts"]["nodes"])

    def test_index_has_no_error_diagnostics(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        errors = [
            diagnostic
            for diagnostic in payload["diagnostics"]
            if diagnostic.get("level") == "error"
        ]
        self.assertEqual(errors, [])

    def test_index_resources_are_owned_by_the_tracked_source_view(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        tracked_refs = {
            path.relative_to(REPO_ROOT).as_posix()
            for path in tracked_tos_paths()
        }
        resource_refs = {resource["path"] for resource in payload["resources"]}

        self.assertLessEqual(resource_refs, tracked_refs)
        self.assertFalse(
            any("payload" in Path(path_ref).parts for path_ref in resource_refs)
        )

    def test_authority_order_declares_all_emitted_layers(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        declared = {entry["layer"] for entry in payload["authority_order"]}
        emitted = set()
        for collection_name in (
            "branches",
            "manifests",
            "nodes",
            "relation_packs",
            "relation_edges",
            "resources",
        ):
            emitted.update(
                item["authority_layer"]
                for item in payload[collection_name]
                if "authority_layer" in item
            )
        self.assertEqual(sorted(emitted - declared), [])


if __name__ == "__main__":
    unittest.main()
