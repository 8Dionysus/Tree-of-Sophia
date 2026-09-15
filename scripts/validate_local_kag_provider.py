#!/usr/bin/env python3
"""Verify one explicit immutable ToS source export for a downstream KAG build.

KAG family/shard/replay validation belongs to the selected aoa-kag consumer.
This entry point has no ambient checkout, index or sibling dependency.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from build_kag_export import verify_export
from corpus_store import CorpusStoreError


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--export', type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        manifest = verify_export(args.export)
    except (CorpusStoreError, ValueError, OSError, KeyError, TypeError) as error:
        parser.exit(1, f'KAG source export rejected: {error}\n')
    print(json.dumps({'export_revision': manifest['export_revision'],
                      'corpus_revision': manifest['corpus_revision'],
                      'primary_source': manifest['primary_source']}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
