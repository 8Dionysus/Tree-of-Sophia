// Test-only JSON comparison, independent of ToS's Rust/WASM implementation.
// Keep integer/float identity, arbitrary integers, signed zero and key order.
export function parseLosslessJson(raw) {
  let at = 0;
  const white = () => { while (/[\x20\t\r\n]/.test(raw[at] ?? 'x')) at++; };
  const string = () => {
    const start = at++;
    while (at < raw.length) {
      const c = raw[at++];
      if (c === '\\') { at++; continue; }
      if (c === '"') return JSON.parse(raw.slice(start, at));
    }
    throw new SyntaxError('unterminated JSON string');
  };
  function value(depth = 0) {
    if (depth > 512) throw new RangeError('JSON comparison depth');
    white();
    if (raw[at] === '"') return {kind: 'str', value: string()};
    if (raw[at] === '[' || raw[at] === '{') {
      const object = raw[at++] === '{', closing = object ? '}' : ']';
      const values = [], indices = new Map();
      white();
      if (raw[at] !== closing) for (;;) {
        white();
        let key;
        if (object) {
          if (raw[at] !== '"') throw new SyntaxError('JSON object key');
          key = string(); white();
          if (raw[at++] !== ':') throw new SyntaxError('JSON colon');
        }
        const item = value(depth + 1);
        if (object) {
          // json.loads retains the first insertion position and last value.
          if (indices.has(key)) values[indices.get(key)][1] = item;
          else { indices.set(key, values.length); values.push([key, item]); }
        } else values.push(item);
        white();
        if (raw[at] === closing) break;
        if (raw[at++] !== ',') throw new SyntaxError('JSON separator');
      }
      at++;
      return {kind: object ? 'dict' : 'list', value: values};
    }
    for (const [token, kind, item] of [['true','bool',true],['false','bool',false],['null','null',null]]) {
      if (raw.startsWith(token, at)) { at += token.length; return {kind, value:item}; }
    }
    const match = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/.exec(raw.slice(at));
    if (!match) throw new SyntaxError('JSON value');
    at += match[0].length;
    const floating = /[.eE]/.test(match[0]);
    const item = floating ? Number(match[0]) : BigInt(match[0]);
    if (floating && !Number.isFinite(item)) throw new SyntaxError('nonfinite JSON number');
    return {kind:floating ? 'float' : 'int', value:item, raw:match[0]};
  }
  const result = value(); white();
  if (at !== raw.length) throw new SyntaxError('trailing JSON input');
  return result;
}

export function stringifyLosslessJson(node) {
  if (node.kind === 'dict') return `{${node.value.map(([key, value]) => `${JSON.stringify(key)}:${stringifyLosslessJson(value)}`).join(',')}}`;
  if (node.kind === 'list') return `[${node.value.map(stringifyLosslessJson).join(',')}]`;
  if (node.kind === 'int' || node.kind === 'float') return node.raw;
  return JSON.stringify(node.value);
}

export function compareLosslessJson(actual, expected, {indexed = false, unordered = () => false} = {}) {
  const left = parseLosslessJson(actual), right = parseLosslessJson(expected);
  if (indexed) for (const packet of [left, right]) {
    if (packet.kind !== 'dict') throw new TypeError('indexed packet object');
    const work = packet.value.findIndex(([key]) => key === 'work');
    if (work < 0) throw new TypeError('indexed work field absent');
    packet.value.splice(work, 1);
    const page = packet.value.find(([key]) => key === 'page')?.[1];
    if (page?.kind !== 'dict') throw new TypeError('indexed page object');
    for (const key of ['cursor','next_cursor']) {
      const pair = page.value.find(([name]) => name === key);
      if (!pair) throw new TypeError('indexed cursor field absent');
      pair[1] = {kind:'bool',value:pair[1].kind !== 'null'};
    }
  }
  return compareLosslessValues(left, right, {unordered});
}

export function compareLosslessValues(left, right, {unordered = () => false} = {}) {
  function diff(a, b, path = '$') {
    if (a.kind !== b.kind) return [`${path}: kind ${a.kind} != ${b.kind}`];
    if (a.kind === 'dict') {
      const ak = a.value.map(([key]) => key), bk = b.value.map(([key]) => key);
      const same = unordered(path)
        ? ak.length === bk.length && ak.every(key => bk.includes(key))
        : ak.length === bk.length && ak.every((key, index) => key === bk[index]);
      if (!same) return [`${path}: ordered keys differ`];
      const other = new Map(b.value);
      return a.value.flatMap(([key, item]) => diff(item, other.get(key), `${path}.${key}`));
    }
    if (a.kind === 'list') {
      if (a.value.length !== b.value.length) return [`${path}: lengths differ`];
      return a.value.flatMap((item, index) => diff(item, b.value[index], `${path}[${index}]`));
    }
    return (a.kind === 'float' ? Object.is(a.value, b.value) : a.value === b.value)
      ? [] : [`${path}: value differs`];
  }
  return diff(left, right);
}
