"""Platform forwarding for Rust normalized-snapshot coverage observations.

Select installed software with --native-prefix or TOS_NATIVE_PREFIX. Imported
APIs use TOS_NATIVE_PREFIX; missing native software is an explicit error.
No Python display, HumanForm, mapping or aggregation rules remain here.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
from .native_io import _bounded_json

_INPUT_CAP = 16 * 1024 * 1024
_FRAME_CAP = 4 * 1024 * 1024


def _language_argument(language):
    # Host frame/type bound only. The ToS lexical language grammar stays Rust.
    if type(language) is not str or len(language) > 128:
        raise ValueError('coverage language argument exceeds the host frame bound')
    return language


def _packets(arguments, value=None, *, prefix=None):
    from .native_io import native_packets
    cap = _FRAME_CAP if arguments[0] == 'coverage-row' else _INPUT_CAP
    return native_packets(arguments, value, prefix=prefix, input_cap=cap,
                          frame_cap=_FRAME_CAP)


def coverage_row(item, *, language='auto'):
    packets = list(_packets(['coverage-row', '--input', '-', '--language', _language_argument(language)], item))
    if len(packets) != 1:
        raise ValueError('native coverage-row did not return one observation')
    return packets[0]


def coverage_rows(graph, *, language='auto'):
    stream = _packets(['coverage', '--graph', '-', '--language', _language_argument(language), '--rows'], graph)
    summary = None
    try:
        for packet in stream:
            if packet.get('schema_version') == 'tos_knowledge_coverage_row_v1' and summary is None:
                yield packet['observation']
            elif packet.get('schema_version') == 'tos_knowledge_coverage_v1' and summary is None:
                summary = packet
            else:
                raise ValueError('unexpected native coverage stream order')
        if summary is None or summary.get('enumeration_complete') is not True:
            raise ValueError('native coverage stream has no terminal summary')
    finally:
        stream.close()


def coverage_report(graph, *, language='auto', emit_row=None):
    arguments = ['coverage', '--graph', '-', '--language', _language_argument(language)]
    if emit_row is not None:
        arguments.append('--rows')
    stream = _packets(arguments, graph)
    summary = None
    try:
        for packet in stream:
            if packet.get('schema_version') == 'tos_knowledge_coverage_row_v1' and summary is None and emit_row is not None:
                emit_row(packet)
            elif packet.get('schema_version') == 'tos_knowledge_coverage_v1' and summary is None:
                summary = packet
            else:
                raise ValueError('unexpected native coverage stream order')
        if summary is None or summary.get('enumeration_complete') is not True:
            raise ValueError('native coverage stream has no terminal summary')
        return summary
    finally:
        stream.close()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native-prefix', default=os.environ.get('TOS_NATIVE_PREFIX'))
    parser.add_argument('--root', required=True)
    parser.add_argument('--language', default='auto')
    parser.add_argument('--rows', action='store_true')
    for option in ('--max-input-bytes', '--max-rows', '--max-seconds'):
        parser.add_argument(option)
    args = parser.parse_args(argv)
    if len(args.language) > 128:
        parser.error('coverage language argument exceeds the host frame bound')
    arguments = ['coverage', '--root', str(Path(args.root).absolute()), '--language', args.language]
    if args.rows:
        arguments.append('--rows')
    for name in ('max_input_bytes', 'max_rows', 'max_seconds'):
        value = getattr(args, name)
        if value is not None:
            arguments.extend(['--' + name.replace('_', '-'), value])
    try:
        summary = None
        for packet in _packets(arguments, prefix=args.native_prefix):
            schema = packet.get('schema_version')
            if schema == 'tos_knowledge_coverage_v1' and summary is None:
                summary = packet
            elif schema == 'tos_knowledge_coverage_row_v1' and args.rows and summary is None:
                print(json.dumps(packet, ensure_ascii=False, sort_keys=True,
                    separators=(',', ':'), allow_nan=False), flush=True)
            else:
                raise ValueError('unexpected native coverage stream order')
        # Completion is published only after the transport owner has observed
        # successful EOF/status and completed its child cleanup.
        if summary is None or summary.get('enumeration_complete') is not True:
            raise ValueError('native coverage stream has no terminal summary')
        print(json.dumps(summary, ensure_ascii=False, sort_keys=True,
            separators=(',', ':'), allow_nan=False), flush=True)
    except (ValueError, OSError, TimeoutError) as error:
        parser.error(str(error))


if __name__ == '__main__':
    main()
