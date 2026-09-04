import { describe, expect, it } from "vitest";
import { agentSurfaceState, PRODUCT_DEMO_PROMPTS } from "./product-shell";

describe("WebMCP product shell", () => {
  it("distinguishes connected, connecting, failed, and graceful fallback states", () => {
    const base = {
      supported: true,
      registered: false,
      stable_tool_count: 0,
      selection_tool_count: 0,
      tool_count: 0,
      context_revision: 4,
      registration_error: null,
    };
    expect(agentSurfaceState(base).state).toBe("connecting");
    expect(agentSurfaceState({ ...base, supported: false }).state).toBe("unavailable");
    expect(agentSurfaceState({ ...base, registration_error: "registration failed" }).state).toBe("error");
    expect(agentSurfaceState({
      ...base,
      registered: true,
      stable_tool_count: 10,
      selection_tool_count: 3,
      tool_count: 13,
    })).toMatchObject({ state: "connected", toolCount: 13, selectionToolCount: 3, contextRevision: 4 });
  });

  it("ships three bounded bilingual demo prompts without canon writeback", () => {
    for (const language of ["en", "ru"] as const) {
      expect(PRODUCT_DEMO_PROMPTS[language]).toHaveLength(3);
      const prompts = PRODUCT_DEMO_PROMPTS[language].map((item) => item.prompt).join("\n");
      expect(prompts).toMatch(/Evidence Lens|основан|основании/i);
      expect(prompts).toMatch(/alternative|альтернатив/i);
      expect(prompts).toMatch(/hypothesis|гипотез/i);
      expect(prompts).toMatch(/human review|человеческого review/i);
      expect(prompts).toMatch(/canon|канон/i);
    }
  });
});
