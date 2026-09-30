"""Existing maintained Claim fixture/oracle, exported before any native BEGIN.
No alternate domain producer, authored admission, or semantic index relabeling.
"""
from pathlib import Path
import json
import sys
import tempfile
from unittest.mock import patch

REPOSITORY = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY / 'tests'))
import test_source_claim_publication as maintained

MAX_PACKET = 16 * 1024 * 1024
MAX_FILES = 2048
MAX_SOURCE_BYTES = 16 * 1024 * 1024

def write_fixture(case, packet_path):
    if case.db.in_transaction:
        raise ValueError('native whole caller must receive fixture before BEGIN')
    with case.operation() as operation:
        expected = case.oracle(operation)  # Actual maintained full union oracle once.
        raw = operation.raw
    files, total = {}, 0
    # Synthetic maintained fixture only, for the exact worker's real sealed cut.
    for ordinal, path in enumerate(sorted((case.root / 'ToS').rglob('*'))):
        if ordinal >= 4096:
            raise ValueError('synthetic fixture traversal budget')
        if path.is_symlink():
            raise ValueError('synthetic schema cut symlink')
        if path.is_file():
            if path.stat().st_size > MAX_SOURCE_BYTES:
                raise ValueError('synthetic selected source file budget')
            payload = path.read_bytes()
            total += len(payload)
            if len(files) >= MAX_FILES or total > MAX_SOURCE_BYTES:
                raise ValueError('synthetic selected worker cut budget')
            files[path.relative_to(case.root).as_posix()] = payload.hex()
    state = json.loads(case.db.execute('SELECT json FROM source_dependency_state WHERE singleton=1').fetchone()[0])
    claim_id = case.claim['claim_id']
    node_id = 'source-claims:' + next(row['node_id'] for row in raw['nodes'].values()
                                    if row['properties'].get('claim_id') == claim_id)
    relation_id = 'source-claims:' + next(row['edge_id'] for row in raw['edges'].values()
                                        if row.get('claim_ref') == claim_id)
    descriptor = json.loads((REPOSITORY / 'rust/crates/tos-compiler/tests/fixtures/query-vocabulary.v1.json').read_bytes())
    packet = {'db_path': str(case.base.path), 'owner_config': str(case.owner),
              'source_inputs': case.base.source.value(), 'binding': case.base.binding,
              'header': case.base.inputs.header, 'entities': case.base.entities,
              'relations': case.base.relations, 'lenses': case.base.inputs.lenses,
              'descriptor': descriptor, 'expected': expected, 'baseline_semantic_report': case.base.graph['counts']['semantic_validation'], 'source_files': files,
              'dependency_implementation_before': state['implementation_sha256'],
              'expected_receipt_sha256': case.expected['expected_receipt_sha256'],
              'expected_request_digest': case.expected['expected_request_digest'],
              'new_node_id': node_id, 'new_relation_id': relation_id,
              'receipt_path': str(Path(packet_path).with_suffix('.receipt.json')),
              'binding_path': str(Path(packet_path).with_suffix('.binding.json'))}
    encoded = json.dumps(packet, ensure_ascii=False, separators=(',', ':')).encode()
    if len(encoded) > MAX_PACKET:
        raise ValueError('maintained Claim fixture packet budget')
    case.db.execute('PRAGMA wal_checkpoint(FULL)')
    Path(packet_path).write_bytes(encoded)


def export(work, packet_path):
    # Every fixture temporary directory stays inside the caller-owned disposable
    # workspace, with no copy or relocation of source roots/SQLite namespaces.
    original = tempfile.TemporaryDirectory
    retained = []
    def directory(*args, **kwargs):
        kwargs['dir'] = str(work)
        result = original(*args, **kwargs)
        result._finalizer.detach()
        retained.append(result)
        return result
    with patch.object(tempfile, 'TemporaryDirectory', directory):
        case = maintained.SourceClaimPublicationTests()
        case.setUp()
        write_fixture(case, packet_path)
        case.db.close()
    # Rust TempDir owns final cleanup after the whole native operation.

def export_agent(work, packet_path):
    import test_source_agent_publication as agents
    original = tempfile.TemporaryDirectory
    def directory(*args, **kwargs):
        kwargs['dir'] = str(work)
        result = original(*args, **kwargs)
        result._finalizer.detach()
        return result
    with patch.object(tempfile, 'TemporaryDirectory', directory):
        case = agents.SourceAgentPublicationTests()
        case.setUp()
        # Actual-cut workers require the maintained catalog/header/slot and
        # referenced shared contracts. Preserve any fixture-owned schema bytes.
        contract_bytes = 0
        for ordinal, contract in enumerate(sorted((REPOSITORY/'ToS/contracts').glob('*.schema.json'))):
            if ordinal >= 512 or contract.is_symlink():
                raise ValueError('Agent selected contract inventory')
            target = case.root/'ToS/contracts'/contract.name
            if not target.exists():
                raw = contract.read_bytes()
                contract_bytes += len(raw)
                if len(raw) > 4*1024*1024 or contract_bytes > 8*1024*1024:
                    raise ValueError('Agent selected contract bytes')
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(raw)
        owner = case.helper.fixture.fixture.owner
        owner.chmod(0o600)
        # Real owner fixture and Python prepare oracle; no source revision is
        # executed here. Rust invokes the one existing native Record writer.
        proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
                    'fields': {'notes': 'Native Agent publication whole consumer correction.'},
                    'forms': case.helper.fixture.fixture.selections,
                    'reason': 'Synthetic descriptive correction; no admission.'}
        import source_commands as commands
        preview = commands.run_legacy_oracle_command(owner, proposal)
        files, total = {}, 0
        for path in sorted((case.root / 'ToS').rglob('*')):
            if path.is_symlink():
                raise ValueError('Agent source symlink')
            if path.is_file() and path.name != '.historical-create.writer.lock':
                raw = path.read_bytes()
                total += len(raw)
                if len(raw) > 8*1024*1024 or total > MAX_SOURCE_BYTES or len(files) >= MAX_FILES:
                    raise ValueError('Agent initial source budget')
                files[path.relative_to(case.root).as_posix()] = raw.hex()
        state = json.loads(case.db.execute('SELECT json FROM source_dependency_state WHERE singleton=1').fetchone()[0])
        packet = {'db_path': str(case.path), 'owner_config': str(owner), 'source_root': str(case.root),
                  'record_id': case.helper.fixture.identity, 'source_path': case.helper.fixture.relative,
                  'proposal': proposal, 'python_preview': preview,
                  'source_inputs': case.source.value(), 'binding': case.binding,
                  'header': case.inputs.header, 'entities': case.entities, 'relations': case.relations,
                  'lenses': case.inputs.lenses, 'source_files': files,
                  'baseline_semantic_report': case.graph['counts']['semantic_validation'],
                  'dependency_implementation_before': state['implementation_sha256'],
                  'descriptor': json.loads((REPOSITORY/'rust/crates/tos-compiler/tests/fixtures/query-vocabulary.v1.json').read_bytes()),
                  'catalog_root_hex': case.snapshot.view.root_bytes.hex(),
                  'catalog_namespace_path': str(case.snapshot.view.namespace_path)}
        case.db.execute('PRAGMA wal_checkpoint(FULL)')
        case.db.close()
        raw = json.dumps(packet, ensure_ascii=False, separators=(',', ':')).encode()
        if len(raw) > MAX_PACKET:
            raise ValueError('Agent packet budget')
        packet_path.write_bytes(raw)

def agent_oracle(packet_path, source_receipt_path, output_path):
    # Independent maintained full tiny builder AFTER the native source writer.
    # It writes only its bounded derivative oracle, no actual source/prepared
    # state or accepted catalog selector.
    import source_catalog_projection as catalog
    import source_witness_bibliographic_graph_common as bibliography
    import tos_corpus_index_common as navigation
    from tos_access import knowledge
    from tos_access.projection_mutation import ProjectionSnapshotView
    import hashlib
    packet = json.loads(packet_path.read_bytes())
    receipt = json.loads(source_receipt_path.read_bytes())
    root = Path(packet['source_root'])
    before_raw = bytes.fromhex(packet['catalog_root_hex'])
    before_sha = hashlib.sha256(before_raw).hexdigest()
    before = catalog.SourceCatalogSnapshot(ProjectionSnapshotView(before_raw,
        Path(packet['catalog_namespace_path'])), expected_root_sha256=before_sha, trusted_baseline_sha256=before_sha)
    transaction = receipt['receipt']['publication']['transaction_id']
    candidate = catalog.stage_agent_catalog_transition(root, before, transaction_id=transaction,
        expected_publication_token=receipt['publication_snapshot'], target_part_bytes=256)
    snapshot = catalog.SourceCatalogSnapshot(candidate.snapshot(), expected_root_sha256=candidate.root_sha256,
                                             trusted_baseline_sha256=candidate.root_sha256)
    # The maintained full builder consumes generated legacy catalog files.
    # Regenerate those only in a bounded derivative oracle copy, so the actual
    # source/current cut remains the exact three-file committed delta.
    import shutil
    import build_source_witness_catalog as legacy
    oracle_root = output_path.parent / 'agent-oracle-root'
    oracle_root.mkdir(mode=0o700)
    copied = 0
    for ordinal, source in enumerate(sorted((root/'ToS').rglob('*'))):
        if ordinal >= 16384 or source.is_symlink():
            raise ValueError('Agent derivative oracle inventory')
        destination = oracle_root/source.relative_to(root)
        if source.is_dir():
            destination.mkdir(parents=True, exist_ok=True)
        elif source.is_file():
            size = source.stat().st_size
            copied += size
            if size > 8*1024*1024 or copied > 32*1024*1024:
                raise ValueError('Agent derivative oracle bytes')
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
        else:
            raise ValueError('Agent derivative oracle file type')
    outputs = legacy.render_outputs(oracle_root)
    if sum(len(text.encode()) for text in outputs.values()) > 16*1024*1024:
        raise ValueError('Agent derivative generated catalog bytes')
    legacy.write_outputs(oracle_root, outputs)
    with patch.object(navigation, 'REPO_ROOT', oracle_root), patch.object(navigation, 'TOS_ROOT', oracle_root/'ToS'):
        corpus = {'source_navigation': navigation.build_source_navigation([], catalog_snapshot=snapshot)}
    graph = knowledge.build_knowledge_graph(corpus, {}, bibliography.build_payload(oracle_root),
                                          packet['entities'], packet['relations'])
    raw = json.dumps(graph, ensure_ascii=False, separators=(',', ':')).encode()
    if len(raw) > MAX_PACKET:
        raise ValueError('Agent oracle packet budget')
    output_path.write_bytes(raw)

if __name__ == '__main__':
    if len(sys.argv) == 5 and sys.argv[1] == 'agent-oracle':
        agent_oracle(*map(Path, sys.argv[2:]))
        raise SystemExit(0)
    agent = len(sys.argv) == 4 and sys.argv[1] == 'agent'
    work, packet = map(Path, sys.argv[2:] if agent else sys.argv[1:])
    if not work.is_dir() or packet.parent != work:
        raise ValueError('explicit disposable fixture workspace required')
    (export_agent if agent else export)(work, packet)
