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
    workspace = 2 * (g.list_base + maximum_objects * g.pointer) + framework
    state.reserve(workspace)
    seen = [None] * maximum_objects
    pending = [None] * maximum_objects
    pending_count = len(roots)
    if pending_count > maximum_objects:
        raise ValueError('native SDK initial owner census exceeds original cap')
    for index, value in enumerate(roots):
        pending[index] = value
    seen_count = total = 0
    try:
        while pending_count:
            state.visit()
            pending_count -= 1
            value = pending[pending_count]
            pending[pending_count] = None
            if isinstance(value, (type, types.ModuleType, types.CodeType)):
                continue
            if isinstance(value, types.FunctionType) and value.__closure__ is None:
                continue  # global source function/code, not a created closure
            duplicate = False
            for existing in state._external_owners:
                state.visit()
                if existing is value:
                    duplicate = True
                    break
            if duplicate:
                continue
            for index in range(seen_count):
                state.visit()
                if seen[index] is value:
                    duplicate = True
                    break
            if duplicate:
                continue
            if seen_count == maximum_objects:
                raise ValueError('native SDK owner census exceeds original object cap')
            seen[seen_count] = value
            seen_count += 1
            total += sys.getsizeof(value)
            if total > state._limit - state._retained:
                raise MemoryError('native SDK retained owner closure exceeds original state')
            if isinstance(value, (str, bytes, bytearray, int, float, bool, type(None))):
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
            if type(value) is dict:
                references = 2 * len(value)
            elif isinstance(value, (list, tuple, set, frozenset, collections.deque)):
                references = len(value)
            elif isinstance(value, types.GeneratorType):
                code = value.gi_code  # borrowed code; never materialize gi_frame
                references = (type(value).__basicsize__ // g.pointer + code.co_nlocals
                              + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize)
            else:
                # Fixed inline pointer fields bound generic GC traversal. A
                # dynamic __dict__ is a separately visited owner, not copied.
                references = type(value).__basicsize__ // g.pointer
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
        del pending, seen
        if sys.exception() is None:
            state.release(workspace)
