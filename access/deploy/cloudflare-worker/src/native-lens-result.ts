import {displaySelection, knowledgeScene, type Item, type KnowledgeNode, type KnowledgeRelation, type LensExecutionCounts, type Inclusion} from './knowledge.ts';
import {KnowledgeRevisionConflict} from './lens-pagination.ts';
import {nativeNumberInfo, codePointCompare, nativeInteger} from '../../../shared/native-semantics.ts';
import {nativeHumanForms} from './native-human-forms.ts';
import {arrayRefs, derived, nativeChild, nativeField, nativeKeys, nativePacketArray, nativePacketObject, nativePacketJson,
  parseNativeRequest, nativeSortedRows, nativeGroups, groupPacket, nativeDigest, objectWith, stringField,
  type NativeRef, type NativePacket, type NativePacketValue, type NativeSpec, type NativeLensResult, type NativeLensGroup} from './native-lens.ts';

function optionalObject(ref: NativeRef): boolean {return !!ref.value && typeof ref.value === 'object' && !Array.isArray(ref.value);}
function sceneProjection(ref: NativeRef, selection: Item, forms?: Item): Item {
  const result: Item = {};
  for (const key of ['id', 'entity_id', 'source_graph', 'type_id', 'content_revision', 'from_id', 'to_id', 'relation_type_id']) {
    const value = nativeField(ref, key).value;
    if (typeof value === 'string') result[key] = value;
    else if (value !== null) throw new Error('scene structural source field must be string: ' + key);
  }
  const ancestors = nativeField(ref, 'semantics.type_ancestors'), claim = nativeField(ref, 'semantics.claim');
  const sceneClaim: Item = {};
  if (optionalObject(claim)) {
    for (const key of ['subject_node_id', 'object_node_id', 'predicate_mapping_status', 'relation_type_id']) {
      const value = nativeChild(claim, key).value;
      if (typeof value === 'string' || value === null) sceneClaim[key] = value;
      else throw new Error('scene claim structural source field must be string: ' + key);
    }
    if (nativeKeys(claim).includes('value_member_node_ids')) {
      const members = nativeChild(claim, 'value_member_node_ids');
      sceneClaim.value_member_node_ids = Array.isArray(members.value) && arrayRefs(members).every(r => typeof r.value === 'string') ? arrayRefs(members).map(r => r.value) : null;
    }
  }
  result.semantics = {type_ancestors: Array.isArray(ancestors.value) ? arrayRefs(ancestors).map(r => r.value).filter(v => typeof v === 'string' && v.length) : [], claim: sceneClaim};
  result.display_selection = selection;
  if (forms) result.human_form_selection = forms;
  return result;
}
export function nativeCarrier(ref: NativeRef, spec: Pick<NativeSpec, 'detail' | 'language'>): {packet: NativePacket; scene: Item} {
  // This existing helper returns only selected strings, booleans, counts and
  // pointers. It never copies arbitrary attributes/semantics/source records.
  const selection = displaySelection(ref.value as KnowledgeNode | KnowledgeRelation, spec.language);
  const replacements = new Map<string, NativePacketValue>([['display_selection', derived(selection)]]);
  const attributes = nativeField(ref, 'attributes'); let formScene: Item | undefined;
  if (optionalObject(attributes) && nativeKeys(attributes).includes('human_forms')) {
    const forms = nativeHumanForms(ref, spec.language); replacements.set('human_form_selection', forms.packet); formScene = forms.structural;
  }
  const omit = new Set<string>();
  if (spec.detail === 'compact') {
    replacements.set('attributes', nativePacketObject([])); omit.add('source_record'); omit.add('readable_context');
    const semantics = nativeField(ref, 'semantics'), claim = nativeField(ref, 'semantics.claim');
    if (optionalObject(claim) && nativeKeys(claim).includes('source_canonical_json')) replacements.set('semantics', objectWith(semantics,
      new Map([['claim', objectWith(claim, new Map(), new Set(['source_canonical_json']))]])));
  }
  return {packet: objectWith(ref, replacements, omit), scene: sceneProjection(ref, selection, formScene)};
}
function cursorOffsets(cursor: string | null, fingerprint: string, nodes: number, relations: number): [number, number] {
  if (cursor === null) return [0, 0];
  let token: NativeRef;
  try {
    const raw = Uint8Array.from(atob(cursor.replace(/-/g, '+').replace(/_/g, '/')), char => char.charCodeAt(0));
    token = parseNativeRequest(new TextDecoder('utf-8', {fatal: true, ignoreBOM: true}).decode(raw), {maxBytes: 512});
    if (!optionalObject(token) || [...nativeKeys(token)].sort().join(',') !== 'fingerprint,n,r,v') throw new Error();
    for (const key of ['v', 'n', 'r']) {const ref = nativeChild(token, key); if (typeof ref.value !== 'number' || nativeNumberInfo(ref).kind !== 'int') throw new Error();}
    if (nativeChild(token, 'v').value !== 1 || typeof nativeChild(token, 'fingerprint').value !== 'string' || !/^[a-f0-9]{64}$/.test(nativeChild(token, 'fingerprint').value as string)) throw new Error();
  } catch {throw new Error('invalid lens cursor');}
  const n = nativeChild(token, 'n').value as number, r = nativeChild(token, 'r').value as number;
  if (n < 0 || n > 2000 || r < 0 || r > 2000) throw new Error('invalid lens cursor position');
  if (nativeChild(token, 'fingerprint').value !== fingerprint) throw new KnowledgeRevisionConflict('lens query or snapshot changed; restart pagination');
  if (n > nodes || r > relations) throw new Error('invalid lens cursor position');
  return [n, r];
}
export async function finalizeNativeLens(authority: NativeRef, revision: string, spec: NativeSpec, publicSpec: NativePacket,
  selectedNodes: NativeRef[], selectedRelations: NativeRef[], execution: LensExecutionCounts, focus: NativeRef | null, inclusion: Inclusion,
  publicationBinding?: NativePacketValue): Promise<NativeLensResult> {
  const sortBudget = {bytes: 0, maxBytes: 4 * 1024 * 1024};
  const nodes = nativeSortedRows(selectedNodes, spec.composition.sort_nodes, sortBudget);
  const relations = nativeSortedRows(selectedRelations, spec.composition.sort_relations, sortBudget);
  const groups = nativeGroups(nodes, relations, spec.composition.group_by, spec.limits.groups);
  if (publicSpec.kind !== 'object') throw new Error('native public LensSpec must be object');
  const fingerprint = await nativeDigest(nativePacketObject([
    ['execution_version', 'tos-lens-execution-v7'], ['source_revision', revision],
    ['lens', nativePacketObject(publicSpec.entries.filter(([key]) => key !== 'pagination'))],
    ['nodes', derived(nodes.map(ref => [stringField(ref, 'id'), nativeField(ref, 'content_revision').value ?? '']))],
    ['relations', derived(relations.map(ref => [stringField(ref, 'id'), nativeField(ref, 'content_revision').value ?? '']))],
    ['groups', nativePacketArray(groups.map(groupPacket))],
  ]));
  const sourceRefs = [...new Set([...nodes, ...relations].flatMap(ref => arrayRefs(nativeField(ref, 'source_refs')).map(source => {
    if (typeof source.value !== 'string') throw new Error('source_refs must contain strings'); return source.value;
  })))].sort(codePointCompare);
  const missingNodeSummaries = nodes.filter(ref => nativeField(ref, 'display.summary_state').value === 'missing').length;
  const missingRelationExplanations = relations.filter(ref => nativeField(ref, 'display.explanation_state').value === 'missing').length;
  const nodesWithoutSourceSummary = nodes.filter(ref => nativeField(ref, 'display.provenance.source_summary_available').value === false).length;
  const relationsWithoutSourceExplanation = relations.filter(ref => nativeField(ref, 'display.provenance.source_explanation_available').value === false).length;
  const truncatedNodes = Math.max(0, execution.matched_nodes - spec.limits.nodes), truncatedRelations = Math.max(0, execution.eligible_relations - relations.length);
  const counts = {...execution, nodes: nodes.length, relations: relations.length, groups: groups.length,
    truncated_nodes: truncatedNodes, truncated_relations: truncatedRelations, missing_node_summaries: missingNodeSummaries,
    missing_relation_explanations: missingRelationExplanations, nodes_without_source_summary: nodesWithoutSourceSummary,
    relations_without_source_explanation: relationsWithoutSourceExplanation};
  const countBy = (refs: NativeRef[], field: string): NativePacket => {
    const values = refs.map(ref => stringField(ref, field));
    return nativePacketObject([...new Set(values)].sort(codePointCompare).map(value => [value, nativeInteger(values.filter(v => v === value).length)]));
  };
  let focusPacket: NativePacketValue = null;
  const focusId = focus ? stringField(focus, 'id') : null;
  if (focus) {
    const requested = spec.seed.focus_node_id!;
    focusPacket = nativePacketObject([
      ['requested_id', requested], ['resolved_by', focusId === requested ? 'id' : nativeField(focus, 'entity_id').value === requested ? 'entity_id' : 'native_id'],
      ['node_id', focusId], ...['entity_id', 'native_id', 'source_graph', 'kind_id', 'type_id', 'display'].map(key => [key, nativeChild(focus!, key)] as const),
    ]);
  }
  let pageNodes = nodes, pageRelations = relations, pageGroups = groups, page: unknown = null, pageInclusion = inclusion;
  if (spec.pagination) {
    const cursorFingerprint = publicationBinding === undefined ? fingerprint : await nativeDigest(nativePacketObject([
      ['schema','tos_published_lens_cursor_v1'],['publication',publicationBinding],['fingerprint',fingerprint]]));
    const options = spec.pagination, [nodeOffset, relationOffset] = cursorOffsets(options.cursor, cursorFingerprint, nodes.length, relations.length);
    const primary = nodes.slice(nodeOffset, nodeOffset + options.nodes); pageRelations = relations.slice(relationOffset, relationOffset + options.relations);
    const primaryIds = new Set(primary.map(ref => stringField(ref, 'id'))), ids = new Set([...primaryIds, ...pageRelations.flatMap(ref => [stringField(ref, 'from_id'), stringField(ref, 'to_id')])]);
    if (focusId !== null) ids.add(focusId);
    pageNodes = nodes.filter(ref => ids.has(stringField(ref, 'id')));
    const nextN = nodeOffset + primary.length, nextR = relationOffset + pageRelations.length, hasMore = nextN < nodes.length || nextR < relations.length;
    const nextCursor = hasMore ? btoa(JSON.stringify({v: 1, fingerprint: cursorFingerprint, n: nextN, r: nextR})).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '') : null;
    const relationIds = new Set(pageRelations.map(ref => stringField(ref, 'id')));
    pageGroups = groups.flatMap(group => {
      const node_ids = group.node_ids.filter(id => ids.has(id)), relation_ids = group.relation_ids.filter(id => relationIds.has(id));
      return node_ids.length || relation_ids.length ? [{...group, node_ids, relation_ids}] : [];
    });
    page = {next_cursor: nextCursor, has_more: hasMore, primary_node_ids: [...primaryIds], context_node_ids: pageNodes.map(ref => stringField(ref, 'id')).filter(id => !primaryIds.has(id)),
      returned_nodes: pageNodes.length, returned_relations: pageRelations.length, scope: 'bounded-lens-result', counts_scope: 'complete-bounded-result'};
    pageInclusion = {...inclusion, nodes: Object.fromEntries(Object.entries(inclusion.nodes).filter(([id]) => ids.has(id))), relations: Object.fromEntries(Object.entries(inclusion.relations).filter(([id]) => relationIds.has(id)))};
  }
  const nodeCarriers = pageNodes.map(ref => nativeCarrier(ref, spec)), relationCarriers = pageRelations.map(ref => nativeCarrier(ref, spec));
  const warnings = [
    ...(execution.identity_expansion_limited ? ['identity carrier expansion reached the node budget; use resumable exploration or narrower sources'] : []),
    ...(missingNodeSummaries ? [`${missingNodeSummaries} nodes expose an explicit missing-summary state`] : []),
    ...(missingRelationExplanations ? [`${missingRelationExplanations} relations expose an explicit missing-explanation state`] : []),
    ...(nodesWithoutSourceSummary ? [`${nodesWithoutSourceSummary} nodes use transparent metadata synthesis because no source summary is projected`] : []),
    ...(relationsWithoutSourceExplanation ? [`${relationsWithoutSourceExplanation} relations use transparent metadata synthesis because no source explanation is projected`] : []),
    ...(truncatedNodes ? [`node selector exceeded its bounded result by ${truncatedNodes} nodes`] : []),
    ...(truncatedRelations ? [`relation selector exceeded its bounded result by ${truncatedRelations} relations`] : []),
  ];
  const entries: [string, NativePacketValue][] = [
    ['schema', 'tos_lens_result_v1'], ['source_revision', revision], ['lens', publicSpec], ['fingerprint', fingerprint],
    ['presentation', derived(spec.presentation)], ['focus', focusPacket],
    ...(spec.explain ? [['inclusion', derived(pageInclusion)] as [string, NativePacketValue]] : []),
    ['nodes', nativePacketArray(nodeCarriers.map(c => c.packet))], ['relations', nativePacketArray(relationCarriers.map(c => c.packet))],
    ['groups', nativePacketArray(pageGroups.map(groupPacket))], ['facets', nativePacketObject([['node_kinds', countBy(nodes, 'kind_id')], ['predicates', countBy(relations, 'predicate_id')], ['sources', countBy(nodes, 'source_graph')]])],
    ['counts', derived(counts)], ['source_refs', derived(sourceRefs)], ['warnings', derived(warnings)], ['authority_boundary', authority],
    ['agent_summary', derived({lens_id: spec.lens_id, focus_node_id: focusId, node_count: nodes.length, relation_count: relations.length, group_count: groups.length, source_ref_count: sourceRefs.length, is_source: false, writes_to_tree: false})],
    ...(page !== null ? [['page', derived(page)] as [string, NativePacketValue]] : []),
    ['scene', derived(knowledgeScene(nodeCarriers.map(c => c.scene), relationCarriers.map(c => c.scene), focusId))],
  ];
  return Object.freeze({packet: nativePacketObject(entries), preview: Object.freeze({node_ids: Object.freeze(pageNodes.map(ref => stringField(ref, 'id'))), relation_ids: Object.freeze(pageRelations.map(ref => stringField(ref, 'id'))), fingerprint, counts: Object.freeze(counts)})});
}
