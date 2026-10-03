/** Retained native JSON transport helpers shared by D1 custody, search,
 * exploration and human forms. Published lens domain execution lives in Rust. */
import {nativeChild, nativeField, nativeKeys, nativeInteger, nativePacketArray, nativePacketObject,
  nativePacketJson, parseNativeJson, parseNativeRequest, isNativeRef,
  type NativeRef, type NativePacketValue, type NativePacket} from '../../../shared/native-semantics.ts';
export {nativeChild, nativeField, nativeKeys, nativePacketArray, nativePacketObject, nativePacketJson, parseNativeJson, parseNativeRequest};
export type {NativeRef, NativePacket, NativePacketValue};

export const NATIVE_LENS_RESPONSE_BYTES = 16 * 1024 * 1024;
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
