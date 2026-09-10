from __future__ import annotations

import copy
import hashlib
import json
import sys
import tempfile
import unittest
from collections import defaultdict
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from philosophy_atlas_projection_common import (  # noqa: E402
    ENDPOINT_ALIASES_REF,
    AuthoredAtlasSnapshot,
    CANDIDATE_NODES_REFS,
    CANDIDATE_RELATIONS_REFS,
    PROJECTION_PATH,
    build_payload,
    render_payload,
    validate_authored_source_context,
)


class AuthoredAtlasSnapshotTest(unittest.TestCase):
    def test_both_owner_schemas_keep_the_same_unknown_preserving_locator_contract(self):
        from jsonschema import Draft202012Validator
        schemas = [json.loads((REPO_ROOT / 'ToS/contracts' / name).read_bytes())
                   for name in ('philosophy-atlas-projection.schema.json', 'philosophy-graph-projection.schema.json')]
        self.assertEqual(schemas[0]['$defs']['authoredSourceProperties'], schemas[1]['$defs']['authoredSourceProperties'])
        context = {'source_record': {'unknown': [None, False, '', {}, []]},
            'source_record_ref': 'ToS/philosophy/atlas/example.jsonl', 'source_file_sha256': '0' * 64,
            'source_record_sha256': '1' * 64, 'source_format': 'jsonl', 'source_row': 1, 'source_line': 2}
        for schema in schemas:
            validator = Draft202012Validator({'$defs': schema['$defs'], '$ref': '#/$defs/authoredSourceProperties'})
            validator.validate(context)
            for altered in ({**context, 'source_row': True}, {**context, 'source_file_sha256': 'not-a-digest'},
                            {**context, 'source_format': 'json'}, {**context, 'source_record': []}):
                self.assertFalse(validator.is_valid(altered))

    def test_jsonl_preserves_unknown_nested_values_and_distinct_file_record_locators(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ref = 'ToS/philosophy/atlas/example.jsonl'
            path = root / ref
            path.parent.mkdir(parents=True)
            records = [{'row_id': 'first', 'unknown': {'null': None, 'false': False, 'empty': [], 'text': ''}},
                       {'row_id': 'second', 'source_ref': ref, 'text': 'line one\nline two'}]
            raw = ('\n' + json.dumps(records[0]) + '\n\n' + json.dumps(records[1]) + '\n').encode()
            path.write_bytes(raw)
            snapshot = AuthoredAtlasSnapshot(root)
            rows = snapshot.rows(ref, 'row_id')
            self.assertEqual([record for record, context in rows], records)
            self.assertEqual([context['source_row'] for record, context in rows], [1, 2])
            self.assertEqual([context['source_line'] for record, context in rows], [2, 4])
            for record, context in rows:
                self.assertEqual(context['source_record'], record)
                self.assertEqual(context['source_file_sha256'], hashlib.sha256(raw).hexdigest())
                canonical = json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()
                self.assertEqual(context['source_record_sha256'], hashlib.sha256(canonical).hexdigest())
                validate_authored_source_context({'source_ref': ref, 'properties': context})
                self.assertNotIn('record_id', record)
                self.assertNotIn('schema_version', record)
            rows[0][1]['source_record']['unknown']['null'] = 'changed copy'
            self.assertIsNone(rows[0][0]['unknown']['null'])
            snapshot.verify_current()
            self.assertEqual(path.read_bytes(), raw)
            path.write_bytes(raw + b'\n')
            with self.assertRaisesRegex(ValueError, 'source changed'):
                snapshot.verify_current()

    def test_whole_json_return_has_no_invented_row_or_native_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ref = 'ToS/philosophy/atlas/example.json'
            path = root / ref
            path.parent.mkdir(parents=True)
            path.write_text('{"atlas_id":"prepared","constraints":{"unresolved":true,"value":null}}')
            record, context = AuthoredAtlasSnapshot(root).object(ref)
            self.assertEqual(context['source_record'], record)
            self.assertEqual(context['source_pointer'], '')
            self.assertEqual(context['source_format'], 'json')
            self.assertNotIn('source_row', context)
            self.assertNotIn('source_line', context)
            validate_authored_source_context({'source_ref': ref, 'properties': context})
            for altered in ({**context, 'source_record': {**record, 'invented': True}},
                            {**context, 'source_record_ref': 'ToS/philosophy/another.json'},
                            {**context, 'source_row': True}):
                with self.assertRaises(ValueError):
                    validate_authored_source_context({'source_ref': ref, 'properties': altered})

    def test_duplicate_or_unbound_source_rows_fail_without_source_writes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ref = 'ToS/philosophy/atlas/example.jsonl'
            path = root / ref
            path.parent.mkdir(parents=True)
            for raw in (b'{"row_id":"one","row_id":"two"}\n', b'{"row_id":"one"}\n{"row_id":"one"}',
                        b'{"row_id":"one","unbound":NaN}', b'{"row_id":null}',
                        b'{"row_id":"one","source_ref":"ToS/private.jsonl"}'):
                path.write_bytes(raw)
                with self.subTest(raw=raw), self.assertRaises(ValueError):
                    AuthoredAtlasSnapshot(root).rows(ref, 'row_id')
                self.assertEqual(path.read_bytes(), raw)
            retained = path.with_name('other.jsonl')
            path.rename(retained)
            path.symlink_to(retained.name)
            with self.assertRaisesRegex(ValueError, 'exact regular'):
                AuthoredAtlasSnapshot(root).rows(ref, 'row_id')


class PhilosophyAtlasProjectionTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        # One fresh production build per class run; individual tests still get
        # independent objects. No persisted result or source-edit reuse.
        cls._built_projection = None
        cls.addClassCleanup(setattr, cls, "_built_projection", None)

    @classmethod
    def rebuilt_projection(cls) -> dict[str, object]:
        if cls._built_projection is None:
            cls._built_projection = build_payload()
        return copy.deepcopy(cls._built_projection)

    def test_generated_projection_matches_builder(self) -> None:
        expected = render_payload(self.rebuilt_projection())
        current = PROJECTION_PATH.read_text(encoding="utf-8")
        self.assertEqual(current, expected)

    def test_projection_has_expected_atlas_counts(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        self.assertEqual(payload["counts"]["master_tables"], 3)
        self.assertEqual(payload["counts"]["master_rows"], 190)
        self.assertEqual(payload["counts"]["dossiers"], 190)
        self.assertEqual(payload["counts"]["dossier_node_rows"], 7193)
        self.assertEqual(payload["counts"]["dossier_relation_rows"], 8564)
        self.assertEqual(payload["counts"]["candidate_nodes"], 7193)
        self.assertEqual(payload["counts"]["candidate_relations"], 8564)
        self.assertEqual(payload["counts"]["candidate_endpoint_placeholders"], 458)

    def test_every_existing_source_row_and_manifest_returns_exact_full_context(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_bytes())
        nodes = {row['node_id']: row for row in payload['nodes']}
        edges = {row['edge_id']: row for row in payload['edges']}
        atlas_ref = 'ToS/philosophy/atlas/atlas.manifest.json'
        atlas = json.loads((REPO_ROOT / atlas_ref).read_bytes())
        sources = [(table['rows'], 'row_id', 'atlas-row:', nodes) for table in atlas['master_tables']]
        sources += [('ToS/philosophy/atlas/dossiers/index.jsonl', 'dossier_id', 'atlas-dossier:', nodes)]
        sources += [(ref, 'candidate_id', 'candidate-node:', nodes) for ref in CANDIDATE_NODES_REFS]
        sources += [(ref, 'candidate_id', 'edge:candidate-relation:', edges) for ref in CANDIDATE_RELATIONS_REFS]
        checked = 0
        for ref, identity, prefix, carriers in sources:
            raw = (REPO_ROOT / ref).read_bytes()
            file_digest = hashlib.sha256(raw).hexdigest()
            row_number = 0
            for line_number, line in enumerate(raw.splitlines(), start=1):
                if not line.strip():
                    continue
                row_number += 1
                record = json.loads(line)
                carrier = carriers[prefix + record[identity]]
                context = carrier['properties']
                self.assertEqual(context['source_record'], record, prefix + record[identity])
                self.assertEqual(context['source_record_ref'], ref)
                self.assertEqual(carrier['source_ref'], ref)
                self.assertEqual(context['source_file_sha256'], file_digest)
                self.assertEqual(context['source_row'], row_number)
                self.assertEqual(context['source_line'], line_number)
                validate_authored_source_context(carrier)
                checked += 1
        self.assertEqual(checked, 190 + 190 + 7193 + 8564)
        for node_id, ref in [('philosophy.atlas', atlas_ref),
                             *[('atlas-table:' + table['table_id'], table['manifest']) for table in atlas['master_tables']]]:
            raw = (REPO_ROOT / ref).read_bytes()
            context = nodes[node_id]['properties']
            self.assertEqual(context['source_record'], json.loads(raw))
            self.assertEqual(context['source_record_ref'], ref)
            self.assertEqual(context['source_file_sha256'], hashlib.sha256(raw).hexdigest())
            self.assertEqual(context['source_pointer'], '')
            self.assertNotIn('source_row', context)

    def test_applied_endpoint_alias_context_retains_owner_limits_and_exact_selection(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_bytes())
        raw = (REPO_ROOT / ENDPOINT_ALIASES_REF).read_bytes()
        source = json.loads(raw)
        selected = set()
        for edge in payload['edges']:
            context = edge['properties'].get('endpoint_alias_source')
            if context is None:
                continue
            self.assertEqual(context['source_record'], source)
            self.assertEqual(context['source_file_sha256'], hashlib.sha256(raw).hexdigest())
            self.assertEqual(context['source_record_ref'], ENDPOINT_ALIASES_REF)
            for pointer in edge['properties']['endpoint_alias_pointers']:
                alias = source['aliases'][int(pointer.rsplit('/', 1)[1])]
                record = edge['properties']['source_record']
                self.assertEqual(alias['origin_dossier_id'], record['dossier_id'])
                self.assertEqual(alias['endpoint_label'], record[alias['endpoint_role'] + '_endpoint_label'])
                selected.add(pointer)
        self.assertEqual(selected, {f'/aliases/{index}' for index in range(len(source['aliases']))})

    def test_projection_keeps_runtime_owner_downstream(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")
        self.assertEqual(payload["source_atlas_ref"], "ToS/philosophy/atlas/atlas.manifest.json")
        self.assertEqual(
            payload["content_language_contract"]["source_ref"],
            "ToS/philosophy/atlas/multilingual/content-labels.json",
        )
        self.assertEqual(payload["content_language_contract"]["display_languages"], ["original", "ru", "en"])
        self.assertEqual(
            payload["content_language_contract"]["language_registry_ref"],
            "ToS/philosophy/atlas/multilingual/language-registry.json",
        )
        self.assertEqual(
            payload["content_language_contract"]["text_bearing_nodes_contract_ref"],
            "ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json",
        )

    def test_projection_links_rows_to_available_dossiers(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        edges = {
            (edge["from_id"], edge["predicate_id"], edge["to_id"])
            for edge in payload["edges"]
        }
        self.assertIn(("atlas-row:A01", "has_prepared_dossier", "atlas-dossier:A01"), edges)
        self.assertIn(("atlas-row:A11", "has_prepared_dossier", "atlas-dossier:A11"), edges)
        self.assertIn(("atlas-row:A43", "has_prepared_dossier", "atlas-dossier:A43"), edges)
        self.assertIn(("atlas-row:A48", "has_prepared_dossier", "atlas-dossier:A48"), edges)
        self.assertIn(("atlas-row:T2-01", "has_prepared_dossier", "atlas-dossier:T2-01"), edges)
        self.assertIn(("atlas-row:T2-26", "has_prepared_dossier", "atlas-dossier:T2-26"), edges)
        self.assertEqual(nodes["atlas-row:T2-26"]["properties"]["dossier_intake_status"], "admitted")
        self.assertEqual(
            nodes["atlas-row:T2-51"]["properties"]["dossier_intake_status"],
            "admitted",
        )
        self.assertEqual(nodes["atlas-row:T3-45"]["properties"]["dossier_intake_status"], "admitted")
        self.assertEqual(nodes["atlas-row:T3-46"]["properties"]["dossier_intake_status"], "admitted")

    def test_exact_admitted_dossier_endpoints_use_existing_dossier_nodes(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}
        nodes = {node["node_id"]: node for node in payload["nodes"]}

        self.assertEqual(
            edges["edge:candidate-relation:table-ii-t2-11-relation-035"]["to_id"],
            "atlas-dossier:T2-16",
        )
        self.assertEqual(
            edges["edge:candidate-relation:table-ii-t2-11-relation-035"]["properties"][
                "projection_endpoint_resolution"
            ],
            "exact_admitted_dossier",
        )
        self.assertEqual(
            edges["edge:candidate-relation:table-ii-t2-31-relation-039"]["to_id"],
            "atlas-dossier:T2-35",
        )
        self.assertEqual(
            edges["edge:candidate-relation:table-i-a18-relation-030"]["to_id"],
            "atlas-dossier:A41",
        )
        self.assertEqual(
            edges["edge:candidate-relation:table-i-a18-relation-030"]["properties"][
                "projection_endpoint_resolution"
            ],
            "exact_admitted_dossier",
        )
        self.assertEqual(
            sum(
                edge["edge_id"].startswith("edge:candidate-relation:table-ii-")
                and edge["to_id"].startswith("atlas-dossier:T2-")
                for edge in payload["edges"]
            ),
            14,
        )
        admitted_ids = {
            node_id.removeprefix("atlas-dossier:")
            for node_id in nodes
            if node_id.startswith("atlas-dossier:")
        }
        self.assertFalse(
            any(
                node["node_type"] == "candidate-endpoint" and node["label"] in admitted_ids
                for node in payload["nodes"]
            )
        )
        self.assertEqual(
            sum(
                edge["properties"].get("projection_endpoint_resolution")
                == "exact_admitted_dossier"
                for edge in payload["edges"]
            ),
            85,
        )

    def test_unresolved_dossier_endpoints_keep_candidate_endpoint(self) -> None:
        payload = self.rebuilt_projection()
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        edge = edges["edge:candidate-relation:table-iii-t3-45-relation-046"]

        endpoint = nodes[edge["to_id"]]
        self.assertEqual(endpoint["node_type"], "candidate-endpoint")
        self.assertTrue(endpoint["label"].startswith("T3-46"))
        self.assertEqual(
            edge["properties"]["endpoint_resolution"],
            "label_endpoint",
        )

    def test_endpoint_placeholders_preserve_all_observed_roles(self) -> None:
        payload = self.rebuilt_projection()
        endpoint_ids = {
            node["node_id"]
            for node in payload["nodes"]
            if node["node_type"] == "candidate-endpoint"
        }
        observed_roles: dict[str, set[str]] = defaultdict(set)
        for edge in payload["edges"]:
            if not edge["edge_id"].startswith("edge:candidate-relation:"):
                continue
            if edge["from_id"] in endpoint_ids:
                observed_roles[edge["from_id"]].add("source")
            if edge["to_id"] in endpoint_ids:
                observed_roles[edge["to_id"]].add("target")

        for node in payload["nodes"]:
            if node["node_type"] != "candidate-endpoint":
                continue
            expected_roles = sorted(observed_roles[node["node_id"]])
            self.assertEqual(node["properties"]["endpoint_roles"], expected_roles)
            self.assertEqual(
                node["properties"]["endpoint_role"],
                expected_roles[0] if len(expected_roles) == 1 else "source_and_target",
            )

        shared_id = "candidate-endpoint:T2-16:e1d923d12c45"
        shared = next(node for node in payload["nodes"] if node["node_id"] == shared_id)
        self.assertEqual(shared["properties"]["endpoint_roles"], ["source", "target"])
        self.assertEqual(shared["properties"]["endpoint_role"], "source_and_target")

    def test_reviewed_qualified_endpoints_resolve_inside_target_dossiers(self) -> None:
        payload = self.rebuilt_projection()
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}
        expected_targets = {
            "edge:candidate-relation:table-ii-t2-03-relation-038": "candidate-node:table-ii-t2-02-node-001",
            "edge:candidate-relation:table-ii-t2-11-relation-008": "candidate-node:table-ii-t2-09-node-001",
            "edge:candidate-relation:table-ii-t2-11-relation-009": "candidate-node:table-ii-t2-10-node-010",
            "edge:candidate-relation:table-ii-t2-11-relation-017": "candidate-node:table-ii-t2-10-node-019",
            "edge:candidate-relation:table-ii-t2-11-relation-019": "candidate-node:table-ii-t2-10-node-019",
        }

        for edge_id, target_id in expected_targets.items():
            edge = edges[edge_id]
            self.assertEqual(edge["to_id"], target_id)
            self.assertEqual(
                edge["properties"]["projection_endpoint_resolution"],
                "reviewed_qualified_alias",
            )
            self.assertEqual(edge["properties"]["endpoint_alias_ref"], ENDPOINT_ALIASES_REF)

    def test_reviewed_origin_role_aliases_resolve_unqualified_cross_dossier_endpoints(self) -> None:
        payload = self.rebuilt_projection()
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}
        expected = {
            "edge:candidate-relation:table-ii-t2-06-relation-008": (
                "from_id",
                "candidate-node:table-i-a37-node-030",
            ),
            "edge:candidate-relation:table-ii-t2-06-relation-015": (
                "from_id",
                "candidate-node:table-i-a37-node-029",
            ),
            "edge:candidate-relation:table-ii-t2-06-relation-016": (
                "from_id",
                "candidate-node:table-i-a37-node-029",
            ),
            "edge:candidate-relation:table-ii-t2-06-relation-018": (
                "from_id",
                "candidate-node:table-i-a37-node-033",
            ),
            "edge:candidate-relation:table-ii-t2-32-relation-033": (
                "to_id",
                "candidate-node:table-i-a19-node-006",
            ),
            "edge:candidate-relation:table-ii-t2-33-relation-030": (
                "to_id",
                "candidate-node:table-i-a19-node-006",
            ),
        }

        for edge_id, (endpoint_field, candidate_id) in expected.items():
            edge = edges[edge_id]
            self.assertEqual(edge[endpoint_field], candidate_id)
            self.assertEqual(
                edge["properties"]["projection_endpoint_resolution"],
                "reviewed_origin_role_alias",
            )
            self.assertEqual(edge["properties"]["endpoint_alias_ref"], ENDPOINT_ALIASES_REF)

    def test_projection_exposes_pre_canon_candidate_graph_material(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        node_ids = {node["node_id"] for node in payload["nodes"]}
        edge_predicates = {edge["predicate_id"] for edge in payload["edges"]}
        self.assertIn("candidate-node:table-i-a01-node-001", node_ids)
        self.assertIn("candidate-node:table-i-a43-node-001", node_ids)
        self.assertIn("candidate-node:table-i-a48-node-001", node_ids)
        self.assertIn("candidate-node:table-ii-t2-01-node-001", node_ids)
        self.assertTrue(any(node_id.startswith("candidate-node:table-ii-t2-26-") for node_id in node_ids))
        self.assertTrue(any(node_id.startswith("candidate-node:table-iii-t3-45-") for node_id in node_ids))
        self.assertIn("uses_script", edge_predicates)
        self.assertIn("develops_concept", edge_predicates)

    def test_projection_preserves_table_ii_manual_review_gates(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}

        dossier = nodes["atlas-dossier:T2-49"]["properties"]
        self.assertEqual(dossier["review_posture"], "manual_review_required")
        self.assertEqual(dossier["master_status"], "B")
        self.assertEqual(dossier["master_confidence"], "3")

        candidate = nodes["candidate-node:table-ii-t2-49-node-001"]["properties"]
        self.assertEqual(candidate["review_posture"], "manual_review_required")
        self.assertEqual(candidate["master_status"], "B")
        self.assertEqual(candidate["master_confidence"], "3")

        relation = edges["edge:candidate-relation:table-ii-t2-49-relation-001"]["properties"]
        self.assertEqual(relation["review_posture"], "manual_review_required")
        self.assertEqual(relation["master_status"], "B")
        self.assertEqual(relation["master_confidence"], "3")

        self.assertEqual(
            sum(
                node["node_type"] == "candidate-node"
                and node["properties"].get("table_id") == "table-ii"
                and node["properties"].get("review_posture") == "manual_review_required"
                for node in payload["nodes"]
            ),
            887,
        )
        self.assertEqual(
            sum(
                edge["edge_id"].startswith("edge:candidate-relation:table-ii-")
                and edge["properties"].get("review_posture") == "manual_review_required"
                for edge in payload["edges"]
            ),
            970,
        )

        gated_endpoints = [
            node
            for node in payload["nodes"]
            if node["node_type"] == "candidate-endpoint"
            and node["properties"].get("table_id") == "table-ii"
            and node["properties"].get("review_posture") == "manual_review_required"
        ]
        self.assertEqual(len(gated_endpoints), 137)
        self.assertTrue(
            all(
                node["properties"].get("review_reason")
                and node["properties"].get("master_status")
                and node["properties"].get("master_confidence")
                for node in gated_endpoints
            )
        )

    def test_projection_exposes_graph_view_routes(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        node_ids = {node["node_id"] for node in payload["nodes"]}
        self.assertIn("graph-view:chronology", node_ids)
        self.assertIn("graph-view:transmission", node_ids)

    def test_projection_exposes_source_owned_multilingual_labels(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        philosophy = nodes["philosophy"]["multilingual"]
        self.assertEqual(philosophy["label"]["ru"], "Философия")
        self.assertEqual(philosophy["label"]["en"], "Philosophy")
        self.assertEqual(philosophy["translation_status"]["original"], "not_applicable")
        dossier = nodes["atlas-dossier:A01"]["multilingual"]
        self.assertEqual(
            dossier["label"]["ru"],
            "ToS Deep Research: A01 — Протоклинопись и учётные онтологии",
        )
        self.assertEqual(
            dossier["label"]["en"],
            "ToS Deep Research: A01 — Proto-Cuneiform and Accounting Ontologies",
        )
        self.assertEqual(dossier["translation_status"]["en"], "reviewed")
        concept = nodes["atlas-node-type:concept"]["multilingual"]
        self.assertEqual(concept["label"]["ru"], "концепт")
        self.assertEqual(concept["label"]["en"], "concept")

    def test_table_ii_prepared_dossiers_use_reviewed_context_titles(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        expected = {
            "T2-09": ("Калām ранний и классический", "Early and Classical Kalam"),
            "T2-10": ("Falsafa и авиценновский синтез", "Falsafa and the Avicennian Synthesis"),
            "T2-16": (
                "Постмонгольский персидатский схоластический коридор",
                "The Post-Mongol Persianate Scholastic Corridor",
            ),
            "T2-41": (
                "Санскритская космопольная книжность ЮВА",
                "Sanskrit Cosmopolitan Literature in Southeast Asia",
            ),
        }
        for dossier_id, (ru_title, en_title) in expected.items():
            multilingual = nodes[f"atlas-dossier:{dossier_id}"]["multilingual"]
            self.assertEqual(multilingual["label"]["ru"], f"ToS Deep Research: {dossier_id} — {ru_title}")
            self.assertEqual(multilingual["label"]["en"], f"ToS Deep Research: {dossier_id} — {en_title}")
            self.assertNotRegex(multilingual["label"]["en"], r"[А-Яа-яЁё]")
            self.assertEqual(multilingual["translation_status"]["ru"], "reviewed")
            self.assertEqual(multilingual["translation_status"]["en"], "reviewed")


if __name__ == "__main__":
    unittest.main()
