"""Owner-selected disposable SQLite checkpoints, separate from source data.

BEGIN IMMEDIATE serializes input-state comparison, source-checked execution,
replay and successor publication across processes. Any failure rolls back the
whole transaction. DELETE journal avoids a growing WAL; max_page_count caps
the database, while one rollback journal may transiently add roughly that cap
plus SQLite headers. Free pages are reused, not an unbounded history log.
"""
from __future__ import annotations

import math
import os
import sqlite3
import stat
from contextlib import contextmanager, closing
from pathlib import Path

from .exploration import ExplorationExpired
from .published_read_metadata import _compact

SCHEMA = "tos_published_exploration_checkpoints_v1"


class PublishedCheckpointError(RuntimeError):
    """Unavailable/incompatible disposable state; never repaired implicitly."""


class PublishedCheckpointClockRollback(PublishedCheckpointError):
    """Wall clock precedes the last committed access; owner must correct it."""


class _Transaction:
    def __init__(self, store, db):
        self.store, self.db = store, db
        self.protected = set()

    def get(self, token):
        row = self.db.execute("SELECT expires,raw FROM checkpoints WHERE token=?", (token,)).fetchone()
        if row is None:
            raise ExplorationExpired("exploration expired or was evicted; restart from focus")
        if (not isinstance(row[1], bytes) or len(row[1]) > self.store.max_bytes
                or not isinstance(row[0], (int, float)) or not math.isfinite(row[0])):
            raise PublishedCheckpointError("checkpoint payload is invalid or over budget")
        return row

    def put(self, token, expires, record):
        raw = _compact(record).encode("utf-8")
        if len(raw) > self.store.max_bytes:
            raise ExplorationExpired("checkpoint exceeds cache capacity; narrow exploration")
        self.db.execute("DELETE FROM checkpoints WHERE token=?", (token,))
        count, total = self.db.execute("SELECT count(*),coalesce(sum(length(raw)),0) FROM checkpoints").fetchone()
        while count and (count >= self.store.max_checkpoints or total + len(raw) > self.store.max_bytes):
            oldest = self.db.execute("SELECT token,length(raw) FROM checkpoints WHERE token NOT IN (SELECT value FROM json_each(?)) ORDER BY sequence LIMIT 1", (_compact(sorted(self.protected)),)).fetchone()
            if oldest is None:
                raise ExplorationExpired("replay and successor together exceed checkpoint capacity; narrow exploration")
            old, size = oldest
            self.db.execute("DELETE FROM checkpoints WHERE token=?", (old,))
            count, total = count - 1, total - size
        sequence = self.db.execute("SELECT coalesce(max(sequence),0)+1 FROM checkpoints").fetchone()[0]
        self.db.execute("INSERT INTO checkpoints VALUES (?,?,?,?)", (token, expires, sequence, raw))
        self.protected.add(token)


class PublishedCheckpointStore:
    def __init__(self, path, source_path, *, max_checkpoints, max_bytes, execution_config):
        self.path = Path(path).absolute()
        source_path = Path(source_path).absolute()
        self.source_path = source_path
        if self.path.resolve() != self.path or self.path in [source_path, *[Path(str(source_path) + suffix) for suffix in ("-wal", "-shm", "-journal")]]:
            raise PublishedCheckpointError("checkpoint path must be a distinct non-symlink owner-selected file")
        self.max_checkpoints, self.max_bytes = max_checkpoints, max_bytes
        self.max_pages = (max_bytes * 3 + 1_048_576 + 4095) // 4096
        self.config = _compact({"schema": SCHEMA, "max_checkpoints": max_checkpoints,
                                "max_bytes": max_bytes, "execution": execution_config})
        created = False
        try:
            fd = os.open(self.path, os.O_CREAT | os.O_EXCL | os.O_RDWR | os.O_NOFOLLOW, 0o600)
            os.close(fd)
            created = True
        except FileExistsError:
            pass
        except OSError as error:
            raise PublishedCheckpointError("checkpoint path cannot be created") from error
        self._identity = self._file_identity()
        try:
            with closing(self._connect()) as db:
                db.execute("BEGIN IMMEDIATE")
                if created:
                    db.execute("CREATE TABLE checkpoint_meta (singleton INTEGER PRIMARY KEY CHECK(singleton=1), config TEXT NOT NULL, last_time REAL NOT NULL)")
                    db.execute("CREATE TABLE checkpoints (token TEXT PRIMARY KEY, expires REAL NOT NULL, sequence INTEGER NOT NULL, raw BLOB NOT NULL)")
                    db.execute("CREATE INDEX checkpoints_sequence ON checkpoints(sequence)")
                    db.execute("INSERT INTO checkpoint_meta VALUES (1,?,0)", (self.config,))
                self._validate(db)
                db.commit()
        except sqlite3.Error as error:
            raise PublishedCheckpointError("checkpoint store is unavailable or incompatible; no automatic reset") from error

    def _file_identity(self):
        try:
            info = self.path.lstat()
            if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
                    or stat.S_IMODE(info.st_mode) != 0o600
                    or self.path.resolve() != self.path
                    or os.path.samefile(self.path, self.source_path)):
                raise PublishedCheckpointError("checkpoint must be a private 0600 regular file distinct from source")
            if info.st_size > self.max_pages * 4096:
                raise PublishedCheckpointError("checkpoint database exceeds its physical page cap")
            return info.st_dev, info.st_ino
        except OSError as error:
            raise PublishedCheckpointError("checkpoint file is unavailable") from error

    def _connect(self):
        if self._file_identity() != self._identity:
            raise PublishedCheckpointError("checkpoint store was replaced; select it explicitly again")
        db = sqlite3.connect(self.path.as_uri() + "?mode=rw", uri=True, isolation_level=None, timeout=0.1)
        try:
            if self._file_identity() != self._identity:
                raise PublishedCheckpointError("checkpoint store changed while opening")
            db.execute("PRAGMA trusted_schema=OFF")
            db.execute("PRAGMA synchronous=FULL")
            if db.execute("PRAGMA journal_mode").fetchone()[0].lower() != "delete":
                raise PublishedCheckpointError("checkpoint store requires bounded DELETE-journal mode")
            if db.execute("PRAGMA page_size").fetchone()[0] != 4096:
                raise PublishedCheckpointError("checkpoint store has incompatible page size")
            db.execute(f"PRAGMA max_page_count={self.max_pages}")
            db.setlimit(sqlite3.SQLITE_LIMIT_LENGTH, self.max_bytes + 65536)
            return db
        except BaseException:
            db.close()
            raise

    def _validate(self, db):
        rows = db.execute("SELECT config,last_time FROM checkpoint_meta WHERE singleton=1 LIMIT 2").fetchall()
        if (len(rows) != 1 or rows[0][0] != self.config
                or not isinstance(rows[0][1], (int, float)) or not math.isfinite(rows[0][1])):
            raise PublishedCheckpointError("checkpoint store schema/execution configuration is incompatible")
        count, total = db.execute("SELECT count(*),coalesce(sum(length(raw)),0) FROM checkpoints").fetchone()
        if count > self.max_checkpoints or total > self.max_bytes:
            raise PublishedCheckpointError("checkpoint store exceeds its logical capacity")
        return rows[0][1]

    @contextmanager
    def transaction(self, clock):
        try:
            with closing(self._connect()) as db:
                db.execute("BEGIN IMMEDIATE")
                try:
                    # Read time after the cross-process lock; a request that
                    # waited must not look like a clock rollback by arrival order.
                    now = clock()
                    if not isinstance(now, (int, float)) or not math.isfinite(now) or now < 0:
                        raise PublishedCheckpointClockRollback("persistent checkpoint clock must be finite UTC epoch seconds")
                    if now < self._validate(db):
                        raise PublishedCheckpointClockRollback("wall clock moved backwards; no checkpoint was changed")
                    db.execute("DELETE FROM checkpoints WHERE expires<=?", (now,))
                    transaction = _Transaction(self, db)
                    transaction.now = now
                    yield transaction
                    if self._file_identity() != self._identity:
                        raise PublishedCheckpointError("checkpoint store changed during query")
                    db.execute("UPDATE checkpoint_meta SET last_time=? WHERE singleton=1", (now,))
                    db.commit()
                except BaseException:
                    db.rollback()
                    raise
        except sqlite3.Error as error:
            raise PublishedCheckpointError("checkpoint transaction unavailable, busy, corrupt or over physical budget; previous state retained") from error
