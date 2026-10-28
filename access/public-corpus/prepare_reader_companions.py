#!/usr/bin/env python3
"""Project two bounded source-owned reader companions without corpus loading."""

import argparse
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="refresh derived copies; default checks parity")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    source = root / "ToS/research-packets/public-prototypes-20261026"
    target = root / "access/public-corpus"
    names = ("reader-textual-variants.v1.json", "contained-reading-targets.v1.json")
    for name in names:
        content = (source / name).read_bytes()
        if len(content) > 64_000:
            raise SystemExit(f"reader companion exceeds its bounded metadata budget: {name}")
        destination = target / name
        if args.write:
            destination.write_bytes(content)
        elif not destination.is_file() or destination.read_bytes() != content:
            raise SystemExit(f"reader companion differs from its source: {name}")
    print(f"Reader companions match their authored sources: {len(names)}")


if __name__ == "__main__":
    main()
