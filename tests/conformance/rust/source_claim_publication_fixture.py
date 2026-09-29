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

if __name__ == '__main__':
    work, packet = map(Path, sys.argv[1:])
    if not work.is_dir() or packet.parent != work:
        raise ValueError('explicit disposable fixture workspace required')
    export(work, packet)
