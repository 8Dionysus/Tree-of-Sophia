import type {WebMcpRules, WebMcpResultChoice, WebMcpToolSession, WebMcpViewSelectorSession} from '../../deploy/cloudflare-worker/generated/tos_web_rules.js';
type WebMcpRuntime={WebMcpRules:typeof WebMcpRules;WebMcpResultChoice:typeof WebMcpResultChoice;WebMcpToolSession:typeof WebMcpToolSession;WebMcpViewSelectorSession:typeof WebMcpViewSelectorSession};
let installedWebMcp:WebMcpRuntime|undefined;
export function installWebMcpRules(runtime:WebMcpRuntime) {
  if(typeof runtime?.WebMcpRules!=='function'||typeof runtime.WebMcpResultChoice!=='function'||typeof runtime.WebMcpToolSession!=='function'||typeof runtime.WebMcpViewSelectorSession!=='function')throw new TypeError('Generated WebMCP Rust rules are incomplete');
  installedWebMcp=runtime;
}
function webMcpRuntime(){if(!installedWebMcp)throw new Error('WebMCP Rust rules are not installed');return installedWebMcp;}
export function webMcpViewSelector(){return webMcpRuntime().WebMcpViewSelectorSession;}
function webMcpRules(){return webMcpRuntime().WebMcpRules;}
const toUnits=(value:string)=>Uint16Array.from({length:value!.length},(_,index)=>value!.charCodeAt(index));
function fromUnits(units:Uint16Array){const chunks:string[]=[];for(let i=0;i<units.length;i+=4096)chunks.push(String.fromCharCode(...units.subarray(i,i+4096)));return chunks.join('');}
function choice(policy:string,reads:Array<()=>unknown>):unknown {
  const session=new (webMcpRuntime().WebMcpResultChoice)(policy);
  try{let value:unknown;while(!session.done()){value=reads[session.need()]!();if(session.needs_array())session.observe_array(Array.isArray(value));else if(policy==='knowledge-cursor')session.observe_string(typeof value==='string',Boolean(value));else session.observe(Boolean(value),value===null||value===undefined,typeof value==='object',value===null);}return value;}
  finally{session.free();}
}
function optionalText(value:string,kind:'undefined'|'null'|'unresolved'='undefined'):string|undefined|null {
  switch(webMcpRules().optional_text(kind,Boolean(value))){case 1:return undefined;case 2:return null;case 3:return 'unresolved';default:return value;}
}
function optionalIdentity(value:unknown):string|undefined {return optionalText(identity(value)) as string|undefined;}
function knowledgeNextAction(value:unknown){return webMcpRules().knowledge_next_action(typeof value==='boolean',Boolean(value));}
function wordNextAction(value:unknown){return webMcpRules().word_next_action(typeof value==='boolean',Boolean(value));}
function trueFlag(value:unknown){return webMcpRules().strict_true(typeof value==='boolean',Boolean(value));}
function shape(kind:string,value:unknown){const object=typeof value==='object';const array=webMcpRules().shape_array_needed(kind,Boolean(value),object)?Array.isArray(value):false;return webMcpRules().shape(kind,Boolean(value),object,array);}

import type { PageCommandId, PageCommandRegistry, PageContext } from "./page-commands";

type JsonSchema = Record<string, unknown>;

type WebMCPTool = {
  name: string;
  title?: string;
  description: string;
  inputSchema?: JsonSchema;
  annotations?: { readOnlyHint?: boolean; untrustedContentHint?: boolean };
  execute: (input: Record<string, unknown>, options: { signal: AbortSignal }) => Promise<unknown>;
};

type ModelContextLike = {
  registerTool(tool: WebMCPTool, options?: { signal?: AbortSignal }): Promise<void>;
};

export type WebMCPDocument = Document & { modelContext?: ModelContextLike };

export type WebMCPStatus = {
  supported: boolean;
  registered: boolean;
  stable_tool_count: number;
  selection_tool_count: number;
  tool_count: number;
  context_revision: number;
  registration_error: string | null;
};

const toolCommands = new WeakMap<WebMCPTool, PageCommandId>();

const emptySchema: JsonSchema = { type: "object", properties: {}, additionalProperties: false };

function objectSchema(properties: Record<string, unknown>, required: string[] = []): JsonSchema {
  return {
    type: "object",
    properties,
    ...(required.length ? { required } : {}),
    additionalProperties: false,
  };
}

function toolResult(value: unknown): Record<string, unknown> {
  return {
    content: [{ type: "text", text: JSON.stringify(value) }],
  };
}

function commandTool(
  registry: PageCommandRegistry,
  commandId: PageCommandId,
  definition: Omit<WebMCPTool, "execute">,
  capturedRevision?: number,
  compact?: (value: Record<string, unknown>) => unknown,
  bindInput?: (input: Record<string, unknown>) => Record<string, unknown>,
): WebMCPTool {
  const tool: WebMCPTool = {
    ...definition,
    execute: async (input, options) => {
      const boundInput = bindInput ? bindInput(input) : input;
      const commandInput = capturedRevision === undefined
        ? boundInput
        : { ...boundInput, context_revision: capturedRevision };
      const value = await registry.invoke(commandId, commandInput, { signal: options.signal });
      return toolResult(compact ? compact(value) : value);
    },
  };
  toolCommands.set(tool, commandId);
  return tool;
}

function identity(value: unknown): string { return webMcpRules().identity_action(typeof value === "string") === 1 ? value as string : ""; }

function compactSelectionResult(result: Record<string, unknown>): unknown {
  const context = result.context as Record<string, unknown> | undefined;
  const selection = result.value as Record<string, unknown> | undefined;
  return {
    selection: shape('selection',selection)===1 ? {
      id: identity(selection!.id),
      page_kind: clipped(selection!.kind, 'compactSelectionResult.text.0'),
      semantic_kind: clipped(choice('selected-kind',[()=>selection!.semantic_kind,()=>selection!.kind]), 'compactSelectionResult.text.1'),
      label: clipped(selection!.label, 'compactSelectionResult.text.2'),
      subtitle: clipped(selection!.subtitle, 'compactSelectionResult.text.3'),
      from_id: optionalIdentity(selection!.from_id),
      to_id: optionalIdentity(selection!.to_id),
      predicate_id: optionalIdentity(selection!.predicate_id),
      source_refs: choice('array-preview-0',[()=>(selection!.source_refs),()=>((selection!.source_refs as unknown[]).slice(0,webMcpRules().array_bound('compactSelectionResult.array.0')).map((ref) => identity(ref))),()=>[]]),
      authority_posture: optionalText(clipped(selection!.authority_posture, 'compactSelectionResult.text.4')),
      review_posture: optionalText(clipped(selection!.review_posture, 'compactSelectionResult.text.5')),
      canon_status: optionalText(clipped(selection!.canon_status, 'compactSelectionResult.text.6')),
      confidence: optionalText(clipped(selection!.confidence, 'compactSelectionResult.text.7')),
    } : null,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
  };
}

function compactPageContext(value: Record<string, unknown>): unknown {
  const selected = value!.selected as Record<string, unknown> | undefined;
  return {
    schema: value!.schema,
    revision: value!.revision,
    mode: value!.mode,
    view_id: clipped(value!.view_id, 'compactPageContext.text.0'),
    graph_mode: value!.graph_mode,
    selected: shape('page-selected',selected)===1 ? {
      id: identity(selected!.id),
      page_kind: clipped(selected!.kind, 'compactPageContext.text.1'),
      semantic_kind: clipped(choice('selected-kind',[()=>selected!.semantic_kind,()=>selected!.kind]), 'compactPageContext.text.2'),
      label: clipped(selected!.label, 'compactPageContext.text.3'),
      from_id: optionalIdentity(selected!.from_id),
      to_id: optionalIdentity(selected!.to_id),
      source_refs: choice('array-preview-1',[()=>(selected!.source_refs),()=>((selected!.source_refs as unknown[]).slice(0,webMcpRules().array_bound('compactPageContext.array.0')).map((ref) => identity(ref))),()=>[]]),
    } : null,
    path_start_node_id: optionalText(identity(value!.path_start_node_id),'null'),
    active_layers: choice('array-preview-2',[()=>(value!.active_layers),()=>((value!.active_layers as unknown[]).slice(0,webMcpRules().array_bound('compactPageContext.array.1'))),()=>[]]),
    active_predicates: choice('array-preview-3',[()=>(value!.active_predicates),()=>((value!.active_predicates as unknown[]).slice(0,webMcpRules().array_bound('compactPageContext.array.2'))),()=>[]]),
    research_workspace: value!.research_workspace,
    deep_link: identity(value!.deep_link),
    pending_command_ids: choice('array-preview-4',[()=>(value!.pending_command_ids),()=>((value!.pending_command_ids as unknown[]).slice(0,webMcpRules().array_bound('compactPageContext.array.3'))),()=>[]]),
  };
}

function compactSearchResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const results = choice('array-preview-5',[()=>(value?.results),()=>(value!.results as Array<Record<string, unknown>>),()=>[]]) as Array<Record<string,unknown>>;
  return {
    query: clipped(value?.query, 'compactSearchResult.text.0'),
    result_count: choice('search-count',[()=>value?.result_count,()=>Number(choice('result-count',[()=>value?.result_count,()=>0]))]),
    results: results.slice(0,webMcpRules().array_bound('compactSearchResult.array.0')).map((item) => ({
      id: identity(item.id),
      kind: clipped(choice('item-kind',[()=>item.semantic_kind,()=>item.kind]), 'compactSearchResult.text.1'),
      label: clipped(item.label, 'compactSearchResult.text.2'),
      posture: clipped(choice('search-posture',[()=>item.review_posture,()=>item.canon_status,()=>item.authority_posture]), 'compactSearchResult.text.3'),
      summary: clipped(item.summary, 'compactSearchResult.text.4'),
    })),
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
    next_action: "select one result by stable id",
  };
}

function compactKnowledgeSearchResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const compactItems = (items: unknown): unknown[] => (choice('array-preview-6',[()=>(items),()=>(items),()=>[]]) as unknown[])
    .slice(0,webMcpRules().array_bound('compactKnowledgeSearchResult.array.0'))
    .map((item) => {
      const source = shape('knowledge-item',item)===1 ? item as Record<string,unknown> : {};
      return {
        id: identity(source!.id),
        kind: clipped(choice('knowledge-kind',[()=>source!.semantic_kind,()=>source!.kind,()=>source!.knowledge_search_kind]), 'compactKnowledgeSearchResult.text.0'),
        label: clipped(source!.label, 'compactKnowledgeSearchResult.text.1'),
        subtitle: clipped(choice('knowledge-subtitle',[()=>source!.subtitle,()=>source!.node_type,()=>source!.predicate_id]), 'compactKnowledgeSearchResult.text.2'),
        from_id: optionalIdentity(source!.from_id),
        to_id: optionalIdentity(source!.to_id),
        source_refs: choice('array-preview-7',[()=>(source!.source_refs),()=>((source!.source_refs as unknown[]).slice(0,webMcpRules().array_bound('compactKnowledgeSearchResult.array.1')).map(identity)),()=>[]]),
      };
    });
  const page = choice('knowledge-page',[()=>value?.page,()=>value!.page,()=>value!.page,()=>({})]) as Record<string,unknown>;
  return {
    schema: value?.schema,
    search_mode: value?.search_mode,
    query: clipped(value?.query, 'compactKnowledgeSearchResult.text.3'),
    result_count: Number(choice('result-count',[()=>value?.result_count,()=>0])),
    nodes: compactItems(value?.nodes),
    relations: compactItems(value?.relations),
    counts: value?.counts,
    // Cursors are opaque continuation state.  Do not truncate them while
    // compacting the human-readable result envelope.
    next_cursor: choice('knowledge-cursor',[()=>page.next_cursor,()=>page.next_cursor,()=>page.next_cursor,()=>null]),
    has_more: trueFlag(page.has_more),
    source_revision: clipped(value?.source_revision, 'compactKnowledgeSearchResult.text.4'),
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
    next_action: knowledgeNextAction(page.has_more),
  };
}

function compactSourceGapResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const gaps = choice('array-preview-8',[()=>(value?.gaps),()=>(value!.gaps as Array<Record<string, unknown>>),()=>[]]) as Array<Record<string,unknown>>;
  return {
    query: clipped(value?.query, 'compactSourceGapResult.text.0'),
    result_count: Number(choice('result-count',[()=>value?.result_count,()=>0])),
    gaps: gaps.slice(0,webMcpRules().array_bound('compactSourceGapResult.array.0')).map((item) => ({
      id: identity(item.id),
      label: clipped(item.label, 'compactSourceGapResult.text.1'),
      status: clipped(choice('gap-posture',[()=>item.review_posture,()=>item.authority_posture]), 'compactSourceGapResult.text.2'),
      summary: clipped(item.summary, 'compactSourceGapResult.text.3'),
    })),
    authority_note: clipped(value?.authority_note, 'compactSourceGapResult.text.4'),
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
    next_actions: ["use web research to verify a lawful current route", "select one gap", "stage a source_route proposal"],
  };
}

function compactEvidenceResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const summary = value?.agent_summary as Record<string, unknown> | undefined;
  if (shape('evidence',summary)===2) return result;
  const context = result.context as Record<string, unknown> | undefined;
  return {
    ...summary,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
  };
}

function clipped(value: unknown, field: string): string {
  const Rule=webMcpRules();
  const action=Rule.text_action(field,typeof value==='string',typeof value==='string'?value!.length:0);
  if(action===0)return '';
  if(action===1)return value as string;
  const prefix=(value as string).slice(0,Rule.text_prefix(field));
  return fromUnits(Rule.text_finish(toUnits(prefix)));
}

function compactPathResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  if (shape('path',value)===2) return result;
  const paths = choice('array-preview-9',[()=>(value!.paths),()=>(value!.paths as Array<Record<string, unknown>>),()=>[]]) as Array<Record<string,unknown>>;
  const first = choice('path-first',[()=>paths[0],()=>({})]) as Record<string,unknown>;
  const nodeIds = choice('array-preview-10',[()=>(first.node_ids),()=>((first.node_ids as unknown[]).map(identity).slice(0,webMcpRules().array_bound('compactPathResult.array.0'))),()=>[]]) as unknown[];
  const edgeIds = choice('array-preview-11',[()=>(first.edge_ids),()=>(first.edge_ids),()=>[]]) as unknown[];
  const context = result.context as Record<string, unknown> | undefined;
  return {
    finding: webMcpRules().path_finding(Boolean(value!.found)),
    found: trueFlag(value!.found),
    from_id: identity(value!.from_id),
    to_id: identity(value!.to_id),
    route_count: Number(choice('path-count',[()=>value!.path_count,()=>paths.length,()=>0])),
    first_route_node_ids: nodeIds,
    first_route_edge_count: edgeIds.length,
    excluded_edge_count: choice('array-preview-12',[()=>(value!.excluded_edge_ids),()=>((value!.excluded_edge_ids as unknown[]).length),()=>0]),
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
    exploration_truncated: trueFlag(value!.exploration_truncated),
    next_actions: choice('path-actions',[()=>value!.next_actions,()=>webMcpRules().path_actions(Boolean(value!.found)).split('\n')]),
  };
}

function compactNeighborhoodResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const node = value?.node as Record<string, unknown> | undefined;
  const neighbors = choice('array-preview-13',[()=>(value?.neighbors),()=>(value!.neighbors as Array<Record<string, unknown>>),()=>[]]) as Array<Record<string,unknown>>;
  const edges = choice('array-preview-14',[()=>(value?.edges),()=>(value!.edges as Array<Record<string, unknown>>),()=>[]]) as Array<Record<string,unknown>>;
  const page = value?.page as Record<string, unknown> | undefined;
  return {
    selection: {
      id: identity(choice('neighborhood-id',[()=>node?.node_id,()=>node?.id])),
      label: clipped(choice('neighborhood-label',[()=>node?.label,()=>node?.title]), 'compactNeighborhoodResult.text.0'),
    },
    neighbor_count: neighbors.length,
    relation_count: edges.length,
    scope: webMcpRules().neighborhood_scope(Boolean(page)),
    page_number: page?.number,
    has_more: shape('neighborhood-page',page)===1 ? Boolean(page!.next_cursor) : undefined,
    neighbors: neighbors.slice(0,webMcpRules().array_bound('compactNeighborhoodResult.array.0')).map((item) => ({
      id: identity(choice('neighbor-id',[()=>item.node_id,()=>item.id])),
      label: clipped(choice('neighbor-label',[()=>item.label,()=>item.title]), 'compactNeighborhoodResult.text.1'),
      kind: clipped(item.node_type, 'compactNeighborhoodResult.text.2'),
    })),
    predicates: choice('array-preview-15',[()=>(value?.predicates),()=>((value!.predicates as unknown[]).slice(0,webMcpRules().array_bound('compactNeighborhoodResult.array.1'))),()=>[]]),
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
  };
}

function compactReadingComparison(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const selection = value?.selection as Record<string, unknown> | undefined;
  const readings = choice('array-preview-16',[()=>(value?.competing_readings),()=>(value!.competing_readings as Array<Record<string, unknown>>),()=>[]]) as Array<Record<string,unknown>>;
  return {
    schema: value?.schema,
    selection: shape('selection',selection)===1 ? { id: identity(selection!.id), kind: clipped(choice('selected-kind',[()=>selection!.semantic_kind,()=>selection!.kind]), 'compactReadingComparison.text.0'), label: clipped(selection!.label, 'compactReadingComparison.text.1') } : null,
    posture: value?.posture,
    can_conclude: trueFlag(value?.can_conclude),
    competing_reading_count: Number(choice('reading-count',[()=>value?.competing_reading_count,()=>0])),
    competing_readings: readings.slice(0,webMcpRules().array_bound('compactReadingComparison.array.0')).map((reading) => ({
      id: identity(reading.id),
      label: clipped(reading.label, 'compactReadingComparison.text.2'),
      predicate_id: identity(reading.predicate_id),
      review_posture: optionalText(clipped(reading.review_posture, 'compactReadingComparison.text.3'),'unresolved'),
      source_refs: choice('array-preview-17',[()=>(reading.source_refs),()=>((reading.source_refs as unknown[]).slice(0,webMcpRules().array_bound('compactReadingComparison.array.1')).map((ref) => identity(ref))),()=>[]]),
    })),
    gaps: choice('array-preview-18',[()=>(value?.gaps),()=>((value!.gaps as unknown[]).slice(0,webMcpRules().array_bound('compactReadingComparison.array.2')).map((gap) => clipped(gap, 'compactReadingComparison.text.4'))),()=>[]]),
    authority_note: clipped(value?.authority_note, 'compactReadingComparison.text.5'),
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
    next_actions: ["inspect a competing relation", "stage a traceable local proposal"],
  };
}

function compactWorkspaceMutation(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const summary = choice('workspace-summary',[()=>value?.summary,()=>context?.research_workspace]);
  const hypothesis = value?.hypothesis as Record<string, unknown> | undefined;
  const route = value?.route as Record<string, unknown> | undefined;
  const proposal = value?.proposal as Record<string, unknown> | undefined;
  return {
    changed: choice('workspace-changed',[()=>value?.changed,()=>value?.added,()=>value?.imported,()=>true]),
    ...(shape('excluded-edge',value?.excluded_edge_id)===1 ? { excluded_edge_id: clipped(value!.excluded_edge_id, 'compactWorkspaceMutation.text.0') } : {}),
    ...(shape('workspace-hypothesis',hypothesis)===1 ? { hypothesis: { id: clipped(hypothesis!.id, 'compactWorkspaceMutation.text.1'), title: clipped(hypothesis!.title, 'compactWorkspaceMutation.text.2'), posture: hypothesis!.posture } } : {}),
    ...(shape('workspace-route',route)===1 ? { route: { id: clipped(route!.id, 'compactWorkspaceMutation.text.3'), label: clipped(route!.label, 'compactWorkspaceMutation.text.4'), node_count: choice('array-preview-19',[()=>(route!.nodeIds),()=>((route!.nodeIds as unknown[]).length),()=>0]), edge_count: choice('array-preview-20',[()=>(route!.edgeIds),()=>((route!.edgeIds as unknown[]).length),()=>0]) } } : {}),
    ...(shape('workspace-proposal',proposal)===1 ? { proposal: {
      id: identity(proposal!.id),
      kind: clipped(proposal!.kind, 'compactWorkspaceMutation.text.5'),
      statement: clipped(proposal!.statement, 'compactWorkspaceMutation.text.6'),
      status: choice('proposal-status',[()=>proposal!.reviewStatus,()=>proposal!.review_status]),
      digest: clipped(proposal!.digest, 'compactWorkspaceMutation.text.7'),
    } } : {}),
    comparison_ready: value?.comparison_ready,
    research_workspace: summary,
    local_only: true,
    authority: { source: false, reviewed: false, canon: false },
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
  };
}

function compactWorkspaceRead(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const packet = value?.packet as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const hypotheses = choice('array-preview-21',[()=>(packet?.hypotheses),()=>(packet!.hypotheses as Array<Record<string, unknown>>),()=>[]]) as Array<Record<string,unknown>>;
  const proposals = choice('array-preview-22',[()=>(packet?.proposals),()=>(packet!.proposals as Array<Record<string, unknown>>),()=>[]]) as Array<Record<string,unknown>>;
  const routes = choice('array-preview-23',[()=>(packet?.route_snapshots),()=>(packet!.route_snapshots as Array<Record<string, unknown>>),()=>[]]) as Array<Record<string,unknown>>;
  const notes = choice('array-preview-24',[()=>(packet?.notes),()=>(packet!.notes as Array<Record<string, unknown>>),()=>[]]) as Array<Record<string,unknown>>;
  const journal = choice('array-preview-25',[()=>(packet?.journal),()=>(packet!.journal as Array<Record<string, unknown>>),()=>[]]) as Array<Record<string,unknown>>;
  return {
    research_workspace: context?.research_workspace,
    selected_lens: choice('workspace-lens',[()=>packet?.selected_lens,()=>null]),
    hypothesis_preview: hypotheses.slice(webMcpRules().array_tail('compactWorkspaceRead.array.0')).map((item) => ({ id: identity(item.id), title: clipped(item.title, 'compactWorkspaceRead.text.0'), body: clipped(item.body, 'compactWorkspaceRead.text.1'), posture: item.posture })),
    proposal_preview: proposals.slice(webMcpRules().array_tail('compactWorkspaceRead.array.1')).map((item) => ({ id: identity(item.id), kind: item.kind, statement: clipped(item.statement, 'compactWorkspaceRead.text.2'), review_status: item.review_status, digest: clipped(item.digest, 'compactWorkspaceRead.text.3') })),
    excluded_edge_ids: (choice('array-preview-26',[()=>(packet?.excluded_edge_ids),()=>(packet!.excluded_edge_ids),()=>[]])).slice(webMcpRules().array_tail('compactWorkspaceRead.array.2')).map((id) => clipped(id, 'compactWorkspaceRead.text.4')),
    route_preview: routes.slice(webMcpRules().array_tail('compactWorkspaceRead.array.3')).map((item) => ({ label: clipped(item.label, 'compactWorkspaceRead.text.5'), node_count: choice('array-preview-27',[()=>(item.node_ids),()=>((item.node_ids as unknown[]).length),()=>0]), edge_count: choice('array-preview-28',[()=>(item.edge_ids),()=>((item.edge_ids as unknown[]).length),()=>0]) })),
    note_preview: notes.slice(webMcpRules().array_tail('compactWorkspaceRead.array.4')).map((item) => ({ body: clipped(item.body, 'compactWorkspaceRead.text.6'), target_id: identity(item.target_id) })),
    recent_actions: journal.slice(webMcpRules().array_tail('compactWorkspaceRead.array.5')).map((item) => ({ action: clipped(item.action, 'compactWorkspaceRead.text.7'), target_id: identity(item.target_id) })),
    local_only: true,
    authority: { source: false, reviewed: false, canon: false },
    context_revision: result.context_revision,
  };
}

function compactWordAnalysisResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const task = value?.task as Record<string, unknown> | undefined;
  const source = task?.source as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  return {
    schema: value?.schema,
    available: trueFlag(value?.available),
    reason: optionalText(clipped(value?.reason, 'compactWordAnalysisResult.text.0'),'null'),
    publication_posture: value?.publication_posture,
    source: shape('word-source',source)===1 ? {
      occurrence_id: clipped(choice('word-occurrence',[()=>source!.occurrence_id,()=>source!.id]), 'compactWordAnalysisResult.text.1'),
      language: clipped(source!.language, 'compactWordAnalysisResult.text.2'),
      surface: clipped(choice('word-surface',[()=>source!.surface,()=>source!.text]), 'compactWordAnalysisResult.text.3'),
      source_ref: clipped(source!.source_ref, 'compactWordAnalysisResult.text.4'),
    } : null,
    task_schema: choice('word-task-schema',[()=>task?.schema_version,()=>task?.schema]),
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
    next_action: wordNextAction(value?.available),
  };
}

function stableTools(registry: PageCommandRegistry): WebMCPTool[] {
  return [
    commandTool(registry, "tos.page.context", {
      name: "tos.page.context",
      title: "Read Tree of Sophia page context",
      description: "Return the current ToS view, selected object, filters, path start, deep link, and context revision.",
      inputSchema: emptySchema,
      annotations: { readOnlyHint: true },
    }, undefined, compactPageContext),
    commandTool(registry, "tos.page.prepare-word-analysis", {
      name: "tos.zarathustra.word-analysis.prepare",
      title: "Prepare a source-bound Zarathustra word analysis",
      description: "Resolve a German, Russian, or English concept query to one exact German occurrence and return the required morphology, syntax, historical-sense, cited-etymology, Russian-comparison, and English-rendering task. The result is local, unreviewed, and non-canonical.",
      inputSchema: objectSchema({
        query: { type: "string", minLength: 1, maxLength: 256 },
        language: { type: "string", enum: ["de", "ru", "en"], default: "ru" },
        rank: { type: "integer", minimum: 1, maximum: 100, default: 1 },
        include_semantic_neighbors: { type: "boolean", default: false },
      }, ["query"]),
      annotations: { readOnlyHint: false, untrustedContentHint: true },
    }, undefined, compactWordAnalysisResult),
    commandTool(registry, "tos.page.research-workspace", {
      name: "tos.page.research-workspace",
      title: "Read the local research workspace",
      description: "Return session-local hypotheses, exclusions, saved route comparisons, notes, and undo state. Nothing here changes ToS source or canon.",
      inputSchema: emptySchema,
      annotations: { readOnlyHint: true, untrustedContentHint: true },
    }, undefined, compactWorkspaceRead),
    commandTool(registry, "tos.page.add-research-note", {
      name: "tos.page.add-research-note",
      title: "Add a local research note",
      description: "Add a bounded global note to this browser's research session without writing to Tree of Sophia sources, review, or canon. Select an object first to receive a context-bound note tool.",
      inputSchema: objectSchema({ text: { type: "string", minLength: 1, maxLength: 2000 } }, ["text"]),
      annotations: { readOnlyHint: false, untrustedContentHint: true },
    }, undefined, compactWorkspaceMutation),
    commandTool(registry, "tos.page.workspace-undo", {
      name: "tos.page.workspace-undo",
      title: "Undo the last research edit",
      description: "Undo one local research workspace change. This never changes Tree of Sophia source, review, or canon.",
      inputSchema: emptySchema,
      annotations: { readOnlyHint: false },
    }, undefined, compactWorkspaceMutation),
    commandTool(registry, "tos.page.workspace-redo", {
      name: "tos.page.workspace-redo",
      title: "Redo the last research edit",
      description: "Redo one previously undone local research workspace change.",
      inputSchema: emptySchema,
      annotations: { readOnlyHint: false },
    }, undefined, compactWorkspaceMutation),
    commandTool(registry, "tos.page.open-view", {
      name: "tos.page.open-view",
      title: "Open a Tree of Sophia view",
      description: "Open a named philosophy or corpus view on the shared ToS page, optionally focusing one object.",
      inputSchema: objectSchema(
        {
          mode: { type: "string", enum: ["philosophy", "corpus"] },
          view_id: { type: "string", minLength: 1 },
          graph_mode: { type: "string", enum: ["clusters", "nodes"] },
          focus_id: { type: "string" },
        },
        ["mode", "view_id"],
      ),
      annotations: { readOnlyHint: false },
    }),
    commandTool(registry, "tos.page.search", {
      name: "tos.page.search",
      title: "Search on the Tree of Sophia page",
      description: "Search the active ToS surface and show the results in the shared page inspector.",
      inputSchema: objectSchema({ query: { type: "string" } }, ["query"]),
      annotations: { readOnlyHint: false },
    }, undefined, compactSearchResult),
    commandTool(registry, "tos.page.knowledge-search", {
      name: "tos.page.knowledge-search",
      title: "Search the ToS knowledge carrier",
      description: "Search normalized Tree of Sophia nodes and relations through a source-revision-bound engine advertised by the selected backend. Keep returned search_mode with an opaque continuation cursor. This is a derived access view; projection search remains available through tos.page.search.",
      inputSchema: objectSchema({
        query: { type: "string", minLength: 1, maxLength: 256 },
        cursor: { type: "string", maxLength: 65536 },
        search_mode: { type: "string", enum: ["indexed", "compressed"] },
        limit: { type: "integer", minimum: 1, maximum: 6, default: 6 },
      }, ["query"]),
      annotations: { readOnlyHint: false, untrustedContentHint: true },
    }, undefined, compactKnowledgeSearchResult, (input) => ({
      ...input,
      // The page keeps up to forty results for the human UI, while the agent
      // envelope is intentionally bounded to six per kind.  Binding this
      // default before invoking the page command prevents a backend cursor
      // from advancing past items that compaction cannot return.
      limit: webMcpRules().knowledge_limit(Number(input.limit)),
    })),
    commandTool(registry, "tos.page.find-source-gaps", {
      name: "tos.page.find-source-gaps",
      title: "Find recorded source-access gaps",
      description: "Search the bounded public Tree of Sophia access-request ledger. Use the result as a starting point for live web research, then select a gap and stage a source_route proposal. This tool does not claim corpus completeness or legal clearance, contact anyone, download sources, or change source and canon.",
      inputSchema: objectSchema({
        query: { type: "string", maxLength: 256 },
        limit: { type: "integer", minimum: 1, maximum: 100, default: 20 },
      }),
      annotations: { readOnlyHint: false, untrustedContentHint: true },
    }, undefined, compactSourceGapResult),
    commandTool(registry, "tos.page.select", {
      name: "tos.page.select",
      title: "Select an object on the Tree of Sophia page",
      description: "Select a node, edge, cluster, or result already present in the active ToS view by its stable ID.",
      inputSchema: objectSchema({ item_id: { type: "string", minLength: 1 } }, ["item_id"]),
      annotations: { readOnlyHint: false },
    }),
    commandTool(registry, "tos.page.clear-focus", {
      name: "tos.page.clear-focus",
      title: "Leave the current graph focus",
      description: "Clear neighborhood, path, and expanded-cluster focus while preserving the active ToS view.",
      inputSchema: emptySchema,
      annotations: { readOnlyHint: false },
    }),
    commandTool(registry, "tos.page.cancel", {
      name: "tos.page.cancel",
      title: "Cancel an active ToS page operation",
      description: "Cancel active page commands, or one command identified by command_id.",
      inputSchema: objectSchema({ command_id: { type: "string" } }),
      annotations: { readOnlyHint: false },
    }),
  ];
}

function dynamicTools(registry:PageCommandRegistry,context:PageContext):WebMCPTool[] {
  const selected=context.selected!;
  const session=new (webMcpRuntime().WebMcpToolSession)(Boolean(selected));
  const tools:WebMCPTool[]=[];
  try{while(session.need()!=='done'){switch(session.need()){
      case 'base': {tools.push(
    commandTool(registry, "tos.page.inspect-selection", {
      name: "tos.page.inspect-selection",
      title: "Inspect this selected ToS object",
      description: `Read the stable identity, semantic kind, provenance posture, and source references of the currently selected ${selected.semantic_kind || selected.kind} ${selected.id}.`,
      inputSchema: emptySchema,
      annotations: { readOnlyHint: true, untrustedContentHint: true },
    }, context.revision, compactSelectionResult),
    commandTool(registry, "tos.page.add-research-note", {
      name: "tos.page.add-note-to-selection",
      title: "Add a note to this selected object",
      description: `Attach a local research note specifically to selected object ${selected.id}. The captured page revision prevents the note from following a later selection.`,
      inputSchema: objectSchema({ text: { type: "string", minLength: 1, maxLength: 2000 } }, ["text"]),
      annotations: { readOnlyHint: false, untrustedContentHint: true },
    }, context.revision, compactWorkspaceMutation, (input) => ({ ...input, target_id: selected.id })),
    commandTool(registry, "tos.page.stage-proposal", {
      name: "tos.page.stage-proposal",
      title: "Stage a traceable proposal from this selection",
      description: `Stage a local, exportable proposal anchored to ${selected.id}. It remains pending scoped review by a competent authorized human or agent, never writes to source, and never changes canon.`,
      inputSchema: objectSchema({
        kind: { type: "string", enum: ["relation", "interpretation", "metadata_correction", "source_route", "concept_enrichment"] },
        statement: { type: "string", minLength: 1, maxLength: 2000 },
        from_id: { type: "string", maxLength: 256 },
        to_id: { type: "string", maxLength: 256 },
        source_refs: { type: "array", items: { type: "string", maxLength: 512 }, maxItems: 32 },
        evidence_refs: { type: "array", items: { type: "string", maxLength: 512 }, maxItems: 32 },
        confidence: { type: "string", enum: ["low", "medium", "high", "unknown"] },
      }, ["kind", "statement"]),
      annotations: { readOnlyHint: false, untrustedContentHint: true },
    }, context.revision, compactWorkspaceMutation, (input) => ({ ...input, target_id: selected.id, actor_origin: "agent" })),
    );session.emitted();break;}
      case 'evidence-tools': {tools.push(
      commandTool(registry, "tos.page.inspect-epistemic", {
        name: "tos.page.inspect-epistemic",
        title: "Open Evidence Lens for this selection",
        description: `Show why the selected ${selected.kind} ${selected.id} may be presented, which owner routes support it, and which conclusions remain forbidden.`,
        inputSchema: objectSchema({ limit: { type: "integer", minimum: 1, maximum: 200 } }),
        annotations: { readOnlyHint: false, untrustedContentHint: true },
      }, context.revision, compactEvidenceResult),
      commandTool(registry, "tos.page.compare-readings", {
        name: "tos.page.compare-readings",
        title: "Compare readings around this selection",
        description: `Show the selected reading beside projected challenges, contextual support, evidence gaps, and review posture for ${selected.id}. This organizes evidence but does not adjudicate it.`,
        inputSchema: objectSchema({ limit: { type: "integer", minimum: 1, maximum: 80 } }),
        annotations: { readOnlyHint: false, untrustedContentHint: true },
      }, context.revision, compactReadingComparison),
    );session.emitted();break;}
      case 'mutation-tools': {tools.push(
      commandTool(registry, "tos.page.add-session-hypothesis", {
        name: "tos.page.add-session-hypothesis",
        title: "Add a hypothesis for this relation",
        description: `Add a visibly non-canonical session hypothesis between ${selected.from_id} and ${selected.to_id}, anchored to selected edge ${selected.id}.`,
        inputSchema: objectSchema({
          statement: { type: "string", minLength: 1, maxLength: 1000 },
          predicate_label: { type: "string", maxLength: 120 },
        }, ["statement"]),
        annotations: { readOnlyHint: false, untrustedContentHint: true },
      }, context.revision, compactWorkspaceMutation),
      commandTool(registry, "tos.page.exclude-selected-edge", {
        name: "tos.page.exclude-selected-edge",
        title: "Exclude this edge from the research view",
        description: `Exclude selected edge ${selected.id} from this local session and record the exclusion in its journal.`,
        inputSchema: emptySchema,
        annotations: { readOnlyHint: false },
      }, context.revision, compactWorkspaceMutation),
      commandTool(registry, "tos.page.save-route-comparison", {
        name: "tos.page.save-route-comparison",
        title: "Save the current route for comparison",
        description: "Save the visible direct relation or current alternative-path packet as a bounded local comparison snapshot.",
        inputSchema: objectSchema({ label: { type: "string", maxLength: 120 } }),
        annotations: { readOnlyHint: false, untrustedContentHint: true },
      }, context.revision, compactWorkspaceMutation),
    );session.emitted();break;}
      case 'neighborhood-tool': {tools.push(
      commandTool(registry, "tos.page.show-neighborhood", {
        name: "tos.page.show-neighborhood",
        title: "Show this node's neighborhood",
        description: `Show the filtered neighborhood of the currently selected node ${selected.id} in the shared graph.`,
        inputSchema: objectSchema({ depth: { type: "integer", minimum: 1, maximum: 3 } }),
        annotations: { readOnlyHint: false },
      }, context.revision, compactNeighborhoodResult),
    );session.emitted();break;}
      case 'start-tool': {tools.push(commandTool(registry, "tos.page.start-path", {
        name: "tos.page.start-path",
        title: "Start a path from this node",
        description: `Use the currently selected node ${selected.id} as the deictic start of the next path query.`,
        inputSchema: emptySchema,
        annotations: { readOnlyHint: false },
      }, context.revision),
    );session.emitted();break;}
      case 'find-tool': {tools.push(
        commandTool(registry, "tos.page.find-path", {
          name: "tos.page.find-path-to-selection",
          title: "Find paths to this node",
          description: `Find deterministic paths from ${context.path_start_node_id} to the currently selected node ${selected.id}.`,
          inputSchema: objectSchema({
            direction: { type: "string", enum: ["outgoing", "incoming", "either"], default: "outgoing" },
            max_depth: { type: "integer", minimum: 1, maximum: 8 },
            alternative_limit: { type: "integer", minimum: 1, maximum: 5 },
            excluded_edge_ids: { type: "array", items: { type: "string" }, maxItems: 64 },
            constrain_to_view: { type: "boolean" },
          }),
          annotations: { readOnlyHint: false },
        }, context.revision, compactPathResult),
      );session.emitted();break;}
      case 'reroute-tool': {tools.push(
      commandTool(registry, "tos.page.reroute-without-selection", {
        name: "tos.page.reroute-without-selection",
        title: "Find alternatives without this edge",
        description: `Find deterministic alternative paths from ${selected.from_id} to ${selected.to_id}, excluding the currently selected edge ${selected.id}.`,
        inputSchema: objectSchema({
          direction: { type: "string", enum: ["outgoing", "incoming", "either"], default: "outgoing" },
          max_depth: { type: "integer", minimum: 1, maximum: 8 },
          alternative_limit: { type: "integer", minimum: 1, maximum: 5 },
          constrain_to_view: { type: "boolean" },
        }),
        annotations: { readOnlyHint: false },
      }, context.revision, compactPathResult),
    );session.emitted();break;}
      case 'evidence-mode-first':session.observe_equal(context.mode===session.literal());break;
      case 'evidence-mode-corpus':session.observe_equal(context.mode===session.literal());break;
      case 'evidence-view':session.observe_equal(context.view_id===session.literal());break;
      case 'evidence-kind-node':session.observe_equal(selected.kind===session.literal());break;
      case 'evidence-kind-edge':session.observe_equal(selected.kind===session.literal());break;
      case 'mutation-kind':session.observe_equal(selected.kind===session.literal());break;
      case 'philosophy-mode':session.observe_equal(context.mode===session.literal());break;
      case 'node-kind':session.observe_equal(selected.kind===session.literal());break;
      case 'reroute-kind':session.observe_equal(selected.kind===session.literal());break;
      case 'evidence-reroutable':session.observe_equal(selected.reroutable===false);break;
      case 'mutation-reroutable':session.observe_equal(selected.reroutable===false);break;
      case 'start-available':session.observe_equal(selected.path_available===false);break;
      case 'find-available':session.observe_equal(selected.path_available===false);break;
      case 'reroute-reroutable':session.observe_equal(selected.reroutable===false);break;
      case 'mutation-from':session.observe_truthy(Boolean(selected.from_id));break;
      case 'mutation-to':session.observe_truthy(Boolean(selected.to_id));break;
      case 'path-start':session.observe_truthy(Boolean(context.path_start_node_id));break;
      case 'reroute-from':session.observe_truthy(Boolean(selected.from_id));break;
      case 'reroute-to':session.observe_truthy(Boolean(selected.to_id));break;
      case 'evidence-available': {const value=selected.evidence_available;session.observe_optional(Boolean(value),value===null||value===undefined);break;}
      case 'layers':session.observe_equal(context.active_layers.length===0);break;
      case 'predicates':session.observe_equal(context.active_predicates.length===0);break;
      case 'path-distinct':session.observe_equal(context.path_start_node_id===selected.id);break;
      default:throw new Error('Unknown maintained WebMCP tool phase');

  }}return tools;}finally{session.free();}
}

export function createWebMCPAdapter(
  registry: PageCommandRegistry,
  targetDocument: WebMCPDocument,
  allowedCommands?: ReadonlySet<PageCommandId>,
) {
  const available = (tool: WebMCPTool): boolean => !allowedCommands || allowedCommands.has(toolCommands.get(tool)!);
  let stableController: AbortController | null = null;
  let dynamicController: AbortController | null = null;
  let unsubscribe: (() => void) | null = null;
  let refreshGeneration = 0;
  let refreshQueue = Promise.resolve();
  let registrationError: string | null = null;
  let probeTimer: ReturnType<typeof setTimeout> | null = null;
  let probeAttempts = 0;
  let stableToolCount = 0;
  let selectionToolCount = 0;
  const statusListeners = new Set<(status: WebMCPStatus) => void>();

  const status = (): WebMCPStatus => {
    const registered = Boolean(stableController && !stableController.signal.aborted);
    const currentStableToolCount = registered ? stableToolCount : 0;
    const currentSelectionToolCount = registered ? selectionToolCount : 0;
    return {
      supported: Boolean(targetDocument.modelContext),
      registered,
      stable_tool_count: currentStableToolCount,
      selection_tool_count: currentSelectionToolCount,
      tool_count: currentStableToolCount + currentSelectionToolCount,
      context_revision: registry.context().revision,
      registration_error: registrationError,
    };
  };

  const publishStatus = (): void => {
    const current = status();
    statusListeners.forEach((listener) => listener(current));
  };

  const registerAll = async (tools: WebMCPTool[], controller: AbortController): Promise<void> => {
    const modelContext = targetDocument.modelContext;
    if (!modelContext) return;
    for (const tool of tools) {
      await modelContext.registerTool(tool, { signal: controller.signal });
    }
  };

  const refresh = (context: PageContext = registry.context()): Promise<void> => {
    const generation = ++refreshGeneration;
    refreshQueue = refreshQueue.then(async () => {
      if (!targetDocument.modelContext || generation !== refreshGeneration) return;
      dynamicController?.abort();
      const controller = new AbortController();
      dynamicController = controller;
      try {
        const tools = dynamicTools(registry, context).filter(available);
        await registerAll(tools, controller);
        selectionToolCount = tools.length;
        registrationError = null;
      } catch (error) {
        controller.abort();
        selectionToolCount = 0;
        registrationError = String(error);
      } finally {
        publishStatus();
      }
    });
    return refreshQueue;
  };

  const start = async (): Promise<void> => {
    if (stableController) return;
    if (!targetDocument.modelContext) {
      stableToolCount = 0;
      selectionToolCount = 0;
      publishStatus();
      if (probeAttempts < 20 && !probeTimer) {
        probeAttempts += 1;
        probeTimer = setTimeout(() => {
          probeTimer = null;
          void start();
        }, 250);
      }
      return;
    }
    stableController = new AbortController();
    try {
      const tools = stableTools(registry).filter(available);
      await registerAll(tools, stableController);
      stableToolCount = tools.length;
      unsubscribe = registry.subscribe((context) => void refresh(context));
      await refresh();
    } catch (error) {
      registrationError = String(error);
      stableController.abort();
      stableController = null;
      stableToolCount = 0;
      selectionToolCount = 0;
      publishStatus();
    }
  };

  const stop = (): void => {
    refreshGeneration += 1;
    unsubscribe?.();
    unsubscribe = null;
    dynamicController?.abort();
    stableController?.abort();
    if (probeTimer) clearTimeout(probeTimer);
    probeTimer = null;
    dynamicController = null;
    stableController = null;
    stableToolCount = 0;
    selectionToolCount = 0;
    publishStatus();
  };

  const subscribeStatus = (listener: (status: WebMCPStatus) => void): (() => void) => {
    statusListeners.add(listener);
    listener(status());
    return () => statusListeners.delete(listener);
  };

  return { start, stop, refresh, status, subscribeStatus };
}
