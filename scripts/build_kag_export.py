#!/usr/bin/env python3
"""Native exact accepted-corpus KAG export; authored source authority is unchanged.

The original source remains in build_kag_export_legacy_oracle.py for independent
compatibility assertions. The maintained command has no Python fallback.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
from publish_kag_release import _native
PRIMARY = 'ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json'
SOURCE_PATHS = (
    PRIMARY, 'ToS/derived-exports/README.md',
    'ToS/public-compatibility/concept_node.example.json',
    'ToS/public-compatibility/source_node.example.json',
    'ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md',
    'ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md',
)
CAPSULE = 'ToS/derived-exports/kag_export.min.json'
SCHEMA = 'tos_kag_source_export_v1'
MAX_SOURCE_BYTES = 8 * 1024 * 1024

def verify_export(root: Path) -> dict:
    return _native(['export-verify', '--release', str(root)])

def build_export(store_root: Path, revision: str, output: Path) -> dict:
    return _native(['export-build', '--store', str(store_root), '--revision', revision, '--output', str(output)])

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    build = commands.add_parser('build')
    build.add_argument('--store', type=Path, required=True)
    build.add_argument('--revision', required=True)
    build.add_argument('--output', type=Path, required=True)
    verify = commands.add_parser('verify')
    verify.add_argument('export', type=Path)
    args = parser.parse_args(argv)
    result = (verify_export(args.export) if args.command == 'verify' else
              build_export(args.store, args.revision, args.output))
    print(json.dumps({key: result[key] for key in ('export_revision', 'corpus_revision', 'primary_source')}))

if __name__ == '__main__':
    main()
