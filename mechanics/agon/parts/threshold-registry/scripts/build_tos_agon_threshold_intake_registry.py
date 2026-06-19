#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
from typing import Any

PART_ROOT = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = pathlib.Path(__file__).resolve().parents[5]
SRC = PART_ROOT / 'config/tos_agon_threshold_intakes.config.json'
OUT = PART_ROOT / 'generated/tos_agon_threshold_intake_registry.min.json'
ITEM_KEY = 'threshold_intakes'
REGISTRY_ID = 'tos.agon_threshold_intake.registry.v1'
REVIEW_PHASE_ORDER = 'XVIII'
REVIEW_PHASE_LABEL = 'Sophian Threshold'
RUNTIME_POSTURE = 'candidate_only'


def digest_obj(obj: Any) -> str:
    return hashlib.sha256(
        json.dumps(obj, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()
    ).hexdigest()


def load_source() -> dict[str, Any]:
    data = json.loads(SRC.read_text(encoding='utf-8'))
    expected_metadata = {
        'registry_id': REGISTRY_ID,
        'review_phase_order': REVIEW_PHASE_ORDER,
        'review_phase_label': REVIEW_PHASE_LABEL,
        'runtime_posture': RUNTIME_POSTURE,
    }
    for field, expected in expected_metadata.items():
        if data.get(field) != expected:
            raise ValueError(f'{field} must be {expected!r} in {SRC.relative_to(REPO_ROOT)}')
    items = data.get(ITEM_KEY)
    if not isinstance(items, list):
        raise ValueError(f'{ITEM_KEY} must be a list in {SRC.relative_to(REPO_ROOT)}')
    return data


def build() -> dict[str, Any]:
    data = load_source()
    items = data[ITEM_KEY]
    return {
        'registry_id': REGISTRY_ID,
        'review_phase_order': REVIEW_PHASE_ORDER,
        'review_phase_label': REVIEW_PHASE_LABEL,
        'runtime_posture': RUNTIME_POSTURE,
        'count': len(items),
        ITEM_KEY: items,
        'digest': digest_obj(items),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    try:
        registry = build()
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(exc, file=sys.stderr)
        return 1
    text = json.dumps(registry, ensure_ascii=False, sort_keys=True, separators=(',', ':')) + '\n'
    if args.check:
        if not OUT.exists():
            print(f'missing generated registry: {OUT.relative_to(REPO_ROOT)}', file=sys.stderr)
            return 1
        if OUT.read_text(encoding='utf-8') != text:
            print('generated registry drift: run builder without --check', file=sys.stderr)
            return 1
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding='utf-8')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
