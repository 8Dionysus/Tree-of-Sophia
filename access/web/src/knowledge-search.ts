import {nativeLower, nativeStrip} from '../../shared/native-unicode.ts';

export type KnowledgeSearchMode = "indexed" | "compressed";

export function chooseKnowledgeSearchMode(capabilities: unknown, requested?: unknown, query?: string): KnowledgeSearchMode {
  if (requested !== undefined && requested !== "indexed" && requested !== "compressed") {
    throw new Error(`knowledge search mode must be indexed or compressed: ${String(requested)}`);
  }
  const modes = capabilities && typeof capabilities === "object" && !Array.isArray(capabilities)
    ? (capabilities as { modes?: unknown }).modes : null;
  const descriptor = (mode: KnowledgeSearchMode): Record<string, unknown> | null => {
    if (!modes || typeof modes !== "object" || Array.isArray(modes)) return null;
    const value = (modes as Record<string, unknown>)[mode];
    return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
  };
  const available = (mode: KnowledgeSearchMode) => descriptor(mode)?.available === true;
  const queryLength = query === undefined ? null : [...nativeLower(nativeStrip(query))].length;
  const supportsQuery = (mode: KnowledgeSearchMode) => {
    const minimum = descriptor(mode)?.min_normalized_query_code_points ?? 1;
    if (!Number.isSafeInteger(minimum) || Number(minimum) < 1) throw new Error(`invalid knowledge search query capability: ${mode}`);
    return queryLength === null || queryLength >= Number(minimum);
  };
  if (requested !== undefined) {
    if (!available(requested)) throw new Error(`knowledge search mode unavailable: ${requested}`);
    if (!supportsQuery(requested)) throw new Error(`knowledge search mode ${requested} requires at least ${descriptor(requested)?.min_normalized_query_code_points ?? 1} normalized query characters`);
    return requested;
  }
  if (available("indexed") && supportsQuery("indexed")) return "indexed";
  if (available("compressed") && supportsQuery("compressed")) return "compressed";
  if (available("indexed") || available("compressed")) throw new Error('knowledge search query is shorter than the minimum supported by the available engines');
  throw new Error("knowledge search unavailable: indexed and compressed engines are unavailable");
}
