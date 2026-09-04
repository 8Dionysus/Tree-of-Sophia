import type { WebMCPStatus } from "./webmcp";

export type ProductLanguage = "en" | "ru";
export type AgentSurfaceStateName = "connected" | "connecting" | "unavailable" | "error";

export type AgentSurfaceModel = {
  state: AgentSurfaceStateName;
  toolCount: number;
  selectionToolCount: number;
  contextRevision: number;
  error: string | null;
};

export type ProductDemoPrompt = {
  id: "evidence" | "alternative" | "hypothesis";
  title: string;
  prompt: string;
};

export const PRODUCT_DEMO_PROMPTS: Record<ProductLanguage, ProductDemoPrompt[]> = {
  en: [
    {
      id: "evidence",
      title: "Check the evidence",
      prompt: "Open Evidence Lens for this relation and tell me what it is grounded in, what remains unresolved, and what we may honestly conclude.",
    },
    {
      id: "alternative",
      title: "Test an alternative",
      prompt: "Find an alternative route without this selected edge, show it on the map, and save both routes for comparison.",
    },
    {
      id: "hypothesis",
      title: "Stage a proposal",
      prompt: "Compare the readings, preserve this interpretation as a local hypothesis, then stage a traceable interpretation proposal pending human review. Do not change source or canon.",
    },
  ],
  ru: [
    {
      id: "evidence",
      title: "Проверить основание",
      prompt: "Открой Evidence Lens для этой связи и покажи, на чём она основана, что остаётся нерешённым и какой вывод мы вправе сделать.",
    },
    {
      id: "alternative",
      title: "Проверить альтернативу",
      prompt: "Найди альтернативный маршрут без выбранного ребра, покажи его на карте и сохрани оба маршрута для сравнения.",
    },
    {
      id: "hypothesis",
      title: "Подготовить предложение",
      prompt: "Сравни прочтения, сохрани эту интерпретацию как локальную гипотезу и подготовь прослеживаемое предложение, ожидающее человеческого review. Не меняй источник или канон.",
    },
  ],
};

export function agentSurfaceState(status: WebMCPStatus): AgentSurfaceModel {
  const state: AgentSurfaceStateName = status.registration_error
    ? "error"
    : !status.supported
      ? "unavailable"
      : status.registered
        ? "connected"
        : "connecting";
  return {
    state,
    toolCount: status.tool_count,
    selectionToolCount: status.selection_tool_count,
    contextRevision: status.context_revision,
    error: status.registration_error,
  };
}
