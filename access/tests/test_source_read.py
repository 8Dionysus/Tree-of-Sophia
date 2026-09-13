from __future__ import annotations

import asyncio
import copy
import hashlib
import http.client
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import threading

import pytest
from jsonschema import Draft202012Validator

ACCESS = Path(__file__).resolve().parents[1]
REPO = ACCESS.parent
sys.path.insert(0, str(ACCESS / "src"))
sys.path.insert(0, str(REPO / "scripts"))

from tos_access.core import ToSAccessCore  # noqa: E402
from tos_access.http_server import make_server  # noqa: E402
from tos_access.source_read import (  # noqa: E402
    PUBLICATION_PROTOCOL,
    SourceEpoch,
    SourceReadBudgetExceeded,
    SourceReadError,
    SourceReadLimits,
    SourceReadService,
    SourceOwnerBinding,
    SourceCatalogClaimReader,
    SourceCatalogTargetIssuer,
    _canonical_digest,
    validate_handle,
)


def _record(record_type: str, identifier: str, *, visibility: str = "public_metadata_only", notes: str = "exact") -> dict:
    return {
        "schema_version": "tos_lexical_description_record_v1" if record_type in {"lexeme", "sense"} else "tos_corpus_record_v1",
        "record_type": record_type,
        "record_id": identifier,
        "record_version": 1,
        "preferred_label": identifier,
        "visibility": visibility,
        "notes": notes,
    }


def _ref(record: dict) -> dict:
    return {
        "id": record["record_id"],
        "version": record["record_version"],
        "digest": "sha256:" + _canonical_digest(record),
    }


def _claim(identifier: str = "tos.claim.fixture.exact", *, visibility: str = "public_metadata_only") -> dict:
    return {
        "schema_version": "tos_claim_packet_v1",
        "claim_id": identifier,
        "claim_type": "relation",
        "claim_version": 1,
        "subject_ref": "tos.agent.fixture",
        "predicate": "authored_by",
        "object": "tos.work.fixture",
        "visibility": visibility,
        "statement": "an exact source Claim",
    }


def _epoch(**overrides: object) -> SourceEpoch:
    value = {
        "source_revision": "a" * 64,
        "catalog_root_sha256": "b" * 64,
        "catalog_namespace": "tos.catalog.fixture",
        "source_publication": {
            "protocol": PUBLICATION_PROTOCOL,
            "token": "sha256:" + "c" * 64,
            "generation": 3,
        },
    }
    value.update(overrides)
    return SourceEpoch.from_value(value)


class _MetadataOwner:
    def __init__(self, records: dict[str, dict], *, restricted: set[str] | None = None):
        self.records = records
        self.restricted = restricted or set()
        self.calls: list[dict] = []

    def source_read_binding(self) -> dict:
        return _epoch().value()

    def verify_current(self) -> None:
        return None

    def resolve_typed(self, exact_ref: dict) -> dict:
        self.calls.append(copy.deepcopy(exact_ref))
        record = self.records.get(exact_ref["id"])
        if record is None:
            return {
                "status": "missing",
                "reason": "exact-version-not-retained",
                "exact_ref": copy.deepcopy(exact_ref),
                "record": None,
                "record_digest": None,
                "provenance": None,
            }
        if record["record_id"] in self.restricted:
            return {
                "status": "access-restricted",
                "reason": "metadata-record-not-public",
                "exact_ref": copy.deepcopy(exact_ref),
                "record": None,
                "record_digest": None,
                "provenance": None,
            }
        if _ref(record) != exact_ref:
            return {
                "status": "stale",
                "reason": "exact-version-digest-mismatch",
                "exact_ref": copy.deepcopy(exact_ref),
                "record": None,
                "record_digest": None,
                "provenance": None,
            }
        return {
            "status": "available",
            "reason": "exact-current-version",
            "exact_ref": copy.deepcopy(exact_ref),
            "record": copy.deepcopy(record),
            "record_digest": exact_ref["digest"],
            "provenance": {
                "catalog": {"record_key": record["record_id"], "row_sha256": "d" * 64},
                "source": {"source_ref": "ToS/source-witnesses/fixture/record.json"},
            },
            "descriptor": {
                "adapter": "declared-profile" if record["record_type"] in {"lexeme", "sense"} else "native-corpus",
                "record_type": record["record_type"],
                "source_scope": "public_metadata_only",
            },
        }


class _ClaimOwner:
    def __init__(self, record: dict):
        self.record = record
        self.calls: list[dict] = []

    def source_read_binding(self) -> dict:
        return _epoch().value()

    def verify_current(self) -> None:
        return None

    def resolve(self, exact_ref: dict) -> dict:
        self.calls.append(copy.deepcopy(exact_ref))
        expected_ref = {
            "id": self.record["claim_id"],
            "version": self.record["claim_version"],
            "digest": "sha256:" + _canonical_digest(self.record),
        }
        if expected_ref != exact_ref:
            return {"status": "stale", "reason": "exact-version-digest-mismatch", "exact_ref": copy.deepcopy(exact_ref), "record": None, "provenance": None}
        return {
            "status": "available",
            "reason": "exact-current-version",
            "exact_ref": copy.deepcopy(exact_ref),
            "record": copy.deepcopy(self.record),
            "record_digest": exact_ref["digest"],
            "provenance": {"source": {"source_ref": "ToS/source-witnesses/relations/fixture.jsonl"}},
        }


class _Slot:
    def __init__(self, kind: str, identity: str, payload: dict, row_sha256: str):
        self.kind, self.identity, self.payload = kind, identity, payload
        self.raw_bytes = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
        self.row_sha256 = row_sha256
        self.provenance = {"source": {"source_ref": "ToS/source-witnesses/relations/fixture.jsonl"}, "row_sha256": row_sha256}


class _SlotOwner:
    def __init__(self, slot: _Slot):
        self.slot = slot
        self.calls: list[tuple[str, str, str]] = []

    def source_read_binding(self) -> dict:
        return _epoch().value()

    def verify_current(self) -> None:
        return None

    def describe(self, kind: str, identity: str) -> dict:
        return {
            "row_sha256": self.slot.row_sha256,
            "canonical_sha256": _canonical_digest(self.slot.payload),
            "visibility": self.slot.payload.get("visibility", "public_metadata_only"),
            "provenance": self.slot.provenance,
        }

    def read_slot(self, kind: str, identity: str, *, expected_row_sha256: str):
        self.calls.append((kind, identity, expected_row_sha256))
        if (kind, identity, expected_row_sha256) != (self.slot.kind, self.slot.identity, self.slot.row_sha256):
            raise FileNotFoundError("slot not selected")
        return self.slot


def _metadata_target(record: dict) -> dict:
    reference = _ref(record)
    return {
        "layer": "metadata_record",
        "record_type": record["record_type"],
        "record_ref": reference,
        "content_revision": reference["digest"],
    }


def _claim_target(record: dict) -> dict:
    reference = {
        "id": record["claim_id"],
        "version": record["claim_version"],
        "digest": "sha256:" + _canonical_digest(record),
    }
    return {"layer": "claim_record", "record_ref": reference, "content_revision": reference["digest"]}


def _slot_target(slot: _Slot) -> dict:
    return {
        "layer": "source_slot",
        "slot_kind": slot.kind,
        "identity": slot.identity,
        "row_sha256": slot.row_sha256,
        "content_revision": "sha256:" + _canonical_digest(slot.payload),
    }


def _schema() -> dict:
    return json.loads((ACCESS / "contracts/source-read.v1.schema.json").read_text(encoding="utf-8"))


def _ready_publication(root: Path) -> str:
    """Install one tiny ready publication in a temporary owner fixture."""
    control = root / "ToS/source-witnesses/.metadata-publication.json"
    control.parent.mkdir(parents=True, exist_ok=True)
    body = {
        "schema_version": "tos_source_metadata_publication_v1",
        "generation": 1,
        "transition_id": "0" * 32,
        "phase": "ready",
        "transaction_id": "sha256:" + "1" * 64,
        "manifest_sha256": "sha256:" + "2" * 64,
        "outcome": "committed",
        "recovery_authorization": None,
    }
    token = "sha256:" + hashlib.sha256(
        json.dumps(body, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    control.write_text(
        json.dumps({**body, "token": token}, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
        encoding="utf-8",
    )
    return token


def test_owner_handles_cover_agent_and_registry_declared_lexical_metadata() -> None:
    agent = _record("agent", "tos.agent.fixture")
    lexeme = _record("lexeme", "tos.lexeme.fixture")
    owner = _MetadataOwner({agent["record_id"]: agent, lexeme["record_id"]: lexeme})
    binding = SourceOwnerBinding.from_owner_readers(metadata_reader=owner)
    service = SourceReadService(binding, metadata_record_types={"agent", "lexeme"})
    validator = Draft202012Validator(_schema())

    for record in (agent, lexeme):
        discovered = service.discover({"target": _metadata_target(record)})
        assert discovered["status"] == "available", discovered
        assert discovered["handle"]
        validator.validate(discovered["handle"])
        assert "source_ref" not in discovered["handle"]
        assert "path" not in discovered["handle"]
        read = service.read({"handle": discovered["handle"], "representation": "record"})
        assert read["status"] == "available", read
        assert read["record"] == record
        assert read["content_revision"] == _ref(record)["digest"]
        validator.validate(read)

    # A registry-declared kind must be explicitly included by the owner
    # binding; this adapter does not broaden the route from an ID prefix.
    sense = _record("sense", "tos.sense.fixture")
    unsupported = service.discover({"target": _metadata_target(sense)})
    assert unsupported["status"] == "unsupported"
    assert unsupported["handle"] is None
    assert unsupported["reason"] == "metadata-type-not-owner-declared"


@pytest.mark.parametrize("schema,kind,field", [
    ("tos_artifact_source_witness_v1", "artifact", "artifact_id"),
    ("tos_artifact_source_witness_v2", "artifact", "artifact_id"),
    ("tos_scholarly_composite_witness_v1", "composite", "composite_id"),
])
def test_native_witness_read_requires_matching_owner_identity_descriptor(schema, kind, field):
    from tos_access.source_read import exact_target_from_record
    record = {"schema_version": schema, field: "tos." + kind + ".fixture", "record_version": 1,
              "visibility": "public_metadata_only", "unknown": {"preserved": True}}
    target = exact_target_from_record(record, layer="metadata_record")

    class NativeOwner:
        descriptor = {"adapter": "native-witness", "record_type": kind, "identity_field": field}
        def source_read_binding(self):
            return _epoch().value()
        def verify_current(self):
            pass
        def resolve_typed(self, reference):
            assert reference == target["record_ref"]
            return {"status": "available", "reason": "exact-current-version", "exact_ref": reference,
                    "record": copy.deepcopy(record), "descriptor": self.descriptor,
                    "provenance": {"source": {"source_ref": "ToS/source-witnesses/fixture/native.json"}}}

    owner = NativeOwner()
    service = SourceReadService(SourceOwnerBinding.from_owner_readers(metadata_reader=owner), metadata_record_types={kind})
    discovered = service.discover({"target": target})
    assert discovered["status"] == "available", discovered
    read = service.read({"handle": discovered["handle"], "representation": "record"})
    assert read["status"] == "available" and read["record"] == record
    assert read["access"]["rights_revalidated"] is False
    for descriptor in ({**owner.descriptor, "identity_field": "record_id"}, {**owner.descriptor, "adapter": "declared-profile"}):
        owner.descriptor = descriptor
        denied = service.discover({"target": target})
        assert denied["status"] != "available" and denied["handle"] is None


def _source_vector_for_catalog(view, token):
    """Owner-hashed complete tiny vector, not an arbitrary revision string."""
    from tos_access.projection_mutation import ProjectionSnapshotView
    from tos_access.projection_store import Collection, write_projection
    import source_agent_publication as publication

    roots = {"source-catalog": view}
    scratch = view.namespace_path.parent / "read-test-scratch"
    scratch.mkdir(exist_ok=True)
    for role, schema in publication.RAW_SCHEMAS.items():
        path = view.namespace_path.parent / (role + "-read-test.json")
        write_projection(path, {"schema_version": schema},
                         {"nodes": Collection([], "node_id", ("node_id",))},
                         work_dir=scratch)
        roots[role] = ProjectionSnapshotView(path.read_bytes(), path)
    return publication.source_vector_inputs(
        roots=roots, dependencies={}, source_publication=token
    )


def test_prepared_source_factory_binds_a_real_catalog_and_metadata_reader() -> None:
    fixture_spec = importlib.util.spec_from_file_location(
        "source_catalog_fixture", REPO / "tests/test_source_catalog_projection.py"
    )
    assert fixture_spec and fixture_spec.loader
    fixture_module = importlib.util.module_from_spec(fixture_spec)
    fixture_spec.loader.exec_module(fixture_module)
    fixture = fixture_module.SourceCatalogProjectionTests()
    fixture.setUp()
    try:
        token = _ready_publication(fixture.root)
        fixture.rebuild()
        candidate = fixture.bootstrap()
        catalog = importlib.import_module("source_catalog_projection")
        metadata = importlib.import_module("metadata_version_reader")
        snapshot = catalog.SourceCatalogSnapshot(
            candidate.snapshot(), expected_root_sha256=candidate.root_sha256,
            trusted_baseline_sha256=candidate.root_sha256,
        )
        from tos_access.prepared_source_binding import PreparedSourceInputs

        inputs = _source_vector_for_catalog(candidate.snapshot(), token)
        reader = metadata.MetadataVersionReader(fixture.root, catalog_snapshot=snapshot)
        for wrong in (
            PreparedSourceInputs(source_revision="a" * 64, source_publication=token,
                                 dependencies={}, roots=inputs.roots()),
            PreparedSourceInputs(source_revision=inputs.value()["source_revision"],
                                 source_publication=token, dependencies={},
                                 roots={"source-catalog": candidate.snapshot()}),
        ):
            with pytest.raises(SourceReadError, match="vector could not be verified"):
                SourceOwnerBinding.from_prepared_source(
                    wrong, catalog_snapshot=snapshot, metadata_reader=reader
                )
        record = fixture.record
        binding = SourceOwnerBinding.from_prepared_source(
            inputs, catalog_snapshot=snapshot, metadata_reader=reader
        )
        issuer = SourceCatalogTargetIssuer(snapshot)
        service = SourceReadService(
            binding, metadata_record_types={record["record_type"]}, target_issuer=issuer
        )
        discovered = service.discover({"selector": {
            "layer": "metadata_record",
            "record_type": record["record_type"],
            "record_id": record["record_id"],
        }})
        assert discovered["status"] == "available", discovered
        result = service.read({"handle": discovered["handle"], "representation": "record"})
        assert result["status"] == "available", result
        assert result["record"] == record
        assert result["source_revision"] == inputs.value()["source_revision"]
        missing = service.discover({"selector": {
            "layer": "metadata_record", "record_type": record["record_type"],
            "record_id": "tos.agent.not-in-this-catalog",
        }})
        assert missing["status"] == "missing", missing
        assert missing["target"] is None and missing["handle"] is None
        assert missing["content_revision"] is None
        Draft202012Validator(_schema()).validate(missing)
        for forbidden in ("digest", "path", "latest"):
            selector = {
                "layer": "metadata_record",
                "record_type": record["record_type"],
                "record_id": record["record_id"],
                forbidden: "not-an-owner-field",
            }
            with pytest.raises(SourceReadError):
                service.discover({"selector": selector})
    finally:
        fixture.doCleanups()


def test_claim_and_current_source_slot_handles_preserve_exact_content_and_epoch() -> None:
    claim = _claim()
    claim_owner = _ClaimOwner(claim)
    slot = _Slot("claim", claim["claim_id"], claim, "e" * 64)
    slot_owner = _SlotOwner(slot)
    binding = SourceOwnerBinding.from_owner_readers(claim_reader=claim_owner, slot_reader=slot_owner)
    service = SourceReadService(
        binding,
        slot_descriptor=slot_owner.describe,
    )
    for target in (_claim_target(claim), _slot_target(slot)):
        discovered = service.discover({"target": target})
        assert discovered["status"] == "available", discovered
        read = service.read({"handle": discovered["handle"], "representation": "record"})
        assert read["status"] == "available", read
        assert read["record"] == claim
        assert read["source_revision"] == "a" * 64
        assert read["record_kind"] in {"claim", "source_slot"}
    assert slot_owner.calls == [("claim", claim["claim_id"], "e" * 64)] * 2
    assert claim_owner.calls == [_claim_target(claim)["record_ref"]] * 2


def test_prepared_source_factory_binds_a_real_current_claim_slot_reader() -> None:
    fixture_spec = importlib.util.spec_from_file_location(
        "source_slot_fixture", REPO / "tests/test_source_catalog_slots.py"
    )
    assert fixture_spec and fixture_spec.loader
    fixture_module = importlib.util.module_from_spec(fixture_spec)
    fixture_spec.loader.exec_module(fixture_module)
    fixture = fixture_module.SourceCatalogSlotTests()
    fixture.setUp()
    try:
        token = _ready_publication(fixture.root)
        fixture.fixture.rebuild()
        candidate = fixture.bootstrap()
        catalog = importlib.import_module("source_catalog_projection")
        snapshot = catalog.SourceCatalogSnapshot(
            candidate.snapshot(), expected_root_sha256=candidate.root_sha256,
            trusted_baseline_sha256=candidate.root_sha256,
        )
        from tos_access.prepared_source_binding import PreparedSourceInputs

        inputs = _source_vector_for_catalog(candidate.snapshot(), token)
        reader = catalog.SourceCatalogSourceReader(fixture.root, catalog_snapshot=snapshot)
        claim_reader = SourceCatalogClaimReader(reader)
        row = snapshot.get_slot("claim", fixture.claim["claim_id"])

        def descriptor(kind: str, identity: str) -> dict:
            selected = reader.read_slot(kind, identity, expected_row_sha256=row.row_sha256)
            return {
                "row_sha256": selected.slot.row_sha256,
                "canonical_sha256": selected.slot.source["canonical_sha256"],
                "visibility": selected.payload.get("visibility"),
                "provenance": selected.provenance,
            }

        binding = SourceOwnerBinding.from_prepared_source(
            inputs, catalog_snapshot=snapshot, claim_reader=claim_reader, slot_reader=reader
        )
        issuer = SourceCatalogTargetIssuer(snapshot)
        service = SourceReadService(
            binding, claim_reader=claim_reader, slot_reader=reader,
            target_issuer=issuer, slot_descriptor=descriptor,
        )
        target = {
            "layer": "source_slot",
            "slot_kind": "claim",
            "identity": fixture.claim["claim_id"],
            "row_sha256": row.row_sha256,
            "content_revision": "sha256:" + row.source["canonical_sha256"],
        }
        discovered = service.discover({"target": target})
        assert discovered["status"] == "available", discovered
        result = service.read({"handle": discovered["handle"], "representation": "record"})
        assert result["status"] == "available", result
        assert result["record"] == fixture.claim
        assert reader.accounting["read_slots"] >= 2

        discovered_claim = service.discover({"selector": {
            "layer": "claim_record", "claim_id": fixture.claim["claim_id"]
        }})
        assert discovered_claim["status"] == "available", discovered_claim
        claim_result = service.read({
            "handle": discovered_claim["handle"], "representation": "record"
        })
        assert claim_result["status"] == "available", claim_result
        assert claim_result["record"] == fixture.claim
    finally:
        fixture.doCleanups()


def test_prepared_source_combines_real_metadata_and_current_claim_readers() -> None:
    fixture_spec = importlib.util.spec_from_file_location(
        "source_slot_combo_fixture", REPO / "tests/test_source_catalog_slots.py"
    )
    assert fixture_spec and fixture_spec.loader
    fixture_module = importlib.util.module_from_spec(fixture_spec)
    fixture_spec.loader.exec_module(fixture_module)
    fixture = fixture_module.SourceCatalogSlotTests()
    fixture.setUp()
    try:
        token = _ready_publication(fixture.root)
        fixture.fixture.rebuild()
        candidate = fixture.bootstrap()
        catalog = importlib.import_module("source_catalog_projection")
        metadata = importlib.import_module("metadata_version_reader")
        snapshot = fixture.snapshot(candidate)
        from tos_access.prepared_source_binding import PreparedSourceInputs

        inputs = _source_vector_for_catalog(candidate.snapshot(), token)
        metadata_reader = metadata.MetadataVersionReader(fixture.root, catalog_snapshot=snapshot)
        source_reader = catalog.SourceCatalogSourceReader(fixture.root, catalog_snapshot=snapshot)
        claim_reader = SourceCatalogClaimReader(source_reader)
        binding = SourceOwnerBinding.from_prepared_source(
            inputs, catalog_snapshot=snapshot,
            metadata_reader=metadata_reader, claim_reader=claim_reader,
        )
        service = SourceReadService(
            binding,
            target_issuer=SourceCatalogTargetIssuer(snapshot),
            metadata_record_types={fixture.fixture.record["record_type"]},
        )

        metadata_discovered = service.discover({"selector": {
            "layer": "metadata_record",
            "record_type": fixture.fixture.record["record_type"],
            "record_id": fixture.fixture.record["record_id"],
        }})
        assert metadata_discovered["status"] == "available", metadata_discovered
        metadata_read = service.read({
            "handle": metadata_discovered["handle"], "representation": "record",
        })
        assert metadata_read["status"] == "available", metadata_read
        assert metadata_read["record"] == fixture.fixture.record

        claim_discovered = service.discover({"selector": {
            "layer": "claim_record", "claim_id": fixture.claim["claim_id"],
        }})
        assert claim_discovered["status"] == "available", claim_discovered
        claim_read = service.read({
            "handle": claim_discovered["handle"], "representation": "record",
        })
        assert claim_read["status"] == "available", claim_read
        assert claim_read["record"] == fixture.claim

        control = fixture.root / "ToS/source-witnesses/.metadata-publication.json"
        original = control.read_bytes()
        try:
            drifted = json.loads(original)
            drifted["generation"] = 2
            control.write_text(json.dumps(drifted, sort_keys=True), encoding="utf-8")
            with pytest.raises(SourceReadError, match="currentness verification failed"):
                service.discover({"selector": {
                    "layer": "metadata_record",
                    "record_type": fixture.fixture.record["record_type"],
                    "record_id": fixture.fixture.record["record_id"],
                }})
        finally:
            control.write_bytes(original)
    finally:
        fixture.doCleanups()


def test_real_preflight_owner_selector_read_has_http_mcp_core_parity() -> None:
    """Read one real ToS Agent and Claim through every selected consumer seam.

    This is an opt-in acceptance check because the preflight catalog and its
    retained parts are host artifacts, not repository test fixtures.  The
    retained complete vector and executing owner profile must match the
    snapshot. No historical execution-root override substitutes for live
    currentness. An explicitly supplied unusable receipt is a failure. The
    retained-input route checks source reading before SQLite finishes; it
    does not claim that the independently running bootstrap completed.
    """
    result_ref = os.environ.get("TOS_REAL_PREFLIGHT_RESULT")
    inputs_ref = os.environ.get("TOS_REAL_SOURCE_INPUTS")
    if not result_ref and not inputs_ref:
        pytest.skip("select a real completed receipt or an exact retained source vector")
    if inputs_ref:
        assert not result_ref, "select only one real source input route"
        owner_root = os.environ.get("TOS_REAL_SOURCE_ROOT")
        assert owner_root, "retained inputs require an explicit source owner root"
        source_root, inputs_path = Path(owner_root), Path(inputs_ref)
        expected_revision = None
    else:
        result_path = Path(result_ref)
        assert result_path.is_file(), f"real preflight receipt is unavailable: {result_path}"
        result = json.loads(result_path.read_text(encoding="utf-8"))
        assert result.get("status") == "completed", f"real preflight is not complete: {result.get('status')!r}"
        source_root = Path(result["source_root"])
        inputs_path = Path(result["output_dir"]) / "inputs.raw"
        expected_revision = result["source_revision"]
    assert source_root.is_dir(), "real source root is unavailable"
    assert inputs_path.is_file(), "the complete retained source vector is required"
    from tos_access.prepared_source_binding import PreparedSourceInputs

    inputs = PreparedSourceInputs.parse(inputs_path.read_bytes())
    source_revision = inputs.value()["source_revision"]
    if expected_revision is not None:
        assert source_revision == expected_revision
    catalog_module = importlib.import_module("source_catalog_projection")
    metadata_module = importlib.import_module("metadata_version_reader")
    view = inputs.roots()["source-catalog"]
    root_sha256 = view.snapshot_digest
    snapshot = catalog_module.SourceCatalogSnapshot(
        view, expected_root_sha256=root_sha256, trusted_baseline_sha256=root_sha256
    )
    header = snapshot.header
    assert header["catalog_namespace"] == "tos.agent.full.bootstrap.20260913"
    assert header["record_count"] == 311
    assert header["claim_count"] == 373
    assert header["source_slot_count"] == 2323
    assert header["source_reference_closure_verified"] is False
    metadata_reader = metadata_module.MetadataVersionReader(
        source_root, catalog_snapshot=snapshot
    )
    source_reader = catalog_module.SourceCatalogSourceReader(
        source_root, catalog_snapshot=snapshot
    )
    claim_reader = SourceCatalogClaimReader(source_reader)
    binding = SourceOwnerBinding.from_prepared_source(
        inputs,
        catalog_snapshot=snapshot,
        metadata_reader=metadata_reader,
        claim_reader=claim_reader,
    )
    issuer = SourceCatalogTargetIssuer(snapshot)
    service = SourceReadService(
        binding,
        target_issuer=issuer,
        metadata_record_types={"agent"},
    )

    agent_selector = {
        "layer": "metadata_record",
        "record_type": "agent",
        "record_id": "tos.agent.friedrich-nietzsche",
    }
    claim_selector = {
        "layer": "claim_record",
        "claim_id": "tos.claim.basel-print.environment-place",
    }
    agent_row = snapshot.get("tos.agent.friedrich-nietzsche")
    claim_row = snapshot.get_claim("tos.claim.basel-print.environment-place")
    assert agent_row.source["record_ref"]["version"] == 2
    assert claim_row.claim_ref["version"] == 1

    core = ToSAccessCore.discover(
        tos_root=source_root, source_read_service=service
    )

    def read_from_core(selector: dict) -> tuple[dict, dict]:
        discovered = core.source_handle_discover({"selector": selector})
        assert discovered["status"] == "available", discovered
        read = core.source_read({
            "handle": discovered["handle"], "representation": "record"
        })
        assert read["status"] == "available", read
        return discovered, read

    agent_core, agent_core_read = read_from_core(agent_selector)
    claim_core, claim_core_read = read_from_core(claim_selector)
    missing_selector = {"layer": "claim_record", "claim_id": "tos.claim.not-in-this-catalog"}
    missing_core = core.source_handle_discover({"selector": missing_selector})
    assert missing_core["status"] == "missing" and missing_core["handle"] is None
    Draft202012Validator(_schema()).validate(missing_core)
    assert agent_core_read["record"]["record_id"] == agent_selector["record_id"]
    assert claim_core_read["record"]["claim_id"] == claim_selector["claim_id"]

    server = make_server(core, port=0)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        def http_call(path: str, payload: dict) -> dict:
            body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
            connection = http.client.HTTPConnection(
                "127.0.0.1", server.server_port, timeout=10
            )
            try:
                connection.request(
                    "POST", path, body,
                    {"Content-Type": "application/json"},
                )
                response = connection.getresponse()
                packet = json.loads(response.read())
                assert response.status == 200, packet
                return packet
            finally:
                connection.close()

        missing_http = http_call("/api/source/handles", {"selector": missing_selector})
        assert missing_http == missing_core
        agent_http = http_call(
            "/api/source/handles", {"selector": agent_selector}
        )
        agent_http_read = http_call(
            "/api/source/read",
            {"handle": agent_http["handle"], "representation": "record"},
        )
        claim_http = http_call(
            "/api/source/handles", {"selector": claim_selector}
        )
        claim_http_read = http_call(
            "/api/source/read",
            {"handle": claim_http["handle"], "representation": "record"},
        )
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)

    assert agent_http["handle"] == agent_core["handle"]
    assert agent_http_read["record"] == agent_core_read["record"]
    assert claim_http["handle"] == claim_core["handle"]
    assert claim_http_read["record"] == claim_core_read["record"]

    async def mcp_reads() -> tuple[tuple, tuple, list[str]]:
        from tos_access.mcp_server import build_server

        mcp = build_server(core=core)
        tools = await mcp.list_tools()
        names = [tool.name for tool in tools]
        missing_mcp = await mcp.call_tool("tos_source_handle_discover", {"selector": missing_selector})
        assert missing_mcp[1] == missing_core
        discovered_agent = await mcp.call_tool(
            "tos_source_handle_discover", {"selector": agent_selector}
        )
        discovered_claim = await mcp.call_tool(
            "tos_source_handle_discover", {"selector": claim_selector}
        )
        read_agent = await mcp.call_tool(
            "tos_source_read",
            {"handle": discovered_agent[1]["handle"], "representation": "record"},
        )
        read_claim = await mcp.call_tool(
            "tos_source_read",
            {"handle": discovered_claim[1]["handle"], "representation": "record"},
        )
        return (discovered_agent, read_agent), (discovered_claim, read_claim), names

    (agent_mcp, agent_mcp_read), (claim_mcp, claim_mcp_read), mcp_names = asyncio.run(mcp_reads())
    assert {"tos_source_handle_discover", "tos_source_read"} <= set(mcp_names)
    assert agent_mcp[1]["handle"] == agent_core["handle"]
    assert agent_mcp_read[1]["record"] == agent_core_read["record"]
    assert claim_mcp[1]["handle"] == claim_core["handle"]
    assert claim_mcp_read[1]["record"] == claim_core_read["record"]


def test_source_read_rejects_paths_ranges_native_text_and_nonpublic_records() -> None:
    agent = _record("agent", "tos.agent.fixture")
    owner = _MetadataOwner({agent["record_id"]: agent}, restricted={agent["record_id"]})
    service = SourceReadService(
        SourceOwnerBinding.from_owner_readers(metadata_reader=owner), metadata_record_types={"agent"}
    )
    target = _metadata_target(agent)
    with pytest.raises(SourceReadError):
        service.discover({"target": {**target, "source_ref": "../../etc/passwd"}})
    with pytest.raises(SourceReadError):
        service.discover({"target": {"layer": "text_layer", "path": "ToS/source-witnesses/x.txt"}})
    restricted = service.discover({"target": target})
    assert restricted["status"] == "access-restricted"
    assert restricted["handle"] is None

    with pytest.raises(SourceReadError):
        service.read({"handle": {}, "representation": "record", "path": "ToS/source-witnesses/fixture/agent.json"})
    with pytest.raises(SourceReadError):
        service.read({"handle": {}, "representation": "range"})


def test_source_read_requires_owner_binding_and_rejects_nonstring_exact_ids() -> None:
    agent = _record("agent", "tos.agent.fixture")
    owner = _MetadataOwner({agent["record_id"]: agent})
    with pytest.raises(TypeError, match="SourceOwnerBinding"):
        SourceReadService(_epoch(), metadata_reader=owner, metadata_record_types={"agent"})
    malformed = _metadata_target(agent)
    malformed["record_ref"]["id"] = None
    with pytest.raises(SourceReadError, match="identity must be a string"):
        SourceOwnerBinding.from_owner_readers(metadata_reader=owner)
        # Target validation is kept separate from owner binding issuance.
        SourceReadService(
            SourceOwnerBinding.from_owner_readers(metadata_reader=owner), metadata_record_types={"agent"}
        ).discover({"target": malformed})


@pytest.mark.parametrize("kind", [[], {}, None, 1, True])
def test_source_slot_kind_is_closed_before_set_lookup(kind) -> None:
    from tos_access.source_read import _selector, _target

    selector = {"layer": "source_slot", "slot_kind": kind, "identity": "tos.anchor.fixture"}
    with pytest.raises(SourceReadError):
        _selector(selector)
    with pytest.raises(SourceReadError):
        _target({**selector, "row_sha256": "a" * 64, "content_revision": "sha256:" + "b" * 64})


@pytest.mark.parametrize("value", [[], {}, None, 1, True, "private"])
def test_nonpublic_or_malformed_visibility_cannot_fall_back_or_escape(value) -> None:
    from tos_access.source_read import _status, _visibility

    assert _status({"status": value, "reason": "owner-result"})[0] == "corrupt"
    with pytest.raises(SourceReadError):
        _visibility({"visibility": value}, {"adapter": "native-corpus"})
    with pytest.raises(SourceReadError):
        _visibility({}, {"adapter": "native-corpus", "source_scope": value})
    agent = _record("agent", "tos.agent.fixture")
    owner = _MetadataOwner({agent["record_id"]: agent})
    service = SourceReadService(SourceOwnerBinding.from_owner_readers(metadata_reader=owner),
                                metadata_record_types={"agent"})
    handle = service.discover({"target": _metadata_target(agent)})["handle"]
    handle["access"]["visibility"] = value
    with pytest.raises(SourceReadError):
        validate_handle(handle)


def test_explicit_unavailable_real_receipt_fails_instead_of_skipping(monkeypatch, tmp_path) -> None:
    monkeypatch.delenv("TOS_REAL_SOURCE_INPUTS", raising=False)
    monkeypatch.delenv("TOS_REAL_SOURCE_ROOT", raising=False)
    monkeypatch.setenv("TOS_REAL_PREFLIGHT_RESULT", str(tmp_path / "missing.json"))
    with pytest.raises(AssertionError, match="receipt is unavailable"):
        test_real_preflight_owner_selector_read_has_http_mcp_core_parity()


def test_source_slot_requires_explicit_public_visibility_from_owner() -> None:
    payload = _claim("tos.claim.visibility-fixture")
    slot = _Slot("claim", payload["claim_id"], payload, "e" * 64)
    owner = _SlotOwner(slot)
    target = _slot_target(slot)

    def missing_visibility(kind: str, identity: str) -> dict:
        value = owner.describe(kind, identity)
        value.pop("visibility")
        return value

    service = SourceReadService(
        SourceOwnerBinding.from_owner_readers(slot_reader=owner),
        slot_descriptor=missing_visibility,
    )
    unavailable = service.discover({"target": target})
    assert unavailable["status"] == "corrupt"
    assert unavailable["handle"] is None

    def contradictory_visibility(kind: str, identity: str) -> dict:
        value = owner.describe(kind, identity)
        value["visibility"] = "public"
        return value

    service = SourceReadService(
        SourceOwnerBinding.from_owner_readers(slot_reader=owner),
        slot_descriptor=contradictory_visibility,
    )
    unavailable = service.discover({"target": target})
    assert unavailable["status"] == "corrupt"
    assert unavailable["handle"] is None


def test_stale_foreign_version_digest_and_overbudget_are_fail_closed() -> None:
    agent = _record("agent", "tos.agent.fixture")
    owner = _MetadataOwner({agent["record_id"]: agent})
    service = SourceReadService(
        SourceOwnerBinding.from_owner_readers(metadata_reader=owner), metadata_record_types={"agent"}
    )
    discovered = service.discover({"target": _metadata_target(agent)})
    handle = discovered["handle"]

    foreign_epoch = _epoch(source_revision="f" * 64)
    class _ForeignMetadataOwner(_MetadataOwner):
        def source_read_binding(self) -> dict:
            return foreign_epoch.value()

    foreign_owner = _ForeignMetadataOwner(owner.records)
    foreign = SourceReadService(
        SourceOwnerBinding.from_owner_readers(metadata_reader=foreign_owner), metadata_record_types={"agent"}
    )
    foreign_handle = foreign.discover({"target": _metadata_target(agent)})["handle"]
    stale = service.read({"handle": foreign_handle, "representation": "record"})
    assert stale["status"] == "stale"
    assert stale["reason"] == "source-epoch-differs"
    assert len(owner.calls) == 1
    assert len(foreign_owner.calls) == 1

    wrong_version = copy.deepcopy(handle)
    wrong_version["target"]["record_ref"]["version"] = 2
    with pytest.raises(SourceReadError, match="handle digest"):
        service.read({"handle": wrong_version, "representation": "record"})
    wrong_digest = copy.deepcopy(handle)
    wrong_digest["handle_digest"] = "sha256:" + "0" * 64
    with pytest.raises(SourceReadError, match="handle digest"):
        service.read({"handle": wrong_digest, "representation": "record"})

    small = SourceReadService(
        SourceOwnerBinding.from_owner_readers(metadata_reader=owner), metadata_record_types={"agent"},
        limits=SourceReadLimits(max_record_bytes=16),
    )
    over = small.read({"handle": handle, "representation": "record"})
    assert over["status"] == "over-budget"
    assert over["record"] is None
    assert over["provenance"] is None

    request_limited = SourceReadService(
        SourceOwnerBinding.from_owner_readers(metadata_reader=owner), metadata_record_types={"agent"},
        limits=SourceReadLimits(max_request_bytes=16),
    )
    with pytest.raises(SourceReadBudgetExceeded, match="request"):
        request_limited.read({"handle": handle, "representation": "record"})
    with pytest.raises(SourceReadBudgetExceeded, match="request"):
        request_limited.discover({"target": _metadata_target(agent)})


def test_allowlisted_source_contract_discovery_needs_no_owner_runtime(tmp_path, monkeypatch) -> None:
    import builtins

    manifest = json.loads((ACCESS / "contracts/runtime-data.v1.json").read_text())
    selected = [row for row in manifest["subjects"]
                if row["source_path"] == "access/contracts/source-read.v1.schema.json"]
    assert len(selected) == 1 and selected[0]["required"] is True
    relative = selected[0]["source_path"]
    destination = tmp_path / relative
    destination.parent.mkdir(parents=True)
    shutil.copyfile(REPO / relative, destination)
    original_import = builtins.__import__

    def without_source_owner(name, *args, **kwargs):
        assert name != "source_agent_publication", "grant-free discovery imported a source owner"
        return original_import(name, *args, **kwargs)

    monkeypatch.setattr(builtins, "__import__", without_source_owner)
    core = ToSAccessCore.discover(tos_root=tmp_path)
    assert core.source_read_capabilities()["available"] is False
    assert core.source_read_contract()["contract"] == _schema()
    with pytest.raises(SourceReadError, match="not-configured"):
        core.source_handle_discover({"selector": {"layer": "claim_record", "claim_id": "tos.claim.fixture"}})


def test_http_contract_capability_discovery_and_read_routes_are_read_only() -> None:
    # Reuse the existing tiny access fixture and add only the new contract to
    # the temporary root. No repository carrier or source payload is touched.
    fixture_spec = importlib.util.spec_from_file_location("access_fixture", ACCESS / "tests/test_access_contract.py")
    assert fixture_spec and fixture_spec.loader
    fixture_module = importlib.util.module_from_spec(fixture_spec)
    fixture_spec.loader.exec_module(fixture_module)
    agent = _record("agent", "tos.agent.fixture")
    owner = _MetadataOwner({agent["record_id"]: agent})
    service = SourceReadService(
        SourceOwnerBinding.from_owner_readers(metadata_reader=owner), metadata_record_types={"agent"}
    )

    with tempfile.TemporaryDirectory(prefix="tos-source-read-http-") as raw:
        root = Path(raw)
        fixture_module.write_fixture(root)
        destination = root / "access/contracts/source-read.v1.schema.json"
        destination.write_text((ACCESS / "contracts/source-read.v1.schema.json").read_text(encoding="utf-8"), encoding="utf-8")
        core = ToSAccessCore.discover(tos_root=root, source_read_service=service)
        server = make_server(core, port=0)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            connection = http.client.HTTPConnection("127.0.0.1", server.server_port)
            connection.request("GET", "/api/source/capabilities")
            capabilities = json.loads(connection.getresponse().read())
            assert capabilities["available"] is True
            connection.close()

            connection = http.client.HTTPConnection("127.0.0.1", server.server_port)
            connection.request("GET", "/api/source/contracts")
            contract_response = connection.getresponse()
            contract = json.loads(contract_response.read())
            assert contract_response.status == 200
            assert contract["schema"] == "tos_source_read_contract_bundle_v1"
            connection.close()

            discovery_body = json.dumps({"target": _metadata_target(agent)}).encode()
            connection = http.client.HTTPConnection("127.0.0.1", server.server_port)
            connection.request("POST", "/api/source/handles", discovery_body, {"Content-Type": "application/json"})
            discovery_response = connection.getresponse()
            discovered = json.loads(discovery_response.read())
            assert discovery_response.status == 200
            assert discovered["status"] == "available"
            connection.close()

            read_body = json.dumps({"handle": discovered["handle"], "representation": "record"}).encode()
            connection = http.client.HTTPConnection("127.0.0.1", server.server_port)
            connection.request("POST", "/api/source/read", read_body, {"Content-Type": "application/json"})
            read_response = connection.getresponse()
            read = json.loads(read_response.read())
            assert read_response.status == 200
            assert read["status"] == "available"
            assert read["record"] == agent
            connection.close()

            malformed = json.dumps({"handle": discovered["handle"], "representation": "record", "path": "/etc/passwd"}).encode()
            connection = http.client.HTTPConnection("127.0.0.1", server.server_port)
            connection.request("POST", "/api/source/read", malformed, {"Content-Type": "application/json"})
            response = connection.getresponse()
            response.read()
            assert response.status == 400
            connection.close()
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
