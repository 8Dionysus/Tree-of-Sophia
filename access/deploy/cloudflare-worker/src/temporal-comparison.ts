/** Compare exact source date envelopes, without parsing or historical judgment. */
import { HttpError, stringArray, type Item } from './common.ts';
import { KnowledgeRevisionConflict } from './lens-pagination.ts';
import {NativeBudgetExceeded, nativeNumberInfo, pythonEquals, pythonMember, codePointCompare} from '../../../shared/native-semantics.ts';
import {nativeStrip} from '../../../shared/native-unicode.ts';
import {nativeChild, nativeField, nativeKeys, parseNativeJson, nativePacketObject, derived,
  type NativeRef, type NativePacket} from './native-lens.ts';
import {canonicalNativeJson} from './native-source-target.ts';

type Selection = { node_id: string; content_revision: string };
export type TemporalRequest = {
  schema_version: 'tos_temporal_comparison_request_v1'; source_revision: string;
  left: Selection; right: Selection;
};
type Status = 'comparable' | 'undetermined' | 'unsupported';
type Operand = { claim: NativeRef; value: NativeRef | null; normalized_time: NativeRef | null };
type CheckedOperand = { packet: Operand; status: Status; issues: string[] };
type Lookup = (identifier: string) => Promise<NativeRef[]>;
type Reason = { side: 'left' | 'right' | 'pair'; code: string };
const MAX_SOURCE_BYTES = 262144;

export function temporalNodeFromJson(text: string): NativeRef {
  try {
    const ref = parseNativeJson(text);
    if (!object(ref.value)) throw new Error('selected normalized carrier is not an object');
    return ref;
  } catch {throw new HttpError(503, 'selected normalized carrier is invalid');}
}

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
    if (typeof id !== 'string' || [...id].length < 1 || [...id].length > 1024 || nativeStrip(id) !== id) {
      throw new HttpError(400, `${side}.node_id must be a nonempty exact normalized ID of at most 1024 characters`);
    }
    if (!revision(ref.content_revision)) throw new HttpError(400, `${side}.content_revision must be an exact carrier revision`);
    // Python dict(ref) retains the accepted selection's member order.
    return Object.keys(ref)[0] === 'node_id'
      ? {node_id:id,content_revision:ref.content_revision}
      : {content_revision:ref.content_revision,node_id:id};
  }
  return { schema_version: 'tos_temporal_comparison_request_v1', source_revision: request.source_revision,
    left: selection('left'), right: selection('right') };
}

/** Temporal JSON equality deliberately excludes Python's bool/int alias. */
function sameJson(left: NativeRef, right: NativeRef): boolean {
  let visits = 300000;
  function equal(a: NativeRef, b: NativeRef): boolean {
    if (--visits < 0) throw new NativeBudgetExceeded('temporal equality visit budget');
    if (typeof a.value === 'boolean' || typeof b.value === 'boolean') return a.value === b.value;
    if (typeof a.value === 'number' && typeof b.value === 'number') return pythonEquals(a,b);
    if (typeof a.value !== typeof b.value || (a.value === null) !== (b.value === null)) return false;
    if (!a.value || typeof a.value !== 'object') return a.value === b.value;
    if (Array.isArray(a.value) !== Array.isArray(b.value)) return false;
    const keys = nativeKeys(a), other = new Set(nativeKeys(b));
    return keys.length === other.size && keys.every(key => other.has(key) && equal(nativeChild(a,key),nativeChild(b,key)));
  }
  return equal(left,right);
}

function safeInteger(ref: NativeRef): boolean {
  return typeof ref.value === 'number' && Number.isSafeInteger(ref.value);
}

async function hashText(text: string): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text));
  return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('');
}

async function documentCatalogueBinding(claimRef: NativeRef, valueRef: NativeRef, lookup: Lookup): Promise<string | null> {
  const claim = fields(claimRef.value), value = fields(valueRef.value);
  const sourceRef = nativeField(claimRef,'attributes.source_claim'), source = fields(sourceRef.value);
  const semanticsRef = nativeField(claimRef,'semantics.claim'), semantics = fields(semanticsRef.value);
  const timeRef = nativeField(valueRef,'semantics.time'), time = fields(timeRef.value);
  const profile = fields(semantics.source_claim_profile), schemas = profile.schemas;
  const attributionRef = nativeField(sourceRef,'qualifiers.catalogue_attribution');
  const attribution = fields(attributionRef.value), rawRef = nativeField(sourceRef,'object'), raw = fields(rawRef.value);
  if (source.predicate !== 'document_catalogue_date' || source.schema_version !== 'tos_document_catalogue_claim_v1'
      || source.assertion_layer !== 'bibliographic_assertion' || semantics.relation_type_id !== 'tos.relation.document-catalogue-date'
      || profile.reader !== 'document-catalogue-temporal-v1'
      || !sameJson(nativeField(semanticsRef,'source_claim_profile.assertion_layers'),parseNativeJson('["bibliographic_assertion"]'))
      || !Array.isArray(schemas) || schemas.length !== 1 || !object(schemas[0])
      || schemas[0].schema_version !== source.schema_version || schemas[0].schema_ref !== 'ToS/contracts/document-catalogue-claim.schema.json'
      || raw.role !== 'catalogue-assigned-document-date' || time.role !== raw.role
      || typeof raw.kind !== 'string' || !['date-assertion', 'interval-assertion', 'unknown-date'].includes(raw.kind)
      || attribution.field_role !== 'assigned-date' || typeof attribution.source_field !== 'string' || !nativeStrip(attribution.source_field)
      || !Array.isArray(source.evidence_refs) || !pythonMember(nativeField(attributionRef,'evidence_ref'),nativeField(sourceRef,'evidence_refs'))
      || !sameJson(nativeField(attributionRef,'source_wording'),nativeField(rawRef,'source_wording'))) return 'document-catalogue-profile-binding-inconsistent';
  const subjects = typeof semantics.subject_node_id === 'string' ? await lookup(semantics.subject_node_id) : [];
  const subject = subjects.length === 1 ? fields(subjects[0]!.value) : {};
  if (subjects.length !== 1 || subject.entity_id !== source.subject_ref || subject.source_graph !== 'source-claims'
      || fields(subject.type_mapping).status !== 'mapped'
      || !stringArray(fields(subject.semantics).type_ancestors).includes('tos.entity.document')) {
    return 'document-catalogue-subject-binding-inconsistent';
  }
  const inconsistent = 'document-catalogue-exact-source-binding-inconsistent';
  const sourceText = canonicalNativeJson(sourceRef), rawText = canonicalNativeJson(rawRef);
  if (new TextEncoder().encode(sourceText).length > MAX_SOURCE_BYTES || semantics.source_canonical_json !== sourceText) return inconsistent;
  const digest = await hashText(sourceText), valueDigest = await hashText(rawText);
  const literalDigest = await hashText('{"claim_ref":' + canonicalNativeJson(nativeField(sourceRef,'claim_id')) + ',"value":' + rawText + '}');
  const left = fields(claim.attributes), right = fields(value.attributes);
  const sourceLine = nativeField(claimRef,'attributes.source_line');
  if (left.source_sha256 !== digest || right.source_sha256 !== digest || right.value_sha256 !== valueDigest
      || await hashText(canonicalNativeJson(nativeField(valueRef,'attributes.value'))) !== valueDigest
      || await hashText(canonicalNativeJson(nativeField(timeRef,'raw'))) !== valueDigest
      || value.native_id !== 'literal:sha256:' + literalDigest
      || typeof sourceLine.value !== 'number' || nativeNumberInfo(sourceLine).kind !== 'int'
      || BigInt(nativeNumberInfo(sourceLine).lexeme) < 1n
      || !pythonEquals(sourceLine,nativeField(valueRef,'attributes.source_line'))
      || !pythonEquals(nativeField(claimRef,'source_refs'),nativeField(valueRef,'source_refs'))) return inconsistent;
  return null;
}

async function operand(ref: Selection, lookup: Lookup): Promise<CheckedOperand> {
  const matches = await lookup(ref.node_id);
  if (matches.length !== 1) throw new HttpError(404, `expected one exact knowledge node: ${ref.node_id}`);
  const claimRef = matches[0]!, claim = fields(claimRef.value);
  if (claim.content_revision !== ref.content_revision) throw new KnowledgeRevisionConflict('selected Claim content changed; select again from the current snapshot');
  requireOperandContainers(claim);
  const packet: Operand = { claim: claimRef, value: null, normalized_time: null };
  const stop = (status: Status, issues: string[]): CheckedOperand => ({ packet, status, issues });
  if (claim.source_graph !== 'source-claims' || claim.kind_id !== 'claim' || claim.type_id !== 'tos.entity.claim'
      || fields(claim.type_mapping).status !== 'mapped') return stop('unsupported', ['selected-node-is-not-a-source-claim']);
  const semantics = fields(fields(claim.semantics).claim);
  const sourceRef = nativeField(claimRef,'attributes.source_claim');
  const source = object(sourceRef.value);
  if (!source || typeof source.claim_id !== 'string' || source.claim_id !== semantics.claim_id
      || !safeInteger(nativeField(sourceRef,'claim_version')) || Number(source.claim_version) < 1
      || !safeInteger(nativeField(claimRef,'semantics.claim.claim_version')) || source.claim_version !== semantics.claim_version
      || !pythonEquals(nativeField(sourceRef,'predicate'),nativeField(claimRef,'semantics.claim.source_predicate_id')) || semantics.predicate_mapping_status !== 'mapped') {
    return stop('undetermined', ['claim-source-binding-inconsistent']);
  }
  if (typeof semantics.object_node_id !== 'string') return stop('undetermined', ['claim-object-binding-unavailable']);
  const values = await lookup(semantics.object_node_id);
  if (values.length !== 1) return stop('undetermined', ['claim-object-unavailable-or-ambiguous']);
  const valueRef = values[0]!, value = fields(valueRef.value);
  requireOperandContainers(value);
  packet.value = valueRef;
  if (value.source_graph !== 'source-claims' || value.type_id !== 'tos.entity.temporal-assertion'
      || fields(value.type_mapping).status !== 'mapped') return stop('unsupported', ['claim-object-is-not-a-declared-temporal-assertion']);
  const attributes = fields(value.attributes);
  const documentary = source.predicate === 'document_catalogue_date' || source.schema_version === 'tos_document_catalogue_claim_v1'
    || fields(fields(value.semantics).time).role === 'catalogue-assigned-document-date';
  if (attributes.claim_ref !== source.claim_id || !Object.hasOwn(attributes, 'value') || (!documentary && !sameJson(nativeField(valueRef,'attributes.value'),nativeField(sourceRef,'object')))) {
    return stop('undetermined', ['temporal-object-source-binding-inconsistent']);
  }
  const time = object(fields(value.semantics).time);
  if (!time) return stop('undetermined', ['temporal-normalization-unavailable']);
  packet.normalized_time = nativeField(valueRef,'semantics.time');
  if (!Object.hasOwn(time, 'raw') || (!documentary && !sameJson(nativeField(valueRef,'semantics.time.raw'),nativeField(valueRef,'attributes.value')))) return stop('undetermined', ['temporal-normalization-source-binding-inconsistent']);
  if (documentary) {
    const issue = await documentCatalogueBinding(claimRef, valueRef, lookup);
    if (issue) return stop('undetermined', [issue]);
  }
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

export async function compareTemporalOperands(sourceRevision: unknown, requestValue: unknown, lookup: Lookup): Promise<NativePacket> {
  const request = normalizeTemporalComparisonRequest(requestValue);
  if (request.source_revision !== sourceRevision) throw new KnowledgeRevisionConflict('knowledge snapshot changed; select both Claims again');
  const left = await operand(request.left, lookup), right = await operand(request.right, lookup);
  const statuses = [left.status, right.status];
  let status: Status = statuses.includes('unsupported') ? 'unsupported' : statuses.includes('undetermined') ? 'undetermined' : 'comparable';
  const reasons: Reason[] = [...left.issues.map(code => ({ side: 'left' as const, code })), ...right.issues.map(code => ({ side: 'right' as const, code }))];
  let relation: string | null = null;
  if (status === 'comparable') {
    const leftTime = fields(left.packet.normalized_time!.value), rightTime = fields(right.packet.normalized_time!.value);
    if (typeof leftTime.role !== 'string' || !leftTime.role || typeof rightTime.role !== 'string' || !rightTime.role) {
      status = 'undetermined'; reasons.push({ side: 'pair', code: 'time-role-unavailable' });
    } else if (leftTime.role !== rightTime.role) {
      status = 'unsupported'; reasons.push({ side: 'pair', code: 'different-time-roles' });
    } else if (!['historical-time', 'catalogue-assigned-document-date'].includes(leftTime.role)) {
      status = 'unsupported'; reasons.push({ side: 'pair', code: 'unsupported-time-role' });
    } else relation = envelopeRelation(leftTime, rightTime);
  }
  const refs = [...new Set([left.packet.claim, left.packet.value, right.packet.claim, right.packet.value]
    .flatMap(item => {
      const values = item ? fields(item.value).source_refs : undefined;
      return Array.isArray(values) ? values.filter((value): value is string => typeof value === 'string') : [];
    }))].sort(codePointCompare);
  // Python's final ensure_ascii=False UTF-8 response rejects escaped lone
  // surrogates too. Validate only returned source carriers, not an unreturned
  // documentary subject's unrelated fields. No source values are rewritten.
  const seen = new WeakSet<object>();
  function validUtf8(ref: NativeRef): void {
    if (typeof ref.value === 'string' && !ref.value.isWellFormed()) throw new HttpError(503,'prepared response contains invalid JSON values');
    if (!ref.value || typeof ref.value !== 'object' || seen.has(ref.value)) return;
    seen.add(ref.value);
    for (const key of nativeKeys(ref)) {
      if (!key.isWellFormed()) throw new HttpError(503,'prepared response contains invalid JSON values');
      validUtf8(nativeChild(ref,key));
    }
  }
  for (const value of [left.packet.claim,left.packet.value,right.packet.claim,right.packet.value]) if (value) validUtf8(value);
  const packet = (operand: Operand) => nativePacketObject([
    ['claim',operand.claim],['value',operand.value],['normalized_time',operand.normalized_time],
  ]);
  return nativePacketObject([
    ['schema_version','tos_temporal_comparison_result_v1'], ['source_revision',request.source_revision], ['request',derived(request)],
    ['comparison',derived({status,relation,reasons,basis:'normalized-source-date-envelopes'})],
    ['left',packet(left.packet)], ['right',packet(right.packet)], ['source_refs',derived(refs)],
    ['authority_boundary',derived({is_source:false,writes_to_tree:false,performs_assessment:false,
      creates_inferred_claim:false,comparison_basis:'normalized-source-date-envelopes',
      note:'Relations describe date envelopes only. They do not establish event simultaneity, duration, causality, identity or the truth/admission of either Claim. Numeric keys are ordering keys, not timestamps or elapsed-time quantities.'})],
  ]);
}
