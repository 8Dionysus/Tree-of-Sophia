"""Wire-only adapter for the installed native acquisition owner.

Fixture fetchers supply bytes only when requested by the native attempt loop.
Selection, validation, custody and receipts stay in Rust. No legacy fallback.
"""
from __future__ import annotations
import base64
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import threading
import time

class NativeAcquisitionError(ValueError):
    def __init__(self, error, message):
        self.error, self.message = error, message
        super().__init__(message)

MAX_RESPONSE_BYTES = 64 * 1024 * 1024

def invoke(request: dict, fetcher=None):
    selected = os.environ.get("TOS_NATIVE_OWNER_COMMAND_BIN") or shutil.which("tos-native-owner-command")
    if not selected:
        raise NativeAcquisitionError("AcquisitionBatchError", "installed tos-native-owner-command is required; set TOS_NATIVE_OWNER_COMMAND_BIN")
    selected = str(Path(selected).absolute())
    child = subprocess.Popen([selected, "acquisition"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    deadline = time.monotonic() + 3600
    events = queue.Queue(maxsize=2)
    stderr = bytearray()
    def read_output():
        try:
            while True:
                line = child.stdout.readline(MAX_RESPONSE_BYTES + 1)
                if not line:
                    events.put(None)
                    break
                if len(line) > MAX_RESPONSE_BYTES or not line.endswith(b"\n"):
                    events.put(NativeAcquisitionError("AcquisitionBatchError", "native acquisition response exceeds its wire budget"))
                    break
                events.put(json.loads(line))
        except Exception as exc:
            events.put(exc)
    def read_error():
        while True:
            block = child.stderr.read(4096)
            if not block:
                return
            if len(stderr) + len(block) <= 65536:
                stderr.extend(block)
    threading.Thread(target=read_output, daemon=True).start()
    threading.Thread(target=read_error, daemon=True).start()
    def write_request(raw):
        # Pipe backpressure is covered by the same whole-operation clock.
        written = queue.Queue(maxsize=1)
        def write():
            try:
                child.stdin.write(raw)
                child.stdin.flush()
                written.put(None)
            except Exception as exc:
                written.put(exc)
        threading.Thread(target=write, daemon=True).start()
        try:
            result = written.get(timeout=max(0.001, deadline-time.monotonic()))
        except queue.Empty:
            raise NativeAcquisitionError("AcquisitionBatchError", "native acquisition request deadline expired")
        if result is not None:
            raise result
    try:
        payload = dict(request)
        payload["fetch_callback"] = fetcher is not None
        raw = (json.dumps(payload, ensure_ascii=False, allow_nan=False) + "\n").encode()
        if len(raw) > 16 * 1024 * 1024:
            raise NativeAcquisitionError("AcquisitionBatchError", "native acquisition request exceeds its wire budget")
        write_request(raw)
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise NativeAcquisitionError("AcquisitionBatchError", "native acquisition operation deadline expired")
            try:
                response = events.get(timeout=min(remaining, 0.1))
            except queue.Empty:
                continue
            if isinstance(response, Exception):
                raise response
            if response is None:
                raise NativeAcquisitionError("AcquisitionBatchError", "native acquisition ended without a response: " + stderr.decode(errors="replace"))
            if response.get("kind") == "fetch":
                if fetcher is None:
                    raise NativeAcquisitionError("SourceFetchError", "native requested an unselected fixture fetcher")
                # Run even a fixture callback within the operation's remaining
                # wall clock; a hung callback cannot keep native custody locked.
                fetched = queue.Queue(maxsize=1)
                def call_fetch():
                    try:
                        value = fetcher(response["payload"])
                        if not isinstance(value, bytes):
                            raise ValueError("fetcher did not return bytes")
                        if len(value) > 300 * 1024 * 1024:
                            raise ValueError("fetcher exceeds bounded payload limit")
                        fetched.put({"body_base64": base64.b64encode(value).decode()})
                    except Exception as exc:
                        fetched.put({"error": str(exc)[:4096]})
                threading.Thread(target=call_fetch, daemon=True).start()
                try:
                    value = fetched.get(timeout=max(0.001, deadline-time.monotonic()))
                except queue.Empty:
                    raise NativeAcquisitionError("SourceFetchError", "fixture fetch deadline expired")
                write_request((json.dumps(value) + "\n").encode())
            elif response.get("kind") == "error":
                raise NativeAcquisitionError(response.get("error", "AcquisitionBatchError"), response.get("message", "native acquisition refused"))
            elif response.get("kind") == "result":
                child.stdin.close()
                code = child.wait(timeout=max(0.001, deadline-time.monotonic()))
                if code:
                    raise NativeAcquisitionError("AcquisitionBatchError", "native acquisition child failed")
                return response["value"]
            else:
                raise NativeAcquisitionError("AcquisitionBatchError", "unexpected native acquisition wire response")
    finally:
        if child.poll() is None:
            import signal
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()
        for pipe in (child.stdin, child.stdout, child.stderr):
            if pipe and not pipe.closed:
                pipe.close()
