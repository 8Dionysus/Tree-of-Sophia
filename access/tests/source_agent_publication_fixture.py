"""Shared software-only prepared publication fixture.

The metadata publication checks live under ``access/tests`` so they also run
from the sparse software checkout.  Keep their predecessor entirely inside
this access-owned fixture: source-shaped bytes come from the already reviewed
frozen source-assembly fixture and all executable modules come from the
explicit code root.  This helper is transport setup only; it grants no
rights, canon status, or semantic admission.
"""
from __future__ import annotations

from types import SimpleNamespace
import copy
import hashlib
import json
from pathlib import Path
import sqlite3
import sys
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
for directory in (
    ROOT / "scripts",
    ROOT / "access" / "src",
    ROOT / "access" / "tests",
    ROOT / "mechanics" / "growth-cycle" / "parts" / "branch-growth-cycle" / "scripts",
):
    if str(directory) not in sys.path:
        sys.path.insert(0, str(directory))

import bibliographic_claim_assembler as assembly
import build_source_witness_catalog as legacy
import source_catalog_projection as catalog
import source_agent_publication as publication
import tos_corpus_index_common as navigation
import source_witness_bibliographic_graph_common as bibliography
from source_assembly_fixture import SourceAssemblyFixture
from source_metadata_snapshot import PublicationSnapshot
from tos_access import knowledge as knowledge_graph
from tos_access.catalog_semantics import CANONICAL_ORDER, CatalogInputs, memory_catalog
from tos_access.prepared_publication import publish_prepared
from tos_access.prepared_semantics import bootstrap_prepared_maintenance_transaction
from tos_access.prepared_source_binding import bootstrap_prepared_source_inputs_transaction
from tos_access.prepared_source_dependencies import (
    ProgressHandlerOwner,
    SourceClaimDependencies,
    bootstrap_source_dependency_index_transaction,
)
from tos_access.projection_mutation import ProjectionSnapshotView
from tos_access.projection_store import Collection, canonical_bytes, write_projection


class _FixtureProxy:
    """Small compatibility surface used by the metadata tests."""

    def __init__(self, root, real, rebuild):
        self.root = Path(root)
        self._rebuild = rebuild
        self.record = copy.deepcopy(real[0])
        self.other = copy.deepcopy(real[1])
        self.identity = self.record["record_id"]
        self.claim_ref = "ToS/source-witnesses/history/fixture/historical-claims.jsonl"

    def write(self, ref, raw):
        path = self.root / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return path

    def rebuild(self):
        return self._rebuild()


class SourceAgentPublicationTests(unittest.TestCase):
    """Build the tiny prepared predecessor without root-test imports."""

    def setUp(self):
        source_fixture = SourceAssemblyFixture(
            code_root=ROOT,
            source_root=ROOT / "access" / "tests" / "fixtures" / "source-assembly",
        )
        self._source_fixture = source_fixture
        context = source_fixture.historical_fixture()
        self.root, self.history, self.real, self.claims, self.rebuild = context.__enter__()
        self.addCleanup(context.__exit__, None, None, None)
        # Keep the predecessor's native social Claim plus its hidden-maker
        # companion.  The records themselves are synthetic test inputs; the
        # frozen public metadata only supplies stable source-shaped neighbors.
        captured_claims = self.claims
        captured_claims.clear()

        def synthetic_agent(record_id, label):
            record = copy.deepcopy(self.real[0])
            record.update(
                record_id=record_id,
                record_version=1,
                preferred_label=label,
                identity_status="provisional",
                source_refs=["test:synthetic"],
                external_identifiers=[
                    {
                        "scheme": "synthetic",
                        "value": record_id.rsplit(".", 1)[-1],
                        "source_ref": "test:synthetic",
                        "status": "unverified",
                    }
                ],
                same_as_posture="no_equivalence_claim",
                notes="Synthetic publication fixture, not a historical assertion.",
                supersedes_ref=None,
            )
            record.pop("variant_labels", None)
            return record

        fixture_record = synthetic_agent(
            "tos.agent.friedrich-nietzsche", "Synthetic source agent"
        )
        fixture_other = synthetic_agent(
            "tos.agent.untouched-fixture", "Untouched synthetic Agent"
        )
        self.helper = SimpleNamespace(
            fixture=_FixtureProxy(self.root, self.real, self.rebuild),
        )
        self.helper.fixture.record = fixture_record
        self.helper.fixture.other = fixture_other
        self.helper.fixture.identity = fixture_record["record_id"]
        # Keep the source bytes consumed by the catalog identical to the
        # synthetic record exposed to the publication tests.  The shared
        # source-assembly seed is frozen metadata; this access fixture uses a
        # deliberately sanitized synthetic Agent in its bounded predecessor.
        self.helper.fixture.write(
            "ToS/source-witnesses/agents/friedrich-nietzsche/agent.json",
            canonical_bytes(fixture_record),
        )
        self.helper.fixture.write(
            "ToS/source-witnesses/agents/untouched-fixture/agent.json",
            canonical_bytes(fixture_other),
        )
        # The source-assembly helper carries several independent fixture
        # families.  This predecessor intentionally retains only one history
        # record plus the two Agent endpoints used by its native Claims, so
        # publication closure remains the same bounded cohort as the original
        # Agent publication fixture.
        for ref in (
            "ToS/source-witnesses/places/chemnitz/place.json",
            "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json",
            "ToS/source-witnesses/history/fixture/historical-process.json",
            "ToS/source-witnesses/history/fixture/historical-state.json",
        ):
            (self.root / ref).unlink(missing_ok=True)
        claim = {
            "schema_version": "tos_social_relation_claim_v1",
            "claim_id": "tos.claim.native-assembly-fixture",
            "claim_version": 1,
            "claim_type": "relation",
            "assertion_layer": "scholarly_report",
            "subject_ref": fixture_record["record_id"],
            "predicate": "learned_from",
            "object": fixture_other["record_id"],
            "evidence_refs": ["tos.anchor.slot-fixture"],
            "maker": {"maker_type": "software", "agent_ref": "software:synthetic"},
            "provenance_event_ref": "tos.event.slot-fixture",
            "epistemic_status": "uncertain",
            "review_status": "unreviewed",
            "visibility": "public_metadata_only",
            "qualifiers": {
                "statement": "Synthetic relationship, not a historical assertion.",
                "statement_language": "en",
                "statement_script": "Latn",
                "relation_basis": "Synthetic unit test only.",
                "social_scope": "Fixture-only relationship.",
                "time_scope_note": "No historical dates asserted.",
                "uninterpreted": "tos.agent.not-an-explicit-lookup",
            },
        }
        hidden_claim = {
            **copy.deepcopy(claim),
            "claim_id": "tos.claim.hidden-maker-fixture",
            "subject_ref": fixture_other["record_id"],
            "object": fixture_other["record_id"],
            "maker": {
                "maker_type": "software",
                "agent_ref": fixture_record["record_id"],
            },
        }
        source_claims = [claim, hidden_claim]
        self.claims = source_claims
        native_claim_ref = "ToS/source-witnesses/relations/native-assembly/source-claims.jsonl"
        native_event_ref = "ToS/source-witnesses/relations/slot-fixture/provenance.jsonl"
        native_anchor_ref = "ToS/source-witnesses/relations/slot-fixture/anchors.jsonl"
        event = {
            "schema_version": "tos_provenance_event_v1",
            "event_id": "tos.event.slot-fixture",
            "event_version": 1,
            "event_type": "annotation",
            "started_at": "2026-09-12T00:00:00Z",
            "ended_at": "2026-09-12T00:00:00Z",
            "agent_refs": ["software:synthetic"],
            "method": {"maker_type": "software", "name": "synthetic", "version": "1"},
            "status": "completed_with_warnings",
        }
        anchor = {
            "anchor_id": "tos.anchor.slot-fixture",
            "unknown": {"literal": "tos.agent.not-an-explicit-lookup"},
        }
        original_rebuild = self.rebuild

        def rebuild():
            self.helper.fixture.write(
                native_claim_ref,
                b"".join(canonical_bytes(row) for row in source_claims),
            )
            self.helper.fixture.write(native_event_ref, canonical_bytes(event))
            self.helper.fixture.write(native_anchor_ref, canonical_bytes(anchor))
            return original_rebuild()

        self.rebuild = rebuild
        self.helper.fixture._rebuild = rebuild
        self.helper.fixture.claim_ref = native_claim_ref
        for ref in (
            "ToS/contracts/source-claim-record.schema.json",
            "ToS/contracts/social-relation-claim.schema.json",
            "ToS/contracts/source-member-structure-claim.schema.json",
            "ToS/contracts/source-structured-value.schema.json",
            "ToS/contracts/scoped-member-structure.schema.json",
            "ToS/contracts/human-form.schema.json",
            "ToS/contracts/human-form-set.schema.json",
            "ToS/contracts/human-form-template.schema.json",
        ):
            self.helper.fixture.write(ref, (ROOT / ref).read_bytes())
        self.claim = copy.deepcopy(claim)
        self.hidden_claim = copy.deepcopy(hidden_claim)

        bibliography_payload = self.rebuild()
        scratch = self.root / "source-agent-publication-scratch"
        scratch.mkdir()
        catalog_path = self.root / "derived" / "source-catalog.json"
        catalog_path.parent.mkdir(parents=True, exist_ok=True)
        candidate = catalog.bootstrap_source_catalog(
            self.root,
            catalog_path,
            catalog_namespace="tos.catalog.synthetic.source-agent-publication",
            expected_manifest_sha256=hashlib.sha256(
                (self.root / legacy.MANIFEST_PATH).read_bytes()
            ).hexdigest(),
            expected_publication_token=PublicationSnapshot(self.root).token,
            work_dir=scratch,
            include_claims=True,
            target_part_bytes=512,
        )
        self.snapshot = catalog.SourceCatalogSnapshot(
            candidate.snapshot(),
            expected_root_sha256=candidate.root_sha256,
            trusted_baseline_sha256=candidate.root_sha256,
        )

        with patch.object(navigation, "REPO_ROOT", self.root), patch.object(
            navigation, "TOS_ROOT", self.root / "ToS"
        ):
            corpus = {
                "source_navigation": navigation.build_source_navigation(
                    [], catalog_snapshot=self.snapshot
                )
            }
        self.corpus = corpus
        self.bibliography = bibliography_payload
        self.entities, self.relations = [
            json.loads(
                (self.root / "ToS" / "doctrine" / "semantic-interchange" / name).read_text()
            )
            for name in ("entity-types.v1.json", "relation-types.v1.json")
        ]
        self.graph = knowledge_graph.build_knowledge_graph(
            self.corpus,
            {},
            self.bibliography,
            self.entities,
            self.relations,
        )

        roots = {"source-catalog": self.snapshot.view}
        for role, value, fields in (
            (
                "source-navigation",
                self.corpus["source_navigation"],
                {"nodes": "node_id", "edges": "edge_id"},
            ),
            (
                "bibliographic-claims",
                self.bibliography,
                {"nodes": "node_id", "edges": "edge_id", "claim_traces": "claim_ref"},
            ),
        ):
            path = self.root / "derived" / (role + ".json")
            write_projection(
                path,
                {"schema_version": publication.RAW_SCHEMAS[role]},
                {
                    name: Collection(value[name], key, (key,))
                    for name, key in fields.items()
                },
                work_dir=scratch,
                target_part_bytes=1024,
            )
            roots[role] = ProjectionSnapshotView(path.read_bytes(), path)

        self.profile = publication.declaration_profile_sha256()
        self.graph["source_revision"] = None
        self.source = publication.source_vector_inputs(
            roots=roots,
            dependencies={
                "declaration-profile": self.profile,
                "agent-publication-profile": publication.execution_profile_sha256(),
                "entity-registry": knowledge_graph._stable_digest(self.entities),
                "relation-registry": knowledge_graph._stable_digest(self.relations),
                "normalization": knowledge_graph._stable_digest(
                    self.graph["normalization_binding"]
                ),
                "nonparticipating-profile": knowledge_graph._stable_digest(
                    {"philosophy": {}, "corpus_without_navigation": {}}
                ),
            },
            source_publication=self.snapshot.header["source_publication"]["token"],
        )
        self.graph["source_revision"] = self.source.value()["source_revision"]
        self.catalog = memory_catalog(
            self.graph,
            self.corpus,
            {},
            self.entities,
            self.relations,
        )
        self.inputs = CatalogInputs.from_graph(
            self.graph,
            self.corpus,
            {},
            self.entities,
            self.relations,
            source_order_profile=CANONICAL_ORDER,
        )
        self.path = self.root / "derived" / "prepared.sqlite"
        self.binding = publish_prepared(
            self.path,
            graph=self.graph,
            catalog=self.catalog,
        )
        self.db = sqlite3.connect(self.path)
        self.addCleanup(self.db.close)
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("BEGIN IMMEDIATE")
        self.owner = ProgressHandlerOwner()
        bootstrap_prepared_maintenance_transaction(
            self.db,
            expected_binding=self.binding,
            inputs=self.inputs,
            ordered_rows=lambda kind: iter(self.graph[kind + "s"]),
        )
        bootstrap_prepared_source_inputs_transaction(
            self.db,
            expected_binding=self.binding,
            inputs=self.source,
        )
        reader = assembly.BibliographicClaimAssembler(
            self.root,
            catalog_snapshot=self.snapshot,
        )
        declarations = []
        for _, row in self.snapshot.view.iter_items("claims"):
            selected = self.snapshot.get_claim(row["claim_id"])
            assembled = reader.assemble(
                row["claim_id"],
                expected_row_sha256=selected.row_sha256,
            )
            declarations.append(
                SourceClaimDependencies(
                    claim_id=row["claim_id"],
                    source_entry=selected.entry,
                    input_sha256=selected.entry["claim_sha256"],
                    dependencies=assembled.dependencies,
                )
            )
        bootstrap_source_dependency_index_transaction(
            self.db,
            expected_binding=self.binding,
            source_inputs_sha256=self.source.digest,
            declaration_profile_sha256=self.profile,
            claims=declarations,
            progress_owner=self.owner,
        )
        publication.bootstrap_agent_context_index_transaction(
            self.db,
            source_root=self.root,
            expected_binding=self.binding,
            source_inputs=self.source,
            catalog_inputs=self.inputs,
            declaration_profile_sha256=self.profile,
            ordered_nodes=self.graph["nodes"],
            ordered_relations=self.graph["relations"],
            source_dossier_refs=sorted(
                {
                    node["source_dossier_ref"]
                    for node in self.graph["nodes"]
                    if node.get("source_dossier_ref") is not None
                }
            ),
            progress_owner=self.owner,
        )
        self.db.commit()


__all__ = ["SourceAgentPublicationTests"]
