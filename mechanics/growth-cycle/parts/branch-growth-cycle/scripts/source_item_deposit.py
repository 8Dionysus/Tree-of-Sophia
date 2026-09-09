"""One granted, bounded, retained local File deposit; never metadata admission.

The caller holds the corpus writer lock. A protected owner-selected companion
to the existing transaction identity owns private command and inode bindings.
The tracked source transaction journal receives only a source-safe receipt.
No payload is removed here,
including on failure or rollback. Same-account hostile writers are not isolated.
"""
from __future__ import annotations

from datetime import datetime, timezone
import copy
import ctypes
import errno
import hashlib
import os
from pathlib import Path
import stat
import struct
import xml.etree.ElementTree as ET
import zipfile
import zlib

import source_commands as source
import source_metadata_transactions as transactions
from source_metadata_snapshot import _identity, _open_owned
from build_source_resource_inventories import build_file_inventory, InventoryBuildError

STAGE_FILE = 'item-deposit.json'
SCHEMA = 'tos_item_deposit_stage_v1'
MAX_BYTES = 512 * 1024 * 1024
CHUNK = 1024 * 1024
MAX_ZIP_MEMBERS = 2048
MAX_MEMBER_BYTES = 16 * 1024 * 1024
MAX_EXPANDED_BYTES = 64 * 1024 * 1024
MAX_ZIP_DIRECTORY_BYTES = 1024 * 1024
PRIVATE_KEYS = {'payload_root', 'input_path', 'payload_authority_ref', 'payload_expires_at', 'recovery_root'}


def destination(config):
    return (Path(config['payload_root']) / Path(config['item_source_path']).parent.relative_to('ToS/source-witnesses')
            / 'payload' / config['payload_basename'])


def validate_config(config):
    for key in ('input_path', 'payload_root', 'recovery_root'):
        value = config[key]
        path = Path(value)
        if (not isinstance(value, str) or not path.is_absolute() or path.as_posix() != value
                or '..' in path.parts or '\\' in value or '\x00' in value):
            raise PermissionError('deposit requires explicit canonical absolute owner input and payload roots')
    if (not isinstance(config['payload_authority_ref'], str) or not config['payload_authority_ref'].strip()
            or source._instant(config['payload_expires_at']) <= datetime.now(timezone.utc)
            or type(config['byte_size']) is not int or not 1 <= config['byte_size'] <= MAX_BYTES):
        raise PermissionError('the separately scoped payload authority is missing, expired or over budget')
    descriptor = _open_owned(Path(config['payload_root']), directory=True)
    os.close(descriptor)
    recovery = Path(config['recovery_root'])
    source_root = Path(config['source_root'])
    payload_root = Path(config['payload_root'])
    excluded = [source_root, payload_root, Path(config['input_path'])]
    if payload_root.parts[-2:] == ('ToS', 'source-witnesses'):
        excluded.append(payload_root.parents[1])
    if any(recovery.is_relative_to(path) or path.is_relative_to(recovery) for path in excluded):
        raise PermissionError('private recovery control must be disjoint from both source checkouts, payload and input')
    descriptor = _open_owned(recovery, directory=True)
    try:
        info = os.fstat(descriptor)
        if info.st_uid != os.getuid() or info.st_mode & 0o077:
            raise PermissionError('private recovery root must be current-account-owned mode 0700')
    finally:
        os.close(descriptor)
    if destination(config) == Path(config['input_path']):
        raise PermissionError('deposit must preserve a distinct original input')


def _pins(path):
    result = {}
    descriptor = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    parent = Path('/')
    try:
        for part in Path(path).parent.parts[1:]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
            parent /= part
            info = os.fstat(descriptor)
            sticky = info.st_uid == 0 and info.st_mode & stat.S_ISVTX
            if info.st_uid not in (0, os.getuid()) or info.st_mode & 0o022 and not sticky:
                raise PermissionError('source ancestor is not a protected owner path')
            result[str(parent)] = [info.st_dev, info.st_ino, info.st_mode, info.st_uid]
    finally:
        os.close(descriptor)
    return result


def _file(path):
    descriptor = _open_owned(path)
    info = os.fstat(descriptor)
    if info.st_uid != os.getuid() or info.st_nlink != 1:
        os.close(descriptor)
        raise PermissionError('input/deposit must be a single-link regular file owned by the current account')
    return descriptor, info


def observe(config, *, inventory=True):
    """Read one unchanged protected input; enumeration reuses the owner library."""
    started_at = datetime.now(timezone.utc).isoformat()
    validate_config(config)
    path = Path(config['input_path'])
    parents = _pins(path)
    descriptor, before = _file(path)
    try:
        if before.st_size != config['byte_size']:
            raise source.JournalConflict('input byte size differs from the exact grant')
        digest = hashlib.sha256()
        total = 0
        while chunk := os.read(descriptor, CHUNK):
            total += len(chunk)
            if total > config['byte_size']:
                raise source.JournalConflict('input exceeds its exact byte budget')
            digest.update(chunk)
        if digest.hexdigest() != config['sha256'] or total != config['byte_size']:
            raise source.JournalConflict('input fixity differs from its exact grant')
        value, limitation = None, None
        if inventory:
            try:
                _inventory_budget(path, config)
                value = build_file_inventory(path, payload_entry(config))
                if len(source._canonical(value)) > 256 * 1024:
                    value = None
                    raise InventoryBuildError('inventory exceeds its bounded command-carrier budget')
            except (InventoryBuildError, OSError, ValueError, ET.ParseError, zipfile.BadZipFile, RuntimeError, zlib.error) as error:
                # Never return parser errors containing a private input path.
                limitation = 'inventory-unavailable:' + type(error).__name__
        if _identity(os.fstat(descriptor)) != _identity(before) or _pins(path) != parents:
            raise source.JournalConflict('input or an input ancestor changed during observation')
        current, current_info = _file(path)
        os.close(current)
        if _identity(current_info) != _identity(before):
            raise source.JournalConflict('input was replaced during observation')
        return {'input_identity': list(_identity(before)), 'input_parents': parents,
                'inventory': value, 'limitation': limitation,
                'observation_interval': {'started_at': started_at,
                                         'ended_at': datetime.now(timezone.utc).isoformat()}}
    finally:
        os.close(descriptor)


def _inventory_budget(path, config):
    """The first native adoption supports only bounded EPUB enumeration.

    Other existing inventory profiles remain available to their legacy owner;
    this adapter does not imply those parsers have resource bounds they lack.
    """
    if config['media_type'] != 'application/epub+zip':
        raise InventoryBuildError('native adoption needs a bounded supported inventory profile')
    # Bound central-directory parsing itself before ZipFile allocates one
    # object per advertised member. ZIP64/multi-disk are a separate route.
    with path.open('rb') as stream:
        stream.seek(max(0, config['byte_size'] - 65557))
        tail = stream.read(65557)
    offset = tail.rfind(b'PK\x05\x06')
    if offset < 0 or offset + 22 > len(tail):
        raise InventoryBuildError('EPUB lacks a bounded ordinary ZIP end record')
    end_record_offset = max(0, config['byte_size'] - 65557) + offset
    if end_record_offset >= 20:
        # Python's ZIP reader consults this locator even when the ordinary
        # EOCD advertises small non-sentinel fields. Reject before ZipFile can
        # replace our bounded count/directory size with ZIP64 values. With a
        # maximum-length comment the locator is just outside the retained tail.
        with path.open('rb') as stream:
            stream.seek(end_record_offset - 20)
            locator = stream.read(20)
        if locator.startswith(b'PK\x06\x07'):
            raise InventoryBuildError('ZIP64 continuation is outside the native bounded EPUB profile')
    _, disk, directory_disk, disk_count, count, size, _, comment = struct.unpack('<4s4H2IH', tail[offset:offset + 22])
    if (disk or directory_disk or disk_count != count or count > MAX_ZIP_MEMBERS
            or size > MAX_ZIP_DIRECTORY_BYTES or offset + 22 + comment != len(tail)):
        raise InventoryBuildError('EPUB directory/member count exceeds the native bounded profile')
    with zipfile.ZipFile(path) as archive:
        members = archive.infolist()
        if (len(members) > MAX_ZIP_MEMBERS or len({entry.filename for entry in members}) != len(members)
                or sum(entry.file_size for entry in members) > MAX_EXPANDED_BYTES
                or any(entry.file_size > MAX_MEMBER_BYTES or entry.flag_bits & 1
                       or entry.compress_type not in {zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED}
                       or entry.filename.startswith('/') or '..' in Path(entry.filename).parts
                       or '\\' in entry.filename for entry in members)):
            raise InventoryBuildError('EPUB member count, paths, compression, encryption or expanded-size budget exceeded')
        # Central-directory sizes alone are not the evidence: stream every
        # member through a bounded decompressor before the owner enumerator.
        expanded = 0
        for entry in members:
            count = 0
            with archive.open(entry) as stream:
                while chunk := stream.read(CHUNK):
                    count += len(chunk)
                    expanded += len(chunk)
                    if count > MAX_MEMBER_BYTES or expanded > MAX_EXPANDED_BYTES:
                        raise InventoryBuildError('EPUB actual decompression budget exceeded')


def payload_entry(config, *, verified_at=None):
    result = {'file_id': config['file_id'], 'relative_path': 'payload/' + config['payload_basename'],
        'original_basename': config['original_basename'], 'media_type': config['media_type'],
        'byte_size': config['byte_size'], 'sha256': config['sha256']}
    if verified_at is not None:
        result['fixity_verified_at'] = verified_at
    return result


def _companion(config, identifier, *, create=False):
    transactions._identifier(identifier)
    root = Path(config['recovery_root'])
    descriptor = _open_owned(root, directory=True)
    try:
        info = os.fstat(descriptor)
        if info.st_uid != os.getuid() or info.st_mode & 0o077:
            raise PermissionError('private recovery root must remain account-owned mode 0700')
        if create:
            try:
                os.mkdir(identifier[7:], 0o700, dir_fd=descriptor)
                os.fsync(descriptor)
            except FileExistsError:
                pass
        child = _open_owned(root / identifier[7:], directory=True)
        info = os.fstat(child)
        if info.st_uid != os.getuid() or info.st_mode & 0o077:
            os.close(child)
            raise PermissionError('private recovery companion must remain account-owned mode 0700')
        return child
    finally:
        os.close(descriptor)


def read_stage(config, identifier):
    try:
        directory = _companion(config, identifier)
    except FileNotFoundError:
        return None
    try:
        try:
            info = os.stat(STAGE_FILE, dir_fd=directory, follow_symlinks=False)
        except FileNotFoundError:
            info = None
        if info is not None and (info.st_uid != os.getuid() or info.st_mode & 0o077 or not stat.S_ISREG(info.st_mode)):
            raise PermissionError('private continuation must remain an account-owned mode 0600 regular file')
        raw = transactions._read_at(directory, STAGE_FILE, source.MAX_SET_BYTES)
        stage = source._json_object(raw) if raw is not None else None
        if stage is not None and (stage.get('schema_version') != SCHEMA or stage.get('transaction_id') != identifier):
            raise source.JournalCorruption('invalid retained Item deposit stage')
        return stage
    finally:
        os.close(directory)


def _save(config, identifier, stage, *, initial=False, previous=None):
    directory = _companion(config, identifier, create=True)
    try:
        if not initial and (previous is None or read_stage(config, identifier) != previous):
            raise source.JournalConflict('private continuation changed before its exact stage transition')
        transactions._atomic_write(directory, STAGE_FILE, source._canonical(stage) + b'\n', no_replace=initial)
    finally:
        os.close(directory)


def _mkdirs(config, target):
    root = Path(config['payload_root'])
    descriptor = _open_owned(root, directory=True)
    try:
        for part in target.parent.relative_to(root).parts:
            try:
                os.mkdir(part, 0o700, dir_fd=descriptor)
                os.fsync(descriptor)
            except FileExistsError:
                pass
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor)
            info = os.fstat(child)
            if info.st_uid != os.getuid() or info.st_mode & 0o022:
                os.close(child)
                raise PermissionError('payload destination parent is not protected account-owned storage')
            os.close(descriptor)
            descriptor = child
    finally:
        os.close(descriptor)


def _inode(info):
    return [info.st_dev, info.st_ino, info.st_mode, info.st_uid]


def verify_deposit(config, stage):
    """Require the retained deposited inode and exact bytes, never equal foreign data."""
    target = destination(config)
    if _pins(target) != stage['target_parents']:
        raise source.JournalConflict('a deposited payload ancestor was replaced')
    descriptor, before = _file(target)
    try:
        if _inode(before) != stage['payload_inode'] or before.st_size != config['byte_size']:
            raise source.JournalConflict('deposited payload is a foreign or changed file')
        digest = hashlib.sha256()
        total = 0
        while chunk := os.read(descriptor, CHUNK):
            total += len(chunk)
            if total > config['byte_size']:
                raise source.JournalConflict('deposited payload exceeded its exact bound')
            digest.update(chunk)
        if digest.hexdigest() != config['sha256'] or _identity(before) != _identity(os.fstat(descriptor)):
            raise source.JournalConflict('deposited payload fixity changed')
        current, info = _file(target)
        os.close(current)
        if _identity(info) != _identity(before) or _pins(target) != stage['target_parents']:
            raise source.JournalConflict('deposited payload was replaced while verifying')
    finally:
        os.close(descriptor)


def ensure_deposit(config, request, identifier, *, authorize, recovery=False):
    """Retain/resume one prefix-checked copy, publishing by no-replace rename.

    A crash before an inode binding is durable is an explicit conflict, not an
    excuse to adopt a same-hash foreign target. The original is never written.
    """
    authorize()
    observed = observe(config)
    # Only the first actual observation interval belongs to this transaction.
    # A later verification changes the clock, not the stable evidence binding.
    observation_interval = observed.pop('observation_interval')
    authorize()
    if observed['inventory'] != request['inventory'] or observed['limitation'] != request['inventory_limitation']:
        raise source.JournalConflict('prepared inventory observation changed')
    target = destination(config)
    stage = read_stage(config, identifier)
    binding = {'request': request, 'configuration': config}
    if stage is not None and stage.get('binding') != binding:
        original = stage['binding']['configuration']
        renewable = {'principal_id', 'maker_type', 'authority_ref', 'expires_at', 'allowed_operations',
                     'payload_authority_ref', 'payload_expires_at'}
        if (not recovery or stage['binding']['request'] != request
                or source._digest(source._canonical(original)) != request['expected_configuration']
                or any(config[key] != original[key] for key in config if key not in renewable and not key.startswith('allowed_'))):
            raise source.JournalConflict('deposit journal belongs to another exact command or destination')
    if stage is None:
        if os.path.lexists(target):
            raise source.JournalConflict('deposit destination is already occupied; equal hashes do not authorize adoption')
        stage = {'schema_version': SCHEMA, 'transaction_id': identifier, 'binding': binding,
            'observation': observed, 'observation_interval': observation_interval,
            'state': 'prepared', 'created_at': datetime.now(timezone.utc).isoformat(),
            'payload_inode': None, 'target_parents': None, 'deposited_at': None, 'recovery_authorization': None}
        _save(config, identifier, stage, initial=True)
    elif stage['observation'] != observed:
        raise source.JournalConflict('the exact original source identity or observation changed before deposit recovery')
    if stage['state'] == 'rolled-back-retained':
        raise source.JournalConflict('this deposit was explicitly rolled back with bytes retained')
    if recovery:
        previous = copy.deepcopy(stage)
        stage['recovery_authorization'] = {'owner_configuration': source._digest(source._canonical(config)),
            'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'],
            'authorized_at': datetime.now(timezone.utc).isoformat(), 'decision': 'resume'}
        _save(config, identifier, stage, previous=previous)
    if stage['state'] == 'deposited':
        verify_deposit(config, stage)
        return stage
    authorize()
    _mkdirs(config, target)
    parents = _pins(target)
    partial = target.with_name('.item-' + identifier[7:] + '.partial')
    directory = _open_owned(target.parent, directory=True)
    try:
        authorize()
        if stage['state'] == 'prepared':
            if os.path.lexists(target):
                raise source.JournalConflict('an unrelated target appeared before deposit')
            descriptor = os.open(partial.name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=directory)
            try:
                previous = copy.deepcopy(stage)
                stage.update(state='copying', payload_inode=_inode(os.fstat(descriptor)), target_parents=parents)
                os.fsync(descriptor)
                os.fsync(directory)
                _save(config, identifier, stage, previous=previous)
            finally:
                os.close(descriptor)
        if stage['target_parents'] != parents:
            raise source.JournalConflict('deposit destination ancestors changed')
        if os.path.lexists(target):
            # Publication may have completed just before the stage update.
            verify_deposit(config, stage)
        else:
            descriptor = os.open(partial.name, os.O_RDWR | os.O_NOFOLLOW, dir_fd=directory)
            original, original_info = _file(Path(config['input_path']))
            try:
                info = os.fstat(descriptor)
                if (_inode(info) != stage['payload_inode'] or not stat.S_ISREG(info.st_mode) or info.st_nlink != 1
                        or info.st_size > config['byte_size'] or list(_identity(original_info)) != observed['input_identity']):
                    raise source.JournalConflict('retained partial or original source was replaced')
                offset, digest = 0, hashlib.sha256()
                while chunk := os.read(original, CHUNK):
                    authorize()
                    if offset + len(chunk) > config['byte_size']:
                        raise source.JournalConflict('input grew beyond its grant during copying')
                    if offset < info.st_size:
                        prefix = min(len(chunk), info.st_size - offset)
                        if os.read(descriptor, prefix) != chunk[:prefix]:
                            raise source.JournalConflict('retained partial differs from the exact input prefix')
                    else:
                        prefix = 0
                    remainder = memoryview(chunk)[prefix:]
                    while remainder:
                        written = os.write(descriptor, remainder)
                        if written == 0:
                            raise OSError('zero-length deposit write')
                        remainder = remainder[written:]
                    digest.update(chunk)
                    offset += len(chunk)
                if (offset != config['byte_size'] or digest.hexdigest() != config['sha256']
                        or list(_identity(os.fstat(original))) != observed['input_identity']
                        or _pins(Path(config['input_path'])) != observed['input_parents']):
                    raise source.JournalConflict('original source changed during copying')
                check, current_info = _file(Path(config['input_path']))
                os.close(check)
                if list(_identity(current_info)) != observed['input_identity'] or _pins(target) != parents:
                    raise source.JournalConflict('source or destination namespace changed during copying')
                authorize()
                os.fsync(descriptor)
                _publish(directory, partial.name, target.name)
                os.fsync(directory)
            finally:
                os.close(original)
                os.close(descriptor)
            verify_deposit(config, stage)
        previous = copy.deepcopy(stage)
        stage.update(state='deposited', deposited_at=datetime.now(timezone.utc).isoformat())
        _save(config, identifier, stage, previous=previous)
        return stage
    finally:
        os.close(directory)


def _publish(directory, partial, target):
    libc = ctypes.CDLL(None, use_errno=True)
    try:
        rename = libc.renameat2
    except AttributeError:
        raise OSError(errno.ENOSYS, 'no-replace payload publication is unavailable') from None
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    if rename(directory, os.fsencode(partial), directory, os.fsencode(target), 1) != 0:
        code = ctypes.get_errno()
        if code == errno.EEXIST:
            raise source.JournalConflict('an unrelated payload target appeared before publication')
        raise OSError(code, os.strerror(code))


def rollback_retained(config, identifier):
    stage = read_stage(config, identifier)
    if stage is None:
        raise source.JournalConflict('no exact retained Item deposit is selected')
    if stage['state'] == 'deposited':
        verify_deposit(config, stage)
    previous = copy.deepcopy(stage)
    stage['state'] = 'rolled-back-retained'
    _save(config, identifier, stage, previous=previous)
    return public_state(stage)


def public_state(stage):
    return {'transaction_id': stage['transaction_id'], 'state': stage['state'],
        'recovery_handle': {'transaction_id': stage['transaction_id'],
                            'owner_configuration': source._digest(source._canonical(stage['binding']['configuration']))},
        'file': payload_entry(stage['binding']['configuration']),
        'inventory_limitation': stage['observation']['limitation'], 'original_preserved': True,
        'metadata_committed': False, 'grants_admission': False}


def public_receipt(stage):
    if stage['state'] != 'deposited':
        raise source.JournalConflict('only a completed deposit can yield a public byte receipt')
    return {'schema_version': 'tos_item_deposit_receipt_v1', 'transaction_id': stage['transaction_id'],
        'owner_configuration': source._digest(source._canonical(stage['binding']['configuration'])),
        'private_stage_digest': source._digest(source._canonical(stage)),
        'recovery_configuration': stage['recovery_authorization']['owner_configuration'] if stage['recovery_authorization'] else None,
        'file': payload_entry(stage['binding']['configuration']), 'started_at': stage['created_at'],
        'observation_interval': {key: stage['observation_interval'][key] for key in ('started_at', 'ended_at')},
        'deposited_at': stage['deposited_at'], 'original_preserved': True,
        'metadata_committed': False, 'grants_admission': False}


def validate_public_receipt(value, scope, request, identifier):
    source._keys(value, {'schema_version', 'transaction_id', 'owner_configuration', 'private_stage_digest',
        'recovery_configuration', 'file', 'started_at', 'deposited_at', 'observation_interval',
        'original_preserved', 'metadata_committed', 'grants_admission'})
    if (value['schema_version'] != 'tos_item_deposit_receipt_v1' or value['transaction_id'] != identifier
            or value['owner_configuration'] != request['expected_configuration']
            or value['file'] != payload_entry(scope) or value['original_preserved'] is not True
            or value['metadata_committed'] is not False or value['grants_admission'] is not False):
        raise source.JournalCorruption('public byte receipt is outside the exact bounded File deposit')
    transactions._identifier(value['private_stage_digest'])
    if value['recovery_configuration'] is not None:
        transactions._identifier(value['recovery_configuration'])
    interval = value['observation_interval']
    source._keys(interval, {'started_at', 'ended_at'})
    if not (source._instant(interval['started_at']) <= source._instant(interval['ended_at'])
            <= source._instant(value['started_at']) <= source._instant(value['deposited_at'])):
        raise source.JournalCorruption('observation and byte deposit timestamps are reversed')
