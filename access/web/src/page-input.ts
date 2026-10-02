import {pageIntegerRule, pageCursorRule, pageDirectionRule} from "./query-operations";

export type PageIntegerProfile = "page-knowledge-limit" | "gaps-limit" | "page-word-rank"
  | "neighborhood-depth" | "page-path-depth" | "page-path-alternatives"
  | "page-reroute-alternatives" | "epistemic-limit" | "page-compare-limit";

export function pageCommandInteger(input: Record<string, unknown>, key: string, profile: PageIntegerProfile): number {
  return pageIntegerRule(Number(input[key]), profile);
}
export function pageCommandOpaqueString(input: Record<string, unknown>, key: string): string | undefined {
  const value = input[key];
  const tag = value === undefined || value === null ? 0 : typeof value === "string" ? 1 : 2;
  const action = pageCursorRule(tag, typeof value === "string" ? value.length : 0);
  if (action === 0) return undefined;
  if (action === 2) throw new Error(`${key} must be a string`);
  return value as string;
}
export function pageCommandDirection(input: Record<string, unknown>): "outgoing" | "incoming" | "either" {
  const value = input.direction ?? "outgoing";
  const direction = (value === undefined || value === null ? "" : String(value)).trim();
  const units = new Uint16Array(direction.length);
  for (let i = 0; i < direction.length; i += 1) units[i] = direction.charCodeAt(i);
  if (!pageDirectionRule(units)) throw new Error("direction must be outgoing, incoming, or either");
  return direction as "outgoing" | "incoming" | "either";
}
