/** Bounded source-form delivery, not generation, assessment or admission. */
type Item = Record<string, unknown>;
type ExactRef = {id: string; version: number; digest: string};
type RoleSelection = {state: string; reason: string; form: ExactRef | null; packet: Item | null};
type Candidate = {form: ExactRef; role: unknown; language: unknown; state: string; source_pointer: string};

export const HUMAN_FORM_ROLES = ['name', 'caption', 'hover', 'statement', 'grounds', 'history', 'technical'];
export const HUMAN_FORM_SELECTION_BUDGET = 16_384;
const LANGUAGE = /^(?:[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*|[iIxX](?:-[A-Za-z0-9]{1,8})+)$(?![\s\S])/;
const STATES = ['ready', 'invalid', 'unavailable', 'stale', 'restricted', 'needs-assessment', 'over-budget'];

function record(value: unknown): Item {
  return value !== null && typeof value === 'object' && !Array.isArray(value) ? value as Item : {};
}

function exactRef(value: unknown): value is ExactRef {
  const ref = record(value);
  return Object.keys(ref).sort().join(',') === 'digest,id,version'
    && typeof ref.id === 'string' && ref.id.length > 0
    && typeof ref.version === 'number' && Number.isSafeInteger(ref.version) && ref.version >= 1
    && typeof ref.digest === 'string' && /^sha256:[a-f0-9]{64}$(?![\s\S])/.test(ref.digest);
}

function sameRef(value: unknown, expected: ExactRef): boolean {
  return exactRef(value) && value.id === expected.id && value.version === expected.version && value.digest === expected.digest;
}

function jsonIdentity(value: unknown): string {
  if (Array.isArray(value)) return '[' + value.map(jsonIdentity).join(',') + ']';
  if (value !== null && typeof value === 'object') {
    return '{' + Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
      .map(([key, member]) => JSON.stringify(key) + ':' + jsonIdentity(member)).join(',') + '}';
  }
  return JSON.stringify(value);
}

function languageContextValid(packet: Item): boolean {
  if (!Object.hasOwn(packet, 'language_context')) return true;
  const context = record(packet.language_context), binding = record(context.binding), value = record(context.value);
  const validBinding = (raw: unknown) => {
    const item = record(raw);
    return Object.keys(item).sort().join(',') === 'pointer,record' && exactRef(item.record)
      && typeof item.pointer === 'string' && /^(?:\/(?:[^~/]|~[01])*)*$(?![\s\S])/.test(item.pointer);
  };
  const entries = packet.context as Item[];
  const dependency = (ref: unknown) => Array.isArray(packet.dependencies)
    && packet.dependencies.some(item => exactRef(item) && jsonIdentity(item) === jsonIdentity(ref));
  if (Object.keys(context).sort().join(',') !== 'binding,value' || !validBinding(binding)
    || !dependency(binding.record)
    || !['language', 'script', 'relation', 'source'].every(key => Object.hasOwn(value, key))
    || value.language !== packet.language || value.script !== packet.script
    || (value.script !== null && (typeof value.script !== 'string' || !/^[A-Za-z]{4}$(?![\s\S])/.test(value.script)))
    || typeof value.relation !== 'string' || !['unknown', 'original', 'translation', 'transliteration', 'adaptation'].includes(value.relation)
    || !entries.some(entry => jsonIdentity(entry.binding) === jsonIdentity(binding) && jsonIdentity(entry.value) === jsonIdentity(value))) return false;
  if (value.relation === 'original' || value.relation === 'unknown') return value.source === null;
  return validBinding(value.source) && dependency(record(value.source).record) && entries.some(entry => jsonIdentity(entry.binding) === jsonIdentity(value.source)
    && typeof entry.value === 'string' && entry.value.trim().length > 0);
}

function assessmentSnapshotValid(packet: Item): boolean {
  if (!Object.hasOwn(packet, 'assessment_snapshot')) return true;
  const snapshot = record(packet.assessment_snapshot);
  if (typeof snapshot.owner_snapshot !== 'string' || !/^sha256:[a-f0-9]{64}$(?![\s\S])/.test(snapshot.owner_snapshot)
    || typeof snapshot.journal_batches !== 'number' || !Number.isSafeInteger(snapshot.journal_batches) || snapshot.journal_batches < 0
    || snapshot.publication_authorized !== false || snapshot.current_runtime_grant !== false
    || !Object.hasOwn(snapshot, 'journal_revision')) return false;
  const revision = snapshot.journal_revision;
  if ((revision === null) !== (snapshot.journal_batches === 0)
    || (revision !== null && (typeof revision !== 'string' || !/^[a-f0-9]{64}$(?![\s\S])/.test(revision)))) return false;
  if (packet.state !== 'ready') return true;
  const admission = record(packet.admission);
  return snapshot.journal_batches > 0 && packet.derivation === 'freeform' && admission.schema_version === 'tos_knowledge_admission_v1'
    && exactRef(packet.form) && sameRef(admission.subject, packet.form) && exactRef(admission.policy)
    && typeof admission.status === 'string' && ['admitted', 'admitted-with-limits'].includes(admission.status)
    && admission.can_use === true && admission.is_semantic_evaluation === false
    && typeof admission.use === 'string' && admission.use.length > 0;
}

export function formDeliveryCost(value: unknown): number {
  if (typeof value === 'string') return new TextEncoder().encode(JSON.stringify(value)).length;
  if (value === null || typeof value === 'boolean') return 5;
  if (typeof value === 'number') return Math.max(32, String(value).length);
  if (Array.isArray(value)) return 2 + value.reduce((sum, member) => sum + 1 + formDeliveryCost(member), 0);
  if (typeof value === 'object' && value !== null) {
    return 2 + Object.entries(value).reduce((sum, [key, member]) => sum + 2 + formDeliveryCost(key) + formDeliveryCost(member), 0);
  }
  throw new Error('human form contains a non-JSON value');
}

export function selectHumanForms(item: Item, language = 'auto') {
  if (typeof language !== 'string' || language.length > 128 || (!['auto', 'original'].includes(language) && !LANGUAGE.test(language))) {
    throw new Error('invalid human form language preference');
  }
  if (typeof item.content_revision !== 'string' || !/^[a-f0-9]{64}$(?![\s\S])/.test(item.content_revision)) {
    throw new Error('human forms require a content revision');
  }
  const attributes = record(item.attributes), forms = Object.hasOwn(attributes, 'human_forms') ? attributes.human_forms : [];
  const empty = (): RoleSelection => ({state: 'missing', reason: 'no-ready-form', form: null, packet: null});
  const result = {schema_version: 'tos_human_form_selection_v1', content_revision: item.content_revision,
    requested_language: language, source_ref: typeof attributes.human_forms_source_ref === 'string'
      && attributes.human_forms_source_ref.length <= 2048 ? attributes.human_forms_source_ref : null,
    state: 'available', roles: Object.fromEntries(HUMAN_FORM_ROLES.map(role => [role, empty()])),
    candidates: [] as Candidate[], issues: [] as string[], performs_translation: false, performs_assessment: false};
  const stop = (state: string, issue: string) => {
    const packet = {...result, state, roles: Object.fromEntries(HUMAN_FORM_ROLES.map(role => [role, empty()])),
      source_ref: state === 'over-budget' ? null : result.source_ref, candidates: [], issues: [issue]};
    if (formDeliveryCost(packet) > HUMAN_FORM_SELECTION_BUDGET) packet.source_ref = null;
    return packet;
  };
  if (!Array.isArray(forms) || forms.length > 32) return stop('invalid', 'forms.invalid-or-excessive-collection');
  if (!forms.length) return formDeliveryCost(result) <= HUMAN_FORM_SELECTION_BUDGET ? result : stop('over-budget', 'forms.inspect-collection-separately');
  if (attributes.source_record != null && attributes.source_claim != null) {
    return stop('invalid', 'forms.ambiguous-source-record-binding');
  }
  const isClaim = attributes.source_record == null && attributes.source_claim !== null
    && typeof attributes.source_claim === 'object' && !Array.isArray(attributes.source_claim);
  const rawSource = isClaim ? attributes.source_claim : attributes.source_record;
  if (rawSource === null || typeof rawSource !== 'object' || Array.isArray(rawSource)) {
    return stop('invalid', 'forms.missing-source-record-binding');
  }
  const source = record(rawSource);
  const nativeIdentities: Record<string, string> = {
    tos_scholarly_composite_witness_v1: 'composite_id',
    tos_artifact_source_witness_v1: 'artifact_id',
    tos_artifact_source_witness_v2: 'artifact_id',
  };
  const nativeIdentity = !isClaim && typeof source.schema_version === 'string' && Object.hasOwn(nativeIdentities, source.schema_version)
    ? nativeIdentities[source.schema_version] : undefined;
  if (nativeIdentity && (Object.hasOwn(source, 'record_id') || typeof source[nativeIdentity] !== 'string'
      || !(source[nativeIdentity] as string).startsWith('tos.' + nativeIdentity.replace(/_id$/, '') + '.'))) {
    return stop('invalid', 'forms.invalid-source-record-binding');
  }
  const subject = {id: source[isClaim ? 'claim_id' : nativeIdentity ?? 'record_id'], version: source[isClaim ? 'claim_version' : 'record_version'],
    digest: 'sha256:' + String(attributes.source_sha256 ?? '')};
  if (!exactRef(subject) || ((isClaim || nativeIdentity) && item.entity_id !== subject.id)) return stop('invalid', 'forms.invalid-source-record-binding');
  const ready: Item[] = [], seen = new Set<string>();
  for (const [index, raw] of forms.entries()) {
    const packet = record(raw);
    if (packet.schema_version !== 'tos_human_form_materialization_v1' || !exactRef(packet.form)
      || !sameRef(packet.subject, subject) || packet.performs_semantic_assessment !== false) {
      return stop('invalid', 'forms.invalid-packet-or-source-binding');
    }
    if (seen.has(packet.form.id)) return stop('invalid', 'forms.duplicate-current-identity');
    seen.add(packet.form.id);
    const state = packet.state, role = packet.role ?? null, actualLanguage = packet.language ?? null;
    if (typeof state !== 'string' || !STATES.includes(state)) return stop('invalid', 'forms.unknown-materialization-state');
    if (!assessmentSnapshotValid(packet)) return stop('invalid', 'forms.invalid-assessment-snapshot');
    if ((role !== null && (typeof role !== 'string' || !HUMAN_FORM_ROLES.includes(role)))
      || (actualLanguage !== null && (typeof actualLanguage !== 'string' || !LANGUAGE.test(actualLanguage)))) {
      return stop('invalid', 'forms.invalid-role-or-language');
    }
    result.candidates.push({form: packet.form, role, language: actualLanguage, state, source_pointer: `/attributes/human_forms/${index}`});
    if (state !== 'ready') {
      if (packet.display_text !== null || !Array.isArray(packet.context) || packet.context.length) {
        return stop('invalid', 'forms.nonready-packet-has-wording');
      }
      continue;
    }
    const context = packet.context;
    if (!Object.hasOwn(packet, 'language') || typeof role !== 'string' || !HUMAN_FORM_ROLES.includes(role)
      || typeof packet.display_text !== 'string' || !packet.display_text.trim()
      || !Array.isArray(context) || context.length > 256
      || (actualLanguage !== null && (typeof actualLanguage !== 'string' || !LANGUAGE.test(actualLanguage)))
      || typeof packet.standalone_reading !== 'boolean' || (context.length > 0 && packet.standalone_reading !== false)
      || context.some((raw: unknown) => {
        const entry = record(raw), binding = record(entry.binding);
        return !['slot', 'binding', 'value'].every(key => Object.hasOwn(entry, key))
          || !exactRef(binding.record) || typeof binding.pointer !== 'string';
      })) return stop('invalid', 'forms.incomplete-ready-packet');
    ready.push(packet);
    if (!languageContextValid(packet)) return stop('invalid', 'forms.invalid-language-context');
  }
  if (formDeliveryCost(result) > HUMAN_FORM_SELECTION_BUDGET) return stop('over-budget', 'forms.inspect-collection-separately');
  for (const role of HUMAN_FORM_ROLES) {
    const candidates = ready.filter(packet => packet.role === role);
    let selected = candidates, reason = language === 'auto' ? 'automatic' : 'fallback';
    if (language === 'original') {
      selected = candidates.filter(packet => record(record(packet.language_context).value).relation === 'original');
      if (!selected.length) {
        result.roles[role] = {...empty(), state: 'unavailable', reason: 'original-role-not-declared'};
        continue;
      }
      reason = 'original';
    } else if (language !== 'auto') {
      let candidate = language;
      while (candidate) {
        const matching = candidates.filter(packet => typeof packet.language === 'string' && packet.language.toLowerCase() === candidate.toLowerCase());
        if (matching.length) { selected = matching; reason = candidate === language ? 'exact-language' : 'less-specific-language'; break; }
        candidate = candidate.includes('-') ? candidate.slice(0, candidate.lastIndexOf('-')) : '';
        if (candidate && candidate.split('-').at(-1)!.length === 1) candidate = candidate.includes('-') ? candidate.slice(0, candidate.lastIndexOf('-')) : '';
      }
    }
    if (selected.length > 1) result.roles[role] = {...empty(), state: 'ambiguous', reason: 'multiple-forms'};
    else if (selected.length) {
      const packet = selected[0]!;
      // The input loop already checked every selected form reference.
      if (!exactRef(packet.form)) throw new Error('invalid selected form reference');
      result.roles[role] = {state: 'ready', reason, form: packet.form, packet};
      if (formDeliveryCost(result) > HUMAN_FORM_SELECTION_BUDGET) {
        result.roles[role] = {state: 'over-budget', reason: 'inspect-exact-form', form: packet.form, packet: null};
      }
    } else if (forms.some(packet => record(packet).state !== 'ready')) result.roles[role]!.state = 'unavailable';
  }
  if (formDeliveryCost(result) > HUMAN_FORM_SELECTION_BUDGET) return stop('over-budget', 'forms.inspect-collection-separately');
  return structuredClone(result);
}
