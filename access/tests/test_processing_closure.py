"""Retained dependency closure is complete, bounded, and read-only."""
from __future__ import annotations

from contextlib import closing
import json
import sqlite3
import tempfile
import unittest
from pathlib import Path

from tos_access.processing import Input, Task, ProcessingScheduler
from tos_access.processing_closure import (
    processing_dependency_closure, ProcessingClosureError,
    ProcessingClosureBudgetExceeded,
)


class ProcessingClosureTests(unittest.TestCase):
    def baseline(self, db):
        source = Input("source:a", {"label": "Alpha"})
        unrelated = Input("source:b", {"label": "Beta"})
        first = Task("node:a", "v1", (source,), None, lambda values: values[0])
        title = Task("title:a", "v1", (first,), None, lambda values: values[0]["label"])
        relation = Task("relation:a-b", "v1", (title, unrelated), None, lambda values: values[0])
        tail = Task("final:a", "v1", (first, relation), None, lambda values: values)
        run = ProcessingScheduler(db)
        run.evaluate(tail)
        run.finish()
        return run

    def test_shared_descendants_are_unique_and_unvisited_inputs_are_not_deleted(self):
        with closing(sqlite3.connect(":memory:")) as db:
            run = self.baseline(db)
            db.execute("BEGIN")
            statements = []
            db.set_trace_callback(statements.append)
            result = processing_dependency_closure(db, run.run_id, ["source:a", "node:a"])
            self.assertEqual(result["coverage"], "complete-retained-dag")
            self.assertEqual({node["id"] for node in result["nodes"]},
                             {"source:a", "node:a", "title:a", "relation:a-b", "final:a"})
            self.assertEqual(len(result["edges"]), 5)
            self.assertEqual(len({node["id"] for node in result["nodes"]}), len(result["nodes"]))
            self.assertFalse(result["is_source_change_completeness"])
            self.assertFalse(result["is_semantic_acceptance"])
            self.assertTrue(db.in_transaction)
            self.assertTrue(all(sql.lstrip().startswith("SELECT") for sql in statements))
            self.assertNotIn("removed", result)

    def test_exact_result_byte_bound_and_node_edge_bounds_fail_without_partial_packet(self):
        with closing(sqlite3.connect(":memory:")) as db:
            run = self.baseline(db)
            db.execute("BEGIN")
            expected = processing_dependency_closure(db, run.run_id, ["source:a"])
            size = len(json.dumps(expected, ensure_ascii=False, separators=(",", ":")).encode())
            self.assertEqual(processing_dependency_closure(db, run.run_id, ["source:a"], max_result_bytes=size), expected)
            for limits in ({"max_result_bytes": size - 1}, {"max_nodes": 4}, {"max_edges": 4},
                           {"max_identifier_bytes": 3}):
                with self.subTest(limits=limits), self.assertRaises(ProcessingClosureBudgetExceeded):
                    processing_dependency_closure(db, run.run_id, ["source:a"], **limits)
            self.assertTrue(db.in_transaction)

    def test_unknown_retired_unfinished_and_missing_index_are_explicit_refusals(self):
        with closing(sqlite3.connect(":memory:")) as db:
            run = self.baseline(db)
            with self.assertRaisesRegex(ProcessingClosureError, "transaction"):
                processing_dependency_closure(db, run.run_id, ["source:a"])
            db.execute("BEGIN")
            for seeds in ([], ["missing"], ["source:a", "source:a"], "source:a"):
                with self.subTest(seeds=seeds), self.assertRaises(ProcessingClosureError):
                    processing_dependency_closure(db, run.run_id, seeds)
            for value in (0, True, 1.5):
                with self.assertRaises(ValueError):
                    processing_dependency_closure(db, run.run_id, ["source:a"], max_nodes=value)
            db.rollback()
            unfinished = ProcessingScheduler(db)
            unfinished.evaluate(Input("unpublished", 1))
            for identifier in (unfinished.run_id, "retired"):
                with self.assertRaisesRegex(ProcessingClosureError, "completed"):
                    processing_dependency_closure(db, identifier, ["source:a"])
            db.execute("DROP INDEX processing_dependencies_reverse")
            with self.assertRaisesRegex(ProcessingClosureError, "index missing"):
                processing_dependency_closure(db, run.run_id, ["source:a"])

    def test_corrupt_dependency_metadata_does_not_hide_cycles_missing_tasks_or_bad_inputs(self):
        for defect in ("cycle", "missing", "input-consumer", "failed", "digest", "wide"):
            with self.subTest(defect=defect), closing(sqlite3.connect(":memory:")) as db:
                run = self.baseline(db)
                if defect in {"cycle", "missing", "input-consumer", "wide"}:
                    consumer = {"cycle": "source:a", "missing": "absent", "input-consumer": "source:b",
                                "wide": "z" * 5000}[defect]
                    db.execute("INSERT INTO processing_dependencies VALUES (?,?,?)", (run.run_id, consumer, "final:a"))
                elif defect == "failed":
                    db.execute("UPDATE processing_tasks SET status='failed' WHERE id='final:a'")
                else:
                    db.execute("UPDATE processing_tasks SET output_digest='invalid' WHERE id='final:a'")
                with self.assertRaises(ProcessingClosureError):
                    processing_dependency_closure(db, run.run_id, ["source:a"])

    def test_reverse_index_keeps_small_closure_work_independent_of_unrelated_edges(self):
        with closing(sqlite3.connect(":memory:")) as db:
            run = self.baseline(db)
            def query_work():
                steps = [0]
                def tick():
                    steps[0] += 1
                    return 0
                db.set_progress_handler(tick, 1)
                try:
                    packet = processing_dependency_closure(db, run.run_id, ["source:a"])
                finally:
                    db.set_progress_handler(None, 0)
                return packet, steps[0]
            db.execute("BEGIN")
            before, small = query_work()
            db.executemany("INSERT INTO processing_dependencies VALUES (?,?,?)",
                           ((run.run_id, f"unrelated-task:{i}", f"unrelated-input:{i}") for i in range(10000)))
            after, grown = query_work()
            self.assertEqual(after, before)
            self.assertLess(grown, small + 100)
            plan = db.execute("EXPLAIN QUERY PLAN SELECT task_id FROM processing_dependencies "
                              "INDEXED BY processing_dependencies_reverse "
                              "WHERE run_id=? AND dependency_id=? ORDER BY task_id LIMIT 10",
                              (run.run_id, "source:a")).fetchall()
            self.assertTrue(any("SEARCH" in row[-1] and "processing_dependencies_reverse" in row[-1] for row in plan))

    def test_caller_read_snapshot_survives_concurrent_retirement_without_selecting_new_run(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "processing.sqlite"
            with closing(sqlite3.connect(path)) as writer:
                writer.execute("PRAGMA journal_mode=WAL")
                run = self.baseline(writer)
                with closing(sqlite3.connect(path)) as reader:
                    reader.execute("BEGIN")
                    expected = processing_dependency_closure(reader, run.run_id, ["source:a"])
                    writer.execute("DELETE FROM processing_tasks WHERE run_id=?", (run.run_id,))
                    writer.execute("DELETE FROM processing_dependencies WHERE run_id=?", (run.run_id,))
                    writer.execute("DELETE FROM processing_runs WHERE id=?", (run.run_id,))
                    writer.commit()
                    self.assertEqual(processing_dependency_closure(reader, run.run_id, ["source:a"]), expected)
                    reader.rollback()
                    reader.execute("BEGIN")
                    with self.assertRaisesRegex(ProcessingClosureError, "retired"):
                        processing_dependency_closure(reader, run.run_id, ["source:a"])


if __name__ == "__main__":
    unittest.main()
