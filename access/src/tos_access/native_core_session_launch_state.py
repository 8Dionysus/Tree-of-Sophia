"""Pre-admit the existing SDK launcher owners within original setup state.

Source code geometry and explicit pipe/frame capacities are the contract. This
is logical retained state, independent of OS residency or allocator bookkeeping.
"""
import io
import contextlib
import socket
import subprocess
import threading
import types


def reserve_sdk_launch(state, config, prefix, root, *, session_operation='tos_native_session',
                       input_cap=65536, frame_cap=65536):
    from . import native_io
    from .native_core_session import (NativeSDKPlacement, NativeSDKHostCustody, NativeSDKHostSessionRequest, NativeSDKSession,
                                      owned_native_sdk_session, owned_native_snapshot_exchange)
    from .native_core_session_control import NativeSessionControl
    if type(input_cap) is not int or type(frame_cap) is not int or min(input_cap, frame_cap) <= 0:
        raise ValueError('native launcher actual transport capacities required')
    g = state.geometry
    scalar = max(g.unicode_bytes(1), g.tuple_base + 5 * g.pointer,
                 int.__basicsize__ + 3 * g.int_digit)
    frame_state = 0
    names = 0
    # Class dictionaries/code objects are original borrowed module owners.
    # Summing code attribute-name slots is a source-derived upper bound for
    # their instance dictionaries, including platform branches; no magic count.
    owners = (subprocess.Popen, native_io._Exchange, NativeSDKPlacement,
                  NativeSDKSession, NativeSessionControl, threading.Event,
                  threading.Condition, contextlib.ExitStack,
                  contextlib._GeneratorContextManager, NativeSDKHostCustody)
    for owner in owners:
        for method in owner.__dict__.values():
            if isinstance(method, types.FunctionType):
                code = method.__code__
                names += len(code.co_names)
                slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
                frame_state += types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
        frame_state += owner.__basicsize__ + g.gc_header
    for fn in (native_io._environment, native_io._borrowed_fds, native_io._contract,
               native_io.owned_exchange.__wrapped__,
               owned_native_sdk_session.__wrapped__, owned_native_snapshot_exchange.__wrapped__, reserve_sdk_launch):
        code = fn.__code__
        slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
        frame_state += types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
    # Nine concrete controller kinds above; two simultaneous dictionary
    # tables bound each resize. Three generator-context instances are live.
    instance_state = (len(owners) + 2) * 2 * g.dict_bytes(names)
    # The base47 argv entries include four authenticated setup-AS selectors.
    # Native-issued search-cache selection adds four exact flag/value pairs.
    # Three simultaneous source/native/Popen argument lists and exact C argv
    # pointer/descriptor arrays coexist before successful exec.
    cache = getattr(config, 'search_cache', None)
    count = 47 + (8 if cache is not None else 0)
    argument_state = (3 * g.list_bytes(count) + (count + 1) * g.pointer + 16 * 4
                      + g.tuple_base + 6 * g.pointer)
    if cache is not None:
        argument_state += g.tuple_base + 2 * g.pointer
    # Selected paths have their original4096-byte bound. Their cached strings
    # are caller-owned; formatted argv and encoded exec bytes are separate.
    selected_paths = ((prefix, root) if isinstance(config, NativeSDKHostSessionRequest) else
                      (config.unshare_exe, config.consumer_cgroup,
                       config.scratch_parent, config.persistent_store, prefix, root))
    for selected in selected_paths:
        if selected is None:
            continue
        parts = getattr(selected, '_tail_cached', None)
        if parts is None:
            parts = getattr(selected, '_parts', None)
        if type(parts) is not list:
            raise ValueError('native launcher selected Path cache unavailable')
        characters = len(parts) + 2
        for part in parts:
            state.visit()
            characters += len(part)
        if characters > 4098:
            raise ValueError('native launcher original path cap differs')
        argument_state += (2 * g.unicode_bytes(characters + 64)
                           + 2 * (g.bytes_base + 4 * (characters + 64)))
    if cache is not None:
        for selected in (cache.path, cache.source_root):
            parts = getattr(selected, '_tail_cached', None)
            if parts is None:
                parts = getattr(selected, '_parts', None)
            if type(parts) is not list:
                raise ValueError('native launcher selected Path cache unavailable')
            characters = len(parts) + 2
            for part in parts:
                state.visit()
                characters += len(part)
            if characters > 4098:
                raise ValueError('native launcher original path cap differs')
            argument_state += (2 * g.unicode_bytes(characters + 64)
                               + 2 * (g.bytes_base + 4 * (characters + 64)))
    argument_state += (10 + (2 if cache is not None else 0)) * (g.unicode_bytes(20) + g.bytes_base + 20)
    # Popen's _fork_exec preparation encodes borrowed static argv literals
    # too. Price each actual source constant independently, not inside frame
    # scalar scratch; the direct launcher constants form a conservative owned
    # superset of its fixed argv entries. No encoding occurs during forecast.
    for owner in (owned_native_sdk_session.__wrapped__, owned_native_snapshot_exchange.__wrapped__, native_io._Exchange._open):
        for literal in owner.__code__.co_consts:
            if type(literal) is str:
                state.visit()
                argument_state += g.bytes_base + 4 * len(literal)
    # The selected operation is a borrowed immutable source identifier; its
    # encoded exec argv bytes are distinct and admitted before Popen.
    state.visit()
    argument_state += g.bytes_base + 4 * len(session_operation)
    # Popen's existing default bufsize creates three default-sized IO buffers.
    # stderr/stdout transport capacities and resize overlap remain distinct.
    pipe_state = (3 * (io.FileIO.__basicsize__ + g.gc_header)
                  + io.BufferedWriter.__basicsize__ + 2 * io.BufferedReader.__basicsize__
                  + 3 * io.DEFAULT_BUFFER_SIZE
                  + 4 * (bytearray.__basicsize__ + frame_cap + 1)
                  + 2 * (g.bytes_base + frame_cap))
    socket_state = 2 * (socket.socket.__basicsize__ + g.gc_header)
    # Event/Condition waiter deques and locks are actual fixed owner structures;
    # source methods above cover their instance dictionaries and frames.
    import _thread, collections
    synchronization = (4 * _thread.LockType.__basicsize__
                       + 2 * (collections.deque.__basicsize__ + g.gc_header + 66 * g.pointer))
    # ExitStack has <=5 registered closure/exit callbacks. Each owns wrapper,
    # bound method and argument/keyword/entry tuples, not another child lease.
    callbacks = 5 * (types.FunctionType.__basicsize__ + g.gc_header
                    + types.MethodType.__basicsize__ + g.gc_header
                    + 3 * (g.tuple_base + 2 * g.pointer) + g.dict_bytes(0)
                    + 2 * (types.CellType.__basicsize__ + g.gc_header))
    # pthread_sigmask returns the full original kernel mask, not only the four
    # selecting cancellation signals. Up to64 scalar members, set resize
    # overlap, sorted list, join-materialized string list and original CSV
    # coexist before Popen. Linux signal numbers have at most two digits.
    mask_slots = 8
    while mask_slots < 4 * 64:
        mask_slots <<= 1
    signal_mask_state = (set.__basicsize__ + g.gc_header
                         + 2 * mask_slots * 2 * g.pointer + 64 * scalar
                         + 3 * g.list_bytes(64) + 64 * g.unicode_bytes(2)
                         + g.unicode_bytes(64 * 3 - 1)
                         + g.bytes_base + 64 * 3 - 1)
    for code in native_io._Exchange._open.__code__.co_consts:
        if isinstance(code, types.CodeType):
            slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            signal_mask_state += (types.GeneratorType.__basicsize__ + g.gc_header
                                  + types.FunctionType.__basicsize__ + g.gc_header
                                  + types.FrameType.__basicsize__ + g.gc_header
                                  + slots * (g.pointer + scalar)
                                  + g.tuple_base + len(code.co_freevars) * g.pointer
                                  + len(code.co_freevars) * (types.CellType.__basicsize__ + g.gc_header))
    # Placement verification can run while all pipes/frame buffers are live.
    # Its bounded cgroup read, strip strings, structseq stamps and /proc Path
    # allocations are distinct owners, not aliases of transport diagnostics.
    import os
    stat_owner = os.stat_result
    stat_state = 4 * (stat_owner.__basicsize__ + stat_owner.n_fields * g.pointer
                      + stat_owner.n_fields * scalar)
    placement_state = (2 * (bytearray.__basicsize__ + 4097)
                       + g.bytes_base + 4097 + 2 * g.unicode_bytes(4096)
                       + stat_state + 2 * (type(root).__basicsize__ + g.gc_header
                                          + g.dict_bytes(8) + g.list_bytes(8)))
    total = signal_mask_state + placement_state + frame_state + instance_state + argument_state + pipe_state + socket_state + synchronization + callbacks
    state.reserve(total)
    return total
