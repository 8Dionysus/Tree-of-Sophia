export type KnowledgeSearchMode = "indexed" | "compressed";
type SearchSession = {
  phase(): number; current(): KnowledgeSearchMode; mode(): KnowledgeSearchMode | undefined;
  error_code(): string | undefined; query(present: boolean, units: Uint16Array): void;
  availability(value: boolean): void; minimum(nullish: boolean, numeric: boolean, value: number): void;
  free(): void;
};
let Session: (new(requested: number) => SearchSession) | undefined;
export function installKnowledgeSearchRules(runtime: {BrowserSearchModeSession?: new(requested: number) => SearchSession}) {
  if (typeof runtime.BrowserSearchModeSession !== "function") throw new TypeError("Knowledge search Rust rules are unavailable");
  Session = runtime.BrowserSearchModeSession;
}
export function chooseKnowledgeSearchMode(capabilities: unknown, requested?: unknown, query?: string): KnowledgeSearchMode {
  if (!Session) throw new Error("Knowledge search Rust rules are not installed");
  const session = new Session(requested === undefined ? 0 : requested === "indexed" ? 1 : requested === "compressed" ? 2 : 3);
  try {
    // Requested-mode refusal precedes even reading capabilities.modes.
    if (session.error_code() === "invalid_mode") throw new Error(`knowledge search mode must be indexed or compressed: ${String(requested)}`);
    const modes = capabilities && typeof capabilities === "object" && !Array.isArray(capabilities)
      ? (capabilities as {modes?: unknown}).modes : null;
    const descriptor = (mode: KnowledgeSearchMode): Record<string, unknown> | null => {
      if (!modes || typeof modes !== "object" || Array.isArray(modes)) return null;
      const value = (modes as Record<string, unknown>)[mode];
      return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
    };
    // Keep runtime non-string refusal before descriptor reads, without coercion.
    if (query !== undefined && typeof query !== "string") (query as string).replace(/$/u, "");
    session.query(query !== undefined, query === undefined ? new Uint16Array() : Uint16Array.from({length:query.length}, (_, index) => query.charCodeAt(index)));
    while (session.phase() !== 4) {
      const mode = session.current();
      if (session.phase() === 1 || session.phase() === 3) session.availability(descriptor(mode)?.available === true);
      else {
        const value = descriptor(mode)?.min_normalized_query_code_points;
        session.minimum(value === null || value === undefined, typeof value === "number", typeof value === "number" ? value : 0);
      }
    }
    const mode = session.mode(); if (mode) return mode;
    const current = session.current();
    switch (session.error_code()) {
      case "invalid_capability": throw new Error(`invalid knowledge search query capability: ${current}`);
      case "mode_unavailable": throw new Error(`knowledge search mode unavailable: ${String(requested)}`);
      // This last descriptor access is deliberately fresh for error wording.
      case "query_too_short": throw new Error(`knowledge search mode ${String(requested)} requires at least ${descriptor(current)?.min_normalized_query_code_points ?? 1} normalized query characters`);
      case "no_eligible_mode": throw new Error("knowledge search query is shorter than the minimum supported by the available engines");
      case "engines_unavailable": throw new Error("knowledge search unavailable: indexed and compressed engines are unavailable");
      default: throw new Error("Knowledge search Rust rule failed");
    }
  } finally { session.free(); }
}
