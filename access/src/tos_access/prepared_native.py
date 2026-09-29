"""Explicit native file-owner adapter; no graph assembly or live-DB transfer."""
from __future__ import annotations

import dataclasses
import json
import os
from pathlib import Path
import selectors
import subprocess
import time

from .published_read_metadata import _compact

HEADER_FRAME_BYTES = 16_843_008


def native_publication(path, *, executable, timeout, operation, header, catalog,
                       limits, row_factory=None, changes=None, expected_binding=None):
    if (not isinstance(executable, (str, Path)) or not Path(executable).is_absolute()
            or not Path(executable).is_file()):
        raise ValueError('native prepared publication requires an absolute selected executable')
    if type(timeout) is not int or timeout < 1:
        raise ValueError('native prepared publication requires explicit positive whole seconds')
    frame = {'operation': operation, 'path': str(Path(path).absolute()),
             'header': header, 'catalog': catalog, 'limits': dataclasses.asdict(limits),
             'max_seconds': timeout}
    if operation == 'delta':
        frame['expected_binding'] = expected_binding

    def encoded(value, cap):
        raw = _compact(value).encode('utf-8')
        if len(raw) > cap:
            raise ValueError('native prepared input frame byte budget exceeded')
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

    started = time.monotonic()
    deadline = started + timeout
    process = subprocess.Popen([str(executable), 'prepared-publication', '--max-seconds', str(timeout)],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, close_fds=True)
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
                        target.extend(chunk)
        status = process.wait(timeout=max(0.001, deadline - time.monotonic()))
        if status:
            raise ValueError(errors.decode('utf-8', 'replace').strip() or
                             f'native prepared publication exited {status}')
        value = json.loads(output)
        if not isinstance(value, dict) or value.get('schema') != 'tos_published_knowledge_snapshot_v1':
            raise ValueError('native prepared publication returned an invalid binding')
        return value
    except BaseException:
        # EOF lets the native owner rollback a normal framing failure. A whole
        # deadline can leave an unselected interrupted candidate, never success.
        if process.stdin and not process.stdin.closed:
            process.stdin.close()
        try:
            process.wait(timeout=min(1, max(0.001, deadline - time.monotonic())))
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        raise
    finally:
        for stream in (process.stdin, process.stdout, process.stderr):
            if stream and not stream.closed:
                stream.close()
