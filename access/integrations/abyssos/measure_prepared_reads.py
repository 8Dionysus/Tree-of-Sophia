#!/usr/bin/env python3
"""Explicit finite CLI or persistent HTTP workload; run inside canonical host resource admission."""
import argparse
import concurrent.futures
import hashlib
import heapq
import math
import http.client
import urllib.parse
import json
import os
from pathlib import Path
import selectors
import signal
import stat
import subprocess
import threading
import time


def positive(value):
    result = int(value)
    if result <= 0:
        raise argparse.ArgumentTypeError('must be positive')
    return result


def identity(path, deadline):
    path = Path(path)
    if not path.is_absolute():
        raise ValueError('input must be an absolute regular non-symlink file')

    def stamp(info):
        return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns,
                info.st_ctime_ns, info.st_mode)

    before = path.lstat()
    mount_readonly = bool(os.statvfs(path).f_flag & os.ST_RDONLY)
    if not stat.S_ISREG(before.st_mode) or (before.st_mode & 0o222 and not mount_readonly):
        raise ValueError('input must be protected read-only regular file')
    digest = hashlib.sha256()
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(descriptor, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        if stamp(before) != stamp(opened):
            raise ValueError('input pathname/FD changed before hash')
        for chunk in iter(lambda: stream.read(65536), b''):
            if time.monotonic() >= deadline:
                raise TimeoutError('whole workload identity deadline')
            digest.update(chunk)
        if stamp(opened) != stamp(os.fstat(stream.fileno())) or stamp(opened) != stamp(path.lstat()):
            raise ValueError('input pathname/FD changed during hash')
    return {'path': str(path), 'bytes': opened.st_size, 'sha256': digest.hexdigest(),
            'device': opened.st_dev, 'inode': opened.st_ino,
            'mtime_ns': opened.st_mtime_ns, 'ctime_ns': opened.st_ctime_ns,
            'mode': opened.st_mode, 'mount_readonly': mount_readonly}


def no_model_sidecars(model):
    if any(os.path.lexists(model + suffix) for suffix in ('-wal', '-shm', '-journal')):
        raise ValueError('model requires immutable standalone main file')


def final_model_coordination(model, output):
    # The selected empty-WAL profile may create bounded SQLite coordination
    # files even for a READ_ONLY connection. They never supply committed data.
    if os.path.lexists(model + '-journal'):
        raise ValueError('model created a rollback journal')
    artifacts = []
    for suffix, maximum in (('-wal', 0), ('-shm', 32768)):
        path = Path(model + suffix)
        if not os.path.lexists(path):
            continue
        if Path(model).parent.resolve() != output.resolve():
            raise ValueError('coordination artifacts require model in owned output')
        before = path.lstat()
        if (not stat.S_ISREG(before.st_mode) or before.st_uid != os.getuid() or
                before.st_nlink != 1 or before.st_size > maximum):
            raise ValueError('model coordination artifact outside selected profile')
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        try:
            opened = os.fstat(fd)
            if (opened != before or path.lstat() != before):
                raise ValueError('model coordination artifact changed during admission')
        finally:
            os.close(fd)
        artifacts.append({'path': str(path), 'bytes': before.st_size,
                          'mode': before.st_mode, 'inode': before.st_ino})
    return artifacts


def parse_server_observation(raw):
    def unique_pairs(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate native observation field')
            result[key] = value
        return result

    value = json.loads(raw.decode('utf-8'), object_pairs_hook=unique_pairs)
    if (not isinstance(value, dict) or set(value) !=
            {'schema', 'complete', 'connections', 'operations', 'queue', 'overflowed'} or
            value['schema'] != 'tos_http_observation_v1' or
            type(value['complete']) is not bool or type(value['overflowed']) is not bool):
        raise ValueError('invalid native observation shape/version')
    for field, keys in (('connections', {'accepted', 'refused_capacity', 'spawn_failed', 'live', 'peak'}),
                        ('operations', {'entered', 'completed_ok', 'completed_error', 'live', 'peak'})):
        counters = value[field]
        if (not isinstance(counters, dict) or set(counters) != keys or
                any(type(n) is not int or not 0 <= n <= 2**64 - 1 for n in counters.values())):
            raise ValueError('invalid native observation counters')
        if counters['live'] > counters['peak'] or (value['complete'] and counters['live'] != 0):
            raise ValueError('inconsistent native observation live counters')
    queue = value['queue']
    if (not isinstance(queue, dict) or set(queue) != {'supported', 'depth'} or
            queue['supported'] is not False or type(queue['depth']) is not int or queue['depth'] != 0):
        raise ValueError('invalid native observation queue')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'model', 'binding', 'schedule', 'output', 'unit'):
        parser.add_argument('--' + name, required=True)
    for name in ('concurrency', 'deadline-seconds', 'output-cap-bytes',
                 'response-cap-bytes', 'schedule-cap-bytes', 'max-requests'):
        parser.add_argument('--' + name, type=positive, required=True)
    parser.add_argument('--transport', choices=('cli', 'http'), default='cli')
    parser.add_argument('--max-actors', type=positive)
    parser.add_argument('--port', type=positive)
    parser.add_argument('--observe-http', action='store_true',
                        help='require owned stdin-EOF shutdown and fixed native server observation')
    parser.add_argument('--server-log-cap-bytes', type=positive)
    args = parser.parse_args()
    if args.transport == 'http' and (args.port is None or args.port > 65535 or args.server_log_cap_bytes is None):
        parser.error('HTTP requires explicit port1..65535 and server-log-cap-bytes')
    if args.observe_http and (args.transport != 'http' or args.deadline_seconds > 3600):
        parser.error('observed HTTP requires transport=http and whole deadline <=3600 seconds')
    started_ns = time.monotonic_ns()
    deadline_ns = started_ns + args.deadline_seconds * 1_000_000_000
    started = started_ns / 1_000_000_000
    deadline = deadline_ns / 1_000_000_000
    target = Path(args.output)
    if not target.is_absolute() or target.is_symlink() or not target.is_dir():
        raise ValueError('output must be an existing absolute owned directory')
    if args.unit not in Path('/proc/self/cgroup').read_text().strip().split('/'):
        # systemd may escape a component; never accept a substring match.
        raise ValueError('not in requested admitted unit')
    receipt = subprocess.Popen(
        ['abyss-machine', 'storage', 'write-reservation', 'list', '--json'],
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, start_new_session=True)
    admission_bytes = bytearray()
    receipt_deadline = time.monotonic() + min(5, args.deadline_seconds)
    receipt_selector = selectors.DefaultSelector()
    receipt_selector.register(receipt.stdout, selectors.EVENT_READ)
    try:
        while receipt_selector.get_map():
            if time.monotonic() >= receipt_deadline:
                raise ValueError('admission receipt deadline')
            for key, _ in receipt_selector.select(.1):
                raw = os.read(key.fd, 65536)
                if not raw:
                    receipt_selector.unregister(key.fileobj)
                elif len(admission_bytes) + len(raw) > 1048576:
                    raise ValueError('admission receipt exceeds 1MiB')
                else:
                    admission_bytes.extend(raw)
        if receipt.wait(timeout=max(.001, receipt_deadline - time.monotonic())):
            raise ValueError('canonical admission receipt failed')
    finally:
        try:
            os.killpg(receipt.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        receipt.wait()
        receipt.stdout.close()
        receipt_selector.close()
    admission = json.loads(admission_bytes)
    leases = [r for r in admission.get('records', []) if r.get('active')
              and r.get('target') and Path(r['target']).resolve() == target.resolve()]
    if not admission.get('ok') or admission.get('state_errors') or len(leases) != 1:
        raise ValueError('fresh unique canonical output lease required')
    lease = leases[0]
    if (not lease.get('hold_until_terminal') or
            lease.get('requested_bytes', 0) < args.output_cap_bytes or
            not lease.get('execution_identity', '').startswith('resource-launch:' + args.unit + ':') or
            not lease['execution_identity'].endswith(':execution')):
        raise ValueError('lease not bound to this terminal execution')
    identities = {name: identity(getattr(args, name), deadline)
                  for name in ('binary', 'model', 'binding', 'schedule')}
    if identities['schedule']['bytes'] > args.schedule_cap_bytes:
        raise ValueError('schedule cap exceeded')
    no_model_sidecars(args.model)
    schedule = json.loads(Path(args.schedule).read_bytes())
    actors = []
    actor_metadata = []
    actor_window = None
    if isinstance(schedule, dict) and set(schedule) == {'shared_responses', 'requests', 'actors', 'window_seconds'}:
        if args.transport != 'http' or args.max_actors is None:
            raise ValueError('actor schedule requires HTTP and explicit max-actors')
        actors = schedule['actors']
        actor_window = schedule['window_seconds']
        if (not isinstance(actors, list) or not 0 < len(actors) <= args.max_actors
                or any(not isinstance(a, str) or not 1 <= len(a) <= 64
                       or not a.isascii() or not all(c.isalnum() or c in '-_.' for c in a) for a in actors)
                or len(set(actors)) != len(actors)
                or isinstance(actor_window, bool) or not isinstance(actor_window, (int, float))
                or not math.isfinite(actor_window) or not 0 < actor_window <= args.deadline_seconds):
            raise ValueError('finite actor labels/window required')
        requests = schedule['requests']
        if not isinstance(requests, list) or not 0 < len(requests) <= args.max_requests:
            raise ValueError('finite actor requests required')
        last_due = {a: -1 for a in actors}
        normalized = []
        for item in requests:
            if not isinstance(item, dict) or set(item) != {'path', 'status', 'response_index', 'actor_id', 'due_seconds'}:
                raise ValueError('exact actor request fields required')
            actor, due = item['actor_id'], item['due_seconds']
            if (not isinstance(actor, str) or actor not in last_due or isinstance(due, bool)
                    or not isinstance(due, (int, float)) or not math.isfinite(due)
                    or not 0 <= due < actor_window or due < last_due[actor]):
                raise ValueError('actor reference/monotonic due time invalid')
            last_due[actor] = due
            actor_metadata.append((actor, due))
            normalized.append({k: item[k] for k in ('path', 'status', 'response_index')})
        if any(due < 0 for due in last_due.values()):
            raise ValueError('every declared actor must have a request')
        schedule = {'shared_responses': schedule['shared_responses'], 'requests': normalized}
    if args.transport == 'http' and isinstance(schedule, dict):
        shared = set(schedule) == {'shared_response', 'requests'}
        indexed = set(schedule) == {'shared_responses', 'requests'}
        if not shared and not indexed:
            raise ValueError('shared HTTP schedule requires exact oracle/requests keys')
        requests = schedule['requests']
        responses = [schedule['shared_response']] if shared else schedule['shared_responses']
        keys = {'path', 'status'} if shared else {'path', 'status', 'response_index'}
        if (not isinstance(requests, list) or not 0 < len(requests) <= args.max_requests or
                not isinstance(responses, list) or not 0 < len(responses) <= len(requests) or
                any(not isinstance(item, dict) or set(item) != keys for item in requests)):
            raise ValueError('shared HTTP schedule requires bounded exact requests/oracles')
        normalized = []
        for item in requests:
            index = 0 if shared else item['response_index']
            if isinstance(index, bool) or not isinstance(index, int) or not 0 <= index < len(responses):
                raise ValueError('shared HTTP oracle index outside explicit array')
            normalized.append({'path': item['path'], 'status': item['status'], 'response': responses[index]})
        schedule = normalized
    if not isinstance(schedule, list) or not 0 < len(schedule) <= args.max_requests:
        raise ValueError('finite bounded schedule required')
    if args.concurrency > len(schedule):
        raise ValueError('concurrency exceeds explicit request count')
    for item in schedule:
        if args.transport == 'http':
            if (not isinstance(item, dict) or set(item) != {'path', 'status', 'response'} or
                    not isinstance(item['path'], str) or len(item['path'].encode()) > 65536 or
                    isinstance(item['status'], bool) or not isinstance(item['status'], int) or
                    not 100 <= item['status'] <= 599):
                raise ValueError('HTTP requires bounded GET path, exact status and JSON response')
            url = urllib.parse.urlsplit(item['path'])
            node = url.path.startswith('/api/knowledge/nodes/') and bool(url.path[len('/api/knowledge/nodes/'):])
            allowed = url.path in ('/api/knowledge/search', '/api/knowledge/search/capabilities') or node
            if (url.scheme or url.netloc or url.fragment or '\r' in item['path'] or '\n' in item['path'] or
                    not allowed or (url.path == '/api/knowledge/search' and
                     urllib.parse.parse_qs(url.query).get('mode') != ['compressed'])):
                raise ValueError('HTTP schedule must use local compressed search/capabilities/nodes')
            continue
        if (not isinstance(item, dict) or set(item) != {'argv', 'exit_code', 'stdout_json', 'stdout_text', 'stderr_text'} or
                not isinstance(item['argv'], list) or
                not all(isinstance(v, str) for v in item['argv']) or
                item['argv'][:2] != ['knowledge', 'search'] or
                not isinstance(item['stderr_text'], str) or
                (item['stdout_text'] is not None and not isinstance(item['stdout_text'], str)) or
                (item['stdout_text'] is not None and item['stdout_json'] is not None) or
                not any(item['argv'][i:i+2] == ['--mode', 'compressed'] for i in range(len(item['argv']))) or
                isinstance(item['exit_code'], bool) or not isinstance(item['exit_code'], int)):
            raise ValueError('schedule requires compressed search, exact exit/stdout/stderr expectations')
    lock = threading.Lock()
    stopped = threading.Event()
    signal.signal(signal.SIGTERM, lambda *_: stopped.set())
    signal.signal(signal.SIGINT, lambda *_: stopped.set())
    written = 0
    failures = 0
    http_outcomes = {}
    # Exclusive output; do not overwrite an earlier attempt.
    with (target / 'measurements.jsonl').open('xb') as output:
        def emit(value):
            nonlocal written
            raw = (json.dumps(value, ensure_ascii=True, separators=(',', ':')) + '\n').encode()
            with lock:
                if written + len(raw) > args.output_cap_bytes:
                    stopped.set()
                    raise ValueError('aggregate output cap exceeded')
                output.write(raw)
                output.flush()
                written += len(raw)
        emit({'event': 'start', 'identities': identities, 'limits': vars(args),
              'lease_execution': lease['execution_identity'],
              'scope': 'finite ' + args.transport + ' read/search measurement; not capacity acceptance'})

        server = None
        server_reader = None
        server_errors = []
        server_observations = []
        observation_errors = []
        observation_line = bytearray()
        observation_discard = False

        def observe_stderr(raw):
            # Bound control parsing separately from the already capped full raw log.
            nonlocal observation_discard
            marker = b'TOS_HTTP_OBSERVATION '
            for piece in raw.splitlines(keepends=True):
                newline = piece.endswith(b'\n')
                if not observation_discard:
                    if len(observation_line) + len(piece) > 1024:
                        prefix = (bytes(observation_line[:len(marker)]) + piece[:len(marker)])[:len(marker)]
                        if prefix == marker:
                            if not observation_errors:
                                observation_errors.append('oversize native observation')
                        observation_discard = True
                        observation_line.clear()
                    else:
                        observation_line.extend(piece)
                if newline:
                    if not observation_discard and observation_line.startswith(marker):
                        try:
                            server_observations.append(parse_server_observation(bytes(observation_line[len(marker):])))
                        except (ValueError, UnicodeError) as exc:
                            if not observation_errors:
                                observation_errors.append(str(exc))
                        if len(server_observations) > 1:
                            raise ValueError('duplicate native observation')
                    observation_line.clear()
                    observation_discard = False

        def owns_listener():
            if server.poll() is not None:
                return False
            sockets = set()
            for fd in Path(f'/proc/{server.pid}/fd').iterdir():
                try:
                    link = os.readlink(fd)
                    if link.startswith('socket:['):
                        sockets.add(link[8:-1])
                except FileNotFoundError:
                    continue
            for row in Path('/proc/net/tcp').read_text().splitlines()[1:]:
                fields = row.split()
                if fields[1] == f'0100007F:{args.port:04X}' and fields[3] == '0A' and fields[9] in sockets:
                    return True
            return False

        def drain_server():
            selector = selectors.DefaultSelector()
            total = 0
            for name, stream in (('stdout', server.stdout), ('stderr', server.stderr)):
                selector.register(stream, selectors.EVENT_READ, name)
            try:
                while selector.get_map():
                    if stopped.is_set() or time.monotonic() >= deadline:
                        stopped.set()
                        try:
                            os.killpg(server.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                        break
                    for key, _ in selector.select(.1):
                        raw = os.read(key.fd, 65536)
                        if not raw:
                            selector.unregister(key.fileobj)
                        else:
                            total += len(raw)
                            if total > args.server_log_cap_bytes:
                                raise ValueError('server log cap exceeded')
                            if args.observe_http and key.data == 'stderr':
                                observe_stderr(raw)
                            emit({'event': 'server_log', 'stream': key.data,
                                  'text': raw.decode('utf-8', errors='backslashreplace')})
            except Exception as exc:
                server_errors.append(str(exc))
                stopped.set()
                try:
                    os.killpg(server.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            finally:
                selector.close()

        measurement_origin = None
        actor_results = {a: {'scheduled': 0, 'started': 0, 'successful': 0, 'outcomes': {},
                              'first_due': None, 'last_due': None, 'first_start': None,
                              'last_finish': None, 'successful_in_window': 0, 'first_success_finish': None,
                              'last_success_finish': None, 'max_success_gap_seconds': None, 'latencies': []} for a in actors}
        for actor, due in actor_metadata:
            row = actor_results[actor]
            row['scheduled'] += 1
            row['first_due'] = due if row['first_due'] is None else row['first_due']
            row['last_due'] = due

        def measured_fields(index, begin, finish, success, outcome):
            if not actors:
                return {}
            actor, due = actor_metadata[index]
            scheduled = measurement_origin + due
            with lock:
                row = actor_results[actor]
                row['started'] += 1
                row['successful'] += int(success)
                row['outcomes'][outcome] = row['outcomes'].get(outcome, 0) + 1
                row['first_start'] = begin - measurement_origin if row['first_start'] is None else row['first_start']
                row['last_finish'] = finish - measurement_origin
                if success:
                    relative_finish = finish - measurement_origin
                    row['successful_in_window'] += int(relative_finish <= actor_window)
                    if row['last_success_finish'] is not None:
                        gap = relative_finish - row['last_success_finish']
                        row['max_success_gap_seconds'] = max(row['max_success_gap_seconds'] or 0, gap)
                    row['first_success_finish'] = relative_finish if row['first_success_finish'] is None else row['first_success_finish']
                    row['last_success_finish'] = relative_finish
                    row['latencies'].append(finish - scheduled)
            return {'actor_id': actor, 'due_seconds': due,
                    'start_seconds': begin - measurement_origin, 'finish_seconds': finish - measurement_origin,
                    'client_wait_seconds': begin - scheduled, 'latency_due_to_finish_seconds': finish - scheduled,
                    'successful_operation': success}

        def run_http(index):
            item = schedule[index]
            begin = time.monotonic()
            status = None
            body = bytearray()
            error = None
            outcome = None
            connection = None
            try:
                if stopped.is_set() or time.monotonic() >= deadline or not owns_listener():
                    raise ValueError('admitted server unavailable/cancelled')
                connection = http.client.HTTPConnection('127.0.0.1', args.port,
                    timeout=max(.001, deadline - time.monotonic()))
                connection.request('GET', item['path'], headers={'Connection': 'close'})
                response = connection.getresponse()
                status = response.status
                while True:
                    if stopped.is_set() or time.monotonic() >= deadline:
                        raise TimeoutError('whole workload deadline/cancellation')
                    if connection.sock is not None:
                        connection.sock.settimeout(max(.001, deadline - time.monotonic()))
                    raw = response.read1(min(65536, args.response_cap_bytes - len(body) + 1))
                    if not raw:
                        break
                    if len(body) + len(raw) > args.response_cap_bytes:
                        raise ValueError('HTTP response body cap exceeded')
                    body.extend(raw)
                if not owns_listener():
                    raise ValueError('server listener ownership changed')
                decoded = json.loads(body)
                matches = decoded == item['response']
                if status == 503 and decoded == {'error': 'server busy', 'code': 'unavailable'}:
                    outcome = 'server-busy-503'
            except Exception as exc:
                error = str(exc)
                if isinstance(exc, TimeoutError) or time.monotonic() >= deadline:
                    outcome = 'timeout'
                elif stopped.is_set():
                    outcome = 'cancelled'
                elif isinstance(exc, ConnectionRefusedError):
                    outcome = 'connect-refused'
                matches = False
            finally:
                if connection is not None:
                    connection.close()
            passed = error is None and status == item['status'] and matches
            outcome = outcome or ('exact-response' if passed else 'error')
            with lock:
                http_outcomes[outcome] = http_outcomes.get(outcome, 0) + 1
            finish = time.monotonic()
            fields = measured_fields(index, begin, finish, passed and 200 <= status < 300, outcome)
            emit({'event': 'request', **fields, 'index': index, 'path': item['path'],
                  'elapsed_seconds': time.monotonic() - begin, 'status': status,
                  'passed': passed, 'error': error, 'outcome': outcome,
                  'body': body.decode('utf-8', errors='backslashreplace')})
            return passed

        def run(index):
            item = schedule[index]
            if stopped.is_set() or time.monotonic() >= deadline:
                return False
            begin = time.monotonic()
            process = subprocess.Popen(
                [args.binary, '--prepared-read-model', args.model,
                 '--prepared-binding', args.binding, *item['argv']],
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                start_new_session=True, cwd=target)
            buffers = {'stdout': bytearray(), 'stderr': bytearray()}
            error = None
            selector = selectors.DefaultSelector()
            for name, stream in (('stdout', process.stdout), ('stderr', process.stderr)):
                selector.register(stream, selectors.EVENT_READ, name)
            try:
                while selector.get_map():
                    if stopped.is_set() or time.monotonic() >= deadline:
                        raise TimeoutError('whole workload deadline/cancellation')
                    for key, _ in selector.select(min(.1, max(0, deadline - time.monotonic()))):
                        raw = os.read(key.fd, 65536)
                        if not raw:
                            selector.unregister(key.fileobj)
                        else:
                            if sum(map(len, buffers.values())) + len(raw) > args.response_cap_bytes:
                                raise ValueError('per-request stdout+stderr cap exceeded')
                            buffers[key.data].extend(raw)
                process.wait(timeout=max(.001, deadline - time.monotonic()))
            except Exception as exc:
                error = str(exc)
            finally:
                # Descendants holding pipes or surviving the CLI are also terminal.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
                selector.close()
                process.stdout.close()
                process.stderr.close()
            try:
                stdout = buffers['stdout'].decode('utf-8', errors='strict')
                stderr = buffers['stderr'].decode('utf-8', errors='strict')
            except UnicodeError:
                stdout = buffers['stdout'].decode('utf-8', errors='backslashreplace')
                stderr = buffers['stderr'].decode('utf-8', errors='backslashreplace')
                error = error or 'response is not UTF-8'
            if item['stdout_text'] is None:
                try:
                    matches = json.loads(stdout) == item['stdout_json']
                except ValueError:
                    matches = False
            else:
                matches = stdout == item['stdout_text']
            passed = (error is None and process.returncode == item['exit_code']
                      and matches and stderr == item['stderr_text'])
            emit({'event': 'request', 'index': index, 'argv': item['argv'],
                  'elapsed_seconds': time.monotonic() - begin, 'exit_code': process.returncode,
                  'passed': passed, 'error': error,
                  'stdout': buffers['stdout'].decode('utf-8', errors='backslashreplace'),
                  'stderr': buffers['stderr'].decode('utf-8', errors='backslashreplace')})
            return passed

        next_index = 0
        try:
            if args.transport == 'http':
                server_argv = [args.binary, '--prepared-read-model', args.model,
                    '--prepared-binding', args.binding, 'serve', f'127.0.0.1:{args.port}']
                if args.observe_http:
                    server_argv += ['--observe-stdin-eof-deadline-ns', str(deadline_ns)]
                server = subprocess.Popen(server_argv,
                    stdin=subprocess.PIPE if args.observe_http else subprocess.DEVNULL,
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                    start_new_session=True, cwd=target)
                server_reader = threading.Thread(target=drain_server)
                server_reader.start()
                while not owns_listener():
                    if server.poll() is not None or stopped.is_set() or time.monotonic() >= deadline:
                        raise ValueError('owned native HTTP listener did not become ready')
                    stopped.wait(.02)
                emit({'event': 'server_ready', 'pid': server.pid, 'port': args.port,
                      'startup_seconds': time.monotonic() - started})
            measurement_origin = time.monotonic()
            # Only C futures live; schedule stays finite and no unbounded executor queue.
            if actors:
                lanes = {a: [] for a in actors}
                for index, (actor, due) in enumerate(actor_metadata):
                    lanes[actor].append(index)
                positions = {a: 0 for a in actors}
                ready = [(actor_metadata[lanes[a][0]][1], order, a) for order, a in enumerate(actors)]
                heapq.heapify(ready)
                with concurrent.futures.ThreadPoolExecutor(max_workers=args.concurrency) as pool:
                    pending = {}
                    while ready or pending:
                        now = time.monotonic()
                        if stopped.is_set() or now >= deadline:
                            stopped.set()
                        while (ready and len(pending) < args.concurrency and not stopped.is_set()
                               and measurement_origin + ready[0][0] <= now):
                            _, order, actor = heapq.heappop(ready)
                            index = lanes[actor][positions[actor]]
                            pending[pool.submit(run_http, index)] = (order, actor)
                            next_index += 1
                        if not pending:
                            if stopped.is_set() or not ready:
                                break
                            stopped.wait(min(.05, max(0, measurement_origin + ready[0][0] - time.monotonic())))
                            continue
                        done, _ = concurrent.futures.wait(pending, timeout=.05,
                            return_when=concurrent.futures.FIRST_COMPLETED)
                        for future in done:
                            order, actor = pending.pop(future)
                            try:
                                failures += not future.result()
                            except Exception:
                                stopped.set()
                                raise
                            positions[actor] += 1
                            if positions[actor] < len(lanes[actor]):
                                index = lanes[actor][positions[actor]]
                                heapq.heappush(ready, (actor_metadata[index][1], order, actor))
            else:
                with concurrent.futures.ThreadPoolExecutor(max_workers=args.concurrency) as pool:
                    next_index = 0
                    pending = set()
                    while pending or next_index < len(schedule):
                        while (len(pending) < args.concurrency and next_index < len(schedule)
                               and not stopped.is_set() and time.monotonic() < deadline):
                            pending.add(pool.submit(run_http if args.transport == 'http' else run, next_index))
                            next_index += 1
                        if not pending:
                            break
                        done, pending = concurrent.futures.wait(pending, return_when=concurrent.futures.FIRST_COMPLETED)
                        for future in done:
                            try:
                                failures += not future.result()
                            except Exception:
                                stopped.set()
                                raise
        finally:
            if server is not None:
                if args.observe_http:
                    # Sole control writer: EOF only, never a command or renewed clock.
                    server.stdin.close()
                    while server.poll() is None and not stopped.is_set() and time.monotonic_ns() < deadline_ns:
                        time.sleep(min(.02, max(0, (deadline_ns - time.monotonic_ns()) / 1_000_000_000)))
                # Reap the whole owned group even if its leader has exited;
                # surviving descendants must not retain drain pipe endpoints.
                try:
                    os.killpg(server.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                server.wait()
                if server_reader is not None:
                    server_reader.join()
                server.stdout.close()
                server.stderr.close()
                if args.observe_http:
                    if observation_line.startswith(b'TOS_HTTP_OBSERVATION '):
                        observation_errors.append('unterminated native observation')
                    if len(server_observations) != 1:
                        observation_errors.append('exactly one native observation required')
                    if server.returncode != 0:
                        observation_errors.append('native observed shutdown failed')
                    if server_observations and (not server_observations[0]['complete'] or server_observations[0]['overflowed']):
                        observation_errors.append('native observation incomplete')
                    server_errors.extend(observation_errors)
                emit({'event': 'server_terminal', 'exit_code': server.returncode,
                      'errors': server_errors,
                      **({'observation': server_observations[0] if len(server_observations) == 1 else None,
                          'observation_available': not observation_errors} if args.observe_http else {})})
        guard_error = None
        coordination = []
        try:
            coordination = final_model_coordination(args.model, target)
            unchanged = identities == {name: identity(getattr(args, name), deadline) for name in identities}
        except Exception as exc:
            unchanged = False
            guard_error = str(exc)
        if actors:
            summary_incomplete = False
            for actor, row in actor_results.items():
                samples = row.pop('latencies')
                row['successful_latency_sample_count'] = len(samples)
                incomplete = time.monotonic() >= deadline
                if not incomplete:
                    samples.sort()
                    incomplete = time.monotonic() >= deadline
                summary_incomplete |= incomplete
                row['summary_incomplete'] = 'whole-deadline' if incomplete else None
                row['successful_latency_nearest_rank'] = {
                    key: samples[math.ceil(q * len(samples)) - 1] if samples and not incomplete else None
                    for key, q in (('p50', .50), ('p95', .95), ('p99', .99), ('max', 1.0))}
                row['unstarted'] = row['scheduled'] - row['started']
            counts = [row['successful'] for row in actor_results.values()]
            emit({'event': 'actors', 'window_seconds': actor_window, 'actors': actor_results,
                  'summary_incomplete': summary_incomplete,
                  'successful_in_window_per_second': sum(r['successful_in_window'] for r in actor_results.values()) / actor_window,
                  'fairness': {'min_successful': min(counts), 'max_successful': max(counts),
                               'mean_successful': sum(counts) / len(counts),
                               'zero_successful_actors': sum(n == 0 for n in counts)},
                  'scope': 'client actor schedules, not server threads or authorization'})
        passed = (unchanged and not server_errors and not failures and next_index == len(schedule)
                  and not stopped.is_set() and (not actors or not summary_incomplete))
        emit({'event': 'finish', 'passed': passed, 'input_unchanged': unchanged,
              'guard_error': guard_error, 'model_coordination': coordination,
              'scheduled': len(schedule), 'started': next_index, 'failures': failures,
              'elapsed_seconds': time.monotonic() - started, 'http_outcomes': http_outcomes})
        return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())
