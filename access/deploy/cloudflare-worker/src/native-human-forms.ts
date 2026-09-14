/** Native numeric identity and source refs through the existing role selector.
 * This is the shared-v2 transport algorithm, not a second admission policy.
 */
import {selectHumanForms, HUMAN_FORM_ROLES, type HumanFormNativeAdapter} from './human-forms.ts';
import {nativeNumberInfo, pythonStr, nativeInteger, isNativeRef, codePointCompare} from '../../../shared/native-semantics.ts';
import {arrayRefs, derived, nativeChild, nativeField, nativeKeys, nativePacketArray, nativePacketObject,
  nativePacketJson, objectWith, parseNativeJson, type NativeRef, type NativePacket, type NativePacketValue} from './native-lens.ts';
type Item = Record<string, unknown>;
const EXPANDED = 524288, PACKET = 65536, WIRE = 16384;
const forbidden = new Set(['__proto__', 'prototype', 'constructor']);
const isObject = (ref: NativeRef) => ref.value !== null && typeof ref.value === 'object' && !Array.isArray(ref.value);
function asRef(value: NativePacketValue): NativeRef {
  return isNativeRef(value) ? value : parseNativeJson(nativePacketJson(value, {maxBytes: EXPANDED}), {maxBytes: EXPANDED});
}

export function nativeFormIdentity(ref: NativeRef): string {
  if (Array.isArray(ref.value)) return '[' + arrayRefs(ref).map(nativeFormIdentity).join(',') + ']';
  if (isObject(ref)) return '{' + [...nativeKeys(ref)].sort(codePointCompare).map(key => JSON.stringify(key) + ':' + nativeFormIdentity(nativeChild(ref, key))).join(',') + '}';
  return typeof ref.value === 'number' ? pythonStr(ref) : JSON.stringify(ref.value);
}
export function nativeFormCost(ref: NativeRef, budget: number): number {
  const stack: [NativeRef, number][] = [[ref, 0]]; let cost = 0, visits = 0;
  const textCost = (text: string) => new TextEncoder().encode(JSON.stringify(text)).length;
  while (stack.length) {
    const [current, depth] = stack.pop()!; const value = current.value;
    if (depth > 64 || ++visits > 30000) throw new Error('human form JSON exceeds structural bounds');
    if (typeof value === 'string') cost += textCost(value);
    else if (value === null || typeof value === 'boolean') cost += 5;
    else if (typeof value === 'number') {
      const info = nativeNumberInfo(current);
      if (info.kind === 'int' && BigInt(info.lexeme).toString(2).replace('-', '').length > 1023) throw new Error('human form integer exceeds portable JSON range');
      cost += Math.max(32, pythonStr(current).length);
    } else if (Array.isArray(value)) {cost += 2 + value.length; for (const ref of arrayRefs(current)) stack.push([ref, depth + 1]);}
    else {
      const keys = nativeKeys(current); cost += 2 + 2 * keys.length;
      for (const key of keys) {if (forbidden.has(key)) throw new Error('human form contains an unsafe object key'); visits++; cost += textCost(key); stack.push([nativeChild(current, key), depth + 1]);}
    }
    if (cost > budget || stack.length + visits > 30000) throw new Error('human form JSON exceeds byte or member budget');
  }
  return cost;
}
function common(values: NativeRef[]): NativePacket {
  const first = values[0], entries: [string, NativePacketValue][] = [];
  if (!first) return nativePacketObject(entries);
  for (const key of nativeKeys(first)) {
    if (!values.every(value => nativeKeys(value).includes(key))) continue;
    const items = values.map(value => nativeChild(value, key)), member = items[0]!;
    if (items.every(value => nativeFormIdentity(value) === nativeFormIdentity(member))) entries.push([key, member]);
    else if (items.every(isObject)) {const nested = common(items); if (nested.kind === 'object' && nested.entries.length) entries.push([key, nested]);}
  }
  return nativePacketObject(entries);
}
function subtract(value: NativeRef, base: NativeRef): NativePacket {
  const entries: [string, NativePacketValue][] = [];
  for (const key of nativeKeys(value)) {
    const member = nativeChild(value, key);
    if (!nativeKeys(base).includes(key)) entries.push([key, member]);
    else if (nativeFormIdentity(member) !== nativeFormIdentity(nativeChild(base, key))) {
      if (!isObject(member) || !isObject(nativeChild(base, key))) throw new Error('invalid common human form data');
      entries.push([key, subtract(member, nativeChild(base, key))]);
    }
  }
  return nativePacketObject(entries);
}
function encode(selection: NativeRef, enforceBudget: boolean): NativePacket {
  nativeFormCost(selection, EXPANDED);
  const roles = nativeField(selection, 'roles'), packets = new Map<string, NativeRef>(), limits: string[] = [];
  for (const role of HUMAN_FORM_ROLES) {
    const source = nativeField(nativeChild(roles, role), 'packet'); if (source.value === null) continue;
    nativeFormCost(source, PACKET);
    const admission = nativeField(source, 'admission'), changes = new Map<string, NativePacketValue>();
    if (isObject(admission)) {
      if (nativeKeys(admission).includes('limit_refs')) throw new Error('reserved admission.limit_refs');
      if (nativeKeys(admission).includes('limits')) {
        const indexes: NativeRef[] = [];
        for (const ref of arrayRefs(nativeField(admission, 'limits'))) {
          if (typeof ref.value !== 'string') throw new Error('invalid admission limit');
          if (!limits.includes(ref.value)) limits.push(ref.value);
          indexes.push(nativeInteger(limits.indexOf(ref.value)));
        }
        changes.set('admission', objectWith(admission, new Map([['limit_refs', nativePacketArray(indexes)]]), new Set(['limits'])));
      }
    }
    packets.set(role, asRef(objectWith(source, changes, new Set(['form']))));
  }
  if (limits.length > 512) throw new Error('excessive shared human form limits');
  const base = asRef(common([...packets.values()]));
  const resultRoles = nativePacketObject(HUMAN_FORM_ROLES.map(role => [role,
    objectWith(nativeChild(roles, role), new Map([['packet_delta', packets.has(role) ? subtract(packets.get(role)!, base) : null]]), new Set(['packet']))]));
  const result = objectWith(selection, new Map<string, NativePacketValue>([['schema_version', 'tos_human_form_selection_v2'], ['roles', resultRoles], ['packet_base', base], ['shared_limits', derived(limits)]]));
  nativeFormCost(asRef(result), enforceBudget ? WIRE : EXPANDED);
  return result;
}

export function nativeHumanForms(item: NativeRef, language: string): {packet: NativePacket; structural: Item} {
  const refs = new WeakMap<object, NativeRef>();
  const pending = [item];
  while (pending.length) {const ref = pending.pop()!; if (ref.value && typeof ref.value === 'object') {refs.set(ref.value, ref); for (const key of nativeKeys(ref)) pending.push(nativeChild(ref, key));}}
  const bridge = (value: unknown): NativePacketValue => {
    if (value && typeof value === 'object') {
      const ref = refs.get(value); if (ref) return ref;
      if (Array.isArray(value)) return nativePacketArray(value.map(bridge));
      return nativePacketObject(Object.entries(value).map(([key, value]) => [key, bridge(value)]));
    }
    return derived(value);
  };
  let packet: NativePacket | undefined, structural: Item = {};
  const adapter: HumanFormNativeAdapter = {
    exactRef(value) {return !value || typeof value !== 'object' || !refs.has(value) || this.integer(value as Item, 'version');},
    integer(value, key) {const ref = refs.get(value); if (!ref) return Number.isSafeInteger(value[key]); const member = nativeChild(ref, key); return typeof member.value === 'number' && nativeNumberInfo(member).kind === 'int';},
    identity(value) {return nativeFormIdentity(asRef(bridge(value)));},
    boundedCost(value, budget) {return nativeFormCost(asRef(bridge(value)), budget);},
    cost(value) {return nativeFormCost(asRef(encode(asRef(bridge(value)), false)), EXPANDED);},
    deliver(value) {
      packet = encode(asRef(bridge(value)), true);
      const roles = value.roles as Record<string, Item>;
      structural = {schema_version: 'tos_human_form_selection_v2', roles: Object.fromEntries(HUMAN_FORM_ROLES.map(role => [role, {state: roles[role]!.state}]))};
      return {}; // Transport result is retained separately, never a lossy packet.
    },
  };
  selectHumanForms(item.value as Item, language, 'shared-v2', adapter);
  if (!packet) throw new Error('native human form delivery did not finish');
  return {packet, structural};
}
