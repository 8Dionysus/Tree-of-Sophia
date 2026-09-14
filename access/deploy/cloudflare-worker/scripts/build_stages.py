"""Local, disposable checkpoints for offline build stages, not corpus authority."""
from __future__ import annotations

import hashlib
import json
import os
import tempfile
from contextlib import contextmanager
from pathlib import Path


def fingerprint(paths):
    """Read actual bytes, including membership/missing files; never trust mtimes."""
    result = {}
    for name, path in sorted(paths.items()):
        path = Path(path)
        if path.is_symlink():
            raise RuntimeError('build dependency must not be a symlink: ' + str(path))
        if not path.is_file():
            result[name] = None
            continue
        with path.open('rb') as source:
            result[name] = hashlib.file_digest(source, 'sha256').hexdigest()
    return result


def tree_paths(root, prefix, *, exclude=()):
    return {prefix + '/' + path.relative_to(root).as_posix(): path
            for path in sorted(root.rglob('*'))
            if path.is_file() and path.relative_to(root).as_posix() not in exclude}


def atomic_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix='.' + path.name + '-', dir=path.parent)
    try:
        with os.fdopen(descriptor, 'w', encoding='utf-8') as target:
            json.dump(value, target, ensure_ascii=False, separators=(',', ':'))
            target.write('\n')
            target.flush()
            os.fsync(target.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)  # Only this call's mkstemp file.


@contextmanager
def build_lock(runtime):
    # The offline edge builder runs on Unix. Queries/portable access do not
    # import this module. An OS lock is released on process exit, unlike a PID file.
    import fcntl
    runtime.mkdir(parents=True, exist_ok=True)
    with (runtime / 'build.lock').open('a') as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError('another edge build owns this runtime directory') from error
        try:
            yield
        finally:
            fcntl.flock(handle, fcntl.LOCK_UN)


class BuildStages:
    def __init__(self, path):
        self.path = path
        try:
            packet = json.loads(path.read_text(encoding='utf-8'))
            self.entries = packet['stages'] if packet.get('schema') == 'tos_build_stages_v1' else {}
            if not isinstance(self.entries, dict):
                self.entries = {}
        except (OSError, ValueError, KeyError, TypeError, AttributeError):
            self.entries = {}
        self.report = {}
        self.checks = {}

    def verify(self):
        for name, (inputs, expected) in self.checks.items():
            if inputs() != expected:
                raise RuntimeError('build inputs changed before completion; retry: ' + name)

    def save(self):
        atomic_json(self.path, {'schema': 'tos_build_stages_v1', 'stages': self.entries})

    @staticmethod
    def checksum(entry):
        return hashlib.sha256(json.dumps({key: value for key, value in entry.items() if key != 'checksum'},
                                        sort_keys=True, separators=(',', ':')).encode()).hexdigest()

    def run(self, name, inputs, outputs, produce):
        before = inputs()
        self.checks[name] = (inputs, before)
        entry = self.entries.get(name)
        if (isinstance(entry, dict) and entry.get('inputs') == before
                and entry.get('checksum') == self.checksum(entry)):
            current = fingerprint(outputs())
            if current and all(value is not None for value in current.values()) and current == entry.get('outputs'):
                if isinstance(entry.get('result'), dict):
                    self.report[name] = 'reused'
                    return entry['result']
        # Invalidate before a producer can overwrite any old outputs. Failure
        # leaves no reusable success, but completed independent stages survive.
        self.entries.pop(name, None)
        self.save()
        result = produce()
        if not isinstance(result, dict):
            raise RuntimeError('build stage must return an object: ' + name)
        after = inputs()
        if after != before:
            raise RuntimeError('build inputs changed during stage; retry: ' + name)
        generated = fingerprint(outputs())
        if not generated or any(value is None for value in generated.values()):
            raise RuntimeError('build stage has missing outputs: ' + name)
        self.entries[name] = {'inputs': before, 'outputs': generated, 'result': result}
        self.entries[name]['checksum'] = self.checksum(self.entries[name])
        self.save()
        self.report[name] = 'computed'
        return result
