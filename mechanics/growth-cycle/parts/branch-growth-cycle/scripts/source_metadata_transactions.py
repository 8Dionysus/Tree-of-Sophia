"""Internal selected-metadata publication and exact owner-authorized recovery.

Callers MUST hold the existing ``historical-create`` corpus writer lock for
apply/resume/rollback. No CLI, source discovery, schema admission, descendant
copying, implicit recovery or orphan cleanup is provided here. The adapter
owns the meaning and exact authority of its plan; this library owns its bounded
byte movement. See ../docs/SELECTED_METADATA_TRANSACTIONS.md.
"""
from __future__ import annotations

import copy
import ctypes
import errno
import os
from pathlib import Path
import stat
import sys
import uuid

SCRIPTS = Path(__file__).resolve().parents[5] / 'scripts'
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))
from source_metadata_snapshot import (
    PublicationSnapshot, PublicationStateError, PublicationChanged,
    CONTROL_REF, TRANSACTIONS_REF, SOURCE_HOME, STATE_SCHEMA, MAX_GENERATION,
    MAX_RECOVERY_AUTHORIZATION_BYTES,
    HASH, _canonical, _digest, _json, _open_owned, _read_owned, _validate_state,
    read_publication_state,
)

MAX_FILES = 64
MAX_DIRECTORIES = 64
MAX_SIDE_BYTES = 8 * 1024 * 1024
MAX_AUTHORIZATION_BYTES = 64 * 1024
MAX_MANIFEST_BYTES = 512 * 1024
MANIFEST_SCHEMA = 'tos_selected_metadata_transaction_v1'
PROFILED_MANIFEST_SCHEMA = 'tos_selected_metadata_transaction_v2'
ITEM_PATH_PROFILE_SCHEMA = 'tos_item_metadata_paths_v1'
ITEM_AUTHORIZATION_SCHEMA = 'tos_item_adoption_authorization_v1'
COMPLETION_SCHEMA = 'tos_selected_metadata_completion_v1'
FORBIDDEN = {'payload', 'private', 'local-content', 'owner-local', 'catalog'}


class TransactionConflict(ValueError):
    """The exact caller plan no longer matches the selected owner state."""


class TransactionCorruption(ValueError):
    """Retained transaction evidence cannot bind its declared bytes/state."""


def _identifier(value):
    if not isinstance(value, str) or not HASH.fullmatch(value):
        raise ValueError('transaction identity must be an exact sha256 identifier')
    return value


def _path(value, *, directory=False, companions=()):
    if not isinstance(value, str) or not value or len(value.encode('utf-8')) > 1024:
        raise PermissionError('selected metadata path exceeds its explicit path contract')
    path = Path(value)
    if (path.is_absolute() or path.as_posix() != value or '\\' in value or '\x00' in value
            or path.parts[:2] != SOURCE_HOME.parts or not 3 <= len(path.parts) <= 24
            or any(part in FORBIDDEN or part.startswith('.') for part in path.parts)
            or not directory and (len(path.parts) < 4
                                  or path.suffix not in {'.json', '.jsonl'} and path not in companions)):
        raise PermissionError('selected path is outside explicit public source metadata')
    return path


def _profile_companions(plan):
    """Validate a versioned additive exception, never a general suffix grant."""
    if 'path_profile' not in plan:
        return ()
    profile, authorization = plan['path_profile'], plan['authorization']
    if (not isinstance(profile, dict) or set(profile) != {'schema_version', 'item_source_path'}
            or profile['schema_version'] != ITEM_PATH_PROFILE_SCHEMA):
        raise ValueError('invalid selected Item metadata path profile')
    item_path = _path(profile['item_source_path'])
    if item_path.name != 'item.json' or item_path.parent.parent.name != 'items':
        raise PermissionError('Item metadata profile must name one exact items home record')
    if (not isinstance(authorization, dict)
            or authorization.get('schema_version') != ITEM_AUTHORIZATION_SCHEMA
            or not isinstance(authorization.get('scope'), dict)
            or authorization['scope'].get('item_source_path') != profile['item_source_path']):
        raise PermissionError('Item metadata profile differs from its exact adoption authorization scope')
    return (item_path.with_name('fixity.sha256'), item_path.with_name('forensic-report.md'))


def _binding(raw):
    return None if raw is None else {'sha256': _digest(raw), 'bytes': len(raw)}


def _validate_summary(summary):
    if (not isinstance(summary, dict)
            or set(summary) not in ({'authorization', 'files', 'new_directories'},
                                    {'authorization', 'files', 'new_directories', 'path_profile'})
            or not isinstance(summary['authorization'], dict)
            or not isinstance(summary['files'], list) or not 1 <= len(summary['files']) <= MAX_FILES
            or not isinstance(summary['new_directories'], list)
            or len(summary['new_directories']) > MAX_DIRECTORIES):
        raise ValueError('invalid bounded selected-metadata plan')
    if len(_canonical(summary['authorization'])) > MAX_AUTHORIZATION_BYTES:
        raise ValueError('transaction authorization bindings exceed their byte budget')
    companions = _profile_companions(summary)
    directories = [_path(value, directory=True) for value in summary['new_directories']]
    if len(set(directories)) != len(directories):
        raise ValueError('duplicate declared new directory')
    paths, sums = [], {'before': 0, 'after': 0}
    changed = False
    for item in summary['files']:
        if not isinstance(item, dict) or set(item) != {'path', 'before', 'after'}:
            raise ValueError('invalid selected-file binding')
        paths.append(_path(item['path'], companions=companions))
        if item['before'] is None and item['after'] is None:
            raise ValueError('an absent-to-absent path is not a selected file')
        changed |= item['before'] != item['after']
        for side in sums:
            ref = item[side]
            if ref is not None:
                if (not isinstance(ref, dict) or set(ref) != {'sha256', 'bytes'}
                        or type(ref['bytes']) is not int or not 0 <= ref['bytes'] <= MAX_SIDE_BYTES):
                    raise ValueError('invalid selected-file byte binding')
                _identifier(ref['sha256'])
                sums[side] += ref['bytes']
    if not changed or any(size > MAX_SIDE_BYTES for size in sums.values()):
        raise ValueError('selected metadata side exceeds its byte budget or has no change')
    if len(set(paths)) != len(paths):
        raise ValueError('duplicate selected metadata path')
    if any(left == right or left in right.parents for left in paths
           for right in [*paths, *directories] if left is not right):
        raise ValueError('selected files collide with an ancestor/descendant target')
    for directory in directories:
        if not any(directory in path.parents and item['before'] is None
                   for path, item in zip(paths, summary['files'])):
            raise ValueError('a new directory must contain an explicitly new selected file')
    if summary['new_directories'] != sorted(summary['new_directories'], key=lambda ref: (len(Path(ref).parts), ref)):
        raise ValueError('new directories must be in canonical parent-first order')
    if [item['path'] for item in summary['files']] != sorted(item['path'] for item in summary['files']):
        raise ValueError('selected files must be in canonical path order')


def _freeze_plan(plan):
    if (not isinstance(plan, dict)
            or set(plan) not in ({'authorization', 'files', 'new_directories'},
                                {'authorization', 'files', 'new_directories', 'path_profile'})):
        raise ValueError('plan fields do not match the selected-metadata protocol')
    if not isinstance(plan['files'], list) or not 1 <= len(plan['files']) <= MAX_FILES:
        raise ValueError('selected-file count exceeds its bounded contract')
    # Freeze caller-owned evidence, including the profile, before using its scope.
    authorization = _json(_canonical(plan['authorization']))
    summary = {'authorization': authorization}
    if 'path_profile' in plan:
        summary['path_profile'] = _json(_canonical(plan['path_profile']))
    companions = _profile_companions(summary)
    blobs, files = {}, []
    for item in plan['files']:
        if not isinstance(item, dict) or set(item) != {'path', 'before', 'after'}:
            raise ValueError('invalid selected-file proposal')
        _path(item['path'], companions=companions)
        result = {'path': item['path']}
        for side in ('before', 'after'):
            raw = item[side]
            if raw is not None and (not isinstance(raw, bytes) or len(raw) > MAX_SIDE_BYTES):
                raise ValueError('selected-file content must be bounded bytes or absent')
            result[side] = _binding(raw)
            if raw is not None:
                blobs[_digest(raw)] = raw
        files.append(result)
    if not isinstance(plan['new_directories'], list):
        raise ValueError('new directories must be explicitly listed')
    for ref in plan['new_directories']:
        _path(ref, directory=True)
    summary.update(files=sorted(files, key=lambda item: item['path']),
                   new_directories=sorted(plan['new_directories'], key=lambda ref: (len(Path(ref).parts), ref)))
    _validate_summary(summary)
    return summary, blobs


def _authorize(guard, summary):
    if not callable(guard) or guard(copy.deepcopy(summary['authorization']), copy.deepcopy(summary)) is not True:
        raise PermissionError('current owner authorization/dependencies do not permit this exact transaction')


def _directory_binding(info):
    return {'device': info.st_dev, 'inode': info.st_ino, 'mode': info.st_mode, 'uid': info.st_uid}


def _parent_refs(summary):
    refs = {SOURCE_HOME.as_posix()}
    for value in [*(item['path'] for item in summary['files']), *summary['new_directories']]:
        parent = Path(value).parent
        while parent.is_relative_to(SOURCE_HOME):
            refs.add(parent.as_posix())
            parent = parent.parent
    return sorted(refs, key=lambda ref: (len(Path(ref).parts), ref))


def _capture_parents(root, summary):
    parents = {}
    new = set(summary['new_directories'])
    for ref in _parent_refs(summary):
        try:
            fd = _open_owned(root / ref, directory=True)
        except FileNotFoundError:
            if ref not in new:
                raise TransactionConflict('an undeclared source parent directory is absent') from None
            parents[ref] = None
        else:
            try:
                if ref in new:
                    raise TransactionConflict('an explicitly new directory already exists')
                parents[ref] = _directory_binding(os.fstat(fd))
            finally:
                os.close(fd)
    # A terminal new directory need not itself be a parent of another path.
    for ref in new:
        if ref not in parents:
            try:
                fd = _open_owned(root / ref, directory=True)
            except FileNotFoundError:
                continue
            else:
                os.close(fd)
                raise TransactionConflict('an explicitly new directory already exists')
    return parents


class _Parents:
    def __init__(self, root, manifest):
        self.root, self.manifest, self.opened = root, manifest, {}

    def __enter__(self):
        self.verify()
        return self

    def __exit__(self, *_):
        for descriptor in self.opened.values():
            os.close(descriptor)

    def get(self, ref):
        if ref not in self.opened:
            descriptor = _open_owned(self.root / ref, directory=True)
            observed = _directory_binding(os.fstat(descriptor))
            expected = self.manifest['parents'].get(ref)
            if expected is not None and observed != expected:
                os.close(descriptor)
                raise TransactionConflict('a selected source parent was replaced or repermissioned')
            self.opened[ref] = descriptor
        return self.opened[ref]

    def verify(self):
        for ref, expected in self.manifest['parents'].items():
            try:
                current = _open_owned(self.root / ref, directory=True)
            except FileNotFoundError:
                if expected is not None or ref in self.opened:
                    raise TransactionConflict('a selected source parent disappeared') from None
                continue
            try:
                observed = _directory_binding(os.fstat(current))
                if ((expected is not None and observed != expected)
                        or ref in self.opened and observed != _directory_binding(os.fstat(self.opened[ref]))):
                    raise TransactionConflict('a selected source parent changed')
            finally:
                os.close(current)


def _read_at(parent, name, limit=MAX_SIDE_BYTES):
    try:
        descriptor = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent)
    except FileNotFoundError:
        return None
    with os.fdopen(descriptor, 'rb') as stream:
        before = os.fstat(stream.fileno())
        if (not stat.S_ISREG(before.st_mode) or before.st_uid not in (0, os.getuid())
                or before.st_mode & 0o022):
            raise PermissionError('selected file is not protected regular metadata')
        if before.st_size > limit:
            raise TransactionConflict('selected file exceeds the expected byte budget')
        raw = stream.read(limit + 1)
        after = os.fstat(stream.fileno())
    fields = lambda info: (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_size,
                           info.st_mtime_ns, info.st_ctime_ns)
    if len(raw) > limit or fields(before) != fields(after):
        raise TransactionConflict('selected file changed while reading')
    try:
        current = os.stat(name, dir_fd=parent, follow_symlinks=False)
    except FileNotFoundError:
        raise TransactionConflict('selected file disappeared while reading') from None
    if fields(current) != fields(before):
        raise TransactionConflict('selected file was replaced while reading')
    return raw


def _selected(parents, item):
    path = Path(item['path'])
    try:
        parent = parents.get(path.parent.as_posix())
    except FileNotFoundError:
        if path.parent.as_posix() not in parents.manifest['plan']['new_directories']:
            raise TransactionConflict('a selected source parent is unexpectedly absent') from None
        return None
    limit = max((ref['bytes'] for ref in (item['before'], item['after']) if ref is not None), default=0)
    return _read_at(parent, path.name, limit)


def _check_files(parents, *, side=None):
    parents.verify()
    for item in parents.manifest['plan']['files']:
        current = _binding(_selected(parents, item))
        if (current != item[side] if side else current not in (item['before'], item['after'])):
            raise TransactionConflict('a selected file is neither the exact permitted before nor after state')


def _new_state(manifest, manifest_digest, phase, outcome=None, recovery_authorization=None):
    generation = manifest['base_publication']['generation'] + (1 if phase == 'pending' else 2)
    if generation > MAX_GENERATION:
        raise ValueError('publication generation capacity is exhausted')
    value = {'schema_version': STATE_SCHEMA, 'generation': generation, 'transition_id': uuid.uuid4().hex,
             'phase': phase, 'transaction_id': manifest['transaction_id'], 'manifest_sha256': manifest_digest,
             'outcome': outcome, 'recovery_authorization': recovery_authorization}
    return _validate_state({**value, 'token': _digest(_canonical(value))})


def _atomic_write(parent, name, raw, *, no_replace=False):
    temporary = '.metadata-' + uuid.uuid4().hex + '.pending'
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=parent)
    try:
        with os.fdopen(descriptor, 'wb') as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        if no_replace:
            libc = ctypes.CDLL(None, use_errno=True)
            try:
                rename = libc.renameat2
            except AttributeError:
                raise OSError(errno.ENOSYS, 'no-replace metadata publication is unavailable') from None
            rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
            rename.restype = ctypes.c_int
            if rename(parent, os.fsencode(temporary), parent, os.fsencode(name), 1) != 0:
                code = ctypes.get_errno()
                if code == errno.EEXIST:
                    raise TransactionConflict('an absent selected file was concurrently created')
                raise OSError(code, os.strerror(code))
        else:
            os.replace(temporary, name, src_dir_fd=parent, dst_dir_fd=parent)
        os.fsync(parent)
    finally:
        # Only this invocation's unpublished temporary file. Abrupt loss leaves
        # it retained; future invocations do not enumerate or clean orphans.
        try:
            os.unlink(temporary, dir_fd=parent)
        except FileNotFoundError:
            pass


def _immutable(parent, name, raw):
    existing = _read_at(parent, name, max(len(raw), 8192))
    if existing is not None:
        if existing != raw:
            raise TransactionCorruption('retained transaction member differs from its exact bytes')
        return
    _atomic_write(parent, name, raw, no_replace=True)


def _journal_dir(root, transaction_id, *, create=False):
    home = _open_owned(root / SOURCE_HOME, directory=True)
    try:
        if create:
            try:
                os.mkdir(TRANSACTIONS_REF.name, 0o700, dir_fd=home)
                os.fsync(home)
            except FileExistsError:
                pass
        journal = _open_owned(root / TRANSACTIONS_REF, directory=True)
    finally:
        os.close(home)
    try:
        name = _identifier(transaction_id)[7:]
        if create:
            try:
                os.mkdir(name, 0o700, dir_fd=journal)
                os.fsync(journal)
            except FileExistsError:
                pass
        return _open_owned(root / TRANSACTIONS_REF / name, directory=True)
    finally:
        os.close(journal)


def _retain(root, manifest, blobs):
    raw = _canonical(manifest) + b'\n'
    if len(raw) > MAX_MANIFEST_BYTES:
        raise ValueError('transaction manifest exceeds its byte budget')
    directory = _journal_dir(root, manifest['transaction_id'], create=True)
    try:
        for digest, content in sorted(blobs.items()):
            _immutable(directory, digest[7:] + '.blob', content)
        _immutable(directory, 'manifest.json', raw)
        os.fsync(directory)
    finally:
        os.close(directory)
    return _digest(raw)


def _validate_manifest(manifest, transaction_id):
    if (not isinstance(manifest, dict)
            or set(manifest) != {'schema_version', 'transaction_id', 'base_publication', 'plan', 'parents'}
            or manifest['schema_version'] not in (MANIFEST_SCHEMA, PROFILED_MANIFEST_SCHEMA)
            or manifest['transaction_id'] != transaction_id):
        raise TransactionCorruption('invalid selected-metadata transaction manifest')
    base = manifest['base_publication']
    if (not isinstance(base, dict) or set(base) != {'token', 'generation'}
            or type(base['generation']) is not int or not 0 <= base['generation'] <= MAX_GENERATION - 2
            or (base['token'] is None) != (base['generation'] == 0)):
        raise TransactionCorruption('invalid transaction publication predecessor')
    if base['token'] is not None:
        _identifier(base['token'])
    _validate_summary(manifest['plan'])
    expected_schema = PROFILED_MANIFEST_SCHEMA if 'path_profile' in manifest['plan'] else MANIFEST_SCHEMA
    if manifest['schema_version'] != expected_schema:
        raise TransactionCorruption('transaction manifest version differs from its path profile grammar')
    parents = manifest['parents']
    if not isinstance(parents, dict) or set(parents) != set(_parent_refs(manifest['plan'])):
        raise TransactionCorruption('transaction parent-directory closure differs')
    for ref, binding in parents.items():
        _path(ref, directory=True) if ref != SOURCE_HOME.as_posix() else None
        if binding is None:
            if ref not in manifest['plan']['new_directories']:
                raise TransactionCorruption('an absent parent was not delegated for creation')
        elif (not isinstance(binding, dict) or set(binding) != {'device', 'inode', 'mode', 'uid'}
                or any(type(value) is not int or value < 0 for value in binding.values())
                or not stat.S_ISDIR(binding['mode']) or binding['mode'] & 0o022
                or binding['uid'] not in (0, os.getuid())):
            raise TransactionCorruption('invalid retained source-parent binding')


def _load_manifest(root, transaction_id):
    transaction_id = _identifier(transaction_id)
    directory = root / TRANSACTIONS_REF / transaction_id[7:]
    raw = _read_owned(directory / 'manifest.json', MAX_MANIFEST_BYTES)
    manifest = _json(raw)
    try:
        _validate_manifest(manifest, transaction_id)
    except (ValueError, TypeError, KeyError) as error:
        raise TransactionCorruption('retained transaction manifest is invalid') from error
    blobs, total = {}, 0
    for item in manifest['plan']['files']:
        for side in ('before', 'after'):
            binding = item[side]
            if binding is None or binding['sha256'] in blobs:
                continue
            total += binding['bytes']
            if total > 2 * MAX_SIDE_BYTES:
                raise TransactionCorruption('retained transaction exceeds the total blob budget')
            try:
                content = _read_owned(directory / (binding['sha256'][7:] + '.blob'), binding['bytes'])
            except FileNotFoundError as error:
                raise TransactionCorruption('retained transaction blob is missing') from error
            if len(content) != binding['bytes'] or _digest(content) != binding['sha256']:
                raise TransactionCorruption('retained transaction blob does not match its exact binding')
            blobs[binding['sha256']] = content
    return manifest, _digest(raw), blobs


def _expanded(manifest, blobs):
    return {**copy.deepcopy(manifest['plan']), 'files': [
        {'path': item['path'], **{side: None if item[side] is None else blobs[item[side]['sha256']]
                                for side in ('before', 'after')}} for item in manifest['plan']['files']]}


def _check_pending(state, manifest, digest):
    if (state is None or state['phase'] != 'pending' or state['transaction_id'] != manifest['transaction_id']
            or state['manifest_sha256'] != digest
            or state['generation'] != manifest['base_publication']['generation'] + 1):
        raise TransactionConflict('control does not select this exact pending transaction')


def read_pending_transaction(root):
    """Return verified selected before/after evidence only for the pending head.

    This read grants nothing. An adapter may use retained original bytes to
    revalidate its record/request before explicitly invoking recovery. Unselected
    orphan directories are not enumerated or treated as pending transactions.
    """
    root = Path(root)
    state = read_publication_state(root)
    if state is None or state['phase'] != 'pending':
        return None
    try:
        manifest, digest, blobs = _load_manifest(root, state['transaction_id'])
    except FileNotFoundError as error:
        raise TransactionCorruption('pending publication manifest is missing') from error
    _check_pending(state, manifest, digest)
    if read_publication_state(root) != state:
        raise PublicationChanged('pending transaction changed while its evidence was read')
    return {'state': copy.deepcopy(state), 'manifest': copy.deepcopy(manifest),
            'plan': _expanded(manifest, blobs), 'writes_to_source': False}


def _completion(root, manifest, digest):
    path = root / TRANSACTIONS_REF / manifest['transaction_id'][7:] / 'completion.json'
    try:
        value = _json(_read_owned(path, 8192))
    except FileNotFoundError:
        return None
    if set(value) != {'schema_version', 'publication'} or value['schema_version'] != COMPLETION_SCHEMA:
        raise TransactionCorruption('invalid retained transaction completion')
    state = _validate_state(value['publication'])
    if (state['phase'] != 'ready' or state['transaction_id'] != manifest['transaction_id']
            or state['manifest_sha256'] != digest
            or state['generation'] != manifest['base_publication']['generation'] + 2):
        raise TransactionCorruption('completion does not bind this exact terminal transaction')
    return state


def _record_completion(root, state):
    """Evidence only, written after ready is durable; never used as pending."""
    directory = _journal_dir(root, state['transaction_id'])
    try:
        _immutable(directory, 'completion.json', _canonical({
            'schema_version': COMPLETION_SCHEMA, 'publication': state}) + b'\n')
    finally:
        os.close(directory)


def inspect_transaction(root, transaction_id):
    """Read exact evidence; distinguish orphan, pending and terminal receipt.

    Historical completion is evidence of a past transport, not verification of
    current selected source bytes or permission to replay it.
    """
    root = Path(root)
    state = read_publication_state(root)
    manifest, digest, blobs = _load_manifest(root, transaction_id)
    completion = _completion(root, manifest, digest)
    selected = state is not None and state['transaction_id'] == transaction_id
    if selected:
        if state['manifest_sha256'] != digest:
            raise TransactionCorruption('publication control and transaction manifest disagree')
        if state['phase'] == 'pending':
            _check_pending(state, manifest, digest)
            if completion is not None:
                raise TransactionCorruption('pending transaction already has terminal evidence')
            status = 'pending'
        else:
            if state['generation'] != manifest['base_publication']['generation'] + 2:
                raise TransactionCorruption('terminal publication generation differs from its predecessor')
            if completion is not None and completion != state:
                raise TransactionCorruption('terminal receipt and current publication disagree')
            completion, status = state, state['outcome']
    else:
        status = completion['outcome'] if completion else 'orphan'
    if read_publication_state(root) != state:
        raise PublicationChanged('transaction publication changed during inspection')
    return {'transaction_id': transaction_id, 'status': status, 'manifest': copy.deepcopy(manifest),
            'manifest_sha256': digest, 'plan': _expanded(manifest, blobs),
            'publication': copy.deepcopy(completion), 'is_current_publication': selected,
            'writes_to_source': False, 'grants_admission': False}


def _publish_state(root, state, expected):
    if read_publication_state(root) != expected:
        raise TransactionConflict('publication control changed outside the corpus writer lock')
    parent = _open_owned(root / SOURCE_HOME, directory=True)
    try:
        _atomic_write(parent, CONTROL_REF.name, _canonical(state) + b'\n', no_replace=expected is None)
    finally:
        os.close(parent)


def _still_pending(root, pending):
    if read_publication_state(root) != pending:
        raise TransactionConflict('pending publication changed outside the corpus writer lock')


def _replace_file(parents, item, raw):
    path = Path(item['path'])
    parent = parents.get(path.parent.as_posix())
    _atomic_write(parent, path.name, raw, no_replace=_selected(parents, item) is None)


def _remove_file(parents, item):
    path = Path(item['path'])
    parent = parents.get(path.parent.as_posix())
    os.unlink(path.name, dir_fd=parent)
    os.fsync(parent)


def _sync_selected(parents):
    """Recovery must re-fsync visible after bytes/renames before ready.

    A prior process may have died after rename/unlink/mkdir but before its
    directory fsync. Merely observing the requested bytes is not durability.
    """
    for item in parents.manifest['plan']['files']:
        path = Path(item['path'])
        try:
            parent = parents.get(path.parent.as_posix())
            descriptor = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent)
        except FileNotFoundError:
            continue
        try:
            if not stat.S_ISREG(os.fstat(descriptor).st_mode):
                raise PermissionError('selected metadata changed file type before durability verification')
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    for descriptor in parents.opened.values():
        os.fsync(descriptor)


def _verify_retained(root, manifest, digest, blobs):
    current, current_digest, current_blobs = _load_manifest(root, manifest['transaction_id'])
    if current != manifest or current_digest != digest or current_blobs != blobs:
        raise TransactionCorruption('retained transaction evidence changed before publication')


def _move(root, manifest, digest, blobs, pending, guard, *, rollback=False, recovery_authorization=None):
    plan = manifest['plan']
    side = 'before' if rollback else 'after'
    _authorize(guard, plan)
    _still_pending(root, pending)
    _verify_retained(root, manifest, digest, blobs)
    with _Parents(root, manifest) as parents:
        _check_files(parents)
        if not rollback:
            for ref in plan['new_directories']:
                _authorize(guard, plan)
                _still_pending(root, pending)
                _check_files(parents)
                path = Path(ref)
                parent = parents.get(path.parent.as_posix())
                try:
                    os.mkdir(path.name, 0o700, dir_fd=parent)
                    os.fsync(parent)
                except FileExistsError:
                    # Exact declared new directories may already exist after
                    # a crash. A symlink/file/unprotected directory still fails.
                    pass
                parents.get(ref)
        for item in plan['files']:
            _authorize(guard, plan)
            _still_pending(root, pending)
            _check_files(parents)
            current = _binding(_selected(parents, item))
            desired = item[side]
            if current == desired:
                continue
            if desired is None:
                _remove_file(parents, item)
            else:
                _replace_file(parents, item, blobs[desired['sha256']])
            parents.verify()
        if rollback:
            for ref in reversed(plan['new_directories']):
                _authorize(guard, plan)
                _still_pending(root, pending)
                _check_files(parents, side=side)
                path = Path(ref)
                try:
                    parent = parents.get(path.parent.as_posix())
                except FileNotFoundError:
                    continue
                try:
                    # No traversal and no recursive deletion. Any unselected
                    # descendant keeps recovery pending for its actual owner.
                    os.rmdir(path.name, dir_fd=parent)
                    os.fsync(parent)
                except FileNotFoundError:
                    pass
                if ref in parents.opened:
                    os.close(parents.opened.pop(ref))
        _authorize(guard, plan)
        _still_pending(root, pending)
        _verify_retained(root, manifest, digest, blobs)
        _check_files(parents, side=side)
        _sync_selected(parents)
        _check_files(parents, side=side)
        # The durability pass can take time; recheck delegated authority at the
        # publication edge instead of relying on the pre-fsync observation.
        _authorize(guard, plan)
        _still_pending(root, pending)
        parents.verify()
        terminal = _new_state(manifest, digest, 'ready', 'rolled-back' if rollback else 'committed',
                              recovery_authorization=recovery_authorization)
        _publish_state(root, terminal, pending)
    _record_completion(root, terminal)
    return {'transaction_id': manifest['transaction_id'], 'status': terminal['outcome'],
            'publication': terminal, 'manifest_ref': (TRANSACTIONS_REF / manifest['transaction_id'][7:] / 'manifest.json').as_posix(),
            'manifest_sha256': digest, 'replayed': False, 'is_current_publication': True,
            'current_selected_bytes_verified': True, 'grants_admission': False}


def apply_transaction(root, plan, *, expected_snapshot, authorization_guard, transaction_id=None,
                      recovery_authorization=None):
    """Apply an exact plan under the caller-held corpus lock; no broad authority.

    A stable caller-supplied transaction ID may be derived from command identity,
    request digest and owner configuration, independently of after-file bytes.
    The manifest separately hashes the complete immutable before/after plan.
    """
    root = Path(root)
    if type(expected_snapshot) is not PublicationSnapshot or expected_snapshot.root != root:
        raise ValueError('an exact caller-owned publication snapshot for this root is required')
    transaction_id = _identifier(transaction_id or _digest(uuid.uuid4().bytes))
    summary, blobs = _freeze_plan(plan)
    _authorize(authorization_guard, summary)
    base = {'token': expected_snapshot.token, 'generation': expected_snapshot.generation}
    current = read_publication_state(root)
    try:
        retained = inspect_transaction(root, transaction_id)
    except FileNotFoundError:
        retained = None
    if recovery_authorization is not None:
        if retained is None or retained['status'] != 'orphan':
            raise PermissionError('pre-publication recovery authorization requires this exact retained orphan plan')
        recovery_authorization = _json(_canonical(recovery_authorization))
        if len(_canonical(recovery_authorization)) > MAX_RECOVERY_AUTHORIZATION_BYTES:
            raise ValueError('recovery authorization exceeds its 4 KiB binding budget')
    if retained is not None:
        manifest = retained['manifest']
        if manifest['plan'] != summary or manifest['base_publication'] != base:
            raise TransactionConflict('transaction identity was reused for a different exact plan')
        if retained['status'] == 'pending':
            return resume_transaction(root, authorization_guard=authorization_guard, transaction_id=transaction_id)
        if retained['status'] in {'committed', 'rolled-back'}:
            if current is not None and current['phase'] == 'pending':
                raise TransactionConflict('another selected-metadata publication requires recovery')
            if retained['is_current_publication']:
                with _Parents(root, manifest) as parents:
                    _check_files(parents, side='after' if retained['status'] == 'committed' else 'before')
                _record_completion(root, retained['publication'])
            _authorize(authorization_guard, summary)
            return {key: retained[key] for key in ('transaction_id', 'status', 'publication', 'manifest_sha256',
                                                  'is_current_publication', 'grants_admission')} | {
                'manifest_ref': (TRANSACTIONS_REF / transaction_id[7:] / 'manifest.json').as_posix(),
                'replayed': True, 'current_selected_bytes_verified': retained['is_current_publication']}
    expected_snapshot.verify_current()
    if current is not None:
        # Retain already committed evidence before a later head can replace it.
        # This records existing transport history, not renewed source authority.
        previous = inspect_transaction(root, current['transaction_id'])
        if previous['publication'] != current:
            raise TransactionCorruption('current ready publication has no exact retained manifest')
        _record_completion(root, current)
    manifest = (retained['manifest'] if retained is not None else {
        'schema_version': PROFILED_MANIFEST_SCHEMA if 'path_profile' in summary else MANIFEST_SCHEMA,
        'transaction_id': transaction_id, 'base_publication': base,
        'plan': summary, 'parents': _capture_parents(root, summary)})
    _validate_manifest(manifest, transaction_id)
    with _Parents(root, manifest) as parents:
        _check_files(parents, side='before')
        if retained is not None:
            # Explicit retry, not orphan authority: the original caller plan,
            # current delegation and unchanged before snapshot must all agree.
            _capture_parents(root, summary)
        digest = _retain(root, manifest, blobs)
        _authorize(authorization_guard, summary)
        expected_snapshot.verify_current()
        _verify_retained(root, manifest, digest, blobs)
        _check_files(parents, side='before')
        pending = _new_state(manifest, digest, 'pending')
        _publish_state(root, pending, current)
    return _move(root, manifest, digest, blobs, pending, authorization_guard,
                 recovery_authorization=recovery_authorization)


def _recover(root, authorization_guard, transaction_id, *, rollback, recovery_authorization):
    root = Path(root)
    pending = read_pending_transaction(root)
    if pending is None:
        raise TransactionConflict('there is no head-selected pending transaction to recover')
    if transaction_id is not None and _identifier(transaction_id) != pending['state']['transaction_id']:
        raise TransactionConflict('recovery selected a different pending transaction')
    if recovery_authorization is not None:
        recovery_authorization = _json(_canonical(recovery_authorization))
        if len(_canonical(recovery_authorization)) > MAX_RECOVERY_AUTHORIZATION_BYTES:
            raise ValueError('recovery authorization exceeds its 4 KiB binding budget')
    summary, blobs = _freeze_plan(pending['plan'])
    manifest = pending['manifest']
    if summary != manifest['plan']:
        raise TransactionCorruption('retained recovery bytes differ from the exact transaction plan')
    return _move(root, manifest, pending['state']['manifest_sha256'], blobs, pending['state'],
                 authorization_guard, rollback=rollback, recovery_authorization=recovery_authorization)


def resume_transaction(root, *, authorization_guard, transaction_id=None, recovery_authorization=None):
    """Explicit exact rollforward of the selected pending head under caller lock."""
    return _recover(root, authorization_guard, transaction_id, rollback=False,
                    recovery_authorization=recovery_authorization)


def rollback_transaction(root, *, authorization_guard, transaction_id=None, recovery_authorization=None):
    """Explicit exact rollback; never rolls back an already committed transport."""
    return _recover(root, authorization_guard, transaction_id, rollback=True,
                    recovery_authorization=recovery_authorization)
