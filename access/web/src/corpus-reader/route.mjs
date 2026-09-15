// A reader URL contains source locators only. Notes, quotations and text payloads
// never enter the URL. Resolution still belongs to the supplied text provider.
const fields = ['documentId', 'versionId', 'unitId', 'revision'];
export function readingRoute(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new TypeError('Invalid reading address.');
  const result = {};
  for (const key of fields) {
    const text = value[key];
    if (typeof text !== 'string' || !text.length || text.length > 512 || /[\u0000-\u001f\u007f]/u.test(text)) {
      throw new TypeError('Incomplete reading address.');
    }
    result[key] = text;
  }
  return result;
}
export function encodeReadingRoute(value) {
  return '#reading=' + encodeURIComponent(JSON.stringify(readingRoute(value)));
}
export function decodeReadingRoute(hash) {
  if (!hash?.startsWith('#reading=')) return null;
  if (hash.length > 14000) throw new TypeError('Reading address is too large.');
  return readingRoute(JSON.parse(decodeURIComponent(hash.slice(9))));
}
