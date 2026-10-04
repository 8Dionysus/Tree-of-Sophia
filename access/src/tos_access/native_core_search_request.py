"""ASCII JSON request mechanics for the genuine weak Store Search owner.

No domain query, normalization, ranking or SQLite implementation lives here.
All encoded owners are admitted before allocation under ReceiverState.
"""

_HEX = '0123456789abcdef'
_PREFIX = ',"operation":"tos_native_call","arguments":{"tool":"tos_knowledge_search","arguments":'
_SUFFIX = '},"work_deadline_ns":'
_KEYS = ('query', 'sources', 'kind_ids', 'predicate_ids', 'offset', 'limit')


def _string(state, value):
    state.active()
    if type(value) is not str:
        raise ValueError('native Search string required')
    # count*12 bounds exact ASCII surrogate-pair spelling of one PEP393 scalar.
    # Two list resize owners, per-scalar escaped String and joined result coexist.
    g = state.geometry
    count = len(value)
    amount = (2 * g.list_bytes(count) + count * g.unicode_bytes(12)
              + 2 * g.unicode_bytes(12 * count + 2) + 12 * g.unicode_bytes(12)
              + g.tuple_base + 8 * g.pointer
              + object.__basicsize__ + 2 * g.pointer + g.gc_header)
    state.reserve(amount)
    parts = []
    for char in value:
        state.visit()
        code = ord(char)
        if char == '"' or char == '\\':
            part = '\\' + char
        elif 0x20 <= code < 0x7f:
            part = char
        elif code <= 0xffff:
            part = '\\u' + _HEX[(code >> 12) & 15] + _HEX[(code >> 8) & 15] + _HEX[(code >> 4) & 15] + _HEX[code & 15]
        else:
            code -= 0x10000
            high, low = 0xd800 + (code >> 10), 0xdc00 + (code & 1023)
            part = ('\\u' + _HEX[(high >> 12) & 15] + _HEX[(high >> 8) & 15] + _HEX[(high >> 4) & 15] + _HEX[high & 15]
                    + '\\u' + _HEX[(low >> 12) & 15] + _HEX[(low >> 8) & 15] + _HEX[(low >> 4) & 15] + _HEX[low & 15])
        parts.append(part)
    result = '"' + ''.join(parts) + '"'
    state.active()
    return result, amount


def validate_search_arguments(state, arguments):
    """Reject foreign objects using borrowed exact types before owner census."""
    if type(arguments) is not dict or len(arguments) > 6:
        raise ValueError('native Search public argument shape differs')
    for key, value in arguments.items():
        state.visit()
        if type(key) is not str or key not in _KEYS:
            raise ValueError('native Search public argument unavailable')
        if key == 'query':
            if type(value) is not str:
                raise ValueError('native Search query must be a string')
        elif key in ('offset', 'limit'):
            maximum = 100000 if key == 'offset' else 100
            if type(value) is not int or not 0 <= value <= maximum:
                raise ValueError('native Search public integer outside original bounds')
        elif value is not None:
            if type(value) is not list:
                raise ValueError('native Search public filter must be a list')
            for entry in value:
                state.visit()
                if type(entry) is not str:
                    raise ValueError('native Search public filter element must be a string')


def search_fragment(state, arguments, max_call_bytes):
    # The sole client invokes the shared exact borrowed validator before census.
    state.active()
    g = state.geometry
    # Key fragments are borrowed code constants; result members may grow only
    # six slots, with old/new list owners, pair tuples and bounded decimal ints.
    amount = (2 * g.list_bytes(6) + 8 * g.unicode_bytes(20)
              + 8 * (g.tuple_base + 2 * g.pointer))
    state.reserve(amount)
    parts = []
    for key in _KEYS:
        state.visit()
        if key not in arguments:
            continue
        value = arguments[key]
        if key == 'query':
            encoded, cost = _string(state, value)
            amount += cost
        elif key in ('offset', 'limit'):
            maximum = 100000 if key == 'offset' else 100
            encoded = str(value)
        elif value is None:
            encoded = 'null'
        else:
            count = len(value)
            # Each element needs at least three ASCII bytes (quotes + comma).
            if count > max_call_bytes // 3:
                raise ValueError('native Search filter exceeds original call frame')
            arrays = 2 * g.list_bytes(count) + 2 * g.unicode_bytes(max_call_bytes)
            state.reserve(arrays)
            amount += arrays
            entries = []
            for entry in value:
                state.visit()
                item, cost = _string(state, entry)
                amount += cost
                entries.append(item)
            encoded = '[' + ','.join(entries) + ']'
        # Admit actual intermediate member string before concatenation.
        member_cost = 2 * g.unicode_bytes(len(key) + len(encoded) + 3) + 3 * g.unicode_bytes(20)
        state.reserve(member_cost)
        amount += member_cost
        parts.append('"' + key + '":' + encoded)
    length = len(_PREFIX) + len(_SUFFIX) + 2
    for part in parts:
        state.visit()
        length += len(part) + 1
    if length + 80 > max_call_bytes:
        raise ValueError('native Search request exceeds original call frame')
    # join, braces and prefix/suffix concatenation are distinct live Strings.
    final_cost = 4 * g.unicode_bytes(length)
    state.reserve(final_cost)
    amount += final_cost
    result = _PREFIX + '{' + ','.join(parts) + '}' + _SUFFIX
    state.active()
    return result, amount
