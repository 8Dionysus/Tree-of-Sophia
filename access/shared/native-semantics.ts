/** Shared native-v7 semantics used by the bounded Worker D1 lens/focus route.
 * Ordinary JS values remain unchanged. Opaque per-document sidecars preserve
 * number lexemes/kinds and source object order, which JSON.parse alone loses.
 */
import {nativeUnicodeVersion} from './native-unicode.ts';
export {codePointCompare, nativeUnicodeVersion, nativeUnicodeAlgorithm} from './native-unicode.ts';

export const nativeSemanticVersion = 'tos-python-native-semantics-v1';
export class NativeContextLost extends Error {}
export class NativeBudgetExceeded extends Error {}
type NumberInfo = {kind: 'int' | 'float'; lexeme: string; parsed: number};
type ContainerInfo = Readonly<{keys: readonly string[]; keySet: Readonly<{has: (key: string) => boolean}>;
  numbers: Readonly<{get: (key: string) => NumberInfo | undefined}>}>;
type Context = Readonly<{lookup: (value: object) => ContainerInfo | undefined}>;
export type NativeRef = Readonly<{value: unknown; context: Context | null; number?: NumberInfo}>;
export type NativeJsonLimits = Readonly<{maxBytes: number; maxDepth: number; maxMembers: number; maxIntegerDigits: number}>;
export type NativePythonRuntime = Readonly<{
  worker_python_value_wasm_v1: (request: Uint8Array) => Uint8Array;
  worker_python_lower_utf16_wasm_v1: (units: Uint16Array, maxUtf16Units: number) => Uint16Array;
}>;
const defaultLimits: NativeJsonLimits = {maxBytes: 1_048_576, maxDepth: 64, maxMembers: 300_000, maxIntegerDigits: 4300};
const referenceBrand = Symbol('native reference');
const semanticsEncoder = new TextEncoder();
const semanticsDecoder = new TextDecoder();
let semanticsRuntime: NativePythonRuntime | null = null;

export function installNativePythonRuntime(runtime: NativePythonRuntime): void {
  if (typeof runtime.worker_python_value_wasm_v1 !== 'function'
    || typeof runtime.worker_python_lower_utf16_wasm_v1 !== 'function') {
    throw new TypeError('native Python WASM runtime is incomplete');
  }
  semanticsRuntime = runtime;
}

type NativePythonResponse = Readonly<{schema_version: string; ok: boolean; value?: unknown; error?: string; message?: string}>;
function runPythonValue(operation: string, leftJson: string, options: Readonly<{
  rightJson?: string; maxUtf16Units?: number; maxVisits?: number; maxValues?: number;
}> = {}): unknown {
  const runtime = semanticsRuntime;
  if (!runtime) throw new NativeContextLost('native Python WASM runtime is unavailable');
  const request = {
    schema_version: 'tos_worker_python_value_request_v1', operation, left_json: leftJson,
    ...(options.rightJson === undefined ? {} : {right_json: options.rightJson}),
    ...(options.maxUtf16Units === undefined ? {} : {max_utf16_units: options.maxUtf16Units}),
    ...(options.maxVisits === undefined ? {} : {max_visits: options.maxVisits}),
    ...(options.maxValues === undefined ? {} : {max_values: options.maxValues}),
  };
  let response: NativePythonResponse;
  try {
    const raw = runtime.worker_python_value_wasm_v1(semanticsEncoder.encode(JSON.stringify(request)));
    response = JSON.parse(semanticsDecoder.decode(raw)) as NativePythonResponse;
  } catch (error) {
    if (error instanceof NativeBudgetExceeded || error instanceof NativeContextLost || error instanceof TypeError) throw error;
    throw new NativeContextLost('native Python WASM response is invalid');
  }
  if (response.schema_version !== 'tos_worker_python_value_response_v1' || typeof response.ok !== 'boolean') {
    throw new NativeContextLost('native Python WASM response schema is invalid');
  }
  if (!response.ok) {
    if (response.error === 'budget_exceeded') throw new NativeBudgetExceeded('native Python value budget');
    if (response.error === 'invalid_unicode_scalar' && operation === 'searchable') throw new TypeError('search source contains an invalid Unicode string');
    if (operation === 'member' && response.message) throw new TypeError(response.message);
    throw new TypeError(`native Python value operation failed: ${response.error ?? 'unknown error'}`);
  }
  return response.value;
}

function rawJson(ref: NativeRef, maxChars = 1_048_576): string {
  requireReference(ref);
  return nativeJson(ref, maxChars);
}

function semanticString(operation: 'str' | 'repr' | 'sort_key', ref: NativeRef, maxChars: number): string {
  requireReference(ref);
  const result = runPythonValue(operation, rawJson(ref, maxChars), {maxUtf16Units: positive(maxChars)});
  if (typeof result !== 'string') throw new NativeContextLost('native Python string result is not text');
  return result;
}

/** Pinned Unicode lowercase delegated to the shared foundation implementation. */
export function nativeLower(value: string, expectedVersion: string = nativeUnicodeVersion): string {
  if (expectedVersion !== nativeUnicodeVersion) throw new Error('native Unicode version is incompatible');
  const runtime = semanticsRuntime;
  if (!runtime) throw new NativeContextLost('native Python WASM runtime is unavailable');
  const units = new Uint16Array(value.length);
  for (let index = 0; index < value.length; index++) units[index] = value.charCodeAt(index);
  let lowered: Uint16Array;
  try {
    lowered = runtime.worker_python_lower_utf16_wasm_v1(units, Math.max(1, value.length));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (/budget|work limit/i.test(message)) throw new NativeBudgetExceeded('native lowercase budget');
    throw new NativeContextLost('native lowercase WASM operation failed');
  }
  if (!(lowered instanceof Uint16Array) || lowered.length > Math.max(1, value.length) * 2) {
    throw new NativeContextLost('native lowercase WASM result is invalid');
  }
  let result = '';
  const chunkSize = 0x8000;
  for (let index = 0; index < lowered.length; index += chunkSize) {
    result += String.fromCharCode(...lowered.subarray(index, index + chunkSize));
  }
  return result;
}

function reference(value: unknown, context: Context | null, number?: NumberInfo): NativeRef {
  // Metadata is neither written into source values nor included in spreads /
  // ordinary JSON serialization of this semantic reference.
  const ref = {value} as NativeRef;
  Object.defineProperty(ref, referenceBrand, {value: ref});
  Object.defineProperty(ref, 'context', {value: context});
  if (number) Object.defineProperty(ref, 'number', {value: number});
  return Object.freeze(ref);
}

export function isNativeRef(value: unknown): value is NativeRef {
  return !!value && typeof value === 'object' && (value as Record<symbol, unknown>)[referenceBrand] === value;
}

function requireReference(ref: NativeRef): void {
  if (!isNativeRef(ref)) throw new NativeContextLost('native reference context was lost');
}

function positive(value: number): number {
  if (!Number.isSafeInteger(value) || value < 1) throw new TypeError('native budget must be a positive integer');
  return value;
}

/** Source rows AND raw request specifications must enter here before parsing
 * elsewhere. The validating JS parser is followed by a bounded lexical walk;
 * there is no second graph, value normalization or generated per-scalar index.
 * The newly parsed tree is frozen, enforcing the source-reference boundary.
 */
export function parseNativeJson(raw: string, overrides: Partial<NativeJsonLimits> = {}): NativeRef {
  const limits = {...defaultLimits, ...overrides};
  Object.values(limits).forEach(positive);
  if (new TextEncoder().encode(raw).length > limits.maxBytes) throw new NativeBudgetExceeded('native JSON byte budget');
  const value: unknown = JSON.parse(raw);
  const containers = new WeakMap<object, ContainerInfo>();
  const context: Context = Object.freeze({lookup: (value: object) => containers.get(value)});
  let at = 0, members = 0;
  const whitespace = () => {while (/[\x20\r\n\t]/.test(raw[at] ?? '\0')) at++;};
  function stringToken(): string {
    const start = at++;
    while (raw[at] !== '"') {if (raw[at] === '\\') at++; at++;}
    at++;
    return JSON.parse(raw.slice(start, at)) as string;
  }
  function walk(current: unknown, depth: number): NumberInfo | undefined {
    if (++members > limits.maxMembers || depth > limits.maxDepth) throw new NativeBudgetExceeded('native JSON structural budget');
    whitespace();
    if (raw[at] === '"') {stringToken(); return;}
    if (raw[at] === '{' || raw[at] === '[') {
      const array = raw[at++] === '[';
      const close = array ? ']' : '}';
      const keys: string[] = [], seen = new Set<string>(), numbers = new Map<string, NumberInfo>();
      whitespace();
      while (raw[at] !== close) {
        const key = array ? String(keys.length) : stringToken();
        if (seen.has(key)) throw new TypeError('native JSON duplicate member');
        seen.add(key); keys.push(key);
        if (!array) {whitespace(); at++;}
        const numeric = walk((current as Record<string, unknown>)[key], depth + 1);
        if (numeric) numbers.set(key, numeric);
        whitespace();
        if (raw[at] === ',') {at++; whitespace();} else break;
      }
      at++;
      containers.set(current as object, Object.freeze({keys: Object.freeze(keys),
        keySet: Object.freeze({has: (key: string) => seen.has(key)}),
        numbers: Object.freeze({get: (key: string) => numbers.get(key)})}));
      Object.freeze(current);
      return;
    }
    if (raw[at] === 't') {at += 4; return;}
    if (raw[at] === 'f') {at += 5; return;}
    if (raw[at] === 'n') {at += 4; return;}
    const start = at;
    while (/[\d.eE+\-]/.test(raw[at] ?? '\0')) at++;
    const lexeme = raw.slice(start, at), kind = /[.eE]/.test(lexeme) ? 'float' : 'int';
    if (kind === 'float' && !Number.isFinite(current)) throw new TypeError('native JSON nonfinite float');
    if (kind === 'int' && lexeme.replace('-', '').length > limits.maxIntegerDigits) throw new NativeBudgetExceeded('native integer digit budget');
    return Object.freeze({kind, lexeme, parsed: current as number});
  }
  const number = walk(value, 0);
  return reference(value, context, number);
}

/** Lossless literal references need no row context. Numbers require raw JSON. */
export function nativeScalar(value: string | boolean | null): NativeRef {
  if (value !== null && typeof value !== 'string' && typeof value !== 'boolean') throw new NativeContextLost('numeric/container reference needs native JSON context');
  return reference(value, null);
}

function container(ref: NativeRef): ContainerInfo {
  requireReference(ref);
  const item = ref.value;
  const info = item && typeof item === 'object' ? ref.context?.lookup(item) : undefined;
  if (!info) throw new NativeContextLost('native container context was lost');
  return info;
}

export function nativeChild(ref: NativeRef, key: string | number): NativeRef {
  const info = container(ref), name = String(key);
  if (!info.keySet.has(name)) return nativeScalar(null);
  const value = (ref.value as Record<string, unknown>)[name];
  const number = info.numbers.get(name);
  if (number && !Object.is(value, number.parsed)) throw new NativeContextLost('native number changed after decoding');
  return reference(value, ref.context, number);
}

export function nativeField(ref: NativeRef, path: string): NativeRef {
  requireReference(ref);
  let current = ref;
  for (const key of path.split('.')) {
    if (!current.value || typeof current.value !== 'object' || Array.isArray(current.value)) return nativeScalar(null);
    current = nativeChild(current, key);
  }
  return current;
}

export function nativeKeys(ref: NativeRef): readonly string[] { return container(ref).keys; }

/** Request/cursor JSON uses Python json.loads last-member-wins, retaining the
 * first insertion position. Source rows continue to use strict duplicate mode.
 * Parse each lexical subtree independently before installing its final value;
 * a duplicate changing scalar/container kind must not borrow the later context.
 */
export function parseNativeRequest(raw: string, overrides: Partial<NativeJsonLimits> = {}): NativeRef {
  const limits = {...defaultLimits, ...overrides};
  Object.values(limits).forEach(positive);
  if (new TextEncoder().encode(raw).length > limits.maxBytes) throw new NativeBudgetExceeded('native JSON byte budget');
  JSON.parse(raw); // Exact JSON grammar, including trailing-input rejection.
  let at = 0, visits = 0;
  const spaces = () => {while (/[\x20\r\n\t]/.test(raw[at] ?? '\0')) at++;};
  const token = () => {
    const start = at++;
    while (raw[at] !== '"') {if (raw[at] === '\\') at++; at++;}
    at++; return raw.slice(start, at);
  };
  function walk(depth: number): NativePacketValue {
    if (++visits > limits.maxMembers || depth > limits.maxDepth) throw new NativeBudgetExceeded('native JSON structural budget');
    spaces();
    if (raw[at] === '{') {
      at++; spaces(); const entries = new Map<string, NativePacketValue>();
      while (raw[at] !== '}') {
        const key = JSON.parse(token()) as string; spaces(); at++;
        entries.set(key, walk(depth + 1)); spaces();
        if (raw[at] !== ',') break;
        at++; spaces();
      }
      at++; return nativePacketObject([...entries]);
    }
    if (raw[at] === '[') {
      at++; spaces(); const items: NativePacketValue[] = [];
      while (raw[at] !== ']') {items.push(walk(depth + 1)); spaces(); if (raw[at] !== ',') break; at++; spaces();}
      at++; return nativePacketArray(items);
    }
    const start = at;
    if (raw[at] === '"') token();
    else while (at < raw.length && !/[\x20\r\n\t,\]}]/.test(raw[at]!)) at++;
    return parseNativeJson(raw.slice(start, at), limits);
  }
  return parseNativeJson(nativePacketJson(walk(0), {maxBytes: limits.maxBytes, maxDepth: limits.maxDepth, maxVisits: limits.maxMembers}), limits);
}

export function nativeNumberInfo(ref: NativeRef): Readonly<{kind: 'int' | 'float'; lexeme: string}> {
  requireReference(ref);
  if (typeof ref.value !== 'number' || !ref.number || !Object.is(ref.value, ref.number.parsed)) throw new NativeContextLost('native numeric kind/lexeme was lost');
  return ref.number;
}

export function pythonTruthy(ref: NativeRef): boolean {
  requireReference(ref);
  const value = runPythonValue('truthy', rawJson(ref));
  if (typeof value !== 'boolean') throw new NativeContextLost('native truthiness result is not boolean');
  return value;
}

export function pythonEquals(left: NativeRef, right: NativeRef, maxVisits = 300_000): boolean {
  const value = runPythonValue('equals', rawJson(left), {rightJson: rawJson(right), maxVisits: positive(maxVisits)});
  if (typeof value !== 'boolean') throw new NativeContextLost('native equality result is not boolean');
  return value;
}

/** Python membership (not the lens operator 'in', which uses set intersection
 * for actual lists and must separately reject unhashable nested values). */
export function pythonMember(needle: NativeRef, haystack: NativeRef, maxVisits = 300_000): boolean {
  const value = runPythonValue('member', rawJson(needle), {rightJson: rawJson(haystack), maxVisits: positive(maxVisits)});
  if (typeof value !== 'boolean') throw new NativeContextLost('native membership result is not boolean');
  return value;
}

function renderJson(ref: NativeRef, maxChars: number): string {
  let remaining = positive(maxChars);
  const emit = (text: string) => {
    remaining -= text.length;
    if (remaining < 0) throw new NativeBudgetExceeded('native string output budget');
    return text;
  };
  function walk(current: NativeRef): string {
    requireReference(current);
    const value = current.value;
    if (value === null) return emit('null');
    if (typeof value === 'boolean') return emit(String(value));
    if (typeof value === 'string') return emit(JSON.stringify(value));
    if (typeof value === 'number') return emit(nativeNumberInfo(current).lexeme);
    const keys = container(current).keys;
    const array = Array.isArray(value);
    const chunks = [emit(array ? '[' : '{')];
    for (const [index, key] of keys.entries()) {
      if (index) chunks.push(emit(','));
      if (!array) chunks.push(emit(JSON.stringify(key) + ':'));
      chunks.push(walk(nativeChild(current, key)));
    }
    chunks.push(emit(array ? ']' : '}'));
    return chunks.join('');
  }
  return walk(ref);
}

/** Source-valued raw JSON keeps exact number lexemes and insertion order. */
export function nativeJson(ref: NativeRef, maxChars = 1_048_576): string {return renderJson(ref, maxChars);}
export function pythonStr(ref: NativeRef, maxChars = 1_048_576): string {return semanticString('str', ref, maxChars);}
export function pythonRepr(ref: NativeRef, maxChars = 1_048_576): string {return semanticString('repr', ref, maxChars);}
export function nativeSortKey(ref: NativeRef, maxChars = 1_048_576): string {return semanticString('sort_key', ref, maxChars);}

export function nativeSearchableText(ref: NativeRef, maxChars = 4 * 1024 * 1024): string {
  requireReference(ref);
  const value = runPythonValue('searchable', rawJson(ref, maxChars), {maxUtf16Units: positive(maxChars)});
  if (typeof value !== 'string') throw new NativeContextLost('native searchable text result is not text');
  return value;
}

export function nativeSearchRankValues(ref: NativeRef, fields: readonly string[], maxValues = 300_000,
  maxUtf16Units = 1_048_576): string[] {
  requireReference(ref);
  if (fields.some(field => typeof field !== 'string')) throw new TypeError('rank fields must be strings');
  const value = runPythonValue('rank_values', rawJson(ref), {rightJson: JSON.stringify(fields),
    maxValues: positive(maxValues), maxUtf16Units: positive(maxUtf16Units)});
  if (!Array.isArray(value) || value.some(item => typeof item !== 'string')) {
    throw new NativeContextLost('native rank values result is invalid');
  }
  return value;
}

const packetBrand = Symbol('native packet');
export type NativePacketValue = NativeRef | NativePacket | string | boolean | null;
export type NativePacket = Readonly<
  {kind: 'array'; items: readonly NativePacketValue[]} |
  {kind: 'object'; entries: readonly (readonly [string, NativePacketValue])[]}
>;
export type NativePacketLimits = Readonly<{maxBytes: number; maxDepth: number; maxVisits: number}>;
const defaultPacketLimits: NativePacketLimits = {maxBytes: 1_048_576, maxDepth: 64, maxVisits: 300_000};

function isPacket(value: unknown): value is NativePacket {
  return !!value && typeof value === 'object' && (value as Record<symbol, unknown>)[packetBrand] === value;
}

function requirePacketValue(value: unknown): asserts value is NativePacketValue {
  if (value === null || typeof value === 'string' || typeof value === 'boolean' || isNativeRef(value) || isPacket(value)) return;
  throw new NativeContextLost('packet values require explicit builders or preserved native references');
}

function packet(value: NativePacket): NativePacket {
  Object.defineProperty(value, packetBrand, {value});
  return Object.freeze(value);
}

/** Snapshot explicit derived members. Raw objects/arrays and JS numbers never
 * implicitly become semantic source values. Cycles cannot be built via this API.
 * Each builder is locally bounded; the writer charges aggregate repeated visits.
 */
export function nativePacketArray(items: readonly NativePacketValue[], maxMembers = 300_000): NativePacket {
  if (!Array.isArray(items)) throw new TypeError('packet array requires an array');
  if (items.length > positive(maxMembers)) throw new NativeBudgetExceeded('packet member budget');
  const copy = Array.from(items, item => {requirePacketValue(item); return item;});
  return packet({kind: 'array', items: Object.freeze(copy)});
}

/** Entries, not a JS object, own order, including numeric-like/reserved names. */
export function nativePacketObject(entries: readonly (readonly [string, NativePacketValue])[], maxMembers = 300_000): NativePacket {
  if (!Array.isArray(entries)) throw new TypeError('packet object requires ordered entries');
  if (entries.length > positive(maxMembers)) throw new NativeBudgetExceeded('packet member budget');
  const seen = new Set<string>();
  const copy = Array.from(entries, entry => {
    if (!Array.isArray(entry) || entry.length !== 2 || typeof entry[0] !== 'string') throw new TypeError('malformed packet member');
    const [key, value] = entry;
    if (seen.has(key)) throw new TypeError('duplicate packet member');
    seen.add(key); requirePacketValue(value);
    return Object.freeze([key, value] as const);
  });
  return packet({kind: 'object', entries: Object.freeze(copy)});
}

/** Explicit kind for newly derived numbers. Source numbers must keep NativeRef. */
export function nativeInteger(value: number | bigint): NativeRef {
  if (typeof value !== 'bigint' && (typeof value !== 'number' || !Number.isSafeInteger(value) || Object.is(value, -0))) {
    throw new TypeError('derived integer requires bigint or a safe non-negative-zero integer');
  }
  return parseNativeJson(String(value));
}

export function nativeFloat(value: number): NativeRef {
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new TypeError('derived float requires a finite number');
  const raw = Object.is(value, -0) ? '-0.0' : String(value);
  const result = runPythonValue('float', raw, {maxUtf16Units: 64});
  if (typeof result !== 'string') throw new NativeContextLost('native float spelling is not text');
  return parseNativeJson(result);
}

/** Serialize a complete newly assembled packet without materializing a lossy
 * intermediate JS object. Every repeated reference is charged again. No partial
 * result escapes on context loss, cycles or aggregate UTF-8/visit/depth overflow.
 */
export function nativePacketJson(value: NativePacketValue, overrides: Partial<NativePacketLimits> = {}): string {
  const limits = {...defaultPacketLimits, ...overrides};
  Object.values(limits).forEach(positive);
  let bytes = 0, visits = 0;
  const encoder = new TextEncoder(), active = new Set<object>(), chunks: string[] = [];
  function emit(text: string): void {
    if (text.length > limits.maxBytes - bytes) throw new NativeBudgetExceeded('packet UTF-8 byte budget');
    bytes += encoder.encode(text).length;
    if (bytes > limits.maxBytes) throw new NativeBudgetExceeded('packet UTF-8 byte budget');
    chunks.push(text);
  }
  function string(value: string): void {
    if (value.length > limits.maxBytes - bytes) throw new NativeBudgetExceeded('packet UTF-8 byte budget');
    emit(JSON.stringify(value));
  }
  function walk(item: NativePacketValue, depth: number): void {
    if (++visits > limits.maxVisits || depth > limits.maxDepth) throw new NativeBudgetExceeded('packet structural budget');
    requirePacketValue(item);
    const ref = isNativeRef(item) ? item : null;
    const current = ref ? ref.value : item;
    if (current === null) {emit('null'); return;}
    if (typeof current === 'string') {string(current); return;}
    if (typeof current === 'boolean') {emit(String(current)); return;}
    if (typeof current === 'number' && ref) {emit(nativeNumberInfo(ref).lexeme); return;}
    const identity = ref ? ref.value as object : item as object;
    if (active.has(identity)) throw new NativeContextLost('cyclic native packet');
    active.add(identity);
    try {
      if (ref) {
        const info = container(ref), array = Array.isArray(current);
        emit(array ? '[' : '{');
        for (const [index, key] of info.keys.entries()) {
          if (index) emit(',');
          if (!array) {string(key); emit(':');}
          walk(nativeChild(ref, key), depth + 1);
        }
        emit(array ? ']' : '}');
      } else if (isPacket(item)) {
        const array = item.kind === 'array';
        emit(array ? '[' : '{');
        if (item.kind === 'array') {
          for (const [index, child] of item.items.entries()) {if (index) emit(','); walk(child, depth + 1);}
        } else {
          for (const [index, [key, child]] of item.entries.entries()) {
            if (index) emit(','); string(key); emit(':'); walk(child, depth + 1);
          }
        }
        emit(array ? ']' : '}');
      } else throw new NativeContextLost('invalid native packet context');
    } finally {active.delete(identity);}
  }
  walk(value, 0);
  return chunks.join('');
}
