"""Original receiving workspace for native JSON results, independent of cgroup RSS.

Only JSON mechanics live here. The selecting SDK owner subtracts all its live
startup/selection/transport/controller objects before constructing this ledger.
Output allocations remain charged across calls; returning a public dict does
not let the next response reuse its allowance while the caller can retain it.
"""
import json
import math
import struct
import sys
import time
import types
from dataclasses import dataclass

from .native_core_snapshot import NativeCoreJsonLimits

_ESCAPES = {'"': '"', '\\': '\\', '/': '/', 'b': '\b', 'f': '\f',
            'n': '\n', 'r': '\r', 't': '\t'}


@dataclass(frozen=True, slots=True)
class ReceiverGeometry:
    abi: str
    pointer: int
    gc_header: int
    float_base: int
    range_base: int
    range_iterator_base: int
    list_base: int
    dict_base: int
    keys_header: int
    unicode_base: int
    bytes_base: int
    tuple_base: int
    int_base: int
    int_digit: int
    int_digit_bits: int

    @classmethod
    def current(cls):
        # CPython compact Unicode-key dictionaries: two pointer entry, minimum
        # table8 / five usable entries. Other interpreters need their owner law.
        if (sys.implementation.name != 'cpython'
                or sys.version_info[:2] not in ((3, 11), (3, 12), (3, 13), (3, 14))
                or struct.calcsize('P') != 8
                or (hasattr(sys, '_is_gil_enabled') and not sys._is_gil_enabled())
                or sys.float_info.radix != 2 or sys.float_info.mant_dig != 53
                or sys.float_info.min_exp != -1021):
            raise ValueError('native receiver allocator ABI has not been admitted')
        pointer = struct.calcsize('P')
        header = sys.getsizeof({'x': None}) - sys.getsizeof({}) - 8 - 5 * 2 * pointer
        if (header != struct.calcsize('@nBBBxInn')
                or sys.getsizeof([]) - [].__sizeof__() != struct.calcsize('@PP')
                or sys.int_info.sizeof_digit != 4 or sys.int_info.bits_per_digit != 30):
            raise ValueError('native receiver runtime geometry differs from CPython owner')
        return cls(sys.implementation.cache_tag, pointer, sys.getsizeof([]) - [].__sizeof__(),
                   float.__basicsize__, range.__basicsize__,
                   type(iter(range(0))).__basicsize__, sys.getsizeof([]),
                   sys.getsizeof({}), header, sys.getsizeof('\U0010ffff') - 8,
                   sys.getsizeof(b''), sys.getsizeof(()),
                   sys.getsizeof(0) - sys.int_info.sizeof_digit,
                   sys.int_info.sizeof_digit, sys.int_info.bits_per_digit)

    def list_bytes(self, count):
        # CPython list_resize append law. This bound includes every final slot;
        # realloc peak is reserved separately before each growth.
        slots = 0 if count == 0 else (count + (count >> 3) + 6) & ~3
        return self.list_base + slots * self.pointer

    def dict_bytes(self, count):
        if count == 0:
            return self.dict_base
        table = 8
        while table * 2 // 3 < count:
            table *= 2
        index = 1 if table <= 128 else 2 if table <= 32768 else 4 if table <= (1 << 31) else 8
        # Generic owner maps (FD/Path and timezone-cache tuple keys) use
        # PyDictKeyEntry(hash,key,value), three pointers. Unicode-only JSON
        # maps use two, so this actual largest supported entry geometry is a
        # conservative owner upper bound for both, not an input multiplier.
        return self.dict_base + self.keys_header + table * index + (table * 2 // 3) * 3 * self.pointer

    def unicode_bytes(self, characters):
        # PEP393 maximum4-byte character kind plus its trailing code point.
        return self.unicode_base + 4 * (characters + 1)


class ReceiverState:
    __slots__ = ('geometry', '_limit', '_retained', '_deadline', '_cancelled', '_json', '_visits', '_outputs', '_bootstrap_reserved', '_framework_reserved', '_external_owners', '_opaque_workspace')

    def __new__(cls, *, original_state_bytes, caller_retained_state_bytes,
                deadline, cancelled, json_limits):
        if (type(original_state_bytes) is not int or not 0 < original_state_bytes <= 536870912
                or type(caller_retained_state_bytes) is not int or caller_retained_state_bytes < 0):
            raise ValueError('native receiver original setup allowance refused')
        if cancelled.is_set() or time.monotonic() >= deadline:
            raise TimeoutError('native receiver original bootstrap cutoff')
        # CPython normal-GIL 64-bit owner layouts, subsequently measured and
        # compared by current(): GC two pointers, compact-key header n/BBB/I/nn,
        # minimum table8, five Unicode entries with two pointers each. These
        # source-derived probe geometries are not input-byte multipliers.
        pointer = struct.calcsize('P')
        gc = struct.calcsize('@PP')
        key_header = struct.calcsize('@nBBBxInn')
        controller = cls.__basicsize__ + gc + 3 * (int.__basicsize__ + 3 * sys.int_info.sizeof_digit)
        geometry = ReceiverGeometry.__basicsize__ + gc
        probes = (dict.__basicsize__ + gc + key_header + 8 + 5 * 2 * pointer
                  + dict.__basicsize__ + gc + 2 * (list.__basicsize__ + gc)
                  + range.__basicsize__ + struct.calcsize('@PPnnnn'))
        # Probe arithmetic and generated dataclass init have only fixed-size
        # scalar metadata. Attribute values themselves are counted below;
        # borrowed type/code/name objects have no new receiving allocation.
        scalar = int.__basicsize__ + 2 * sys.int_info.sizeof_digit
        init_code = ReceiverGeometry.__init__.__code__
        current_code = ReceiverGeometry.current.__func__.__code__
        frames = 0
        for code in (init_code, current_code, cls.__init__.__code__):
            frames += (types.FrameType.__basicsize__ + gc
                       + (code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize) * (pointer + scalar))
        geometry_values = len(ReceiverGeometry.__slots__) * scalar
        # Fixed decoder/ledger/geometry methods remain available across every call.
        # Their original code-owned frame slots are distinct from recursive
        # parser frames and raw/result buffers. Pre-admit before raw decoding.
        framework = 0
        fixed_scalar = max(int.__basicsize__ + 3 * sys.int_info.sizeof_digit,
                           str.__basicsize__ + 8,
                           tuple.__basicsize__ + gc + 5 * pointer)
        for fn in (cls.decode, cls.active, cls.reserve, cls.release, cls.reserve_opaque_workspace, cls.settle_opaque_workspace, cls.visit,
                   ReceiverGeometry.list_bytes, ReceiverGeometry.dict_bytes,
                   ReceiverGeometry.unicode_bytes):
            code = fn.__code__
            local_slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            framework += types.FrameType.__basicsize__ + gc + local_slots * (pointer + fixed_scalar)
        peak = controller + geometry + 2 * (list.__basicsize__ + gc) + probes + frames + geometry_values + framework
        if peak > original_state_bytes - caller_retained_state_bytes:
            raise MemoryError('native receiver original bootstrap state exhausted')
        owner = object.__new__(cls)
        owner._bootstrap_reserved = peak
        owner._framework_reserved = framework
        return owner

    def __init__(self, *, original_state_bytes, caller_retained_state_bytes,
                 deadline, cancelled, json_limits):
        if (type(original_state_bytes) is not int or not 0 < original_state_bytes <= 536870912
                or type(caller_retained_state_bytes) is not int or caller_retained_state_bytes < 0
                or caller_retained_state_bytes >= original_state_bytes):
            raise ValueError('native receiver original additive state allowance refused')
        if not isinstance(json_limits, NativeCoreJsonLimits):
            raise TypeError('native receiver requires original typed JSON owner limits')
        for name in ('max_bytes', 'max_depth', 'max_visits', 'max_integer_digits'):
            value = getattr(json_limits, name)
            if type(value) is not int or not 0 < value < 1 << 64:
                raise ValueError('native receiver typed JSON limit differs')
        self.geometry = ReceiverGeometry.current()
        self._limit = original_state_bytes
        self._retained = caller_retained_state_bytes + self._bootstrap_reserved
        self._deadline, self._cancelled, self._json = deadline, cancelled, json_limits
        self._visits = 0
        self._outputs = []
        self._external_owners = []
        self._opaque_workspace = None
        # Owner-produced receiver controller/geometry and initial registry are
        # charged once in addition to the caller's distinct live allocations.
        retained = (self._framework_reserved + sys.getsizeof(self) + sys.getsizeof(self.geometry)
                    + sys.getsizeof(self._outputs) + sys.getsizeof(self._external_owners)
                    + 3 * (int.__basicsize__ + 3 * sys.int_info.sizeof_digit))
        for name in ReceiverGeometry.__slots__:
            value = getattr(self.geometry, name)
            # ABI string is a borrowed sys.implementation owner. Every numeric
            # geometry field is its actual retained scalar allocation.
            if type(value) is int:
                retained += sys.getsizeof(value)
        if retained > self._bootstrap_reserved:
            raise MemoryError('native receiver measured bootstrap geometry exceeds admission')
        self.release(self._bootstrap_reserved - retained)
        self._bootstrap_reserved = 0

    def active(self):
        if self._cancelled.is_set():
            raise InterruptedError('native receiver original owner cancelled')
        if time.monotonic() >= self._deadline:
            raise TimeoutError('native receiver original work cutoff expired')

    def reserve(self, amount):
        self.active()
        if type(amount) is not int or amount < 0 or amount > self._limit - self._retained:
            raise MemoryError('native receiver original simultaneous state exhausted')
        self._retained += amount

    def release(self, amount):
        if type(amount) is not int or not 0 <= amount <= self._retained:
            raise ValueError('native receiver invalid reservation release')
        self._retained -= amount

    def reserve_opaque_workspace(self):
        """Hold the entire original remainder for controlled compile/import.

        The dedicated entry's original aggregate AS ceiling separately bounds
        the opaque operation. This ledger does not claim PyMem allocation hooks.
        Failure preserves this debit and all traceback-retained owners.
        """
        self.active()
        if self._opaque_workspace is not None:
            raise ValueError('native receiver opaque workspace already held')
        amount = self._limit - self._retained
        self.reserve(amount)
        self._opaque_workspace = amount
        return amount

    def settle_opaque_workspace(self, retained_state_bytes):
        """Atomically replace our held peak with its conservative live owner.

        Only a successful controlled operation may settle. Its entry supplies
        a mapped-state upper bound; preexisting caller baseline is untouched.
        No work/visit/clock refund occurs. Failure keeps the full reservation.
        """
        self.active()
        amount = self._opaque_workspace
        if (type(amount) is not int or type(retained_state_bytes) is not int
                or not 0 <= retained_state_bytes <= amount):
            raise ValueError('native receiver opaque retained settlement refused')
        self._retained = self._retained - amount + retained_state_bytes
        self._opaque_workspace = None

    def visit(self):
        self.active()
        if self._visits >= self._json.max_visits:
            raise ValueError('native receiver original cumulative JSON work exhausted')
        self._visits += 1

    def decode(self, frame):
        if not isinstance(frame, memoryview) or len(frame) > self._json.max_bytes:
            raise ValueError('native receiver bounded borrowed JSON frame required')
        # The raw transport buffers are already part of the caller's live census.
        # Bytes copy and UTF8 decoder old/widened Unicode buffers coexist at
        # the decoding peak. Both Unicode capacities are bounded by input
        # bytes; the old buffer is released only after widening has copied it.
        scratch = (self.geometry.bytes_base + len(frame)
                   + self.geometry.unicode_bytes(len(frame))
                   + self.geometry.unicode_bytes(len(frame)))
        self.reserve(scratch)
        try:
            raw = frame.tobytes()
            text = raw.decode('utf-8', 'strict')
            # Interpreter frames are a distinct logical receiving workspace.
            # Geometry comes from the actual maintained Python code objects,
            # not a guessed multiplier on input bytes or cgroup residency.
            # Code objects are borrowed module owners. Every active local/
            # operand slot also forecasts its bounded scalar index temporary;
            # large coefficient/division owners are separately numeric scratch.
            scalar_digits = max(1, ((self._json.max_bytes + 400).bit_length()
                                      + self.geometry.int_digit_bits - 1)
                                     // self.geometry.int_digit_bits)
            scalar_bytes = max(self.geometry.int_base + scalar_digits * self.geometry.int_digit,
                               self.geometry.range_base, self.geometry.range_iterator_base,
                               self.geometry.unicode_bytes(1), self.geometry.float_base,
                               self.geometry.tuple_base + 5 * self.geometry.pointer)
            frame_bytes = 0
            closure_bytes = 0
            for fn in (_ReceivingParser.__init__, _ReceivingParser.space,
                       _ReceivingParser.parse, _ReceivingParser.value,
                       _ReceivingParser.container, _ReceivingParser.string,
                       _ReceivingParser.number):
                code = fn.__code__
                slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
                frame_bytes += (types.FrameType.__basicsize__ + self.geometry.gc_header
                                + slots * (self.geometry.pointer + scalar_bytes))
                for nested in code.co_consts:
                    if isinstance(nested, types.CodeType):
                        cells = len(nested.co_freevars)
                        closure_bytes += (types.FunctionType.__basicsize__ + self.geometry.gc_header
                                          + self.geometry.tuple_base + cells * self.geometry.pointer
                                          + cells * (types.CellType.__basicsize__ + self.geometry.gc_header))
                        frame_bytes += (types.FrameType.__basicsize__ + self.geometry.gc_header
                                        + (nested.co_nlocals + len(nested.co_cellvars) + len(nested.co_freevars) + nested.co_stacksize)
                                        * (self.geometry.pointer + scalar_bytes))
            parser_state = (_ReceivingParser.__basicsize__ + self.geometry.gc_header
                            + (self._json.max_depth + 1) * (frame_bytes + closure_bytes))
            self.reserve(parser_state)
            parser = _ReceivingParser(self, text)
            result = parser.parse()
            self.active()
            # Results deliberately remain retained: no new per-frame grant.
            old_registry = sys.getsizeof(self._outputs)
            registry = self.geometry.list_bytes(len(self._outputs) + 1)
            self.reserve(registry)
            self._outputs.append(result)
            actual_registry = sys.getsizeof(self._outputs)
            if actual_registry > registry:
                raise MemoryError('native receiver output registry geometry changed')
            self.release(old_registry + registry - actual_registry)
            self.release(parser_state)
            return result
        except BaseException:
            # A failed operation is terminal. Do not recycle partial allocations
            # into another call while traceback/parser state can still retain them.
            raise
        finally:
            # Input scratch is no longer retained by any successful result.
            # On failure its traceback may retain it, so the caller terminates.
            if sys.exception() is None:
                self.release(scratch)


class _ReceivingParser:
    __slots__ = ('state', 'text', 'pos')
    def __init__(self, state, text):
        self.state, self.text, self.pos = state, text, 0

    def space(self):
        while self.pos < len(self.text) and self.text[self.pos] in ' \r\n\t':
            self.state.visit()
            self.pos += 1

    def parse(self):
        result = self.value(0)
        self.space()
        if self.pos != len(self.text):
            raise ValueError('native receiver trailing JSON data')
        return result

    def string(self):
        begin = self.pos
        end = begin + 1
        escaped = False
        while end < len(self.text):
            self.state.visit()
            c = self.text[end]
            if c == '"' and not escaped:
                break
            escaped = not escaped if c == '\\' else False
            end += 1
        if end == len(self.text):
            raise ValueError('native receiver unterminated JSON string')
        n = end - begin
        g = self.state.geometry
        # Each decoded code point has a bounded standalone Unicode owner.
        # Two slot arrays cover append realloc overlap; join coexists with
        # the chunks. No regex or opaque scanstring loop escapes active work.
        scratch = (2 * g.list_bytes(n) + n * g.unicode_bytes(1)
                   + g.unicode_bytes(n) + 4 * sys.getsizeof(0x10ffff))
        self.state.reserve(scratch)
        chunks = []
        at = begin + 1
        def hex4(offset):
            result = 0
            for index in range(offset, offset + 4):
                self.state.visit()
                if index >= end:
                    raise ValueError('native receiver incomplete Unicode escape')
                digit = self.text[index]
                position = '0123456789abcdef'.find(digit.lower())
                if position < 0:
                    raise ValueError('native receiver invalid Unicode escape')
                result = result * 16 + position
            return result
        while at < end:
            self.state.visit()
            c = self.text[at]
            at += 1
            if c == '\\':
                if at >= end:
                    raise ValueError('native receiver incomplete string escape')
                c = self.text[at]
                at += 1
                if c == 'u':
                    code = hex4(at)
                    at += 4
                    if (0xd800 <= code <= 0xdbff and at + 6 <= end
                            and self.text[at] == '\\' and self.text[at + 1] == 'u'):
                        second = hex4(at + 2)
                        if 0xdc00 <= second <= 0xdfff:
                            code = 0x10000 + ((code - 0xd800) << 10) + second - 0xdc00
                            at += 6
                    c = chr(code)
                else:
                    c = _ESCAPES.get(c)
                    if c is None:
                        raise ValueError('native receiver invalid string escape')
            elif ord(c) < 32:
                raise ValueError('native receiver invalid string control character')
            chunks.append(c)
        value = ''.join(chunks)
        self.pos = end + 1
        charge = sys.getsizeof(value)
        if charge > scratch:
            raise MemoryError('native receiver string geometry exceeded admission')
        self.state.release(scratch - charge)
        return value

    def value(self, depth):
        self.state.visit()
        if depth > self.state._json.max_depth:
            raise ValueError('native receiver original JSON depth exhausted')
        self.space()
        if self.pos >= len(self.text):
            raise ValueError('native receiver incomplete JSON')
        c = self.text[self.pos]
        if c == '"':
            return self.string()
        if c in '[{':
            return self.container(depth, c)
        for token, value in (('null', None), ('true', True), ('false', False)):
            if self.text.startswith(token, self.pos):
                self.pos += len(token)
                return value
        return self.number()

    def number(self):
        # JSON lexical mechanics avoid an uninterruptible regex over a large
        # numeric token. Every consumed digit spends the original work ledger.
        start = self.pos
        end = start
        negative = self.text[end] == '-'
        if negative:
            end += 1
        def digits(at):
            first = at
            while at < len(self.text) and '0' <= self.text[at] <= '9':
                self.state.visit()
                at += 1
            return at, at - first
        if end >= len(self.text) or not '0' <= self.text[end] <= '9':
            raise ValueError('native receiver invalid JSON number')
        if self.text[end] == '0':
            self.state.visit()
            end += 1
            integer_digits = 1
        else:
            end, integer_digits = digits(end)
        integer_end = end
        fraction_start = fraction_end = end
        floating = False
        if end < len(self.text) and self.text[end] == '.':
            floating = True
            fraction_start = end + 1
            end, count = digits(fraction_start)
            if not count:
                raise ValueError('native receiver fractional digit required')
            fraction_end = end
        exponent_start = exponent_end = end
        exponent_negative = False
        if end < len(self.text) and self.text[end] in 'eE':
            floating = True
            end += 1
            if end < len(self.text) and self.text[end] in '+-':
                exponent_negative = self.text[end] == '-'
                end += 1
            exponent_start = end
            end, count = digits(end)
            if not count:
                raise ValueError('native receiver exponent digit required')
            exponent_end = end
        n = end - start
        if not floating and integer_digits > self.state._json.max_integer_digits:
            raise ValueError('native receiver original integer digit cap')
        g = self.state.geometry
        def integer_size(decimal_digits):
            # 10 < 2**4: owner digit slots, including a zero representation.
            slots = max(1, (4 * decimal_digits + g.int_digit_bits - 1) // g.int_digit_bits)
            return g.int_base + slots * g.int_digit
        coefficient_bound = integer_size(n)
        if not floating:
            scratch = coefficient_bound * 3  # old / multiply / add expressions
        else:
            numerator_slots = max(1, (4 * (n + 308) + g.int_digit_bits - 1) // g.int_digit_bits)
            denominator_slots = max(1, (4 * (n + 324) + g.int_digit_bits - 1) // g.int_digit_bits)
            # CPython long_true_divide x: minimum binary shift -1076 for
            # binary64, then x_divrem owns v(x+1), w(b), quotient(x+1).
            x_slots = numerator_slots + (1076 + g.int_digit_bits - 1) // g.int_digit_bits + 1
            long_size = lambda slots: g.int_base + slots * g.int_digit
            scratch = (3 * coefficient_bound + 3 * long_size(numerator_slots)
                       + 3 * long_size(denominator_slots) + long_size(x_slots)
                       + long_size(x_slots + 1) + long_size(denominator_slots)
                       + long_size(x_slots + 1) + 2 * g.float_base
                       + OverflowError.__basicsize__ + g.gc_header + g.tuple_base + g.pointer
                       + g.unicode_bytes(44) + types.TracebackType.__basicsize__ + g.gc_header
                       + types.FrameType.__basicsize__ + g.gc_header
                       + (type(self).number.__code__.co_nlocals
                          + len(type(self).number.__code__.co_cellvars)
                          + len(type(self).number.__code__.co_freevars)
                          + type(self).number.__code__.co_stacksize) * g.pointer)
        self.state.reserve(scratch)
        value = 0
        first_nonzero = None
        coefficient_digits = 0
        for left, right in ((start + int(negative), integer_end),
                            (fraction_start, fraction_end)):
            for at in range(left, right):
                self.state.visit()
                digit = ord(self.text[at]) - 48
                if digit and first_nonzero is None:
                    first_nonzero = coefficient_digits
                coefficient_digits += 1
                value = value * 10 + digit
        if floating:
            exponent = 0
            for at in range(exponent_start, exponent_end):
                self.state.visit()
                # Larger exponents have exactly the same overflow/underflow
                # classification; saturation avoids an unnecessary huge int.
                exponent = min(n + 400, exponent * 10 + ord(self.text[at]) - 48)
            scale = (-exponent if exponent_negative else exponent) - (fraction_end - fraction_start)
            order = (coefficient_digits - (first_nonzero or 0)) + scale
            if value == 0 or order < -324:
                value = 0.0
            elif order > 309:
                value = math.inf
            else:
                numerator, denominator = value, 1
                for _ in range(max(0, scale)):
                    self.state.visit()
                    numerator *= 10
                for _ in range(max(0, -scale)):
                    self.state.visit()
                    denominator *= 10
                self.state.active()
                try:
                    # Correctly rounded binary64 conversion of the exact
                    # decimal rational; no dtoa ASCII/Bigint cache owner.
                    value = numerator / denominator
                except OverflowError:
                    value = math.inf
                self.state.active()
        if negative:
            value = -value
        actual = sys.getsizeof(value)
        if actual > scratch:
            raise MemoryError('native receiver numeric geometry exceeded admission')
        self.state.release(scratch - actual)
        self.pos = end
        return value

    def container(self, depth, opening):
        g = self.state.geometry
        is_object = opening == '{'
        base = g.dict_base if is_object else g.list_base
        # Empty container has no payload allocation; subsequent growth always
        # reserves old+new representation before the real owner mutates it.
        self.state.reserve(base)
        result = {} if is_object else []
        self.pos += 1
        closing = '}' if is_object else ']'
        self.space()
        if self.pos < len(self.text) and self.text[self.pos] == closing:
            self.pos += 1
            return result
        count = 0
        while True:
            self.space()
            key = None
            if is_object:
                if self.pos >= len(self.text) or self.text[self.pos] != '"':
                    raise ValueError('native receiver object key required')
                self.state.visit()
                key = self.string()
                self.space()
                if self.pos >= len(self.text) or self.text[self.pos] != ':':
                    raise ValueError('native receiver object colon required')
                self.pos += 1
                if key in result:
                    raise ValueError('native receiver duplicate JSON member')
            value = self.value(depth + 1)
            count += 1
            forecast = g.dict_bytes(count) if is_object else g.list_bytes(count)
            old = sys.getsizeof(result)
            self.state.reserve(forecast)
            if is_object:
                result[key] = value
            else:
                result.append(value)
            actual = sys.getsizeof(result)
            if actual > forecast:
                raise MemoryError('native receiver container geometry exceeded admission')
            self.state.release(old + forecast - actual)
            self.space()
            if self.pos >= len(self.text):
                raise ValueError('native receiver incomplete container')
            c = self.text[self.pos]
            self.pos += 1
            if c == closing:
                return result
            if c != ',':
                raise ValueError('native receiver container separator required')
