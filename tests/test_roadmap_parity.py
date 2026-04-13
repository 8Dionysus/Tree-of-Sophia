from __future__ import annotations

import json
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def read_text(relative_path: str) -> str:
    return (REPO_ROOT / relative_path).read_text(encoding="utf-8")


def load_json(relative_path: str) -> object:
    return json.loads((REPO_ROOT / relative_path).read_text(encoding="utf-8"))


class RoadmapParityTestCase(unittest.TestCase):
    def test_roadmap_keeps_root_entry_routes_in_current_phase(self) -> None:
        roadmap = read_text("ROADMAP.md")
        payload = load_json("generated/root_entry_map.min.json")

        route_ids = {route["route_id"] for route in payload["routes"]}

        self.assertIn("current-tiny-entry", route_ids)
        self.assertIn("bounded-export", route_ids)
        self.assertIn("shared `node_id`", roadmap)
        self.assertIn("tiny-entry seam", roadmap)
        self.assertIn("bounded export seam", roadmap)
        self.assertIn("`aoa-kag`", roadmap)


if __name__ == "__main__":
    unittest.main()
