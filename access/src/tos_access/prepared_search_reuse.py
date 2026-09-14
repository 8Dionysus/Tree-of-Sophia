"""Bounded, read-only search donor for an explicit new-file publication.

This is not a delta, source admission or a normalization compatibility claim.
The fresh normalized row stream remains authoritative for the new publication.
"""
from dataclasses import dataclass
from collections import Counter
import copy
import hashlib
import json
from pathlib import Path
import sqlite3
import stat
from typing import Callable

from .compressed_search_store import (DDL, CHUNK_SIZE, PreparedSearchDocument,
    SearchChange, SearchStore, decode_postings, encode_postings, order_key, _reverse_terms, _Writer,
    ALGORITHM, STORAGE_VERSION, MAX_ADDRESS, BLOCK_SIZE)
from .published_read_metadata import (TOP_KEY, emitted_row_digest,
    published_row_digest_key, published_snapshot_binding)


@dataclass(frozen=True)
class PreparedSearchReuse:
    path: Path
    binding: dict
    max_source_bytes: int = 256 * 1024**2
    max_copy_bytes: int = 256 * 1024**2
    max_copy_rows: int = 2_000_000
    max_batch_bytes: int = 8 * 1024**2
    max_queries: int = 2_000_000
    max_vm_steps: int = 1_000_000_000
    max_validation_state_bytes: int = 128 * 1024**2
    progress: Callable[[dict], None] | None = None

    def __post_init__(self):
        if any(type(value) is not int or value < 1 for value in (
                self.max_source_bytes, self.max_copy_bytes, self.max_copy_rows,
                self.max_batch_bytes, self.max_queries, self.max_vm_steps,
                self.max_validation_state_bytes)):
            raise ValueError('positive search reuse budgets required')
        if self.progress is not None and not callable(self.progress):
            raise ValueError('search reuse progress must be callable')


_TABLES = {
    'search_documents': ('doc_id', 'kind', 'identifier', 'sort_key', 'filters'),
    'search_values': ('doc_id', 'category', 'field', 'byte_length'),
    'search_text_chunks': ('doc_id', 'category', 'field', 'chunk', 'payload'),
    'search_terms': ('term_id', 'kind', 'plane', 'n', 'term_key', 'posting_count'),
    'search_blocks': ('term_id', 'lower_fence', 'posting_count', 'payload'),
    'search_document_terms': ('doc_id', 'term_count', 'payload', 'digest'),
}


class _ReadCursor:
    """Charge returned scalar bytes, including metadata and dictionary reads."""
    def __init__(self, cursor, owner):
        self.cursor, self.owner = cursor, owner

    def fetchone(self):
        row = self.cursor.fetchone()
        if row is not None:
            self.owner.source_bytes += sum(len(value) if isinstance(value, bytes)
                else len(str(value).encode('utf-8', 'surrogatepass')) for value in row if value is not None)
            if self.owner.source_bytes > self.owner.request.max_source_bytes:
                raise ValueError('search donor source read budget exceeded')
        return row

    def __iter__(self):
        while (row := self.fetchone()) is not None:
            yield row

    def fetchall(self):
        return list(self)


class _SearchDonor:
    def __init__(self, request, publication_limits):
        if not isinstance(request, PreparedSearchReuse):
            raise ValueError('explicit PreparedSearchReuse required')
        self.request, self.limits = request, publication_limits
        self.binding = copy.deepcopy(request.binding)
        self.db = None
        self.changed = set()
        self.source_bytes = self.changed_bytes = 0
        self.queries = self.vm_steps = self.state_bytes = 0
        self.vm_interval = min(1000, request.max_vm_steps)
        self.terms = {}
        self.documents = {}
        self.rows_digest = hashlib.sha256()
        self.histograms = {'node': Counter(), 'relation': Counter()}

    def execute(self, sql, parameters=()):
        self.queries += 1
        if self.queries > self.request.max_queries:
            raise ValueError('search donor query budget exceeded')
        return _ReadCursor(self.db.execute(sql, parameters), self)

    def _progress(self):
        self.vm_steps += self.vm_interval
        return int(self.vm_steps > self.request.max_vm_steps)

    def _report(self, phase, **details):
        if self.request.progress:
            self.request.progress(dict(phase=phase, source_bytes=self.source_bytes,
                queries=self.queries, vm_steps=self.vm_steps,
                validation_state_bytes=self.state_bytes, changed_documents=len(self.changed),
                changed_bytes=self.changed_bytes, **details))

    def _retain(self, amount):
        self.state_bytes += amount
        if self.state_bytes > self.request.max_validation_state_bytes:
            raise ValueError('search donor validation state budget exceeded')

    def __enter__(self):
        from .prepared_publication import SCHEMA, _metadata
        path = Path(self.request.path).absolute()
        if not stat.S_ISREG(path.lstat().st_mode):
            raise ValueError('search donor must be a regular non-symlink file')
        self.db = sqlite3.connect(path.as_uri() + '?mode=ro', uri=True, isolation_level=None)
        try:
            self.db.setlimit(sqlite3.SQLITE_LIMIT_LENGTH,
                max(self.limits.max_row_bytes, self.request.max_batch_bytes,
                    self.limits.max_metadata_bytes) + 65536)
            self.db.set_progress_handler(self._progress, self.vm_interval)
            self.execute('BEGIN')
            top = _metadata(self, TOP_KEY)
            epoch = self.execute('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1').fetchone()
            if (top.get('read_model_schema') != SCHEMA or epoch is None
                    or published_snapshot_binding(top, epoch[0]) != self.binding):
                raise ValueError('stale or foreign search donor binding')
            self._verify_schema()
            self._verify_metadata(top)
            self.generation = SearchStore._check_header(self, SearchStore._header(self.binding))
            high = self.execute('SELECT high_water FROM search_header WHERE singleton=1').fetchone()[0]
            prepared = self.execute('SELECT high_water FROM prepared_state WHERE singleton=1').fetchone()
            counts = [self.execute('SELECT count(*) FROM ' + table).fetchone()[0]
                      for table in ('prepared_documents', 'search_documents', 'search_document_terms')]
            node_count = self.execute('SELECT count(*) FROM knowledge_nodes').fetchone()[0]
            relation_count = self.execute('SELECT count(*) FROM knowledge_relations').fetchone()[0]
            self.count = node_count + relation_count
            if (prepared is None or high != prepared[0] or high != self.count
                    or counts != [self.count] * 3 or self.count > self.limits.max_mutations):
                raise ValueError('search donor requires a complete dense bootstrap address population')
            for row in self.execute('SELECT term_id,kind,plane,n,term_key,posting_count FROM search_terms ORDER BY term_id'):
                term, kind, plane, n, key, total = row
                if (type(term) is not int or not 1 <= term <= MAX_ADDRESS or kind not in self.histograms
                        or type(plane) is not int or plane not in (0, 1, 2, 3)
                        or type(n) is not int or n not in (0, 1, 2, 3) or not isinstance(key, bytes)
                        or type(total) is not int or total < 1 or total > self.count):
                    raise ValueError('search donor term dictionary framing differs')
                self._retain(512 + len(key))
                self.terms[term] = [kind, plane, n, key, total, 0]
            self._report('donor_metadata_validated', documents=self.count, terms=len(self.terms))
            return self
        except BaseException:
            self.db.close()
            self.db = None
            raise

    def __exit__(self, *_):
        if self.db is not None:
            self.db.set_progress_handler(None, 0)
            if self.db.in_transaction:
                self.db.rollback()
            self.db.close()

    def _verify_schema(self):
        reference = sqlite3.connect(':memory:')
        try:
            reference.executescript(DDL)
            objects = reference.execute('SELECT type,name,tbl_name,sql FROM sqlite_master').fetchall()
            for expected in objects:
                actual = self.execute('SELECT type,name,tbl_name,sql FROM sqlite_master WHERE name=?', (expected[1],)).fetchone()
                normalize = lambda row: None if row is None else (*row[:3], ' '.join((row[3] or '').split()))
                if normalize(actual) != normalize(expected):
                    raise ValueError('search donor storage schema differs')
        finally:
            reference.close()

    def _verify_metadata(self, top):
        from .prepared_publication import (SCHEMA, DESCRIPTOR_SCHEMA, CAPABILITIES,
            _metadata, _hash, _compact, CATALOG_KEY, LENS_META_KEY,
            published_reader_metadata, validate_lens_metadata)
        selected = self.execute('SELECT CASE WHEN length(CAST(descriptor AS BLOB))<=? THEN descriptor END '
                                'FROM prepared_state WHERE singleton=1', (self.limits.max_metadata_bytes,)).fetchone()
        if selected is None or not isinstance(selected[0], str):
            raise ValueError('search donor descriptor absent or oversized')
        descriptor = json.loads(selected[0])
        if (not isinstance(descriptor, dict) or _compact(descriptor) != selected[0]
                or _hash(descriptor) != top['data_revision']
                or _metadata(self, 'data_revision') != {'sha256': top['data_revision']}):
            raise ValueError('search donor descriptor or data revision differs')
        required = dict(schema=DESCRIPTOR_SCHEMA, mode='bootstrap', profile=SCHEMA,
                        algorithm=ALGORITHM, search_storage_version=STORAGE_VERSION,
                        capabilities=CAPABILITIES)
        if any(descriptor.get(key) != value for key, value in required.items()):
            raise ValueError('search donor requires the current bootstrap descriptor')
        catalog = _metadata(self, CATALOG_KEY)
        self.lens = _metadata(self, LENS_META_KEY)
        validate_lens_metadata(self.lens, top['source_revision'])
        if (descriptor.get('catalog_sha256') != _hash(catalog)
                or published_reader_metadata(descriptor.get('header', {}), catalog, SCHEMA,
                    top['data_revision'], lens_metadata=self.lens)[TOP_KEY] != top):
            raise ValueError('search donor catalog, lens or header differs')
        self.descriptor = descriptor
        self._retain(len(selected[0].encode('utf-8')) * 8 + len(_compact(self.lens).encode('utf-8')) * 8)

    def observe(self, kind, identifier, raw, address, token):
        from .prepared_publication import _metadata
        key = (kind, identifier)
        found = self.execute('SELECT doc_id,source_order FROM prepared_documents WHERE kind=? AND id=?', key).fetchone()
        if found != (address, token):
            raise ValueError('search donor identity, address or source order differs; full bootstrap required')
        old = self.execute(f'SELECT CASE WHEN length(CAST(json AS BLOB))<=? THEN json END '
                              f'FROM knowledge_{kind}s WHERE id=?',
                              (self.limits.max_row_bytes, identifier)).fetchone()
        if old is None or not isinstance(old[0], str):
            raise ValueError('search donor carrier missing or oversized')
        if _metadata(self, published_row_digest_key(*key)) != emitted_row_digest(old[0]):
            raise ValueError('search donor carrier checksum differs')
        identity = self.execute('SELECT kind,identifier,sort_key FROM search_documents WHERE doc_id=?', (address,)).fetchone()
        encoded_id = json.dumps(identifier, ensure_ascii=False, sort_keys=True).encode('utf-8', 'surrogatepass')
        if identity != (kind, encoded_id, order_key(identifier, token)):
            raise ValueError('search donor document address or order differs')
        from .prepared_publication import _compact, _DIMENSIONS
        item = json.loads(old[0])
        self.rows_digest.update((_compact([kind, identifier, address, token, emitted_row_digest(old[0])['sha256']]) + '\n').encode('utf-8'))
        dimensions = tuple(str(item.get(field) or '') for field in _DIMENSIONS[kind])
        if dimensions not in self.histograms[kind]:
            self._retain(512 + sum(len(value.encode('utf-8')) * 4 for value in dimensions))
        self.histograms[kind][dimensions] += 1
        self._verify_document(PreparedSearchDocument.from_item(address, kind, item, token))
        if raw != old[0]:
            self.changed.add(key)
            self.changed_bytes += len(raw.encode('utf-8'))
            self._retain(len(raw.encode('utf-8')) * 16 + 1024)
            if len(self.changed) > self.limits.max_changes or self.changed_bytes > self.limits.max_change_bytes:
                raise ValueError('search donor changed-document budget exceeded; full bootstrap required')
        if address % 4096 == 0:
            self._report('donor_source_terms_progress', documents=address, total_documents=self.count)

    def _verify_document(self, document):
        encoded = lambda value: json.dumps(value, ensure_ascii=False, sort_keys=True).encode('utf-8', 'surrogatepass')
        filters = self.execute('SELECT filters FROM search_documents WHERE doc_id=?', (document.doc_id,)).fetchone()
        if filters != (encoded(document.filters),):
            raise ValueError('search donor filters differ from source row')
        values = {(category, field): value.encode('utf-8', 'surrogatepass')
                  for category, entries in (('identity', document.identities),
                                            ('visible', document.visible), ('full', (document.searchable,)))
                  for field, value in enumerate(entries)}
        lengths = self.execute('SELECT category,field,byte_length FROM search_values WHERE doc_id=? LIMIT ?',
                                 (document.doc_id, len(values) + 1)).fetchall()
        if len(lengths) != len(values) or any(values.get((category, field)) is None
                or len(values[category, field]) != size for category, field, size in lengths):
            raise ValueError('search donor value framing differs from source row')
        expected_chunks = sum((len(value) + CHUNK_SIZE - 1) // CHUNK_SIZE for value in values.values())
        chunks = self.execute('SELECT category,field,chunk,CASE WHEN length(payload)<=? THEN payload END '
            'FROM search_text_chunks WHERE doc_id=? ORDER BY category,field,chunk LIMIT ?',
            (CHUNK_SIZE, document.doc_id, expected_chunks + 1))
        count = 0
        for category, field, chunk, payload in chunks:
            value = values.get((category, field))
            if (value is None or type(chunk) is not int or chunk < 0 or not isinstance(payload, bytes)
                    or not payload or payload != value[chunk * CHUNK_SIZE:(chunk + 1) * CHUNK_SIZE]):
                raise ValueError('search donor text differs from source row')
            count += 1
        if count != expected_chunks:
            raise ValueError('search donor text chunk population differs')
        frame = self.execute('SELECT length(payload) FROM search_document_terms WHERE doc_id=?',
                                (document.doc_id,)).fetchone()
        if frame is None or type(frame[0]) is not int:
            raise ValueError('search donor reverse frame absent')
        reverse = _reverse_terms(self, document.doc_id, document.kind, validate_terms=False)
        expected = _Writer.terms(None, document)
        reverse_digest = hashlib.sha256()
        for term in reverse:
            entry = self.terms.get(term)
            if entry is None or entry[0] != document.kind or tuple(entry[1:4]) not in expected:
                raise ValueError('search donor reverse membership differs from source terms')
            expected.remove(tuple(entry[1:4]))
            reverse_digest.update(term.to_bytes(8, 'big'))
        if expected:
            raise ValueError('search donor reverse membership omits source terms')
        key = order_key(document.identifier, document.source_order)
        self._retain(768 + len(key))
        self.documents[document.doc_id] = [document.kind, key, len(reverse), reverse_digest.digest(), hashlib.sha256(), 0]

    def finish(self, count):
        if count != self.count:
            raise ValueError('search donor population differs; full bootstrap required')
        if self.rows_digest.hexdigest() != self.descriptor.get('rows_sha256'):
            raise ValueError('search donor complete carrier digest differs')
        for kind, histogram in self.histograms.items():
            if [[*key, histogram[key]] for key in sorted(histogram)] != self.lens[kind + '_counts']:
                raise ValueError('search donor lens population differs')
        self._report('donor_source_terms_validated', documents=count)

    def initialize(self, target, binding, documents, maximum_pages, max_mutations):
        # Consume the second fresh row pass first: it emits all new carriers and
        # rechecks the complete first-pass digest. Keep only bounded changes.
        changes = [SearchChange('update', document.doc_id, document)
                   for document in documents if (document.kind, document.identifier) in self.changed]
        if len(changes) != len(self.changed):
            raise ValueError('search donor replacement set differs')
        for statement in DDL.split(';'):
            if statement.strip():
                target.execute(statement)
        copied = copy_bytes = 0
        previous_term, previous_last = None, None
        for table, columns in _TABLES.items():
            size = '+'.join(f'coalesce(length(CAST({column} AS BLOB)),0)' for column in columns)
            query = 'SELECT ' + ','.join(f'CASE WHEN ({size})<=? THEN {column} END' for column in columns) + f' FROM {table}'
            if table == 'search_blocks':
                query += ' ORDER BY term_id,lower_fence'
            cursor = self.execute(query, (self.request.max_batch_bytes,) * len(columns))
            batch, batch_bytes = [], 0
            insert = f'INSERT INTO {table} VALUES (' + ','.join('?' for _ in columns) + ')'
            for row in cursor:
                if any(value is None for value in row):
                    raise ValueError('search donor row missing or exceeds copy batch budget')
                if table in ('search_values', 'search_text_chunks') and row[0] not in self.documents:
                    raise ValueError('search donor orphan document value or text chunk')
                if table == 'search_blocks':
                    term, fence, declared, payload = row
                    entry = self.terms.get(term)
                    if (entry is None or not isinstance(fence, bytes) or not isinstance(payload, bytes)
                            or len(payload) > BLOCK_SIZE * 8 or type(declared) is not int
                            or not 0 <= declared <= BLOCK_SIZE):
                        raise ValueError('search donor block framing differs')
                    addresses = decode_postings(payload)
                    if len(addresses) != declared:
                        raise ValueError('search donor block count differs')
                    if payload != encode_postings(addresses):
                        raise ValueError('search donor posting frame is not canonical')
                    if term != previous_term:
                        previous_term, previous_last = term, None
                    if previous_last is not None and previous_last >= fence:
                        raise ValueError('search donor block fence overlaps preceding membership')
                    for address in addresses:
                        document = self.documents.get(address)
                        if (document is None or document[0] != entry[0] or document[1] < fence
                                or (previous_last is not None and document[1] <= previous_last)):
                            raise ValueError('search donor posting identity, kind, order or fence differs')
                        previous_last = document[1]
                        document[4].update(term.to_bytes(8, 'big'))
                        document[5] += 1
                    entry[5] += declared
                size_bytes = sum(len(value) if isinstance(value, bytes) else len(str(value).encode('utf-8')) for value in row)
                copied += 1
                copy_bytes += size_bytes
                if copied > min(self.request.max_copy_rows, max_mutations - 2) or copy_bytes > self.request.max_copy_bytes:
                    raise ValueError('search donor copy budget exceeded')
                if batch and batch_bytes + size_bytes > self.request.max_batch_bytes:
                    target.executemany(insert, batch)
                    batch, batch_bytes = [], 0
                batch.append(row)
                batch_bytes += size_bytes
                if len(batch) >= 512:
                    target.executemany(insert, batch)
                    batch, batch_bytes = [], 0
            if batch:
                target.executemany(insert, batch)
            self._report('donor_table_copied', table=table, copied_rows=copied, copied_bytes=copy_bytes)
        if any(entry[4] != entry[5] for entry in self.terms.values()):
            raise ValueError('search donor term posting total differs')
        if any(document[2] != document[5] or document[3] != document[4].digest()
               for document in self.documents.values()):
            raise ValueError('search donor forward and reverse memberships differ')
        self.terms.clear()
        self.documents.clear()
        old_header = SearchStore._header(self.binding)
        target.execute('INSERT INTO search_header VALUES (1,?,?,?,?)',
                       (old_header, self.generation, self.count, maximum_pages))
        SearchStore.apply_delta_transaction(target, expected_binding=self.binding,
            new_binding=binding, changes=changes,
            max_mutations=min(20_000_000, max_mutations - copied - 1))
        self._report('search_successor_prepared', copied_rows=copied, copied_bytes=copy_bytes,
                     committed=False)
