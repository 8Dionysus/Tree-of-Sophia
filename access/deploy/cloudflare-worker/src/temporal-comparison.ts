/** Compare exact source date envelopes, without parsing or historical judgment. */
import { HttpError, stringArray, type Item } from './common.ts';
import { KnowledgeRevisionConflict } from './lens-pagination.ts';

type Selection = { node_id: string; content_revision: string };
export type TemporalRequest = {
  schema_version: 'tos_temporal_comparison_request_v1'; source_revision: string;
  left: Selection; right: Selection;
};
type Status = 'comparable' | 'undetermined' | 'unsupported';
type Operand = { claim: Item; value: Item | null; normalized_time: Item | null };
type CheckedOperand = { packet: Operand; status: Status; issues: string[] };
type Lookup = (identifier: string) => Promise<Item[]>;
type Reason = { side: 'left' | 'right' | 'pair'; code: string };

function object(value: unknown): Item | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value) ? value as Item : null;
}
function fields(value: unknown): Item { return object(value) ?? {}; }
function requireOperandContainers(node: Item): void {
  if (['attributes', 'semantics', 'type_mapping'].some(key => !object(node[key]))) {
    throw new HttpError(503, 'selected normalized carrier has invalid structural containers');
  }
  const semantics = fields(node.semantics);
  if (['claim', 'time'].some(key => Object.hasOwn(semantics, key) && !object(semantics[key]))) {
    throw new HttpError(503, 'selected normalized carrier has invalid structural containers');
  }
}
function exactKeys(value: Item, keys: string[]): boolean {
  return Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}
function revision(value: unknown): value is string {
  return typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
}

export function normalizeTemporalComparisonRequest(value: unknown): TemporalRequest {
  const request = object(value);
  if (!request || !exactKeys(request, ['schema_version', 'source_revision', 'left', 'right'])) {
    throw new HttpError(400, 'temporal comparison requires schema_version, source_revision, left and right only');
  }
  if (request.schema_version !== 'tos_temporal_comparison_request_v1') throw new HttpError(400, 'unsupported temporal comparison request schema');
  if (!revision(request.source_revision)) throw new HttpError(400, 'source_revision must be an exact knowledge snapshot revision');
  function selection(side: 'left' | 'right'): Selection {
    const ref = object(request![side]);
    if (!ref || !exactKeys(ref, ['node_id', 'content_revision'])) throw new HttpError(400, `${side} requires node_id and content_revision only`);
    const id = ref.node_id;
    // Count Unicode code points, as the public schema/Python reader do.
    if (typeof id !== 'string' || [...id].length < 1 || [...id].length > 1024 || id.trim() !== id) {
      throw new HttpError(400, `${side}.node_id must be a nonempty exact normalized ID of at most 1024 characters`);
    }
    if (!revision(ref.content_revision)) throw new HttpError(400, `${side}.content_revision must be an exact carrier revision`);
    return { node_id: id, content_revision: ref.content_revision };
  }
  return { schema_version: 'tos_temporal_comparison_request_v1', source_revision: request.source_revision,
    left: selection('left'), right: selection('right') };
}

function sameJson(left: unknown, right: unknown): boolean {
  if (typeof left !== typeof right) return false;
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left) && Array.isArray(right) && left.length === right.length && left.every((item, index) => sameJson(item, right[index]));
  }
  const a = object(left), b = object(right);
  if (a || b) return Boolean(a && b && exactKeys(a, Object.keys(b)) && Object.keys(a).every(key => sameJson(a[key], b[key])));
  return left === right;
}

async function operand(ref: Selection, lookup: Lookup): Promise<CheckedOperand> {
  const matches = await lookup(ref.node_id);
  if (matches.length !== 1) throw new HttpError(404, `expected one exact knowledge node: ${ref.node_id}`);
  const claim = matches[0]!;
  if (claim.content_revision !== ref.content_revision) throw new KnowledgeRevisionConflict('selected Claim content changed; select again from the current snapshot');
  requireOperandContainers(claim);
  const packet: Operand = { claim: structuredClone(claim), value: null, normalized_time: null };
  const stop = (status: Status, issues: string[]): CheckedOperand => ({ packet, status, issues });
  if (claim.source_graph !== 'source-claims' || claim.kind_id !== 'claim' || claim.type_id !== 'tos.entity.claim'
      || fields(claim.type_mapping).status !== 'mapped') return stop('unsupported', ['selected-node-is-not-a-source-claim']);
  const semantics = fields(fields(claim.semantics).claim);
  const source = object(fields(claim.attributes).source_claim);
  if (!source || typeof source.claim_id !== 'string' || source.claim_id !== semantics.claim_id
      || !Number.isSafeInteger(source.claim_version) || Number(source.claim_version) < 1
      || !Number.isSafeInteger(semantics.claim_version) || source.claim_version !== semantics.claim_version
      || source.predicate !== semantics.source_predicate_id || semantics.predicate_mapping_status !== 'mapped') {
    return stop('undetermined', ['claim-source-binding-inconsistent']);
  }
  if (typeof semantics.object_node_id !== 'string') return stop('undetermined', ['claim-object-binding-unavailable']);
  const values = await lookup(semantics.object_node_id);
  if (values.length !== 1) return stop('undetermined', ['claim-object-unavailable-or-ambiguous']);
  const value = values[0]!;
  requireOperandContainers(value);
  packet.value = structuredClone(value);
  if (value.source_graph !== 'source-claims' || value.type_id !== 'tos.entity.temporal-assertion'
      || fields(value.type_mapping).status !== 'mapped') return stop('unsupported', ['claim-object-is-not-a-declared-temporal-assertion']);
  const attributes = fields(value.attributes);
  if (attributes.claim_ref !== source.claim_id || !Object.hasOwn(attributes, 'value') || !sameJson(attributes.value, source.object)) {
    return stop('undetermined', ['temporal-object-source-binding-inconsistent']);
  }
  const time = object(fields(value.semantics).time);
  if (!time) return stop('undetermined', ['temporal-normalization-unavailable']);
  packet.normalized_time = structuredClone(time);
  if (!Object.hasOwn(time, 'raw') || !sameJson(time.raw, attributes.value)) return stop('undetermined', ['temporal-normalization-source-binding-inconsistent']);
  const rawIssues = time.issues ?? [];
  if ((Object.hasOwn(time, 'issues') && time.issues === null) || !Array.isArray(rawIssues)
      || rawIssues.some(issue => typeof issue !== 'string' || !issue)) return stop('unsupported', ['temporal-normalization-issues-invalid']);
  const issues = stringArray(rawIssues);
  if (typeof time.kind !== 'string' || !['date-assertion', 'interval-assertion', 'relative-order', 'unknown-date'].includes(time.kind)) {
    return stop('unsupported', ['unsupported-temporal-normalization-kind']);
  }
  const unsupported = issues.filter(issue => issue.startsWith('conflicting-') || ['reversed-interval', 'invalid-date-parts', 'unparsed-date-value'].includes(issue));
  if (time.calendar !== undefined && time.calendar !== null && (typeof time.calendar !== 'string'
      || !['gregorian', 'proleptic-gregorian'].includes(time.calendar))) unsupported.push('unsupported-declared-calendar');
  if (time.declared_year_numbering !== undefined && time.declared_year_numbering !== null && time.declared_year_numbering !== 'astronomical') unsupported.push('unsupported-declared-year-numbering');
  if (unsupported.length) return stop('unsupported', [...new Set([...issues, ...unsupported])]);
  if (!['date-assertion', 'interval-assertion'].includes(time.kind)) issues.push('no-absolute-date-bounds');
  if (time.calendar === undefined || time.calendar === null) issues.push('declared-calendar-unavailable');
  if (time.declared_year_numbering === undefined || time.declared_year_numbering === null) issues.push('declared-year-numbering-unavailable');
  if (typeof time.precision === 'string' && ['approximate', 'uncertain', 'unknown'].includes(time.precision)) issues.push('non-exact-date-precision');
  if (time.certainty !== 'exact') issues.push('explicit-exact-certainty-unavailable');
  if (time.comparison_calendar !== 'proleptic-gregorian') issues.push('comparison-calendar-unavailable');
  if (time.year_numbering !== 'astronomical') issues.push('comparison-year-numbering-unavailable');
  if (!Number.isSafeInteger(time.sort_start) || !Number.isSafeInteger(time.sort_end)) issues.push('absolute-date-envelope-unavailable');
  else if (Number(time.sort_start) > Number(time.sort_end)) issues.push('reversed-date-envelope');
  return stop(issues.length ? 'undetermined' : 'comparable', [...new Set(issues)]);
}

function envelopeRelation(left: Item, right: Item): string {
  const a = Number(left.sort_start), b = Number(left.sort_end), c = Number(right.sort_start), d = Number(right.sort_end);
  if (b < c) return 'before';
  if (d < a) return 'after';
  if (a === c && b === d) return 'equal';
  if (a <= c && b >= d) return 'contains';
  if (c <= a && d >= b) return 'contained-by';
  return 'overlaps';
}

export async function compareTemporalOperands(sourceRevision: unknown, requestValue: unknown, lookup: Lookup): Promise<Item> {
  const request = normalizeTemporalComparisonRequest(requestValue);
  if (request.source_revision !== sourceRevision) throw new KnowledgeRevisionConflict('knowledge snapshot changed; select both Claims again');
  const left = await operand(request.left, lookup), right = await operand(request.right, lookup);
  const statuses = [left.status, right.status];
  let status: Status = statuses.includes('unsupported') ? 'unsupported' : statuses.includes('undetermined') ? 'undetermined' : 'comparable';
  const reasons: Reason[] = [...left.issues.map(code => ({ side: 'left' as const, code })), ...right.issues.map(code => ({ side: 'right' as const, code }))];
  let relation: string | null = null;
  if (status === 'comparable') {
    const leftTime = left.packet.normalized_time!, rightTime = right.packet.normalized_time!;
    if (typeof leftTime.role !== 'string' || !leftTime.role || typeof rightTime.role !== 'string' || !rightTime.role) {
      status = 'undetermined'; reasons.push({ side: 'pair', code: 'time-role-unavailable' });
    } else if (leftTime.role !== rightTime.role) {
      status = 'unsupported'; reasons.push({ side: 'pair', code: 'different-time-roles' });
    } else if (leftTime.role !== 'historical-time') {
      status = 'unsupported'; reasons.push({ side: 'pair', code: 'unsupported-time-role' });
    } else relation = envelopeRelation(leftTime, rightTime);
  }
  const refs = [...new Set([left.packet.claim, left.packet.value, right.packet.claim, right.packet.value]
    .flatMap(item => stringArray(item?.source_refs)))].sort();
  return { schema_version: 'tos_temporal_comparison_result_v1', source_revision: sourceRevision, request,
    comparison: { status, relation, reasons, basis: 'normalized-source-date-envelopes' },
    left: left.packet, right: right.packet, source_refs: refs,
    authority_boundary: { is_source: false, writes_to_tree: false, performs_assessment: false,
      creates_inferred_claim: false, comparison_basis: 'normalized-source-date-envelopes',
      note: 'Relations describe date envelopes only. They do not establish event simultaneity, duration, causality, identity or the truth/admission of either Claim. Numeric keys are ordering keys, not timestamps or elapsed-time quantities.' } };
}
