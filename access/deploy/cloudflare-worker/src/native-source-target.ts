/** Project exact source-read targets from already-retained native rows.
 *
 * This is a pure packet projection. It never opens a source owner, resolves a
 * path, or claims that the owner can currently serve the target. NativeRef is
 * kept throughout so Python integer kinds, large integer lexemes, Unicode and
 * source object order remain available to the canonical source digest.
 */
import {HttpError} from './common.ts';
import {NativeBudgetExceeded, nativeChild, nativeField, nativeKeys, nativeNumberInfo, pythonStr, codePointCompare,
  type NativeRef} from '../../../shared/native-semantics.ts';
import {nativePacketJson, nativePacketObject, type NativePacket} from './native-lens.ts';

const TRANSFORM_VERSION = 'tos-knowledge-normalization-v2';
const BARE_DIGEST = /^[a-f0-9]{64}(?![\s\S])/;
const METADATA_ID = /^tos\.(?!claim\.)[a-z0-9]+(?:[.-][a-z0-9]+)*(?![\s\S])/;
const CLAIM_ID = /^tos\.claim\.[a-z0-9]+(?:[.-][a-z0-9]+)*(?![\s\S])/;
const RECORD_TYPE = /^[a-z][a-z0-9-]{0,63}(?![\s\S])/;
const MAX_SAFE_INTEGER = 9007199254740991n;
const NATIVE_METADATA_SCHEMAS = new Map<string, readonly [string, string]>([
  ['tos_scholarly_composite_witness_v1', ['composite', 'composite_id']],
  ['tos_artifact_source_witness_v1', ['artifact', 'artifact_id']],
  ['tos_artifact_source_witness_v2', ['artifact', 'artifact_id']],
]);

const isObject = (ref: NativeRef): boolean => ref.value !== null && typeof ref.value === 'object' && !Array.isArray(ref.value);

/** Python json.dumps(sort_keys=True, ensure_ascii=False, allow_nan=False). */
export function canonicalNativeJson(ref: NativeRef): string {
  // A short legal float token can expand during Python-compatible rendering.
  // Keep this work budget independent from the row and source companion caps.
  let remaining = 8 * 1024 * 1024;
  const emit = (text: string): string => {
    remaining -= text.length;
    if (remaining < 0) throw new NativeBudgetExceeded('temporal canonical character-work budget');
    return text;
  };
  function visit(value: NativeRef): string {
    if (typeof value.value === 'number') return emit(pythonStr(value));
    if (typeof value.value === 'string' && !value.value.isWellFormed()) throw new HttpError(503, 'prepared response contains invalid JSON values');
    if (value.value === null || typeof value.value === 'boolean' || typeof value.value === 'string') return emit(JSON.stringify(value.value));
    const array = Array.isArray(value.value);
    const keys = array ? nativeKeys(value) : [...nativeKeys(value)].sort(codePointCompare);
    return emit(array ? '[' : '{') + keys.map((key, index) => {
      if (!key.isWellFormed()) throw new HttpError(503, 'prepared response contains invalid JSON values');
      return (index ? emit(',') : '') + (array ? '' : emit(JSON.stringify(key) + ':')) + visit(nativeChild(value, key));
    }).join('') + emit(array ? ']' : '}');
  }
  return visit(ref);
}

async function sha256(text: string): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map(byte => byte.toString(16).padStart(2, '0')).join('');
}

function exactVersion(ref: NativeRef): NativeRef | null {
  if (typeof ref.value !== 'number') return null;
  try {
    const info = nativeNumberInfo(ref);
    if (info.kind !== 'int') return null;
    const value = BigInt(info.lexeme);
    return value >= 1n && value <= MAX_SAFE_INTEGER ? ref : null;
  } catch {
    return null;
  }
}

async function exactTargetFromRecord(record: NativeRef, layer: 'metadata_record' | 'claim_record'): Promise<NativePacket | null> {
  if (!isObject(record)) return null;
  try {
    let native: readonly [string, string] | undefined;
    if (layer === 'metadata_record') {
      const schema = nativeField(record, 'schema_version').value;
      if (schema !== null && typeof schema === 'object') return null;
      native = typeof schema === 'string' ? NATIVE_METADATA_SCHEMAS.get(schema) : undefined;
      if (native && (nativeKeys(record).includes('record_id') || nativeKeys(record).includes('record_type'))) return null;
    }
    const id = nativeField(record, layer === 'claim_record' ? 'claim_id' : native?.[1] ?? 'record_id');
    const type = layer === 'metadata_record' ? native?.[0] ?? nativeField(record, 'record_type').value : null;
    const version = nativeField(record, layer === 'claim_record' ? 'claim_version' : 'record_version');
    if (typeof id.value !== 'string' || !(layer === 'claim_record' ? CLAIM_ID : METADATA_ID).test(id.value)) return null;
    if (layer === 'metadata_record' && (typeof type !== 'string' || !RECORD_TYPE.test(type))) return null;
    if (native && !id.value.startsWith('tos.' + native[0] + '.')) return null;
    const exactVersionRef = exactVersion(version);
    if (!exactVersionRef) return null;
    const digest = 'sha256:' + await sha256(canonicalNativeJson(record));
    const target = layer === 'claim_record'
      ? nativePacketObject([
        ['layer', 'claim_record'],
        ['record_ref', nativePacketObject([['id', id.value], ['version', exactVersionRef], ['digest', digest]])],
        ['content_revision', digest],
      ])
      : nativePacketObject([
        ['layer', 'metadata_record'],
        ['record_type', type as string],
        ['record_ref', nativePacketObject([['id', id.value], ['version', exactVersionRef], ['digest', digest]])],
        ['content_revision', digest],
      ]);
    return target;
  } catch {
    // Projection is deliberately fail-closed: a malformed or unsupported
    // carrier disappears from the target map instead of becoming a guess.
    return null;
  }
}

async function targetForItem(item: NativeRef): Promise<NativePacket | null> {
  if (!isObject(item)) return null;
  const envelope = nativeField(item, 'source_record');
  if (!isObject(envelope) || nativeKeys(envelope).length !== 4
      || !['payload', 'digest', 'transform_version', 'field_map'].every(key => nativeKeys(envelope).includes(key))) return null;
  const digest = nativeField(envelope, 'digest').value;
  if (typeof digest !== 'string' || !BARE_DIGEST.test(digest)
      || nativeField(envelope, 'transform_version').value !== TRANSFORM_VERSION) return null;
  const fieldMap = nativeField(envelope, 'field_map');
  if (!isObject(fieldMap)) return null;
  for (const key of nativeKeys(fieldMap)) if (typeof nativeChild(fieldMap, key).value !== 'string') return null;
  const properties = nativeField(envelope, 'payload.properties');
  if (!isObject(properties)) return null;
  const rawMetadata = nativeField(properties, 'source_record'), rawClaim = nativeField(properties, 'source_claim');
  if (rawMetadata.value !== null && rawClaim.value !== null) return null;
  if (rawClaim.value !== null) return exactTargetFromRecord(rawClaim, 'claim_record');
  if (rawMetadata.value !== null) {
    const payload = nativeField(envelope, 'payload');
    if (nativeKeys(payload).includes('pack_id') || nativeKeys(payload).includes('edge_id')) {
      const pack = nativeField(payload, 'pack_id').value, edge = nativeField(payload, 'edge_id').value;
      const ordinal = exactVersion(nativeField(properties, 'source_row'));
      const fileDigest = nativeField(properties, 'source_file_sha256').value;
      const bytes = (value: string) => new TextEncoder().encode(value).length;
      if (typeof pack !== 'string' || !pack.isWellFormed() || bytes(pack) > 2048 || !/^(canon\/relations\/|candidate-intake\/)/.test(pack)
          || pack.split('/').some(part => !part || part.startsWith('.') || part === 'payload')
          || /[\\\u0000]/.test(pack) || typeof edge !== 'string' || !edge.isWellFormed() || !edge || bytes(edge) > 2048 || edge.includes('\0')
          || !ordinal || typeof fileDigest !== 'string' || !BARE_DIGEST.test(fileDigest) || !isObject(rawMetadata)) return null;
      for (const key of nativeKeys(rawMetadata)) {
        const value = nativeChild(rawMetadata, key).value;
        if (value !== null && typeof value !== 'string') return null;
      }
      try {
        return nativePacketObject([['layer', 'authored_csv_record'], ['pack_id', pack], ['edge_id', edge],
          ['source_row', ordinal], ['source_file_sha256', fileDigest],
          ['content_revision', 'sha256:' + await sha256(canonicalNativeJson(rawMetadata))]]);
      } catch { return null; }
    }
    return exactTargetFromRecord(rawMetadata, 'metadata_record');
  }
  return null;
}

/** Build the same deterministic target-only map as Python inspection. */
export async function nativeSourceReadTargets(items: readonly NativeRef[], sourceRevision: NativeRef): Promise<NativePacket> {
  if (typeof sourceRevision.value !== 'string' || !BARE_DIGEST.test(sourceRevision.value)) return nativePacketObject([]);
  const seen = new Map<string, {target: NativePacket | null; key: string | null}>(), conflicts = new Set<string>();
  for (const item of items) {
    if (!isObject(item)) continue;
    const itemId = nativeField(item, 'id').value;
    if (typeof itemId !== 'string' || !itemId) continue;
    const target = await targetForItem(item);
    const key = target ? nativePacketJson(target, {maxBytes: 16384}) : null;
    const previous = seen.get(itemId);
    if (previous) {
      if (previous.key !== key) conflicts.add(itemId);
      continue;
    }
    seen.set(itemId, {target, key});
  }
  const entries: [string, NativePacket][] = [];
  for (const [itemId, value] of [...seen.entries()].sort(([left], [right]) => codePointCompare(left, right))) {
    if (value.target && !conflicts.has(itemId)) entries.push([itemId, nativePacketObject([
      ['source_revision', sourceRevision], ['target', value.target],
    ])]);
  }
  return nativePacketObject(entries);
}
