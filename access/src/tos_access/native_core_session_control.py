"""Strict multipart bytes for one owner-issued native SDK control socket.

This codec selects no data/code and grants no stage, memory or process authority.
The session owner supplies its already-admitted receiver workspace and retains
controller/image/cgroup custody through close acknowledgment AND terminal EOF.
"""
from __future__ import annotations
import math
import select
import socket
import struct
import threading
import time
from dataclasses import dataclass

_HEADER = struct.Struct('<8sBBHQQQI')
_MAGIC = b'TOSSES1\0'
_PACKET_BYTES = 65536
_PAYLOAD_BYTES = _PACKET_BYTES - _HEADER.size
_KINDS = frozenset(range(1, 7))
_U64_MAX = (1 << 64) - 1

class NativeSessionProtocolError(ValueError):
    pass


@dataclass(frozen=True)
class NativeSessionLimits:
    """The native owner's exact six original transport fields, no defaults."""
    max_call_bytes: int
    max_reply_bytes: int
    max_chunks_per_frame: int
    max_calls: int
    max_total_request_bytes: int
    max_total_reply_bytes: int

    def validate(self):
        for name in self.__dataclass_fields__:
            value = getattr(self, name)
            if type(value) is not int or not 0 < value <= _U64_MAX:
                raise ValueError('native session explicit transport limit outside positive u64')
        if (self.max_call_bytes > self.max_total_request_bytes
                or self.max_reply_bytes > self.max_total_reply_bytes):
            raise ValueError('native session frame allowance exceeds original total')
        # Original call count gives a finite cumulative packet-work forecast:
        # startup + each request/reply pair + close/ack. Failure terminates scope.
        maximum_chunks = (2 * self.max_calls + 3) * self.max_chunks_per_frame
        if maximum_chunks > _U64_MAX:
            raise ValueError('native session cumulative packet forecast exceeds u64')
        return maximum_chunks

    def wire(self):
        self.validate()
        return {name: getattr(self, name) for name in self.__dataclass_fields__}

class NativeSessionControl:
    """Borrow a verified issued socket; its owner alone closes the scope/socket.

    receiver_buffer and frame_buffer are distinct ORIGINAL pre-admitted writable
    bytearrays supplied by the receiving-state owner before construction. No
    growing raw-response buffer or independent workspace grant is created here.
    A returned memoryview is valid only until the next receive; the session
    owner must finish decoding/charging delivery before reusing that buffer.
    """
    def __init__(self, peer, *, deadline, cancelled, receiver_buffer, frame_buffer,
                 limits, progress=None):
        if (not isinstance(peer, socket.socket)
                or peer.family != socket.AF_UNIX
                or peer.getsockopt(socket.SOL_SOCKET, socket.SO_TYPE) != socket.SOCK_SEQPACKET):
            raise TypeError('native session requires its issued AF_UNIX SEQPACKET socket')
        peer.getpeername()  # Refuse an unconnected endpoint before IO.
        if type(deadline) not in (float, int) or not math.isfinite(deadline):
            raise ValueError('native session requires its original finite cutoff')
        if not isinstance(cancelled, threading.Event):
            raise TypeError('native session requires its owner cancellation event')
        if (type(receiver_buffer) is not bytearray or len(receiver_buffer) != _PACKET_BYTES
                or type(frame_buffer) is not bytearray or not frame_buffer
                or frame_buffer is receiver_buffer):
            raise TypeError('native session requires distinct pre-admitted packet/frame buffers')
        if not isinstance(limits, NativeSessionLimits):
            raise TypeError('native session requires its original typed transport limits')
        maximum_chunks = limits.validate()
        if len(frame_buffer) != max(limits.max_call_bytes, limits.max_reply_bytes):
            raise ValueError('native session frame buffer differs from original largest allowance')
        if progress is not None and not callable(progress):
            raise TypeError("native session owner progress callback required")
        self._progress = progress
        self._peer = peer
        self._deadline = deadline
        self._cancelled = cancelled
        self._packet = receiver_buffer
        self._frame = frame_buffer
        self._limits = limits
        self._sent_request_bytes = 0
        self._received_reply_bytes = 0
        self._maximum_chunks = maximum_chunks
        self._spent_chunks = 0
        # Nonblocking IO only changes this owned endpoint, never global state.
        peer.setblocking(False)
        self._poll = select.poll()
        self._poll.register(peer, select.POLLIN | select.POLLOUT | select.POLLERR | select.POLLHUP)
        self._active()

    def _active(self):
        if self._progress is not None:
            self._progress()
        if self._cancelled.is_set():
            raise InterruptedError('native session original owner cancelled')
        if time.monotonic() >= self._deadline:
            raise TimeoutError('native session original cutoff expired')

    def _wait(self, event):
        while True:
            self._active()
            # No renewed socket timeout: every wait is inside the original cutoff.
            milliseconds = max(1, min(50, math.ceil((self._deadline - time.monotonic()) * 1000)))
            self._poll.modify(self._peer, event | select.POLLERR | select.POLLHUP)
            ready = self._poll.poll(milliseconds)
            self._active()
            if ready:
                flags = ready[0][1]
                if flags & event:
                    return
                raise EOFError('native session control endpoint ended before its complete frame')

    def _charge_chunk(self):
        self._active()
        if self._spent_chunks >= self._maximum_chunks:
            raise NativeSessionProtocolError('native session original cumulative chunk allowance exhausted')
        self._spent_chunks += 1

    @staticmethod
    def _fields(kind, sequence):
        if kind not in _KINDS or type(kind) is not int:
            raise NativeSessionProtocolError('unknown native session control kind')
        if type(sequence) is not int or not 0 <= sequence <= _U64_MAX:
            raise NativeSessionProtocolError('native session sequence is outside u64')

    def send(self, kind, sequence, payload):
        self._fields(kind, sequence)
        if kind not in (1, 4):
            raise NativeSessionProtocolError("SDK may send only call or close frames")
        if type(payload) is not bytes or len(payload) > self._limits.max_call_bytes:
            raise NativeSessionProtocolError('native session send exceeds its original frame allowance')
        if kind in (4, 5) and payload:
            raise NativeSessionProtocolError('native close control must be empty')
        chunks = max(1, (len(payload) + _PAYLOAD_BYTES - 1) // _PAYLOAD_BYTES)
        if chunks > self._limits.max_chunks_per_frame:
            raise NativeSessionProtocolError("native session original per-frame chunks exhausted")
        if self._sent_request_bytes + len(payload) > self._limits.max_total_request_bytes:
            raise NativeSessionProtocolError("native session original total request bytes exhausted")
        self._sent_request_bytes += len(payload)
        body = memoryview(payload)
        offset = 0
        first = True
        while first or offset < len(body):
            first = False
            self._charge_chunk()
            length = min(_PAYLOAD_BYTES, len(body) - offset)
            header = _HEADER.pack(_MAGIC, kind, 0, 0, sequence, offset, len(body), length)
            while True:
                self._wait(select.POLLOUT)
                try:
                    written = self._peer.sendmsg([header, body[offset:offset + length]], [], socket.MSG_NOSIGNAL)
                    break
                except BlockingIOError:
                    continue
            if written != _HEADER.size + length:
                raise NativeSessionProtocolError('native SEQPACKET write was not atomic')
            offset += length
            self._active()

    def receive(self, allowed_kinds, sequence):
        if not isinstance(allowed_kinds, frozenset) or not allowed_kinds or not allowed_kinds <= frozenset((2, 3, 5, 6)):
            raise NativeSessionProtocolError('native session expected kinds must be explicit')
        self._fields(next(iter(allowed_kinds)), sequence)
        packet = memoryview(self._packet)
        frame = memoryview(self._frame)
        total = kind = None
        offset = chunks = 0
        while True:
            self._charge_chunk()
            chunks += 1
            if chunks > self._limits.max_chunks_per_frame:
                raise NativeSessionProtocolError("native session original per-frame chunks exhausted")
            while True:
                self._wait(select.POLLIN)
                try:
                    # No ancillary capacity: all descriptors/credentials/control
                    # data are prohibited on this bytes-only issued endpoint.
                    size, controls, flags, _ = self._peer.recvmsg_into([packet], 0)
                    break
                except BlockingIOError:
                    continue
            if not size:
                raise EOFError('native session EOF before logical-frame completion; kind=' + str(kind)
                                   + '; offset=' + str(offset) + '; total=' + str(total)
                                   + '; packet_ordinal=' + str(chunks))
            if controls or flags & (socket.MSG_TRUNC | socket.MSG_CTRUNC):
                raise NativeSessionProtocolError('native session truncated or ancillary control packet')
            if size < _HEADER.size:
                raise NativeSessionProtocolError('native session packet header is incomplete')
            magic, incoming_kind, field_flags, reserved, incoming_sequence, incoming_offset, incoming_total, length = _HEADER.unpack_from(packet)
            if (magic != _MAGIC or field_flags or reserved or incoming_kind not in allowed_kinds
                    or incoming_sequence != sequence or incoming_offset != offset
                    or length > _PAYLOAD_BYTES or size != _HEADER.size + length
                    or incoming_total > self._limits.max_reply_bytes or offset > incoming_total
                    or length > incoming_total - offset):
                raise NativeSessionProtocolError('native session packet geometry or association differs')
            if total is None:
                total, kind = incoming_total, incoming_kind
                if self._received_reply_bytes + total > self._limits.max_total_reply_bytes:
                    raise NativeSessionProtocolError("native session original total reply bytes exhausted")
                self._received_reply_bytes += total
            if incoming_total != total or incoming_kind != kind:
                raise NativeSessionProtocolError('native session logical frame changed mid-delivery')
            if kind in (4, 5) and total:
                raise NativeSessionProtocolError('native close control must be empty')
            if not length and total:
                raise NativeSessionProtocolError('native session nonterminal empty chunk')
            frame[offset:offset + length] = packet[_HEADER.size:size]
            offset += length
            self._active()
            if offset == total:
                return kind, frame[:total]
