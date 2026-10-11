// The initialized tos-web-rules module owns literal matching and Unicode
// semantics. This host adapter only transfers JSON and restores Error values.
let readerOccurrenceRules = null;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function installReaderOccurrenceRules(bindings) {
  if (typeof bindings?.reader_occurrences_wasm_v1 !== 'function') {
    throw new Error('reader occurrence WASM rule is unavailable');
  }
  readerOccurrenceRules = bindings;
}

export function findInParagraphs(paragraphs, query, {limit = 500} = {}) {
  if (!readerOccurrenceRules) throw new Error('reader occurrence WASM rule is not installed');
  let request;
  try {
    // JavaScript's RegExp.exec coerces a sparse-array hole to "undefined";
    // JSON would otherwise collapse it to null before the Rust input check.
    const strings = Array.isArray(paragraphs)
      ? Array.from({length: paragraphs.length}, (_, index) => {
        if (!(index in paragraphs)) return 'undefined';
        const value = paragraphs[index];
        return typeof value === 'string' ? value : null;
      })
      : null;
    // JSON serialization must not let user-defined toJSON hooks turn invalid
    // JavaScript input types into valid Rust request fields.
    request = encoder.encode(JSON.stringify({
      paragraphs: strings,
      query: typeof query === 'string' ? query : null,
      limit: typeof limit === 'number' ? limit : null,
    }));
  } catch {
    throw new Error('invalid-input');
  }
  let response;
  try {
    response = readerOccurrenceRules.reader_occurrences_wasm_v1(request);
  } catch (error) {
    const message = typeof error === 'string' ? error : error?.message;
    if (message === 'limit' || message === 'invalid-input') throw new Error(message);
    throw error;
  }
  return JSON.parse(decoder.decode(response));
}
