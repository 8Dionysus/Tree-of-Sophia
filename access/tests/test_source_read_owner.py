"""Owner selection and request-lifetime checks, not semantic acceptance."""
import copy
import json
import os
from pathlib import Path
import sys
import subprocess
import threading
from threading import BoundedSemaphore
from unittest.mock import Mock, patch
from types import ModuleType
from importlib.machinery import ModuleSpec

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))
from tos_access.cli import main
from tos_access.source_read import SourceReadError
from tos_access.source_read_owner import SelectedSourceReadService


def test_each_request_has_an_isolated_reader_and_saturation_is_not_queued():
    # Test the lifecycle wrapper independently of source content and catalogs.
    selected = object.__new__(SelectedSourceReadService)
    selected._slots = BoundedSemaphore(2)
    sessions = []
    def fresh():
        result = Mock()
        result.read.return_value = {"session": len(sessions)}
        sessions.append(result)
        return result
    selected._new_session = fresh
    assert selected.read({}) == {"session": 0}
    assert selected.read({}) == {"session": 1}
    assert sessions[0] is not sessions[1]
    selected._slots.acquire(); selected._slots.acquire()
    with pytest.raises(SourceReadError, match="concurrency budget"):
        selected.read({})
    assert len(sessions) == 2
    selected._slots.release(); selected._slots.release()
    selected._new_session = Mock(side_effect=ValueError("owner changed"))
    with pytest.raises(ValueError, match="owner changed"):
        selected.read({})
    assert selected._slots.acquire(blocking=False)
    assert selected._slots.acquire(blocking=False)


def test_owner_selection_refuses_foreign_preloaded_publication_verifier():
    from tos_access.source_read_owner import _owner_modules
    foreign = ModuleType("source_agent_publication")
    foreign.__file__ = "/foreign-owner/source_agent_publication.py"
    foreign.__spec__ = ModuleSpec("source_agent_publication", loader=None, origin=foreign.__file__)
    with patch.dict(sys.modules, {"source_agent_publication": foreign}):
        with pytest.raises(SourceReadError, match="module origin differs.*source_agent_publication"):
            _owner_modules()


def test_cli_source_selection_requires_explicit_prepared_pair():
    with pytest.raises(SystemExit, match="requires --root"):
        main(["--source-inputs", "/unselected/inputs.raw", "source", "capabilities"])


@pytest.mark.parametrize("posture", ["public", "private", "conditional", "corrupt"])
def test_exact_metadata_handle_never_bypasses_native_owner_gates(tmp_path, posture):
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
    from test_native_text_binding import NativeTextBindingFixture
    from test_source_read import _MetadataOwner, _record, _metadata_target, _schema
    from tos_access.source_read import SourceOwnerBinding, SourceReadService
    from jsonschema import Draft202012Validator
    fixture = NativeTextBindingFixture(tmp_path)
    if posture != "private":
        fixture.make_public()
    if posture == "conditional":
        fixture.rights["redistribution_posture"] = "authorized_with_conditions"
        fixture.refresh()
    if posture == "corrupt":
        fixture.write_bytes(fixture.content_ref, b"WRONG_PRIVATE_SENTINEL")
    record = _record("occurrence", "tos.occurrence.fixture")
    record["native_text_binding"] = copy.deepcopy(fixture.binding)
    owner = _MetadataOwner({record["record_id"]: record})
    session = SourceReadService(SourceOwnerBinding.from_owner_readers(metadata_reader=owner),
                                metadata_record_types={"occurrence"})
    selected = object.__new__(SelectedSourceReadService)
    selected.source_root, selected.limits = tmp_path, session.limits
    selected.local_text_selection = None
    selected._slots = BoundedSemaphore(2)
    selected._new_session = lambda: session
    handle = session.discover({"target": _metadata_target(record)})["handle"]
    request = {"handle": handle, "representation": "native_public_unit"}
    result = selected.read(request)
    local = selected.read({**request, "representation": "native_local_unit"})
    assert local["status"] == "unsupported"
    assert local["native_unit"] is local["text_access"] is None
    Draft202012Validator(_schema()).validate(local)
    Draft202012Validator(_schema()).validate(result)
    assert result["record"] is None
    assert result["handle"]["access"]["rights_revalidated"] is False
    assert result["grants_current_use"] is False
    if posture == "public":
        assert result["status"] == "available", result
        assert result["native_unit"]["spans"][0]["text"] == "cafe\u0301"
        assert result["text_access"]["recorded_rights_verified"] is True
        from native_text_return import LocalTextReadError
        selected.local_text_selection = Mock()
        selected.local_text_selection.verify.side_effect = LocalTextReadError("revoked after owner return")
        with patch("native_text_return.read_local_unit", return_value=result["native_unit"]):
            revoked = selected.read({**request, "representation": "native_local_unit"})
        assert revoked["status"] == "access-restricted"
        assert revoked["native_unit"] is revoked["text_access"] is None
        Draft202012Validator(_schema()).validate(revoked)
    else:
        assert result["status"] == ("corrupt" if posture == "corrupt" else "access-restricted")
        assert result["native_unit"] is result["text_access"] is None
        assert "WRONG_PRIVATE_SENTINEL" not in json.dumps(result)
    with pytest.raises(SourceReadError, match="handle and representation only"):
        selected.read({**request, "path": fixture.content_ref})
    selected._slots.acquire(); selected._slots.acquire()
    with pytest.raises(SourceReadError, match="concurrency budget"):
        selected.read(request)
    selected._slots.release(); selected._slots.release()


def test_cli_default_source_capabilities_do_not_load_owner(capsys):
    with patch("tos_access.source_read_owner._owner_modules", side_effect=AssertionError("owner import")):
        main(["source", "capabilities"])
    assert json.loads(capsys.readouterr().out)["available"] is False


def test_source_operations_are_discoverable_without_owner_activation():
    from tos_access.core import ToSAccessCore
    from tos_access.cli import _parser
    root = Path(__file__).resolve().parents[2]
    with patch("tos_access.source_read_owner._owner_modules", side_effect=AssertionError("owner import")):
        core = ToSAccessCore.discover(tos_root=root)
        catalog = core.knowledge_contracts()
        api = catalog["contracts"]["api"]
        operations = {row["operation_id"]: row for row in api["operations"] if row["operation_id"].startswith("tos.source.")}
        assert set(operations) == {"tos.source.read.capabilities", "tos.source.read.contracts",
                                   "tos.source.handle.discover", "tos.source.record.read"}
        for row in operations.values():
            command = row["cli"].split()[1:]
            assert _parser().parse_args(command).command == "source"
            assert row["http"]["path"].startswith("/api/source/")
            assert "Python" in row["available_on"]
        assert api["data_contracts"]["source_read"] == "source-read.v1.schema.json"
        assert catalog["contracts"]["source_read"]["$defs"]["handle"]
        assert core.source_read_capabilities()["available"] is False


def test_cli_rejects_oversized_and_duplicate_request_before_dispatch(tmp_path):
    request = tmp_path / "request.json"
    core = Mock()
    for raw in (b" " * 65537, b'{"target":{},"target":{}}'):
        request.write_bytes(raw)
        with patch("tos_access.cli.ToSAccessCore.discover", return_value=core):
            with pytest.raises(SystemExit, match="cannot read exact source"):
                main(["source", "discover", str(request)])
    core.source_handle_discover.assert_not_called()


def test_cli_keeps_explicit_source_selection_in_the_mcp_core(tmp_path):
    binding = tmp_path / "binding.json"
    binding.write_text(json.dumps({"source_revision": "a" * 64}))
    inputs = tmp_path / "inputs.raw"
    selected, core, server = Mock(), Mock(), Mock()
    with patch("tos_access.source_read_owner.SelectedSourceReadService", return_value=selected) as choose, \
         patch("tos_access.cli.ToSAccessCore.discover", return_value=core) as discover, \
         patch("tos_access.mcp_server.build_server", return_value=server) as build, \
         patch("tos_access.mcp_server._run_server") as run:
        main(["--root", str(tmp_path), "--prepared-read-model", str(tmp_path / "snapshot.sqlite"),
              "--prepared-binding", str(binding), "--source-inputs", str(inputs), "mcp"])
    choose.assert_called_once_with(tmp_path, inputs, expected_revision="a" * 64)
    assert discover.call_args.kwargs["source_read_service"] is selected
    build.assert_called_once_with(core=core)
    run.assert_called_once_with(server)


def test_real_selected_owner_inspection_target_and_fresh_request_parity():
    ref = os.environ.get("TOS_REAL_SOURCE_INPUTS")
    if not ref:
        pytest.skip("select the exact retained real source vector")
    root = Path(os.environ["TOS_REAL_SOURCE_ROOT"])
    raw = Path(ref).read_bytes()
    revision = json.loads(raw)["source_revision"]
    selected = SelectedSourceReadService(root, Path(ref), expected_revision=revision)
    from tos_access.knowledge import _normalize_node, inspect_knowledge_node
    observed = []
    for selector in (
        {"layer": "metadata_record", "record_type": "agent", "record_id": "tos.agent.friedrich-nietzsche"},
        {"layer": "claim_record", "claim_id": "tos.claim.basel-print.environment-place"},
    ):
        first = selected.discover({"selector": selector})
        assert first["status"] == "available", first
        request = {"handle": first["handle"], "representation": "record"}
        record = selected.read(request)
        assert record["status"] == "available", record
        assert selected.read(request) == record  # fresh reader, identical exact result
        metadata = selector["layer"] == "metadata_record"
        # A bounded normalization seam over actual owner-read source material.
        # This is not a full graph publication or a claim about graph coverage.
        raw_node = {"node_id": record["record_ref"]["id"],
                    "node_kind": "agent" if metadata else "claim",
                    "properties": {"source_record" if metadata else "source_claim": record["record"]}}
        node = _normalize_node(raw_node, "source-claims", source_kind_id="agent" if metadata else "claim")
        before = copy.deepcopy(node)
        packet = inspect_knowledge_node({"source_revision": revision, "nodes": [node], "relations": []}, node["id"], 0)
        target = packet["source_read_targets"][node["id"]]
        assert target == {"source_revision": revision, "target": first["target"]}
        assert selected.discover({"target": target["target"]}) == first
        assert node == before
        # Cross the real HTTP -> human client -> owner seam on this bounded
        # real-material inspection fixture. Only inspection is fixture-bound;
        # source selection/discovery/reading and the JS client are production.
        # This does not certify the full graph UI or publication currentness.
        from tos_access.core import ToSAccessCore
        from tos_access.http_server import make_server
        core = ToSAccessCore.discover(tos_root=root, source_read_service=selected)
        with patch.object(ToSAccessCore, "knowledge_node", return_value=packet):
            server = make_server(core, port=0)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                web = Path(__file__).resolve().parents[1] / "web/src/observatory"
                program = (
                    f"import {{KnowledgeClient}} from {json.dumps((web / 'knowledge-client.mjs').as_uri())};\n"
                    f"import {{readExactSource}} from {json.dumps((web / 'exact-source-read.mjs').as_uri())};\n"
                    "const input=JSON.parse(process.argv[1]);\n"
                    "const client=new KnowledgeClient({fetcher:(path,options)=>fetch(input.url+path,options)});\n"
                    "const result=await readExactSource(client,input.selection);\n"
                    "process.stdout.write(JSON.stringify(result));\n"
                )
                args = {"url": f"http://127.0.0.1:{server.server_port}", "selection": {
                    "kind": "node", "id": node["id"], "source_revision": revision,
                    "content_revision": node["content_revision"]}}
                completed = subprocess.run(["node", "--input-type=module", "-e", program, json.dumps(args)],
                    text=True, capture_output=True, timeout=25)
                assert completed.returncode == 0, completed.stderr
                human_read = json.loads(completed.stdout)
                assert human_read["status"] == "available", human_read
                assert human_read["record"] == record["record"]
                assert human_read["handle"] == first["handle"]
            finally:
                server.shutdown(); server.server_close(); thread.join(timeout=5)
        observed.append(record["record_ref"]["id"])
    assert observed == ["tos.agent.friedrich-nietzsche", "tos.claim.basel-print.environment-place"]
    with pytest.raises(SourceReadError, match="revisions differ"):
        SelectedSourceReadService(root, Path(ref), expected_revision="0" * 64)
