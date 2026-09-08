#!/usr/bin/env python3
"""Frame bounded producer SQL without changing literals or publication triggers.

Producer records end with LF, but their SQL may contain literal line endings
and trigger-body semicolons. SQLite determines completeness; callers execute
one statement at a time or copy its original bytes into a bounded upload file.
This is framing, not SQL syntax or safety validation: input must come from the
trusted producer and remain immutable across chunk calls.
"""
from __future__ import annotations

import argparse
import json
import sqlite3
from pathlib import Path
from typing import BinaryIO, Iterator


MAX_SQL_STATEMENT_BYTES = 100_000


def sql_statements(stream: BinaryIO) -> Iterator[bytes]:
    pending = b''
    # The producer's record separator is not part of its SQL byte budget.
    # Also accept CRLF separators without translating any literal CR/LF.
    envelope = MAX_SQL_STATEMENT_BYTES + 2
    while fragment := stream.readline(envelope + 1):
        pending += fragment
        if len(pending) > envelope:
            raise ValueError('producer SQL statement exceeds 100000 bytes')
        if sqlite3.complete_statement(pending.decode('utf-8')):
            if len(pending.rstrip(b'\r\n')) > MAX_SQL_STATEMENT_BYTES:
                raise ValueError('producer SQL statement exceeds 100000 bytes')
            yield pending
            pending = b''
    if pending.strip():
        raise ValueError('producer SQL ends with an incomplete statement')
    if pending:
        yield pending  # Preserve trailing whitespace for byte-exact chunk copies.


def write_sql_chunk(source: Path, output: Path, offset: int, maximum_bytes: int) -> dict:
    """Write at most one upload chunk and return its next source byte offset.

    A single legal statement may exceed the preferred chunk size. No later
    chunk is written in advance; the caller resumes only after consuming this
    file. Lookahead is bounded to one statement and never normalizes SQL bytes.
    """
    if offset < 0 or maximum_bytes < 1:
        raise ValueError('SQL offset must be nonnegative and chunk size positive')
    written = 0
    count = 0
    next_offset = offset
    eof = True
    with source.open('rb') as stream, output.open('xb') as target:
        stream.seek(offset)
        for statement in sql_statements(stream):
            if written and written + len(statement) > maximum_bytes:
                eof = False
                break
            target.write(statement)
            written += len(statement)
            count += bool(statement.strip())
            next_offset = stream.tell()
    return {'next_offset': next_offset, 'bytes': written, 'statements': count, 'eof': eof}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--offset', type=int, required=True)
    parser.add_argument('--maximum-bytes', type=int, required=True)
    args = parser.parse_args()
    print(json.dumps(write_sql_chunk(args.source, args.output, args.offset, args.maximum_bytes)))
