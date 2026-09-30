// Packet types used by the retained pagination and native fixture consumers.
import type { Pagination } from './lens-pagination.ts';
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
export type LensFilter = { field?: string; property_id?: string; op: string; value: unknown; _property_binding?: QueryProperty };
export type SortRule = { field: string; direction: "asc" | "desc" };
export type PathCondition = {
  path_id: string;
  quantifier: "exists" | "not_exists";
  steps: { direction: "incoming" | "outgoing" | "either";
    node_query: LensSpec["node_query"]; relation_query: LensSpec["relation_query"] }[];
};
export type Inclusion = { nodes: Record<string, Item>; relations: Record<string, Item>; authority: "query-execution-not-semantic-proof" };
export type LensSpec = {
  pagination: Pagination | null;
  path_query: PathCondition[];
  explain: boolean;
  detail: "full" | "compact";
  schema_version: "tos_lens_spec_v1";
  lens_id: string;
  title: LocalizedText;
  description: LocalizedText;
  language: string;
  sources: string[];
  seed: { focus_node_id: string | null; node_ids: string[]; text_query: string };
  node_query: { enabled: boolean; match: "all" | "any"; filters: LensFilter[] };
  relation_query: { enabled: boolean; match: "all" | "any"; filters: LensFilter[] };
  traversal: { depth: number; direction: "outgoing" | "incoming" | "either"; predicate_ids: string[]; profile: "all" | "overview" };
  composition: {
    endpoint_policy: "both" | "either" | "independent";
    group_by: string[];
    sort_nodes: SortRule[];
    sort_relations: SortRule[];
  };
  presentation: {
    layout: string;
    color_by: string | null;
    lane_by: string | null;
    size_by: string | null;
    inspector_fields: string[];
  };
  limits: { nodes: number; relations: number; groups: number };
};
