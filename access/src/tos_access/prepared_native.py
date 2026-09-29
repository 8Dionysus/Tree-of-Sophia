"""Explicit native file-owner adapter; no graph assembly or live-DB transfer."""
from __future__ import annotations

import dataclasses
import json
import os
from pathlib import Path
import selectors
import shutil
import subprocess
import time

from .published_read_metadata import _compact

HEADER_FRAME_BYTES = 16_843_008


def select_publication_executor(executable=None, timeout=None):
    selected = executable if executable is not None else os.environ.get('TOS_PREPARED_EXECUTOR')
    if selected is None:
        installed = shutil.which('tos-access')
        selected = Path(installed).resolve() if installed is not None else None
    if selected is None:
        raise ValueError('native prepared executable must be explicitly configured or installed')
    if not isinstance(selected, (str, Path)):
        raise ValueError('native prepared executable must be an absolute selected path')
    selected = Path(selected)
    if not selected.is_absolute() or not selected.is_file() or not os.access(selected, os.X_OK):
        raise ValueError('native prepared executable must be an absolute executable regular file')
    if timeout is None:
        raw = os.environ.get('TOS_PREPARED_MAX_SECONDS')
        if raw is None or not raw.isascii() or not raw.isdecimal():
            raise ValueError('native prepared explicit positive whole seconds required')
        timeout = int(raw)
    if type(timeout) is not int or not 1 <= timeout <= (1 << 64) - 1:
        raise ValueError('native prepared explicit positive whole seconds required')
    return selected, timeout


def native_publication(path, *, executable, timeout, operation, header, catalog,
                       limits, row_factory=None, changes=None, expected_binding=None,
                       search_reuse=None, search_scratch_path=None, search_scratch_limits=None):
    if (not isinstance(executable, (str, Path)) or not Path(executable).is_absolute()
            or not Path(executable).is_file() or not os.access(executable, os.X_OK)):
        raise ValueError('native prepared publication requires an absolute selected executable')
    if type(timeout) is not int or not 1 <= timeout <= (1 << 64) - 1:
        raise ValueError('native prepared publication requires explicit positive whole seconds')
    deadline = time.monotonic() + timeout
    frame = {'operation': operation, 'path': str(Path(path).absolute()),
             'header': header, 'catalog': catalog, 'limits': dataclasses.asdict(limits),
             'max_seconds': timeout}
    if search_reuse is not None:
        frame['search_reuse'] = {field.name: getattr(search_reuse, field.name)
                                for field in dataclasses.fields(search_reuse)
                                if field.name not in ('path', 'progress')}
        frame['search_reuse']['path'] = str(Path(search_reuse.path).absolute())
    if search_scratch_path is not None:
        frame['search_scratch_path'] = str(Path(search_scratch_path).absolute())
        frame['search_scratch_limits'] = dataclasses.asdict(search_scratch_limits)
    if operation == 'delta':
        frame['expected_binding'] = expected_binding

    def encoded(value, cap):
        raw = _compact(value).encode('utf-8')
        if len(raw) > cap:
            raise ValueError('native prepared input frame byte budget exceeded')
        if time.monotonic() >= deadline:
            raise TimeoutError('native prepared whole-operation deadline exceeded')
        return raw + b'\n'

    first = encoded(frame, HEADER_FRAME_BYTES)

    def frames():
        yield first
        if operation == 'bootstrap':
            for _ in range(2):
                for kind in ('node', 'relation'):
                    count = 0
                    for item in row_factory(kind):
                        count += 1
                        if count > limits.max_mutations:
                            raise ValueError('native prepared input row budget exceeded')
                        yield encoded({'row': item}, limits.max_row_bytes + 32)
                    yield b'{"end":true}\n'
        else:
            from .prepared_publication import PreparedChange
            changed_bytes = 0
            for count, change in enumerate(changes, 1):
                if count > limits.max_changes:
                    raise ValueError('native prepared input change count exceeded')
                if not isinstance(change, PreparedChange):
                    raise ValueError("explicit PreparedChange required")
                value = {key: getattr(change, key) for key in
                         ('operation', 'kind', 'identifier', 'item', 'source_order')}
                raw = encoded(value, limits.max_row_bytes + 32768)
                changed_bytes += len(raw)
                if changed_bytes > limits.max_change_bytes + limits.max_metadata_bytes:
                    raise ValueError('native prepared retained input changes exceeded')
                yield raw
            yield b'{"end":true}\n'

    callback = search_reuse.progress if search_reuse is not None else None
    ack_read, ack_write = os.pipe() if callback is not None else (None, None)
    command = [str(executable), 'prepared-publication', '--max-seconds', str(timeout)]
    if callback is not None:
        command.extend(['--progress-ack-fd', str(ack_read)])
    try:
        process = subprocess.Popen(command,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, close_fds=True,
                               pass_fds=() if ack_read is None else (ack_read,))
    except BaseException:
        for fd in (ack_read, ack_write):
            if fd is not None:
                os.close(fd)
        raise
    if ack_read is not None:
        os.close(ack_read)
        os.set_blocking(ack_write, False)
    progress_buffer, progress_bytes = bytearray(), 0

    output, errors = bytearray(), bytearray()
    try:
        with selectors.DefaultSelector() as selector:
            for stream in (process.stdin, process.stdout, process.stderr):
                os.set_blocking(stream.fileno(), False)
            selector.register(process.stdin, selectors.EVENT_WRITE, 'input')
            selector.register(process.stdout, selectors.EVENT_READ, 'output')
            selector.register(process.stderr, selectors.EVENT_READ, 'error')
            iterator, pending, offset = iter(frames()), None, 0
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError('native prepared whole-operation deadline exceeded')
                for key, _ in selector.select(min(remaining, 0.25)):
                    stream = key.fileobj
                    if key.data == 'input':
                        if pending is None:
                            pending = next(iterator, None)
                            offset = 0
                            if pending is None:
                                selector.unregister(stream)
                                stream.close()
                                continue
                        try:
                            written = os.write(stream.fileno(), memoryview(pending)[offset:offset + 65536])
                        except BrokenPipeError:
                            selector.unregister(stream)
                            stream.close()
                            pending = None
                            continue
                        offset += written
                        if offset == len(pending):
                            pending = None
                    else:
                        chunk = os.read(stream.fileno(), 65536)
                        if not chunk:
                            selector.unregister(stream)
                            stream.close()
                            continue
                        target, cap = (output, 65536) if key.data == 'output' else (errors, 8192)
                        if len(target) + len(chunk) > cap:
                            raise ValueError('native prepared output/error byte budget exceeded')
                        if key.data == 'output' and callback is not None:
                            progress_buffer.extend(chunk)
                            if len(progress_buffer) > 65536:
                                raise ValueError('native prepared output frame budget exceeded')
                            while b'\n' in progress_buffer:
                                raw, _, tail = progress_buffer.partition(b'\n')
                                progress_buffer[:] = tail
                                value = json.loads(raw)
                                if isinstance(value, dict) and set(value) == {'progress'}:
                                    progress_bytes += len(raw)
                                    if len(raw) > 4096 or progress_bytes > 1024 * 1024:
                                        raise ValueError('native prepared progress byte budget exceeded')
                                    report = value['progress']
                                    phases = {'donor_metadata_validated', 'donor_source_terms_progress',
                                              'donor_source_terms_validated', 'donor_table_copied',
                                              'search_successor_prepared'}
                                    if not isinstance(report, dict) or report.get('phase') not in phases:
                                        raise ValueError('native prepared invalid progress report')
                                    # Callbacks are cooperative synchronous user code.
                                    # Native still bounds its ack wait by the absolute
                                    # deadline; Python checks again when callback returns.
                                    callback(report)
                                    if time.monotonic() >= deadline:
                                        raise TimeoutError('native prepared callback exceeded deadline')
                                    if os.write(ack_write, b'{"ack":true}\n') != 13:
                                        raise ValueError('native prepared incomplete progress acknowledgement')
                                else:
                                    target.extend(raw + b'\n')
                        else:
                            target.extend(chunk)
        if progress_buffer:
            raise ValueError('incomplete native prepared output frame')
        status = process.wait(timeout=max(0.001, deadline - time.monotonic()))
        if status:
            raise ValueError(errors.decode('utf-8', 'replace').strip() or
                             f'native prepared publication exited {status}')
        value = json.loads(output)
        if not isinstance(value, dict) or value.get('schema') != 'tos_published_knowledge_snapshot_v1':
            raise ValueError('native prepared publication returned an invalid binding')
        return value
    except BaseException:
        if ack_write is not None:
            os.close(ack_write)
            ack_write = None
        # EOF lets the native owner rollback a normal framing failure. A whole
        # deadline can leave an unselected interrupted candidate, never success.
        if process.stdin and not process.stdin.closed:
            process.stdin.close()
        try:
            process.wait(timeout=max(0.001, deadline - time.monotonic()))
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        raise
    finally:
        if ack_write is not None:
            os.close(ack_write)
        for stream in (process.stdin, process.stdout, process.stderr):
            if stream and not stream.closed:
                stream.close()
