"""Count an existing SDK owner closure without copying its values.

The fixed traversal workspace is debited to the original receiving ledger.
This measures retained owners; it does not replace launch-peak pre-admission.
Global module/type/code/function objects are borrowed runtime owners. Bound
methods retain their real instance; shared/alias allocations are counted once.
"""
import collections
import gc
import sys
import types


def retained_owner_state(state, roots, *, maximum_objects):
    if type(roots) is not tuple or type(maximum_objects) is not int or maximum_objects <= 0:
        raise TypeError('native SDK bounded owner census selection required')
    g = state.geometry
    # Exact fixed list slots, without append/growing visited sets.
    code = retained_owner_state.__code__
    slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
    scalar = max(g.unicode_bytes(1), g.tuple_base + 4 * g.pointer,
                 g.range_iterator_base, int.__basicsize__ + 3 * g.int_digit)
    framework = types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
    # One fixed identity table covers both already-retained and new owners.
    # At most half full; every inspected slot spends the SAME original work.
    # No growing set/dict or linear scan of every previously seen allocation.
    identity_slots = 1
    while identity_slots < 2 * (len(state._external_owners) + maximum_objects):
        identity_slots <<= 1
    workspace = (2 * (g.list_base + maximum_objects * g.pointer)
                 + g.list_base + identity_slots * g.pointer + framework)
    state.reserve(workspace)
    seen = [None] * maximum_objects
    pending = [None] * maximum_objects
    # Borrowed skipped type is an unambiguous empty slot, including for None.
    identities = [types.ModuleType] * identity_slots
    pending_count = len(roots)
    if pending_count > maximum_objects:
        raise ValueError('native SDK initial owner census exceeds original cap')
    for index, value in enumerate(roots):
        pending[index] = value
    seen_count = total = 0
    try:
        for value in state._external_owners:
            slot = (id(value) >> 4) & (identity_slots - 1)
            while True:
                state.visit()
                existing = identities[slot]
                if existing is types.ModuleType or existing is value:
                    identities[slot] = value
                    break
                slot = (slot + 1) & (identity_slots - 1)
        while pending_count:
            state.visit()
            pending_count -= 1
            value = pending[pending_count]
            pending[pending_count] = None
            if isinstance(value, (type, types.ModuleType, types.CodeType)):
                continue
            if isinstance(value, types.FunctionType) and value.__closure__ is None:
                continue  # global source function/code, not a created closure
            slot = (id(value) >> 4) & (identity_slots - 1)
            while True:
                state.visit()
                existing = identities[slot]
                if existing is types.ModuleType or existing is value:
                    break
                slot = (slot + 1) & (identity_slots - 1)
            if existing is value:
                continue
            identities[slot] = value
            if seen_count == maximum_objects:
                raise ValueError('native SDK owner census exceeds original object cap')
            seen[seen_count] = value
            seen_count += 1
            total += sys.getsizeof(value)
            if isinstance(value, collections.deque):
                # deque.__sizeof__ counts active blocks only. Its per-instance
                # freeblock cache retains up to MAXFREEBLOCKS=16 blocks, each
                # BLOCKLEN=64 values plus two links, even after clear().
                total += 16 * (64 + 2) * g.pointer
            # Selected CPython3.14 GIL: INLINE_VALUES=(1 << 2). The
            # allocation appends shared-key values after __basicsize__, so
            # sys.getsizeof omits them even after a dict is materialized.
            # _PyInlineValuesSize: rounded order prefix + (capacity+1) slots.
            if sys.version_info[:2] == (3, 14) and type(value).__flags__ & 4:
                total += ((30 + g.pointer - 1) // g.pointer) * g.pointer
                total += (30 + 1) * g.pointer
            if total > state._limit - state._retained:
                raise MemoryError('native SDK retained owner closure exceeds original state')
            if type(value) in (str, bytes, bytearray, int, float, bool, type(None)):
                continue
            if isinstance(value, types.FunctionType):
                # A created closure owns these actual tuple/cell/default owners;
                # its globals/code/name are borrowed module authority.
                for child in (value.__closure__, value.__defaults__, value.__kwdefaults__):
                    if child is not None:
                        if pending_count == maximum_objects:
                            raise ValueError('native SDK closure pending cap')
                        pending[pending_count] = child
                        pending_count += 1
                continue
            if isinstance(value, dict):
                references = 2 * len(value)
                if type(value) is not dict:
                    # subtype_traverse adds instance slots/dict/type to the
                    # base container traversal. Do not materialize __dict__.
                    references += type(value).__basicsize__ // g.pointer
                    if type(value).__dictoffset__:
                        references += 30
            elif isinstance(value, (list, tuple, set, frozenset, collections.deque)):
                references = len(value)
                if isinstance(value, collections.deque):
                    # Its base traverse always visits Py_TYPE as well.
                    references += 1
                if type(value) not in (list, tuple, set, frozenset, collections.deque):
                    references += type(value).__basicsize__ // g.pointer
                    if type(value).__dictoffset__:
                        references += 30
            elif isinstance(value, types.GeneratorType):
                code = value.gi_code  # borrowed code; never materialize gi_frame
                references = (type(value).__basicsize__ // g.pointer + code.co_nlocals
                              + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize)
            else:
                # Fixed object slots plus CPython's managed instance values.
                # subtype_traverse visits one materialized dict OR up to
                # SHARED_KEYS_MAX_SIZE=30 inline values; __basicsize__ omits
                # that trailing storage. No __dict__ is materialized here.
                references = type(value).__basicsize__ // g.pointer
                if type(value).__dictoffset__:
                    references += 30
            # gc.get_referents uses append; final slots and realloc overlap are
            # admitted before that existing mechanical GC owner allocates.
            referent_scratch = 2 * g.list_bytes(references)
            state.reserve(referent_scratch)
            children = gc.get_referents(value)
            if len(children) > references or sys.getsizeof(children) > g.list_bytes(references):
                raise MemoryError('native SDK GC owner geometry exceeds source forecast')
            for child in children:
                if pending_count == maximum_objects:
                    raise ValueError('native SDK owner pending closure exceeds original cap')
                pending[pending_count] = child
                pending_count += 1
            del children
            state.release(referent_scratch)
        state.active()
        # Retain identity ownership so later factory census does not charge
        # the same selected arguments a second time. New exact-sized registry
        # and old list coexist before replacing the real controller field.
        old_registry = state._external_owners
        new_count = len(old_registry) + seen_count
        registry_bytes = g.list_base + new_count * g.pointer
        state.reserve(total + registry_bytes)
        registered = [None] * new_count
        if sys.getsizeof(registered) > registry_bytes:
            raise MemoryError('native SDK census registry geometry differs')
        for index, value in enumerate(old_registry):
            registered[index] = value
        for index in range(seen_count):
            registered[len(old_registry) + index] = seen[index]
        old_registry_bytes = sys.getsizeof(old_registry)
        state._external_owners = registered
        del old_registry
        state.release(old_registry_bytes)
        return total
    finally:
        # On success or failure, this function retains no traversal owners.
        # Failure is terminal; traceback-owned values cannot buy another call.
        del pending, seen, identities
        if sys.exception() is None:
            state.release(workspace)
