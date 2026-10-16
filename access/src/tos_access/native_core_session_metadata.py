"""Original state admission around the maintained stdlib TOML toolchain reader.

Only parsing mechanics are bounded here. tomllib retains syntax/interpretation;
this owner supplies its existing parse_float hook using the same exact rational
receiver, rather than introducing an opaque dtoa scratch/cache allocation.
"""
import datetime
import math
import re
import sys
import tomllib
import types


def bounded_toolchain_toml(raw, state):
    if type(raw) is not bytes or len(raw) > 8192:
        raise ValueError('native installed toolchain metadata exceeds original cap')
    n = len(raw)
    g = state.geometry
    slots = 8
    # CPython set growth uses fourfold minused for <=50000 entries. A final
    # power-of-two table is <8*entries; old/new tables can coexist at resize.
    while slots < 4 * max(1, n):
        slots <<= 1
    set_peak = set.__basicsize__ + g.gc_header + 2 * slots * 2 * g.pointer
    tuple_key = g.tuple_base + n * g.pointer
    integer = g.int_base + max(1, (4 * n + g.int_digit_bits - 1) // g.int_digit_bits) * g.int_digit
    scalar_node = max(g.unicode_bytes(n), integer, g.float_base,
                      datetime.datetime.__basicsize__, datetime.time.__basicsize__,
                      datetime.date.__basicsize__, datetime.timedelta.__basicsize__,
                      datetime.timezone.__basicsize__)
    parsed_node = max(2 * g.dict_bytes(max(1, n)), 2 * g.list_bytes(n), scalar_node)
    # Each namespace consumes an input key character. Flags.set creates one
    # three-field record with two inline sets (only FROZEN/EXPLICIT_NEST) and
    # its nested dictionary. Pending records own key/flag tuples independently.
    flags_node = (g.dict_bytes(3) + 2 * (set.__basicsize__ + g.gc_header)
                  + 2 * g.dict_bytes(max(1, n)) + tuple_key + g.tuple_base + 2 * g.pointer)
    module = tomllib._parser
    frames = 0
    from tomllib import _re
    for owner_module in (module, _re):
        for value in owner_module.__dict__.values():
            if isinstance(value, types.FunctionType):
                code = value.__code__
                local_slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
                frames += types.FrameType.__basicsize__ + g.gc_header + local_slots * (
                    g.pointer + max(g.unicode_bytes(1), g.tuple_base + 4 * g.pointer,
                                    int.__basicsize__ + 3 * g.int_digit))
    # Output/Flags/NestedDict instance methods are genuine parser owners,
    # not top-level module functions. Their nested code and inline dictionaries
    # coexist with the parsed result and pending namespace records.
    for owner_type in (module.Output, module.Flags, module.NestedDict):
        for method in owner_type.__dict__.values():
            if isinstance(method, types.FunctionType):
                code = method.__code__
                local_slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
                frames += types.FrameType.__basicsize__ + g.gc_header + local_slots * (
                    g.pointer + max(g.unicode_bytes(1), g.tuple_base + 4 * g.pointer,
                                    int.__basicsize__ + 3 * g.int_digit))
    instance_peak = sum(owner.__basicsize__ + g.gc_header + g.dict_bytes(3)
                        for owner in (module.Output, module.Flags, module.NestedDict))
    # make_safe_parse_float creates one nested interpreter frame. Its closure
    # allocation is priced separately below; original source code is borrowed.
    for code in module.make_safe_parse_float.__code__.co_consts:
        if isinstance(code, types.CodeType):
            local_slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            frames += types.FrameType.__basicsize__ + g.gc_header + local_slots * (
                g.pointer + max(g.unicode_bytes(1), g.tuple_base + 4 * g.pointer,
                                int.__basicsize__ + 3 * g.int_digit))
    # Syntax nesting consumes input characters; non-consuming helper chains
    # are included together in the sum of original parser function frames.
    frame_peak = (n + 1) * frames
    groups = max(module.RE_NUMBER.groups, module.RE_DATETIME.groups, module.RE_LOCALTIME.groups)
    match_peak = (re.Match.__basicsize__ + 2 * (groups + 1) * g.pointer
                  + (groups + 1) * g.unicode_bytes(n)
                  + g.tuple_base + (groups + 1) * g.pointer
                  + (groups + 1) * (g.tuple_base + 2 * g.pointer))
    # Raw UTF8 decoding old/new buffers, loads CRLF normalization, and active
    # basic-string old/part/join buffers are separate, source-named owners.
    strings_peak = (2 * g.unicode_bytes(n) + g.unicode_bytes(n)
                    + 3 * g.unicode_bytes(n))
    closure_peak = (2 * (types.FunctionType.__basicsize__ + g.gc_header)
                    + 2 * (g.tuple_base + 3 * g.pointer)
                    + 6 * (types.CellType.__basicsize__ + g.gc_header))
    # Below6000 source digits CPython converts nonbinary input into one
    # preallocated long plus normalized token/ASCII scratch. At/above6000 the
    # conservative namespace/table forecast already exceeds original512M;
    # such input refuses BEFORE stdlib can enter its Python big-int fallback.
    numeric_peak = integer + g.unicode_bytes(n) + g.bytes_base + n
    # cached_tz uses maintained functools.lru_cache. Each newly retained key
    # consumes datetime syntax input; key tuple/three numeric strings, timezone
    # and the C PyObject_HEAD plus five-pointer lru_list_elem coexist with its dict old/new table.
    timezone_cache = (2 * g.dict_bytes(max(1, n)) + n * (
        7 * g.pointer + g.tuple_base + 3 * g.pointer
        + 3 * g.unicode_bytes(n) + datetime.timezone.__basicsize__
        + datetime.timedelta.__basicsize__))
    workspace = (n * (parsed_node + flags_node + tuple_key) + set_peak + frame_peak
                 + instance_peak + timezone_cache
                 + match_peak + strings_peak + closure_peak + numeric_peak)
    state.reserve(workspace)
    # Flags prefix walks can revisit the same key characters; meter their
    # conservative original quadratic statement work before entering stdlib.
    for _ in range(n * n + n):
        state.visit()

    def parse_float(token):
        state.active()
        if token in ('inf', '+inf', '-inf', 'nan', '+nan', '-nan'):
            state.reserve(g.float_base)
            if token.endswith('nan'):
                return -math.nan if token.startswith('-') else math.nan
            return -math.inf if token.startswith('-') else math.inf
        scratch = (2 * g.unicode_bytes(len(token)) + g.bytes_base + len(token)
                   + memoryview.__basicsize__ + g.gc_header)
        state.reserve(scratch)
        clean = token.replace('_', '')
        if clean.startswith('+'):
            clean = clean[1:]
        encoded = clean.encode('ascii')
        result = state.decode(memoryview(encoded))
        del clean, encoded
        state.release(scratch)
        if type(result) is not float:
            raise ValueError('native toolchain parse_float result type differs')
        return result

    text = raw.decode('utf-8', 'strict')
    result = tomllib.loads(text, parse_float=parse_float)
    state.active()
    # Retain the conservative reservation with the resolver's held metadata.
    # No post-parse size measurement is misrepresented as allocation admission.
    return result
