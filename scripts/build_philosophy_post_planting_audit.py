#!/usr/bin/env python3
"""Build the ToS philosophy post-planting audit packet."""

from __future__ import annotations

import argparse

from philosophy_post_planting_audit_common import AUDIT_JSON_PATH, AUDIT_MD_PATH, build_payload, render_markdown, render_payload


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build the Table I post-planting audit packet.")
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify the generated audit files match the canonical rebuild instead of rewriting them.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    payload = build_payload()
    rendered_json = render_payload(payload)
    rendered_md = render_markdown(payload)
    if args.check:
        if AUDIT_JSON_PATH.read_text(encoding="utf-8") != rendered_json:
            raise SystemExit(f"{AUDIT_JSON_PATH.relative_to(AUDIT_JSON_PATH.parents[4])} is out of date")
        if AUDIT_MD_PATH.read_text(encoding="utf-8") != rendered_md:
            raise SystemExit(f"{AUDIT_MD_PATH.relative_to(AUDIT_MD_PATH.parents[4])} is out of date")
        print("[ok] verified ToS philosophy post-planting audit")
        return 0
    AUDIT_JSON_PATH.write_text(rendered_json, encoding="utf-8")
    AUDIT_MD_PATH.write_text(rendered_md, encoding="utf-8")
    print("[ok] wrote ToS philosophy post-planting audit")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
