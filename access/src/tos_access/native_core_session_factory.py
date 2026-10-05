"""Owned ordinary Linux SourceRoot session using the maintained child issuer.

The caller supplies the original setup ReceiverState. Configuration is checked
against held kernel placement by the launcher before any native result. This
factory owns no Stage/model FD in Python and never selects a Reference fallback.
"""
from contextlib import contextmanager
import types

from .native_core_session import (NativeSDKStageConfiguration, NativeSDKHostSessionRequest,
                                  owned_native_sdk_session, _path)
from .native_core_session_admission import NativeCoreSessionAdmission
from .native_core_session_receiver import ReceiverState
from .native_core_session_result import NativeCoreSessionResultClient
from .native_core_probe_session_result import NativeCoreProbeSessionResultClient
from .native_core_lazy_session_result import (NativeCoreLazySessionResultClient,
                                             NativeCoreOrdinarySessionResultClient)
from .native_core_session_census import retained_owner_state
from .native_core_session_control import _PACKET_BYTES, NativeSessionLimits
from .native_core_session_startup import startup_bytes, ordinary_startup_bytes
from .native_core_snapshot import NativeCoreSnapshotSelection


@contextmanager
def owned_native_source_session(*, prefix, selection, admission, state,
                                cancelled, config=None, maximum_owner_objects,
                                _selected_probe=False, _selected_lazy=False):
    if (not isinstance(state, ReceiverState)
            or not isinstance(selection, NativeCoreSnapshotSelection)
            or not isinstance(admission, NativeCoreSessionAdmission)):
        raise TypeError('native source session requires original typed owners')
    if type(_selected_probe) is not bool or type(_selected_lazy) is not bool or (_selected_probe and _selected_lazy):
        raise TypeError('native session owner profile required')
    if selection.query_store_configured and not (_selected_probe or _selected_lazy):
        raise ValueError('ordinary captured Root session cannot substitute selected QueryStore')
    g = state.geometry
    code = owned_native_source_session.__wrapped__.__code__
    slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
    scalar = max(g.unicode_bytes(1), g.tuple_base + 5 * g.pointer,
                 int.__basicsize__ + 3 * g.int_digit)
    state.reserve(types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
                  + types.GeneratorType.__basicsize__ + g.gc_header + slots * g.pointer)
    # These source-selected owners existed before this receiving controller.
    # Identity aliases are measured once. New config/transport/result owners
    # each have their own allocation pre-admission, never a fresh whole grant.
    roots = (prefix, selection, admission, cancelled, config)
    retained_owner_state(state, roots, maximum_objects=maximum_owner_objects)
    prefix = _path(prefix, state)
    if config is None:
        config = NativeSDKStageConfiguration.from_bootstrap_environment(receiving_state=state)
    if not isinstance(config, NativeSDKStageConfiguration):
        raise TypeError('native session requires maintained bootstrap configuration')
    if (state._deadline != config.original_work_deadline_ns / 1e9
            or state._cancelled is not cancelled or state._limit != 536870912):
        raise ValueError('native session original setup clock/cancellation/allowance differs')
    wire = startup_bytes(admission, selection, config, state, selected_probe=_selected_probe, selected_lazy=_selected_lazy)
    # Codec's owned bytearrays are a distinct original setup allocation.
    receive_size = _PACKET_BYTES
    frame_size = max(admission.transport.max_call_bytes, admission.transport.max_reply_bytes)
    state.reserve(2 * bytearray.__basicsize__ + receive_size + frame_size + 2)
    receiver = bytearray(receive_size)
    frame = bytearray(frame_size)
    with owned_native_sdk_session(prefix=prefix, root=selection.tos_root,
            startup_bytes=wire, limits=admission.transport, cancelled=cancelled,
            receiver_buffer=receiver, frame_buffer=frame, config=config,
            receiving_state=state,
            session_operation='tos_native_lazy_session' if _selected_lazy else
                              'tos_native_probe_session' if _selected_probe else 'tos_native_session') as (session, startup):
        client_class = NativeCoreLazySessionResultClient if _selected_lazy else NativeCoreProbeSessionResultClient if _selected_probe else NativeCoreSessionResultClient
        # The actual constructor controller must be admitted before entering it;
        # its own result/framework reserve occurs only after its planning walk.
        constructor_code = client_class.__new__.__code__
        constructor_slots = (constructor_code.co_nlocals + len(constructor_code.co_cellvars)
                             + len(constructor_code.co_freevars) + constructor_code.co_stacksize)
        state.reserve(types.FrameType.__basicsize__ + g.gc_header
                      + constructor_slots * (g.pointer + scalar))
        client = client_class(session, state, startup,
                                               config.original_work_deadline_ns)
        yield client
        # All successful public dictionaries remain charged in the original
        # state owner. Session close performs ACK/EOF/terminal custody; parent
        # verified_image remains held through that exact final lifetime.


@contextmanager
def owned_native_discovered_source_session(*, prefix, prepared, admission, state,
        cancelled, carrier_selectors, maximum_owner_objects, tos_root=None,
        query_store_path=None, config=None, selected_probes=False, lazy_selected=False):
    """Connect ordinary explicit/env/root discovery to one owned child scope.

    This is the declared SourceRoot or four-probe subset. A configured OR
    existing default Store never silently becomes SourceRoot. The future lazy
    selected Store/carrier/Root Driver remains a distinct native owner change.
    """
    if (not isinstance(state, ReceiverState)
            or not isinstance(admission, NativeCoreSessionAdmission)
            or type(carrier_selectors) is not dict or type(selected_probes) is not bool
            or type(lazy_selected) is not bool or (selected_probes and lazy_selected)):
        raise TypeError('native discovered session requires original typed owners')
    g = state.geometry
    code = owned_native_discovered_source_session.__wrapped__.__code__
    slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
    scalar = max(g.unicode_bytes(1), g.tuple_base + 11 * g.pointer,
                 g.int_base + 3 * g.int_digit)
    state.reserve(types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
                  + types.GeneratorType.__basicsize__ + g.gc_header + slots * g.pointer)
    retained_owner_state(state, (prefix, prepared, admission, cancelled, config,
                                tos_root, query_store_path, carrier_selectors),
                         maximum_objects=maximum_owner_objects)
    if config is None:
        config = NativeSDKStageConfiguration.from_bootstrap_environment(receiving_state=state)
    if not isinstance(config, NativeSDKStageConfiguration):
        raise TypeError('native discovery requires original bootstrap configuration')
    if (state._deadline != config.original_work_deadline_ns / 1e9
            or state._cancelled is not cancelled or state._limit != 536870912):
        raise ValueError('native discovery original setup owner differs')
    config.active()
    selection = NativeCoreSnapshotSelection.discover_under_admission(prepared, state,
        tos_root=tos_root, query_store_path=query_store_path,
        carrier_selectors=carrier_selectors, maximum_owner_objects=maximum_owner_objects,
        source_root_only=not (selected_probes or lazy_selected))
    with owned_native_source_session(prefix=prefix, selection=selection,
            admission=admission, state=state, cancelled=cancelled, config=config,
            maximum_owner_objects=maximum_owner_objects,
            _selected_probe=selected_probes, _selected_lazy=lazy_selected) as client:
        yield client


@contextmanager
def owned_native_ordinary_source_session(*, prefix, selection, transport, state,
        cancelled, config=None, maximum_owner_objects, search_read_model=None,
        snapshot_root=None, expected_snapshot_guard=None):
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
    retained_owner_state(state, (prefix, selection, transport, cancelled, config, search_read_model, snapshot_root, expected_snapshot_guard),
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
                                  expected_snapshot_guard=expected_snapshot_guard)
    frame_size = max(transport.max_call_bytes, transport.max_reply_bytes)
    state.reserve(2 * bytearray.__basicsize__ + _PACKET_BYTES + frame_size + 2)
    receiver, frame = bytearray(_PACKET_BYTES), bytearray(frame_size)
    with owned_native_sdk_session(prefix=prefix, root=selection.tos_root,
            startup_bytes=wire, limits=transport, cancelled=cancelled,
            receiver_buffer=receiver, frame_buffer=frame, config=config,
            receiving_state=state, session_operation='tos_native_ordinary_session',
            snapshot_root=snapshot_root,
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
