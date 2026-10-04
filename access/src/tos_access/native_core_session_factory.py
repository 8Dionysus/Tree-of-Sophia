"""Owned ordinary Linux SourceRoot session using the maintained child issuer.

The caller supplies the original setup ReceiverState. Configuration is checked
against held kernel placement by the launcher before any native result. This
factory owns no Stage/model FD in Python and never selects a Reference fallback.
"""
from contextlib import contextmanager
import types

from .native_core_session import (NativeSDKStageConfiguration,
                                  owned_native_sdk_session, _path)
from .native_core_session_admission import NativeCoreSessionAdmission
from .native_core_session_receiver import ReceiverState
from .native_core_session_result import NativeCoreSessionResultClient
from .native_core_probe_session_result import NativeCoreProbeSessionResultClient
from .native_core_lazy_session_result import NativeCoreLazySessionResultClient
from .native_core_session_census import retained_owner_state
from .native_core_session_startup import startup_bytes
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
    receive_size = admission.transport.max_reply_bytes
    frame_size = 65536
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
