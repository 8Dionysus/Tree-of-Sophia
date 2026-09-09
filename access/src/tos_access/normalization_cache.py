"""Opt-in, build-time cache of completed pure projection steps.

Queries never create this cache. The builder explicitly chooses an ignored
runtime path; keys include processor bytes, inputs, and actual dependencies.
"""
from __future__ import annotations

import hashlib
import json
import sqlite3
import ast
import sys
from contextvars import ContextVar
from pathlib import Path
from .processing import Input, Task, ProcessingScheduler, DEFAULT_CACHE_BYTES, DEFAULT_CACHE_ENTRIES

active_cache: ContextVar['NormalizationCache | None'] = ContextVar('tos_normalization_cache', default=None)


def normalization_processor_digest(path: Path):
    """Bind the transitive authored helper definitions, not unrelated queries.

    Imports and referenced module constants are included. An AST-based digest
    ignores comments/formatting but changes when a normalizer or its helper does.
    """
    tree = ast.parse(path.read_text(encoding='utf-8'))
    definitions = {}
    imports = []
    for statement in tree.body:
        if isinstance(statement, (ast.Import, ast.ImportFrom)):
            imports.append(statement)
        elif isinstance(statement, (ast.FunctionDef, ast.ClassDef)):
            definitions[statement.name] = statement
        elif isinstance(statement, (ast.Assign, ast.AnnAssign)):
            targets = statement.targets if isinstance(statement, ast.Assign) else [statement.target]
            for target in targets:
                if isinstance(target, ast.Name):
                    definitions[target.id] = statement
    pending = ['_normalize_node', '_normalize_relation', 'validate_knowledge_semantics', '_finalize_knowledge_node']
    selected = {}
    while pending:
        name = pending.pop()
        if name in selected or name not in definitions:
            continue
        statement = selected[name] = definitions[name]
        pending.extend(node.id for node in ast.walk(statement) if isinstance(node, ast.Name))
    material = [ast.dump(s, include_attributes=False) for s in imports]
    material.extend(ast.dump(selected[name], include_attributes=False) for name in sorted(selected))
    # These helpers own dependency projection and cache admission as well.
    for helper in (Path(__file__), Path(__file__).with_name('processing.py')):
        material.append(ast.dump(ast.parse(helper.read_text(encoding='utf-8')), include_attributes=False))
    material.append(str(sys.version_info[:2]))
    return hashlib.sha256('\n'.join(material).encode()).hexdigest()


class NormalizationCache:
    def __init__(self, path: Path, processor_digest: str, *, max_cache_bytes=DEFAULT_CACHE_BYTES,
                 max_cache_entries=DEFAULT_CACHE_ENTRIES, keep_runs=3):
        if keep_runs < 1 or max_cache_bytes < 1 or max_cache_entries < 1:
            raise ValueError('cache retention limits must be positive')
        self.keep_runs = keep_runs
        path.parent.mkdir(parents=True, exist_ok=True)
        self.lock = None
        if str(path) != ':memory:':
            import fcntl
            self.lock = path.with_name(path.name + '.lock').open('a')
            try:
                fcntl.flock(self.lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as error:
                self.lock.close()
                raise RuntimeError('normalization cache is owned by another builder') from error
        self.connection = None
        try:
            self.connection = sqlite3.connect(path)
            self.connection.execute('PRAGMA journal_mode=WAL')
            self.connection.execute('PRAGMA synchronous=NORMAL')
            self.connection.execute('CREATE TABLE IF NOT EXISTS completed_steps (cache_key TEXT PRIMARY KEY, payload TEXT NOT NULL)')
            self.processor_digest = processor_digest
            self.hits = self.misses = 0
            # Exclusive ownership means a previous running record belongs to an
            # interrupted process, not a live sibling. Its completed outputs survive.
            if self.connection.execute("SELECT 1 FROM sqlite_master WHERE name='processing_runs'").fetchone():
                self.connection.execute("UPDATE processing_runs SET status='interrupted' WHERE status='running'")
                self.connection.commit()
            self.scheduler = ProcessingScheduler(self.connection, max_cache_bytes=max_cache_bytes, max_cache_entries=max_cache_entries)
            self.node_tasks = {}
            self.processing_report = None
        except BaseException:
            if self.connection is not None:
                self.connection.close()
            if self.lock is not None:
                self.lock.close()
            raise

    def normalize(self, kind, identifier, inputs, dependencies, compute):
        def action(_):
            # Re-enter the existing pure normalizer without re-entering the
            # scheduler. This keeps one authored normalization implementation.
            token = active_cache.set(None)
            try:
                return compute()
            finally:
                active_cache.reset(token)
        task = Task(f'{kind}:{identifier}', self.processor_digest, tuple(dependencies), inputs, action)
        result, _ = self.scheduler.evaluate(task)
        if kind == 'node':
            self.node_tasks[result['id']] = task
        self.hits, self.misses = self.scheduler.reused, self.scheduler.executed
        return result

    def node_title(self, identifier, value):
        parent = self.node_tasks.get(identifier)
        if parent is None:
            return Input('endpoint-title:' + identifier, value)
        return Task('endpoint-title:' + identifier, self.processor_digest, (parent,), None,
                    lambda values: values[0].get('display', {}).get('title'))

    def memo(self, kind, identifier, dependencies, compute):
        """Cache one finalization/check without hiding its complete dependencies."""
        task = Task(f'{kind}:{identifier}', self.processor_digest, tuple(dependencies), None, lambda _: compute())
        result, _ = self.scheduler.evaluate(task)
        self.hits, self.misses = self.scheduler.reused, self.scheduler.executed
        return result

    def key(self, step: str, inputs) -> str:
        payload = json.dumps([self.processor_digest, step, inputs], ensure_ascii=False, sort_keys=True, separators=(',', ':'))
        return hashlib.sha256(payload.encode()).hexdigest()

    def get(self, key: str):
        found, value = self.scheduler.outputs.get(key)
        if not found:
            self.misses += 1
            return None
        self.hits += 1
        # Return a fresh object; graph assembly may attach view memberships.
        return value

    def put(self, key: str, payload):
        self.scheduler.outputs.put(key, payload)
        if self.misses % 100 == 0:
            self.connection.commit()

    def __enter__(self):
        self.token = active_cache.set(self)
        return self

    def __exit__(self, error_type, error, traceback):
        active_cache.reset(self.token)
        try:
            self.scheduler.finish(error)
            self.processing_report = self.scheduler.report()
        finally:
            try:
                retired = self.scheduler.prune_history(self.keep_runs)
                if self.processing_report is not None:
                    self.processing_report['retired_runs'] = retired
                self.connection.execute('PRAGMA wal_checkpoint(TRUNCATE)')
                pages = self.connection.execute('PRAGMA page_count').fetchone()[0]
                free = self.connection.execute('PRAGMA freelist_count').fetchone()[0]
                if free > max(256, pages // 4):
                    self.connection.execute('VACUUM')
                    self.connection.execute('PRAGMA wal_checkpoint(TRUNCATE)')
            finally:
                self.connection.close()
                if self.lock is not None:
                    self.lock.close()
