"""Maintained Metadata predecessor and full-union oracle for the native caller.

The exporter stops BEFORE the final initial creation. Only the native owner
creates that package; the oracle observes it without publishing prepared state.
"""
from pathlib import Path
import hashlib
import json
import sys
import tempfile
from types import SimpleNamespace
from unittest.mock import patch

REPOSITORY = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY / 'access/tests'))
import test_source_metadata_publication as maintained

MAX_PACKET = 16 * 1024 * 1024
MAX_FILES = 2048


def write(path, value):
    raw = json.dumps(value, ensure_ascii=False, separators=(',', ':')).encode()
    if len(raw) > MAX_PACKET:
        raise ValueError('Metadata oracle packet budget')
    path.write_bytes(raw)


def export(work, packet_path):
    class BeforeNativeCreation(Exception):
        pass
    original_directory = tempfile.TemporaryDirectory
    original_command = maintained.commands.run_legacy_oracle_command
    selected_request = None
    def directory(*args, **kwargs):
        kwargs['dir'] = str(work)
        result = original_directory(*args, **kwargs)
        result._finalizer.detach()  # Whole native consumer owns disposal.
        return result
    def command(owner, request, *args, **kwargs):
        nonlocal selected_request
        if (Path(owner).name == 'metadata-addition-owner.json'
                and request.get('operation') == 'source.create'):
            selected_request = request
            raise BeforeNativeCreation()
        return original_command(owner, request, *args, **kwargs)
    with patch.object(tempfile, 'TemporaryDirectory', directory), patch.object(
            maintained.commands, 'run_legacy_oracle_command', command):
        case = maintained.SourceMetadataPublicationTests()
        try:
            case.setUp()
        except BeforeNativeCreation:
            pass
        else:
            raise ValueError('maintained Metadata native creation boundary missing')
        if selected_request is None or case.db.in_transaction:
            raise ValueError('Metadata predecessor must precede native creation/BEGIN')
        # Actual-cut workers require the shared authored contracts. Preserve
        # fixture-owned overrides and limit this synthetic source inventory.
        contracts = 0
        for ordinal, source in enumerate(sorted((REPOSITORY/'ToS/contracts').glob('*.schema.json'))):
            if ordinal >= 512 or source.is_symlink():
                raise ValueError('Metadata selected contract inventory')
            destination = case.root/'ToS/contracts'/source.name
            if not destination.exists():
                size = source.stat().st_size
                contracts += size
                if size > 4*1024*1024 or contracts > 8*1024*1024:
                    raise ValueError('Metadata selected contract bytes')
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(source.read_bytes())
        files, total = {}, 0
        for ordinal, source in enumerate(sorted((case.root/'ToS').rglob('*'))):
            if ordinal >= 4096 or source.is_symlink():
                raise ValueError('Metadata synthetic source inventory')
            if source.is_file() and source.name != '.historical-create.writer.lock':
                size = source.stat().st_size
                total += size
                if size > 8*1024*1024 or total > 16*1024*1024 or len(files) >= MAX_FILES:
                    raise ValueError('Metadata synthetic source bytes')
                files[source.relative_to(case.root).as_posix()] = source.read_bytes().hex()
        case.owner.chmod(0o600)
        state = json.loads(case.db.execute(
            'SELECT json FROM source_dependency_state WHERE singleton=1').fetchone()[0])
        packet = {'db_path': str(case.base.path), 'source_root': str(case.root),
            'owner_config': str(case.owner), 'source_path': case.relative,
            'record_id': case.record['record_id'], 'creation_request': selected_request,
            'source_inputs': case.base.source.value(), 'binding': case.base.binding,
            'header': case.base.inputs.header, 'entities': case.base.entities,
            'relations': case.base.relations, 'lenses': case.base.inputs.lenses,
            'corpus': case.base.corpus, 'bibliography': case.base.bibliography,
            'source_files': files,
            'baseline_semantic_report': case.base.graph['counts']['semantic_validation'],
            'dependency_implementation_before': state['implementation_sha256'],
            'descriptor': json.loads((REPOSITORY/'rust/crates/tos-compiler/tests/fixtures/query-vocabulary.v1.json').read_bytes())}
        case.db.execute('PRAGMA wal_checkpoint(FULL)')
        case.db.close()
        write(packet_path, packet)


def oracle(packet_path, output_path):
    from tos_access.catalog_semantics import CatalogInputs, CANONICAL_ORDER
    from tos_access.prepared_source_binding import PreparedSourceInputs
    from tos_access.prepared_source_dependencies import ProgressHandlerOwner
    packet = json.loads(packet_path.read_bytes())
    root = Path(packet['source_root'])
    owner = Path(packet['owner_config'])
    receipt = (root/packet['source_path']).with_name('source-create-receipt.json')
    raw = receipt.read_bytes()
    if len(raw) > 1024*1024:
        raise ValueError('Metadata native receipt budget')
    selected = json.loads(raw)
    source = PreparedSourceInputs.parse(maintained.canonical_bytes(packet['source_inputs']))
    inputs = CatalogInputs(packet['header'], packet['entities'], packet['relations'],
                          packet['lenses'], source_order_profile=CANONICAL_ORDER)
    # Use the same maintained operation and exact maintained full-union oracle.
    # No Python source.create, SQLite transaction or prepared publication occurs.
    with maintained.publication.metadata_addition_publication(owner,
            source_inputs=source, expected_binding=packet['binding'], catalog_inputs=inputs,
            progress_owner=ProgressHandlerOwner(),
            expected_receipt_sha256=hashlib.sha256(raw).hexdigest(),
            expected_request_digest=selected['request_digest']) as operation:
        case = maintained.SourceMetadataPublicationTests()
        case.base = SimpleNamespace(corpus=packet['corpus'], bibliography=packet['bibliography'],
                                    entities=packet['entities'], relations=packet['relations'])
        graph, corpus = case.oracle(operation)
        new_node_id = 'source-navigation:' + next(row['node_id'] for (g, _), row
            in operation.raw['nodes'].items() if g == 'source-navigation'
            and row['node_id'] == packet['record_id'])
        new_relation_id = 'source-navigation:' + next(row['edge_id'] for (g, _), row
            in operation.raw['edges'].items() if g == 'source-navigation')
    write(output_path, {'expected': graph, 'corpus': corpus, 'new_node_id': new_node_id,
                        'new_relation_id': new_relation_id})


if __name__ == '__main__':
    if len(sys.argv) == 4 and sys.argv[1] == 'oracle':
        oracle(*map(Path, sys.argv[2:]))
    elif len(sys.argv) == 3:
        work, packet = map(Path, sys.argv[1:])
        if not work.is_dir() or packet.parent != work:
            raise ValueError('explicit Metadata disposable workspace required')
        export(work, packet)
    else:
        raise ValueError('Metadata fixed export/oracle action')
