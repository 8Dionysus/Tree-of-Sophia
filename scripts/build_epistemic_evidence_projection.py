#!/usr/bin/env python3
"""Build the public-safe ToS Evidence Lens projection."""

from __future__ import annotations

import argparse

from epistemic_evidence_projection_common import PROJECTION_PATH, build_payload, render_payload


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    rendered = render_payload(build_payload())
    if args.check:
        if not PROJECTION_PATH.is_file() or PROJECTION_PATH.read_text(encoding="utf-8") != rendered:
            raise SystemExit(f"{PROJECTION_PATH.relative_to(PROJECTION_PATH.parents[2])} is out of date")
        print("[ok] verified ToS Evidence Lens projection")
        return 0
    PROJECTION_PATH.write_text(rendered, encoding="utf-8")
    print("[ok] wrote ToS Evidence Lens projection")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
