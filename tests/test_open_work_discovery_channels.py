from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from measure_open_work_discovery_channels import (  # noqa: E402
    build_superseding_discovery,
    main,
    retarget_timing_receipt,
    validate_output_paths,
)


class OpenWorkDiscoveryChannelsTest(unittest.TestCase):
    def test_output_modes_cannot_emit_two_instrumented_discoveries(self) -> None:
        original_argv = sys.argv
        try:
            sys.argv = [
                "measure_open_work_discovery_channels.py",
                "unused.json",
                "--superseding-output",
                "superseding.json",
                "--instrumented-output",
                "instrumented.json",
                "--new-discovery-id",
                "tos.discovery.superseding",
                "--supersedes-ref",
                "original.json",
                "--provenance-event-ref",
                "tos.event.discovery.superseding",
            ]
            with self.assertRaises(SystemExit) as raised:
                main()
        finally:
            sys.argv = original_argv
        self.assertEqual(2, raised.exception.code)

    def test_retarget_timing_receipt_rebinds_superseding_discovery(self) -> None:
        original = {
            "timing_id": "open-work-channel-timing.original",
            "discovery_id": "tos.discovery.original",
            "discovery_ref": "ToS/source-witnesses/discovery/runs/original.json",
            "measurements": [],
        }

        retargeted = retarget_timing_receipt(
            original,
            discovery_id="tos.discovery.superseding",
            discovery_ref="ToS/source-witnesses/discovery/runs/superseding.json",
        )

        self.assertEqual("open-work-channel-timing.superseding", retargeted["timing_id"])
        self.assertEqual("tos.discovery.superseding", retargeted["discovery_id"])
        self.assertEqual(
            "ToS/source-witnesses/discovery/runs/superseding.json",
            retargeted["discovery_ref"],
        )
        self.assertEqual(original["discovery_id"], "tos.discovery.original")
        self.assertEqual(retargeted["measurements"], original["measurements"])
        self.assertIsNot(retargeted, original)
        self.assertEqual(retargeted, copy.deepcopy(retargeted))

    def test_superseding_discovery_must_name_the_input_discovery(self) -> None:
        with self.assertRaisesRegex(ValueError, "supersedes_ref must match"):
            build_superseding_discovery(
                {"discovery_id": "tos.discovery.original"},
                {"measurements": []},
                discovery_id="tos.discovery.superseding",
                supersedes_ref="tos.discovery.other",
                provenance_event_ref="tos.event.discovery.superseding",
            )

    def test_outputs_must_not_alias_input_or_each_other(self) -> None:
        discovery = Path("ToS/source-witnesses/discovery/runs/original.json")
        with self.assertRaisesRegex(ValueError, "must not overwrite input discovery"):
            validate_output_paths(discovery, [("--output", discovery)])

        with self.assertRaisesRegex(ValueError, "must not alias --output"):
            validate_output_paths(
                discovery,
                [
                    ("--output", Path("out.json")),
                    ("--instrumented-output", Path("./out.json")),
                ],
            )


if __name__ == "__main__":
    unittest.main()
