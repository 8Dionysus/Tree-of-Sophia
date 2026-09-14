"""Demand-driven, resumable DAG of pure access-projection steps.

Completed outputs are content-addressed. Run/dependency records are execution
metadata, not corpus assertions or acceptance. No worker or query opens this
database: the offline builder explicitly supplies its cache connection.
"""
from __future__ import annotations

import hashlib
import json
import uuid
import time
from dataclasses import dataclass, field
from typing import Callable

DEFAULT_CACHE_BYTES = 1024 * 1024 * 1024
DEFAULT_CACHE_ENTRIES = 500000

def digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


@dataclass(frozen=True)
class Input:
    id: str
    value: object


@dataclass(frozen=True)
class Task:
    id: str
    version: str
    dependencies: tuple
    inputs: object
    action: Callable = field(compare=False, repr=False)


class CompletedSteps:
    """Bounded, disposable outputs. Execution receipts never require retention."""
    def __init__(self, db, max_bytes=DEFAULT_CACHE_BYTES, max_entries=DEFAULT_CACHE_ENTRIES):
        if max_bytes < 1 or max_entries < 1:
            raise ValueError('cache limits must be positive')
        self.db, self.max_bytes, self.max_entries = db, max_bytes, max_entries
        db.execute('CREATE TABLE IF NOT EXISTS completed_step_usage (cache_key TEXT PRIMARY KEY, bytes INTEGER NOT NULL, used INTEGER NOT NULL)')
        if 'digest' not in {row[1] for row in db.execute('PRAGMA table_info(completed_step_usage)')}:
            db.execute('ALTER TABLE completed_step_usage ADD COLUMN digest TEXT')
        db.execute('CREATE INDEX IF NOT EXISTS completed_step_usage_age ON completed_step_usage(used,cache_key)')
        db.execute('INSERT OR IGNORE INTO completed_step_usage(cache_key,bytes,used) SELECT cache_key,length(CAST(payload AS BLOB)),0 FROM completed_steps')
        db.execute('DELETE FROM completed_step_usage WHERE cache_key NOT IN (SELECT cache_key FROM completed_steps)')
        self.bytes, self.entries = db.execute('SELECT coalesce(sum(bytes),0),count(*) FROM completed_step_usage').fetchone()
        self.evicted = self.oversized = 0
        self.trim()

    def get(self, key):
        found, payload = self.get_serialized(key)
        return (True, json.loads(payload)) if found else (False, None)

    def get_serialized(self, key):
        row = self.db.execute('SELECT p.payload,u.digest,u.bytes FROM completed_steps p JOIN completed_step_usage u USING(cache_key) WHERE p.cache_key=?', (key,)).fetchone()
        if row is not None:
            if row[1] != hashlib.sha256(row[0].encode()).hexdigest():
                # Legacy/unverified or corrupted cache entries are disposable.
                self.db.execute('DELETE FROM completed_steps WHERE cache_key=?', (key,))
                self.db.execute('DELETE FROM completed_step_usage WHERE cache_key=?', (key,))
                self.bytes -= row[2]; self.entries -= 1; self.evicted += 1
                return False, None
            self.db.execute('UPDATE completed_step_usage SET used=? WHERE cache_key=?', (time.time_ns(), key))
            return True, row[0]
        return False, None

    def put(self, key, value):
        payload = json.dumps(value, ensure_ascii=False, separators=(',', ':'))
        size = len(payload.encode())
        if size > self.max_bytes:
            self.oversized += 1
            return payload  # Only caching is declined, not transport of the result.
        old = self.db.execute('SELECT bytes FROM completed_step_usage WHERE cache_key=?', (key,)).fetchone()
        self.db.execute('INSERT OR REPLACE INTO completed_steps VALUES (?,?)', (key, payload))
        self.db.execute('INSERT OR REPLACE INTO completed_step_usage VALUES (?,?,?,?)',
                        (key, size, time.time_ns(), hashlib.sha256(payload.encode()).hexdigest()))
        self.bytes += size - (old[0] if old else 0)
        self.entries += int(old is None)
        self.trim()
        return payload

    def trim(self):
        while self.bytes > self.max_bytes or self.entries > self.max_entries:
            # A small oldest-first batch avoids loading all cache keys in RAM.
            rows = self.db.execute('SELECT cache_key,bytes FROM completed_step_usage ORDER BY used,cache_key LIMIT 128').fetchall()
            if not rows:
                break
            for key, size in rows:
                if self.bytes <= self.max_bytes and self.entries <= self.max_entries:
                    break
                self.db.execute('DELETE FROM completed_steps WHERE cache_key=?', (key,))
                self.db.execute('DELETE FROM completed_step_usage WHERE cache_key=?', (key,))
                self.bytes -= size; self.entries -= 1; self.evicted += 1

    def report(self):
        return {'payload_bytes':self.bytes, 'entries':self.entries, 'max_payload_bytes':self.max_bytes,
                'max_entries':self.max_entries, 'evicted_outputs':self.evicted, 'uncached_oversized_outputs':self.oversized}


class ProcessingScheduler:
    def __init__(self, connection, *, max_cache_bytes=DEFAULT_CACHE_BYTES, max_cache_entries=DEFAULT_CACHE_ENTRIES):
        self.db = connection
        self.run_id = uuid.uuid4().hex
        self.db.executescript('''
          CREATE TABLE IF NOT EXISTS processing_runs (
            id TEXT PRIMARY KEY, status TEXT NOT NULL, error_kind TEXT
          );
          CREATE TABLE IF NOT EXISTS processing_tasks (
            run_id TEXT NOT NULL, id TEXT NOT NULL, kind TEXT NOT NULL,
            cache_key TEXT, output_digest TEXT, status TEXT NOT NULL,
            PRIMARY KEY(run_id,id)
          );
          CREATE TABLE IF NOT EXISTS processing_dependencies (
            run_id TEXT NOT NULL, task_id TEXT NOT NULL, dependency_id TEXT NOT NULL,
            PRIMARY KEY(run_id,task_id,dependency_id)
          );
          CREATE TABLE IF NOT EXISTS completed_steps (cache_key TEXT PRIMARY KEY, payload TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS processing_publication (
            singleton INTEGER PRIMARY KEY CHECK(singleton=1), run_id TEXT NOT NULL
          );
          CREATE TABLE IF NOT EXISTS processing_run_bases (run_id TEXT PRIMARY KEY, base_run_id TEXT);
        ''')
        self.outputs = CompletedSteps(self.db, max_cache_bytes, max_cache_entries)
        self.baseline = self.db.execute('SELECT run_id FROM processing_publication WHERE singleton=1').fetchone()
        self.db.execute("INSERT INTO processing_runs VALUES (?, 'running', NULL)", (self.run_id,))
        self.db.execute('INSERT INTO processing_run_bases VALUES (?,?)', (self.run_id, self.baseline[0] if self.baseline else None))
        self.db.commit()
        self.results = {}
        self.definitions = {}
        self.visiting = set()
        self.reused = self.executed = 0
        self.failed = False
        self.by_kind = {}

    def evaluate(self, node):
        serialized, output_digest = self._evaluate(node)
        return json.loads(serialized), output_digest

    def _evaluate(self, node):
        """Carry serialized dependencies until a missing task needs their values.

        Reuse metadata may remain transactional until a new output, failure or
        finish. Abrupt process loss may discard those receipts, never an already
        committed output or publish an incomplete dependency snapshot.
        """
        if not isinstance(node, (Input, Task)) or not node.id:
            self.failed = True
            raise ValueError('processing dependency must be an identified Input or Task')
        if node.id in self.visiting:
            self.failed = True
            raise ValueError('cyclic processing dependency: ' + node.id)
        # One meaning per task ID within a run. The caller can keep immutable
        # historical keys, but cannot silently replace a definition mid-build.
        previous = self.definitions.get(node.id)
        if previous is not None and previous != node:
            self.failed = True
            raise ValueError('conflicting processing task identity: ' + node.id)
        self.definitions[node.id] = node
        if node.id in self.results:
            return self.results[node.id]
        self.visiting.add(node.id)
        try:
            if isinstance(node, Input):
                value, output_digest = node.value, digest(node.value)
                serialized = json.dumps(value, ensure_ascii=False, separators=(',', ':'))
                self.db.execute('INSERT INTO processing_tasks VALUES (?,?,?,?,?,?)',
                                (self.run_id, node.id, 'input', None, output_digest, 'complete'))
            else:
                dependencies = [self._evaluate(dep) for dep in node.dependencies]
                key = digest([node.id, node.version, node.inputs,
                              [[dep.id, result[1]] for dep, result in zip(node.dependencies, dependencies)]])
                self.db.execute('INSERT INTO processing_tasks VALUES (?,?,?,?,?,?)',
                                (self.run_id, node.id, 'task', key, None, 'running'))
                self.db.executemany('INSERT INTO processing_dependencies VALUES (?,?,?)',
                                   [(self.run_id, node.id, dep.id) for dep in node.dependencies])
                found, serialized = self.outputs.get_serialized(key)
                kind = node.id.split(':', 1)[0]
                counters = self.by_kind.setdefault(kind, {'executed':0, 'reused':0})
                if found:
                    value = json.loads(serialized)
                    self.reused += 1
                    counters['reused'] += 1
                else:
                    value = node.action([json.loads(result[0]) for result in dependencies])
                    serialized = self.outputs.put(key, value)
                    self.executed += 1
                    counters['executed'] += 1
                output_digest = digest(value)
                self.db.execute("UPDATE processing_tasks SET status='complete',output_digest=? WHERE run_id=? AND id=?",
                                (output_digest, self.run_id, node.id))
                if not found:
                    # New pure work is durable immediately. Reuse receipts and
                    # LRU touches can share a transaction: their outputs were
                    # already durable, and finish/error flushes pending metadata.
                    self.db.commit()
            self.results[node.id] = serialized, output_digest
            return serialized, output_digest
        except Exception:
            self.failed = True
            self.db.execute("UPDATE processing_tasks SET status='failed' WHERE run_id=? AND id=?", (self.run_id, node.id))
            self.db.commit()
            raise
        finally:
            self.visiting.remove(node.id)

    def finish(self, error=None):
        if error is None and self.failed:
            failure = RuntimeError('processing graph contains failed tasks; cannot publish')
            self.finish(failure)
            raise failure
        if error is not None:
            self.db.execute("UPDATE processing_runs SET status='failed',error_kind=? WHERE id=?", (type(error).__name__, self.run_id))
            self.db.execute("UPDATE processing_tasks SET status='interrupted' WHERE run_id=? AND status='running'", (self.run_id,))
            self.db.commit()
            return
        if not self.definitions:
            # A caller may have obtained a graph from an in-process read cache.
            # No executed DAG is not an empty authoritative dependency snapshot.
            self.db.execute("UPDATE processing_runs SET status='skipped' WHERE id=?", (self.run_id,))
            self.db.commit()
            return
        # Publish execution metadata only if no competing build changed its
        # baseline. Cached pure outputs remain reusable even for a losing run.
        self.db.commit()
        self.db.execute('BEGIN IMMEDIATE')
        current = self.db.execute('SELECT run_id FROM processing_publication WHERE singleton=1').fetchone()
        if current != self.baseline:
            self.db.execute("UPDATE processing_runs SET status='superseded' WHERE id=?", (self.run_id,))
            self.db.commit()
            raise RuntimeError('processing baseline changed; retry build before publication')
        self.db.execute("UPDATE processing_runs SET status='complete' WHERE id=?", (self.run_id,))
        self.db.execute('INSERT OR REPLACE INTO processing_publication VALUES (1,?)', (self.run_id,))
        self.db.commit()

    def report(self):
        status = self.db.execute('SELECT status FROM processing_runs WHERE id=?', (self.run_id,)).fetchone()[0]
        active_ids = set(self.definitions)
        old_ids = set() if self.baseline is None else {
            r[0] for r in self.db.execute('SELECT id FROM processing_tasks WHERE run_id=?', self.baseline)
        }
        inputs = dict(self.db.execute("SELECT id,output_digest FROM processing_tasks WHERE run_id=? AND kind='input'", (self.run_id,)))
        previous_inputs = {} if self.baseline is None else dict(self.db.execute("SELECT id,output_digest FROM processing_tasks WHERE run_id=? AND kind='input'", self.baseline))
        changes = {'added': sorted(inputs.keys() - previous_inputs.keys()),
                   'changed': sorted(key for key in inputs.keys() & previous_inputs.keys() if inputs[key] != previous_inputs[key]),
                   'removed': sorted(previous_inputs.keys() - inputs.keys()) if status == 'complete' else []}
        return {'run_id': self.run_id, 'executed': self.executed, 'reused': self.reused,
                'steps_by_kind': self.by_kind,
                'input_changes': {kind: {'count':len(ids), 'sample_ids':ids[:100], 'sample_limit':100} for kind,ids in changes.items()},
                'input_coverage': 'complete' if status == 'complete' else 'partial' if inputs else 'not-run',
                'cache': self.outputs.report(),
                'removed_task_ids': sorted(old_ids - active_ids) if status == 'complete' else [],
                'status': status,
                'scope': 'incremental-access-projection', 'is_semantic_acceptance': False}

    def prune_history(self, keep_runs=3):
        """Caller owns exclusive cache access; never use this on source ledgers."""
        if keep_runs < 1:
            raise ValueError('keep_runs must be positive')
        keep = {row[0] for row in self.db.execute('SELECT id FROM processing_runs ORDER BY rowid DESC LIMIT ?', (keep_runs,))}
        keep.add(self.run_id)
        active = self.db.execute('SELECT run_id FROM processing_publication WHERE singleton=1').fetchone()
        if active:
            keep.add(active[0])
        retired = [row[0] for row in self.db.execute('SELECT id FROM processing_runs') if row[0] not in keep]
        for run in retired:
            self.db.execute('DELETE FROM processing_run_bases WHERE run_id=?', (run,))
            self.db.execute('DELETE FROM processing_dependencies WHERE run_id=?', (run,))
            self.db.execute('DELETE FROM processing_tasks WHERE run_id=?', (run,))
            self.db.execute('DELETE FROM processing_runs WHERE id=?', (run,))
        self.db.commit()
        return len(retired)


def processing_input_changes(db, run_id, *, after='', limit=100, source_only=True):
    """Read a bounded, ID-addressed input delta; retired baselines fail explicitly."""
    if not isinstance(after, str) or isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= 1000:
        raise ValueError('invalid processing change page')
    run = db.execute('SELECT status FROM processing_runs WHERE id=?', (run_id,)).fetchone()
    base = db.execute('SELECT base_run_id FROM processing_run_bases WHERE run_id=?', (run_id,)).fetchone()
    if run is None or base is None:
        raise KeyError('processing run or baseline metadata was retired')
    base_id = base[0]
    if base_id is not None and db.execute('SELECT 1 FROM processing_runs WHERE id=?', (base_id,)).fetchone() is None:
        raise KeyError('processing baseline was retired; this delta is no longer available')
    sql = '''WITH current AS (SELECT id,output_digest FROM processing_tasks WHERE run_id=? AND kind='input'),
       previous AS (SELECT id,output_digest FROM processing_tasks WHERE run_id=? AND kind='input'),
       changes AS (
         SELECT c.id,p.output_digest AS before,c.output_digest AS after,
                CASE WHEN p.id IS NULL THEN 'added' ELSE 'changed' END AS change
           FROM current c LEFT JOIN previous p USING(id)
           WHERE p.id IS NULL OR p.output_digest!=c.output_digest
         UNION ALL SELECT p.id,p.output_digest,NULL,'removed' FROM previous p
           LEFT JOIN current c USING(id) WHERE c.id IS NULL AND ?='complete'
       ) SELECT id,before,after,change FROM changes WHERE id>?
    '''
    if source_only:
        sql += " AND (id LIKE 'source-%' OR id LIKE 'entity-type:%' OR id LIKE 'relation-type:%')"
    rows = db.execute(sql + ' ORDER BY id LIMIT ?', (run_id, base_id, run[0], after, limit+1)).fetchall()
    items = [{'id':row[0], 'before':row[1], 'after':row[2], 'change':row[3]} for row in rows[:limit]]
    return {'schema':'tos_processing_input_changes_v1', 'run_id':run_id, 'base_run_id':base_id,
            'coverage':'complete' if run[0]=='complete' else 'partial', 'items':items,
            'next_after':items[-1]['id'] if len(rows)>limit else None, 'is_semantic_acceptance':False}
