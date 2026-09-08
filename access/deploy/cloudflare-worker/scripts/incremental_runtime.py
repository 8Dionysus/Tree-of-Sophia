"""Row-level D1 delta generation, with staged and atomic publication.

The input is our own deterministic SQL producer, not caller-supplied SQL.
Unchanged rows are not uploaded. A publication trigger applies all table deltas
in one SQLite statement/transaction after a compare-and-swap revision guard.
Staging can be interrupted/replayed without changing the serving tables.
"""
from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

PRIMARY_KEYS = {
    'edge_meta': ('key', 'part'), 'philosophy_nodes': ('id',), 'philosophy_edges': ('id',),
    'philosophy_aux': ('collection', 'ord'), 'philosophy_clusters': ('id', 'part'),
    'philosophy_cluster_nodes': ('cluster_id', 'member_ord'),
    'philosophy_cluster_edges': ('cluster_id', 'member_ord'),
    'philosophy_review_packets': ('view_id',), 'corpus_items': ('collection', 'ord'),
    'corpus_edges': ('ord',), 'corpus_packs': ('id',), 'knowledge_nodes': ('id',),
    'knowledge_relations': ('id',),
}
INSERT = re.compile(r'^INSERT INTO (\w+)_next \(([^)]+)\) VALUES \((.*)\);$', re.S)


def sql_prefix_values(text: str, count: int) -> list[str]:
    """Read only the leading key literals; JSON/text columns need no parsing."""
    result = []
    start = 0
    quoted = False
    index = 0
    while index < len(text):
        char = text[index]
        if char == "'":
            if quoted and index + 1 < len(text) and text[index + 1] == "'":
                index += 2
                continue
            quoted = not quoted
        elif char == ',' and not quoted:
            result.append(text[start:index].strip())
            if len(result) == count:
                return result
            start = index + 1
        index += 1
    result.append(text[start:].strip())
    if len(result) < count or quoted:
        raise ValueError('invalid producer SQL key literals')
    return result[:count]


class DeltaRecorder:
    def __init__(self, target: Path, revision: str, schema: str, previous: dict | None = None):
        self.target = target
        self.revision = revision
        self.schema = schema
        self.previous = previous if previous and previous.get('schema') == schema else None
        self.index: dict[str, dict[str, dict]] = {table: {} for table in PRIMARY_KEYS}
        self.staged_counts = {table: 0 for table in PRIMARY_KEYS}
        self.key_counts = {table: 0 for table in PRIMARY_KEYS}
        self.pending: tuple[str, str, list[str], list[str]] | None = None
        self.changed_rows = 0
        self.reused_rows = 0
        self.removed_rows = 0
        self.statements = 0
        self.pending_path = target.with_name(target.name + '.next')
        self.stream = self.pending_path.open('w', encoding='utf-8')
        self.prefix = 'tos_delta_' + revision[:16]
        if self.previous:
            for table, keys in PRIMARY_KEYS.items():
                stage = self.stage(table)
                self.write(f'DROP TABLE IF EXISTS {stage};')
                self.write(f'CREATE TABLE {stage} AS SELECT * FROM {table} WHERE 0;')
                self.write(f'DROP TABLE IF EXISTS {stage}_keys;')
                self.write(f'CREATE TABLE {stage}_keys AS SELECT {", ".join(keys)} FROM {table} WHERE 0;')
                self.write(f'CREATE UNIQUE INDEX {stage}_keys_idx ON {stage}_keys ({", ".join(keys)});')

    def stage(self, table: str) -> str:
        return self.prefix + '_' + table

    def write(self, statement: str):
        if len(statement.encode('utf-8')) > 100_000:
            raise ValueError('delta SQL statement exceeds D1 limit')
        self.stream.write(statement + '\n')
        self.statements += 1

    def observe(self, statement: str):
        match = INSERT.match(statement)
        if match:
            self.flush()
            table, columns_text, values_text = match.groups()
            if table not in PRIMARY_KEYS:
                raise ValueError('unregistered incremental table: ' + table)
            columns = [col.strip() for col in columns_text.split(',')]
            positions = [columns.index(key) for key in PRIMARY_KEYS[table]]
            prefix = sql_prefix_values(values_text, max(positions) + 1)
            values = [prefix[position] for position in positions]
            key = json.dumps(values, ensure_ascii=False, separators=(',', ':'))
            self.pending = table, key, values, [statement]
        elif self.pending and statement.startswith(f'UPDATE {self.pending[0]}_next SET '):
            self.pending[3].append(statement)
        else:
            self.flush()

    def flush(self):
        if not self.pending:
            return
        table, key, values, statements = self.pending
        self.pending = None
        if key in self.index[table]:
            raise ValueError(f'duplicate producer row {table}:{key}')
        digest = hashlib.sha256('\n'.join(statements).encode()).hexdigest()
        self.index[table][key] = {'digest': digest, 'values': values}
        before = (self.previous or {}).get('rows', {}).get(table, {}).get(key)
        if before and before['digest'] == digest:
            self.reused_rows += 1
            return
        self.changed_rows += 1
        if not self.previous:
            return
        stage = self.stage(table)
        self.staged_counts[table] += 1
        self.key_counts[table] += 1
        self.write(f'INSERT INTO {stage}_keys VALUES ({", ".join(values)});')
        for statement in statements:
            self.write(statement.replace(table + '_next', stage, 1))

    def finish(self) -> dict:
        self.flush()
        if self.previous:
            for table, prior_rows in self.previous['rows'].items():
                if table not in PRIMARY_KEYS:
                    raise ValueError('baseline contains an unknown table')
                for key, before in prior_rows.items():
                    if key not in self.index[table]:
                        if len(before['values']) != len(PRIMARY_KEYS[table]) or any(not re.fullmatch(r"'(?:''|[^'])*'|-?[0-9]+|NULL", value, re.S) for value in before['values']):
                            raise ValueError('invalid baseline key literal')
                        self.removed_rows += 1
                        self.key_counts[table] += 1
                        self.write(f'INSERT INTO {self.stage(table)}_keys VALUES ({", ".join(before["values"])});')
            base = self.previous['revision']
            if not re.fullmatch(r'[0-9a-f]{64}', base) or not re.fullmatch(r'[0-9a-f]{64}', self.revision):
                raise ValueError('invalid revision digest')
            current_revision = "(SELECT json_extract(group_concat(json_chunk, ''), '$.sha256') FROM (SELECT json_chunk FROM edge_meta WHERE key = 'data_revision' ORDER BY part))"
            publication = f'{self.prefix}_publish'
            self.write('CREATE TABLE IF NOT EXISTS tos_delta_publications (revision TEXT PRIMARY KEY, base_revision TEXT NOT NULL);')
            self.write(f'DROP TRIGGER IF EXISTS {publication};')
            operations = [f"SELECT CASE WHEN {current_revision} IS NOT '{base}' THEN RAISE(ABORT, 'stale delta baseline') END;"]
            for table, keys in PRIMARY_KEYS.items():
                if not self.key_counts[table]:
                    continue
                stage = self.stage(table)
                operations.append(f"SELECT CASE WHEN (SELECT count(*) FROM {stage}) != {self.staged_counts[table]} OR (SELECT count(*) FROM {stage}_keys) != {self.key_counts[table]} THEN RAISE(ABORT, 'incomplete delta staging') END;")
                equal = ' AND '.join(f'{table}.{key} IS changed.{key}' for key in keys)
                operations.append(f'DELETE FROM {table} WHERE EXISTS (SELECT 1 FROM {stage}_keys changed WHERE {equal});')
                operations.append(f'INSERT INTO {table} SELECT * FROM {stage};')
            operations.append(f"SELECT CASE WHEN {current_revision} IS NOT '{self.revision}' THEN RAISE(ABORT, 'delta revision mismatch') END;")
            self.write(f'CREATE TRIGGER {publication} AFTER INSERT ON tos_delta_publications WHEN NEW.revision = \'{self.revision}\' BEGIN ' + ' '.join(operations) + ' END;')
            # Retry skips only when the actual serving revision is the target.
            # A historical publication receipt alone is never currentness.
            self.write(f"INSERT OR REPLACE INTO tos_delta_publications SELECT '{self.revision}', '{base}' WHERE {current_revision} IS NOT '{self.revision}';")
            self.write(f'DROP TRIGGER {publication};')
            for table in PRIMARY_KEYS:
                self.write(f'DROP TABLE {self.stage(table)};')
                self.write(f'DROP TABLE {self.stage(table)}_keys;')
        self.stream.flush()
        self.stream.close()
        self.pending_path.replace(self.target)
        return {'schema': self.schema, 'revision': self.revision, 'rows': self.index}

    def summary(self) -> dict:
        return {'available': self.previous is not None,
                'base_revision': self.previous['revision'] if self.previous else None,
                'target_revision': self.revision, 'changed_rows': self.changed_rows,
                'reused_rows': self.reused_rows, 'removed_rows': self.removed_rows,
                'sql_statements': self.statements, 'publication': 'single-statement-transaction',
                'resume': 'replay-staging-and-idempotent-publication'}
