from __future__ import annotations

import copy
import importlib.util
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "prepare_retained_corpus",
    ROOT / "access/public-corpus/prepare_retained_corpus.py",
)
assert SPEC is not None and SPEC.loader is not None
PREPARE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PREPARE)


class CompleteProfileExtensionTests(unittest.TestCase):
    def test_retained_import_preserves_aesthetics_subset(self) -> None:
        selection = json.loads((ROOT / "access/public-corpus/selection.v1.json").read_text(encoding="utf-8"))
        profiles = copy.deepcopy(selection["profiles"])
        previous_ids = list(dict.fromkeys(document["id"] for document in selection["documents"]))
        new_id = "tos.work.example.retained-future"

        self.assertEqual(len(previous_ids), 51)
        self.assertEqual(len(profiles["aesthetics"]["documentIds"]), 3)
        PREPARE.extend_complete_profiles(profiles, previous_ids, [*previous_ids, new_id])

        for name in ("zarathustra", "antiquity"):
            self.assertEqual(profiles[name]["documentIds"], [*selection["profiles"][name]["documentIds"], new_id])
        self.assertEqual(profiles["aesthetics"]["documentIds"], selection["profiles"]["aesthetics"]["documentIds"])

    def test_only_full_profiles_grow_and_existing_order_is_kept(self) -> None:
        profiles = {
            "full": {"documentIds": ["work-b", "work-a"]},
            "subset": {"documentIds": ["work-a"]},
            "empty": {"documentIds": []},
        }

        PREPARE.extend_complete_profiles(
            profiles,
            ["work-a", "work-b"],
            ["work-a", "work-b", "work-c", "work-c"],
        )

        self.assertEqual(profiles["full"]["documentIds"], ["work-b", "work-a", "work-c"])
        self.assertEqual(profiles["subset"]["documentIds"], ["work-a"])
        self.assertEqual(profiles["empty"]["documentIds"], [])


if __name__ == "__main__":
    unittest.main()
