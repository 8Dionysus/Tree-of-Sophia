/** Exact native-v7 values for the bounded lens path. No source-bearing JS result. */
import {bindQueryProperties, normalizeLensSpec, type LensSpec, type LensFilter, type QueryProperty, type SortRule} from './knowledge.ts';
import {nativeCasefold} from '../../../shared/native-unicode.ts';
import {NativeBudgetExceeded, nativeChild, nativeField, nativeKeys, nativeNumberInfo, nativeScalar,
  nativeInteger, nativePacketArray, nativePacketObject, nativePacketJson, parseNativeJson, parseNativeRequest,
  isNativeRef, pythonEquals, pythonMember, pythonStr, pythonTruthy, nativeSortKey, nativeLower, codePointCompare,
  type NativeRef, type NativePacketValue, type NativePacket} from '../../../shared/native-semantics.ts';
export {nativeChild, nativeField, nativeKeys, nativePacketArray, nativePacketObject, nativePacketJson, parseNativeJson, parseNativeRequest};
export type {NativeRef, NativePacket, NativePacketValue};

export const NATIVE_LENS_RESPONSE_BYTES = 16 * 1024 * 1024;
export type NativeLensStructuralPreview = Readonly<{node_ids: readonly string[]; relation_ids: readonly string[];
  fingerprint: string; counts: Readonly<Record<string, number | boolean>>}>;
export type NativeLensResult = Readonly<{packet: NativePacket; preview: NativeLensStructuralPreview}>;
export type NativeFilter = LensFilter & {readonly valueRef: NativeRef};
export type NativeGroup = {enabled: boolean; match: 'all' | 'any'; filters: NativeFilter[]};
export type NativeSpec = LensSpec & {node_query: NativeGroup; relation_query: NativeGroup;
  path_query: (Omit<LensSpec['path_query'][number], 'steps'> & {steps: {direction: 'incoming' | 'outgoing' | 'either'; node_query: NativeGroup; relation_query: NativeGroup}[]})[]};

/** Only newly derived schema fields enter this bridge. Source subtrees retain refs. */
export function derived(value: unknown): NativePacketValue {
  if (isNativeRef(value)) return value;
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number') return nativeInteger(value);
  if (Array.isArray(value)) return nativePacketArray(value.map(derived));
  if (value && typeof value === 'object') return nativePacketObject(Object.entries(value).map(([k, v]) => [k, derived(v)]));
  throw new TypeError('derived lens field must be JSON');
}
export function objectWith(ref: NativeRef, replace: ReadonlyMap<string, NativePacketValue>, omit: ReadonlySet<string> = new Set()): NativePacket {
  const entries: [string, NativePacketValue][] = nativeKeys(ref).filter(k => !omit.has(k)).map(k => [k, replace.has(k) ? replace.get(k)! : nativeChild(ref, k)]);
  for (const [key, value] of replace) if (!nativeKeys(ref).includes(key) && !omit.has(key)) entries.push([key, value]);
  return nativePacketObject(entries);
}
export function stringField(ref: NativeRef, path: string): string {
  const value = nativeField(ref, path).value;
  if (typeof value !== 'string') throw new Error('published lens structural field must be string: ' + path);
  return value;
}
export function arrayRefs(ref: NativeRef): NativeRef[] {
  if (!Array.isArray(ref.value)) throw new TypeError('native array required');
  return nativeKeys(ref).map(key => nativeChild(ref, key));
}

export function compileNativeSpec(input: NativeRef, definitions: QueryProperty[]): {spec: NativeSpec; publicPacket: NativePacket} {
  if (!input.value || typeof input.value !== 'object' || Array.isArray(input.value)) throw new Error('lens spec must be an object');
  // The existing validator owns field/operator/schema law. Its temporary values
  // are never used for matching, fingerprinting or delivery. Overflow integers
  // need a finite placeholder solely for its scalar-type validation.
  const validation = structuredClone(input.value) as Record<string, unknown>;
  const groups: {raw: NativeRef; target: Record<string, unknown>}[] = [];
  const addGroup = (raw: NativeRef, target: unknown) => {
    if (!target || typeof target !== 'object' || Array.isArray(target)) return;
    groups.push({raw, target: target as Record<string, unknown>});
  };
  addGroup(nativeField(input, 'node_query'), validation.node_query);
  addGroup(nativeField(input, 'relation_query'), validation.relation_query);
  if (Array.isArray(validation.path_query)) validation.path_query.forEach((path, p) => {
    if (!path || typeof path !== 'object' || !Array.isArray(path.steps)) return;
    path.steps.forEach((step: Record<string, unknown>, s: number) => {
      if (!step || typeof step !== 'object') return;
      for (const kind of ['node', 'relation']) addGroup(nativeField(nativeChild(nativeField(nativeChild(nativeField(input, 'path_query'), p), 'steps'), s), kind + '_query'), step[kind + '_query']);
    });
  });
  for (const {target} of groups) if (Array.isArray(target.filters)) for (const rule of target.filters) {
    if (!rule || typeof rule !== 'object') continue;
    const finite = (v: unknown) => typeof v === 'number' && !Number.isFinite(v) ? 0 : v;
    rule.value = Array.isArray(rule.value) ? rule.value.map(finite) : finite(rule.value);
  }
  const pagination = nativeField(input, 'pagination');
  if (pagination.value && typeof pagination.value === 'object' && !Array.isArray(pagination.value)) {
    for (const key of ['nodes', 'relations']) if (nativeKeys(pagination).includes(key)) {
      const size = nativeChild(pagination, key);
      if (typeof size.value !== 'number' || nativeNumberInfo(size).kind !== 'int') throw new Error('pagination.' + key + ' must be an integer');
    }
  }
  const publicSpec = normalizeLensSpec(validation);
  const bound = bindQueryProperties(definitions, publicSpec) as NativeSpec;
  function groupPacket(publicGroup: LensSpec['node_query'], compiled: NativeGroup, raw: NativeRef): NativePacket {
    return nativePacketObject([['enabled', publicGroup.enabled], ['match', publicGroup.match], ['filters', nativePacketArray(publicGroup.filters.map((rule, index) => {
      const valueRef = nativeField(nativeChild(nativeField(raw, 'filters'), index), 'value');
      compiled.filters[index] = {...compiled.filters[index]!, valueRef};
      return nativePacketObject(Object.entries(rule).map(([key, value]) => [key, key === 'value' ? valueRef : derived(value)]));
    }))]]);
  }
  const node = groupPacket(publicSpec.node_query, bound.node_query, nativeField(input, 'node_query'));
  const relation = groupPacket(publicSpec.relation_query, bound.relation_query, nativeField(input, 'relation_query'));
  const paths = nativePacketArray(publicSpec.path_query.map((path, p) => nativePacketObject([
    ['path_id', path.path_id], ['quantifier', path.quantifier], ['steps', nativePacketArray(path.steps.map((step, s) => {
      const raw = nativeChild(nativeField(nativeChild(nativeField(input, 'path_query'), p), 'steps'), s);
      const compiled = bound.path_query[p]!.steps[s]!;
      return nativePacketObject([['direction', step.direction],
        ['node_query', groupPacket(step.node_query, compiled.node_query, nativeField(raw, 'node_query'))],
        ['relation_query', groupPacket(step.relation_query, compiled.relation_query, nativeField(raw, 'relation_query'))]]);
    }))],
  ])));
  const replacements = new Map<string, NativePacketValue>([['node_query', node], ['relation_query', relation], ['path_query', paths]]);
  return {spec: bound, publicPacket: nativePacketObject(Object.entries(publicSpec).map(([key, value]) => [key, replacements.get(key) ?? derived(value)]))};
}

export function nativeMatchesFilter(item: NativeRef, rule: NativeFilter): boolean {
  if (!rule.field) throw new Error('unbound property filter');
  const definition = rule._property_binding;
  if (definition) {
    const types = [nativeField(item, 'type_id').value];
    const inherited = nativeField(item, 'semantics.type_ancestors');
    if (definition.inherited && Array.isArray(inherited.value)) types.push(...arrayRefs(inherited).map(r => r.value));
    if (!definition.applies_to.some(type => types.includes(type))) return false;
  }
  const actual = nativeField(item, rule.field), expected = rule.valueRef;
  if (rule.op === 'exists') return (actual.value !== null) === pythonTruthy(expected);
  if (definition && actual.value === null) return false;
  if (definition && typeof actual.value === 'string' && ['contains', 'prefix'].includes(rule.op)) {
    return typeof expected.value === 'string' && (rule.op === 'contains' ? actual.value.includes(expected.value) : actual.value.startsWith(expected.value));
  }
  const eq = () => pythonEquals(actual, expected) || (Array.isArray(actual.value) && pythonMember(expected, actual));
  if (rule.op === 'eq') return eq();
  if (rule.op === 'neq') return !eq();
  const values = Array.isArray(expected.value) ? arrayRefs(expected) : [expected];
  if (rule.op === 'in') {
    if (Array.isArray(actual.value)) {
      const members = arrayRefs(actual);
      // Python set(actual) must finish even when an earlier member matches.
      if ([...members, ...values].some(ref => ref.value !== null && typeof ref.value === 'object')) throw new TypeError('unhashable native lens set member');
      return members.some(member => values.some(value => pythonEquals(member, value)));
    }
    return values.some(value => pythonEquals(actual, value));
  }
  if (rule.op === 'contains' && Array.isArray(actual.value)) return values.every(value => pythonMember(value, actual));
  if (rule.op === 'contains' && Array.isArray(expected.value)) return false;
  if (rule.op === 'contains' || rule.op === 'prefix') {
    const left = nativeLower(pythonStr(pythonTruthy(actual) ? actual : nativeScalar(''))), right = nativeLower(pythonStr(expected));
    return rule.op === 'contains' ? left.includes(right) : left.startsWith(right);
  }
  // Native _number deliberately casts integers to binary64; exact integer
  // equality and relational numeric coercion are different native contracts.
  if (typeof actual.value !== 'number' || typeof expected.value !== 'number') return false;
  if (!Number.isFinite(actual.value) || !Number.isFinite(expected.value)) throw new RangeError('native integer too large to convert to float');
  if (rule.op === 'gt') return actual.value > expected.value;
  if (rule.op === 'gte') return actual.value >= expected.value;
  if (rule.op === 'lt') return actual.value < expected.value;
  if (rule.op === 'lte') return actual.value <= expected.value;
  return false;
}
export function nativeMatchesGroup(item: NativeRef, group: NativeGroup): boolean {
  const results = group.filters.map(rule => nativeMatchesFilter(item, rule));
  return !results.length || (group.match === 'all' ? results.every(Boolean) : results.some(Boolean));
}
export function nativeSortedRows(rows: NativeRef[], rules: SortRule[], budget: {bytes: number; maxBytes: number}): NativeRef[] {
  // Final endpoint/focus closure may add rows after top-k selection. Render
  // each bounded source sort key once, not repeatedly inside the comparator.
  return rows.map(ref => ({ref, id: stringField(ref, 'id'), keys: rules.map(rule => {
    const key = nativeSortKey(nativeField(ref, rule.field)); budget.bytes += new TextEncoder().encode(key).length;
    if (budget.bytes > budget.maxBytes) throw new NativeBudgetExceeded('native lens final sort-key byte budget');
    return key;
  })}))
    .sort((a, b) => {
      for (let i = 0; i < rules.length; i++) {
        const delta = codePointCompare(a.keys[i]!, b.keys[i]!);
        if (delta) return rules[i]!.direction === 'desc' ? -delta : delta;
      }
      return codePointCompare(a.id, b.id);
    }).map(item => item.ref);
}
export type NativeLensGroup = {field: string; value: NativeRef; node_ids: string[]; relation_ids: string[]};
export function nativeGroups(nodes: NativeRef[], relations: NativeRef[], fields: string[], limit: number): NativeLensGroup[] {
  const result: NativeLensGroup[] = [];
  for (const field of fields) {
    const groups = new Map<string, NativeLensGroup>();
    for (const [kind, refs] of [['node', nodes], ['relation', relations]] as const) for (const ref of refs) {
      const raw = nativeField(ref, field);
      for (const member of Array.isArray(raw.value) ? arrayRefs(raw) : [raw]) {
        if (member.value === null) continue;
        const key = pythonStr(member);
        if (!groups.has(key)) groups.set(key, {field, value: member, node_ids: [], relation_ids: []});
        groups.get(key)![kind === 'node' ? 'node_ids' : 'relation_ids'].push(stringField(ref, 'id'));
      }
    }
    for (const key of [...groups.keys()].sort((a, b) => codePointCompare(nativeCasefold(a), nativeCasefold(b)))) {
      result.push(groups.get(key)!); if (result.length >= limit) return result;
    }
  }
  return result;
}
export function groupPacket(group: NativeLensGroup): NativePacket {
  return nativePacketObject([['field', group.field], ['value', group.value], ['node_ids', derived(group.node_ids)],
    ['relation_ids', derived(group.relation_ids)], ['node_count', nativeInteger(group.node_ids.length)], ['relation_count', nativeInteger(group.relation_ids.length)]]);
}

export async function nativeDigest(packet: NativePacketValue): Promise<string> {
  // Bounded reparse retains refs/kinds, never JSON.parse -> inferred JS numbers.
  const root = parseNativeJson(nativePacketJson(packet, {maxBytes: NATIVE_LENS_RESPONSE_BYTES}), {maxBytes: NATIVE_LENS_RESPONSE_BYTES});
  let size = 0; const chunks: string[] = [];
  const emit = (text: string) => {size += new TextEncoder().encode(text).length; if (size > NATIVE_LENS_RESPONSE_BYTES) throw new NativeBudgetExceeded('native fingerprint byte budget'); chunks.push(text);};
  const string = (value: string) => {
    if (/[\uD800-\uDFFF]/u.test(value)) throw new TypeError('native digest cannot encode lone surrogate');
    emit('s' + new TextEncoder().encode(value).length + ':' + value);
  };
  const walk = (ref: NativeRef): void => {
    const value = ref.value;
    if (value === null) emit('n;');
    else if (typeof value === 'string') string(value);
    else if (typeof value === 'boolean') emit(value ? 'b1;' : 'b0;');
    else if (typeof value === 'number') {
      if (!Number.isFinite(value)) throw new RangeError('native digest cannot encode nonfinite float');
      const bytes = new Uint8Array(8); new DataView(bytes.buffer).setFloat64(0, value === 0 ? 0 : value, false);
      emit('d' + [...bytes].map(b => b.toString(16).padStart(2, '0')).join('') + ';');
    } else if (Array.isArray(value)) {emit('a' + value.length + '['); arrayRefs(ref).forEach(walk); emit(']');}
    else {const keys = [...nativeKeys(ref)].sort(codePointCompare); emit('o' + keys.length + '{'); for (const key of keys) {string(key); walk(nativeChild(ref, key));} emit('}');}
  };
  walk(root);
  return [...new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(chunks.join(''))))].map(b => b.toString(16).padStart(2, '0')).join('');
}
