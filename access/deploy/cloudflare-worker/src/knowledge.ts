// Packet types used by native fixture consumers.
import type { selectHumanForms } from './human-forms.ts';

export type Item = Record<string, unknown>;


export type LocalizedText = {
  [language: string]: string | null | undefined;
  default: string;
  ru: string | null;
  en: string | null;
  original?: string | null;
};

export type KnowledgeNode = {
  readable_context?: Item;
  human_form_selection?: ReturnType<typeof selectHumanForms>;
  source_record?: Item;
  id: string;
  entity_id: string;
  native_id: string;
  source_dossier_ref?: string;
  source_graph: string;
  kind_id: string;
  type_id: string;
  type_mapping: Item;
  semantics: Item;
  content_revision: string;
  display: {
    title: LocalizedText;
    kind_label: LocalizedText;
    summary: LocalizedText;
    summary_state: string;
    provenance: Item;
  };
  epistemic: Item;
  graph_layers: string[];
  view_ids: string[];
  source_refs: string[];
  attributes: Item;
};

export type KnowledgeRelation = {
  readable_context?: Item;
  human_form_selection?: ReturnType<typeof selectHumanForms>;
  source_record?: Item;
  id: string;
  native_id: string;
  source_graph: string;
  from_id: string;
  to_id: string;
  predicate_id: string;
  relation_type_id: string;
  predicate_mapping: Item;
  semantics: Item;
  content_revision: string;
  display: {
    label: LocalizedText;
    inverse_label: LocalizedText | null;
    statement: LocalizedText;
    explanation: LocalizedText;
    explanation_state: string;
    provenance: Item;
  };
  epistemic: Item;
  graph_layers: string[];
  view_ids: string[];
  source_refs: string[];
  attributes: Item;
};

export type KnowledgeGraph = {
  schema: "tos_knowledge_graph_v1";
  source_revision: string;
  query_properties?: QueryProperty[];
  nodes: KnowledgeNode[];
  relations: KnowledgeRelation[];
  counts: Item;
  authority_boundary: Item;
};

export type QueryProperty = {property_id: string; field: string; value_type: 'string' | 'number' | 'boolean' | 'string-array';
  applies_to: string[]; inherited: boolean; operators: string[]};
export type LensPresentation = {
  layout: string;
  color_by: string | null;
  lane_by: string | null;
  size_by: string | null;
  inspector_fields: string[];
};
