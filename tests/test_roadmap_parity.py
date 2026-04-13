from __future__ import annotations

import json
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def read_text(relative_path: str) -> str:
    return (REPO_ROOT / relative_path).read_text(encoding="utf-8")


def load_json(relative_path: str) -> object:
    return json.loads((REPO_ROOT / relative_path).read_text(encoding="utf-8"))


def test_roadmap_keeps_root_entry_routes_in_current_phase() -> None:
    roadmap = read_text("ROADMAP.md")
    payload = load_json("generated/root_entry_map.min.json")

    route_ids = {route["route_id"] for route in payload["routes"]}

    assert "current-tiny-entry" in route_ids
    assert "bounded-export" in route_ids
    assert "shared `node_id`" in roadmap
    assert "tiny-entry seam" in roadmap
    assert "bounded export seam" in roadmap
    assert "`aoa-kag`" in roadmap
