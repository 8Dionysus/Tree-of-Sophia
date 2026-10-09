"""Maintained Metadata predecessor and full-union oracle for the native caller.

The exporter stops BEFORE the final initial creation. Only the native owner
creates that package; the oracle observes it without publishing prepared state.
"""
from pathlib import Path
import hashlib
import json
import os
import stat
import sys
import tempfile
from types import SimpleNamespace
from unittest.mock import patch

REPOSITORY = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY / 'access/tests'))
import test_source_metadata_publication as maintained

MAX_PACKET = 16 * 1024 * 1024
MAX_FILES = 2048
CAPTURE_TOTAL_MAX = 680 * 1024 * 1024
CAPTURE_DB_MAX = 600 * 1024 * 1024


def _capture_root():
    raw = os.environ.get('TOS_NATIVE_METADATA_CAPTURE_DIR')
    if not raw:
        return None
    root = Path(raw)
    if not root.is_absolute() or root.is_symlink():
        raise ValueError('absolute private Metadata capture directory required')
    info = root.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
        raise ValueError('Metadata capture directory must be caller-owned mode 0700')
    return root


def _stamp(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_size,
            info.st_mtime_ns, info.st_ctime_ns, info.st_nlink)


def _capture_file(source, name, cap):
    root = _capture_root()
    if root is None:
        return None
    if Path(name).name != name or name in ('', '.', '..'):
        raise ValueError('single Metadata capture filename required')
    source, destination = Path(source), root / name
    flags = os.O_RDONLY | getattr(os, 'O_CLOEXEC', 0) | getattr(os, 'O_NOFOLLOW', 0)
    source_fd = os.open(source, flags)
    destination_fd = None
    try:
        before = os.fstat(source_fd)
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_size > cap:
            raise ValueError('Metadata capture input type/link/size budget')
        destination_fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL |
                                 getattr(os, 'O_CLOEXEC', 0) | getattr(os, 'O_NOFOLLOW', 0), 0o600)
        digest, total = hashlib.sha256(), 0
        while True:
            chunk = os.read(source_fd, 65536)
            if not chunk:
                break
            total += len(chunk)
            if total > cap:
                raise ValueError('Metadata capture input byte budget')
            digest.update(chunk)
            view = memoryview(chunk)
            while view:
                written = os.write(destination_fd, view)
                if written <= 0:
                    raise OSError('short Metadata capture write')
                view = view[written:]
        after = os.fstat(source_fd)
        entry = source.lstat()
        if total != before.st_size or _stamp(before) != _stamp(after) or _stamp(after) != _stamp(entry):
            raise ValueError('Metadata capture input changed during read')
        os.fsync(destination_fd)
        copied = os.fstat(destination_fd)
        if not stat.S_ISREG(copied.st_mode) or copied.st_size != total or stat.S_IMODE(copied.st_mode) != 0o600:
            raise ValueError('Metadata capture output verification')
        return {'name': name, 'bytes': total, 'sha256': digest.hexdigest(), 'held_full_EOF': True}
    finally:
        os.close(source_fd)
        if destination_fd is not None:
            os.close(destination_fd)


def _capture_manifest(name, phase, files):
    root = _capture_root()
    if root is None:
        return
    total = sum(row['bytes'] for row in files)
    if total > CAPTURE_TOTAL_MAX:
        raise ValueError('Metadata capture aggregate byte budget')
    path = root / name
    value = {'schema_version': 'tos_native_metadata_fixture_capture_v1',
             'phase': phase, 'files': files, 'total_bytes': total}
    raw = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL |
                 getattr(os, 'O_CLOEXEC', 0) | getattr(os, 'O_NOFOLLOW', 0), 0o600)
    try:
        view = memoryview(raw)
        while view:
            written = os.write(fd, view)
            if written <= 0:
                raise OSError('short Metadata capture manifest write')
            view = view[written:]
        os.fsync(fd)
    finally:
        os.close(fd)


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
    class CallerOwnedDirectory(original_directory):
        def cleanup(self):
            # Generator context exit must not delete the exported predecessor.
            # The caller's Rust TempDir owns this entire nested workspace.
            self._finalizer.detach()
    def directory(*args, **kwargs):
        kwargs['dir'] = str(work)
        result = CallerOwnedDirectory(*args, **kwargs)
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
            'descriptor': json.loads((REPOSITORY/'rust/crates/tos-compiler/tests/fixtures/query-vocabulary.v1.json').read_bytes()),
            'owner_config_document': json.loads(case.owner.read_bytes())}
        case.db.execute('PRAGMA wal_checkpoint(FULL)')
        case.db.close()
        for suffix in ('-wal', '-shm', '-journal'):
            if Path(str(case.base.path) + suffix).exists():
                raise ValueError('Metadata prepared capture must be checkpointed')
        write(packet_path, packet)
        if _capture_root() is not None:
            captured = [
                _capture_file(case.base.path, 'metadata-prepared-before.sqlite', CAPTURE_DB_MAX),
                _capture_file(packet_path, 'metadata-prepared-before.packet.json', MAX_PACKET),
                _capture_file(case.owner, 'metadata-owner-before.json', 1024 * 1024),
            ]
            _capture_manifest('metadata-export-capture.json', 'before-native-creation', captured)


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
    if _capture_root() is not None:
        captured = [
            _capture_file(receipt, 'metadata-native-source-create-receipt.json', 1024 * 1024),
            _capture_file(output_path, 'metadata-full-union-oracle.json', MAX_PACKET),
        ]
        _capture_manifest('metadata-oracle-capture.json', 'after-native-creation', captured)


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
