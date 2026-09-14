/** Internal bounded native D1 delivery and emitted-row integrity reader. */
import {HttpError} from './common.ts';
import {nativeLower, codePointCompare, NativeBudgetExceeded, nativeNumberInfo, pythonStr} from '../../../shared/native-semantics.ts';
import {nativeKeys, nativeField, nativeChild, nativePacketJson, parseNativeJson, stringField, type NativeRef} from './native-lens.ts';
export type NativeKind = 'node' | 'relation';
const IDENTITY = {node: ['id', 'entity_id', 'native_id', 'source_graph', 'kind_id', 'type_id'],
  relation: ['id', 'native_id', 'source_graph', 'from_id', 'to_id', 'predicate_id', 'relation_type_id']} as const;
export const nativeD1Limits = Object.freeze({maxCandidates: 2048, maxCallbacks: 32768, maxDecodedBytes: 16 * 1024 * 1024,
  maxSortBytes: 4 * 1024 * 1024, maxCacheBytes: 2 * 1024 * 1024, maxCacheEntries: 64, maxPathSteps: 100000,
  maxRows: 4096, maxSqlReads: 200000, maxQueries: 2000, blockSize: 16});
export type NativeD1Limits = typeof nativeD1Limits;
export const nativeBytes = (text: string) => new TextEncoder().encode(text).length;
const compact = (value: unknown) => JSON.stringify(value);
export async function nativeSha256(raw: string): Promise<string> {
  return [...new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(raw)))].map(b => b.toString(16).padStart(2, '0')).join('');
}
export function nativeUnavailable(message: string): never {throw new HttpError(503, message);}

export class NativeD1Read {
  returned = 0; deliveredBytes = 0; queries = 0; sqlReads = 0;
  private pending: Promise<void> = Promise.resolve();
  readonly db: D1Database; readonly limits: NativeD1Limits; readonly typedSizeBudgets: boolean;
  constructor(db: D1Database, limits: NativeD1Limits, typedSizeBudgets = false) {
    this.db = db; this.limits = limits; this.typedSizeBudgets = typedSizeBudgets;
  }
  sourceSizeExceeded(message: string): never {
    if (this.typedSizeBudgets) throw new NativeBudgetExceeded(message);
    return nativeUnavailable(message);
  }
  async query<T>(input: string | (() => {sql: string; args: unknown[]}), ...bindings: unknown[]): Promise<T[]> {
    // A request may merge several endpoint streams concurrently. Serialize
    // delivery admission so they cannot each reserve the same remaining bytes.
    const previous = this.pending; let release!: () => void;
    this.pending = new Promise<void>(resolve => {release = resolve;});
    await previous;
    try {
      if (this.returned >= this.limits.maxRows) throw new NativeBudgetExceeded('native D1 returned-row budget');
      if (++this.queries > this.limits.maxQueries) throw new NativeBudgetExceeded('native D1 query budget');
      const {sql, args} = typeof input === 'string' ? {sql: input, args: bindings} : input();
      const result = await this.db.prepare(sql).bind(...args).all<T>();
      this.sqlReads += result.meta?.rows_read ?? 0;
      if (this.sqlReads > this.limits.maxSqlReads) throw new NativeBudgetExceeded('native D1 rows-read budget');
      for (const row of result.results) {
        if (++this.returned > this.limits.maxRows) throw new NativeBudgetExceeded('native D1 returned-row budget');
        for (const value of Object.values(row as Record<string, unknown>)) if (typeof value === 'string') this.deliveredBytes += nativeBytes(value);
        if (this.deliveredBytes > this.limits.maxDecodedBytes) throw new NativeBudgetExceeded('native D1 returned-byte budget');
      }
      return result.results;
    } finally {release();}
  }
  async textRows<T>(columns: readonly string[], order: readonly string[], sql: string, ...args: unknown[]): Promise<T[]> {
    // Column/order names are internal literals from the concrete plan, never
    // request fields. Both source-cell and aggregate bounds precede delivery.
    if ([...columns, ...order].some(name => !/^[a-z_]+$/.test(name)) || order.some(name => !columns.includes(name))) throw new Error('invalid native header projection');
    let allowance = 0;
    const rows = await this.query<T & {_native_valid: number; _native_bytes: number}>(() => {
      allowance = this.limits.maxDecodedBytes - this.deliveredBytes;
      const valid = columns.map(name => `typeof(${name})='text' AND length(CAST(${name} AS BLOB))<=1048576`).join(' AND ');
      const cost = columns.map(name => `coalesce(length(CAST(${name} AS BLOB)),0)`).join('+');
      return {sql: `WITH selected AS (SELECT * FROM (${sql}) LIMIT ?), framed AS (
        SELECT *,CASE WHEN ${valid} THEN 1 ELSE 0 END AS _native_valid,
        sum(${cost}) OVER (ORDER BY ${order.join(',')} ROWS UNBOUNDED PRECEDING) AS _native_bytes FROM selected)
        SELECT ${columns.map(name => `CASE WHEN _native_valid=1 AND _native_bytes<=? THEN ${name} ELSE NULL END AS ${name}`).join(',')},
        _native_valid,_native_bytes FROM framed ORDER BY ${order.join(',')}`,
        args: [...args, this.limits.maxRows - this.returned + 1, ...columns.map(() => allowance)]};
    });
    for (const row of rows) {
      if (row._native_valid !== 1) nativeUnavailable('native lens header is not bounded text');
      if (row._native_bytes > allowance) throw new NativeBudgetExceeded('native lens header delivery-byte budget');
    }
    return rows;
  }
  async metadata(key: string, maxBytes = 8 * 1024 * 1024): Promise<{raw: string; ref: NativeRef}> {
    const chunks: string[] = []; let total = 0;
    while (true) {
      // Never deliver oversized source text into the Worker before checking it.
      const page = await this.query<{part: number | null; json_chunk: string | null; json_bytes: number | null; json_valid: number; part_count: number; has_more: number}>(() => ({
        sql: `SELECT CASE WHEN typeof(part)='integer' THEN part ELSE NULL END AS part,
        CASE WHEN typeof(json_chunk)='text' AND length(CAST(json_chunk AS BLOB))<=? THEN json_chunk ELSE NULL END AS json_chunk,
        length(CAST(json_chunk AS BLOB)) AS json_bytes,CASE WHEN typeof(json_chunk)='text' THEN 1 ELSE 0 END AS json_valid,
        (SELECT count(*) FROM edge_meta same WHERE same.key=m.key AND same.part=m.part) AS part_count,
        EXISTS(SELECT 1 FROM edge_meta later WHERE later.key=m.key AND later.part>m.part) AS has_more
        FROM edge_meta m WHERE key=?${chunks.length ? ' AND part>?' : ''} ORDER BY part LIMIT 1`,
        args: [Math.min(131072, maxBytes - total, this.limits.maxDecodedBytes - this.deliveredBytes), key, ...(chunks.length ? [chunks.length - 1] : [])],
      }));
      if (!page.length) break;
      for (const row of page) {
        if (row.json_valid !== 1 || row.json_bytes === null) nativeUnavailable('native lens metadata chunk is not bounded text: ' + key);
        if (row.json_bytes > 131072 || row.json_bytes > maxBytes - total) this.sourceSizeExceeded('native lens metadata chunk is not bounded text: ' + key);
        if (row.json_bytes > this.limits.maxDecodedBytes - this.deliveredBytes && row.json_chunk === null) throw new NativeBudgetExceeded('native lens metadata delivery-byte budget');
        if (chunks.length >= 256) this.sourceSizeExceeded('native lens metadata chunks unavailable: ' + key);
        if (row.part !== chunks.length || row.part_count !== 1 || typeof row.json_chunk !== 'string') nativeUnavailable('native lens metadata chunks unavailable: ' + key);
        total += nativeBytes(row.json_chunk); if (total > maxBytes) nativeUnavailable('native lens metadata exceeds byte budget: ' + key);
        chunks.push(row.json_chunk);
      }
      if (page[0]!.has_more === 0) break;
    }
    if (!chunks.length) nativeUnavailable('native lens metadata chunks unavailable: ' + key);
    const raw = chunks.join('');
    try {return {raw, ref: parseNativeJson(raw, {maxBytes})};}
    catch {return nativeUnavailable('native lens metadata invalid: ' + key);}
  }
}

export class NativeD1Rows {
  compact = false; // Enabled only by a covered lens after exact store admission.
  decoded = 0; cacheBytes = 0;
  cache = new Map<string, {ref: NativeRef; size: number}>();
  readonly read: NativeD1Read; readonly limits: NativeD1Limits; readonly verifyOrder: boolean;
  constructor(read: NativeD1Read, limits: NativeD1Limits, verifyOrder = true) {
    this.read = read; this.limits = limits; this.verifyOrder = verifyOrder;
  }
  async load(kind: NativeKind, identifiers: Iterable<string>): Promise<Map<string, NativeRef>> {
    const ids = [...new Set(identifiers)].sort(codePointCompare), result = new Map<string, NativeRef>(), missing: string[] = [];
    for (const id of ids) {
      const key = kind + ':' + id, cached = this.cache.get(key);
      if (cached) {this.cache.delete(key); this.cache.set(key, cached); result.set(id, cached.ref);} else missing.push(id);
    }
    for (let offset = 0; offset < missing.length; offset += this.limits.blockSize) {
      const page = missing.slice(offset, offset + this.limits.blockSize);
      let allowance = 0;
      const fields = [...IDENTITY[kind], 'json', ...(this.compact ? ['source_sha256','seed_sha256'] : [])];
      const costs = fields.map(field => `coalesce(length(CAST(${field} AS BLOB)),0)`).join('+');
      const valid = ["typeof(json)='text'", ...IDENTITY[kind].map(field => `typeof(${field})='text' AND length(CAST(${field} AS BLOB))<=1048576`),
        ...(this.compact ? ['source_sha256','seed_sha256'].map(field=>`typeof(${field})='text' AND length(${field})=64`) : [])].join(' AND ');
      const permitted = valid + ' AND json_bytes<=1048576 AND delivered_bytes<=?';
      const selection = this.compact
        ? `SELECT ${IDENTITY[kind].map(key=>`n.${key} AS ${key}`).join(',')},c.json,c.source_sha256,c.seed_sha256,length(CAST(c.json AS BLOB)) AS json_bytes FROM knowledge_compact_lens c LEFT JOIN knowledge_${kind}s n ON n.id=c.id WHERE c.kind=? AND c.id IN (SELECT value FROM json_each(?)) ORDER BY c.id LIMIT ?`
        : `SELECT ${fields.join(',')},length(CAST(json AS BLOB)) AS json_bytes FROM knowledge_${kind}s WHERE id IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT ?`;
      const items = await this.read.query<Record<string, unknown>>(() => {
        allowance = Math.min(this.limits.maxDecodedBytes - this.read.deliveredBytes, this.limits.maxDecodedBytes - this.decoded);
        return {sql: `WITH selected AS (
        ${selection}), framed AS (
        SELECT *,sum(${costs}) OVER (ORDER BY id ROWS UNBOUNDED PRECEDING) AS delivered_bytes FROM selected)
        SELECT ${fields.map(field => `CASE WHEN ${permitted} THEN ${field} ELSE NULL END AS ${field}`).join(',')},
        json_bytes,CASE WHEN ${valid} THEN 1 ELSE 0 END AS json_valid,delivered_bytes FROM framed ORDER BY id`,
        args: [...(this.compact ? [kind] : []), compact(page), Math.min(page.length + 1, this.limits.maxRows - this.read.returned + 1), ...fields.map(() => allowance)]};
      });
      const orders = this.verifyOrder ? await this.read.textRows<{kind: string; id: string; sort_key: string; from_id: string; to_id: string}>(['kind','id','sort_key','from_id','to_id'], ['id'], 'SELECT kind,id,sort_key,from_id,to_id FROM knowledge_lens_order WHERE kind=? AND id IN (SELECT value FROM json_each(?)) LIMIT ?', kind, compact(page), page.length + 1) : [];
      if (items.length !== page.length || (this.verifyOrder && orders.length !== page.length)) nativeUnavailable('native lens selected payload/order closure missing');
      const byOrder = new Map(orders.map(row => [row.id, row]));
      for (const row of items) {
        if (row.json_valid !== 1 || typeof row.json_bytes !== 'number') nativeUnavailable('native lens row JSON is not bounded text');
        if (row.json_bytes > 1048576) this.read.sourceSizeExceeded('native lens row JSON is not bounded text');
        if (typeof row.delivered_bytes !== 'number' || row.delivered_bytes > allowance) throw new NativeBudgetExceeded('native lens payload delivery-byte budget');
        if (typeof row.json !== 'string') nativeUnavailable('native lens row JSON is not text');
        // Overflow is still the exact emitted native value, not a JSON.parse
        // compatibility projection. Metadata delivery enforces the same row
        // and request budgets before reconstruction and digest admission.
        const raw = row.json === '' && !this.compact
          ? (await this.read.metadata(`knowledge_${kind}_payload:${row.id}`, 1048576)).raw
          : row.json as string;
        const size = nativeBytes(raw);
        this.decoded += size;
        if (this.decoded > this.limits.maxDecodedBytes) throw new NativeBudgetExceeded('native lens decoded-byte budget');
        const expected = await this.read.metadata(`knowledge_${kind}_digest:${row.id}`, 1024);
        const rawDigest = await nativeSha256(raw);
        if (nativeKeys(expected.ref).join(',') !== 'sha256' || nativeField(expected.ref, 'sha256').value !== (this.compact ? row.source_sha256 : rawDigest)
          || (this.compact && (typeof row.source_sha256!=='string' || !/^[a-f0-9]{64}$/.test(row.source_sha256) || row.seed_sha256!==rawDigest))) nativeUnavailable('emitted knowledge row or compact seed checksum differs');
        let ref: NativeRef;
        try {ref = parseNativeJson(raw);} catch {return nativeUnavailable('native lens source row invalid');}
        if (!ref.value || typeof ref.value !== 'object' || Array.isArray(ref.value) || IDENTITY[kind].some(key => nativeField(ref, key).value !== row[key])) nativeUnavailable('knowledge row identity/index columns differ');
        const id = stringField(ref, 'id'), order = byOrder.get(id);
        if (this.verifyOrder && (!order || order.kind !== kind || order.sort_key !== nativeLower(id) || order.from_id !== (kind === 'relation' ? stringField(ref, 'from_id') : '') || order.to_id !== (kind === 'relation' ? stringField(ref, 'to_id') : ''))) nativeUnavailable('native lens ordered carrier differs from payload');
        result.set(id, ref);
        if (size <= this.limits.maxCacheBytes) {
          while (this.cache.size && (this.cache.size >= this.limits.maxCacheEntries || this.cacheBytes + size > this.limits.maxCacheBytes)) {
            const [key, retired] = this.cache.entries().next().value!; this.cache.delete(key); this.cacheBytes -= retired.size;
          }
          this.cache.set(kind + ':' + id, {ref, size}); this.cacheBytes += size;
        }
      }
    }
    return result;
  }
  async get(kind: NativeKind, id: string): Promise<NativeRef> {return (await this.load(kind, [id])).get(id)!;}
}

/** Python compact emitted framing, including UTF-8 encodability of every key/value. */
export function requireEmittedHeader(raw: string, ref: NativeRef): void {
  const pending = [ref];
  while (pending.length) {
    const value = pending.pop()!;
    if (typeof value.value === 'string' && !value.value.isWellFormed()) nativeUnavailable('prepared header contains an invalid Unicode string');
    if (typeof value.value === 'number' && nativeNumberInfo(value).lexeme !== pythonStr(value)) nativeUnavailable('prepared header is not in its declared emitted JSON framing');
    if (value.value && typeof value.value === 'object') for (const key of nativeKeys(value)) {
      if (!key.isWellFormed()) nativeUnavailable('prepared header contains an invalid Unicode key');
      pending.push(nativeChild(value,key));
    }
  }
  if (nativePacketJson(ref,{maxBytes:65536}) !== raw) nativeUnavailable('prepared header is not in its declared emitted JSON framing');
}

/** Validate the common owner header without loading catalog or lens histograms. */
export async function readNativePublication(read: NativeD1Read, expectedRevision?: string, mode: 'lens' | 'inspection' = 'lens'): Promise<{raw: string; ref: NativeRef}> {
  const top = await read.metadata('knowledge_reader_top', 65536);
  const version = nativeField(top.ref, 'schema').value;
  const lens = version === 'tos_published_knowledge_reader_v2';
  const topKeys = ['schema','read_model_schema','source_revision','data_revision','graph_schema','normalization_binding','catalog_sha256','row_integrity','authority_boundary', ...(lens ? ['lens_sha256'] : [])];
  if ([...nativeKeys(top.ref)].sort().join(',') !== topKeys.sort().join(',') || nativeField(top.ref, 'graph_schema').value !== 'tos_knowledge_graph_v1'
    || ['source_revision','data_revision','catalog_sha256', ...(lens ? ['lens_sha256'] : [])].some(key => typeof nativeField(top.ref, key).value !== 'string' || !/^[a-f0-9]{64}$/.test(nativeField(top.ref, key).value as string))) nativeUnavailable('native lens publication header invalid');
  const normalization = nativeField(top.ref, 'normalization_binding');
  const normalizationKeys = ['schema','processor_digest','entity_registry_digest','relation_registry_digest','configuration_digest'];
  if (!normalization.value || typeof normalization.value !== 'object' || Array.isArray(normalization.value)
    || [...nativeKeys(normalization)].sort().join(',') !== normalizationKeys.sort().join(',') || nativeField(normalization, 'schema').value !== 'tos_knowledge_graph_normalization_binding_v1'
    || normalizationKeys.filter(key => key !== 'schema').some(key => typeof nativeField(normalization, key).value !== 'string' || !/^[a-f0-9]{64}$/.test(nativeField(normalization, key).value as string))) nativeUnavailable('native lens normalization binding invalid');
  if (nativeField(top.ref, 'authority_boundary.source_owner').value !== 'Tree-of-Sophia' || ['is_source','is_canon','writes_to_tree'].some(key => nativeField(top.ref, 'authority_boundary.' + key).value !== false)) nativeUnavailable('native lens source authority boundary invalid');
  if (expectedRevision !== undefined && nativeField(top.ref, 'data_revision').value !== expectedRevision) nativeUnavailable('native lens publication header differs from serving revision');
  const model = nativeField(top.ref, 'read_model_schema').value;
  if (nativeField(top.ref, 'row_integrity').value !== 'sha256-emitted-json-v1'
    || (mode === 'lens' ? (!lens || model !== 'tos_cloudflare_edge_read_model_v9')
      : (!['tos_published_knowledge_reader_v1','tos_published_knowledge_reader_v2'].includes(String(version))
        || !['tos_cloudflare_edge_read_model_v8','tos_cloudflare_edge_read_model_v9'].includes(String(model))
        || (model === 'tos_cloudflare_edge_read_model_v9' && !lens)))) nativeUnavailable('native publication metadata version unavailable');
  requireEmittedHeader(top.raw, top.ref);
  return top;
}
