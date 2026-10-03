#!/usr/bin/env python3
"""Observe current public source-catalog fields in the native read model.

Run an explicit offline coverage scan through the installed Rust ``tos``
command. Rows disclose catalog identities, source references, field names and
mechanical states, not source wording. A stream without its terminal summary
is incomplete. This diagnostic assesses no meaning, admits no source and
writes no source.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys

REPO_ROOT = Path(__file__).resolve().parents[1]
_ROW_SCHEMA = 'tos_source_projection_coverage_row_v1'
_REPORT_SCHEMA = 'tos_source_projection_coverage_v1'
_GRAPH_INPUT_CAP = 256 * 1024 * 1024
_OBSERVE_INPUT_CAP = 16 * 1024 * 1024
_FRAME_CAP = 64 * 1024 * 1024
_DEFAULT_MAX_SECONDS = 120
_CLEANUP_SECONDS = 5


def _native_packets(arguments, value=None, *, prefix=None,
                    input_cap=_OBSERVE_INPUT_CAP,
                    operation_seconds=_DEFAULT_MAX_SECONDS + _CLEANUP_SECONDS):
    """Forward exact source-domain work to the installed native command."""
    access_src = REPO_ROOT / 'access' / 'src'
    if str(access_src) not in sys.path:
        sys.path.insert(0, str(access_src))
    from tos_access.native_io import native_packets

    return native_packets(arguments, value, prefix=prefix, input_cap=input_cap,
                          frame_cap=_FRAME_CAP, operation_seconds=operation_seconds)


def _wire(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True,
                      separators=(',', ':'), allow_nan=False)


def observe_record(identity, record, source_ref, candidates, *, kind, adapter,
                   source_line=None):
    """Run the native exact-field comparison with a 16 MiB host request cap."""
    request = {
        'identity': identity,
        'record': record,
        'source_ref': source_ref,
        'candidates': candidates,
        'kind': kind,
        'adapter': adapter,
        'source_line': source_line,
    }
    stream = _native_packets(['source-projection-coverage', '--observe-record',
                              '--input', '-'], request)
    try:
        packets = list(stream)
    finally:
        stream.close()
    if len(packets) != 1 or not isinstance(packets[0], dict):
        raise ValueError('native source observation did not return exactly one packet')
    return packets[0]


def coverage_report(root, graph, *, invocation=None, emit_row=None, verify_graph=None):
    """Enumerate current root catalog inputs against one supplied graph packet.

    The native provider owns catalog/schema/native-text validation, complete
    source membership and byte rechecks, and exact field comparison. This
    compatibility API keeps caller-owned callbacks at their former boundary.
    Graph input is capped at 256 MiB encoded JSON, depth 64 and 1,000,000
    visits. Its 120-second operation clock starts before encoding and setup.
    """
    root = Path(root).resolve(strict=True)
    if not isinstance(graph, dict) or not isinstance(graph.get('source_revision'), str):
        raise ValueError('coverage graph must carry a string source_revision')
    if invocation is None:
        raise ValueError('source coverage requires protected native invocation')
    arguments = ['source-projection-coverage', '--root', str(root), '--invocation',
                 str(Path(invocation).resolve(strict=True)), '--graph', '-']
    if emit_row is not None:
        arguments.append('--rows')
    stream = _native_packets(arguments, graph, input_cap=_GRAPH_INPUT_CAP,
                             operation_seconds=_DEFAULT_MAX_SECONDS + _CLEANUP_SECONDS)
    summary = None
    try:
        for packet in stream:
            if not isinstance(packet, dict):
                raise ValueError('native source coverage frame must be an object')
            schema = packet.get('schema_version')
            if schema == _ROW_SCHEMA and summary is None and emit_row is not None:
                if packet.get('source_revision') != graph['source_revision']:
                    raise ValueError('native source coverage row uses another graph revision')
                emit_row(packet)
            elif schema == _REPORT_SCHEMA and summary is None:
                summary = packet
            else:
                raise ValueError('unexpected native source coverage stream order')
    finally:
        stream.close()
    if (summary is None or summary.get('enumeration_complete') is not True
            or summary.get('source_revision') != graph['source_revision']):
        raise ValueError('native source coverage stream has no matching terminal summary')
    if verify_graph is not None:
        verify_graph(graph['source_revision'])
    return summary


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native-prefix', default=os.environ.get('TOS_NATIVE_PREFIX'),
                        help='installed native software prefix (defaults to TOS_NATIVE_PREFIX)')
    parser.add_argument('--root', type=Path, default=REPO_ROOT,
                        help='source repository root; defaults to this checkout')
    parser.add_argument('--invocation', type=Path, required=True,
                        help='protected native Foundation invocation selecting exact worker, custody and budgets')
    parser.add_argument('--graph', type=Path,
                        help='optional supplied normalized graph JSON snapshot; default reads the held native snapshot')
    parser.add_argument('--rows', action='store_true',
                        help='stream per-source NDJSON before the terminal summary')
    parser.add_argument('--max-input-bytes', type=int,
                        help='combined selected-source and graph input budget (maximum 256 MiB)')
    parser.add_argument('--max-rows', type=int,
                        help='combined public catalog and graph metadata row budget')
    parser.add_argument('--max-seconds', type=int,
                        help='one operation deadline in seconds (maximum 3600)')
    args = parser.parse_args(argv)
    root = args.root.resolve(strict=True)
    arguments = ['source-projection-coverage', '--root', str(root), '--invocation', str(args.invocation.resolve(strict=True))]
    if args.graph is not None:
        arguments.extend(['--graph', str(args.graph.resolve(strict=True))])
    if args.rows:
        arguments.append('--rows')
    for name in ('max_input_bytes', 'max_rows', 'max_seconds'):
        value = getattr(args, name)
        if value is not None:
            arguments.extend(['--' + name.replace('_', '-'), str(value)])
    operation_seconds = (args.max_seconds if args.max_seconds is not None
                         else _DEFAULT_MAX_SECONDS) + _CLEANUP_SECONDS
    stream = None
    try:
        stream = _native_packets(arguments, prefix=args.native_prefix,
                                 operation_seconds=operation_seconds)
        summary = None
        for packet in stream:
            if not isinstance(packet, dict):
                raise ValueError('native source coverage frame must be an object')
            schema = packet.get('schema_version')
            if schema == _ROW_SCHEMA and args.rows and summary is None:
                print(_wire(packet), flush=True)
            elif schema == _REPORT_SCHEMA and summary is None:
                summary = packet
            else:
                raise ValueError('unexpected native source coverage stream order')
        # The native transport has observed EOF/status and completed child
        # cleanup before iteration completes; only now publish completion.
        if summary is None or summary.get('enumeration_complete') is not True:
            raise ValueError('native source coverage stream has no terminal summary')
        print(_wire(summary), flush=True)
        return 0
    except (ValueError, OSError, RuntimeError, TimeoutError) as error:
        parser.error(str(error))
    finally:
        if stream is not None:
            stream.close()
    return 2


if __name__ == '__main__':
    raise SystemExit(main())
