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
import sqlite3
from pathlib import Path

PRIMARY_KEYS = {
    'edge_meta': ('key', 'part'), 'philosophy_nodes': ('id',), 'philosophy_edges': ('id',),
    'philosophy_aux': ('collection', 'ord'), 'philosophy_clusters': ('id', 'part'),
    'philosophy_cluster_nodes': ('cluster_id', 'member_ord'),
    'philosophy_cluster_edges': ('cluster_id', 'member_ord'),
    'philosophy_review_packets': ('view_id',), 'corpus_items': ('collection', 'ord'),
    'corpus_edges': ('ord',), 'corpus_packs': ('id',), 'knowledge_nodes': ('id',),
    'knowledge_relations': ('id',),
    'knowledge_search_documents': ('kind', 'position'),
    'knowledge_search_grams': ('kind', 'n', 'gram', 'position'),
    'knowledge_search_gram_stats': ('kind', 'n', 'gram'),
    'knowledge_lens_order': ('kind', 'id'),
}
# Keep this producer limit in the same module as the delta parser and import it
# into build_runtime.  A valid producer statement is no larger than this many
# UTF-8 bytes; the row-index limits below are derived from that contract rather
# than from an identifier-shaped guess.
MAX_D1_SQL_STATEMENT_BYTES = 100_000
# append_chunkable_insert admits a complete SQLite row whose SQL value
# literals total at most this many UTF-8 bytes.  Keep the historical row-index
# contract at least this broad even though ordinary primary keys are much
# smaller than one statement.
MAX_D1_SQL_ROW_VALUE_BYTES = 2_000_000
# json.dumps(..., ensure_ascii=False) can expand one source code point to six
# characters (for example ``\\u0000``).  The outer rows object JSON-encodes the
# already JSON-encoded composite key once more, doubling its backslashes and
# quotes.  Reserve framing for the key list/object, digest, and delimiters.
ROW_INDEX_JSON_ESCAPE_MAX_CHARS = 6
ROW_INDEX_JSON_FRAMING_CHARS = 4_096
ROW_INDEX_MAX_INNER_KEY_CHARS = (
    ROW_INDEX_JSON_ESCAPE_MAX_CHARS * MAX_D1_SQL_STATEMENT_BYTES
    + ROW_INDEX_JSON_FRAMING_CHARS
)
ROW_INDEX_MAX_ROW_CHARS = (
    ROW_INDEX_JSON_ESCAPE_MAX_CHARS * MAX_D1_SQL_ROW_VALUE_BYTES
    + ROW_INDEX_JSON_FRAMING_CHARS
)
ROW_INDEX_MAX_KEY_CHARS = (
    2 * ROW_INDEX_MAX_INNER_KEY_CHARS + ROW_INDEX_JSON_FRAMING_CHARS
)
# The current envelope has only short schema/revision fields.  Unknown future
# header values remain bounded and fail closed until their contract is explicit.
ROW_INDEX_MAX_HEADER_CHARS = 8_192
INSERT = re.compile(r'^INSERT INTO (\w+)_next \(([^)]+)\) VALUES (.*);$', re.S)


def sql_value_literals(text: str) -> list[str]:
    """Split one producer VALUES row without interpreting its literals."""
    result = []
    start = 0
    quoted = False
    depth = 0
    index = 0
    while index < len(text):
        char = text[index]
        if char == "'":
            if quoted and index + 1 < len(text) and text[index + 1] == "'":
                index += 2
                continue
            quoted = not quoted
        elif not quoted and char == '(':
            depth += 1
        elif not quoted and char == ')':
            depth -= 1
            if depth < 0:
                raise ValueError('invalid producer SQL value literals')
        elif char == ',' and not quoted and depth == 0:
            result.append(text[start:index].strip())
            start = index + 1
        index += 1
    if quoted or depth:
        raise ValueError('invalid producer SQL value literals')
    result.append(text[start:].strip())
    if any(not value for value in result):
        raise ValueError('invalid producer SQL value literals')
    return result


def sql_value_rows(text: str) -> list[str]:
    """Split a bounded INSERT VALUES list into parenthesis-free row bodies."""
    rows = []
    index = 0
    length = len(text)
    while index < length:
        while index < length and text[index].isspace():
            index += 1
        if index >= length or text[index] != '(':
            raise ValueError('invalid producer SQL VALUES rows')
        start = index + 1
        depth = 1
        quoted = False
        index += 1
        while index < length:
            char = text[index]
            if char == "'":
                if quoted and index + 1 < length and text[index + 1] == "'":
                    index += 2
                    continue
                quoted = not quoted
            elif not quoted and char == '(':
                depth += 1
            elif not quoted and char == ')':
                depth -= 1
                if depth == 0:
                    rows.append(text[start:index].strip())
                    index += 1
                    break
            index += 1
        else:
            raise ValueError('invalid producer SQL VALUES rows')
        if quoted or depth:
            raise ValueError('invalid producer SQL VALUES rows')
        while index < length and text[index].isspace():
            index += 1
        if index == length:
            return rows
        if text[index] != ',':
            raise ValueError('invalid producer SQL VALUES rows')
        index += 1
    return rows


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


class DiskRowIndex:
    """Bounded-on-RAM row index used by the offline full producer.

    The published ``read-model.rows.json`` shape is intentionally unchanged.
    This temporary SQLite store keeps the same key/digest/value-literal records
    on disk while SQL is emitted, then writes that JSON shape in producer table
    and encounter order at finish.  The sidecar is never a serving carrier.
    """

    def __init__(self, path: Path) -> None:
        self.path = path
        self.path.parent.mkdir(parents=True, exist_ok=True)
        # A previous interrupted attempt cannot be a baseline for this run.
        # The exact sidecar path belongs to the caller's locked build runtime.
        self.path.unlink(missing_ok=True)
        self.connection = sqlite3.connect(self.path)
        self.connection.execute("PRAGMA journal_mode=OFF")
        self.connection.execute("PRAGMA synchronous=OFF")
        self.connection.execute("PRAGMA temp_store=FILE")
        self.connection.execute("PRAGMA cache_size=-32768")
        self.connection.execute(
            "CREATE TABLE rows ("
            "table_name TEXT NOT NULL, sequence INTEGER NOT NULL, "
            "row_key TEXT NOT NULL, digest TEXT NOT NULL, values_json TEXT NOT NULL, "
            "PRIMARY KEY (table_name, row_key)) WITHOUT ROWID"
        )
        self.connection.execute(
            "CREATE INDEX rows_table_sequence ON rows(table_name, sequence)"
        )
        self.sequence = 0
        self.closed = False

    def record(self, table: str, key: str, digest: str, values: list[str]) -> None:
        values_json = json.dumps(values, ensure_ascii=False, separators=(",", ":"))
        # The producer's statement byte limit and the nested JSON framing above
        # bound both representations.  Keep writer and streaming reader on the
        # same fail-closed contract for malformed/future callers too.
        if len(json.dumps(key, ensure_ascii=False, separators=(",", ":"))) > ROW_INDEX_MAX_KEY_CHARS:
            raise ValueError("producer row key exceeds its bounded JSON size")
        if len(values_json) + len('{"digest":"' + digest + '","values":}') > ROW_INDEX_MAX_ROW_CHARS:
            raise ValueError("producer row values exceed their bounded JSON size")
        try:
            self.connection.execute(
                "INSERT INTO rows(table_name,sequence,row_key,digest,values_json) VALUES (?,?,?,?,?)",
                (
                    table,
                    self.sequence,
                    key,
                    digest,
                    values_json,
                ),
            )
        except sqlite3.IntegrityError as error:
            raise ValueError(f"duplicate producer row {table}:{key}") from error
        self.sequence += 1
        # Keep rollback/journal and dirty-page retention bounded even for a
        # large full corpus.  The sidecar is disposable and never published.
        if self.sequence % 4096 == 0:
            self.connection.commit()

    def contains(self, table: str, key: str) -> bool:
        return self.connection.execute(
            "SELECT 1 FROM rows WHERE table_name=? AND row_key=? LIMIT 1",
            (table, key),
        ).fetchone() is not None

    def iter_table(self, table: str):
        return self.connection.execute(
            "SELECT row_key,digest,values_json FROM rows "
            "WHERE table_name=? ORDER BY sequence",
            (table,),
        )

    def write_json(self, output: Path, schema: str, revision: str) -> None:
        """Materialize the historical JSON row-index contract once, at EOF."""
        self.connection.commit()
        output.parent.mkdir(parents=True, exist_ok=True)
        with output.open("w", encoding="utf-8") as stream:
            stream.write("{\"schema\":")
            stream.write(json.dumps(schema, ensure_ascii=False, separators=(",", ":")))
            stream.write(",\"revision\":")
            stream.write(json.dumps(revision, ensure_ascii=False, separators=(",", ":")))
            stream.write(",\"rows\":{")
            for table_index, table in enumerate(PRIMARY_KEYS):
                if table_index:
                    stream.write(",")
                stream.write(json.dumps(table, ensure_ascii=False, separators=(",", ":")))
                stream.write(":{")
                first = True
                for key, digest, values_json in self.iter_table(table):
                    if not first:
                        stream.write(",")
                    first = False
                    stream.write(json.dumps(key, ensure_ascii=False, separators=(",", ":")))
                    stream.write(":")
                    stream.write("{\"digest\":")
                    stream.write(json.dumps(digest, ensure_ascii=False, separators=(",", ":")))
                    stream.write(",\"values\":")
                    stream.write(values_json)
                    stream.write("}")
                stream.write("}")
            stream.write("}}\n")

    def close(self) -> None:
        if self.closed:
            return
        try:
            self.connection.commit()
        finally:
            try:
                self.connection.close()
            finally:
                self.closed = True


class _JsonStream:
    """Small incremental JSON reader used only for the previous row index."""

    def __init__(self, path: Path, *, chunk_chars: int = 64 * 1024) -> None:
        self.stream = path.open("r", encoding="utf-8")
        self.chunk_chars = chunk_chars
        self.buffer = ""
        self.position = 0
        self.eof = False
        self.decoder = json.JSONDecoder()

    def close(self) -> None:
        self.stream.close()

    def _compact(self) -> None:
        if self.position:
            self.buffer = self.buffer[self.position:]
            self.position = 0

    def _fill(self, max_chars: int | None = None) -> None:
        if self.eof:
            return
        self._compact()
        if max_chars is not None:
            remaining = max_chars - len(self.buffer)
            if remaining <= 0:
                raise ValueError("row index JSON value exceeds its bounded size")
            chunk = self.stream.read(min(self.chunk_chars, remaining))
        else:
            chunk = self.stream.read(self.chunk_chars)
        if chunk:
            self.buffer += chunk
        else:
            self.eof = True

    def _skip_whitespace(self) -> None:
        while True:
            while self.position < len(self.buffer) and self.buffer[self.position].isspace():
                self.position += 1
            if self.position < len(self.buffer) or self.eof:
                return
            self._fill()

    def _peek(self) -> str | None:
        self._skip_whitespace()
        if self.position >= len(self.buffer):
            return None
        return self.buffer[self.position]

    def _expect(self, expected: str) -> None:
        self._skip_whitespace()
        if self.position >= len(self.buffer) or self.buffer[self.position] != expected:
            raise ValueError(f"invalid row index JSON; expected {expected!r}")
        self.position += 1

    def value(self, *, max_chars: int | None = None):
        self._skip_whitespace()
        self._compact()
        while True:
            if not self.buffer and not self.eof:
                self._fill(max_chars)
            try:
                value, end = self.decoder.raw_decode(self.buffer, 0)
            except json.JSONDecodeError as error:
                if self.eof:
                    raise ValueError("invalid row index JSON") from error
                if max_chars is not None and len(self.buffer) >= max_chars:
                    raise ValueError("row index JSON value exceeds its bounded size") from error
                self._fill(max_chars)
                continue
            if max_chars is not None and end > max_chars:
                raise ValueError("row index JSON value exceeds its bounded size")
            self.position = end
            return value

    def string(self, *, max_chars: int | None = None) -> str:
        value = self.value(max_chars=max_chars)
        if not isinstance(value, str):
            raise ValueError("row index JSON object keys must be strings")
        return value

    def object(self, consume, *, key_max_chars: int | None = None):
        self._expect("{")
        if self._peek() == "}":
            self.position += 1
            return
        while True:
            key = self.string(max_chars=key_max_chars)
            self._expect(":")
            consume(key, self)
            separator = self._peek()
            if separator == "}":
                self.position += 1
                return
            self._expect(",")


class DiskRowBaseline:
    """Stream a published row-index JSON into a disposable lookup database.

    Incremental builds need random digest lookups while producing the next
    carrier, but loading the historical JSON into one Python object defeats the
    producer's memory bound.  Only one JSON row is decoded at a time; the
    complete previous index remains on disk and is never published.
    """

    def __init__(self, source: Path, expected_schema: str, path: Path | None = None,
                 *, max_value_chars: int = ROW_INDEX_MAX_ROW_CHARS) -> None:
        if max_value_chars < 1:
            raise ValueError("row index JSON value bound must be positive")
        self.source = source
        self.path = path or source.with_name(source.name + ".baseline.sqlite")
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.unlink(missing_ok=True)
        self.connection = sqlite3.connect(self.path)
        self.connection.execute("PRAGMA journal_mode=OFF")
        self.connection.execute("PRAGMA synchronous=OFF")
        self.connection.execute("PRAGMA temp_store=FILE")
        self.connection.execute("PRAGMA cache_size=-32768")
        self.connection.execute(
            "CREATE TABLE rows ("
            "table_name TEXT NOT NULL, sequence INTEGER NOT NULL, "
            "row_key TEXT NOT NULL, digest TEXT NOT NULL, values_json TEXT NOT NULL, "
            "PRIMARY KEY (table_name, row_key)) WITHOUT ROWID"
        )
        self.connection.execute(
            "CREATE INDEX rows_table_sequence ON rows(table_name, sequence)"
        )
        self.schema = None
        self.revision = None
        self.rows_seen = False
        self.closed = False
        self._load(expected_schema, max_value_chars)

    def _load(self, expected_schema: str, max_value_chars: int) -> None:
        parser = _JsonStream(self.source)
        sequence = 0

        def read_rows(_key: str, stream: _JsonStream) -> None:
            nonlocal sequence

            def read_table(table: str, table_stream: _JsonStream) -> None:
                nonlocal sequence

                def read_row(row_key: str, row_stream: _JsonStream) -> None:
                    nonlocal sequence
                    row = row_stream.value(max_chars=max_value_chars)
                    if not isinstance(row, dict):
                        raise ValueError("row index entries must be objects")
                    digest = row.get("digest")
                    values = row.get("values")
                    if not isinstance(digest, str) or not isinstance(values, list):
                        raise ValueError("row index entries require digest and values")
                    self.connection.execute(
                        "INSERT INTO rows(table_name,sequence,row_key,digest,values_json) VALUES (?,?,?,?,?)",
                        (
                            table,
                            sequence,
                            row_key,
                            digest,
                            json.dumps(values, ensure_ascii=False, separators=(",", ":")),
                        ),
                    )
                    sequence += 1
                    if sequence % 4096 == 0:
                        self.connection.commit()

                table_stream.object(read_row, key_max_chars=ROW_INDEX_MAX_KEY_CHARS)

            stream.object(read_table, key_max_chars=ROW_INDEX_MAX_KEY_CHARS)

        def read_header(key: str, stream: _JsonStream) -> None:
            if key == "schema":
                self.schema = stream.value(max_chars=ROW_INDEX_MAX_HEADER_CHARS)
            elif key == "revision":
                self.revision = stream.value(max_chars=ROW_INDEX_MAX_HEADER_CHARS)
            elif key == "rows":
                self.rows_seen = True
                read_rows(key, stream)
            else:
                # Keep the parser strict about structure while ignoring only
                # future top-level metadata fields.
                stream.value(max_chars=ROW_INDEX_MAX_HEADER_CHARS)

        try:
            parser.object(read_header, key_max_chars=ROW_INDEX_MAX_KEY_CHARS)
            if parser._peek() is not None:
                raise ValueError("invalid row index JSON trailing content")
        finally:
            parser.close()
        self.connection.commit()
        if self.schema != expected_schema:
            # Keep the parsed metadata for DeltaRecorder's schema gate, but no
            # rows from a different schema may participate in a delta.
            self.connection.execute("DELETE FROM rows")
            self.connection.commit()
        elif not self.rows_seen or not isinstance(self.revision, str):
            raise ValueError("row index baseline is missing its revision or rows")

    def digest(self, table: str, key: str) -> str | None:
        row = self.connection.execute(
            "SELECT digest FROM rows WHERE table_name=? AND row_key=? LIMIT 1",
            (table, key),
        ).fetchone()
        return None if row is None else row[0]

    def tables(self):
        return self.connection.execute(
            "SELECT table_name FROM rows GROUP BY table_name ORDER BY min(sequence)"
        )

    def iter_table(self, table: str):
        return self.connection.execute(
            "SELECT row_key,digest,values_json FROM rows "
            "WHERE table_name=? ORDER BY sequence",
            (table,),
        )

    def close(self) -> None:
        if self.closed:
            return
        try:
            self.connection.commit()
        finally:
            try:
                self.connection.close()
            finally:
                self.closed = True
                self.path.unlink(missing_ok=True)

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:
            pass


class DeltaRecorder:
    def __init__(
        self,
        target: Path,
        revision: str,
        schema: str,
        previous: dict | None = None,
        *,
        index_store_path: Path | None = None,
    ):
        self.target = target
        self.revision = revision
        self.schema = schema
        self._disk_baseline = previous if isinstance(previous, DiskRowBaseline) else None
        if self._disk_baseline is not None:
            self.previous = self._disk_baseline if self._disk_baseline.schema == schema else None
        else:
            self.previous = previous if previous and previous.get('schema') == schema else None
        self._disk_index = DiskRowIndex(index_store_path) if index_store_path is not None else None
        self.index: dict[str, dict[str, dict]] = ({table: {} for table in PRIMARY_KEYS}
                                                   if self._disk_index is None else {})
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
        self.finished = False
        self.published = False
        if self.previous:
            for table, keys in PRIMARY_KEYS.items():
                stage = self.stage(table)
                self.write(f'DROP TABLE IF EXISTS {stage};')
                self.write(f'CREATE TABLE {stage} AS SELECT * FROM {table} WHERE 0;')
                self.write(f'DROP TABLE IF EXISTS {stage}_keys;')
                self.write(f'CREATE TABLE {stage}_keys AS SELECT {", ".join(keys)} FROM {table} WHERE 0;')
                self.write(f'CREATE UNIQUE INDEX {stage}_keys_idx ON {stage}_keys ({", ".join(keys)});')

    def __del__(self) -> None:
        # A failed producer must leave its diagnostic .next SQL file readable,
        # but no open descriptor or disposable SQLite sidecar.  Normal finish
        # already performs these actions explicitly; this is only the failure
        # path after a caller abandons the recorder.
        try:
            stream = getattr(self, 'stream', None)
            if stream is not None and not stream.closed:
                stream.close()
            index = getattr(self, '_disk_index', None)
            if index is not None:
                index.close()
                index.path.unlink(missing_ok=True)
            baseline = getattr(self, '_disk_baseline', None)
            if baseline is not None:
                baseline.close()
        except Exception:
            pass

    def stage(self, table: str) -> str:
        return self.prefix + '_' + table

    def write(self, statement: str):
        if len(statement.encode('utf-8')) > MAX_D1_SQL_STATEMENT_BYTES:
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
            rows = sql_value_rows(values_text)
            if len(rows) == 1:
                values = sql_value_literals(rows[0])
                key_values = [values[position] for position in positions]
                key = json.dumps(key_values, ensure_ascii=False, separators=(',', ':'))
                self.pending = table, key, key_values, [statement]
            else:
                # The producer batches only independent posting/stat rows.
                # Split them before indexing so unchanged rows can be omitted
                # from a delta and changed rows can be staged individually.
                for row in rows:
                    values = sql_value_literals(row)
                    key_values = [values[position] for position in positions]
                    key = json.dumps(key_values, ensure_ascii=False, separators=(',', ':'))
                    row_statement = f'INSERT INTO {table}_next ({columns_text}) VALUES ({row});'
                    self._record(table, key, key_values, [row_statement])
        elif self.pending and statement.startswith(f'UPDATE {self.pending[0]}_next SET '):
            self.pending[3].append(statement)
        else:
            self.flush()

    def _record(self, table: str, key: str, values: list[str], statements: list[str]):
        digest = hashlib.sha256('\n'.join(statements).encode()).hexdigest()
        if self._disk_index is None:
            if key in self.index[table]:
                raise ValueError(f'duplicate producer row {table}:{key}')
            self.index[table][key] = {'digest': digest, 'values': values}
        else:
            self._disk_index.record(table, key, digest, values)
        if self._disk_baseline is not None:
            before_digest = self._disk_baseline.digest(table, key) if self.previous else None
        else:
            before = (self.previous or {}).get('rows', {}).get(table, {}).get(key)
            before_digest = before.get('digest') if before else None
        if before_digest == digest:
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

    def flush(self):
        if not self.pending:
            return
        table, key, values, statements = self.pending
        self.pending = None
        self._record(table, key, values, statements)

    def finish(self, *, index_output: Path | None = None, publish: bool = True) -> dict | None:
        if self._disk_index is not None and index_output is None:
            raise ValueError('disk-backed row index requires an index output path')
        self.flush()
        if self.previous:
            if self._disk_baseline is not None:
                baseline_tables = ((table, self._disk_baseline.iter_table(table))
                                   for (table,) in self._disk_baseline.tables())
            else:
                baseline_tables = self.previous['rows'].items()
            for table, prior_rows in baseline_tables:
                if table not in PRIMARY_KEYS:
                    raise ValueError('baseline contains an unknown table')
                rows = (prior_rows.items() if self._disk_baseline is None else
                        ((key, {'digest': digest, 'values': json.loads(values_json)})
                         for key, digest, values_json in prior_rows))
                for key, before in rows:
                    present = (self._disk_index.contains(table, key)
                               if self._disk_index is not None else key in self.index[table])
                    if not present:
                        if len(before['values']) != len(PRIMARY_KEYS[table]) or any(not re.fullmatch(r"'(?:''|[^'])*'|-?[0-9]+|NULL", value, re.S) for value in before['values']):
                            raise ValueError('invalid baseline key literal')
                        self.removed_rows += 1
                        self.key_counts[table] += 1
                        self.write(f'INSERT INTO {self.stage(table)}_keys VALUES ({", ".join(before["values"])});')
            base = (self._disk_baseline.revision if self._disk_baseline is not None
                    else self.previous['revision'])
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
        self.finished = True
        if self._disk_index is not None:
            try:
                self._disk_index.write_json(index_output, self.schema, self.revision)
            finally:
                self._disk_index.close()
                # The JSON row index is the durable companion.  The SQLite
                # index is only a bounded build-time implementation detail.
                self._disk_index.path.unlink(missing_ok=True)
            if publish:
                self.publish()
            if self._disk_baseline is not None:
                self._disk_baseline.close()
            return None
        if publish:
            self.publish()
        if self._disk_baseline is not None:
            self._disk_baseline.close()
        return {'schema': self.schema, 'revision': self.revision, 'rows': self.index}

    def publish(self) -> None:
        if not self.finished:
            raise ValueError('delta must be finished before publication')
        if self.published:
            return
        self.pending_path.replace(self.target)
        self.published = True

    def summary(self) -> dict:
        return {'available': self.previous is not None,
                'base_revision': ((self._disk_baseline.revision if self._disk_baseline is not None
                                   else self.previous['revision']) if self.previous else None),
                'target_revision': self.revision, 'changed_rows': self.changed_rows,
                'reused_rows': self.reused_rows, 'removed_rows': self.removed_rows,
                'sql_statements': self.statements, 'publication': 'single-statement-transaction',
                'resume': 'replay-staging-and-idempotent-publication'}
