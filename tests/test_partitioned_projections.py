"""Source-owned semantic parity across the bounded projection carrier."""
from __future__ import annotations

import copy
import json
import gzip
from pathlib import Path
import sys
import tempfile
import unittest
from jsonschema import Draft202012Validator
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from partitioned_projection_common import (
    ProjectionReader, build_storage, check_partitioned_payload, disk_payload,
    json_chunks, write_partitioned_payload, schema_validator, DiskSequence,
)
import source_witness_bibliographic_graph_common as graph
import tos_corpus_index_common as corpus
from tests import test_source_witness_bibliographic_graph as graph_tests


class PartitionedSourceProjectionTests(unittest.TestCase):
    def fixture(self):
        return graph_tests.SourceWitnessBibliographicGraphTest().historical_fixture()

    def test_corpus_collection_order_does_not_depend_on_build_storage(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for branch in ('branch', 'branch-extra'):
                folder = root / 'ToS/canon' / branch
                folder.mkdir(parents=True)
                (folder / 'node.json').write_text(json.dumps({'node_id': branch, 'node_type': 'concept'}))
                (folder / 'tree.manifest.json').write_text('{}')
                (folder / 'edges.csv').write_text('edge_id,from_id,predicate_id,to_id\ne,from,related_to,to\n')
            paths = tuple(sorted(path for path in (root / 'ToS').rglob('*') if path.is_file()))
            with patch.object(corpus, 'REPO_ROOT', root), patch.object(corpus, 'TOS_ROOT', root / 'ToS'), build_storage() as storage:
                for build in (corpus.build_manifests, corpus.build_nodes, corpus.build_relations):
                    with self.subTest(builder=build.__name__):
                        self.assertEqual(''.join(json_chunks(build([], paths))),
                                         ''.join(json_chunks(build([], paths, storage))))
                self.assertEqual(''.join(json_chunks(corpus.build_resources(paths))),
                                 ''.join(json_chunks(corpus.build_resources(paths, storage))))

    def test_source_graph_disk_build_is_exact_and_query_preserves_trace(self):
        with self.fixture() as (root, _history, _real, claims, rebuild):
            expected = rebuild()
            with build_storage() as storage:
                actual = graph.build_payload(root, storage=storage)
                self.assertEqual(''.join(json_chunks(actual)) + '\n', graph.render_payload(expected))
                path = root / graph.GRAPH_REF
                write_partitioned_payload(path, actual)
                Draft202012Validator(json.loads((ROOT / "ToS/contracts/partitioned-projection.schema.json").read_text())).validate(json.loads(path.read_text()))
                reader = check_partitioned_payload(path, actual)
                self.assertEqual(reader.materialize(), expected)
                disk = disk_payload(reader, storage)
                graph.validate_payload_schema(disk, root)
                self.assertEqual(graph.query_projection(disk, repo_root=root, claim_ref=claims[0]['claim_id'], storage=storage),
                                 graph.query_projection(expected, repo_root=root, claim_ref=claims[0]['claim_id']))

    def test_source_parity_accepts_equivalent_gzip_encoding(self):
        import tos_access.projection_store as transport
        with self.fixture() as (root, _history, _real, _claims, rebuild):
            expected = rebuild()
            path = root / graph.GRAPH_REF
            write_partitioned_payload(path, expected)
            original = path.read_bytes()
            with patch.object(transport, "_gzip", side_effect=lambda raw: gzip.compress(raw, compresslevel=1, mtime=0)):
                write_partitioned_payload(path, expected)
            self.assertNotEqual(path.read_bytes(), original)
            check_partitioned_payload(path, expected)

    def test_streaming_schema_keeps_item_assertions_without_index_scans(self):
        rows = [{"id": str(index)} for index in range(100)]
        rows[37] = {"id": 37}
        schema = {"type": "array", "prefixItems": [{"type": "object"}],
                  "items": {"type": "object", "required": ["id"],
                            "properties": {"id": {"type": "string"}}}}
        errors = lambda validator, value: [(list(error.path), error.validator)
                                         for error in validator.iter_errors(value)]
        with build_storage() as storage:
            staged = storage.sequence(rows)
            with patch.object(DiskSequence, "__getitem__", side_effect=AssertionError("indexed scan")):
                self.assertEqual(errors(schema_validator(schema), staged),
                                 errors(Draft202012Validator(schema), rows))
                schema["items"] = False
                self.assertEqual(errors(schema_validator(schema), staged),
                                 errors(Draft202012Validator(schema), rows))

    def test_staged_source_claims_reject_source_change_before_completion(self):
        with self.fixture() as (root, _history, _real, _claims, rebuild):
            expected = rebuild()
            trace = expected["claim_traces"][0]
            ref, line = trace["source_claim_file_ref"], trace["source_claim_line"]
            with build_storage() as storage:
                rows = graph._SourceClaimRows(root, storage)
                read = lambda: graph.iter_jsonl(root / ref, root)
                self.assertIsNotNone(rows.get(ref, line, read))
                with (root / ref).open("a") as stream:
                    stream.write("\n")
                with self.assertRaisesRegex(graph.BibliographicGraphBuildError, "changed during build"):
                    rows.verify_current()

    def test_navigation_disk_build_preserves_source_records_and_boundaries(self):
        with self.fixture() as (root, _history, _real, _claims, rebuild):
            rebuild()
            with patch.object(corpus, 'REPO_ROOT', root), patch.object(corpus, 'TOS_ROOT', root / 'ToS'):
                expected_diagnostics = []
                expected = corpus.build_source_navigation(expected_diagnostics)
                with build_storage() as storage:
                    diagnostics = []
                    actual = corpus.build_source_navigation(diagnostics, storage=storage)
                    self.assertEqual(json.loads(''.join(json_chunks(actual))), expected)
                    self.assertEqual(diagnostics, expected_diagnostics)

    def test_disk_cross_reference_checks_reject_omitted_or_misattributed_edges(self):
        with self.fixture() as (_root, _history, _real, _claims, rebuild):
            expected = rebuild()
            changed = copy.deepcopy(expected)
            changed['claim_traces'][0]['edge_ids'].pop()
            with build_storage() as storage:
                for payload in (expected, changed):
                    payload['nodes'] = storage.sequence(payload['nodes'])
                    payload['edges'] = storage.sequence(payload['edges'])
                    payload['claim_traces'] = storage.sequence(payload['claim_traces'])
                graph._validate_cross_references(expected, storage)
                with self.assertRaisesRegex(graph.BibliographicGraphBuildError, 'trace edge set differs'):
                    graph._validate_cross_references(changed, storage)


if __name__ == '__main__':
    unittest.main()
