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
  return {
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
}

function compactSelectionResult(result: Record<string, unknown>): unknown {
  const context = result.context as Record<string, unknown> | undefined;
  const selection = result.value as Record<string, unknown> | undefined;
  return {
    selection: selection ? {
      id: clipped(selection.id, 96),
      page_kind: clipped(selection.kind, 24),
      semantic_kind: clipped(selection.semantic_kind || selection.kind, 48),
      label: clipped(selection.label, 120),
      subtitle: clipped(selection.subtitle, 100),
      from_id: clipped(selection.from_id, 96) || undefined,
      to_id: clipped(selection.to_id, 96) || undefined,
      predicate_id: clipped(selection.predicate_id, 64) || undefined,
      source_refs: Array.isArray(selection.source_refs) ? selection.source_refs.slice(0, 3).map((ref) => clipped(ref, 160)) : [],
      authority_posture: clipped(selection.authority_posture, 48) || undefined,
      review_posture: clipped(selection.review_posture, 48) || undefined,
      canon_status: clipped(selection.canon_status, 48) || undefined,
      confidence: clipped(selection.confidence, 48) || undefined,
    } : null,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
  };
}

function compactPageContext(value: Record<string, unknown>): unknown {
  const selected = value.selected as Record<string, unknown> | undefined;
  return {
    schema: value.schema,
    revision: value.revision,
    mode: value.mode,
    view_id: clipped(value.view_id, 96),
    graph_mode: value.graph_mode,
    selected: selected ? {
      id: clipped(selected.id, 96),
      page_kind: clipped(selected.kind, 24),
      semantic_kind: clipped(selected.semantic_kind || selected.kind, 48),
      label: clipped(selected.label, 120),
      from_id: clipped(selected.from_id, 96) || undefined,
      to_id: clipped(selected.to_id, 96) || undefined,
      source_refs: Array.isArray(selected.source_refs) ? selected.source_refs.slice(0, 3).map((ref) => clipped(ref, 140)) : [],
    } : null,
    path_start_node_id: clipped(value.path_start_node_id, 96) || null,
    active_layers: Array.isArray(value.active_layers) ? value.active_layers.slice(0, 16) : [],
    active_predicates: Array.isArray(value.active_predicates) ? value.active_predicates.slice(0, 16) : [],
    research_workspace: value.research_workspace,
    deep_link: clipped(value.deep_link, 240),
    pending_command_ids: Array.isArray(value.pending_command_ids) ? value.pending_command_ids.slice(0, 8) : [],
  };
}

function compactSearchResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const results = Array.isArray(value?.results) ? value.results as Array<Record<string, unknown>> : [];
  return {
    query: clipped(value?.query, 160),
    result_count: Number(value?.result_count || 0),
    results: results.slice(0, 6).map((item) => ({
      id: clipped(item.id, 96),
      kind: clipped(item.semantic_kind || item.kind, 48),
      label: clipped(item.label, 80),
      posture: clipped(item.review_posture || item.canon_status || item.authority_posture, 56),
      summary: clipped(item.summary, 72),
    })),
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
    next_action: "select one result by stable id",
  };
}

function compactSourceGapResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const gaps = Array.isArray(value?.gaps) ? value.gaps as Array<Record<string, unknown>> : [];
  return {
    query: clipped(value?.query, 160),
    result_count: Number(value?.result_count || 0),
    gaps: gaps.slice(0, 6).map((item) => ({
      id: clipped(item.id, 140),
      label: clipped(item.label, 160),
      status: clipped(item.review_posture || item.authority_posture, 64),
      summary: clipped(item.summary, 180),
    })),
    authority_note: clipped(value?.authority_note, 300),
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
    next_actions: ["use web research to verify a lawful current route", "select one gap", "stage a source_route proposal"],
  };
}

function compactEvidenceResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const summary = value?.agent_summary as Record<string, unknown> | undefined;
  if (!summary) return result;
  const context = result.context as Record<string, unknown> | undefined;
  return {
    ...summary,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
  };
}

function clipped(value: unknown, maximum = 96): string {
  const output = typeof value === "string" ? value : "";
  return output.length > maximum ? `${output.slice(0, maximum - 1)}…` : output;
}

function compactPathResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  if (!value) return result;
  const paths = Array.isArray(value.paths) ? value.paths as Array<Record<string, unknown>> : [];
  const first = paths[0] || {};
  const nodeIds = Array.isArray(first.node_ids) ? first.node_ids.map((id) => clipped(id)).slice(0, 10) : [];
  const edgeIds = Array.isArray(first.edge_ids) ? first.edge_ids : [];
  const context = result.context as Record<string, unknown> | undefined;
  return {
    finding: value.found ? "alternative route found and shown on the page" : "no route found within the requested bounds",
    found: value.found === true,
    from_id: clipped(value.from_id),
    to_id: clipped(value.to_id),
    route_count: Number(value.path_count || paths.length || 0),
    first_route_node_ids: nodeIds,
    first_route_edge_count: edgeIds.length,
    excluded_edge_count: Array.isArray(value.excluded_edge_ids) ? value.excluded_edge_ids.length : 0,
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
    next_actions: value.found ? ["save the route for comparison", "inspect its uncertain edges"] : ["widen the route bounds", "restore an excluded edge"],
  };
}

function compactNeighborhoodResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const node = value?.node as Record<string, unknown> | undefined;
  const neighbors = Array.isArray(value?.neighbors) ? value.neighbors as Array<Record<string, unknown>> : [];
  const edges = Array.isArray(value?.edges) ? value.edges as Array<Record<string, unknown>> : [];
  return {
    selection: {
      id: clipped(node?.node_id || node?.id, 96),
      label: clipped(node?.label || node?.title, 120),
    },
    neighbor_count: neighbors.length,
    relation_count: edges.length,
    neighbors: neighbors.slice(0, 6).map((item) => ({
      id: clipped(item.node_id || item.id, 72),
      label: clipped(item.label || item.title, 72),
      kind: clipped(item.node_type, 36),
    })),
    predicates: Array.isArray(value?.predicates) ? value.predicates.slice(0, 12) : [],
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
  };
}

function compactReadingComparison(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const selection = value?.selection as Record<string, unknown> | undefined;
  const readings = Array.isArray(value?.competing_readings) ? value.competing_readings as Array<Record<string, unknown>> : [];
  return {
    schema: value?.schema,
    selection: selection ? { id: clipped(selection.id, 96), kind: clipped(selection.semantic_kind || selection.kind, 48), label: clipped(selection.label, 120) } : null,
    posture: value?.posture,
    can_conclude: value?.can_conclude === true,
    competing_reading_count: Number(value?.competing_reading_count || 0),
    competing_readings: readings.slice(0, 4).map((reading) => ({
      id: clipped(reading.id, 80),
      label: clipped(reading.label, 96),
      predicate_id: clipped(reading.predicate_id, 48),
      review_posture: clipped(reading.review_posture, 48) || "unresolved",
      source_refs: Array.isArray(reading.source_refs) ? reading.source_refs.slice(0, 2).map((ref) => clipped(ref, 120)) : [],
    })),
    gaps: Array.isArray(value?.gaps) ? value.gaps.slice(0, 4).map((gap) => clipped(gap, 120)) : [],
    authority_note: clipped(value?.authority_note, 180),
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
    next_actions: ["inspect a competing relation", "stage a traceable local proposal"],
  };
}

function compactWorkspaceMutation(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const context = result.context as Record<string, unknown> | undefined;
  const summary = value?.summary || context?.research_workspace;
  const hypothesis = value?.hypothesis as Record<string, unknown> | undefined;
  const route = value?.route as Record<string, unknown> | undefined;
  const proposal = value?.proposal as Record<string, unknown> | undefined;
  return {
    changed: value?.changed ?? value?.added ?? value?.imported ?? true,
    ...(value?.excluded_edge_id ? { excluded_edge_id: clipped(value.excluded_edge_id) } : {}),
    ...(hypothesis ? { hypothesis: { id: clipped(hypothesis.id), title: clipped(hypothesis.title, 140), posture: hypothesis.posture } } : {}),
    ...(route ? { route: { id: clipped(route.id), label: clipped(route.label, 140), node_count: Array.isArray(route.nodeIds) ? route.nodeIds.length : 0, edge_count: Array.isArray(route.edgeIds) ? route.edgeIds.length : 0 } } : {}),
    ...(proposal ? { proposal: {
      id: clipped(proposal.id, 96),
      kind: clipped(proposal.kind, 48),
      statement: clipped(proposal.statement, 180),
      status: proposal.reviewStatus || proposal.review_status,
      digest: clipped(proposal.digest, 96),
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
  const hypotheses = Array.isArray(packet?.hypotheses) ? packet.hypotheses as Array<Record<string, unknown>> : [];
  const proposals = Array.isArray(packet?.proposals) ? packet.proposals as Array<Record<string, unknown>> : [];
  const routes = Array.isArray(packet?.route_snapshots) ? packet.route_snapshots as Array<Record<string, unknown>> : [];
  const notes = Array.isArray(packet?.notes) ? packet.notes as Array<Record<string, unknown>> : [];
  const journal = Array.isArray(packet?.journal) ? packet.journal as Array<Record<string, unknown>> : [];
  return {
    research_workspace: context?.research_workspace,
    selected_lens: packet?.selected_lens || null,
    hypothesis_preview: hypotheses.slice(-1).map((item) => ({ id: clipped(item.id, 72), title: clipped(item.title, 80), body: clipped(item.body, 96), posture: item.posture })),
    proposal_preview: proposals.slice(-2).map((item) => ({ id: clipped(item.id, 72), kind: item.kind, statement: clipped(item.statement, 120), review_status: item.review_status, digest: clipped(item.digest, 80) })),
    excluded_edge_ids: (Array.isArray(packet?.excluded_edge_ids) ? packet.excluded_edge_ids : []).slice(-3).map((id) => clipped(id, 72)),
    route_preview: routes.slice(-2).map((item) => ({ label: clipped(item.label, 80), node_count: Array.isArray(item.node_ids) ? item.node_ids.length : 0, edge_count: Array.isArray(item.edge_ids) ? item.edge_ids.length : 0 })),
    note_preview: notes.slice(-1).map((item) => ({ body: clipped(item.body, 96), target_id: clipped(item.target_id, 72) })),
    recent_actions: journal.slice(-3).map((item) => ({ action: clipped(item.action, 48), target_id: clipped(item.target_id, 72) })),
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
    available: value?.available === true,
    reason: clipped(value?.reason, 180) || null,
    publication_posture: value?.publication_posture,
    source: source ? {
      occurrence_id: clipped(source.occurrence_id || source.id, 96),
      language: clipped(source.language, 24),
      surface: clipped(source.surface || source.text, 160),
      source_ref: clipped(source.source_ref, 180),
    } : null,
    task_schema: task?.schema_version || task?.schema,
    page_updated: true,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
    next_action: value?.available === true ? "perform the source-bound analysis and preserve citations" : "install the local source-bound provider",
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

function dynamicTools(registry: PageCommandRegistry, context: PageContext): WebMCPTool[] {
  const selected = context.selected;
  if (!selected) return [];
  const tools: WebMCPTool[] = [
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
      description: `Stage a local, exportable proposal anchored to ${selected.id}. It remains pending human review, never writes to source, and never changes canon.`,
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
  ];
  const evidenceAvailable = context.mode === "philosophy"
    || (context.mode === "corpus" && context.view_id === "route-graph");
  if (!evidenceAvailable) return tools;
  if ((selected.kind === "node" || selected.kind === "edge") && selected.reroutable !== false) {
    tools.push(
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
    );
  }
  if (
    selected.kind === "edge" &&
    selected.reroutable !== false &&
    selected.from_id &&
    selected.to_id
  ) {
    tools.push(
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
    );
  }
  if (context.mode !== "philosophy") return tools;
  if (context.active_layers.length === 0 || context.active_predicates.length === 0) return tools;
  if (selected.kind === "node") {
    tools.push(
      commandTool(registry, "tos.page.show-neighborhood", {
        name: "tos.page.show-neighborhood",
        title: "Show this node's neighborhood",
        description: `Show the filtered neighborhood of the currently selected node ${selected.id} in the shared graph.`,
        inputSchema: objectSchema({ depth: { type: "integer", minimum: 1, maximum: 3 } }),
        annotations: { readOnlyHint: false },
      }, context.revision, compactNeighborhoodResult),
      commandTool(registry, "tos.page.start-path", {
        name: "tos.page.start-path",
        title: "Start a path from this node",
        description: `Use the currently selected node ${selected.id} as the deictic start of the next path query.`,
        inputSchema: emptySchema,
        annotations: { readOnlyHint: false },
      }, context.revision),
    );
    if (context.path_start_node_id && context.path_start_node_id !== selected.id) {
      tools.push(
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
      );
    }
  }
  if (
    selected.kind === "edge" &&
    selected.reroutable !== false &&
    selected.from_id &&
    selected.to_id
  ) {
    tools.push(
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
    );
  }
  return tools;
}

export function createWebMCPAdapter(
  registry: PageCommandRegistry,
  targetDocument: WebMCPDocument,
) {
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
        const tools = dynamicTools(registry, context);
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
      const tools = stableTools(registry);
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
