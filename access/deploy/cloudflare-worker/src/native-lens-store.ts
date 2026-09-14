/** Bounded native-v7 plan over owner-published v9 D1 rows.
 * SQL selects identities/dimensions/incidence; NativeRefs own general semantics.
 */
import {HttpError} from './common.ts';
import {OVERVIEW_EXCLUDED_PREDICATES, OVERVIEW_EXCLUDED_RELATION_TYPES, type Inclusion, type QueryProperty, type SortRule} from './knowledge.ts';
import {nativeLower, nativeUnicodeVersion, nativeSortKey, codePointCompare, NativeBudgetExceeded} from '../../../shared/native-semantics.ts';
import {arrayRefs, compileNativeSpec, derived, nativeMatchesGroup, nativeChild, nativeField, nativeKeys,
  nativePacketJson, parseNativeJson, stringField, type NativeFilter, type NativeRef, type NativeSpec, type NativeGroup, type NativeLensResult} from './native-lens.ts';
import {finalizeNativeLens} from './native-lens-result.ts';

import {NativeD1Read as Read, NativeD1Rows, nativeD1Limits as nativeLensLimits, nativeBytes as bytes, nativeSha256 as sha256, nativeUnavailable as unavailable, readNativePublication, type NativeD1Limits as Limits} from './native-d1-read.ts';
export {nativeD1Limits as nativeLensLimits} from './native-d1-read.ts';
const compact = (value: unknown) => JSON.stringify(value);
type Kind = 'node' | 'relation';
type Header = {id: string; from_id: string; to_id: string; sort_key: string; key?: string[]};
type Cell = [string, string, string, number];
type LensMetadata = {node_counts: Cell[]; relation_counts: Cell[]; query_properties: QueryProperty[]};
const DIMENSIONS = {node: ['source_graph', 'kind_id', 'type_id'], relation: ['source_graph', 'predicate_id', 'relation_type_id']} as const;
type IdentityField = 'id' | 'entity_id' | 'native_id';
type IdentitySelector = {field: IdentityField; values: string[]; index: string};
type IdentityPlan = {mode: 'all' | 'any'; selectors: IdentitySelector[]};
const IDENTITY_INDEXES: Record<Kind, Partial<Record<IdentityField, string>>> = {
  node: {id: 'sqlite_autoindex_knowledge_nodes_1', entity_id: 'knowledge_nodes_identity_seek', native_id: 'knowledge_nodes_native_idx'},
  relation: {id: 'sqlite_autoindex_knowledge_relations_1', native_id: 'knowledge_relations_native_idx'},
};

const INDEXES = ['knowledge_lens_order_sort', 'knowledge_lens_order_from', 'knowledge_lens_order_to', 'knowledge_lens_order_pair',
  'knowledge_nodes_source_kind_idx', 'knowledge_relations_source_predicate_idx'];

class Plan extends NativeD1Rows {
  callbacks = 0; candidates = 0; sortBytes = 0; pathSteps = 0;
  matchedNodes = 0; matchedRelations = 0; genericRelations: Header[] | null = null;
  sources: Set<string>;
  readonly metadata: LensMetadata; readonly spec: NativeSpec;
  constructor(read: Read, metadata: LensMetadata, spec: NativeSpec, limits: Limits) {
    super(read, limits); this.metadata = metadata; this.spec = spec; this.sources = new Set(spec.sources);
  }
  callback<T>(fn: () => T): T {if (++this.callbacks > this.limits.maxCallbacks) throw new NativeBudgetExceeded('native lens callback budget'); return fn();}
  group(ref: NativeRef, group: NativeGroup): boolean {return this.callback(() => nativeMatchesGroup(ref, group));}
  scopeCells(kind: Kind): Cell[] {return this.metadata[kind === 'node' ? 'node_counts' : 'relation_counts'].filter(cell => this.sources.has(cell[0]));}
  dimensional(kind: Kind, group: NativeGroup): boolean {return group.filters.every(rule => !rule._property_binding && (DIMENSIONS[kind] as readonly string[]).includes(rule.field ?? ''));}
  regime(item: {predicate_id: unknown; relation_type_id: unknown}): boolean {
    const traversal = this.spec.traversal;
    return (!traversal.predicate_ids.length || traversal.predicate_ids.includes(item.predicate_id as string))
      && (traversal.profile !== 'overview' || (!OVERVIEW_EXCLUDED_PREDICATES.includes(item.predicate_id as string) && !OVERVIEW_EXCLUDED_RELATION_TYPES.includes(item.relation_type_id as string)));
  }
  allowedCells(kind: Kind, group: NativeGroup): Cell[] {
    if (!group.enabled) return [];
    return this.scopeCells(kind).filter(cell => {
      const value = Object.fromEntries(DIMENSIONS[kind].map((key, i) => [key, cell[i]]));
      return this.group(parseNativeJson(compact(value)), group) && (kind === 'node' || this.regime(value as {predicate_id: string; relation_type_id: string}));
    });
  }
  scope(alias: string): {sql: string; args: unknown[]} {return {sql: `${alias}.source_graph IN (SELECT value FROM json_each(?))`, args: [compact(this.spec.sources)]};}
  where(kind: Kind, alias: string): {sql: string; args: unknown[]} {
    const group = kind === 'node' ? this.spec.node_query : this.spec.relation_query;
    if (!group.enabled) return {sql: '0', args: []};
    let {sql, args} = this.scope(alias);
    if (this.dimensional(kind, group) && group.filters.length) {
      const cells = this.allowedCells(kind, group).map(c => c.slice(0, 3));
      sql += ' AND EXISTS (SELECT 1 FROM json_each(?) cell WHERE ' + DIMENSIONS[kind].map((field, i) => `${alias}.${field}=json_extract(cell.value,'$[${i}]')`).join(' AND ') + ')';
      args.push(compact(cells));
    }
    if (kind === 'relation') {
      if (this.spec.traversal.predicate_ids.length) {sql += ` AND ${alias}.predicate_id IN (SELECT value FROM json_each(?))`; args.push(compact(this.spec.traversal.predicate_ids));}
      if (this.spec.traversal.profile === 'overview') {sql += ` AND ${alias}.predicate_id NOT IN (SELECT value FROM json_each(?)) AND ${alias}.relation_type_id NOT IN (SELECT value FROM json_each(?))`; args.push(compact(OVERVIEW_EXCLUDED_PREDICATES), compact(OVERVIEW_EXCLUDED_RELATION_TYPES));}
    }
    return {sql, args};
  }
  identitySelector(kind: Kind, rule: NativeFilter): IdentitySelector | null {
    const field = rule.field as IdentityField | undefined;
    if (!field || !Object.hasOwn(IDENTITY_INDEXES[kind], field)) return null;
    const index = IDENTITY_INDEXES[kind][field];
    if (rule._property_binding || !index || (rule.op !== 'eq' && rule.op !== 'in')) return null;
    // Candidate columns are emitted as text, while the native matcher keeps
    // Python's exact value/type semantics over the decoded source packet.
    // Refuse non-string inputs here: otherwise SQLite affinity/NULL handling
    // could discard a row that the native predicate must inspect.
    const raw = rule.valueRef.value;
    if (rule.op === 'eq' && typeof raw !== 'string') return null;
    const values = Array.isArray(raw) ? arrayRefs(rule.valueRef).map(ref => ref.value) : [raw];
    if (values.some(value => typeof value !== 'string')) return null;
    return {field, values: [...new Set(values as string[])].sort(codePointCompare), index};
  }
  identityPlan(kind: Kind, group: NativeGroup): IdentityPlan | null {
    if (!group.filters.length) return null;
    const selectors = group.filters.map(rule => this.identitySelector(kind, rule));
    if (group.match === 'all') {
      const positive = selectors.filter((selector): selector is IdentitySelector => selector !== null);
      return positive.length ? {mode: 'all', selectors: positive} : null;
    }
    return selectors.every((selector): selector is IdentitySelector => selector !== null)
      ? {mode: 'any', selectors}
      : null;
  }
  seedPlan(kind: Kind, seeds: string[]): IdentityPlan | null {
    if (kind !== 'node' || !seeds.length) return null;
    const values = [...new Set(seeds)].sort(codePointCompare);
    return {mode: 'any', selectors: (['id', 'native_id', 'entity_id'] as IdentityField[]).map(field => ({
      field, values, index: IDENTITY_INDEXES.node[field]!,
    }))};
  }
  identityCondition(alias: string, plan: IdentityPlan): {sql: string; args: unknown[]} {
    const parts: string[] = [], args: unknown[] = [];
    for (const selector of plan.selectors) {
      if (!selector.values.length) parts.push('0');
      else {parts.push(`${alias}.${selector.field} IN (SELECT value FROM json_each(?))`); args.push(compact(selector.values));}
    }
    if (!parts.length) return {sql: '0', args: []};
    return {sql: parts.join(plan.mode === 'all' ? ' AND ' : ' OR '), args};
  }
  async identityBranchScan(kind: Kind, plan: IdentityPlan, after: string): Promise<{id: string}[]> {
    const alias = kind === 'node' ? 'n' : 'r', scope = this.scope(alias), branches: string[] = [], args: unknown[] = [];
    for (const selector of plan.selectors) {
      if (!selector.values.length) continue;
      branches.push(`SELECT id FROM (SELECT ${alias}.id FROM knowledge_${kind}s ${alias} INDEXED BY ${selector.index}
        WHERE ${alias}.${selector.field} IN (SELECT value FROM json_each(?)) AND ${alias}.id>? AND ${scope.sql}
        ORDER BY ${alias}.id LIMIT ?)`);
      args.push(compact(selector.values), after, ...scope.args, this.limits.blockSize);
    }
    if (!branches.length) return [];
    return this.read.textRows<{id: string}>(['id'], ['id'], 'WITH candidates AS (' + branches.join(' UNION ') + ') SELECT id FROM candidates ORDER BY id LIMIT ?', ...args, this.limits.blockSize);
  }
  async focus(): Promise<NativeRef | null> {
    const requested = this.spec.seed.focus_node_id;
    if (requested === null) return null;
    const scope = this.scope('n');
    for (const field of ['id', 'entity_id', 'native_id']) {
      const order = field === 'entity_id' ? "CASE n.source_graph WHEN 'source-navigation' THEN 0 WHEN 'canon' THEN 1 WHEN 'source-claims' THEN 2 WHEN 'philosophy' THEN 3 WHEN 'candidate-intake' THEN 4 WHEN 'repository' THEN 5 WHEN 'semantic-interchange' THEN 6 ELSE 99 END,n.id" : 'n.id';
      const rows = await this.read.textRows<{id: string}>(['id'], ['id'], `SELECT n.id FROM knowledge_nodes n WHERE ${scope.sql} AND n.${field}=? ORDER BY ${order} LIMIT ?`, ...scope.args, requested, field === 'native_id' ? 2 : 1);
      if (rows.length > 1) throw new HttpError(400, 'ambiguous ToS knowledge focus; use a namespaced node id');
      if (rows.length) return this.get('node', rows[0]!.id);
    }
    throw new HttpError(400, 'unknown ToS knowledge focus: ' + requested);
  }
  async *scan(kind: Kind, seeds: string[] = []): AsyncGenerator<NativeRef> {
    let after = ''; const alias = kind === 'node' ? 'n' : 'r', scope = this.scope(alias), block = this.limits.blockSize;
    const group = kind === 'node' ? this.spec.node_query : this.spec.relation_query;
    const groupPlan = this.identityPlan(kind, group), seedPlan = this.seedPlan(kind, seeds);
    while (true) {
      let rows: {id: string}[];
      if (seedPlan && !groupPlan) rows = await this.identityBranchScan(kind, seedPlan, after);
      else if (groupPlan && !seedPlan && groupPlan.mode === 'any') rows = await this.identityBranchScan(kind, groupPlan, after);
      else {
        const conditions: string[] = [], conditionArgs: unknown[] = [];
        if (seedPlan) {const condition = this.identityCondition(alias, seedPlan); conditions.push('(' + condition.sql + ')'); conditionArgs.push(...condition.args);}
        if (groupPlan) {const condition = this.identityCondition(alias, groupPlan); conditions.push('(' + condition.sql + ')'); conditionArgs.push(...condition.args);}
        // ORDER BY id otherwise makes SQLite walk the global identity index
        // before applying source scope on every page. The published source
        // index keeps generic candidate work inside the selected corpus.
        const index = groupPlan && !seedPlan && groupPlan.mode === 'all' ? groupPlan.selectors[0]!.index
          : kind === 'node' ? 'knowledge_nodes_source_kind_idx' : 'knowledge_relations_source_predicate_idx';
        const indexed = ` INDEXED BY ${index}`;
        rows = await this.read.textRows(['id'], ['id'], `SELECT ${alias}.id FROM knowledge_${kind}s ${alias}${indexed} WHERE ${scope.sql}${conditions.length ? ' AND ' + conditions.join(' AND ') : ''} AND ${alias}.id>? ORDER BY ${alias}.id LIMIT ?`, ...scope.args, ...conditionArgs, after, block);
      }
      if (!rows.length) return;
      const loaded = await this.load(kind, rows.map(row => row.id));
      for (const row of rows) {if (++this.candidates > this.limits.maxCandidates) throw new NativeBudgetExceeded('native lens candidate budget'); yield loaded.get(row.id)!;}
      after = rows.at(-1)!.id;
    }
  }
  sortKey(ref: NativeRef, rules: SortRule[]): string[] {
    const key = rules.map(rule => this.callback(() => nativeSortKey(nativeField(ref, rule.field))));
    this.sortBytes += key.reduce((sum, value) => sum + bytes(value), 0);
    if (this.sortBytes > this.limits.maxSortBytes) throw new NativeBudgetExceeded('native lens sort-key byte budget');
    return [...key, stringField(ref, 'id')];
  }
  compareKeys(a: string[], b: string[], rules: SortRule[]): number {
    for (let i = 0; i < a.length; i++) {const delta = codePointCompare(a[i]!, b[i]!); if (delta) return rules[i]?.direction === 'desc' ? -delta : delta;}
    return 0;
  }
  async *ordered(kind: Kind, where = this.where(kind, kind === 'node' ? 'n' : 'r'), endpoint?: ['from' | 'to', string]): AsyncGenerator<Header> {
    const alias = kind === 'node' ? 'n' : 'r', index = endpoint ? 'knowledge_lens_order_' + endpoint[0] : 'knowledge_lens_order_sort';
    let after = ['', ''], first = true;
    while (true) {
      const endpoints = kind === 'relation' ? `${alias}.from_id,${alias}.to_id` : "'' AS from_id,'' AS to_id";
      const rows = await this.read.textRows<Header & {order_from: string; order_to: string}>(['id','sort_key','from_id','to_id','order_from','order_to'], ['sort_key','id'], `SELECT l.id,l.sort_key,${endpoints},l.from_id AS order_from,l.to_id AS order_to FROM knowledge_lens_order l INDEXED BY ${index} CROSS JOIN knowledge_${kind}s ${alias} ON ${alias}.id=l.id WHERE l.kind=?${endpoint ? ` AND l.${endpoint[0]}_id=?` : ''} AND (l.sort_key,l.id)>(?,?) AND ${where.sql} ORDER BY l.sort_key,l.id LIMIT ?`, kind, ...(endpoint ? [endpoint[1]] : []), ...after, ...where.args, first && endpoint ? 1 : this.limits.blockSize);
      if (!rows.length) return;
      for (const row of rows) {if (row.sort_key !== nativeLower(row.id) || row.from_id !== row.order_from || row.to_id !== row.order_to) unavailable('native lens ordered key or endpoints differ'); yield row;}
      after = [rows.at(-1)!.sort_key, rows.at(-1)!.id]; first = false;
    }
  }
  async selectNodes(focus: NativeRef | null): Promise<{nodes: NativeRef[]; proofs: Map<string, unknown[]>}> {
    const spec = this.spec, group = spec.node_query, proofs = new Map<string, unknown[]>();
    if (!group.enabled) return {nodes: [], proofs};
    const simple = this.dimensional('node', group) && !spec.seed.node_ids.length && !spec.seed.text_query && !spec.path_query.length;
    if (simple && defaultSort(spec.composition.sort_nodes)) {
      this.matchedNodes = this.allowedCells('node', group).reduce((sum, cell) => sum + cell[3], 0);
      const ids: string[] = [];
      if (this.matchedNodes) for await (const row of this.ordered('node')) {ids.push(row.id); if (ids.length >= spec.limits.nodes) break;}
      if (ids.length !== Math.min(this.matchedNodes, spec.limits.nodes)) unavailable('native lens selector/count/order closure missing');
      if (focus && this.group(focus, group)) proofs.set(stringField(focus, 'id'), []);
      const loaded = await this.load('node', ids); return {nodes: ids.map(id => loaded.get(id)!), proofs};
    }
    const top: {ref: NativeRef; key: string[]; proof: unknown[]}[] = [];
    for await (const node of this.scan('node', spec.seed.node_ids)) {
      if (spec.seed.text_query) {
        // Producer search_text is native json.dumps(sort_keys=True).lower(),
        // but use the source-owned row here rather than SQL ASCII folding.
        const text = this.callback(() => nativeSearchable(node));
        if (!text.includes(nativeLower(spec.seed.text_query))) continue;
      }
      if (!this.group(node, group)) continue;
      const proof: unknown[] = []; let matched = true;
      for (const condition of spec.path_query) {
        const witness = await this.pathWitness(stringField(node, 'id'), condition);
        if ((witness !== null) !== (condition.quantifier === 'exists')) {matched = false; break;}
        proof.push(witness ?? {path_id: condition.path_id, absence_in_scope: true});
      }
      if (!matched) continue;
      this.matchedNodes++;
      const id = stringField(node, 'id'); if (focus && id === stringField(focus, 'id')) proofs.set(id, proof);
      const entry = {ref: node, key: this.sortKey(node, spec.composition.sort_nodes), proof};
      // Bounded sorted top-k, never sort or retain the complete candidate set.
      let lo = 0, hi = top.length;
      while (lo < hi) {const middle = (lo + hi) >>> 1; if (this.compareKeys(top[middle]!.key, entry.key, spec.composition.sort_nodes) <= 0) lo = middle + 1; else hi = middle;}
      if (lo < spec.limits.nodes) {top.splice(lo, 0, entry); if (top.length > spec.limits.nodes) top.pop();}
    }
    for (const entry of top) proofs.set(stringField(entry.ref, 'id'), entry.proof);
    return {nodes: top.map(entry => entry.ref), proofs};
  }
  async prepareRelations(): Promise<void> {
    const group = this.spec.relation_query;
    if (!group.enabled) {this.genericRelations = []; return;}
    if (this.dimensional('relation', group) && defaultSort(this.spec.composition.sort_relations)) {
      this.matchedRelations = this.allowedCells('relation', group).reduce((sum, cell) => sum + cell[3], 0); return;
    }
    const headers: Header[] = [];
    for await (const ref of this.scan('relation')) if (this.regime({predicate_id: nativeField(ref, 'predicate_id').value, relation_type_id: nativeField(ref, 'relation_type_id').value}) && this.group(ref, group)) {
      headers.push({id: stringField(ref, 'id'), from_id: stringField(ref, 'from_id'), to_id: stringField(ref, 'to_id'), sort_key: '', key: this.sortKey(ref, this.spec.composition.sort_relations)});
    }
    headers.sort((a, b) => this.compareKeys(a.key!, b.key!, this.spec.composition.sort_relations)); this.genericRelations = headers; this.matchedRelations = headers.length;
  }
  async *relations(frontier?: Set<string>, direction: 'incoming' | 'outgoing' | 'either' = 'either'): AsyncGenerator<Header> {
    if (this.genericRelations !== null) {for (const row of this.genericRelations) if (!frontier || (direction !== 'incoming' && frontier.has(row.from_id)) || (direction !== 'outgoing' && frontier.has(row.to_id))) yield row; return;}
    if (!frontier) {yield* this.ordered('relation'); return;}
    const streams: AsyncGenerator<Header>[] = [];
    for (const id of [...frontier].sort(codePointCompare)) for (const side of ['from', 'to'] as const) if ((side === 'from' && direction !== 'incoming') || (side === 'to' && direction !== 'outgoing')) streams.push(this.ordered('relation', undefined, [side, id]));
    // Only one head per bounded endpoint stream. No full local-edge materialization.
    const heads = await Promise.all(streams.map(stream => stream.next())); let previous: string | null = null;
    while (true) {
      let best = -1;
      for (let i = 0; i < heads.length; i++) if (!heads[i]!.done && (best < 0 || headerCompare(heads[i]!.value!, heads[best]!.value!) < 0)) best = i;
      if (best < 0) return;
      const row = heads[best]!.value!; if (row.id !== previous) {previous = row.id; yield row;}
      heads[best] = await streams[best]!.next();
    }
  }
  async nodeSources(ids: Iterable<string>): Promise<Map<string, string>> {
    const list = [...new Set(ids)]; if (!list.length) return new Map();
    return new Map((await this.read.textRows<{id: string; source_graph: string}>(['id','source_graph'], ['id'], 'SELECT id,source_graph FROM knowledge_nodes WHERE id IN (SELECT value FROM json_each(?)) LIMIT ?', compact(list), list.length + 1)).map(row => [row.id, row.source_graph]));
  }
  async aliases(frontier: string[], selected: Set<string>, remaining: number): Promise<{ids: string[]; limited: boolean; origins: Map<string, string>}> {
    const origins = new Map<string, string>();
    for (const [id, node] of [...await this.load('node', frontier)].sort(([a], [b]) => codePointCompare(a, b))) {const entity = nativeField(node, 'entity_id').value; if (typeof entity === 'string' && entity.startsWith('tos.') && !origins.has(entity)) origins.set(entity, id);}
    if (!origins.size) return {ids: [], limited: false, origins};
    const scope = this.scope('n');
    const rows = await this.read.textRows<{id: string}>(['id'], ['id'], 'SELECT n.id FROM knowledge_nodes n INDEXED BY knowledge_nodes_identity_seek WHERE n.entity_id IN (SELECT value FROM json_each(?)) AND ' + scope.sql + ' AND n.id NOT IN (SELECT value FROM json_each(?)) ORDER BY n.id LIMIT ?', compact([...origins.keys()]), ...scope.args, compact([...selected]), remaining + 1);
    return {ids: rows.slice(0, remaining).map(row => row.id), limited: rows.length > remaining, origins};
  }
  async pathWitness(start: string, condition: NativeSpec['path_query'][number]): Promise<Record<string, unknown> | null> {
    const adjacent = async function* (plan: Plan, id: string): AsyncGenerator<NativeRef> {
      let after = '';
      while (true) {
        const packets = await Promise.all((['from', 'to'] as const).map(side => plan.read.textRows<{id: string}>(['id'], ['id'], `SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_${side}_seek WHERE ${side}_id=? AND id>? ORDER BY id LIMIT ?`, id, after, plan.limits.blockSize)));
        const ids = [...new Set(packets.flatMap(rows => rows.map(row => row.id)))].sort(codePointCompare).slice(0, plan.limits.blockSize);
        if (!ids.length) return;
        const loaded = await plan.load('relation', ids);
        for (const id of ids) {const ref = loaded.get(id)!; if (plan.sources.has(stringField(ref, 'source_graph'))) yield ref;}
        after = ids.at(-1)!;
      }
    };
    const walk = async (current: string, depth: number, nodes: string[], relations: string[]): Promise<Record<string, unknown> | null> => {
      if (depth === condition.steps.length) return {path_id: condition.path_id, node_ids: nodes, relation_ids: relations};
      const step = condition.steps[depth]!;
      for await (const relation of adjacent(this, current)) {
        if (++this.pathSteps > this.limits.maxPathSteps) throw new NativeBudgetExceeded('native lens path work budget');
        if (!step.relation_query.enabled || !this.group(relation, step.relation_query)) continue;
        const header = {from_id: stringField(relation, 'from_id'), to_id: stringField(relation, 'to_id')};
        for (const neighbor of neighbors(header, current, step.direction)) {
          if (!step.node_query.enabled || !this.sources.has((await this.nodeSources([neighbor])).get(neighbor) ?? '')) continue;
          if (!this.group(await this.get('node', neighbor), step.node_query)) continue;
          // Fixed-length native walks may revisit nodes. Budget exhaustion is
          // an error even for not_exists, never a manufactured absence proof.
          const found = await walk(neighbor, depth + 1, [...nodes, neighbor], [...relations, stringField(relation, 'id')]);
          if (found !== null) return found;
        }
      }
      return null;
    };
    return walk(start, 0, [start], []);
  }
  async eligible(basis: Set<string>, traversed: Set<string>): Promise<{count: number; stream: AsyncIterable<Header>}> {
    const policy = this.spec.composition.endpoint_policy;
    if (this.genericRelations !== null) {
      const items = this.genericRelations.filter(row => policy === 'independent' || traversed.has(row.id) || (policy === 'both' && basis.has(row.from_id) && basis.has(row.to_id)) || (policy === 'either' && (basis.has(row.from_id) || basis.has(row.to_id))));
      return {count: items.length, stream: (async function* () {yield* items;})()};
    }
    if (policy === 'independent') return {count: this.matchedRelations, stream: this.relations()};
    const where = this.where('relation', 'r'), basisJson = compact([...basis]), traversedJson = compact([...traversed]);
    const ids = policy === 'both'
      ? "SELECT l.id FROM json_each(?) a CROSS JOIN json_each(?) b CROSS JOIN knowledge_lens_order l INDEXED BY knowledge_lens_order_pair WHERE l.kind='relation' AND l.from_id=a.value AND l.to_id=b.value UNION SELECT value AS id FROM json_each(?)"
      : 'SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek WHERE from_id IN (SELECT value FROM json_each(?)) UNION SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_seek WHERE to_id IN (SELECT value FROM json_each(?)) UNION SELECT value AS id FROM json_each(?)';
    const args = [basisJson, basisJson, traversedJson];
    const count = (await this.read.query<{total: number}>(`WITH eligible AS (${ids}) SELECT count(*) AS total FROM eligible e CROSS JOIN knowledge_relations r ON r.id=e.id WHERE ${where.sql}`, ...args, ...where.args))[0]!.total;
    const plan = this;
    const stream = (async function* () {
      let after = ['', ''];
      while (true) {
        const rows = await plan.read.textRows<Header & {order_from: string; order_to: string}>(['id','from_id','to_id','sort_key','order_from','order_to'], ['sort_key','id'], `WITH eligible AS (${ids}) SELECT r.id,r.from_id,r.to_id,l.sort_key,l.from_id AS order_from,l.to_id AS order_to FROM eligible e CROSS JOIN knowledge_lens_order l INDEXED BY sqlite_autoindex_knowledge_lens_order_1 ON l.kind='relation' AND l.id=e.id CROSS JOIN knowledge_relations r ON r.id=e.id WHERE ${where.sql} AND (l.sort_key,l.id)>(?,?) ORDER BY l.sort_key,l.id LIMIT ?`, ...args, ...where.args, ...after, plan.limits.blockSize);
        if (!rows.length) return;
        for (const row of rows) {if (row.sort_key !== nativeLower(row.id) || row.from_id !== row.order_from || row.to_id !== row.order_to) unavailable('native lens eligible ordering or endpoints differ'); yield row;}
        after = [rows.at(-1)!.sort_key, rows.at(-1)!.id];
      }
    })();
    return {count, stream};
  }
}
function defaultSort(rules: SortRule[]): boolean {return rules.length === 1 && rules[0]!.field === 'id' && rules[0]!.direction === 'asc';}
function headerCompare(a: Header, b: Header): number {return codePointCompare(a.sort_key, b.sort_key) || codePointCompare(a.id, b.id);}
function neighbors(row: {from_id: string; to_id: string}, node: string, direction: string): string[] {
  return [...(direction !== 'incoming' && row.from_id === node ? [row.to_id] : []), ...(direction !== 'outgoing' && row.to_id === node ? [row.from_id] : [])].filter(Boolean);
}
function nativeSearchable(ref: NativeRef): string {
  const render = (ref: NativeRef): string => {
    if (Array.isArray(ref.value)) return '[' + arrayRefs(ref).map(render).join(', ') + ']';
    if (ref.value && typeof ref.value === 'object') return '{' + [...nativeKeys(ref)].sort(codePointCompare).map(key => JSON.stringify(key) + ': ' + render(nativeChild(ref, key))).join(', ') + '}';
    return nativePacketJson(ref);
  };
  return nativeLower(render(ref));
}

export async function executeNativeLensD1(db: D1Database, input: NativeRef, overrides: Partial<Limits> = {}, expectedRevision?: string): Promise<NativeLensResult> {
  const limits = {...nativeLensLimits, ...overrides};
  if (Object.values(limits).some(value => !Number.isSafeInteger(value) || value < 1) || limits.blockSize > 64) throw new Error('invalid native lens budgets');
  const read = new Read(db, limits, true), top = await readNativePublication(read, expectedRevision);
  const sourceRevision = stringField(top.ref, 'source_revision'), metadata = await read.metadata('knowledge_lens_top', 1048576);
  if (nativeField(top.ref, 'lens_sha256').value !== await sha256(metadata.raw)) unavailable('native lens metadata checksum differs');
  const lensKeys = ['schema','execution_version','source_revision','sort_key','unicode_version','query_properties','node_counts','relation_counts'];
  if ([...nativeKeys(metadata.ref)].sort().join(',') !== lensKeys.sort().join(',')) unavailable('native lens metadata framing invalid');
  if (nativeField(metadata.ref, 'schema').value !== 'tos_published_lens_metadata_v1' || nativeField(metadata.ref, 'execution_version').value !== 'tos-lens-execution-v7' || nativeField(metadata.ref, 'source_revision').value !== sourceRevision || nativeField(metadata.ref, 'sort_key').value !== 'python-str-or-empty-lower-v1' || nativeField(metadata.ref, 'unicode_version').value !== nativeUnicodeVersion) unavailable('native lens metadata incompatible');
  for (const name of ['node_counts', 'relation_counts']) {
    const refs = arrayRefs(nativeField(metadata.ref, name)); let previous: string[] | null = null;
    if (refs.length > 16384) unavailable('native lens histogram too large');
    for (const ref of refs) {
      const cell = arrayRefs(ref), key = cell.slice(0, 3).map(r => r.value as string), count = cell[3];
      if (cell.length !== 4 || key.some(k => typeof k !== 'string' || !k) || !count || typeof count.value !== 'number' || !Number.isSafeInteger(count.value) || count.value < 1 || /[.eE]/.test(nativePacketJson(count))) unavailable('native lens histogram cell invalid');
      if (previous) {let delta = 0; for (let i = 0; i < 3 && !delta; i++) delta = codePointCompare(previous[i]!, key[i]!); if (delta >= 0) unavailable('native lens histogram cells duplicated or unordered');}
      previous = key;
    }
  }
  const indexes = await read.textRows<{name: string}>(['name'], ['name'], "SELECT name FROM sqlite_master WHERE type='index' AND name IN (SELECT value FROM json_each(?))", compact(INDEXES));
  if (new Set(indexes.map(row => row.name)).size !== INDEXES.length) unavailable('native lens ordered-index migration unavailable');
  const definitions = nativeField(metadata.ref, 'query_properties').value;
  if (!Array.isArray(definitions) || definitions.length > 4096) unavailable('native lens query property metadata invalid');
  if (definitions.some(definition => !definition || typeof definition !== 'object' || Array.isArray(definition)
    || ['property_id','field','value_type'].some(key => typeof definition[key] !== 'string' || !definition[key]) || typeof definition.inherited !== 'boolean'
    || ['applies_to','operators'].some(key => !Array.isArray(definition[key]) || definition[key].some((value: unknown) => typeof value !== 'string' || !value)))) unavailable('native lens query property framing invalid');
  const compiled = compileNativeSpec(input, definitions as QueryProperty[]), plan = new Plan(read, metadata.ref.value as LensMetadata, compiled.spec, limits), spec = plan.spec;
  const focus = await plan.focus(), selection = await plan.selectNodes(focus);
  const selected = new Map<string, NativeRef>(), inclusion: Inclusion = {nodes: Object.create(null), relations: Object.create(null), authority: 'query-execution-not-semantic-proof'};
  if (focus) {selected.set(stringField(focus, 'id'), focus); inclusion.nodes[stringField(focus, 'id')] = {kind: 'focus'};}
  for (const node of selection.nodes) {if (selected.size >= spec.limits.nodes) break; const id = stringField(node, 'id'); if (!selected.has(id)) {selected.set(id, node); inclusion.nodes[id] = {kind: 'selector', path_witnesses: selection.proofs.get(id) ?? []};}}
  const matchedNodes = plan.matchedNodes + Number(focus !== null && !selection.proofs.has(stringField(focus, 'id')));
  await plan.prepareRelations();
  let frontier = [...selected.keys()], identityLimited = false; const traversed = new Set<string>();
  for (let depth = 0; depth < spec.traversal.depth; depth++) {
    if (spec.traversal.profile === 'overview') {
      const aliases = await plan.aliases(frontier, new Set(selected.keys()), spec.limits.nodes - selected.size); identityLimited ||= aliases.limited;
      for (const [id, node] of await plan.load('node', aliases.ids)) {selected.set(id, node); const entity = stringField(node, 'entity_id'); inclusion.nodes[id] = {kind: 'identity-carrier', via_node_id: aliases.origins.get(entity)!, entity_id: entity, depth}; frontier.push(id);}
    }
    const next: string[] = [], current = new Set(frontier);
    for await (const relation of plan.relations(current, spec.traversal.direction)) {
      if (selected.size >= spec.limits.nodes && traversed.size >= spec.limits.relations) break;
      let touched = false;
      for (const origin of frontier) if (origin === relation.from_id || origin === relation.to_id) for (const neighbor of neighbors(relation, origin, spec.traversal.direction)) {
        touched = true;
        if (!selected.has(neighbor) && selected.size < spec.limits.nodes && plan.sources.has((await plan.nodeSources([neighbor])).get(neighbor) ?? '')) {selected.set(neighbor, await plan.get('node', neighbor)); inclusion.nodes[neighbor] = {kind: 'traversal', via_node_id: origin, via_relation_id: relation.id, depth: depth + 1}; next.push(neighbor);}
      }
      if (touched && traversed.size < spec.limits.relations) traversed.add(relation.id);
    }
    frontier = [...new Set(next)]; if (!frontier.length) break;
  }
  const eligible = await plan.eligible(new Set(selected.keys()), traversed), relationIds: string[] = [];
  let examined = 0, exhausted = true;
  for await (const relation of eligible.stream) {
    if (relationIds.length >= spec.limits.relations) {exhausted = false; break;}
    examined++;
    const missing = [...new Set([relation.from_id, relation.to_id].filter(id => !selected.has(id)))];
    if (selected.size + missing.length > spec.limits.nodes) continue;
    const allowed = await plan.nodeSources(missing);
    for (const id of missing) if (plan.sources.has(allowed.get(id) ?? '')) {selected.set(id, await plan.get('node', id)); inclusion.nodes[id] = {kind: 'endpoint', via_relation_id: relation.id};}
    if (selected.has(relation.from_id) && selected.has(relation.to_id)) {relationIds.push(relation.id); inclusion.relations[relation.id] = {kind: traversed.has(relation.id) ? 'traversal' : 'endpoint-policy', endpoint_policy: spec.composition.endpoint_policy};}
  }
  if (exhausted && examined !== eligible.count) unavailable('native lens eligible/count/order closure incomplete');
  const relations = await plan.load('relation', relationIds);
  return finalizeNativeLens(nativeField(top.ref, 'authority_boundary'), sourceRevision, spec, compiled.publicPacket, [...selected.values()], relationIds.map(id => relations.get(id)!), {
    available_nodes: plan.scopeCells('node').reduce((sum, cell) => sum + cell[3], 0), available_relations: plan.scopeCells('relation').reduce((sum, cell) => sum + cell[3], 0),
    matched_nodes: matchedNodes, matched_relations: plan.matchedRelations, eligible_relations: eligible.count, identity_expansion_limited: identityLimited,
  }, focus, inclusion);
}
