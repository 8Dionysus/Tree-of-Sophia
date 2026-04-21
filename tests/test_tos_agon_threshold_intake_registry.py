from __future__ import annotations
import json, pathlib, subprocess, sys
ROOT = pathlib.Path(__file__).resolve().parents[1]
GENERATED = ROOT / 'generated/tos_agon_threshold_intake_registry.min.json'
SCRIPT = ROOT / 'scripts/build_tos_agon_threshold_intake_registry.py'
EXPECTED_COUNT = 8
ITEM_KEY = 'threshold_intakes'

def test_generated_registry_shape():
    reg = json.loads(GENERATED.read_text(encoding='utf-8'))
    assert reg['wave'] == 'XVIII'
    assert reg['count'] == EXPECTED_COUNT
    assert len(reg[ITEM_KEY]) == EXPECTED_COUNT
    assert all(item.get('live_protocol') is False for item in reg[ITEM_KEY])
    assert all(item.get('review_status') == 'candidate_only' for item in reg[ITEM_KEY])

def test_builder_check():
    result = subprocess.run([sys.executable, str(SCRIPT), '--check'], cwd=str(ROOT), text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
