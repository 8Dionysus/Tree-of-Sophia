#!/usr/bin/env python3
"""PROPOSED immutable producer payload; not activated or executed.

This source is embedded in an immutable workflow object for external review.
No ref points at it and no runtime was started. One shared budget covers host,
attachment and output; a self-reported binding never substitutes for owner authority.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import sys
import time

TOTAL_CAP = 8 * 1024 * 1024
SECONDS = 5
COUNT_CAP = 2048
POLICY = Path('/sys/kernel/security/apparmor/policy')
OUTPUT_CAP = 524288


class BudgetAbort(RuntimeError):
    pass


class InputBudgetExceeded(BudgetAbort):
    pass


class DeadlineExceeded(BudgetAbort):
    pass


class FieldLimit(ValueError):
    pass


def require(value, reason):
    if not value:
        raise RuntimeError(reason)


class Budget:
    def __init__(self, byte_cap=TOTAL_CAP, seconds=SECONDS, clock=time.monotonic):
        self.clock = clock
        self.started = clock()
        self.deadline = self.started + seconds
        self.byte_cap = byte_cap
        self.used = 0

    @property
    def remaining(self):
        return self.byte_cap - self.used

    def check(self):
        if self.clock() >= self.deadline:
            raise DeadlineExceeded('one whole-producer deadline exhausted')

    def call(self, operation, *args, **kwargs):
        self.check()
        try:
            result = operation(*args, **kwargs)
        except BaseException:
            self.check()  # Errors do not create a fresh deadline.
            raise
        self.check()
        return result

    def read_chunks(self, read_operation, cap, single=False):
        chunks = []
        size = 0
        while True:
            self.check()
            if self.remaining <= 0:
                raise InputBudgetExceeded('whole-producer input budget exhausted before EOF')
            requested = min(65536, cap - size + 1, self.remaining)
            try:
                raw = read_operation(requested)
            except BaseException:
                self.check()
                raise
            require(isinstance(raw, bytes) and len(raw) <= requested, 'invalid reader result')
            # Charge BEFORE deadline/field-limit checks, even on their Err path.
            self.used += len(raw)
            self.check()
            size += len(raw)
            if size > cap:
                raise FieldLimit('diagnostic byte bound')
            chunks.append(raw)
            if not raw or single:
                break
        self.check()
        return b''.join(chunks)

    def read_root(self, path, cap, single=False):
        self.check()
        try:
            fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
        except BaseException:
            self.check()
            raise
        try:
            self.check()  # FD is already owned, so late-open failure still closes it.
            before = self.call(os.fstat, fd)
            require(stat.S_ISREG(before.st_mode) and before.st_uid == 0
                    and not before.st_mode & 0o022, 'root evidence custody differs')
            raw = self.read_chunks(lambda count: os.read(fd, count), cap, single)
            after = self.call(os.fstat, fd)
            require(all(getattr(before, k) == getattr(after, k) for k in
                        ('st_dev', 'st_ino', 'st_size', 'st_mtime_ns', 'st_ctime_ns')),
                    'held evidence changed')
            return raw
        finally:
            try:
                os.close(fd)  # Owned-FD cleanup still occurs on budget failure.
            finally:
                self.check()

    def soft_text(self, path, cap):
        try:
            raw = self.read_root(path, cap)
            text = self.call(raw.decode, 'utf-8')
            return text
        except BudgetAbort:
            raise  # Never serialize a global bound failure into a cheap field.
        except (OSError, UnicodeError, FieldLimit) as error:
            self.check()
            return {'unavailable': 'diagnostic byte bound' if isinstance(error, FieldLimit)
                    else type(error).__name__}


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def profile_inventory(budget, walker=os.walk):
    rows = []
    iterator = iter(budget.call(walker, POLICY / 'profiles', followlinks=False))
    while True:
        try:
            current, dirs, files = budget.call(next, iterator)
        except StopIteration:
            break
        dirs.sort()
        if {'name', 'mode', 'attach'} <= set(files):
            require(len(rows) < COUNT_CAP, 'profile count bound')
            directory = Path(current)
            values = {key: budget.soft_text(directory / key, 4096)
                      for key in ('name', 'mode', 'attach', 'sha256')}
            rows.append((directory, values))
        budget.check()
    require(rows, 'profile inventory unavailable')
    return rows


def policy_digest(budget, rows):
    budget.check()
    values = sorted((value for _, value in rows), key=lambda value: json.dumps(value, sort_keys=True))
    wire = json.dumps(values, sort_keys=True, separators=(',', ':')).encode()
    budget.check()
    require(len(wire) <= 262144, 'policy metadata output bound')
    return digest(wire)


def collect_host(budget):
    budget.check()  # This SAME budget already exists before the first host read.
    rows = profile_inventory(budget)
    report = {
        'schema': 'tos_ci_host_diagnostic_v2',
        'observed_realtime': int(budget.call(time.time)),
        'image_os': os.environ.get('ImageOS'), 'image_version': os.environ.get('ImageVersion'),
        'kernel': budget.call(os.uname).release,
        'os_release': budget.soft_text('/etc/os-release', 8192),
        'lsm': budget.soft_text('/sys/kernel/security/lsm', 4096),
        'apparmor_enabled': budget.soft_text('/sys/module/apparmor/parameters/enabled', 64),
        'userns_restrict': budget.soft_text('/proc/sys/kernel/apparmor_restrict_unprivileged_userns', 64),
        'userns_clone': budget.soft_text('/proc/sys/kernel/unprivileged_userns_clone', 64),
        'max_user_namespaces': budget.soft_text('/proc/sys/user/max_user_namespaces', 64),
        'loaded_profiles': sorted((v for _, v in rows), key=lambda v: json.dumps(v, sort_keys=True)),
        'policy_sha256': policy_digest(budget, rows),
        'package_version_evidence': {'state': 'not-collected',
                                     'reason': 'no subprocess outside common input accounting'},
        'policy_changed': False, 'namespace_attempted': False,
    }
    require(all(v is None or isinstance(v, str) and len(v) <= 256
                for v in (report['image_os'], report['image_version'], report['kernel'])),
            'platform metadata bound')
    context = dict(report)
    context.pop('observed_realtime')
    report['context_sha256'] = digest(json.dumps(context, sort_keys=True,
                                                separators=(',', ':')).encode())
    budget.check()
    return report, rows


def normalize_kernel_link(directory, target):
    require(not os.path.isabs(target), 'absolute kernel raw-data link')
    path = Path(os.path.normpath(str(directory / target)))
    require(path.is_relative_to(POLICY / 'raw_data'), 'kernel raw-data link escaped policy')
    return path


def kernel_link(budget, directory, name):
    return normalize_kernel_link(directory, budget.call(os.readlink, directory / name))


def collect_attachment(budget, host, before, revision):
    # No new ledger, timer, JSON-as-input charge, or child process here.
    rows, streams = [], {}
    for directory, values in before:
        budget.check()
        require(all(isinstance(values.get(k), str) for k in ('name', 'mode', 'attach', 'sha256')),
                'required profile metadata unavailable')
        profile_sha = values['sha256'].strip()
        require(re.fullmatch('[0-9a-f]{64}', profile_sha), 'profile hash unavailable')
        raw = kernel_link(budget, directory, 'raw_data')
        sha_path = kernel_link(budget, directory, 'raw_sha256')
        abi_path = kernel_link(budget, directory, 'raw_abi')
        require(raw.name == 'raw_data' and sha_path.parent == raw.parent
                and abi_path.parent == raw.parent and sha_path.name == 'sha256'
                and abi_path.name == 'abi', 'kernel metadata links differ')
        raw_sha = budget.read_root(sha_path, 128).decode().strip()
        abi = budget.read_root(abi_path, 64).decode().strip()
        require(re.fullmatch('[0-9a-f]{64}', raw_sha) and re.fullmatch('v[0-9]+', abi),
                'raw stream hash/ABI unavailable')
        info = budget.call(raw.lstat)  # NEVER open/decompress raw_data.
        require(stat.S_ISREG(info.st_mode) and info.st_uid == 0 and not info.st_mode & 0o022,
                'raw endpoint custody differs')
        stream = {'kernel_raw_sha256': raw_sha, 'raw_abi': abi,
                  'raw_endpoint_custody_checked': True, 'raw_data_opened': False}
        require(raw_sha not in streams or streams[raw_sha] == stream, 'stream identity differs')
        streams[raw_sha] = stream
        rows.append({'profile_sha256': profile_sha, 'raw_sha256': raw_sha, 'raw_abi': abi,
                     'unknown_attachment': values['attach'].strip() == '<unknown>'})
    config_path = Path('/etc/apparmor/parser.conf')
    try:
        content = budget.read_root(config_path, 262144)
        config = {'path': str(config_path), 'bytes': len(content), 'sha256': digest(content),
                  'contents_exported': False}
    except FileNotFoundError:
        budget.check()
        config = {'path': str(config_path), 'state': 'absent', 'contents_exported': False}
    executables = []
    for name in ('/usr/sbin/apparmor_parser', '/usr/bin/unshare'):
        path = budget.call(Path(name).resolve, strict=True)
        content = budget.read_root(path, TOTAL_CAP)
        executables.append({'path': name, 'resolved': str(path), 'bytes': len(content),
                            'sha256': digest(content)})
        budget.check()
    require(budget.read_root(POLICY / 'revision', 64, single=True) == revision
            and policy_digest(budget, profile_inventory(budget)) == host['policy_sha256'],
            'policy changed during whole-producer read')
    return {'schema': 'tos_ci_attachment_evidence_v2', 'collection_complete': True,
            'source_closure_proven': False, 'attachment_admission': False,
            'policy_source_bytes_exported': False, 'raw_data_opened': False,
            'profiles': rows, 'raw_streams': streams, 'parser_config_digest': config,
            'executables': executables, 'guard': 'unknown->refuse remains unchanged'}


def encode_metadata(budget, report):
    chunks, size = [], 0
    for part in json.JSONEncoder(sort_keys=True).iterencode(report):
        budget.check()
        raw = part.encode()
        require(len(raw) <= OUTPUT_CAP - size, 'metadata output bound')
        chunks.append(raw)
        size += len(raw)
    require(size < OUTPUT_CAP, 'metadata final newline bound')
    budget.check()
    return b''.join(chunks) + b'\n'


def emit_metadata(budget, wire, writer=os.write):
    require(len(wire) <= OUTPUT_CAP, 'output bound')
    def write_all(fd, data):
        offset = 0
        while offset < len(data):
            budget.check()
            count = budget.call(writer, fd, memoryview(data)[offset:])
            require(isinstance(count, int) and 0 < count <= len(data) - offset,
                    'incomplete metadata output')
            offset += count
    write_all(1, wire)
    pin = ('TOS_ANCHORED_DIAGNOSTIC_SHA256 ' + digest(wire)
           + ' bytes=' + str(len(wire)) + '\n').encode()
    write_all(2, pin)
    budget.check()


def _build_candidate_only(binding):
    """Proposed activation: only the immutable driver calls this after binding checks."""
    budget = Budget()  # Established BEFORE any diagnostic host read.
    require(budget.call(os.geteuid) == 0, 'future admitted root context required')
    old_handler = budget.call(signal.getsignal, signal.SIGALRM)
    old_timer = budget.call(signal.getitimer, signal.ITIMER_REAL)
    require(old_timer == (0.0, 0.0), 'existing process timer; no takeover')
    def expired(signum, frame):
        raise DeadlineExceeded('one whole-producer timer expired')
    handler_changed = False
    try:
        signal.signal(signal.SIGALRM, expired)
        handler_changed = True
        signal.setitimer(signal.ITIMER_REAL, max(0.000001, budget.deadline - budget.clock()))
        revision = budget.read_root(POLICY / 'revision', 64, single=True)
        require(re.fullmatch(rb'[0-9]+\n', revision), 'incomplete revision')
        host, rows = collect_host(budget)
        attachment = collect_attachment(budget, host, rows, revision)
        report = {'schema': 'tos_ci_combined_metadata_candidate_v1',
                  'outcome': 'diagnostic-only', 'diagnostic_exit_code': 79,
                  'host': host, 'attachment': attachment,
                  'ticket_issued': False, 'namespace_attempted': False, 'policy_changed': False,
                  'source_closure_proven': False, 'attachment_admission': False,
                  'policy_source_bytes_exported': False,
                  'accounting': {'scope': 'all candidate os.read results from first host read',
                                 'input_bytes': budget.used, 'input_cap': TOTAL_CAP,
                                 'deadline_seconds': SECONDS, 'serialized_JSON_is_not_input_cost': True}}
        report['immutable_launch_binding'] = binding
        emit_metadata(budget, encode_metadata(budget, report))
        return 79
    except BaseException:
        budget.check()
        raise
    finally:
        if handler_changed:
            try:
                signal.setitimer(signal.ITIMER_REAL, 0)
            finally:
                signal.signal(signal.SIGALRM, old_handler)
        budget.check()


def collect(binding):
    require(isinstance(binding, dict) and binding.get('binding_check') == 'platform-coherent',
            'immutable driver binding required; external owner approval is separate')
    return _build_candidate_only(binding)


def run(binding):
    try:
        return collect(binding)
    except Exception:
        # No new host IO, JSON serialization or late logging after budget failure.
        return 1


if __name__ == '__main__':
    raise SystemExit('Direct checkout execution disabled; only immutable producer driver.')
