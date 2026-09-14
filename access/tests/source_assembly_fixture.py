"""Shared bounded source-assembly fixture for root and access tests."""
from __future__ import annotations

import json
import sys
import tempfile
from contextlib import contextmanager
from pathlib import Path


class SourceAssemblyFixture:
    """Build the synthetic historical cohort from explicit code and source roots."""

    def __init__(self, *, code_root: Path, source_root: Path) -> None:
        self.code_root = Path(code_root)
        self.source_root = Path(source_root)

    @contextmanager
    def historical_fixture(self):
        """Synthetic history associations to unchanged real bibliographic identities.

        No fixture event or association is historical evidence or admission.
        """
        scripts = self.code_root / "scripts"
        if str(scripts) not in sys.path:
            sys.path.insert(0, str(scripts))
        from build_source_witness_catalog import render_outputs, write_outputs
        from source_witness_bibliographic_graph_common import build_payload

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def write(ref, payload):
                path = root / ref
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
                return path

            for ref in (
                "ToS/contracts/corpus-record.schema.json",
                "ToS/contracts/claim-packet.schema.json",
                "ToS/contracts/semantic-entity-type-registry.schema.json",
                "ToS/contracts/semantic-relation-type-registry.schema.json",
                "ToS/contracts/source-witness-bibliographic-graph.schema.json",
                "ToS/contracts/source-witness-catalog.schema.json",
                "ToS/contracts/historical-record.schema.json",
                "ToS/contracts/historical-claim.schema.json",
                "ToS/contracts/knowledge-assessment.schema.json",
                "ToS/doctrine/semantic-interchange/entity-types.v1.json",
                "ToS/doctrine/semantic-interchange/relation-types.v1.json",
            ):
                write(ref, json.loads((self.code_root / ref).read_text()))
            real_refs = (
                "ToS/source-witnesses/agents/friedrich-nietzsche/agent.json",
                "ToS/source-witnesses/places/chemnitz/place.json",
                "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json",
            )
            real = [json.loads((self.source_root / ref).read_text()) for ref in real_refs]
            for ref, payload in zip(real_refs, real):
                write(ref, payload)
            history = []
            for kind, name in (
                ("historical-event", "Условный эпизод"),
                ("historical-process", "Условный процесс"),
                ("historical-state", "Условное состояние"),
            ):
                payload = {
                    "schema_version": "tos_historical_record_v1",
                    "record_type": kind,
                    "record_id": f"tos.{kind}.fixture",
                    "record_version": 1,
                    "preferred_label": name,
                    "variant_labels": [],
                    "identity_status": "provisional",
                    "source_refs": [real_refs[2]],
                    "external_identifiers": [],
                    "same_as_posture": "no_equivalence_claim",
                    "visibility": "public_metadata_only",
                    "notes": "Синтетический тест. Историческое существование не утверждается.",
                }
                history.append((write(f"ToS/source-witnesses/history/fixture/{kind}.json", payload), payload))
            event_id = "tos.event.historical-fixture-capture"
            write(
                "ToS/source-witnesses/history/fixture/provenance.jsonl",
                {
                    "schema_version": "tos_provenance_event_v1",
                    "event_id": event_id,
                    "event_type": "annotation",
                    "started_at": "2026-09-06T00:00:00Z",
                    "ended_at": "2026-09-06T00:00:00Z",
                    "agent_refs": ["software:test-fixture"],
                    "inputs": [],
                    "outputs": [],
                    "method": {"maker_type": "software", "name": "synthetic-test", "version": "1"},
                    "status": "completed_with_warnings",
                    "event_version": 1,
                },
            )
            claims = []
            for index, (predicate, target) in enumerate(
                zip(("historical_participant", "historical_place", "historical_work"), real)
            ):
                claims.append(
                    {
                        "schema_version": "tos_historical_claim_v1",
                        "claim_id": f"tos.claim.historical-fixture-{index}",
                        "claim_version": 1,
                        "claim_type": "relation",
                        "assertion_layer": "scholarly_report",
                        "subject_ref": history[0][1]["record_id"],
                        "predicate": predicate,
                        "object": target["record_id"],
                        "evidence_refs": [real_refs[index]],
                        "maker": {"maker_type": "software", "agent_ref": "software:test-fixture"},
                        "provenance_event_ref": event_id,
                        "epistemic_status": "uncertain",
                        "review_status": "unreviewed",
                        "visibility": "public_metadata_only",
                        "qualifiers": {
                            "participation_role": "test-participant",
                            "negated": True,
                            "scope": "synthetic-only",
                            "x-unknown": False,
                        },
                    }
                )
            claim_path = root / "ToS/source-witnesses/history/fixture/historical-claims.jsonl"

            def rebuild():
                claim_path.write_text(
                    "".join(json.dumps(claim, ensure_ascii=False) + "\n" for claim in claims)
                )
                write_outputs(root, render_outputs(root))
                return build_payload(root)

            yield root, history, real, claims, rebuild

    def historical_knowledge(self, root: Path, projection):
        access_src = self.code_root / "access/src"
        if str(access_src) not in sys.path:
            sys.path.insert(0, str(access_src))
        from tos_access.knowledge import build_knowledge_graph

        entities, relations = [
            json.loads((root / "ToS/doctrine/semantic-interchange" / name).read_text())
            for name in ("entity-types.v1.json", "relation-types.v1.json")
        ]
        return build_knowledge_graph({}, {}, projection, entities, relations), entities, relations
