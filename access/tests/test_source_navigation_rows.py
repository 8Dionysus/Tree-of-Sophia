"""Parity tests for the shared source-navigation serving-row projection."""

from __future__ import annotations

import json
from pathlib import Path
import sys
import tempfile
import unittest


ACCESS_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = ACCESS_ROOT / "deploy/cloudflare-worker/scripts"
sys.path.insert(0, str(SCRIPTS_ROOT))

import build_runtime as builder  # noqa: E402
from source_navigation_rows import (  # noqa: E402
    COLUMNS,
    SQL_CHUNK_BYTES,
    project_rows,
    project_source_navigation_row,
)


class _Writer:
    def __init__(self) -> None:
        self.statements: list[str] = []

    def append(self, statement: str) -> None:
        self.statements.append(statement)


class SourceNavigationRowsTests(unittest.TestCase):
    def _emit(self, projected) -> list[str]:
        """Use the existing full-builder SQL boundary for exact statement parity."""
        writer = _Writer()
        table = projected.table + "_next"
        if projected.payload_rows:
            writer.append(builder.sql_insert(table, projected.columns, projected.sql_values))
            payload_table = projected.payload_table + "_next"
            payload_columns = COLUMNS[projected.payload_table]
            for item_id, part, chunk in projected.payload_rows:
                writer.append(
                    builder.sql_insert(
                        payload_table,
                        payload_columns,
                        (builder.sql_text(item_id), str(part), builder.sql_text(chunk)),
                    )
                )
        else:
            builder.append_chunkable_insert(
                writer,
                table,
                projected.columns,
                projected.sql_values,
                selector_sql=f"{projected.id_column} = {builder.sql_text(projected.item_id)}",
                chunked_text=dict(projected.chunked_text),
            )
        return writer.statements

    def test_nodes_edges_and_rights_match_the_previous_emitted_row_shapes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            node = {
                "node_id": "tos.node.é",
                "node_kind": "work",
                "source_ref": str(root / "ToS/source.json"),
                "label": "L'ink",
                "identity_status": "verified",
                "properties": {
                    "packet_id": "packet-1",
                    "access_status": "open_download",
                    "not_a_selection_field": "omitted",
                },
            }
            edge = {
                "edge_id": "edge-é",
                "from_id": "from",
                "to_id": "to",
                "edge_kind": "evidence_claim",
                "predicate_id": "supports",
                "review_status": "unreviewed",
                "source_refs": [str(root / "ToS/edge.json")],
            }
            rights = {
                "rights_id": "rights-é",
                "scope_refs": ["tos.item.fixture"],
                "assessment_status": "licensed",
            }
            cases = (("nodes", 2, node), ("edges", 4, edge), ("rights", 6, rights))
            for kind, ordinal, item in cases:
                with self.subTest(kind=kind):
                    projected = project_source_navigation_row(kind, ordinal, item, root)
                    rows = project_rows(kind, ordinal, item, repo_root=root)
                    self.assertEqual(rows, {projected.table: [projected.values]})
                    self.assertIs(type(projected.values[1]), int)
                    self.assertEqual(projected.values[1], ordinal)
                    self.assertFalse(any(
                        isinstance(value, str) and value.startswith("'")
                        for value in projected.values
                    ))
                    self.assertEqual(projected.payload_rows, ())
                    self.assertEqual(
                        self._emit(projected),
                        [builder.sql_insert(
                            projected.table + "_next",
                            COLUMNS[projected.table],
                            projected.sql_values,
                        )],
                    )

            node_rows = project_rows("nodes", 2, node, root)
            node_values = node_rows["source_navigation_nodes"][0]
            self.assertEqual(node_values[:6], (
                "tos.node.é",
                2,
                "work",
                "ToS/source.json",
                "L'ink",
                "verified",
            ))
            self.assertEqual(
                json.loads(node_values[6]),
                {"packet_id": "packet-1", "access_status": "open_download"},
            )
            self.assertEqual(json.loads(node_values[7]), {
                **node,
                "source_ref": "ToS/source.json",
            })

    def test_large_unicode_selection_uses_lossless_bounded_payload_rows(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            access_status = "🌳" * 600_000
            node = {
                "node_id": "tos.node.large",
                "node_kind": "text-unit",
                "source_ref": str(root / "ToS/large.json"),
                "label": "Unicode boundary",
                "identity_status": "provisional",
                "properties": {"access_status": access_status},
            }
            projected = project_source_navigation_row("nodes", 0, node, root)
            expected_json = builder.compact_json(
                builder.normalize_paths(node, root)
            )
            self.assertEqual(projected.values[0], "tos.node.large")
            self.assertEqual(projected.values[1], 0)
            self.assertEqual(projected.values[6], "{}")
            self.assertEqual(projected.values[7], "")
            self.assertGreater(len(projected.payload_rows), 1)
            self.assertEqual(
                "".join(chunk for _item_id, _part, chunk in projected.payload_rows),
                expected_json,
            )
            self.assertEqual(
                [part for _item_id, part, _chunk in projected.payload_rows],
                list(range(len(projected.payload_rows))),
            )
            self.assertTrue(any("🌳" in chunk for _item_id, _part, chunk in projected.payload_rows))
            self.assertTrue(all(
                len(chunk.encode("utf-8")) <= SQL_CHUNK_BYTES
                for _item_id, _part, chunk in projected.payload_rows
            ))
            rows = project_rows("nodes", 0, node, repo_root=root)
            self.assertEqual(rows[projected.table][0], projected.values)
            self.assertEqual(
                rows[projected.payload_table],
                list(projected.payload_rows),
            )
            statements = self._emit(projected)
            self.assertEqual(len(statements), 1 + len(projected.payload_rows))
            self.assertEqual(
                statements[0],
                builder.sql_insert(
                    "source_navigation_nodes_next",
                    COLUMNS["source_navigation_nodes"],
                    projected.sql_values,
                ),
            )
            self.assertIn(
                "🌳",
                statements[-1],
            )


if __name__ == "__main__":
    unittest.main()
