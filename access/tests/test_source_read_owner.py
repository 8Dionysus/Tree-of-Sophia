"""Owner selection and request-lifetime checks, not semantic acceptance."""
import copy
import asyncio
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


def test_authored_csv_selected_vector_exact_return_and_closed_boundaries(tmp_path):
    from test_source_catalog_projection import SourceCatalogProjectionTests
    from test_source_read import _ready_publication, _source_vector_for_catalog, _schema
    from authored_corpus_source_read import bootstrap_authored_csv_index
    from tos_corpus_index_common import read_edge_rows, owner_branch, authority_layer
    from tos_access.source_read import exact_csv_target
    from jsonschema import Draft202012Validator
    import source_agent_publication as publication

    fixture = SourceCatalogProjectionTests()
    fixture.setUp()
    try:
        token = _ready_publication(fixture.root)
        fixture.rebuild()
        catalog = fixture.bootstrap(include_claims=True)
        old_inputs = _source_vector_for_catalog(catalog.snapshot(), token)
        pack = 'canon/relations/fixture'
        ref = 'ToS/' + pack + '/edges.csv'
        path = fixture.root / ref
        path.parent.mkdir(parents=True)
        path.write_bytes('edge_id,unknown,missing\r\ne01,"строка\nещё",\r\ne02,last\r\n'.encode())
        columns, records, digest = read_edge_rows(path)
        corpus = {'relation_packs': [{'pack_id': pack, 'path': ref, 'columns': columns,
                   'sha256': digest, 'edge_count': len(records), 'owner_branch': owner_branch(ref),
                   'authority_layer': authority_layer(ref)}],
                  'relation_edges': [{'pack_id': pack, 'edge_id': row['edge_id'], 'properties': {
                      'source_row': i, 'source_file_sha256': digest, 'source_record': row}}
                      for i, row in enumerate(records, 1)]}
        output = tmp_path / 'authored.json'
        subprocess.run(['git', '-C', str(fixture.root), 'init', '-q'], check=True, capture_output=True)
        with pytest.raises(SourceReadError, match='tracked'):
            bootstrap_authored_csv_index(fixture.root, output, corpus, work_dir=tmp_path)
        assert not output.exists()
        subprocess.run(['git', '-C', str(fixture.root), 'add', '--', ref], check=True, capture_output=True)
        view = bootstrap_authored_csv_index(fixture.root, output, corpus, work_dir=tmp_path)
        inputs = publication.source_vector_inputs(roots={**old_inputs.roots(), 'authored-corpus': view},
            dependencies=old_inputs.value()['dependencies'], source_publication=token)
        assert inputs.value()['source_revision'] != old_inputs.value()['source_revision']
        inputs_path = tmp_path / 'inputs.raw'
        inputs_path.write_bytes(inputs.raw)
        selected = SelectedSourceReadService(fixture.root, inputs_path, expected_revision=inputs.value()['source_revision'])
        selector = {'layer': 'authored_csv_record', 'pack_id': pack, 'edge_id': 'e01'}
        discovered = selected.discover({'selector': selector})
        assert discovered['status'] == 'available', discovered
        assert discovered['target'] == exact_csv_target(corpus['relation_edges'][0])
        assert discovered['handle']['issuer'] == 'Tree-of-Sophia/authored-corpus'
        assert 'raw_record' not in discovered['provenance']
        Draft202012Validator(_schema()).validate(discovered)
        request = {'handle': discovered['handle'], 'representation': 'record'}
        result = selected.read(request)
        assert result['status'] == 'available', result
        assert selected.read(request) == result
        assert result['record'] == records[0]
        assert result['record_ref'] is None and result['record_kind'] == 'authored_csv'
        assert result['provenance']['raw_record'] == 'e01,"строка\nещё",\r\n'
        assert result['access']['rights_revalidated'] is False
        Draft202012Validator(_schema()).validate(result)
        missing = selected.discover({'selector': {**selector, 'edge_id': 'absent'}})
        assert missing['status'] == 'missing' and missing['handle'] is None
        for bad in ({**selector, 'path': ref}, {**selector, 'pack_id': 'canon/relations/../private'},
                    {**selector, 'pack_id': 'candidate-intake/payload/x'}):
            with pytest.raises(SourceReadError):
                selected.discover({'selector': bad})
        wrong = copy.deepcopy(discovered['handle'])
        wrong['issuer'] = 'Tree-of-Sophia/source-witnesses'
        with pytest.raises(SourceReadError, match='issuer'):
            selected.read({'handle': wrong, 'representation': 'record'})
        old_path = tmp_path / 'old-inputs.raw'
        old_path.write_bytes(old_inputs.raw)
        old = SelectedSourceReadService(fixture.root, old_path, expected_revision=old_inputs.value()['source_revision'])
        assert old.discover({'selector': selector})['status'] == 'unsupported'
        assert old.read(request)['status'] == 'stale'
        from tos_access.projection_mutation import MutationLimits
        session = selected._new_session()
        session.authored_reader._reader.budget.limits = MutationLimits(max_keys=0)
        assert session.discover({'target': discovered['target']})['status'] == 'over-budget'
        session = selected._new_session()
        with patch.object(session.authored_reader._reader, '_rows', return_value=[
                (json.dumps([pack, 'e01'], separators=(',', ':')), {'invalid': 'owner row'})]):
            malformed = session.discover({'target': discovered['target']})
        assert malformed['status'] == 'corrupt' and malformed['handle'] is None
        session = selected._new_session()
        session.target_issuer.authored_reader = None
        with pytest.raises(SourceReadError, match='authored issuer changed'):
            session.discover({'target': discovered['target']})
        path.write_bytes(b'edge_id,unknown,missing\ne01,CHANGED,\n')
        stale = selected.read(request)
        assert stale['status'] == 'corrupt' and stale['record'] is None
        assert 'CHANGED' not in json.dumps(stale)
        path.unlink()
        path.symlink_to(tmp_path / 'outside.csv')
        assert selected.read(request)['status'] != 'available'
    finally:
        fixture.tearDown()
        fixture.doCleanups()


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


def test_real_authored_csv_inspection_human_agent_return(tmp_path):
    """Real canon/intake source slice; not full prepared/UI publication proof."""
    inputs_ref = os.environ.get('TOS_REAL_SOURCE_INPUTS')
    if not inputs_ref:
        pytest.skip('select the retained real source vector and source root')
    from authored_corpus_source_read import bootstrap_authored_csv_index
    import tos_corpus_index_common as corpus
    import source_agent_publication as publication
    from tos_access.prepared_source_binding import PreparedSourceInputs
    from tos_access.knowledge import build_knowledge_graph, inspect_knowledge_relation
    from tos_access.source_read_projection import source_read_targets
    from tos_access.core import ToSAccessCore
    from tos_access.http_server import make_server
    from tos_access.mcp_server import build_server

    root = Path(os.environ['TOS_REAL_SOURCE_ROOT'])
    retained = PreparedSourceInputs.parse(Path(inputs_ref).read_bytes())
    errors = []
    with patch.object(corpus, 'REPO_ROOT', root):
        paths = corpus.tracked_tos_paths()
        packs, edges = corpus.build_relations(errors, tuple(p for p in paths if p.name == 'edges.csv'))
        nodes = corpus.build_nodes(errors, tuple(p for p in paths if p.name == 'node.json'))
    assert not errors and packs and edges
    view = bootstrap_authored_csv_index(root, tmp_path / 'authored.json',
        {'relation_packs': packs, 'relation_edges': edges}, work_dir=tmp_path)
    inputs = publication.source_vector_inputs(roots={**retained.roots(), 'authored-corpus': view},
        dependencies=retained.value()['dependencies'], source_publication=retained.value()['source_publication'])
    revision = inputs.value()['source_revision']
    selected_path = tmp_path / 'inputs.raw'
    selected_path.write_bytes(inputs.raw)
    selected = SelectedSourceReadService(root, selected_path, expected_revision=revision)
    entities, predicates = [json.loads((root / 'ToS/doctrine/semantic-interchange' / name).read_text())
                            for name in ('entity-types.v1.json', 'relation-types.v1.json')]
    graph = build_knowledge_graph({'nodes': nodes, 'relation_packs': packs, 'relation_edges': edges},
                                  {}, {}, entities, predicates)
    graph['source_revision'] = revision  # Explicit source slice, not a full prepared publication.
    relations = [r for r in graph['relations'] if 'pack_id' in r['source_record']['payload']]
    targets = source_read_targets(relations, revision)
    assert len(targets) == len(edges)
    # Native inspection must emit the same exact target for every actual row.
    execution = Path(__file__).resolve().parents[2]
    program = (
        f"import {{parseNativeJson,nativeChild,nativeKeys}} from {json.dumps((execution/'access/shared/native-semantics.ts').as_uri())};\n"
        f"import {{nativeSourceReadTargets}} from {json.dumps((execution/'access/deploy/cloudflare-worker/src/native-source-target.ts').as_uri())};\n"
        f"import {{nativePacketJson}} from {json.dumps((execution/'access/deploy/cloudflare-worker/src/native-lens.ts').as_uri())};\n"
        "import {readFileSync} from 'node:fs';for(const line of readFileSync(0,'utf8').trimEnd().split('\\n')){const p=parseNativeJson(line);\n"
        "process.stdout.write(nativePacketJson(await nativeSourceReadTargets([nativeChild(p,'relation')],nativeChild(p,'revision')))+'\\n');}"
    )
    native = subprocess.run(['node', '--experimental-strip-types', '--input-type=module', '-e', program],
        input='\n'.join(json.dumps({'relation': relation, 'revision': revision}, ensure_ascii=False) for relation in relations),
        text=True, capture_output=True, timeout=25)
    assert native.returncode == 0, native.stderr
    assert {key: value for line in native.stdout.splitlines() for key, value in json.loads(line).items()} == targets
    core = ToSAccessCore.discover(tos_root=root, source_read_service=selected)
    observed = []
    # Two independent seams, one per authored status; no huge end-to-end query.
    for pack in packs:
        relation = next(r for r in relations if r['source_record']['payload']['pack_id'] == pack['pack_id'])
        packet = inspect_knowledge_relation(graph, relation['id'])
        target = packet['source_read_targets'][relation['id']]['target']
        exact = selected.discover({'target': target})
        assert exact['status'] == 'available', exact
        record = selected.read({'handle': exact['handle'], 'representation': 'record'})
        assert record['status'] == 'available', record
        assert record['record'] == relation['source_record']['payload']['properties']['source_record']
        with patch.object(ToSAccessCore, 'knowledge_relation', return_value=packet):
            server = make_server(core, port=0)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                web = execution / 'access/web/src/observatory'
                human_program = (
                    f"import {{KnowledgeClient}} from {json.dumps((web/'knowledge-client.mjs').as_uri())};\n"
                    f"import {{readExactSource}} from {json.dumps((web/'exact-source-read.mjs').as_uri())};\n"
                    "const p=JSON.parse(process.argv[1]);const c=new KnowledgeClient({fetcher:(path,options)=>fetch(p.url+path,options)});\n"
                    "process.stdout.write(JSON.stringify(await readExactSource(c,p.selection)));"
                )
                args = {'url': f'http://127.0.0.1:{server.server_port}', 'selection': {
                    'kind': 'relation', 'id': relation['id'], 'source_revision': revision,
                    'content_revision': relation['content_revision']}}
                human = subprocess.run(['node', '--input-type=module', '-e', human_program, json.dumps(args)],
                    text=True, capture_output=True, timeout=25)
                assert human.returncode == 0, human.stderr
                human_read = json.loads(human.stdout)
                assert human_read['record'] == record['record']
                assert human_read['handle'] == exact['handle']
                async def agent_read():
                    mcp = build_server(core=core)
                    inspection = await mcp.call_tool('tos_knowledge_relation', {'relation_id': relation['id']})
                    assert inspection[1]['source_read_targets'][relation['id']]['target'] == target
                    discovered = await mcp.call_tool('tos_source_handle_discover', {'target': target})
                    return await mcp.call_tool('tos_source_read', {'handle': discovered[1]['handle'], 'representation': 'record'})
                agent = asyncio.run(agent_read())
                assert agent[1] == record
            finally:
                server.shutdown(); server.server_close(); thread.join(timeout=5)
        observed.append({'id': relation['id'], 'owner_branch': pack['owner_branch'],
                         'authority_layer': record['provenance']['authority_layer'],
                         'exact_human_agent_return': True})
    print(json.dumps({'status': 'passed', 'scope': 'real source slice, not full prepared publication or rendered UI',
        'rows': len(edges), 'packs': len(packs), 'native_python_target_parity': True,
        'source_revision': revision, 'observed': observed}, ensure_ascii=False))
