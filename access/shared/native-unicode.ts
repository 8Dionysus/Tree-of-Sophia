import {nativeUnicodeVersion, nativeUnicodeAlgorithm, nativeLowerMappings, nativeCasefoldMappings, nativeDecimalMappings,
  nativeCasedRanges, nativeCaseIgnorableRanges, nativePrintableRanges} from './native-unicode.generated.ts';

export {nativeUnicodeVersion, nativeUnicodeAlgorithm};
const lower = new Map(nativeLowerMappings);
const casefold = new Map(nativeCasefoldMappings);
const decimal = new Map(nativeDecimalMappings);
const stripEdges = /^[\u0009-\u000d\u001c-\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+|[\u0009-\u000d\u001c-\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+$/gu;
export function nativeStrip(value: string): string {return value.replace(stripEdges, '');}
export function nativeIntegerString(value: string): number {
  // Python int accepts decimal Unicode digits, underscores and Unicode space,
  // but its ASCII integer grammar excludes the four C0 record separators.
  if (/[\u001c-\u001f]/u.test(value)) return NaN;
  const normalized = Array.from(nativeStrip(value), char => decimal.get(char.codePointAt(0)!) ?? char).join('');
  return /^[+-]?[0-9](?:_?[0-9])*$/u.test(normalized) ? Number(normalized.replaceAll('_', '')) : NaN;
}
export function nativeCasefold(value: string): string {
  return Array.from(value, char => casefold.get(char.codePointAt(0)!) ?? char).join('');
}

function inRanges(point: number, ranges: ReadonlyArray<readonly [number, number]>): boolean {
  let lo = 0, hi = ranges.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    const [start, end] = ranges[mid]!;
    if (point < start) hi = mid;
    else if (point > end) lo = mid + 1;
    else return true;
  }
  return false;
}

export function nativeIsPrintable(point: number): boolean {
  return inRanges(point, nativePrintableRanges);
}

export function codePointCompare(left: string, right: string): number {
  let a = 0, b = 0;
  while (a < left.length && b < right.length) {
    const first = left.codePointAt(a)!, second = right.codePointAt(b)!;
    if (first !== second) return first < second ? -1 : 1;
    a += first > 0xffff ? 2 : 1;
    b += second > 0xffff ? 2 : 1;
  }
  return a < left.length ? 1 : b < right.length ? -1 : 0;
}

/** Pinned Unicode Default Lowercase. Final_Sigma inspects original text.
 * Two passes avoid quadratic scans through long Case_Ignorable runs.
 */
export function nativeLower(value: string, expectedVersion: string = nativeUnicodeVersion): string {
  if (expectedVersion !== nativeUnicodeVersion) throw new Error('native Unicode version is incompatible');
  const chars = Array.from(value);
  const followingCased = new Uint8Array(chars.length);
  let next = false;
  for (let index = chars.length - 1; index >= 0; index--) {
    const point = chars[index]!.codePointAt(0)!;
    followingCased[index] = Number(next);
    if (!inRanges(point, nativeCaseIgnorableRanges)) next = inRanges(point, nativeCasedRanges);
  }
  let previous = false;
  for (let index = 0; index < chars.length; index++) {
    const point = chars[index]!.codePointAt(0)!;
    chars[index] = point === 0x3a3 && previous && !followingCased[index]
      ? '\u03c2' : lower.get(point) ?? chars[index]!;
    if (!inRanges(point, nativeCaseIgnorableRanges)) previous = inRanges(point, nativeCasedRanges);
  }
  return chars.join('');
}
