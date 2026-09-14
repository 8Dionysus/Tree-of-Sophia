export type KnowledgeSearchMode = "indexed" | "compressed";

export function chooseKnowledgeSearchMode(capabilities: unknown, requested?: unknown): KnowledgeSearchMode {
  if (requested !== undefined && requested !== "indexed" && requested !== "compressed") {
    throw new Error(`knowledge search mode must be indexed or compressed: ${String(requested)}`);
  }
  const modes = capabilities && typeof capabilities === "object" && !Array.isArray(capabilities)
    ? (capabilities as { modes?: unknown }).modes : null;
  const available = (mode: KnowledgeSearchMode) => {
    if (!modes || typeof modes !== "object" || Array.isArray(modes)) return false;
    const descriptor = (modes as Record<string, unknown>)[mode];
    return Boolean(descriptor && typeof descriptor === "object" && !Array.isArray(descriptor)
      && (descriptor as { available?: unknown }).available === true);
  };
  if (requested !== undefined) {
    if (!available(requested)) throw new Error(`knowledge search mode unavailable: ${requested}`);
    return requested;
  }
  if (available("indexed")) return "indexed";
  if (available("compressed")) return "compressed";
  throw new Error("knowledge search unavailable: indexed and compressed engines are unavailable");
}
