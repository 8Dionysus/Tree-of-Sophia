#!/usr/bin/env python3
"""Stream producer SQL into an explicitly selected, offline local D1 store.

Local bootstrap accelerator only. Remote imports use Wrangler. The caller
resolves one local store and verifies its serving revision through Wrangler
before and after this operation. Workerd must not be serving this local store
during bootstrap. Never reads SQL into a single large Python/JavaScript string.
"""
from __future__ import annotations

import argparse
import json
import sqlite3
from pathlib import Path


def revision(connection):
    if not connection.execute("SELECT 1 FROM sqlite_master WHERE name='edge_meta'").fetchone():
        return None
    columns = {row[1] for row in connection.execute('PRAGMA table_info(edge_meta)')}
    query = "SELECT json_chunk FROM edge_meta WHERE key='data_revision' ORDER BY part" if 'json_chunk' in columns else "SELECT json FROM edge_meta WHERE key='data_revision'"
    value = ''.join(row[0] for row in connection.execute(query))
    return json.loads(value).get('sha256') if value else None


def import_sql(database: Path, source: Path, base: str | None, target: str) -> int:
    count = 0
    # Existing explicit file only: never create an accidental database path.
    connection = sqlite3.connect(database.resolve().as_uri() + '?mode=rw', uri=True)
    connection.setlimit(sqlite3.SQLITE_LIMIT_LENGTH, 2_000_000)
    connection.setlimit(sqlite3.SQLITE_LIMIT_SQL_LENGTH, 100_000)
    try:
        connection.execute('BEGIN IMMEDIATE')
        if revision(connection) != base:
            raise RuntimeError('local bootstrap baseline changed')
        with source.open(encoding='utf-8') as stream:
            for statement in stream:
                if not statement.strip():
                    continue
                if not sqlite3.complete_statement(statement):
                    raise ValueError('producer SQL must contain one complete statement per line')
                connection.execute(statement)
                count += 1
        if revision(connection) != target:
            raise RuntimeError('local bootstrap target revision mismatch')
        connection.commit()
    except BaseException:
        connection.rollback()
        raise
    finally:
        connection.close()
    return count


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--database', type=Path, required=True)
    parser.add_argument('--sql', type=Path, required=True)
    parser.add_argument('--base', required=True)
    parser.add_argument('--target', required=True)
    args = parser.parse_args()
    count = import_sql(args.database, args.sql, None if args.base == 'null' else args.base, args.target)
    print(json.dumps({'statements': count, 'revision': args.target, 'publication': 'local-sqlite-transaction'}))
