"""Read-only publication guard for participating selected-metadata transactions.

The source files still own meaning. This small control record says only whether
cooperating readers may finish a snapshot. It is not a catalog, source resolver,
filesystem-wide snapshot, authorization grant, or detector for legacy/manual
writes. No code in this module creates a file or takes a writer lock.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat

SOURCE_HOME = Path('ToS/source-witnesses')
CONTROL_REF = SOURCE_HOME / '.metadata-publication.json'
TRANSACTIONS_REF = SOURCE_HOME / '.metadata-transactions'
STATE_SCHEMA = 'tos_source_metadata_publication_v1'
MAX_STATE_BYTES = 8192
MAX_RECOVERY_AUTHORIZATION_BYTES = 4096
MAX_GENERATION = 9_007_199_254_740_991
HASH = re.compile(r'sha256:[a-f0-9]{64}')


class PublicationStateError(ValueError):
    """The publication protocol cannot establish a safe read boundary."""


class PublicationPending(PublicationStateError):
    """An explicitly selected source transaction requires owner recovery."""


class PublicationChanged(PublicationStateError):
    """A participating publication changed during this read."""


def _canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True,
                      separators=(',', ':'), allow_nan=False).encode('utf-8')


def _digest(raw):
    return 'sha256:' + hashlib.sha256(raw).hexdigest()


def _json(raw):
    def pairs(items):
        value = {}
        for key, item in items:
            if key in value:
                raise PublicationStateError('duplicate publication metadata key')
            value[key] = item
        return value

    def nonfinite(value):
        raise PublicationStateError('nonfinite publication metadata')

    try:
        value = json.loads(raw, object_pairs_hook=pairs, parse_constant=nonfinite)
        _canonical(value)
    except (ValueError, TypeError, UnicodeError, RecursionError) as error:
        raise PublicationStateError('invalid publication metadata JSON') from error
    if not isinstance(value, dict):
        raise PublicationStateError('publication metadata must be an object')
    return value


def _open_owned(path, *, directory=False):
    """Pin a protected owner path without following any symlink component.

    This retains the existing same-Unix-account trust boundary: hostile code
    running as the owner is not isolated from that owner's source repository.
    """
    path = Path(path)
    uid = os.getuid()
    if (not path.is_absolute() or '..' in path.parts or path == Path('/')
            or os.geteuid() != uid):
        raise PermissionError('an absolute protected owner path is required')
    descriptor = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    try:
        for index, part in enumerate(path.parts[1:]):
            final = index == len(path.parts) - 2
            flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
            if directory or not final:
                flags |= os.O_DIRECTORY
            child = os.open(part, flags, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
            info = os.fstat(descriptor)
            sticky = (not final and stat.S_ISDIR(info.st_mode)
                      and info.st_uid == 0 and info.st_mode & stat.S_ISVTX)
            if (info.st_uid not in (0, uid) or info.st_mode & 0o022 and not sticky
                    or final and not directory and not stat.S_ISREG(info.st_mode)):
                raise PermissionError('publication path is not protected owner metadata')
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _identity(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_size,
            info.st_mtime_ns, info.st_ctime_ns)


def _read_owned(path, limit):
    descriptor = _open_owned(path)
    with os.fdopen(descriptor, 'rb') as stream:
        before = os.fstat(stream.fileno())
        if before.st_size > limit:
            raise PublicationStateError('publication metadata exceeds its byte budget')
        raw = stream.read(limit + 1)
        after = os.fstat(stream.fileno())
    if len(raw) > limit or _identity(before) != _identity(after):
        raise PublicationChanged('publication metadata changed during read')
    descriptor = _open_owned(path)
    try:
        current = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if _identity(current) != _identity(before):
        raise PublicationChanged('publication metadata was replaced during read')
    return raw


def _validate_state(value):
    keys = {'schema_version', 'generation', 'transition_id', 'phase',
            'transaction_id', 'manifest_sha256', 'outcome', 'recovery_authorization', 'token'}
    if (not isinstance(value, dict) or set(value) != keys
            or value['schema_version'] != STATE_SCHEMA
            or type(value['generation']) is not int
            or not 1 <= value['generation'] <= MAX_GENERATION
            or not isinstance(value['transition_id'], str)
            or not re.fullmatch(r'[a-f0-9]{32}', value['transition_id'])
            or not isinstance(value['phase'], str) or value['phase'] not in {'pending', 'ready'}
            or not all(isinstance(value[key], str) and HASH.fullmatch(value[key])
                       for key in ('transaction_id', 'manifest_sha256', 'token'))
            or value['phase'] == 'pending' and (value['outcome'] is not None or value['recovery_authorization'] is not None)
            or value['phase'] == 'ready' and (not isinstance(value['outcome'], str)
                                            or value['outcome'] not in {'committed', 'rolled-back'})
            or value['recovery_authorization'] is not None
            and (not isinstance(value['recovery_authorization'], dict)
                 or len(_canonical(value['recovery_authorization'])) > MAX_RECOVERY_AUTHORIZATION_BYTES)):
        raise PublicationStateError('invalid selected-metadata publication control')
    if value['token'] != _digest(_canonical({key: item for key, item in value.items() if key != 'token'})):
        raise PublicationStateError('publication control digest does not match its contents')
    return value


def read_publication_state(root):
    """Read one bounded control record, including pending; never recover it.

    Absence is the legacy baseline. The writer never deletes an initialized
    control record; externally removing it is outside this cooperating protocol.
    """
    root = Path(root)
    os.close(_open_owned(root, directory=True))
    try:
        raw = _read_owned(root / CONTROL_REF, MAX_STATE_BYTES)
    except FileNotFoundError:
        return None
    return _validate_state(_json(raw))


class PublicationSnapshot:
    """One operation's ready epoch, checked again before return/publication.

    An absent legacy control yields ``token=None`` and ``generation=0``.
    Never refresh an instance in place or use it as a persistent read cache.
    """
    def __init__(self, root):
        self.root = Path(root)
        self._state = read_publication_state(self.root)
        if self._state is not None and self._state['phase'] != 'ready':
            raise PublicationPending('selected metadata publication is pending owner recovery')

    @property
    def token(self):
        return self._state['token'] if self._state else None

    @property
    def generation(self):
        return self._state['generation'] if self._state else 0

    def verify_current(self):
        current = read_publication_state(self.root)
        if current is not None and current['phase'] != 'ready':
            raise PublicationPending('selected metadata publication is pending owner recovery')
        if current != self._state:
            raise PublicationChanged('selected metadata publication changed during read')
