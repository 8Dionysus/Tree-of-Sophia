#!/usr/bin/env python3
"""Validate the checked-in public-safe ToS Evidence Lens projection."""

from __future__ import annotations

from epistemic_evidence_projection_common import PROJECTION_PATH, build_payload, render_payload, validate_payload


def main() -> int:
    payload = build_payload()
    validate_payload(payload)
    if not PROJECTION_PATH.is_file() or PROJECTION_PATH.read_text(encoding="utf-8") != render_payload(payload):
        raise SystemExit("ToS/derived-exports/epistemic_evidence_projection.min.json is out of date")
    print("[ok] validated ToS Evidence Lens projection")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
