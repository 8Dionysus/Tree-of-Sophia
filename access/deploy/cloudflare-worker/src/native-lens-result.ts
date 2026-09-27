/** Existing exploration carrier presentation; published lens finalization is Rust. */
import {displaySelection, type Item, type KnowledgeNode, type KnowledgeRelation, type LensSpec} from './knowledge.ts';
import {nativeHumanForms} from './native-human-forms.ts';
import {arrayRefs, derived, nativeChild, nativeField, nativeKeys, nativePacketObject, objectWith,
  type NativeRef, type NativePacket, type NativePacketValue} from './native-lens.ts';

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
export function nativeCarrier(ref: NativeRef, spec: Pick<LensSpec, 'detail' | 'language'>): {packet: NativePacket; scene: Item} {
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
