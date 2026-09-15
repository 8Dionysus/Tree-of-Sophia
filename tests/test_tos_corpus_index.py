from __future__ import annotations

import copy
import csv
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from tos_corpus_index_common import (  # noqa: E402
    TOS_CORPUS_INDEX_PATH,
    _nearest_branch_parents,
    build_payload,
    render_payload,
    tracked_tos_paths,
    project_text_packet,
)


class ToSCorpusIndexTest(unittest.TestCase):
    def test_exact_csv_row_preserves_multiline_unicode_nulls_and_byte_offsets(self):
        import tos_corpus_index_common as corpus
        raw = ('edge_id,note,extra\r\n\r\n'
               'one,"first\r\nλόγος\u2028last",\r\n'
               '\r\ntwo,second\r\n').encode('utf-8')
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'edges.csv'
            path.write_bytes(raw)
            columns, records, digest = corpus.read_edge_rows(path)
            for ordinal, record in enumerate(records, 1):
                result = corpus.read_exact_edge_row(path, source_file_sha256=digest,
                    source_row=ordinal, source_record=record)
                self.assertEqual(result['columns'], columns)
                self.assertEqual(result['record'], record)
                start = result['byte_offset']
                row_raw = raw[start:start + result['row_bytes']]
                self.assertEqual(result['raw_record'].encode('utf-8'), row_raw)
                self.assertTrue(row_raw.startswith(record['edge_id'].encode()))
                self.assertEqual(result['raw_record_sha256'], hashlib.sha256(row_raw).hexdigest())
            self.assertEqual(records[0]['note'], 'first\r\nλόγος\u2028last')
            self.assertEqual(records[0]['extra'], '')
            self.assertIsNone(records[1]['extra'])
            self.assertEqual(path.read_bytes(), raw)

    def test_exact_csv_row_refuses_stale_binding_wrong_row_and_budgets(self):
        import tos_corpus_index_common as corpus
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'edges.csv'
            raw = b'edge_id,note\none,original\ntwo,second\n'
            path.write_bytes(raw)
            _, rows, digest = corpus.read_edge_rows(path)
            options = dict(source_file_sha256=digest, source_row=1, source_record=rows[0])
            for change in ({'source_file_sha256': '0'*64}, {'source_row': 2}, {'source_row': 3},
                           {'source_row': True}, {'source_record': {**rows[0], 'note': 'invented'}},
                           {'source_record': {'edge_id': 'one', 'note': 1}},
                           {'max_file_bytes': len(raw)-1}, {'max_record_bytes': 1}, {'max_file_bytes': False}):
                with self.subTest(change=change), self.assertRaises(ValueError):
                    corpus.read_exact_edge_row(path, **{**options, **change})
            path.write_bytes(raw.replace(b'original', b'changed!'))
            with self.assertRaisesRegex(ValueError, 'digest differs'):
                corpus.read_exact_edge_row(path, **options)
            linked = path.with_name('linked.csv')
            linked.symlink_to(path)
            with self.assertRaises(OSError):
                corpus.read_exact_edge_row(linked, **options)

    def test_exact_csv_row_refuses_source_replacement_during_parsing(self):
        import tos_corpus_index_common as corpus
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'edges.csv'
            raw = b'edge_id,note\none,original\n'
            path.write_bytes(raw)
            _, rows, digest = corpus.read_edge_rows(path)
            reader = corpus.csv.reader
            def replaced(*args, **kwargs):
                replacement = path.with_name('replacement.csv')
                replacement.write_bytes(raw)
                replacement.replace(path)
                return reader(*args, **kwargs)
            with patch.object(corpus.csv, 'reader', side_effect=replaced):
                with self.assertRaisesRegex(ValueError, 'changed during exact read'):
                    corpus.read_exact_edge_row(path, source_file_sha256=digest,
                                              source_row=1, source_record=rows[0])

    def test_relation_csv_projection_preserves_every_cell_and_source_binding(self):
        import tos_corpus_index_common as corpus
        from jsonschema import Draft202012Validator
        access_src = str(REPO_ROOT / 'access/src')
        if access_src not in sys.path:
            sys.path.insert(0, access_src)
        from tos_access.knowledge import _normalize_relation
        schema = json.loads((REPO_ROOT / corpus.SCHEMA_REF).read_text())['$defs']['relationEdge']
        validator = Draft202012Validator(schema)
        raw = ('edge_id,from_id,predicate_id,to_id,confidence,note,x-unknown\r\n'
               'edge.one,left,related_to,right,0.70,"first\r\nsecond",λόγος\r\n'
               'edge.two,left,related_to,right,0,,\r\n'
               'edge.three,left,related_to,right,0.20\r\n').encode('utf-8')
        digest = hashlib.sha256(raw).hexdigest()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for branch in ('canon', 'candidate-intake'):
                path = root / f'ToS/{branch}/synthetic/edges.csv'
                path.parent.mkdir(parents=True)
                path.write_bytes(raw)
                with patch.object(corpus, 'REPO_ROOT', root):
                    errors = []
                    packs, edges = corpus.build_relations(errors, (path,))
                self.assertEqual(errors, [])
                self.assertEqual(packs[0]['sha256'], digest)
                self.assertEqual([edge['properties']['source_row'] for edge in edges], [1, 2, 3])
                first = edges[0]['properties']['source_record']
                self.assertEqual(first['confidence'], '0.70')
                self.assertEqual(first['note'], 'first\r\nsecond')
                self.assertEqual(first['x-unknown'], 'λόγος')
                self.assertEqual(edges[1]['properties']['source_record']['note'], '')
                self.assertIsNone(edges[2]['properties']['source_record']['note'])
                for edge in edges:
                    with self.subTest(branch=branch, edge=edge['edge_id']):
                        validator.validate(edge)
                        self.assertEqual(edge['properties']['source_file_sha256'], digest)
                        self.assertEqual(edge['status'], 'canon' if branch == 'canon' else 'unmarked')
                        material = {**edge, 'source_ref': packs[0]['path']}
                        normalized = _normalize_relation(material, branch, {})
                        self.assertEqual(normalized['attributes']['source_record'], edge['properties']['source_record'])
                        self.assertEqual(normalized['attributes']['source_row'], edge['properties']['source_row'])
                        self.assertEqual(normalized['attributes']['source_file_sha256'], digest)
                        self.assertEqual(normalized['source_refs'], [packs[0]['path']])
                        self.assertEqual(normalized['epistemic']['canon_status'], edge['status'])
                self.assertEqual(path.read_bytes(), raw)
                legacy = {key: value for key, value in edges[0].items() if key != 'properties'}
                validator.validate(legacy)
                for field, value in (('source_row', True), ('source_file_sha256', 'invented'),
                                     ('source_record', {'confidence': 0.7})):
                    changed = copy.deepcopy(edges[0])
                    changed['properties'][field] = value
                    with self.subTest(invalid_field=field):
                        self.assertFalse(validator.is_valid(changed))

    def test_relation_csv_refuses_ambiguous_headers_or_unnamed_cells_without_partial_pack(self):
        import tos_corpus_index_common as corpus
        for raw in ('edge_id,note,note\none,left,right\n', 'edge_id,\none,value\n',
                    'edge_id,note\none,known,unnamed\n', 'edge_id,note\none,"unterminated\n'):
            with self.subTest(raw=raw), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                path = root / 'ToS/candidate-intake/synthetic/edges.csv'
                path.parent.mkdir(parents=True)
                path.write_text(raw, encoding='utf-8')
                with patch.object(corpus, 'REPO_ROOT', root):
                    errors = []
                    self.assertEqual(corpus.build_relations(errors, (path,)), ([], []))
                self.assertEqual(len(errors), 1)
                self.assertEqual(errors[0]['level'], 'error')

    def test_relation_csv_changed_after_parse_cannot_publish_a_mixed_binding(self):
        import tos_corpus_index_common as corpus
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = root / 'ToS/canon/synthetic/edges.csv'
            path.parent.mkdir(parents=True)
            path.write_text('edge_id,note\none,original\n', encoding='utf-8')
            original_reader = corpus.read_edge_rows
            def changed_reader(selected):
                result = original_reader(selected)
                selected.write_text('edge_id,note\none,changed\n', encoding='utf-8')
                return result
            with patch.object(corpus, 'REPO_ROOT', root), \
                    patch.object(corpus, 'read_edge_rows', side_effect=changed_reader):
                with self.assertRaisesRegex(ValueError, 'relation source changed'):
                    corpus.build_relations([], (path,))

    def test_all_authored_relation_rows_are_retained_without_accepting_intake(self):
        import tos_corpus_index_common as corpus
        access_src = str(REPO_ROOT / 'access/src')
        if access_src not in sys.path:
            sys.path.insert(0, access_src)
        from tos_access.knowledge import _normalize_relation
        paths = tuple(path for path in tracked_tos_paths() if path.name == 'edges.csv')
        self.assertTrue(paths)
        errors = []
        packs, edges = corpus.build_relations(errors, paths)
        self.assertEqual(errors, [])
        for path in paths:
            ref = path.relative_to(REPO_ROOT).as_posix()
            pack = next(pack for pack in packs if pack['path'] == ref)
            with path.open(encoding='utf-8', newline='') as stream:
                source_rows = list(csv.DictReader(stream))
            actual = [edge for edge in edges if edge['pack_id'] == pack['pack_id']]
            self.assertEqual([edge['properties']['source_record'] for edge in actual], source_rows)
            self.assertEqual([edge['properties']['source_row'] for edge in actual], list(range(1, len(source_rows) + 1)))
            self.assertEqual(pack['sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for row, edge in zip(source_rows, actual):
                self.assertEqual(edge['edge_id'], row['edge_id'])
                self.assertEqual(edge['properties']['source_file_sha256'], pack['sha256'])
                self.assertEqual(edge['status'], row.get('status') or ('canon' if ref.startswith('ToS/canon/') else 'unmarked'))
                normalized = _normalize_relation({**edge, 'source_ref': ref},
                    'canon' if ref.startswith('ToS/canon/') else 'candidate-intake', {},
                    identity_id=f"{edge['pack_id']}:{edge['edge_id']}")
                returned = corpus.read_exact_edge_row(path, **{field: normalized['attributes'][field]
                    for field in ('source_file_sha256', 'source_row', 'source_record')})
                self.assertEqual(returned['record'], row)
                self.assertEqual(returned['source_file_sha256'], pack['sha256'])

    def test_text_packet_projection_is_versioned_and_visibility_bounded(self):
        path = REPO_ROOT / "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-a-source-layout-observation.json"
        packet = json.loads(path.read_text())
        nodes, edges = project_text_packet(packet, path.relative_to(REPO_ROOT).as_posix())
        self.assertTrue({"text-layer", "anchor", "text-unit", "annotation"}.issubset({n['node_kind'] for n in nodes}))
        self.assertTrue(any(e['predicate_id'] == 'has_text_layer' for e in edges))
        for node in nodes:
            self.assertIn('packet_version', node['properties'])
        packet['rights_and_visibility']['packet_visibility'] = 'restricted'
        self.assertEqual(project_text_packet(packet, 'ToS/private.json'), ([], []))

    def test_private_text_metadata_does_not_publish_word_hashes(self):
        path = REPO_ROOT / "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.dta-layout.v1.json"
        packet = json.loads(path.read_text())
        nodes, edges = project_text_packet(packet, path.relative_to(REPO_ROOT).as_posix())
        self.assertGreater(len(nodes), 0)
        rendered = json.dumps(nodes)
        self.assertNotIn('exact_sha256', rendered)
        self.assertTrue(all(n['properties']['content_available'] is False for n in nodes))

    def test_branch_parents_preserve_nearest_authored_ancestor_and_path_identity(self) -> None:
        paths = [
            "tree/a/deep/leaf", "tree/ab/leaf", "tree/a", "tree/ab",
            "elsewhere/a", "tree/a//", "tree", "tree/a/gap/leaf",
            "tree/a/deep", "tree/a/./",
        ]
        expected = {
            "tree/a": "tree", "tree/a/./": "tree", "tree/a//": "tree",
            "tree/ab": "tree", "tree/a/deep": "tree/a",
            "tree/a/gap/leaf": "tree/a", "tree/ab/leaf": "tree/ab",
            "tree/a/deep/leaf": "tree/a/deep",
        }
        for input_paths in (paths, list(reversed(paths)), []):
            with self.subTest(input_paths=input_paths):
                self.assertEqual(_nearest_branch_parents(input_paths), expected if input_paths else {})

    def test_partitioned_validator_rejects_diagnostics_outside_header(self) -> None:
        import validate_tos_corpus_index as validator
        from partitioned_projection_common import write_partitioned_payload, ProjectionReader
        payload = {'schema_version': 'tos_corpus_index_v1',
            'diagnostics': [{'level': 'error', 'path': 'ToS/example', 'message': 'unresolved reference'}],
            'nodes': [], 'resources': [], 'manifests': [], 'relation_packs': [], 'relation_edges': [],
            'source_navigation': {'nodes': [], 'edges': [], 'rights': []}}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'corpus.json'
            write_partitioned_payload(path, payload)
            self.assertNotIn('diagnostics', ProjectionReader(path).metadata())
            with patch.object(validator, 'TOS_CORPUS_INDEX_PATH', path), \
                    patch.object(validator, 'build_payload', return_value=payload):
                with self.assertRaisesRegex(SystemExit, 'contains error diagnostics.*unresolved reference'):
                    validator.main()

    def test_validator_rejects_noncanonical_encoding_with_one_rebuild(self) -> None:
        import validate_tos_corpus_index as validator

        # Isolate serialization parity from the independently tested schema
        # and whole-corpus build; both encodings parse to the expected object.
        payload = {"counts": {}}
        for text in (' {"counts": {}}\n', '{"counts": {}, "counts": {}}\n'):
            with (
                self.subTest(text=text),
                patch.object(validator, "build_payload", return_value=payload) as build,
                patch.object(validator, "validate_payload_schema"),
                patch.object(validator, "TOS_CORPUS_INDEX_PATH") as source,
            ):
                source.read_text.return_value = text
                self.assertEqual(json.loads(text), payload)
                with self.assertRaisesRegex(SystemExit, "canonical rebuild"):
                    validator.main()
                build.assert_called_once_with()

    def test_generated_index_matches_builder(self) -> None:
        expected = render_payload(build_payload())
        current = TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8")
        self.assertEqual(current, expected)

    def test_index_keeps_runtime_projection_subordinate(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")
        self.assertIn("runtime_projection", [entry["layer"] for entry in payload["authority_order"]])
        self.assertGreater(payload["counts"]["resources"], payload["counts"]["nodes"])

    def test_index_has_no_error_diagnostics(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        errors = [
            diagnostic
            for diagnostic in payload["diagnostics"]
            if diagnostic.get("level") == "error"
        ]
        self.assertEqual(errors, [])

    def test_authored_node_and_source_record_fields_survive_projection_losslessly(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        indexed_nodes = {node["node_id"]: node for node in payload["nodes"]}
        source_nodes = {
            node["node_id"]: node for node in payload["source_navigation"]["nodes"]
        }
        access_src = str(REPO_ROOT / 'access/src')
        if access_src not in sys.path:
            sys.path.insert(0, access_src)
        from tos_access.knowledge import _normalize_node
        authored_ids = set()
        for path in tracked_tos_paths():
            if path.name != 'node.json':
                continue
            source = json.loads(path.read_text(encoding='utf-8'))
            authored_ids.add(source['node_id'])
            projected = indexed_nodes[source['node_id']]
            self.assertEqual(projected['properties'], source)
            self.assertEqual(projected['source_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            normalized = _normalize_node(projected, 'canon')
            self.assertEqual(normalized['source_record']['payload']['properties'], source)
            self.assertIn(path.relative_to(REPO_ROOT).as_posix(), normalized['source_refs'])
        self.assertEqual(set(indexed_nodes), authored_ids)

        authored_path = REPO_ROOT / (
            "ToS/canon/synthesis/friedrich-nietzsche/thus-spoke-zarathustra/"
            "prologue-1/departure-from-reflective-origin/node.json"
        )
        authored = json.loads(authored_path.read_text(encoding="utf-8"))
        projected = indexed_nodes[authored["node_id"]]
        self.assertEqual(projected["properties"], authored)
        self.assertNotEqual(projected["label"], authored["source_anchor"])
        self.assertIn("departure", projected["label"].casefold())
        self.assertEqual(projected["properties"]["key_terms"], authored["key_terms"])
        self.assertEqual(projected["properties"]["relations"], authored["relations"])

        record_path = REPO_ROOT / (
            "ToS/source-witnesses/works/friedrich-nietzsche/"
            "also-sprach-zarathustra/work.json"
        )
        record = json.loads(record_path.read_text(encoding="utf-8"))
        source_node = source_nodes[record["record_id"]]
        self.assertEqual(source_node["properties"]["source_record"], record)
        self.assertEqual(source_node["properties"]["record_version"], 10)
        self.assertEqual(
            source_node["properties"]["same_as_posture"],
            "no_equivalence_claim",
        )

    def test_source_navigation_joins_branch_work_item_file_and_links(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        navigation = payload["source_navigation"]
        nodes = {node["node_id"]: node for node in navigation["nodes"]}
        edges = {
            (edge["from_id"], edge["predicate_id"], edge["to_id"])
            for edge in navigation["edges"]
        }
        n16 = "tos.work.proto-cuneiform.n16-in-the-archaic-texts"
        self.assertIn(
            (
                "philosophy.eras.bronze-age.regions.west-asia.traditions.proto-cuneiform-accounting-ontologies",
                "has_source_planting",
                "tos.planting.a01.cdlb-2006-1-n16",
            ),
            edges,
        )
        self.assertIn(
            (n16, "downloadable_at", "tos.link.cdli.cdlb-2006-1.pdf"),
            edges,
        )
        item = (
            "tos.item.egyptian-scholarship.on-four-songs-contained-in-an-"
            "egyptian-papyrus-in-the-british-museum.en-goodwin-1874."
            "internet-archive-nls-scan-pdf"
        )
        file_id = "tos.file.sha256.71ca30507a61a791b503102b43c034aad08b45c5d78bbedf16043632add33675"
        self.assertIn((item, "has_file", file_id), edges)
        self.assertEqual(nodes[file_id]["properties"]["sha256"], file_id.removeprefix("tos.file.sha256."))
        self.assertEqual(nodes["tos.link.internet-archive.onfoursongsconta00good.pdf-download"]["properties"]["access_status"], "open_download")
        self.assertIn(
            {
                "value": "Фридрих Ницше",
                "language": "ru",
                "source_ref": "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/work.json",
                "status": "verified",
            },
            nodes["tos.agent.friedrich-nietzsche"]["properties"]["variant_labels"],
        )
        self.assertIn(
            "Так говорил Заратустра",
            {
                entry["value"]
                for entry in nodes["tos.work.friedrich-nietzsche.also-sprach-zarathustra"]["properties"]["variant_labels"]
            },
        )
        self.assertIn(
            (
                "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
                "authored_by",
                "tos.agent.friedrich-nietzsche",
            ),
            edges,
        )
        authorship = next(
            edge
            for edge in navigation["edges"]
            if edge["from_id"] == "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
            and edge["predicate_id"] == "authored_by"
            and edge["to_id"] == "tos.agent.friedrich-nietzsche"
        )
        self.assertEqual(authorship["review_status"], "unreviewed")
        self.assertTrue(authorship["claim_ref"].endswith("authored-by-friedrich-nietzsche"))
        self.assertIn(
            "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/responsibility-claims.jsonl",
            authorship["source_refs"],
        )
        exact_scan_rights = next(
            right
            for right in navigation["rights"]
            if right["rights_id"].endswith("layer.exact-nls-digital-scan")
        )
        self.assertEqual(exact_scan_rights["review_status"], "unreviewed")

    def test_index_resources_are_owned_by_the_tracked_source_view(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        tracked_refs = {
            path.relative_to(REPO_ROOT).as_posix()
            for path in tracked_tos_paths()
        }
        resource_refs = {resource["path"] for resource in payload["resources"]}

        self.assertLessEqual(resource_refs, tracked_refs)
        self.assertFalse(
            any("payload" in Path(path_ref).parts for path_ref in resource_refs)
        )

    def test_authority_order_declares_all_emitted_layers(self) -> None:
        payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
        declared = {entry["layer"] for entry in payload["authority_order"]}
        emitted = set()
        for collection_name in (
            "branches",
            "manifests",
            "nodes",
            "relation_packs",
            "relation_edges",
            "resources",
        ):
            emitted.update(
                item["authority_layer"]
                for item in payload[collection_name]
                if "authority_layer" in item
            )
        self.assertEqual(sorted(emitted - declared), [])


if __name__ == "__main__":
    unittest.main()
