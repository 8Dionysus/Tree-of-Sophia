"""Owned ordinary Linux SourceRoot session using the maintained child issuer.

The caller supplies the original setup ReceiverState. Configuration is checked
against held kernel placement by the launcher before any native result. This
factory owns no Stage/model FD in Python and never selects a Reference fallback.
"""
from contextlib import contextmanager
import types

from .native_core_session import (NativeSDKStageConfiguration, NativeSDKHostSessionRequest,
                                  owned_native_sdk_session, _path)
from .native_core_session_receiver import ReceiverState
from .native_core_session_result import NativeCoreOrdinarySessionResultClient
from .native_core_session_census import retained_owner_state
from .native_core_session_control import _PACKET_BYTES, NativeSessionLimits
from .native_core_session_startup import ordinary_startup_bytes
from .native_core_snapshot import NativeCoreSnapshotSelection




@contextmanager
def owned_native_ordinary_source_session(*, prefix, selection, transport, state,
        cancelled, config=None, maximum_owner_objects, search_read_model=None,
        snapshot_root=None, expected_snapshot_guard=None, release_root=None,
        expected_reference_release_guard=None):
    """Use the native-owned operation profile with existing caller transport.

    Stage/Cold/Query/publication authority never enters this Python constructor.
    The existing bootstrap owns placement and cutoff; the caller keeps its same
    setup receiving ledger and six finite transport fields through close.
    """
    if (not isinstance(state, ReceiverState)
            or not isinstance(selection, NativeCoreSnapshotSelection)
            or not isinstance(transport, NativeSessionLimits)):
        raise TypeError('native ordinary session requires original typed transport owners')
    g = state.geometry
    code = owned_native_ordinary_source_session.__wrapped__.__code__
    slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
    scalar = max(g.unicode_bytes(20), g.tuple_base + 9 * g.pointer,
                 int.__basicsize__ + 3 * g.int_digit)
    state.reserve(types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
                  + types.GeneratorType.__basicsize__ + g.gc_header + slots * g.pointer)
    retained_owner_state(state, (prefix, selection, transport, cancelled, config, search_read_model,
                                  snapshot_root, expected_snapshot_guard, release_root),
                         maximum_objects=maximum_owner_objects)
    prefix = _path(prefix, state)
    if config is None:
        import os
        config = (NativeSDKStageConfiguration.from_bootstrap_environment(receiving_state=state)
                  if 'TOS_SDK_STAGE_CONFIG' in os.environ else
                  NativeSDKHostSessionRequest.from_operation_state(state))
    if not isinstance(config, (NativeSDKStageConfiguration, NativeSDKHostSessionRequest)):
        raise TypeError('native ordinary session requires authentic bootstrap configuration')
    if (state._deadline != config.original_work_deadline_ns / 1e9
            or state._cancelled is not cancelled or state._limit != 536870912):
        raise ValueError('native ordinary original setup owner differs')
    transport.validate()
    serializer_code = ordinary_startup_bytes.__code__
    serializer_slots = (serializer_code.co_nlocals + len(serializer_code.co_cellvars)
                        + len(serializer_code.co_freevars) + serializer_code.co_stacksize)
    state.reserve(types.FrameType.__basicsize__ + g.gc_header
                  + serializer_slots * (g.pointer + scalar))
    wire = ordinary_startup_bytes(selection, transport, config, state,
                                  search_read_model=search_read_model, snapshot_root=snapshot_root,
                                  expected_snapshot_guard=expected_snapshot_guard,
                                  expected_reference_release_guard=expected_reference_release_guard)
    frame_size = max(transport.max_call_bytes, transport.max_reply_bytes)
    state.reserve(2 * bytearray.__basicsize__ + _PACKET_BYTES + frame_size + 2)
    receiver, frame = bytearray(_PACKET_BYTES), bytearray(frame_size)
    with owned_native_sdk_session(prefix=prefix, root=selection.tos_root,
            startup_bytes=wire, limits=transport, cancelled=cancelled,
            receiver_buffer=receiver, frame_buffer=frame, config=config,
            receiving_state=state, session_operation='tos_native_ordinary_session',
            snapshot_root=snapshot_root, release_root=release_root,
            search_cache_path=(search_read_model['path'] if search_read_model is not None
                                and not selection.query_store_configured else None)) as (session, startup):
        constructor = NativeCoreOrdinarySessionResultClient.__new__.__code__
        constructor_slots = (constructor.co_nlocals + len(constructor.co_cellvars)
                             + len(constructor.co_freevars) + constructor.co_stacksize)
        state.reserve(types.FrameType.__basicsize__ + g.gc_header
                      + constructor_slots * (g.pointer + scalar))
        client = NativeCoreOrdinarySessionResultClient(session, state, startup,
                                                      config.original_work_deadline_ns)
        yield client
        # The existing session owner waits for close ACK plus terminal custody.
        # Successful dictionaries and native receipts stay in this same ledger.
