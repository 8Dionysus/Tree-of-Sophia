"""Build/check the native implementation-only catalog from the maintained handler owner.

No owner configurations, grants, source targets or clock are read. The generated
companion describes implementations only and grants no authority.
"""
from pathlib import Path
import argparse
import json
import source_commands


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[5]
    output = root / 'rust/crates/tos-command/src/source_command_catalog.json'
    raw = (json.dumps(source_commands.discover_commands(), ensure_ascii=True,
                      sort_keys=True, separators=(',', ':'), allow_nan=False) + '\n').encode('ascii')
    if args.check:
        if output.read_bytes() != raw:
            raise SystemExit('native implementation catalog is stale')
    else:
        output.write_bytes(raw)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
