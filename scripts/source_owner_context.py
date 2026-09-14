"""Explicit confidential source transport, not a second knowledge grammar.

Logical references retain one owner. Reserved owner-local refs never fall
back to a checkout, and private files never override public schemas/records.
Only an independently selected protected configuration can construct this
context. It grants no operation, source-read, assessment or publication right.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat

from jsonschema import Draft202012Validator
from jsonschema.exceptions import SchemaError
from referencing import Registry
from referencing.exceptions import Unresolvable


CONTEXT_SCHEMA_REF = 'ToS/contracts/owner-local-source-context.schema.json'
OWNER_LOCAL_HOME = Path('ToS/source-witnesses/owner-local')
MAX_CONTEXT_BYTES = 1_048_576


class SourceOwnerContextError(ValueError):
    """The selected transport is unsafe, ambiguous or no longer current."""


def _canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True,
                      separators=(',', ':'), allow_nan=False).encode('utf-8')


def _hash(raw):
    return hashlib.sha256(raw).hexdigest()


def _object(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise SourceOwnerContextError('owner context has duplicate JSON fields')
            result[key] = value
        return result
    try:
        value = json.loads(raw.decode('utf-8'), object_pairs_hook=pairs)
        if not isinstance(value, dict):
            raise ValueError('not an object')
        _canonical(value)
        return value
    except (ValueError, UnicodeError, RecursionError) as error:
        raise SourceOwnerContextError('owner context is not finite bounded UTF-8 JSON') from error


def _absolute(value):
    if not isinstance(value, (str, Path)):
        raise SourceOwnerContextError('owner context needs an explicit absolute path')
    path = Path(value)
    if (not path.is_absolute() or path == Path('/') or '..' in path.parts
            or path.as_posix() != str(value) or '\x00' in str(value) or '\\' in str(value)):
        raise SourceOwnerContextError('owner context needs a normalized dedicated absolute path')
    return path


def _open(path, *, directory=False, private_root=None, confidential_file=False):
    """No-follow owner checks; confidentiality is stricter than write safety."""
    path = _absolute(path)
    uid = os.getuid()
    if uid != os.geteuid():
        raise SourceOwnerContextError('owner context is not a setuid interface')
    descriptor = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    current = Path('/')
    try:
        for index, part in enumerate(path.parts[1:]):
            final = index == len(path.parts) - 2
            flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
            if not final or directory:
                flags |= os.O_DIRECTORY
            child = os.open(part, flags, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
            current /= part
            info = os.fstat(descriptor)
            sticky_ancestor = (not final and stat.S_ISDIR(info.st_mode)
                               and info.st_uid == 0 and info.st_mode & stat.S_ISVTX)
            if (info.st_uid not in (0, uid) or info.st_mode & 0o022 and not sticky_ancestor
                    or final and not directory and not stat.S_ISREG(info.st_mode)):
                raise SourceOwnerContextError('owner context path is not protected from other users')
            private = private_root is not None and current.is_relative_to(private_root)
            if private or final and confidential_file:
                expected = 0o700 if stat.S_ISDIR(info.st_mode) else 0o600
                if info.st_uid != uid or stat.S_IMODE(info.st_mode) != expected:
                    raise SourceOwnerContextError('owner-local storage requires owner-only directories and files')
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _stat_identity(info):
    return info.st_dev, info.st_ino, info.st_uid, stat.S_IMODE(info.st_mode)


def _read(path, limit, *, private_root=None, confidential_file=False):
    try:
        descriptor = _open(path, private_root=private_root, confidential_file=confidential_file)
        with os.fdopen(descriptor, 'rb') as stream:
            before = os.fstat(stream.fileno())
            raw = stream.read(limit + 1)
            after = os.fstat(stream.fileno())
        if (len(raw) > limit or (*_stat_identity(before), before.st_size, before.st_mtime_ns, before.st_ctime_ns)
                != (*_stat_identity(after), after.st_size, after.st_mtime_ns, after.st_ctime_ns)):
            raise SourceOwnerContextError('owner context input exceeds its budget or changed while reading')
        return raw
    except OSError as error:
        raise SourceOwnerContextError('owner context input is absent or unsafe') from error


class OwnerLocalSourceContext:
    """A fixed local transport snapshot with a singleton owner per ref.

    ``public_root`` names the source/contract checkout, NOT content visibility.
    Existing private representation bytes there still need an explicit native
    exact-owner-local read scope. This object does not choose that scope.
    """

    @classmethod
    def load(cls, config_path):
        path = _absolute(config_path)
        raw = _read(path, MAX_CONTEXT_BYTES, confidential_file=True)
        config = _object(raw)
        public_root = _absolute(config.get('public_root'))
        schema_raw = _read(public_root / CONTEXT_SCHEMA_REF, MAX_CONTEXT_BYTES)
        schema = _object(schema_raw)
        if schema.get('$id') != 'https://tree-of-sophia.local/' + CONTEXT_SCHEMA_REF:
            raise SourceOwnerContextError('owner context schema identity differs')
        def refuse_resource(_uri):
            raise SourceOwnerContextError('owner context schema selects an undeclared resource')
        try:
            Draft202012Validator.check_schema(schema)
            if not Draft202012Validator(schema, registry=Registry(retrieve=refuse_resource)).is_valid(config):
                raise SourceOwnerContextError('owner context violates its exact configuration contract')
        except (SchemaError, Unresolvable, RecursionError) as error:
            raise SourceOwnerContextError('owner context grammar is invalid or unresolved') from error
        private_root = _absolute(config['private_root'])
        prefix = f"{OWNER_LOCAL_HOME.as_posix()}/{config['store_id']}/"
        if (config['private_prefix'] != prefix or private_root.is_relative_to(public_root)
                or public_root.is_relative_to(private_root)):
            raise SourceOwnerContextError('owner-local prefix or distinct-root partition differs')
        value = cls.__new__(cls)
        value._config_path, value._config_raw, value._schema_raw = path, raw, schema_raw
        value.contract_digest = _hash(schema_raw)
        value.public_root, value.private_root = public_root, private_root
        value.store_id, value.private_prefix = config['store_id'], prefix
        value._prefix = Path(prefix)
        value._roots = value._root_identities()
        value._context_digest = 'sha256:' + _hash(_canonical({
            'schema_version': config['schema_version'], 'configuration_path': path.as_posix(),
            'configuration_sha256': _hash(raw), 'schema_sha256': _hash(schema_raw),
            'roots': value._roots, 'routing': {'private_prefix': prefix,
                'public_root': public_root.as_posix(), 'private_root': private_root.as_posix()}}))
        value.snapshot()
        return value

    def _root_identities(self):
        result = {}
        try:
            for role, root in (('source-contract-root', self.public_root), ('owner-local-root', self.private_root)):
                descriptor = _open(root, directory=True,
                                   private_root=self.private_root if role == 'owner-local-root' else None)
                try:
                    result[role] = _stat_identity(os.fstat(descriptor))
                finally:
                    os.close(descriptor)
            # An alias in the checkout is not a fallback, even if byte-identical.
            # Check the reserved home itself, including a broken symlink.
            if os.path.lexists(self.public_root / OWNER_LOCAL_HOME):
                raise SourceOwnerContextError('owner-local namespace collides with the source checkout')
            return result
        except OSError as error:
            raise SourceOwnerContextError('owner context root is absent or unsafe') from error

    def path(self, ref):
        if not isinstance(ref, str) or '\x00' in ref or '\\' in ref:
            raise SourceOwnerContextError('owner context reference is not a logical source path')
        path = Path(ref)
        if (path.is_absolute() or '..' in path.parts or path.as_posix() != ref
                or not path.is_relative_to('ToS') or path == Path('ToS')):
            raise SourceOwnerContextError('owner context reference escapes its source namespace')
        if path.is_relative_to(OWNER_LOCAL_HOME):
            if not path.is_relative_to(self._prefix) or path == self._prefix:
                raise SourceOwnerContextError('owner-local reference belongs to another store or lacks a file')
            return self.private_root / path
        return self.public_root / path

    def role(self, ref):
        return 'owner-local-root' if self.path(ref).is_relative_to(self.private_root) else 'source-contract-root'

    def read_bytes(self, path, limit, *, read_bytes=None):
        """Preserve caller budget accounting while enforcing private modes."""
        path = _absolute(path)
        if type(limit) is not int or limit < 0:
            raise SourceOwnerContextError('owner context needs a finite byte budget')
        if path.is_relative_to(self.private_root):
            ref = path.relative_to(self.private_root).as_posix()
            private = self.private_root
        elif path.is_relative_to(self.public_root):
            ref = path.relative_to(self.public_root).as_posix()
            private = None
        else:
            raise SourceOwnerContextError('owner context read leaves both declared roots')
        if self.path(ref) != path:
            raise SourceOwnerContextError('owner context read chooses the wrong root role')
        if read_bytes is None:
            return _read(path, limit, private_root=private)
        try:
            def observe():
                descriptor = _open(path, private_root=private)
                try:
                    info = os.fstat(descriptor)
                    return (*_stat_identity(info), info.st_size, info.st_mtime_ns, info.st_ctime_ns)
                finally:
                    os.close(descriptor)
            before = observe()
            raw = read_bytes(path, limit)
            if not isinstance(raw, bytes) or len(raw) > limit or before != observe():
                raise SourceOwnerContextError('owner context input exceeded its budget or changed')
            return raw
        except OSError as error:
            raise SourceOwnerContextError('owner context input is absent or unsafe') from error

    def snapshot(self):
        if (_read(self._config_path, MAX_CONTEXT_BYTES, confidential_file=True) != self._config_raw
                or _read(self.public_root / CONTEXT_SCHEMA_REF, MAX_CONTEXT_BYTES) != self._schema_raw
                or self._root_identities() != self._roots):
            raise SourceOwnerContextError('owner context configuration, contract or root identity changed')
        return self._context_digest
