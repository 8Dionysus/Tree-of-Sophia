from __future__ import annotations

import json
import unittest
from pathlib import Path

import sys


sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from root_entry_map_common import ARTIFACT_IDENTITY, CORE_ROUTE_IDS, SURFACE_PAYLOAD, build_payload


class RootEntryMapTests(unittest.TestCase):
    def test_build_payload_stays_tree_first(self) -> None:
        payload = build_payload()

        self.assertEqual(payload["schema_version"], "tos_root_entry_map_v1")
        self.assertEqual(payload["schema_ref"], "ToS/contracts/root-entry-map.schema.json")
        self.assertEqual(payload["owner_repo"], "Tree-of-Sophia")
        self.assertEqual(payload["surface_kind"], "root_entry_map")
        self.assertEqual(payload["authority_ref"], SURFACE_PAYLOAD["authority_ref"])
        self.assertEqual(payload["public_root_ref"], "README.md")
        self.assertEqual(payload["artifact_identity"], ARTIFACT_IDENTITY)
        route_ids = [route["route_id"] for route in payload["routes"]]
        self.assertEqual(len(route_ids), len(set(route_ids)))
        self.assertLessEqual(CORE_ROUTE_IDS, set(route_ids))

    def test_current_tiny_entry_route_keeps_example_first(self) -> None:
        payload = build_payload()
        route = next(route for route in payload["routes"] if route["route_id"] == "current-tiny-entry")

        self.assertEqual(route["surface_ref"], "ToS/public-compatibility/tos_tiny_entry_route.example.json")
        self.assertEqual(
            route["verification_refs"],
            [
                "ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md",
                "ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md",
            ],
        )

    def test_payload_is_json_serializable(self) -> None:
        payload = build_payload()
        rendered = json.dumps(payload, separators=(",", ":"))

        self.assertIn("root_entry_map", rendered)
        self.assertNotIn('"surface_ref":"scripts/', rendered)
        self.assertNotIn('"surface_ref":"src/', rendered)

    def test_artifact_identity_names_consumer_checks_without_media_claims(self) -> None:
        identity = build_payload()["artifact_identity"]

        self.assertEqual(identity["artifact_class"], "tree_of_sophia_generated_readmodel_bundle")
        self.assertEqual(identity["contract_version"], "ToS/contracts/root-entry-map.schema.json")
        self.assertEqual(identity["trust_layer"], ["abi_contract_signature", "source_schema_validation"])
        self.assertIn("build_root_entry_map --check", identity["consumer_expectation"])
        self.assertIn("validate_root_entry_map", identity["consumer_expectation"])
        self.assertIn("OS Abyss ABI bundle", identity["consumer_expectation"])
        self.assertIn("no private host evidence", identity["privacy_boundary"])
        self.assertIn("media credential claims", identity["privacy_boundary"])
        self.assertNotIn("c2pa", json.dumps(identity, separators=(",", ":")).lower())

    def test_artifact_bundle_manifest_requires_registry_lifecycle(self) -> None:
        manifest = json.loads(
            (
                Path(__file__).resolve().parents[1]
                / "mechanics"
                / "release-support"
                / "parts"
                / "artifact-bundles"
                / "manifests"
                / "generated_readmodel.bundle.json"
            ).read_text(encoding="utf-8")
        )

        self.assertTrue(manifest["public_safe"])
        self.assertEqual(manifest["artifact_source"]["kind"], "generated_public_readmodel_bundle")
        self.assertEqual(manifest["lifecycle"]["initial_state"], "candidate")
        self.assertIn("release-ready", manifest["lifecycle"]["promotion_path"])
        self.assertTrue(manifest["consumer_contract"]["registry_required"])
        command_text = "\n".join(manifest["consumer_command"])
        self.assertIn("bundle-register", command_text)
        self.assertIn("materialize-subjects", command_text)
        self.assertIn("trust-gate", command_text)
        self.assertIn("registry-latest", command_text)


if __name__ == "__main__":
    unittest.main()
