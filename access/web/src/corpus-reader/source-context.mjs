import {editorialRanges,sourceLayerKind} from './editorial.mjs';

// Context coordinates always refer to the original unit, in Unicode code
// points. Do not rewrite the source text or the selected exact reference.
export function selectedSourceContext({reference, version, unit, limit = 2000} = {}) {
  const text = Array.from(String(unit?.text ?? ''));
  const selector = reference?.selector;
  const revision = version?.revision || version?.source?.textLayerSha256;
  if (!reference || !version || !unit || typeof unit.text !== 'string'
    || reference.versionId !== version.id || reference.unitId !== (unit.unitId || unit.id)
    || !revision || reference.target?.textLayerSha256 !== revision
    || (version.source?.textLayerSha256 && version.source.textLayerSha256 !== revision)
    || !Number.isSafeInteger(selector?.start) || !Number.isSafeInteger(selector?.end)
    || selector.start < 0 || selector.end < selector.start || selector.end > text.length
    || !Number.isSafeInteger(limit) || limit <= 0) return null;

  const length = Math.min(limit, text.length);
  const selectedLength = selector.end - selector.start;
  const padding = Math.max(0, Math.floor((length - selectedLength) / 2));
  const start = Math.max(0, Math.min(selector.start - padding, text.length - length));
  const end = start + length;
  const editorialAnnotations = editorialRanges(version, unit)
    .filter(span => span.start < end && span.end > start)
    .map(span => ({kind:span.kind, start:Math.max(start,span.start), end:Math.min(end,span.end)}));
  const wholeUnitRole=sourceLayerKind(version,unit);
  return {
    context:text.slice(start,end).join(''),
    contextRange:{start,end,positionUnit:'unicode_code_point',interval:'half_open'},
    contextTruncated:start > 0 || end < text.length,
    editorialAnnotations,
    ...(wholeUnitRole ? {sourceLayerKind:wholeUnitRole} : {}),
  };
}
