/** Lossless bounded transport only. Does not assess or admit source material. */
type Item = Record<string, unknown>;
export const FORM_CODEC_ROLES = ['name', 'caption', 'hover', 'statement', 'grounds', 'history', 'technical'];
export const FORM_WIRE_BUDGET = 16_384;
export const FORM_EXPANDED_BUDGET = 524_288;
export const FORM_PACKET_BUDGET = 65_536;
const FORBIDDEN = new Set(['__proto__', 'prototype', 'constructor']);
const encoder = new TextEncoder();

function object(value: unknown): value is Item {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null);
}

function exactRef(value: unknown): boolean {
  return object(value) && Object.keys(value).sort().join(',') === 'digest,id,version'
    && typeof value.id === 'string' && value.id.length > 0
    && typeof value.version === 'number' && Number.isSafeInteger(value.version) && value.version >= 1
    && typeof value.digest === 'string' && /^sha256:[a-f0-9]{64}$(?![\s\S])/.test(value.digest);
}

export function boundedFormCost(value: unknown, budget: number): number {
  const stack: [unknown, number][] = [[value, 0]];
  let cost = 0, members = 0;
  while (stack.length) {
    const [node, depth] = stack.pop()!;
    if (depth > 64 || ++members > 30_000) throw new Error('human form JSON exceeds structural bounds');
    if (typeof node === 'string') {
      if (node.length > budget - cost) throw new Error('human form JSON exceeds byte budget');
      cost += encoder.encode(JSON.stringify(node)).length;
    }
    else if (node === null || typeof node === 'boolean') cost += 5;
    else if (typeof node === 'number' && Number.isFinite(node)) cost += Math.max(32, String(node).length);
    else if (Array.isArray(node)) {
      if (stack.length + members + node.length > 30_000) throw new Error('human form JSON exceeds member budget');
      const keys = Reflect.ownKeys(node);
      if (keys.length !== node.length + 1 || keys.some(key => key !== 'length' && (typeof key !== 'string' || !/^(0|[1-9][0-9]*)$/.test(key)))) throw new Error('human form contains a non-JSON array property');
      cost += 2 + node.length;
      for (let index = 0; index < node.length; index++) {
        const descriptor = Object.getOwnPropertyDescriptor(node, String(index));
        if (!descriptor?.enumerable || !Object.hasOwn(descriptor, 'value')) throw new Error('human form contains a sparse or accessor array');
        stack.push([descriptor.value, depth + 1]);
      }
    } else if (object(node)) {
      const keys = Reflect.ownKeys(node);
      if (stack.length + members + 2 * keys.length > 30_000) throw new Error('human form JSON exceeds member budget');
      if (keys.some(key => typeof key !== 'string' || FORBIDDEN.has(key))) throw new Error('human form contains an unsafe object key');
      cost += 2 + 2 * keys.length;
      for (const key of keys) {
        const descriptor = Object.getOwnPropertyDescriptor(node, key)!;
        if (!descriptor.enumerable || !Object.hasOwn(descriptor, 'value')) throw new Error('human form contains a non-JSON property');
        stack.push([key, depth + 1], [descriptor.value, depth + 1]);
      }
    } else throw new Error('human form contains a non-JSON value');
    if (cost > budget || stack.length + members > 30_000) throw new Error('human form JSON exceeds byte or member budget');
  }
  return cost;
}

function identity(value: unknown): string {
  if (Array.isArray(value)) return '[' + value.map(identity).join(',') + ']';
  if (object(value)) return '{' + Object.keys(value).sort().map(key => JSON.stringify(key) + ':' + identity(value[key])).join(',') + '}';
  return JSON.stringify(value);
}

function common(values: Item[]): Item {
  const result: Item = {}, first = values[0];
  if (!first) return result;
  for (const [key, member] of Object.entries(first)) {
    if (!values.every(value => Object.hasOwn(value, key))) continue;
    const items = values.map(value => value[key]);
    if (items.every(value => identity(value) === identity(member))) result[key] = structuredClone(member);
    else if (items.every(object)) {
      const nested = common(items);
      if (Object.keys(nested).length) result[key] = nested;
    }
  }
  return result;
}

function subtract(value: Item, base: Item): Item {
  const result: Item = {};
  for (const [key, member] of Object.entries(value)) {
    if (!Object.hasOwn(base, key)) result[key] = structuredClone(member);
    else if (identity(member) !== identity(base[key])) {
      if (!object(member) || !object(base[key])) throw new Error('invalid common human form data');
      result[key] = subtract(member, base[key]);
    }
  }
  return result;
}

function merge(base: Item, delta: Item): Item {
  const result = structuredClone(base);
  for (const [key, member] of Object.entries(delta)) {
    const original = base[key];
    if (!Object.hasOwn(base, key)) result[key] = structuredClone(member);
    else if (object(original) && object(member) && Object.keys(original).length && Object.keys(member).length) result[key] = merge(original, member);
    else throw new Error('human form delta overlaps a base leaf');
  }
  return result;
}

function selectionRoles(value: unknown, version: string, packetKey: string): Record<string, Item> {
  if (!object(value) || value.schema_version !== version) throw new Error('invalid human form selection version');
  const roles = value.roles;
  if (!object(roles) || Object.keys(roles).sort().join(',') !== [...FORM_CODEC_ROLES].sort().join(',')) throw new Error('invalid human form selection roles');
  const result: Record<string, Item> = {};
  for (const role of FORM_CODEC_ROLES) {
    const selected = roles[role];
    if (!object(selected) || Object.keys(selected).sort().join(',') !== ['state', 'reason', 'form', packetKey].sort().join(',')) throw new Error('invalid human form role envelope');
    if ((selected.state === 'ready' && !object(selected[packetKey])) || (selected.state !== 'ready' && selected[packetKey] !== null)) throw new Error('human form role state contradicts packet');
    result[role] = selected;
  }
  return result;
}

export function encodeHumanFormSelection(selection: Item, {enforceBudget = true}: {enforceBudget?: boolean} = {}): Item {
  boundedFormCost(selection, FORM_EXPANDED_BUDGET);
  const roles = selectionRoles(selection, 'tos_human_form_selection_v1', 'packet');
  if (Object.hasOwn(selection, 'packet_base') || Object.hasOwn(selection, 'shared_limits')) throw new Error('reserved human form wire field in v1 selection');
  const packets: Record<string, Item> = {}, limits: string[] = [];
  for (const role of FORM_CODEC_ROLES) {
    const source = roles[role]!.packet;
    if (source === null) continue;
    if (!object(source)) throw new Error('invalid human form packet');
    boundedFormCost(source, FORM_PACKET_BUDGET);
    if (!exactRef(roles[role]!.form) || !exactRef(source.form) || identity(roles[role]!.form) !== identity(source.form)) throw new Error('human form role and packet exact refs differ');
    const packet = structuredClone(source), admission = packet.admission;
    delete packet.form;
    if (object(admission)) {
      if (Object.hasOwn(admission, 'limit_refs')) throw new Error('reserved admission.limit_refs in source packet');
      if (Object.hasOwn(admission, 'limits')) {
        if (!Array.isArray(admission.limits) || !admission.limits.every((limit): limit is string => typeof limit === 'string')) throw new Error('invalid admission limits');
        const refs: number[] = [];
        for (const limit of admission.limits) {
          if (!limits.includes(limit)) limits.push(limit);
          refs.push(limits.indexOf(limit));
        }
        delete admission.limits;
        admission.limit_refs = refs;
      }
    }
    packets[role] = packet;
  }
  if (limits.length > 512) throw new Error('excessive shared human form limits');
  const base = common(Object.values(packets)), result = structuredClone(selection);
  result.schema_version = 'tos_human_form_selection_v2';
  result.packet_base = base;
  result.shared_limits = limits;
  const resultRoles = selectionRoles(result, 'tos_human_form_selection_v2', 'packet');
  for (const role of FORM_CODEC_ROLES) {
    const selected = resultRoles[role]!;
    delete selected.packet;
    selected.packet_delta = Object.hasOwn(packets, role) ? subtract(packets[role]!, base) : null;
  }
  boundedFormCost(result, enforceBudget ? FORM_WIRE_BUDGET : FORM_EXPANDED_BUDGET);
  return result;
}

export function decodeHumanFormSelection(selection: unknown): Item {
  boundedFormCost(selection, FORM_WIRE_BUDGET);
  const roles = selectionRoles(selection, 'tos_human_form_selection_v2', 'packet_delta');
  if (!object(selection)) throw new Error('invalid human form selection');
  const base = selection.packet_base, limits = selection.shared_limits;
  if (!object(base) || !Array.isArray(limits) || limits.length > 512 || !limits.every((limit): limit is string => typeof limit === 'string')) throw new Error('invalid shared human form data');
  if (Object.hasOwn(base, 'form')) throw new Error('inline form ref in shared human form base');
  if (new Set(limits).size !== limits.length) throw new Error('duplicate shared human form limit');
  if (!FORM_CODEC_ROLES.some(role => roles[role]!.state === 'ready') && (Object.keys(base).length || limits.length)) throw new Error('unused shared human form data');
  const result = structuredClone(selection), resultRoles = selectionRoles(result, 'tos_human_form_selection_v2', 'packet_delta');
  result.schema_version = 'tos_human_form_selection_v1';
  delete result.packet_base;
  delete result.shared_limits;
  const used = new Set<number>();
  for (const role of FORM_CODEC_ROLES) {
    const selected = resultRoles[role]!, delta = selected.packet_delta;
    delete selected.packet_delta;
    let packet: Item | null = null;
    if (delta !== null) {
      if (!object(delta)) throw new Error('invalid human form delta');
      if (Object.hasOwn(delta, 'form') || !exactRef(selected.form)) throw new Error('invalid or shadowed shared human form ref');
      packet = merge(base, delta);
      packet.form = structuredClone(selected.form);
      const admission = packet.admission;
      if (object(admission) && Object.hasOwn(admission, 'limits')) throw new Error('inline limits in shared human form packet');
      if (object(admission) && Object.hasOwn(admission, 'limit_refs')) {
        const refs = admission.limit_refs;
        if (!Array.isArray(refs) || !refs.every((index): index is number => typeof index === 'number' && Number.isSafeInteger(index) && index >= 0 && index < limits.length)) throw new Error('invalid shared human form limit index');
        delete admission.limit_refs;
        admission.limits = refs.map(index => { used.add(index); return limits[index]!; });
      }
      boundedFormCost(packet, FORM_PACKET_BUDGET);
    }
    selected.packet = packet;
  }
  if (used.size !== limits.length) throw new Error('unused shared human form limit');
  boundedFormCost(result, FORM_EXPANDED_BUDGET);
  return result;
}
