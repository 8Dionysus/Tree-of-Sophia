"""Bounded reverse navigation of one retained, completed processing DAG.

This is execution metadata, not source-change completeness or publication
authority. The caller holds a SQLite read transaction and names the exact run;
this module never evaluates tasks, publishes a run, or writes cache metadata.
"""
from __future__ import annotations

from collections import deque
import json
import re


class ProcessingClosureError(ValueError):
    """Missing, incompatible or inconsistent retained dependency evidence."""


class ProcessingClosureBudgetExceeded(ProcessingClosureError):
    """A complete closure cannot be returned within the declared bounds."""


_DIGEST = re.compile(r"[0-9a-f]{64}\Z")


def processing_dependency_closure(
    db, run_id, changed_ids, *, max_nodes=4096, max_edges=16384,
    max_identifier_bytes=4096, max_result_bytes=1_048_576,
):
    """Return the complete retained reverse closure or raise, never a prefix.

    Seeds must already exist in this run. New source identities and incomplete
    normalizer dependency mappings require their owner, not a guessed empty
    closure. Unvisited tasks remain outside this result; none are removed.

    Run retention/current publication may change on another connection: the
    caller's read transaction pins the requested evidence until this returns.
    No current-source or current-publication claim is made by this historical
    query. ``processing_dependencies_reverse`` is installed by the scheduler's
    explicit cache initialization, never by this read route.
    """
    if not db.in_transaction:
        raise ProcessingClosureError("caller-owned read transaction required")
    for value in (max_nodes, max_edges, max_identifier_bytes, max_result_bytes):
        if type(value) is not int or value < 1:
            raise ValueError("closure budgets must be positive integers")

    def identifier(value):
        if not isinstance(value, str) or not value:
            raise ProcessingClosureError("nonempty processing identity required")
        try:
            size = len(value.encode("utf-8"))
        except UnicodeError as error:
            raise ProcessingClosureError("invalid processing identity encoding") from error
        if size > max_identifier_bytes:
            raise ProcessingClosureBudgetExceeded("processing identity byte budget")
        return value

    identifier(run_id)
    state = db.execute("SELECT status='complete' FROM processing_runs WHERE id=?", (run_id,)).fetchone()
    if state is None or not state[0]:
        raise ProcessingClosureError("exact completed processing run required; it may have been retired")
    if not db.execute("SELECT 1 FROM sqlite_master WHERE type='index' AND name='processing_dependencies_reverse'").fetchone():
        raise ProcessingClosureError("reverse dependency index missing; explicit cache initialization required")
    if isinstance(changed_ids, (str, bytes)):
        raise ProcessingClosureError("changed identities must be an iterable of identities")
    seeds = set()
    seed_bytes = 0
    for value in changed_ids:
        value = identifier(value)
        if value in seeds:
            # Also bound redundant caller input rather than looping forever.
            raise ProcessingClosureError("duplicate changed processing identity")
        if len(seeds) >= max_nodes:
            raise ProcessingClosureBudgetExceeded("processing closure node budget")
        seed_bytes += len(json.dumps(value, ensure_ascii=False).encode("utf-8")) + 1
        if seed_bytes > max_result_bytes:
            raise ProcessingClosureBudgetExceeded("processing closure result byte budget")
        seeds.add(value)
    if not seeds:
        raise ProcessingClosureError("at least one changed processing identity required")

    packet = {
        "schema": "tos_processing_dependency_closure_v1", "run_id": run_id,
        "changed_ids": sorted(seeds), "nodes": [], "edges": [],
        "coverage": "complete-retained-dag", "is_semantic_acceptance": False,
        "is_source_change_completeness": False,
    }
    encode = lambda value: json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    used = len(encode(packet))

    def append(field, value):
        nonlocal used
        addition = len(encode(value)) + int(bool(packet[field]))
        if used + addition > max_result_bytes:
            raise ProcessingClosureBudgetExceeded("processing closure result byte budget")
        packet[field].append(value)
        used += addition

    if used > max_result_bytes:
        raise ProcessingClosureBudgetExceeded("processing closure result byte budget")
    pending = deque(sorted(seeds))
    seen = set(seeds)
    children = {}
    indegrees = {}
    while pending:
        current = pending.popleft()
        # Bounded projections prevent malformed cache fields from returning
        # arbitrarily large strings before Python can reject them.
        row = db.execute(
            "SELECT CASE WHEN kind IN ('input','task') THEN kind END,"
            "CASE WHEN length(CAST(output_digest AS BLOB))=64 THEN output_digest END,"
            "status='complete' FROM processing_tasks WHERE run_id=? AND id=?",
            (run_id, current),
        ).fetchone()
        if row is None or row[0] is None or not row[2] or not isinstance(row[1], str) or not _DIGEST.fullmatch(row[1]):
            raise ProcessingClosureError("incomplete or malformed retained processing task")
        if row[0] == "input" and indegrees.get(current, 0):
            raise ProcessingClosureError("retained input cannot depend on a task")
        append("nodes", {"id": current, "kind": row[0], "output_digest": row[1]})
        remaining = max_edges - len(packet["edges"])
        rows = db.execute(
            "SELECT CASE WHEN length(CAST(task_id AS BLOB))<=? THEN task_id END "
            "FROM processing_dependencies INDEXED BY processing_dependencies_reverse "
            "WHERE run_id=? AND dependency_id=? ORDER BY task_id LIMIT ?",
            (max_identifier_bytes, run_id, current, remaining + 1),
        )
        for (consumer,) in rows:
            if len(packet["edges"]) >= max_edges:
                raise ProcessingClosureBudgetExceeded("processing closure edge budget")
            if consumer is None:
                raise ProcessingClosureBudgetExceeded("processing identity byte budget")
            consumer = identifier(consumer)
            if consumer not in seen:
                if len(seen) >= max_nodes:
                    raise ProcessingClosureBudgetExceeded("processing closure node budget")
                seen.add(consumer)
                pending.append(consumer)
            append("edges", {"dependency_id": current, "task_id": consumer})
            children.setdefault(current, []).append(consumer)
            indegrees[consumer] = indegrees.get(consumer, 0) + 1

    # A normal completed scheduler cannot emit a cycle. Fail explicitly if
    # retained execution metadata was damaged rather than hiding it via seen.
    ready = deque(key for key in seen if not indegrees.get(key))
    visited = 0
    while ready:
        current = ready.popleft()
        visited += 1
        for consumer in children.get(current, ()):
            indegrees[consumer] -= 1
            if not indegrees[consumer]:
                ready.append(consumer)
    if visited != len(seen):
        raise ProcessingClosureError("cyclic retained processing dependencies")
    # Input seeds may have been processed before an incoming edge was found.
    consumers = {edge["task_id"] for edge in packet["edges"]}
    if any(node["kind"] == "input" and node["id"] in consumers for node in packet["nodes"]):
        raise ProcessingClosureError("retained input cannot depend on a task")
    return packet
