from __future__ import annotations

import importlib.util
import json
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def load_common() -> object:
    path = REPO_ROOT / "scripts/epistemic_evidence_projection_common.py"
    spec = importlib.util.spec_from_file_location("epistemic_evidence_projection_common", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_projection_rebuilds_two_contrasting_public_scenes() -> None:
    common = load_common()
    payload = common.build_payload()
    common.validate_payload(payload)

    scenes = {scene["scene_id"]: scene for scene in payload["scenes"]}
    assert set(scenes) == {"zarathustra-canon-route", "a01-archaic-tribute-frontier"}

    zarathustra = scenes["zarathustra-canon-route"]
    assert "m113" in zarathustra["selection_ids"]
    assert zarathustra["posture"] == "canon-retained-evidence-open"
    assert zarathustra["conclusion"]["canon_membership"] is True
    assert zarathustra["conclusion"]["claim_evidence_closed"] is False
    assert zarathustra["source_anchors"][0]["anchor_segment_ids"] == [
        "seg.1.1.1.5",
        "seg.1.1.1.6",
    ]
    assert any(route["route_kind"] == "review" for route in zarathustra["routes"])
    assert any(route["route_kind"] == "rights" for route in zarathustra["routes"])

    tribute = scenes["a01-archaic-tribute-frontier"]
    assert "edge:candidate-relation:table-i-a01-relation-027" in tribute["selection_ids"]
    assert tribute["posture"] == "contested-pre-canon"
    assert tribute["conclusion"]["canon_membership"] is False
    assert tribute["conclusion"]["claim_evidence_closed"] is False
    assert tribute["conclusion"]["can_conclude"] is False
    assert any("proto-literary" in gap for gap in tribute["gaps"])
    assert all(route["exists"] for route in tribute["routes"])


def test_checked_in_projection_matches_canonical_rebuild() -> None:
    common = load_common()
    expected = common.render_payload(common.build_payload())
    actual = (
        REPO_ROOT / "ToS/derived-exports/epistemic_evidence_projection.min.json"
    ).read_text(encoding="utf-8")
    assert actual == expected


def test_projection_is_public_safe_and_reference_closed() -> None:
    common = load_common()
    payload = common.build_payload()
    encoded = json.dumps(payload, ensure_ascii=False)
    assert "/payload/" not in encoded
    assert "file://" not in encoded
    assert "/srv/" not in encoded
    for scene in payload["scenes"]:
        for route in scene["routes"]:
            assert (REPO_ROOT / route["ref"]).is_file()
