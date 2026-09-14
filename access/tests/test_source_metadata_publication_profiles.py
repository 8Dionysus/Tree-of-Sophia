"""Profile-shaped source metadata growth over one bounded synthetic predecessor.

These tests exercise source creation and the prepared publication seam without
reading the authored corpus.  The Work, historical-environment and Claim
records are deliberately synthetic: a green test proves transport and
projection closure, not publication rights, canon admission or historical
truth.
"""

import copy
import hashlib
import json
import os
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
for directory in (
    "scripts",
    "access/src",
    "access/tests",
    "tests",
    "mechanics/growth-cycle/tests",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts",
):
    sys.path.insert(0, str(ROOT / directory))

import test_source_agent_publication as fixtures
import source_agent_publication as agent_publication
import source_claim_publication as claim_publication
import source_commands as commands
import source_metadata_publication as metadata_publication
from tos_access import knowledge as knowledge_graph
from tos_access.catalog_semantics import CANONICAL_ORDER, CatalogInputs, memory_catalog
from tos_access.prepared_source_binding import read_prepared_source_inputs_transaction
from tos_access.projection_store import canonical_bytes
from tos_access.published_read_model import PublishedKnowledgeReadModel
from tos_access.published_search import PublishedSearchService


class SourceMetadataPublicationProfileTests(unittest.TestCase):
    """Use the existing tiny SourceAgentPublicationTests database as a base."""

    def setUp(self):
        self.base = fixtures.SourceAgentPublicationTests()
        self.base.setUp()
        self.addCleanup(self.base.doCleanups)
        self.root, self.db = self.base.root, self.base.db

    def _copy_contracts(self, *refs):
        for ref in refs:
            self.base.helper.fixture.write(ref, (ROOT / ref).read_bytes())

    def _write_owner(self, name, config):
        owner = self.root / name
        owner.write_bytes(canonical_bytes(config))
        owner.chmod(0o600)
        return owner

    def _create_metadata_package(self, record, relative, *, schema_version,
                                 form_id, authority_ref, event_id,
                                 profile_type_id=None, record_type=None):
        """Create one exact source package through source.create, not a shim."""
        self._copy_contracts("ToS/contracts/provenance-event-v2.schema.json")
        path = self.root / relative
        # Native/profile creation atomically renames the subject directory;
        # only its already-owned parent is prepared here.
        path.parent.parent.mkdir(parents=True, exist_ok=True)
        config = {
            "schema_version": schema_version,
            "uid": os.getuid(),
            "principal_id": "software:synthetic",
            "maker_type": "software",
            "source_root": str(self.root),
            "source_path": relative,
            "record_id": record["record_id"],
            "authority_ref": authority_ref,
            "expires_at": "2099-01-01T00:00:00Z",
            "provenance_event_id": event_id,
            "allowed_operations": ["source.create"],
            "allowed_form_ids": [form_id],
        }
        if profile_type_id is not None:
            config["profile_type_id"] = profile_type_id
        if record_type is not None:
            config["record_type"] = record_type
        owner = self._write_owner("{}-owner.json".format(record["record_id"].rsplit(".", 1)[-1]), config)
        request = {
            "schema_version": "tos_local_source_command_v1",
            "operation": "prepare-create",
            "record": record,
            "forms": [{"field_id": "metadata.preferred-name", "form_id": form_id}],
        }
        preview = commands.run_local_command(owner, request)
        request.update(
            operation="source.create",
            command_id=record["record_id"].replace(".", "-") + "-create",
            expected_configuration=preview["owner_configuration"],
            expected_dependencies=preview["expected_dependencies"],
            expected_revision=None,
            expected_source=None,
        )
        created = commands.run_local_command(owner, request)
        receipt = path.with_name("source-create-receipt.json")
        expected = {
            "expected_receipt_sha256": hashlib.sha256(receipt.read_bytes()).hexdigest(),
            "expected_request_digest": created["receipt"]["request_digest"],
        }
        return owner, config, expected

    def _work_record(self, *, source_refs=None):
        record = copy.deepcopy(self.base.helper.fixture.record)
        record.update(
            record_id="tos.work.synthetic-profile",
            record_type="work",
            preferred_label="Synthetic profile Work",
            notes="Synthetic metadata, not a work admission.",
            source_refs=source_refs or ["test:synthetic-work"],
            expression_claim_refs=[],
        )
        record.pop("visibility", None)
        return record

    def _create_work(self, *, source_refs=None):
        return self._create_metadata_package(
            self._work_record(source_refs=source_refs),
            "ToS/source-witnesses/works/synthetic-profile/work.json",
            schema_version=commands.CORPUS_CONFIG,
            record_type="work",
            form_id="tos.form.synthetic-work.name",
            authority_ref="test:work-profile",
            event_id="tos.event.synthetic-work",
        )

    def _environment_record(self):
        return {
            "schema_version": "tos_historical_context_record_v1",
            "record_type": "historical-environment",
            "record_id": "tos.historical-environment.synthetic-profile",
            "record_version": 1,
            "identity_status": "provisional",
            "same_as_posture": "no_equivalence_claim",
            "preferred_label": "Synthetic historical environment",
            "variant_labels": [],
            "field_languages": {
                "preferred_label": {"language": "en", "script": "Latn"},
                "notes": {"language": "en", "script": "Latn"},
            },
            "notes": "Synthetic metadata, not a historical admission.",
            "external_identifiers": [],
            "source_refs": ["test:synthetic-environment"],
            "semantic_content": {
                "environment_account": "Synthetic scoped configuration of conditions.",
                "political_account": "Synthetic political scope; no person-level claim.",
                "language": "en",
                "script": "Latn",
            },
            "semantic_scope": {
                "identity_criterion": "Synthetic bounded conditions, not a place or complete world model.",
                "scope_note": "Synthetic test context only; no causal influence asserted.",
                "language": "en",
                "script": "Latn",
            },
            "visibility": "public_metadata_only",
        }

    def _create_environment(self):
        self._copy_contracts(
            "ToS/contracts/source-metadata-record.schema.json",
            "ToS/contracts/semantic-description-record.schema.json",
            "ToS/contracts/historical-context-record.schema.json",
        )
        return self._create_metadata_package(
            self._environment_record(),
            "ToS/source-witnesses/history/synthetic-profile/historical-environment.json",
            schema_version=commands.PROFILE_CONFIG,
            profile_type_id="tos.entity.historical-environment",
            form_id="tos.form.synthetic-environment.name",
            authority_ref="test:environment-profile",
            event_id="tos.event.synthetic-environment",
        )

    def _create_context_claim(self, environment_id):
        self._copy_contracts("ToS/contracts/historical-context-claim.schema.json")
        evidence = "ToS/review-ledger/synthetic-environment-context.md"
        self.base.helper.fixture.write(evidence, b"Synthetic integration evidence only.\n")
        claim = {
            "schema_version": "tos_historical_context_claim_v1",
            "claim_id": "tos.claim.synthetic-environment-context",
            "claim_version": 1,
            "claim_type": "relation",
            "assertion_layer": "scholarly_report",
            "subject_ref": self.base.helper.fixture.identity,
            "predicate": "contextualized_by_environment",
            "object": environment_id,
            "evidence_refs": [evidence],
            "counterevidence_refs": [],
            "alternative_claim_refs": [],
            "maker": {"maker_type": "software", "agent_ref": "software:synthetic"},
            "provenance_event_ref": "tos.event.synthetic-environment-claim",
            "epistemic_status": "uncertain",
            "review_status": "unreviewed",
            "visibility": "public_metadata_only",
            "qualifiers": {
                "statement": "Synthetic environment context; not causal or semantic acceptance.",
                "statement_language": "en",
                "statement_script": "Latn",
                "relation_basis": "Synthetic test fixture only.",
                "context_scope": "Synthetic context fixture only; no causal influence asserted.",
                "time_scope_note": "No historical dates asserted.",
            },
        }
        relative = "ToS/source-witnesses/relations/synthetic-environment-context/source-claims.jsonl"
        config = {
            "schema_version": commands.CLAIM_CONFIG,
            "uid": os.getuid(),
            "principal_id": "software:synthetic",
            "maker_type": "software",
            "source_root": str(self.root),
            "source_path": relative,
            "authority_ref": "test:claim-profile",
            "expires_at": "2099-01-01T00:00:00Z",
            "provenance_event_id": "tos.event.synthetic-environment-claim",
            "allowed_operations": ["claims.create"],
            "allowed_claim_ids": [claim["claim_id"]],
            "allowed_subject_refs": [claim["subject_ref"]],
            "allowed_object_refs": [claim["object"]],
            "allowed_predicates": [claim["predicate"]],
            "allowed_evidence_refs": [evidence],
        }
        owner = self._write_owner("synthetic-environment-claim-owner.json", config)
        request = {
            "schema_version": "tos_local_source_command_v1",
            "operation": "prepare-create",
            "claims": [claim],
        }
        preview = commands.run_local_command(owner, request)
        request.update(
            operation="claims.create",
            command_id="synthetic-environment-claim-create",
            expected_configuration=preview["owner_configuration"],
            expected_dependencies=preview["expected_dependencies"],
            expected_inputs=preview["source_bindings"],
            expected_revision=None,
        )
        created = commands.run_local_command(owner, request)
        receipt = (self.root / relative).with_name("source-create-receipt.json")
        expected = {
            "expected_receipt_sha256": hashlib.sha256(receipt.read_bytes()).hexdigest(),
            "expected_request_digest": created["receipt"]["request_digest"],
        }
        return owner, config, claim, expected

    def _metadata_operation(self, owner, expected, *, limits=None):
        return metadata_publication.metadata_addition_publication(
            owner,
            source_inputs=self.base.source,
            expected_binding=self.base.binding,
            catalog_inputs=self.base.inputs,
            progress_owner=self.base.owner,
            limits=limits,
            **expected,
        )

    def _publish_metadata(self, owner, expected, *, limits=None):
        with self._metadata_operation(owner, expected, limits=limits) as operation:
            raw = copy.deepcopy(operation.raw)
            self.db.execute("BEGIN IMMEDIATE")
            result = operation.apply_transaction(self.db)
            committed = operation.commit_transaction(self.db)
        return raw, committed

    def _publish_claim(self, owner, expected):
        with claim_publication.claim_addition_publication(
            owner,
            source_inputs=self.base.source,
            expected_binding=self.base.binding,
            catalog_inputs=self.base.inputs,
            progress_owner=self.base.owner,
            **expected,
        ) as operation:
            raw = copy.deepcopy(operation.raw)
            self.db.execute("BEGIN IMMEDIATE")
            result = operation.apply_transaction(self.db)
            committed = operation.commit_transaction(self.db)
        return raw, committed

    def _advance_prepared_inputs(self, result):
        self.db.execute("BEGIN")
        paired = read_prepared_source_inputs_transaction(
            self.db, expected_binding=result["binding"]
        )
        self.db.rollback()
        self.base.binding = result["binding"]
        self.base.source = paired
        self.base.inputs = CatalogInputs(
            copy.deepcopy(result["source_header"]),
            self.base.entities,
            self.base.relations,
            self.base.inputs.lenses,
            source_order_profile=CANONICAL_ORDER,
        )

    def _union_oracle(self, raws):
        """Build the independent tiny union, merging retained Claim rows by ID."""
        corpus = copy.deepcopy(self.base.corpus)
        bibliography = copy.deepcopy(self.base.bibliography)
        collections = {
            "source-navigation": (corpus["source_navigation"], "node_id", "edge_id"),
            "source-claims": (bibliography, "node_id", "edge_id"),
        }
        for raw in raws:
            for (graph, _), row in raw["nodes"].items():
                target, key, _ = collections[graph]
                rows = {item[key]: item for item in target["nodes"]}
                rows[row[key]] = row
                target["nodes"] = list(rows.values())
            for (graph, _), row in raw["edges"].items():
                target, _, key = collections[graph]
                rows = {item[key]: item for item in target["edges"]}
                rows[row[key]] = row
                target["edges"] = list(rows.values())
            for key, row in raw["traces"].items():
                claim_key = row.get("claim_ref", key)
                rows = {item["claim_ref"]: item for item in bibliography["claim_traces"]}
                rows[claim_key] = row
                bibliography["claim_traces"] = list(rows.values())
        return (
            knowledge_graph.build_knowledge_graph(
                corpus,
                {},
                bibliography,
                self.base.entities,
                self.base.relations,
            ),
            corpus,
        )

    def _assert_union_lanes(self, raws, result, query):
        expected, corpus = self._union_oracle(raws)
        expected.update(result["source_header"])
        for kind in ("node", "relation"):
            stored = {
                identity: json.loads(raw)
                for identity, raw in self.db.execute(
                    "SELECT id,json FROM knowledge_" + kind + "s"
                )
            }
            self.assertEqual(stored, {row["id"]: row for row in expected[kind + "s"]})
        reader = PublishedKnowledgeReadModel(self.base.path, result["binding"])
        self.assertEqual(
            reader.catalog(),
            memory_catalog(expected, corpus, {}, self.base.entities, self.base.relations),
        )
        self.assertEqual(
            PublishedSearchService(reader).search(query, limit=10)["nodes"],
            knowledge_graph.search_knowledge_graph(expected, query, limit=10)["nodes"],
        )
        return expected

    def _context_state(self):
        raw = self.db.execute("SELECT json FROM agent_context_state WHERE singleton=1").fetchone()[0]
        return json.loads(raw)

    def test_standalone_work_uses_native_create_and_retains_dossier_membership(self):
        owner, config, expected = self._create_work()
        self.assertEqual(config["schema_version"], commands.CORPUS_CONFIG)
        self.assertEqual(config["record_type"], "work")
        raw, result = self._publish_metadata(owner, expected)
        self.assertEqual(result["changed_nodes"], 3)
        self.assertEqual(result["changed_relations"], 2)
        self.assertFalse(result["consumer_switched"])
        self.assertFalse(result["is_semantic_acceptance"])
        self.assertIn(
            "tos.work.synthetic-profile",
            {key for key in self._context_state()["dossier_refs"]},
        )
        self._assert_union_lanes([raw], result, "Synthetic profile Work")

    def test_historical_environment_uses_generic_profile_not_agent_lane(self):
        owner, config, expected = self._create_environment()
        self.assertEqual(config["schema_version"], commands.PROFILE_CONFIG)
        self.assertEqual(config["profile_type_id"], "tos.entity.historical-environment")
        self.assertNotEqual(config["schema_version"], commands.CORPUS_CONFIG)
        raw, result = self._publish_metadata(owner, expected)
        navigation = [
            row for (graph, _), row in raw["nodes"].items()
            if graph == "source-navigation" and row["node_id"] == "tos.historical-environment.synthetic-profile"
        ]
        self.assertEqual(len(navigation), 1)
        self.assertEqual(navigation[0]["node_kind"], "historical-environment")
        self.assertNotIn(
            "tos.historical-environment.synthetic-profile",
            self._context_state()["dossier_refs"],
        )
        self._assert_union_lanes([raw], result, "Synthetic historical environment")

    def test_environment_then_claim_addition_matches_union_on_same_prepared_db(self):
        env_owner, _, env_expected = self._create_environment()
        env_raw, env_result = self._publish_metadata(env_owner, env_expected)
        self._advance_prepared_inputs(env_result)
        claim_owner, _, claim, claim_expected = self._create_context_claim(
            "tos.historical-environment.synthetic-profile"
        )
        claim_raw, claim_result = self._publish_claim(claim_owner, claim_expected)
        self.assertEqual(claim["predicate"], "contextualized_by_environment")
        self.assertEqual(claim_result["claim_ids"], [claim["claim_id"]])
        self.assertEqual(claim_result["changed_nodes"], 3)
        self.assertEqual(claim_result["changed_relations"], 5)
        self.assertFalse(claim_result["consumer_switched"])
        self.assertFalse(claim_result["is_semantic_acceptance"])
        self.assertNotIn(
            "tos.historical-environment.synthetic-profile",
            self._context_state()["dossier_refs"],
        )
        self._assert_union_lanes([env_raw, claim_raw], claim_result, "Synthetic environment context")

    def test_budget_refusal_can_be_explicitly_rolled_back_without_source_omit(self):
        owner, _, expected = self._create_work()
        before = list(self.db.iterdump())
        limits = agent_publication.AgentPublicationLimits(
            max_nodes=3, max_relations=1, max_claims=1, max_bytes=16 * 1024 * 1024
        )
        with self._metadata_operation(owner, expected, limits=limits) as operation:
            self.db.execute("BEGIN IMMEDIATE")
            with self.assertRaisesRegex(ValueError, "metadata normalized cohort count budget exceeded"):
                operation.apply_transaction(self.db)
            rollback = operation.rollback_transaction(self.db)
            self.assertTrue(rollback["prepared_rolled_back"])
        self.assertEqual(list(self.db.iterdump()), before)
        self.assertTrue((self.root / "ToS/source-witnesses/works/synthetic-profile/work.json").is_file())

    def test_canon_grounding_is_rejected_instead_of_silently_omitted(self):
        owner, _, expected = self._create_work(
            source_refs=["ToS/canon/synthetic-grounding/node.json"]
        )
        before = list(self.db.iterdump())
        with self.assertRaisesRegex(ValueError, "source-cited canon grounding requires"):
            with self._metadata_operation(owner, expected):
                self.fail("canon-grounded metadata must not yield a publishable candidate")
        self.assertEqual(list(self.db.iterdump()), before)


if __name__ == "__main__":
    unittest.main()
